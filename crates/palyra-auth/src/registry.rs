use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use palyra_vault::Vault;

use crate::{
    constants::{
        DEFAULT_EXPIRING_WINDOW_MS, DEFAULT_REFRESH_WINDOW_MS, MAX_PROFILE_COUNT,
        MAX_PROFILE_PAGE_LIMIT,
    },
    error::AuthProfileError,
    models::{
        AuthCredential, AuthExpiryDistribution, AuthHealthReport, AuthHealthSummary,
        AuthProfileHealthState, AuthProfileListFilter, AuthProfileRecord, AuthProfileScope,
        AuthProfileSetRequest, AuthProfilesPage, OAuthRefreshRequest,
    },
    refresh::{
        compute_backoff_ms, evaluate_profile_health, load_secret_utf8, oauth_expires_at,
        persist_secret_utf8, prepare_oauth_refresh_snapshot, sanitize_refresh_error,
        should_attempt_oauth_refresh, update_expiry_distribution, OAuthRefreshAdapter,
        OAuthRefreshOutcome, OAuthRefreshOutcomeKind, OAuthRefreshSnapshot, PreparedOAuthRefresh,
    },
    storage::{
        persist_registry, resolve_registry_path, resolve_state_root, unix_ms_now, RegistryDocument,
    },
    validation::{
        next_profile_updated_at, normalize_agent_id, normalize_document, normalize_profile_id,
        normalize_set_request, profile_matches_filter, profile_merge_key,
    },
};

#[derive(Debug)]
pub struct AuthProfileRegistry {
    registry_path: PathBuf,
    state: Mutex<RegistryDocument>,
}

impl AuthProfileRegistry {
    pub fn open(identity_store_root: &Path) -> Result<Self, AuthProfileError> {
        let state_root = resolve_state_root(identity_store_root)?;
        let registry_path = resolve_registry_path(state_root.as_path())?;
        if let Some(parent) = registry_path.parent() {
            fs::create_dir_all(parent).map_err(|source| AuthProfileError::WriteRegistry {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let mut document = if registry_path.exists() {
            let raw = fs::read_to_string(&registry_path).map_err(|source| {
                AuthProfileError::ReadRegistry { path: registry_path.clone(), source }
            })?;
            toml::from_str::<RegistryDocument>(&raw).map_err(|source| {
                AuthProfileError::ParseRegistry {
                    path: registry_path.clone(),
                    source: Box::new(source),
                }
            })?
        } else {
            RegistryDocument::default()
        };
        normalize_document(&mut document)?;
        persist_registry(registry_path.as_path(), &document)?;

        Ok(Self { registry_path, state: Mutex::new(document) })
    }

    pub fn list_profiles(
        &self,
        filter: AuthProfileListFilter,
    ) -> Result<AuthProfilesPage, AuthProfileError> {
        let guard = self.state.lock().map_err(|_| AuthProfileError::LockPoisoned)?;
        let limit = filter.limit.unwrap_or(100).clamp(1, MAX_PROFILE_PAGE_LIMIT);
        let mut filtered = guard
            .profiles
            .iter()
            .filter(|profile| profile_matches_filter(profile, &filter))
            .cloned()
            .collect::<Vec<_>>();
        let start = if let Some(after) = filter.after_profile_id.as_deref() {
            filtered
                .iter()
                .position(|profile| profile.profile_id == after)
                .map(|index| index.saturating_add(1))
                .ok_or_else(|| AuthProfileError::InvalidField {
                    field: "after_profile_id",
                    message: "cursor does not exist in current result set".to_owned(),
                })?
        } else {
            0
        };
        let mut page = filtered.drain(start..).take(limit.saturating_add(1)).collect::<Vec<_>>();
        let has_more = page.len() > limit;
        if has_more {
            page.truncate(limit);
        }
        Ok(AuthProfilesPage {
            next_after_profile_id: if has_more {
                page.last().map(|profile| profile.profile_id.clone())
            } else {
                None
            },
            profiles: page,
        })
    }

    pub fn get_profile(
        &self,
        profile_id: &str,
    ) -> Result<Option<AuthProfileRecord>, AuthProfileError> {
        let profile_id = normalize_profile_id(profile_id)?;
        let guard = self.state.lock().map_err(|_| AuthProfileError::LockPoisoned)?;
        Ok(guard.profiles.iter().find(|profile| profile.profile_id == profile_id).cloned())
    }

    pub fn set_profile(
        &self,
        request: AuthProfileSetRequest,
    ) -> Result<AuthProfileRecord, AuthProfileError> {
        let normalized = normalize_set_request(request)?;
        let now = unix_ms_now()?;

        let mut guard = self.state.lock().map_err(|_| AuthProfileError::LockPoisoned)?;
        let mut next = guard.clone();
        let mut record = AuthProfileRecord {
            profile_id: normalized.profile_id,
            provider: normalized.provider,
            profile_name: normalized.profile_name,
            scope: normalized.scope,
            credential: normalized.credential,
            created_at_unix_ms: now,
            updated_at_unix_ms: now,
        };
        if let Some(existing) =
            next.profiles.iter_mut().find(|profile| profile.profile_id == record.profile_id)
        {
            record.created_at_unix_ms = existing.created_at_unix_ms;
            *existing = record.clone();
        } else {
            if next.profiles.len() >= MAX_PROFILE_COUNT {
                return Err(AuthProfileError::RegistryLimitExceeded);
            }
            next.profiles.push(record.clone());
        }
        next.profiles.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        persist_registry(self.registry_path.as_path(), &next)?;
        *guard = next;
        Ok(record)
    }

    pub fn delete_profile(&self, profile_id: &str) -> Result<bool, AuthProfileError> {
        let profile_id = normalize_profile_id(profile_id)?;
        let mut guard = self.state.lock().map_err(|_| AuthProfileError::LockPoisoned)?;
        let mut next = guard.clone();
        let before = next.profiles.len();
        next.profiles.retain(|profile| profile.profile_id != profile_id);
        let deleted = next.profiles.len() != before;
        if deleted {
            persist_registry(self.registry_path.as_path(), &next)?;
            *guard = next;
        }
        Ok(deleted)
    }

    pub fn merged_profiles_for_agent(
        &self,
        agent_id: Option<&str>,
    ) -> Result<Vec<AuthProfileRecord>, AuthProfileError> {
        let guard = self.state.lock().map_err(|_| AuthProfileError::LockPoisoned)?;
        if let Some(agent_id_raw) = agent_id {
            let normalized_agent_id = normalize_agent_id(agent_id_raw)?;
            let mut merged = BTreeMap::<String, AuthProfileRecord>::new();
            for profile in &guard.profiles {
                if matches!(profile.scope, AuthProfileScope::Global) {
                    merged.insert(profile_merge_key(profile), profile.clone());
                }
            }
            for profile in &guard.profiles {
                if matches!(
                    profile.scope,
                    AuthProfileScope::Agent { ref agent_id } if agent_id == &normalized_agent_id
                ) {
                    merged.insert(profile_merge_key(profile), profile.clone());
                }
            }
            return Ok(merged.into_values().collect());
        }
        Ok(guard.profiles.clone())
    }

    pub fn health_report(
        &self,
        vault: &Vault,
        agent_id: Option<&str>,
    ) -> Result<AuthHealthReport, AuthProfileError> {
        self.health_report_with_clock(vault, agent_id, unix_ms_now()?, DEFAULT_EXPIRING_WINDOW_MS)
    }

    pub fn health_report_with_clock(
        &self,
        vault: &Vault,
        agent_id: Option<&str>,
        now_unix_ms: i64,
        expiring_window_ms: i64,
    ) -> Result<AuthHealthReport, AuthProfileError> {
        let profiles = self.merged_profiles_for_agent(agent_id)?;
        let mut report = AuthHealthReport {
            summary: AuthHealthSummary::default(),
            expiry_distribution: AuthExpiryDistribution::default(),
            profiles: Vec::with_capacity(profiles.len()),
        };

        for profile in profiles {
            let health = evaluate_profile_health(&profile, vault, now_unix_ms, expiring_window_ms);
            report.summary.total = report.summary.total.saturating_add(1);
            match health.state {
                AuthProfileHealthState::Ok => {
                    report.summary.ok = report.summary.ok.saturating_add(1)
                }
                AuthProfileHealthState::Expiring => {
                    report.summary.expiring = report.summary.expiring.saturating_add(1)
                }
                AuthProfileHealthState::Expired => {
                    report.summary.expired = report.summary.expired.saturating_add(1)
                }
                AuthProfileHealthState::Missing => {
                    report.summary.missing = report.summary.missing.saturating_add(1)
                }
                AuthProfileHealthState::Static => {
                    report.summary.static_count = report.summary.static_count.saturating_add(1)
                }
            }
            update_expiry_distribution(
                &mut report.expiry_distribution,
                health.state,
                health.expires_at_unix_ms,
                now_unix_ms,
            );
            report.profiles.push(health);
        }

        Ok(report)
    }

    pub fn refresh_due_oauth_profiles(
        &self,
        vault: &Vault,
        adapter: &dyn OAuthRefreshAdapter,
        agent_id: Option<&str>,
    ) -> Result<Vec<OAuthRefreshOutcome>, AuthProfileError> {
        self.refresh_due_oauth_profiles_with_clock(
            vault,
            adapter,
            agent_id,
            unix_ms_now()?,
            DEFAULT_REFRESH_WINDOW_MS,
        )
    }

    pub fn refresh_due_oauth_profiles_with_clock(
        &self,
        vault: &Vault,
        adapter: &dyn OAuthRefreshAdapter,
        agent_id: Option<&str>,
        now_unix_ms: i64,
        refresh_window_ms: i64,
    ) -> Result<Vec<OAuthRefreshOutcome>, AuthProfileError> {
        let profiles = self.merged_profiles_for_agent(agent_id)?;
        let mut outcomes = Vec::new();
        for profile in profiles {
            let profile_id = profile.profile_id.clone();
            if let AuthCredential::Oauth { .. } = profile.credential {
                if !should_attempt_oauth_refresh(&profile, vault, now_unix_ms, refresh_window_ms) {
                    outcomes.push(OAuthRefreshOutcome {
                        profile_id: profile_id.clone(),
                        provider: profile.provider.label(),
                        kind: OAuthRefreshOutcomeKind::SkippedNotDue,
                        reason: "refresh skipped because token is not yet due".to_owned(),
                        next_allowed_refresh_unix_ms: None,
                        expires_at_unix_ms: oauth_expires_at(&profile),
                    });
                    continue;
                }
                match prepare_oauth_refresh_snapshot(&profile, now_unix_ms) {
                    PreparedOAuthRefresh::Snapshot(snapshot) => {
                        outcomes.push(self.refresh_oauth_profile_snapshot_with_clock(
                            snapshot,
                            vault,
                            adapter,
                            now_unix_ms,
                        )?)
                    }
                    PreparedOAuthRefresh::Outcome(outcome) => outcomes.push(outcome),
                }
            }
        }
        Ok(outcomes)
    }

    pub fn refresh_oauth_profile(
        &self,
        profile_id: &str,
        vault: &Vault,
        adapter: &dyn OAuthRefreshAdapter,
    ) -> Result<OAuthRefreshOutcome, AuthProfileError> {
        self.refresh_oauth_profile_with_clock(profile_id, vault, adapter, unix_ms_now()?)
    }

    pub fn refresh_oauth_profile_with_clock(
        &self,
        profile_id: &str,
        vault: &Vault,
        adapter: &dyn OAuthRefreshAdapter,
        now_unix_ms: i64,
    ) -> Result<OAuthRefreshOutcome, AuthProfileError> {
        let profile_id = normalize_profile_id(profile_id)?;

        let prepared = {
            let guard = self.state.lock().map_err(|_| AuthProfileError::LockPoisoned)?;
            let profile = guard
                .profiles
                .iter()
                .find(|profile| profile.profile_id == profile_id)
                .ok_or_else(|| AuthProfileError::ProfileNotFound(profile_id.clone()))?;
            prepare_oauth_refresh_snapshot(profile, now_unix_ms)
        };
        match prepared {
            PreparedOAuthRefresh::Snapshot(snapshot) => self
                .refresh_oauth_profile_snapshot_with_clock(snapshot, vault, adapter, now_unix_ms),
            PreparedOAuthRefresh::Outcome(outcome) => Ok(outcome),
        }
    }

    fn refresh_oauth_profile_snapshot_with_clock(
        &self,
        snapshot: OAuthRefreshSnapshot,
        vault: &Vault,
        adapter: &dyn OAuthRefreshAdapter,
        now_unix_ms: i64,
    ) -> Result<OAuthRefreshOutcome, AuthProfileError> {
        let refresh_token = match load_secret_utf8(vault, snapshot.refresh_token_vault_ref.as_str())
        {
            Ok(token) => token,
            Err(_) => {
                return self.persist_refresh_failure(
                    snapshot,
                    now_unix_ms,
                    "refresh token reference is missing or unreadable".to_owned(),
                );
            }
        };

        let client_secret =
            if let Some(client_secret_ref) = snapshot.client_secret_vault_ref.as_deref() {
                match load_secret_utf8(vault, client_secret_ref) {
                    Ok(secret) => Some(secret),
                    Err(_) => {
                        return self.persist_refresh_failure(
                            snapshot,
                            now_unix_ms,
                            "client secret reference is missing or unreadable".to_owned(),
                        );
                    }
                }
            } else {
                None
            };

        let response = adapter.refresh_access_token(&OAuthRefreshRequest {
            provider: snapshot.provider.clone(),
            token_endpoint: snapshot.token_endpoint.clone(),
            client_id: snapshot.client_id.clone(),
            client_secret,
            refresh_token,
            scopes: snapshot.scopes.clone(),
        });
        match response {
            Ok(payload) => {
                if let Some(refresh_token) = payload.refresh_token.as_deref() {
                    if persist_secret_utf8(
                        vault,
                        snapshot.refresh_token_vault_ref.as_str(),
                        refresh_token,
                    )
                    .is_err()
                    {
                        return self.persist_refresh_failure(
                            snapshot,
                            now_unix_ms,
                            "failed to persist rotated refresh token into vault".to_owned(),
                        );
                    }
                }
                if persist_secret_utf8(
                    vault,
                    snapshot.access_token_vault_ref.as_str(),
                    payload.access_token.as_str(),
                )
                .is_err()
                {
                    return self.persist_refresh_failure(
                        snapshot,
                        now_unix_ms,
                        "failed to persist refreshed access token into vault".to_owned(),
                    );
                }
                let computed_expires_at = payload
                    .expires_in_seconds
                    .map(|seconds| {
                        now_unix_ms.saturating_add((seconds as i64).saturating_mul(1_000))
                    })
                    .or(snapshot.expires_at_unix_ms);
                let mut guard = self.state.lock().map_err(|_| AuthProfileError::LockPoisoned)?;
                let mut next = guard.clone();
                let (profile_id, provider, expires_at_unix_ms) = {
                    let profile = next
                        .profiles
                        .iter_mut()
                        .find(|profile| profile.profile_id == snapshot.profile_id)
                        .ok_or_else(|| {
                            AuthProfileError::ProfileNotFound(snapshot.profile_id.clone())
                        })?;
                    let provider = profile.provider.label();
                    let profile_id = profile.profile_id.clone();
                    let AuthCredential::Oauth { expires_at_unix_ms, refresh_state, .. } =
                        &mut profile.credential
                    else {
                        return Ok(OAuthRefreshOutcome {
                            profile_id,
                            provider,
                            kind: OAuthRefreshOutcomeKind::SkippedNotOauth,
                            reason: "profile credential type changed before refresh completed"
                                .to_owned(),
                            next_allowed_refresh_unix_ms: None,
                            expires_at_unix_ms: None,
                        });
                    };
                    *expires_at_unix_ms = computed_expires_at;
                    refresh_state.failure_count = 0;
                    refresh_state.last_error = None;
                    refresh_state.last_attempt_unix_ms = Some(now_unix_ms);
                    refresh_state.last_success_unix_ms = Some(now_unix_ms);
                    refresh_state.next_allowed_refresh_unix_ms = None;
                    profile.updated_at_unix_ms =
                        next_profile_updated_at(profile.updated_at_unix_ms, now_unix_ms);
                    (profile_id, provider, *expires_at_unix_ms)
                };
                persist_registry(self.registry_path.as_path(), &next)?;
                *guard = next;
                Ok(OAuthRefreshOutcome {
                    profile_id,
                    provider,
                    kind: OAuthRefreshOutcomeKind::Succeeded,
                    reason: "oauth access token refreshed".to_owned(),
                    next_allowed_refresh_unix_ms: None,
                    expires_at_unix_ms,
                })
            }
            Err(error) => {
                self.persist_refresh_failure(snapshot, now_unix_ms, sanitize_refresh_error(&error))
            }
        }
    }

    fn persist_refresh_failure(
        &self,
        snapshot: OAuthRefreshSnapshot,
        now_unix_ms: i64,
        reason: String,
    ) -> Result<OAuthRefreshOutcome, AuthProfileError> {
        let mut guard = self.state.lock().map_err(|_| AuthProfileError::LockPoisoned)?;
        let mut next = guard.clone();
        let profile = next
            .profiles
            .iter_mut()
            .find(|profile| profile.profile_id == snapshot.profile_id)
            .ok_or_else(|| AuthProfileError::ProfileNotFound(snapshot.profile_id.clone()))?;
        let provider = profile.provider.label();
        let profile_id = profile.profile_id.clone();
        let (next_allowed, expires_at_unix_ms) = {
            let AuthCredential::Oauth { refresh_state, expires_at_unix_ms, .. } =
                &mut profile.credential
            else {
                return Ok(OAuthRefreshOutcome {
                    profile_id,
                    provider,
                    kind: OAuthRefreshOutcomeKind::SkippedNotOauth,
                    reason: "profile credential type changed before refresh failure persisted"
                        .to_owned(),
                    next_allowed_refresh_unix_ms: None,
                    expires_at_unix_ms: None,
                });
            };
            if profile.updated_at_unix_ms != snapshot.observed_updated_at_unix_ms {
                return Ok(OAuthRefreshOutcome {
                    profile_id,
                    provider,
                    kind: OAuthRefreshOutcomeKind::SkippedCooldown,
                    reason: "stale refresh failure ignored because profile state changed"
                        .to_owned(),
                    next_allowed_refresh_unix_ms: refresh_state.next_allowed_refresh_unix_ms,
                    expires_at_unix_ms: *expires_at_unix_ms,
                });
            }
            let failure_count = snapshot.failure_count.saturating_add(1);
            let backoff_ms = compute_backoff_ms(&snapshot.provider, failure_count);
            let next_allowed = now_unix_ms.saturating_add(backoff_ms as i64);

            refresh_state.failure_count = failure_count;
            refresh_state.last_error = Some(reason.clone());
            refresh_state.last_attempt_unix_ms = Some(now_unix_ms);
            refresh_state.next_allowed_refresh_unix_ms = Some(next_allowed);
            (next_allowed, *expires_at_unix_ms)
        };
        profile.updated_at_unix_ms =
            next_profile_updated_at(profile.updated_at_unix_ms, now_unix_ms);

        persist_registry(self.registry_path.as_path(), &next)?;
        *guard = next;

        Ok(OAuthRefreshOutcome {
            profile_id,
            provider,
            kind: OAuthRefreshOutcomeKind::Failed,
            reason,
            next_allowed_refresh_unix_ms: Some(next_allowed),
            expires_at_unix_ms,
        })
    }
}
