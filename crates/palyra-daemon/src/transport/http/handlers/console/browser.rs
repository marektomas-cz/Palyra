use crate::*;

pub(crate) async fn console_browser_profiles_list_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConsoleBrowserProfilesQuery>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let principal = resolve_console_browser_principal(
        query.principal.as_deref(),
        session.context.principal.as_str(),
    )?;
    let mut client = build_console_browser_client(&state).await?;
    let mut request = TonicRequest::new(browser_v1::ListProfilesRequest {
        v: palyra_common::CANONICAL_PROTOCOL_MAJOR,
        principal: principal.clone(),
    });
    apply_browser_service_auth(&state, request.metadata_mut())?;
    let response =
        client.list_profiles(request).await.map_err(runtime_status_response)?.into_inner();
    let profiles =
        response.profiles.into_iter().map(console_browser_profile_to_json).collect::<Vec<_>>();
    Ok(Json(json!({
        "principal": principal,
        "active_profile_id": response.active_profile_id.map(|value| value.ulid),
        "profiles": profiles,
        "page": build_page_info(profiles.len().max(1), profiles.len(), None),
    })))
}

pub(crate) async fn console_browser_profile_create_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConsoleBrowserCreateProfileRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, true)?;
    let principal = resolve_console_browser_principal(
        payload.principal.as_deref(),
        session.context.principal.as_str(),
    )?;
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "profile name cannot be empty",
        )));
    }
    let mut client = build_console_browser_client(&state).await?;
    let mut request = TonicRequest::new(browser_v1::CreateProfileRequest {
        v: palyra_common::CANONICAL_PROTOCOL_MAJOR,
        principal,
        name: name.to_owned(),
        theme_color: payload.theme_color.as_deref().map(str::trim).unwrap_or_default().to_owned(),
        persistence_enabled: payload.persistence_enabled.unwrap_or(false),
        private_profile: payload.private_profile.unwrap_or(false),
    });
    apply_browser_service_auth(&state, request.metadata_mut())?;
    let response =
        client.create_profile(request).await.map_err(runtime_status_response)?.into_inner();
    let profile = response.profile.ok_or_else(|| {
        runtime_status_response(tonic::Status::internal(
            "browser create_profile response is missing profile payload",
        ))
    })?;
    Ok(Json(json!({
        "profile": console_browser_profile_to_json(profile),
    })))
}

pub(crate) async fn console_browser_profile_rename_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
    Json(payload): Json<ConsoleBrowserRenameProfileRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, true)?;
    validate_canonical_id(profile_id.as_str()).map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument(
            "profile_id must be a canonical ULID",
        ))
    })?;
    let principal = resolve_console_browser_principal(
        payload.principal.as_deref(),
        session.context.principal.as_str(),
    )?;
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "profile name cannot be empty",
        )));
    }
    let mut client = build_console_browser_client(&state).await?;
    let mut request = TonicRequest::new(browser_v1::RenameProfileRequest {
        v: palyra_common::CANONICAL_PROTOCOL_MAJOR,
        principal,
        profile_id: Some(common_v1::CanonicalId { ulid: profile_id }),
        name: name.to_owned(),
    });
    apply_browser_service_auth(&state, request.metadata_mut())?;
    let response =
        client.rename_profile(request).await.map_err(runtime_status_response)?.into_inner();
    let profile = response.profile.ok_or_else(|| {
        runtime_status_response(tonic::Status::internal(
            "browser rename_profile response is missing profile payload",
        ))
    })?;
    Ok(Json(json!({
        "profile": console_browser_profile_to_json(profile),
    })))
}

pub(crate) async fn console_browser_profile_delete_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
    Json(payload): Json<ConsoleBrowserProfileScopeRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, true)?;
    validate_canonical_id(profile_id.as_str()).map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument(
            "profile_id must be a canonical ULID",
        ))
    })?;
    let principal = resolve_console_browser_principal(
        payload.principal.as_deref(),
        session.context.principal.as_str(),
    )?;
    let mut client = build_console_browser_client(&state).await?;
    let mut request = TonicRequest::new(browser_v1::DeleteProfileRequest {
        v: palyra_common::CANONICAL_PROTOCOL_MAJOR,
        principal,
        profile_id: Some(common_v1::CanonicalId { ulid: profile_id }),
    });
    apply_browser_service_auth(&state, request.metadata_mut())?;
    let response =
        client.delete_profile(request).await.map_err(runtime_status_response)?.into_inner();
    Ok(Json(json!({
        "deleted": response.deleted,
        "active_profile_id": response.active_profile_id.map(|value| value.ulid),
    })))
}

pub(crate) async fn console_browser_profile_activate_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
    Json(payload): Json<ConsoleBrowserProfileScopeRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, true)?;
    validate_canonical_id(profile_id.as_str()).map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument(
            "profile_id must be a canonical ULID",
        ))
    })?;
    let principal = resolve_console_browser_principal(
        payload.principal.as_deref(),
        session.context.principal.as_str(),
    )?;
    let mut client = build_console_browser_client(&state).await?;
    let mut request = TonicRequest::new(browser_v1::SetActiveProfileRequest {
        v: palyra_common::CANONICAL_PROTOCOL_MAJOR,
        principal,
        profile_id: Some(common_v1::CanonicalId { ulid: profile_id }),
    });
    apply_browser_service_auth(&state, request.metadata_mut())?;
    let response =
        client.set_active_profile(request).await.map_err(runtime_status_response)?.into_inner();
    let profile = response.profile.ok_or_else(|| {
        runtime_status_response(tonic::Status::internal(
            "browser set_active_profile response is missing profile payload",
        ))
    })?;
    Ok(Json(json!({
        "profile": console_browser_profile_to_json(profile),
    })))
}

pub(crate) async fn console_browser_downloads_list_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConsoleBrowserDownloadsQuery>,
) -> Result<Json<Value>, Response> {
    let _session = authorize_console_session(&state, &headers, false)?;
    let session_id = query.session_id.trim();
    if session_id.is_empty() {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "session_id cannot be empty",
        )));
    }
    validate_canonical_id(session_id).map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument(
            "session_id must be a canonical ULID",
        ))
    })?;
    let mut client = build_console_browser_client(&state).await?;
    let mut request = TonicRequest::new(browser_v1::ListDownloadArtifactsRequest {
        v: palyra_common::CANONICAL_PROTOCOL_MAJOR,
        session_id: Some(common_v1::CanonicalId { ulid: session_id.to_owned() }),
        limit: query.limit.unwrap_or(50).clamp(1, 250),
        quarantined_only: query.quarantined_only.unwrap_or(false),
    });
    apply_browser_service_auth(&state, request.metadata_mut())?;
    let response = client
        .list_download_artifacts(request)
        .await
        .map_err(runtime_status_response)?
        .into_inner();
    let artifacts = response
        .artifacts
        .into_iter()
        .map(console_browser_download_artifact_to_json)
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "artifacts": artifacts,
        "truncated": response.truncated,
        "error": response.error,
        "page": build_page_info(query.limit.unwrap_or(50).clamp(1, 250) as usize, artifacts.len(), None),
    })))
}

pub(crate) async fn console_browser_relay_token_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConsoleBrowserRelayTokenRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, true)?;
    let session_id = payload.session_id.trim();
    if session_id.is_empty() {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "session_id cannot be empty",
        )));
    }
    validate_canonical_id(session_id).map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument(
            "session_id must be a canonical ULID",
        ))
    })?;
    let extension_id = normalize_browser_extension_id(payload.extension_id.as_str())?;
    let ttl_ms = clamp_console_relay_token_ttl_ms(payload.ttl_ms);
    let issued_at_unix_ms = unix_ms_now().map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to read system clock: {error}"
        )))
    })?;
    let expires_at_unix_ms =
        issued_at_unix_ms.saturating_add(i64::try_from(ttl_ms).unwrap_or(i64::MAX));
    let relay_token = mint_console_relay_token();
    let token_hash_sha256 = sha256_hex(relay_token.as_bytes());
    let record = ConsoleRelayToken {
        token_hash_sha256: token_hash_sha256.clone(),
        principal: session.context.principal.clone(),
        device_id: session.context.device_id.clone(),
        channel: session.context.channel.clone(),
        session_id: session_id.to_owned(),
        extension_id: extension_id.clone(),
        issued_at_unix_ms,
        expires_at_unix_ms,
    };
    {
        let mut relay_tokens = lock_relay_tokens(&state.relay_tokens);
        prune_console_relay_tokens(&mut relay_tokens, issued_at_unix_ms);
        relay_tokens.insert(token_hash_sha256.clone(), record.clone());
        prune_console_relay_tokens(&mut relay_tokens, issued_at_unix_ms);
    }

    state
        .runtime
        .record_console_event(
            &session.context,
            "browser.relay.token.minted",
            json!({
                "session_id": record.session_id,
                "extension_id": record.extension_id,
                "issued_at_unix_ms": record.issued_at_unix_ms,
                "expires_at_unix_ms": record.expires_at_unix_ms,
                "token_hash_sha256": record.token_hash_sha256,
            }),
        )
        .await
        .map_err(runtime_status_response)?;

    Ok(Json(json!({
        "relay_token": relay_token,
        "session_id": record.session_id,
        "extension_id": record.extension_id,
        "issued_at_unix_ms": record.issued_at_unix_ms,
        "expires_at_unix_ms": record.expires_at_unix_ms,
        "token_ttl_ms": ttl_ms,
        "warning": "Relay token grants scoped browser extension actions; keep it short-lived and private.",
    })))
}

pub(crate) async fn console_browser_relay_action_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConsoleBrowserRelayActionRequest>,
) -> Result<Json<Value>, Response> {
    let relay_token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(extract_bearer_token)
        .ok_or_else(|| {
            runtime_status_response(tonic::Status::permission_denied(
                "relay action requires bearer relay token",
            ))
        })?;
    let now = unix_ms_now().map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to read system clock: {error}"
        )))
    })?;
    let relay_token_hash_sha256 = sha256_hex(relay_token.as_bytes());
    let record = {
        let mut relay_tokens = lock_relay_tokens(&state.relay_tokens);
        prune_console_relay_tokens(&mut relay_tokens, now);
        let relay_token_key =
            find_hashed_secret_map_key(&relay_tokens, relay_token_hash_sha256.as_str())
                .ok_or_else(|| {
                    runtime_status_response(tonic::Status::permission_denied(
                        "relay token is missing, invalid, or expired",
                    ))
                })?;
        relay_tokens.get(relay_token_key.as_str()).cloned().ok_or_else(|| {
            runtime_status_response(tonic::Status::permission_denied(
                "relay token is missing, invalid, or expired",
            ))
        })?
    };

    let session_id = payload.session_id.trim();
    if session_id.is_empty() {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "session_id cannot be empty",
        )));
    }
    validate_canonical_id(session_id).map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument(
            "session_id must be a canonical ULID",
        ))
    })?;
    if session_id != record.session_id {
        return Err(runtime_status_response(tonic::Status::permission_denied(
            "relay token is not valid for the requested session_id",
        )));
    }
    let extension_id = normalize_browser_extension_id(payload.extension_id.as_str())?;
    if extension_id != record.extension_id {
        return Err(runtime_status_response(tonic::Status::permission_denied(
            "relay token is not valid for the requested extension_id",
        )));
    }

    let action = parse_console_relay_action_kind(payload.action.as_str())?;
    let relay_payload = match action {
        browser_v1::RelayActionKind::OpenTab => {
            let open_tab = payload.open_tab.ok_or_else(|| {
                runtime_status_response(tonic::Status::invalid_argument(
                    "open_tab payload is required for action=open_tab",
                ))
            })?;
            let url = open_tab.url.trim();
            if url.is_empty() {
                return Err(runtime_status_response(tonic::Status::invalid_argument(
                    "open_tab.url cannot be empty",
                )));
            }
            Some(browser_v1::relay_action_request::Payload::OpenTab(
                browser_v1::RelayOpenTabPayload {
                    url: url.to_owned(),
                    activate: open_tab.activate.unwrap_or(true),
                    timeout_ms: open_tab.timeout_ms.unwrap_or(0),
                },
            ))
        }
        browser_v1::RelayActionKind::CaptureSelection => {
            let capture = payload.capture_selection.ok_or_else(|| {
                runtime_status_response(tonic::Status::invalid_argument(
                    "capture_selection payload is required for action=capture_selection",
                ))
            })?;
            let selector = capture.selector.trim();
            if selector.is_empty() {
                return Err(runtime_status_response(tonic::Status::invalid_argument(
                    "capture_selection.selector cannot be empty",
                )));
            }
            Some(browser_v1::relay_action_request::Payload::CaptureSelection(
                browser_v1::RelayCaptureSelectionPayload {
                    selector: selector.to_owned(),
                    max_selection_bytes: capture.max_selection_bytes.unwrap_or(0),
                },
            ))
        }
        browser_v1::RelayActionKind::SendPageSnapshot => {
            let snapshot =
                payload.page_snapshot.unwrap_or(ConsoleBrowserRelayPageSnapshotPayload {
                    include_dom_snapshot: Some(true),
                    include_visible_text: Some(true),
                    max_dom_snapshot_bytes: Some(16 * 1_024),
                    max_visible_text_bytes: Some(8 * 1_024),
                });
            Some(browser_v1::relay_action_request::Payload::PageSnapshot(
                browser_v1::RelayPageSnapshotPayload {
                    include_dom_snapshot: snapshot.include_dom_snapshot.unwrap_or(true),
                    include_visible_text: snapshot.include_visible_text.unwrap_or(true),
                    max_dom_snapshot_bytes: snapshot.max_dom_snapshot_bytes.unwrap_or(0),
                    max_visible_text_bytes: snapshot.max_visible_text_bytes.unwrap_or(0),
                },
            ))
        }
        browser_v1::RelayActionKind::Unspecified => None,
    };

    let mut client = build_console_browser_client(&state).await?;
    let mut request = TonicRequest::new(browser_v1::RelayActionRequest {
        v: palyra_common::CANONICAL_PROTOCOL_MAJOR,
        session_id: Some(common_v1::CanonicalId { ulid: session_id.to_owned() }),
        extension_id: extension_id.clone(),
        action: action as i32,
        payload: relay_payload,
        max_payload_bytes: payload
            .max_payload_bytes
            .unwrap_or(CONSOLE_MAX_RELAY_ACTION_PAYLOAD_BYTES)
            .clamp(1, CONSOLE_MAX_RELAY_ACTION_PAYLOAD_BYTES),
    });
    apply_browser_service_auth(&state, request.metadata_mut())?;
    let response =
        client.relay_action(request).await.map_err(runtime_status_response)?.into_inner();

    let result = match response.result {
        Some(browser_v1::relay_action_response::Result::OpenedTab(tab)) => {
            json!({ "opened_tab": console_browser_tab_to_json(tab) })
        }
        Some(browser_v1::relay_action_response::Result::Selection(selection)) => json!({
            "selection": {
                "selector": selection.selector,
                "selected_text": selection.selected_text,
                "truncated": selection.truncated,
            }
        }),
        Some(browser_v1::relay_action_response::Result::Snapshot(snapshot)) => json!({
            "snapshot": {
                "dom_snapshot": snapshot.dom_snapshot,
                "visible_text": snapshot.visible_text,
                "dom_truncated": snapshot.dom_truncated,
                "visible_text_truncated": snapshot.visible_text_truncated,
                "page_url": snapshot.page_url,
            }
        }),
        None => Value::Null,
    };

    let audit_context = gateway::RequestContext {
        principal: record.principal.clone(),
        device_id: record.device_id.clone(),
        channel: record.channel.clone(),
    };
    state
        .runtime
        .record_console_event(
            &audit_context,
            "browser.relay.action",
            json!({
                "session_id": record.session_id,
                "extension_id": record.extension_id,
                "action": relay_action_kind_label(response.action),
                "success": response.success,
                "error": response.error,
                "token_hash_sha256": record.token_hash_sha256,
            }),
        )
        .await
        .map_err(runtime_status_response)?;

    Ok(Json(json!({
        "success": response.success,
        "action": relay_action_kind_label(response.action),
        "error": response.error,
        "result": result,
    })))
}

#[allow(clippy::result_large_err)]
fn resolve_console_browser_principal(
    requested: Option<&str>,
    fallback: &str,
) -> Result<String, Response> {
    let value =
        requested.map(str::trim).filter(|value| !value.is_empty()).unwrap_or(fallback).trim();
    if value.is_empty() {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "principal cannot be empty",
        )));
    }
    if value.len() > 128 {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "principal exceeds max bytes (128)",
        )));
    }
    Ok(value.to_owned())
}

#[allow(clippy::result_large_err)]
fn normalize_browser_extension_id(raw: &str) -> Result<String, Response> {
    let extension_id = raw.trim();
    if extension_id.is_empty() {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "extension_id cannot be empty",
        )));
    }
    if extension_id.len() > CONSOLE_MAX_RELAY_EXTENSION_ID_BYTES {
        return Err(runtime_status_response(tonic::Status::invalid_argument(format!(
            "extension_id exceeds max bytes ({CONSOLE_MAX_RELAY_EXTENSION_ID_BYTES})",
        ))));
    }
    if !extension_id
        .bytes()
        .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'.' | b'-' | b'_'))
    {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "extension_id contains unsupported characters",
        )));
    }
    Ok(extension_id.to_owned())
}

pub(crate) fn clamp_console_relay_token_ttl_ms(value: Option<u64>) -> u64 {
    value
        .unwrap_or(CONSOLE_RELAY_TOKEN_DEFAULT_TTL_MS)
        .clamp(CONSOLE_RELAY_TOKEN_MIN_TTL_MS, CONSOLE_RELAY_TOKEN_MAX_TTL_MS)
}

pub(crate) fn mint_console_secret_token() -> String {
    let token_bytes: [u8; 32] = rand::random();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token_bytes)
}

pub(crate) fn mint_console_relay_token() -> String {
    mint_console_secret_token()
}

fn lock_relay_tokens<'a>(
    tokens: &'a Arc<Mutex<HashMap<String, ConsoleRelayToken>>>,
) -> std::sync::MutexGuard<'a, HashMap<String, ConsoleRelayToken>> {
    match tokens.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!("relay token map lock poisoned; recovering");
            poisoned.into_inner()
        }
    }
}

pub(crate) fn constant_time_eq_bytes(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

pub(crate) fn find_hashed_secret_map_key<T>(
    values: &HashMap<String, T>,
    candidate_hash: &str,
) -> Option<String> {
    let mut matched: Option<String> = None;
    for token_hash in values.keys() {
        if constant_time_eq_bytes(token_hash.as_bytes(), candidate_hash.as_bytes()) {
            matched = Some(token_hash.clone());
        }
    }
    matched
}

pub(crate) fn prune_console_relay_tokens(
    tokens: &mut HashMap<String, ConsoleRelayToken>,
    now_unix_ms: i64,
) {
    tokens.retain(|_, value| value.expires_at_unix_ms > now_unix_ms);
    while tokens.len() > CONSOLE_MAX_RELAY_TOKENS {
        let removable = tokens
            .iter()
            .min_by(|left, right| left.1.expires_at_unix_ms.cmp(&right.1.expires_at_unix_ms))
            .map(|(token, _)| token.clone());
        if let Some(token) = removable {
            tokens.remove(token.as_str());
        } else {
            break;
        }
    }
}

fn extract_bearer_token(raw_authorization: &str) -> Option<String> {
    let trimmed = raw_authorization.trim();
    let prefix = "bearer ";
    if trimmed.len() <= prefix.len() || !trimmed[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }
    let token = trimmed[prefix.len()..].trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_owned())
    }
}

#[allow(clippy::result_large_err)]
fn parse_console_relay_action_kind(raw: &str) -> Result<browser_v1::RelayActionKind, Response> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "open_tab" => Ok(browser_v1::RelayActionKind::OpenTab),
        "capture_selection" => Ok(browser_v1::RelayActionKind::CaptureSelection),
        "send_page_snapshot" => Ok(browser_v1::RelayActionKind::SendPageSnapshot),
        _ => Err(runtime_status_response(tonic::Status::invalid_argument(
            "action must be one of open_tab|capture_selection|send_page_snapshot",
        ))),
    }
}

fn relay_action_kind_label(raw: i32) -> &'static str {
    match browser_v1::RelayActionKind::try_from(raw)
        .unwrap_or(browser_v1::RelayActionKind::Unspecified)
    {
        browser_v1::RelayActionKind::OpenTab => "open_tab",
        browser_v1::RelayActionKind::CaptureSelection => "capture_selection",
        browser_v1::RelayActionKind::SendPageSnapshot => "send_page_snapshot",
        browser_v1::RelayActionKind::Unspecified => "unspecified",
    }
}

pub(crate) async fn build_console_browser_client(
    state: &AppState,
) -> Result<
    browser_v1::browser_service_client::BrowserServiceClient<tonic::transport::Channel>,
    Response,
> {
    if !state.browser_service_config.enabled {
        return Err(runtime_status_response(tonic::Status::failed_precondition(
            "browser service is disabled (tool_call.browser_service.enabled=false)",
        )));
    }
    let endpoint =
        tonic::transport::Endpoint::from_shared(state.browser_service_config.endpoint.clone())
            .map_err(|error| {
                runtime_status_response(tonic::Status::invalid_argument(format!(
                    "invalid browser service endpoint '{}': {error}",
                    state.browser_service_config.endpoint
                )))
            })?
            .connect_timeout(std::time::Duration::from_millis(
                state.browser_service_config.connect_timeout_ms,
            ))
            .timeout(std::time::Duration::from_millis(
                state.browser_service_config.request_timeout_ms,
            ));
    let channel = endpoint.connect().await.map_err(|error| {
        runtime_status_response(tonic::Status::unavailable(format!(
            "failed to connect to browser service '{}': {error}",
            state.browser_service_config.endpoint
        )))
    })?;
    Ok(browser_v1::browser_service_client::BrowserServiceClient::new(channel))
}

#[allow(clippy::result_large_err)]
pub(crate) fn apply_browser_service_auth(
    state: &AppState,
    metadata: &mut tonic::metadata::MetadataMap,
) -> Result<(), Response> {
    if let Some(token) = state.browser_service_config.auth_token.as_deref() {
        let bearer = MetadataValue::try_from(format!("Bearer {token}").as_str()).map_err(|_| {
            runtime_status_response(tonic::Status::internal(
                "failed to encode browser service authorization metadata",
            ))
        })?;
        metadata.insert("authorization", bearer);
    }
    Ok(())
}

fn console_browser_profile_to_json(profile: browser_v1::BrowserProfile) -> Value {
    json!({
        "profile_id": profile.profile_id.map(|value| value.ulid),
        "principal": profile.principal,
        "name": profile.name,
        "theme_color": profile.theme_color,
        "created_at_unix_ms": profile.created_at_unix_ms,
        "updated_at_unix_ms": profile.updated_at_unix_ms,
        "last_used_unix_ms": profile.last_used_unix_ms,
        "persistence_enabled": profile.persistence_enabled,
        "private_profile": profile.private_profile,
        "active": profile.active,
    })
}

fn console_browser_tab_to_json(tab: browser_v1::BrowserTab) -> Value {
    json!({
        "tab_id": tab.tab_id.map(|value| value.ulid),
        "url": tab.url,
        "title": tab.title,
        "active": tab.active,
    })
}

fn console_browser_download_artifact_to_json(artifact: browser_v1::DownloadArtifact) -> Value {
    json!({
        "artifact_id": artifact.artifact_id.map(|value| value.ulid),
        "profile_id": artifact.profile_id.map(|value| value.ulid),
        "source_url": artifact.source_url,
        "file_name": artifact.file_name,
        "mime_type": artifact.mime_type,
        "size_bytes": artifact.size_bytes,
        "sha256": artifact.sha256,
        "created_at_unix_ms": artifact.created_at_unix_ms,
        "quarantined": artifact.quarantined,
        "quarantine_reason": artifact.quarantine_reason,
    })
}
