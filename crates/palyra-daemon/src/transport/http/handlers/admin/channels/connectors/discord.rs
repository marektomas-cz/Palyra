use crate::transport::http::handlers::console::channels::connectors::discord::{
    apply_discord_onboarding, build_discord_onboarding_preflight, remove_discord_onboarding_config,
};
use crate::*;

pub(crate) async fn admin_discord_onboarding_probe_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DiscordOnboardingRequest>,
) -> Result<Json<DiscordOnboardingPreflightResponse>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let _context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    state.runtime.record_admin_status_request();
    let response = build_discord_onboarding_preflight(&state, payload).await?;
    Ok(Json(response))
}

pub(crate) async fn admin_discord_onboarding_apply_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DiscordOnboardingRequest>,
) -> Result<Json<Value>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let _context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    state.runtime.record_admin_status_request();
    let response = apply_discord_onboarding(&state, payload).await?;
    Ok(Json(response))
}

pub(crate) async fn admin_discord_account_logout_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(account_id): Path<String>,
    Json(payload): Json<DiscordAccountLifecycleRequest>,
) -> Result<Json<Value>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let _context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    state.runtime.record_admin_status_request();
    build_admin_discord_account_logout_response(&state, account_id, payload.keep_credential)
}

pub(crate) async fn admin_discord_account_logout_action_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DiscordAccountLifecycleActionRequest>,
) -> Result<Json<Value>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let _context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    state.runtime.record_admin_status_request();
    build_admin_discord_account_logout_response(&state, payload.account_id, payload.keep_credential)
}

pub(crate) async fn admin_discord_account_remove_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(account_id): Path<String>,
    Json(payload): Json<DiscordAccountLifecycleRequest>,
) -> Result<Json<Value>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let _context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    state.runtime.record_admin_status_request();
    build_admin_discord_account_remove_response(&state, account_id, payload.keep_credential)
}

pub(crate) async fn admin_discord_account_remove_action_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DiscordAccountLifecycleActionRequest>,
) -> Result<Json<Value>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let _context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    state.runtime.record_admin_status_request();
    build_admin_discord_account_remove_response(&state, payload.account_id, payload.keep_credential)
}

#[allow(clippy::result_large_err)]
fn build_admin_discord_account_logout_response(
    state: &AppState,
    account_id: String,
    keep_credential: Option<bool>,
) -> Result<Json<Value>, Response> {
    let normalized_account_id = channels::normalize_discord_account_id(account_id.as_str())
        .map_err(channel_platform_error_response)?;
    let connector_id = channels::discord_connector_id(normalized_account_id.as_str());
    let status = state
        .channels
        .set_enabled(connector_id.as_str(), false)
        .map_err(channel_platform_error_response)?;
    let credential_deleted = delete_discord_credential_if_requested(
        state,
        normalized_account_id.as_str(),
        keep_credential,
    )?;
    Ok(Json(json!({
        "action": "logout",
        "provider": "discord",
        "account_id": normalized_account_id,
        "connector_id": connector_id,
        "keep_credential": keep_credential.unwrap_or(false),
        "credential_deleted": credential_deleted,
        "status": status,
    })))
}

#[allow(clippy::result_large_err)]
fn build_admin_discord_account_remove_response(
    state: &AppState,
    account_id: String,
    keep_credential: Option<bool>,
) -> Result<Json<Value>, Response> {
    let normalized_account_id = channels::normalize_discord_account_id(account_id.as_str())
        .map_err(channel_platform_error_response)?;
    let connector_id = channels::discord_connector_id(normalized_account_id.as_str());
    let disabled_status = state
        .channels
        .set_enabled(connector_id.as_str(), false)
        .map_err(channel_platform_error_response)?;
    let credential_deleted = delete_discord_credential_if_requested(
        state,
        normalized_account_id.as_str(),
        keep_credential,
    )?;
    let config_path = remove_discord_onboarding_config(normalized_account_id.as_str())?;
    state
        .channels
        .remove_connector(connector_id.as_str())
        .map_err(channel_platform_error_response)?;
    Ok(Json(json!({
        "action": "remove",
        "provider": "discord",
        "account_id": normalized_account_id,
        "connector_id": connector_id,
        "keep_credential": keep_credential.unwrap_or(false),
        "credential_deleted": credential_deleted,
        "config_updated": config_path.is_some(),
        "config_path": config_path.map(|path| path.display().to_string()),
        "removed": true,
        "status_before_remove": disabled_status,
    })))
}

#[allow(clippy::result_large_err)]
fn delete_discord_credential_if_requested(
    state: &AppState,
    normalized_account_id: &str,
    keep_credential: Option<bool>,
) -> Result<bool, Response> {
    if keep_credential.unwrap_or(false) {
        return Ok(false);
    }
    let vault_ref = channels::discord_token_vault_ref(normalized_account_id);
    let parsed_ref = VaultRef::parse(vault_ref.as_str()).map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to parse discord token vault ref: {error}"
        )))
    })?;
    state.vault.delete_secret(&parsed_ref.scope, parsed_ref.key.as_str()).map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to delete discord token from vault: {error}"
        )))
    })
}
