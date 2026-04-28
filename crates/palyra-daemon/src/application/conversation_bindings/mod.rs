use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use palyra_common::redaction::{redact_auth_error, redact_url_segments_in_text};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const BINDING_SCHEMA_VERSION: u32 = 1;
const DEFAULT_IDLE_TIMEOUT_MS: i64 = 8 * 60 * 60 * 1_000;
const DEFAULT_MAX_AGE_MS: i64 = 30 * 24 * 60 * 60 * 1_000;
const MAX_BINDING_LIST_LIMIT: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationBindingKind {
    Main,
    Thread,
    DelegatedRun,
    Acp,
    Routine,
}

impl ConversationBindingKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Thread => "thread",
            Self::DelegatedRun => "delegated_run",
            Self::Acp => "acp",
            Self::Routine => "routine",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationBindingLifecycleState {
    Active,
    Detached,
    Expired,
    Stale,
}

impl ConversationBindingLifecycleState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Detached => "detached",
            Self::Expired => "expired",
            Self::Stale => "stale",
        }
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationBindingLifecycle {
    pub idle_timeout_ms: Option<i64>,
    pub max_age_ms: Option<i64>,
    pub detach_on_parent_reset: bool,
}

impl Default for ConversationBindingLifecycle {
    fn default() -> Self {
        Self {
            idle_timeout_ms: Some(DEFAULT_IDLE_TIMEOUT_MS),
            max_age_ms: Some(DEFAULT_MAX_AGE_MS),
            detach_on_parent_reset: true,
        }
    }
}

impl ConversationBindingLifecycle {
    #[must_use]
    pub fn expires_at(
        &self,
        created_at_unix_ms: i64,
        last_activity_at_unix_ms: i64,
    ) -> Option<i64> {
        let idle_expiry =
            self.idle_timeout_ms.and_then(|ttl| last_activity_at_unix_ms.checked_add(ttl.max(0)));
        let max_age_expiry =
            self.max_age_ms.and_then(|ttl| created_at_unix_ms.checked_add(ttl.max(0)));
        match (idle_expiry, max_age_expiry) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(value), None) | (None, Some(value)) => Some(value),
            (None, None) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationBindingRecord {
    pub schema_version: u32,
    pub binding_id: String,
    pub binding_kind: ConversationBindingKind,
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_identity: Option<String>,
    pub principal: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    pub policy_scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_binding_id: Option<String>,
    pub lifecycle: ConversationBindingLifecycle,
    pub state: ConversationBindingLifecycleState,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
    pub last_activity_at_unix_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_ms: Option<i64>,
}

impl ConversationBindingRecord {
    #[must_use]
    pub fn active(&self, now_unix_ms: i64) -> bool {
        self.state.is_active()
            && self.expires_at_unix_ms.is_none_or(|expires_at| expires_at > now_unix_ms)
    }

    #[must_use]
    pub fn scope_key(&self) -> ConversationBindingScopeKey {
        ConversationBindingScopeKey {
            binding_kind: self.binding_kind,
            channel: self.channel.clone(),
            conversation_id: self.conversation_id.clone(),
            thread_id: self.thread_id.clone(),
            sender_identity: self.sender_identity.clone(),
            principal: self.principal.clone(),
        }
    }

    #[must_use]
    pub fn conflict_scope_key(&self) -> ConversationBindingConflictScopeKey {
        ConversationBindingConflictScopeKey {
            binding_kind: self.binding_kind,
            channel: self.channel.clone(),
            conversation_id: self.conversation_id.clone(),
            thread_id: self.thread_id.clone(),
            sender_identity: self.sender_identity.clone(),
        }
    }

    #[must_use]
    pub fn safe_snapshot_json(&self) -> Value {
        json!({
            "schema_version": self.schema_version,
            "binding_id": self.binding_id,
            "binding_kind": self.binding_kind.as_str(),
            "channel": safe_text(self.channel.as_str()),
            "conversation_id": self.conversation_id.as_deref().map(safe_text),
            "thread_id": self.thread_id.as_deref().map(safe_text),
            "sender_identity": self.sender_identity.as_deref().map(safe_text),
            "principal": safe_text(self.principal.as_str()),
            "session_id": self.session_id,
            "workspace_id": self.workspace_id.as_deref().map(safe_text),
            "policy_scope": self.policy_scope,
            "parent_binding_id": self.parent_binding_id,
            "state": self.state.as_str(),
            "created_at_unix_ms": self.created_at_unix_ms,
            "updated_at_unix_ms": self.updated_at_unix_ms,
            "last_activity_at_unix_ms": self.last_activity_at_unix_ms,
            "expires_at_unix_ms": self.expires_at_unix_ms,
        })
    }

    fn touch(&mut self, now_unix_ms: i64) {
        self.updated_at_unix_ms = now_unix_ms;
        self.last_activity_at_unix_ms = now_unix_ms;
        self.expires_at_unix_ms =
            self.lifecycle.expires_at(self.created_at_unix_ms, self.last_activity_at_unix_ms);
    }

    fn expire(&mut self, now_unix_ms: i64) {
        self.state = ConversationBindingLifecycleState::Expired;
        self.updated_at_unix_ms = now_unix_ms;
        self.expires_at_unix_ms = Some(now_unix_ms);
    }

    fn detach(&mut self, now_unix_ms: i64) {
        self.state = ConversationBindingLifecycleState::Detached;
        self.updated_at_unix_ms = now_unix_ms;
        self.expires_at_unix_ms = Some(now_unix_ms);
    }

    fn mark_stale(&mut self, now_unix_ms: i64) {
        self.state = ConversationBindingLifecycleState::Stale;
        self.updated_at_unix_ms = now_unix_ms;
    }

    fn validate(&self) -> Result<(), ConversationBindingError> {
        ensure_non_empty(self.binding_id.as_str(), "binding_id")?;
        ensure_non_empty(self.channel.as_str(), "channel")?;
        ensure_non_empty(self.principal.as_str(), "principal")?;
        ensure_non_empty(self.session_id.as_str(), "session_id")?;
        ensure_non_empty(self.policy_scope.as_str(), "policy_scope")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConversationBindingScopeKey {
    pub binding_kind: ConversationBindingKind,
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_identity: Option<String>,
    pub principal: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConversationBindingConflictScopeKey {
    pub binding_kind: ConversationBindingKind,
    pub channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationBindingCreateRequest {
    pub binding_kind: ConversationBindingKind,
    pub channel: String,
    pub conversation_id: Option<String>,
    pub thread_id: Option<String>,
    pub sender_identity: Option<String>,
    pub principal: String,
    pub session_id: String,
    pub workspace_id: Option<String>,
    pub policy_scope: String,
    pub parent_binding_id: Option<String>,
    pub lifecycle: ConversationBindingLifecycle,
    pub now_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationBindingResolveRequest {
    pub channel: String,
    pub conversation_id: Option<String>,
    pub thread_id: Option<String>,
    pub sender_identity: Option<String>,
    pub principal: String,
    pub now_unix_ms: i64,
}

impl ConversationBindingResolveRequest {
    #[must_use]
    pub fn scope_key(&self, binding_kind: ConversationBindingKind) -> ConversationBindingScopeKey {
        ConversationBindingScopeKey {
            binding_kind,
            channel: normalize_component(self.channel.as_str()).unwrap_or_default(),
            conversation_id: normalize_optional_component(self.conversation_id.as_deref()),
            thread_id: if matches!(
                binding_kind,
                ConversationBindingKind::Thread | ConversationBindingKind::DelegatedRun
            ) {
                normalize_optional_component(self.thread_id.as_deref())
            } else {
                None
            },
            sender_identity: normalize_optional_component(self.sender_identity.as_deref()),
            principal: normalize_component(self.principal.as_str()).unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConversationBindingListFilter {
    pub channel: Option<String>,
    pub principal: Option<String>,
    pub session_id: Option<String>,
    pub include_inactive: bool,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConversationBindingMutationOutcome {
    pub record: ConversationBindingRecord,
    pub created: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConversationBindingResolution {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<ConversationBindingRecord>,
    pub reason: String,
    pub expired_count: usize,
    pub conflicts: Vec<ConversationBindingConflict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationBindingConflictKind {
    DuplicateActiveBinding,
    StaleThread,
    PrincipalMismatch,
    WorkspaceMismatch,
    ExpiredReferenced,
    ParentMissing,
}

impl ConversationBindingConflictKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateActiveBinding => "duplicate_active_binding",
            Self::StaleThread => "stale_thread",
            Self::PrincipalMismatch => "principal_mismatch",
            Self::WorkspaceMismatch => "workspace_mismatch",
            Self::ExpiredReferenced => "expired_referenced",
            Self::ParentMissing => "parent_missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationBindingConflict {
    pub kind: ConversationBindingConflictKind,
    pub binding_ids: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationBindingRepairAction {
    Detach,
    Rebind,
    Expire,
    Split,
    MarkStale,
}

impl ConversationBindingRepairAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Detach => "detach",
            Self::Rebind => "rebind",
            Self::Expire => "expire",
            Self::Split => "split",
            Self::MarkStale => "mark_stale",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationBindingRepairStep {
    pub binding_id: String,
    pub action: ConversationBindingRepairAction,
    pub automatic: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConversationBindingRepairPlan {
    pub conflicts: Vec<ConversationBindingConflict>,
    pub steps: Vec<ConversationBindingRepairStep>,
    pub safe_to_auto_apply: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConversationBindingExplainReport {
    pub matches: Vec<Value>,
    pub repair_plan: ConversationBindingRepairPlan,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ConversationBindingReconcileReport {
    pub expired_count: usize,
    pub conflict_count: usize,
    pub repair_plan: ConversationBindingRepairPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConversationBindingRepairOutcome {
    pub applied_count: usize,
    pub skipped_count: usize,
    pub records: Vec<ConversationBindingRecord>,
}

#[derive(Debug, Clone)]
pub struct ConversationBindingStore {
    path: PathBuf,
    records: Arc<Mutex<BTreeMap<String, ConversationBindingRecord>>>,
}

impl ConversationBindingStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ConversationBindingError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let envelope = if path.exists() {
            let bytes = fs::read(&path)?;
            if bytes.is_empty() {
                ConversationBindingStoreEnvelope::default()
            } else {
                serde_json::from_slice::<ConversationBindingStoreEnvelope>(bytes.as_slice())?
            }
        } else {
            ConversationBindingStoreEnvelope::default()
        };
        if envelope.schema_version != BINDING_SCHEMA_VERSION {
            return Err(ConversationBindingError::InvalidRecord(format!(
                "unsupported conversation binding schema version {}",
                envelope.schema_version
            )));
        }
        for record in envelope.records.values() {
            record.validate()?;
        }
        let store = Self { path, records: Arc::new(Mutex::new(envelope.records)) };
        store.persist()?;
        Ok(store)
    }

    #[cfg(test)]
    pub fn open_temp() -> Self {
        let path = std::env::temp_dir()
            .join(format!("palyra-conversation-bindings-{}.json", ulid::Ulid::new()));
        Self::open(path).expect("temporary conversation binding store should open")
    }

    pub fn create_or_touch(
        &self,
        request: ConversationBindingCreateRequest,
    ) -> Result<ConversationBindingMutationOutcome, ConversationBindingError> {
        let mut record = build_record(request)?;
        record.validate()?;
        let mut guard = self.lock_records()?;
        let created = !guard.contains_key(record.binding_id.as_str());
        let reason = if let Some(existing) = guard.get_mut(record.binding_id.as_str()) {
            existing.session_id = record.session_id.clone();
            existing.workspace_id = record.workspace_id.clone();
            existing.policy_scope = record.policy_scope.clone();
            existing.parent_binding_id = record.parent_binding_id.clone();
            existing.lifecycle = record.lifecycle.clone();
            existing.state = ConversationBindingLifecycleState::Active;
            existing.touch(record.updated_at_unix_ms);
            record = existing.clone();
            "binding_touched".to_owned()
        } else {
            let reason = "binding_created".to_owned();
            guard.insert(record.binding_id.clone(), record.clone());
            reason
        };
        drop(guard);
        self.persist()?;
        Ok(ConversationBindingMutationOutcome { record, created, reason })
    }

    pub fn resolve(
        &self,
        request: ConversationBindingResolveRequest,
    ) -> Result<ConversationBindingResolution, ConversationBindingError> {
        let expired = self.expire_due(request.now_unix_ms)?;
        let guard = self.lock_records()?;
        let delegated_key = request.scope_key(ConversationBindingKind::DelegatedRun);
        let thread_key = request.scope_key(ConversationBindingKind::Thread);
        let main_key = request.scope_key(ConversationBindingKind::Main);
        let mut candidates = guard
            .values()
            .filter(|record| record.active(request.now_unix_ms))
            .filter(|record| {
                let key = record.scope_key();
                key == delegated_key || key == thread_key || key == main_key
            })
            .cloned()
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .binding_kind
                .cmp(&left.binding_kind)
                .then_with(|| right.last_activity_at_unix_ms.cmp(&left.last_activity_at_unix_ms))
                .then_with(|| left.binding_id.cmp(&right.binding_id))
        });
        let conflicts = detect_conflicts(guard.values(), request.now_unix_ms);
        let record = candidates.into_iter().next();
        let reason = match record.as_ref() {
            Some(value) if value.binding_kind == ConversationBindingKind::DelegatedRun => {
                "delegated_run_binding_resolved"
            }
            Some(value) if value.binding_kind == ConversationBindingKind::Thread => {
                "thread_binding_resolved"
            }
            Some(_) => "main_binding_resolved",
            None => "binding_not_found",
        }
        .to_owned();
        Ok(ConversationBindingResolution {
            record,
            reason,
            expired_count: expired.len(),
            conflicts,
        })
    }

    pub fn touch(
        &self,
        binding_id: &str,
        now_unix_ms: i64,
    ) -> Result<Option<ConversationBindingRecord>, ConversationBindingError> {
        let mut guard = self.lock_records()?;
        let Some(record) = guard.get_mut(binding_id) else {
            return Ok(None);
        };
        record.touch(now_unix_ms);
        let record = record.clone();
        drop(guard);
        self.persist()?;
        Ok(Some(record))
    }

    pub fn list(
        &self,
        filter: ConversationBindingListFilter,
        now_unix_ms: i64,
    ) -> Result<Vec<ConversationBindingRecord>, ConversationBindingError> {
        let guard = self.lock_records()?;
        let limit = filter.limit.unwrap_or(MAX_BINDING_LIST_LIMIT).min(MAX_BINDING_LIST_LIMIT);
        let mut records = guard
            .values()
            .filter(|record| filter.include_inactive || record.active(now_unix_ms))
            .filter(|record| {
                filter
                    .channel
                    .as_deref()
                    .is_none_or(|channel| record.channel.eq_ignore_ascii_case(channel))
            })
            .filter(|record| {
                filter.principal.as_deref().is_none_or(|principal| record.principal == principal)
            })
            .filter(|record| {
                filter
                    .session_id
                    .as_deref()
                    .is_none_or(|session_id| record.session_id == session_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.channel
                .cmp(&right.channel)
                .then_with(|| left.conversation_id.cmp(&right.conversation_id))
                .then_with(|| left.thread_id.cmp(&right.thread_id))
                .then_with(|| left.binding_id.cmp(&right.binding_id))
        });
        records.truncate(limit);
        Ok(records)
    }

    pub fn unbind(
        &self,
        binding_id: &str,
        now_unix_ms: i64,
    ) -> Result<Option<ConversationBindingRecord>, ConversationBindingError> {
        let mut guard = self.lock_records()?;
        let Some(record) = guard.get_mut(binding_id) else {
            return Ok(None);
        };
        record.detach(now_unix_ms);
        let record = record.clone();
        drop(guard);
        self.persist()?;
        Ok(Some(record))
    }

    pub fn expire_due(
        &self,
        now_unix_ms: i64,
    ) -> Result<Vec<ConversationBindingRecord>, ConversationBindingError> {
        let mut guard = self.lock_records()?;
        let mut expired = Vec::new();
        for record in guard.values_mut() {
            if record.state.is_active()
                && record.expires_at_unix_ms.is_some_and(|expires_at| expires_at <= now_unix_ms)
            {
                record.expire(now_unix_ms);
                expired.push(record.clone());
            }
        }
        drop(guard);
        if !expired.is_empty() {
            self.persist()?;
        }
        Ok(expired)
    }

    pub fn reconcile_on_startup(
        &self,
        now_unix_ms: i64,
    ) -> Result<ConversationBindingReconcileReport, ConversationBindingError> {
        let expired = self.expire_due(now_unix_ms)?;
        let guard = self.lock_records()?;
        let conflicts = detect_conflicts(guard.values(), now_unix_ms);
        let repair_plan = build_repair_plan(conflicts);
        Ok(ConversationBindingReconcileReport {
            expired_count: expired.len(),
            conflict_count: repair_plan.conflicts.len(),
            repair_plan,
        })
    }

    pub fn explain(
        &self,
        request: ConversationBindingResolveRequest,
    ) -> Result<ConversationBindingExplainReport, ConversationBindingError> {
        let resolution = self.resolve(request.clone())?;
        let matches = self
            .list(
                ConversationBindingListFilter {
                    channel: Some(request.channel),
                    principal: Some(request.principal),
                    session_id: None,
                    include_inactive: true,
                    limit: Some(64),
                },
                request.now_unix_ms,
            )?
            .iter()
            .map(ConversationBindingRecord::safe_snapshot_json)
            .collect::<Vec<_>>();
        let repair_plan = build_repair_plan(resolution.conflicts);
        let summary = match resolution.record {
            Some(record) => format!(
                "binding {} resolves session {} via {} scope",
                record.binding_id,
                record.session_id,
                record.binding_kind.as_str()
            ),
            None => "no active conversation binding matched this scope".to_owned(),
        };
        Ok(ConversationBindingExplainReport { matches, repair_plan, summary: safe_text(&summary) })
    }

    pub fn apply_repair_plan(
        &self,
        plan: &ConversationBindingRepairPlan,
        now_unix_ms: i64,
    ) -> Result<ConversationBindingRepairOutcome, ConversationBindingError> {
        let mut guard = self.lock_records()?;
        let mut records = Vec::new();
        let mut applied_count = 0usize;
        let mut skipped_count = 0usize;
        for step in &plan.steps {
            if !step.automatic {
                skipped_count = skipped_count.saturating_add(1);
                continue;
            }
            let Some(record) = guard.get_mut(step.binding_id.as_str()) else {
                skipped_count = skipped_count.saturating_add(1);
                continue;
            };
            match step.action {
                ConversationBindingRepairAction::Detach => record.detach(now_unix_ms),
                ConversationBindingRepairAction::Expire => record.expire(now_unix_ms),
                ConversationBindingRepairAction::MarkStale => record.mark_stale(now_unix_ms),
                ConversationBindingRepairAction::Rebind
                | ConversationBindingRepairAction::Split => {
                    skipped_count = skipped_count.saturating_add(1);
                    continue;
                }
            }
            applied_count = applied_count.saturating_add(1);
            records.push(record.clone());
        }
        drop(guard);
        if applied_count > 0 {
            self.persist()?;
        }
        Ok(ConversationBindingRepairOutcome { applied_count, skipped_count, records })
    }

    fn lock_records(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, BTreeMap<String, ConversationBindingRecord>>,
        ConversationBindingError,
    > {
        self.records
            .lock()
            .map_err(|_| ConversationBindingError::PoisonedLock("conversation binding store"))
    }

    fn persist(&self) -> Result<(), ConversationBindingError> {
        let records = self.lock_records()?.clone();
        let envelope =
            ConversationBindingStoreEnvelope { schema_version: BINDING_SCHEMA_VERSION, records };
        let bytes = serde_json::to_vec_pretty(&envelope)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, bytes)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ConversationBindingStoreEnvelope {
    schema_version: u32,
    records: BTreeMap<String, ConversationBindingRecord>,
}

impl Default for ConversationBindingStoreEnvelope {
    fn default() -> Self {
        Self { schema_version: BINDING_SCHEMA_VERSION, records: BTreeMap::new() }
    }
}

#[derive(Debug)]
pub enum ConversationBindingError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidRecord(String),
    PoisonedLock(&'static str),
}

impl ConversationBindingError {
    #[must_use]
    pub fn safe_message(&self) -> String {
        safe_text(self.to_string().as_str())
    }
}

impl fmt::Display for ConversationBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "conversation binding store I/O failed: {error}"),
            Self::Json(error) => {
                write!(formatter, "conversation binding store JSON failed: {error}")
            }
            Self::InvalidRecord(message) => {
                write!(formatter, "invalid conversation binding: {message}")
            }
            Self::PoisonedLock(name) => write!(formatter, "{name} lock poisoned"),
        }
    }
}

impl Error for ConversationBindingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::InvalidRecord(_) | Self::PoisonedLock(_) => None,
        }
    }
}

impl From<std::io::Error> for ConversationBindingError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ConversationBindingError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

fn build_record(
    request: ConversationBindingCreateRequest,
) -> Result<ConversationBindingRecord, ConversationBindingError> {
    let channel = normalize_component(request.channel.as_str()).ok_or_else(|| {
        ConversationBindingError::InvalidRecord("channel must not be empty".to_owned())
    })?;
    let principal = normalize_component(request.principal.as_str()).ok_or_else(|| {
        ConversationBindingError::InvalidRecord("principal must not be empty".to_owned())
    })?;
    let session_id = normalize_component(request.session_id.as_str()).ok_or_else(|| {
        ConversationBindingError::InvalidRecord("session_id must not be empty".to_owned())
    })?;
    let policy_scope = normalize_component(request.policy_scope.as_str()).ok_or_else(|| {
        ConversationBindingError::InvalidRecord("policy_scope must not be empty".to_owned())
    })?;
    let conversation_id = normalize_optional_component(request.conversation_id.as_deref());
    let thread_id = normalize_optional_component(request.thread_id.as_deref());
    let sender_identity = normalize_optional_component(request.sender_identity.as_deref());
    let workspace_id = normalize_optional_component(request.workspace_id.as_deref());
    let parent_binding_id = normalize_optional_component(request.parent_binding_id.as_deref());
    let now = request.now_unix_ms;
    let expires_at_unix_ms = request.lifecycle.expires_at(now, now);
    let binding_id = stable_binding_id(
        request.binding_kind,
        channel.as_str(),
        conversation_id.as_deref(),
        thread_id.as_deref(),
        sender_identity.as_deref(),
        principal.as_str(),
        session_id.as_str(),
    );
    Ok(ConversationBindingRecord {
        schema_version: BINDING_SCHEMA_VERSION,
        binding_id,
        binding_kind: request.binding_kind,
        channel,
        conversation_id,
        thread_id,
        sender_identity,
        principal,
        session_id,
        workspace_id,
        policy_scope,
        parent_binding_id,
        lifecycle: request.lifecycle,
        state: ConversationBindingLifecycleState::Active,
        created_at_unix_ms: now,
        updated_at_unix_ms: now,
        last_activity_at_unix_ms: now,
        expires_at_unix_ms,
    })
}

fn detect_conflicts<'a>(
    records: impl Iterator<Item = &'a ConversationBindingRecord>,
    now_unix_ms: i64,
) -> Vec<ConversationBindingConflict> {
    let records = records.cloned().collect::<Vec<_>>();
    let mut conflicts = Vec::new();
    let mut active_by_scope: BTreeMap<ConversationBindingScopeKey, Vec<String>> = BTreeMap::new();
    let mut active_by_conflict_scope: BTreeMap<
        ConversationBindingConflictScopeKey,
        Vec<ConversationBindingRecord>,
    > = BTreeMap::new();
    let ids = records.iter().map(|record| record.binding_id.clone()).collect::<BTreeSet<_>>();
    for record in &records {
        if record.state.is_active()
            && record.expires_at_unix_ms.is_some_and(|expires_at| expires_at <= now_unix_ms)
        {
            conflicts.push(ConversationBindingConflict {
                kind: ConversationBindingConflictKind::ExpiredReferenced,
                binding_ids: vec![record.binding_id.clone()],
                reason: "active binding has expired and must be marked expired".to_owned(),
            });
        }
        if record.binding_kind == ConversationBindingKind::Thread && record.thread_id.is_none() {
            conflicts.push(ConversationBindingConflict {
                kind: ConversationBindingConflictKind::StaleThread,
                binding_ids: vec![record.binding_id.clone()],
                reason: "thread binding is missing thread_id".to_owned(),
            });
        }
        if let Some(parent_id) = record.parent_binding_id.as_deref() {
            if !ids.contains(parent_id) {
                conflicts.push(ConversationBindingConflict {
                    kind: ConversationBindingConflictKind::ParentMissing,
                    binding_ids: vec![record.binding_id.clone()],
                    reason: "binding references a missing parent binding".to_owned(),
                });
            }
        }
        if record.active(now_unix_ms) {
            active_by_scope.entry(record.scope_key()).or_default().push(record.binding_id.clone());
            active_by_conflict_scope
                .entry(record.conflict_scope_key())
                .or_default()
                .push(record.clone());
        }
    }
    for binding_ids in active_by_scope.values() {
        if binding_ids.len() > 1 {
            conflicts.push(ConversationBindingConflict {
                kind: ConversationBindingConflictKind::DuplicateActiveBinding,
                binding_ids: binding_ids.clone(),
                reason: "multiple active bindings match the same channel scope".to_owned(),
            });
        }
    }
    for records in active_by_conflict_scope.values() {
        let principals =
            records.iter().map(|record| record.principal.as_str()).collect::<BTreeSet<_>>();
        if principals.len() > 1 {
            conflicts.push(ConversationBindingConflict {
                kind: ConversationBindingConflictKind::PrincipalMismatch,
                binding_ids: records.iter().map(|record| record.binding_id.clone()).collect(),
                reason: "bindings share a channel scope across different principals".to_owned(),
            });
        }
        let workspaces = records
            .iter()
            .filter_map(|record| record.workspace_id.as_deref())
            .collect::<BTreeSet<_>>();
        if workspaces.len() > 1 {
            conflicts.push(ConversationBindingConflict {
                kind: ConversationBindingConflictKind::WorkspaceMismatch,
                binding_ids: records.iter().map(|record| record.binding_id.clone()).collect(),
                reason: "bindings share a channel scope across different workspaces".to_owned(),
            });
        }
    }
    conflicts.sort_by(|left, right| {
        left.kind.cmp(&right.kind).then_with(|| left.binding_ids.cmp(&right.binding_ids))
    });
    conflicts
}

fn build_repair_plan(conflicts: Vec<ConversationBindingConflict>) -> ConversationBindingRepairPlan {
    let mut steps = Vec::new();
    for conflict in &conflicts {
        match conflict.kind {
            ConversationBindingConflictKind::DuplicateActiveBinding => {
                for binding_id in conflict.binding_ids.iter().skip(1) {
                    steps.push(ConversationBindingRepairStep {
                        binding_id: binding_id.clone(),
                        action: ConversationBindingRepairAction::Detach,
                        automatic: true,
                        reason: "detach duplicate active binding after keeping the first stable id"
                            .to_owned(),
                    });
                }
            }
            ConversationBindingConflictKind::ExpiredReferenced => {
                for binding_id in &conflict.binding_ids {
                    steps.push(ConversationBindingRepairStep {
                        binding_id: binding_id.clone(),
                        action: ConversationBindingRepairAction::Expire,
                        automatic: true,
                        reason: "mark expired active binding as expired".to_owned(),
                    });
                }
            }
            ConversationBindingConflictKind::StaleThread
            | ConversationBindingConflictKind::ParentMissing => {
                for binding_id in &conflict.binding_ids {
                    steps.push(ConversationBindingRepairStep {
                        binding_id: binding_id.clone(),
                        action: ConversationBindingRepairAction::MarkStale,
                        automatic: true,
                        reason: "mark binding stale until an operator repairs the scope".to_owned(),
                    });
                }
            }
            ConversationBindingConflictKind::PrincipalMismatch
            | ConversationBindingConflictKind::WorkspaceMismatch => {
                for binding_id in &conflict.binding_ids {
                    steps.push(ConversationBindingRepairStep {
                        binding_id: binding_id.clone(),
                        action: ConversationBindingRepairAction::Split,
                        automatic: false,
                        reason: "scope conflict needs explicit operator review".to_owned(),
                    });
                }
            }
        }
    }
    let safe_to_auto_apply = steps.iter().all(|step| step.automatic);
    ConversationBindingRepairPlan { conflicts, steps, safe_to_auto_apply }
}

fn stable_binding_id(
    binding_kind: ConversationBindingKind,
    channel: &str,
    conversation_id: Option<&str>,
    thread_id: Option<&str>,
    sender_identity: Option<&str>,
    principal: &str,
    session_id: &str,
) -> String {
    let payload = serde_json::to_vec(&json!({
        "schema_version": BINDING_SCHEMA_VERSION,
        "binding_kind": binding_kind.as_str(),
        "channel": channel,
        "conversation_id": conversation_id,
        "thread_id": thread_id,
        "sender_identity": sender_identity,
        "principal": principal,
        "session_id": session_id,
    }))
    .unwrap_or_default();
    format!("cb_{}", sha256_hex(payload.as_slice()))
}

fn ensure_non_empty(value: &str, field: &str) -> Result<(), ConversationBindingError> {
    if value.trim().is_empty() {
        Err(ConversationBindingError::InvalidRecord(format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}

fn normalize_component(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn normalize_optional_component(value: Option<&str>) -> Option<String> {
    value.and_then(normalize_component)
}

fn safe_text(value: &str) -> String {
    redact_url_segments_in_text(&redact_auth_error(value))
}

fn sha256_hex(payload: &[u8]) -> String {
    let digest = Sha256::digest(payload);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ConversationBindingCreateRequest, ConversationBindingKind, ConversationBindingLifecycle,
        ConversationBindingLifecycleState, ConversationBindingListFilter,
        ConversationBindingResolveRequest, ConversationBindingStore,
    };

    fn create_request(session_id: &str, now_unix_ms: i64) -> ConversationBindingCreateRequest {
        ConversationBindingCreateRequest {
            binding_kind: ConversationBindingKind::Thread,
            channel: "discord:default".to_owned(),
            conversation_id: Some("conv-1".to_owned()),
            thread_id: Some("thread-1".to_owned()),
            sender_identity: Some("discord:user:42".to_owned()),
            principal: "user:ops".to_owned(),
            session_id: session_id.to_owned(),
            workspace_id: Some("workspace-a".to_owned()),
            policy_scope: "channel:discord:default".to_owned(),
            parent_binding_id: None,
            lifecycle: ConversationBindingLifecycle::default(),
            now_unix_ms,
        }
    }

    #[test]
    fn create_touch_list_and_reload_are_durable() {
        let path =
            std::env::temp_dir().join(format!("palyra-bindings-test-{}.json", ulid::Ulid::new()));
        let store = ConversationBindingStore::open(&path).expect("store opens");
        let created = store
            .create_or_touch(create_request("01ARZ3NDEKTSV4RRFFQ69G5FAV", 1_000))
            .expect("binding create succeeds");
        assert!(created.created);

        let touched = store
            .create_or_touch(create_request("01ARZ3NDEKTSV4RRFFQ69G5FAV", 2_000))
            .expect("binding touch succeeds");
        assert!(!touched.created);
        assert_eq!(touched.record.last_activity_at_unix_ms, 2_000);

        let reloaded = ConversationBindingStore::open(&path).expect("store reloads");
        let records =
            reloaded.list(ConversationBindingListFilter::default(), 2_000).expect("list succeeds");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].binding_id, created.record.binding_id);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn resolver_prefers_thread_scope_and_expires_due_records() {
        let store = ConversationBindingStore::open_temp();
        let mut request = create_request("01ARZ3NDEKTSV4RRFFQ69G5FAW", 1_000);
        request.lifecycle.idle_timeout_ms = Some(100);
        let created = store.create_or_touch(request).expect("binding create succeeds");

        let resolved = store
            .resolve(ConversationBindingResolveRequest {
                channel: "discord:default".to_owned(),
                conversation_id: Some("conv-1".to_owned()),
                thread_id: Some("thread-1".to_owned()),
                sender_identity: Some("discord:user:42".to_owned()),
                principal: "user:ops".to_owned(),
                now_unix_ms: 1_050,
            })
            .expect("resolve succeeds");
        assert_eq!(
            resolved.record.as_ref().map(|record| record.binding_id.as_str()),
            Some(created.record.binding_id.as_str())
        );
        assert_eq!(resolved.reason, "thread_binding_resolved");

        let expired = store
            .resolve(ConversationBindingResolveRequest {
                channel: "discord:default".to_owned(),
                conversation_id: Some("conv-1".to_owned()),
                thread_id: Some("thread-1".to_owned()),
                sender_identity: Some("discord:user:42".to_owned()),
                principal: "user:ops".to_owned(),
                now_unix_ms: 1_101,
            })
            .expect("resolve succeeds after expiry");
        assert!(expired.record.is_none());
        assert_eq!(expired.expired_count, 1);
        let records = store
            .list(
                ConversationBindingListFilter { include_inactive: true, ..Default::default() },
                1_101,
            )
            .expect("list succeeds");
        assert_eq!(records[0].state, ConversationBindingLifecycleState::Expired);
    }

    #[test]
    fn resolver_prefers_delegated_run_scope_over_thread_scope() {
        let store = ConversationBindingStore::open_temp();
        let thread = store
            .create_or_touch(create_request("01ARZ3NDEKTSV4RRFFQ69G5FAV", 1_000))
            .expect("thread binding create succeeds");
        let mut delegated = create_request("01ARZ3NDEKTSV4RRFFQ69G5FAW", 1_001);
        delegated.binding_kind = ConversationBindingKind::DelegatedRun;
        delegated.thread_id = Some("delegation:task-1".to_owned());
        delegated.sender_identity = Some("delegated-run:child-run".to_owned());
        delegated.policy_scope = "delegation:parent-run".to_owned();
        let delegated = store.create_or_touch(delegated).expect("delegated binding succeeds");

        let resolved = store
            .resolve(ConversationBindingResolveRequest {
                channel: "discord:default".to_owned(),
                conversation_id: Some("conv-1".to_owned()),
                thread_id: Some("delegation:task-1".to_owned()),
                sender_identity: Some("delegated-run:child-run".to_owned()),
                principal: "user:ops".to_owned(),
                now_unix_ms: 1_050,
            })
            .expect("resolve succeeds");

        assert_eq!(
            resolved.record.as_ref().map(|record| record.binding_id.as_str()),
            Some(delegated.record.binding_id.as_str())
        );
        assert_ne!(
            resolved.record.as_ref().map(|record| record.binding_id.as_str()),
            Some(thread.record.binding_id.as_str())
        );
        assert_eq!(resolved.reason, "delegated_run_binding_resolved");
    }

    #[test]
    fn explain_reports_principal_conflict_without_auto_repair() {
        let store = ConversationBindingStore::open_temp();
        store
            .create_or_touch(create_request("01ARZ3NDEKTSV4RRFFQ69G5FAX", 1_000))
            .expect("first create succeeds");
        let mut second = create_request("01ARZ3NDEKTSV4RRFFQ69G5FAY", 1_001);
        second.principal = "user:other".to_owned();
        store.create_or_touch(second).expect("second create succeeds");

        let report = store
            .explain(ConversationBindingResolveRequest {
                channel: "discord:default".to_owned(),
                conversation_id: Some("conv-1".to_owned()),
                thread_id: Some("thread-1".to_owned()),
                sender_identity: Some("discord:user:42".to_owned()),
                principal: "user:ops".to_owned(),
                now_unix_ms: 2_000,
            })
            .expect("explain succeeds");
        assert!(report
            .repair_plan
            .conflicts
            .iter()
            .any(|conflict| conflict.kind.as_str() == "principal_mismatch"));
        assert!(!report.repair_plan.safe_to_auto_apply);
    }
}
