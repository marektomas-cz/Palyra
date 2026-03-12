use super::*;
use crate::application::service_authorization::authorize_vault_action;

pub(crate) fn map_vault_error(operation: &str, error: VaultError) -> Status {
    match error {
        VaultError::NotFound => Status::not_found("secret not found"),
        VaultError::InvalidScope(message)
        | VaultError::InvalidKey(message)
        | VaultError::InvalidObjectId(message)
        | VaultError::Crypto(message) => Status::invalid_argument(message),
        VaultError::ValueTooLarge { actual, max } => {
            Status::invalid_argument(format!("secret value exceeds limit ({actual} > {max})"))
        }
        VaultError::BackendUnavailable(message) => Status::failed_precondition(message),
        VaultError::Io(message) => Status::internal(format!("{operation} failed: {message}")),
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn parse_vault_scope(raw: &str) -> Result<VaultScope, Status> {
    raw.parse::<VaultScope>()
        .map_err(|error| Status::invalid_argument(format!("invalid vault scope: {error}")))
}

#[allow(clippy::result_large_err)]
pub(crate) fn enforce_vault_scope_access(
    scope: &VaultScope,
    context: &RequestContext,
) -> Result<(), Status> {
    match scope {
        VaultScope::Global => Ok(()),
        VaultScope::Principal { principal_id } => {
            if principal_id == &context.principal {
                Ok(())
            } else {
                Err(Status::permission_denied(
                    "vault principal scope must match authenticated principal context",
                ))
            }
        }
        VaultScope::Channel { channel_name, account_id } => {
            let context_channel = context.channel.as_deref().ok_or_else(|| {
                Status::permission_denied(
                    "vault channel scope requires authenticated channel context",
                )
            })?;
            let expected_with_account = format!("{channel_name}:{account_id}");
            if context_channel == expected_with_account {
                Ok(())
            } else {
                Err(Status::permission_denied(
                    "vault channel scope must match authenticated channel context",
                ))
            }
        }
        VaultScope::Skill { .. } => Err(Status::permission_denied(
            "vault skill scope is not allowed over external RPC context",
        )),
    }
}

pub(crate) fn vault_secret_metadata_message(
    metadata: &VaultSecretMetadata,
) -> gateway_v1::VaultSecretMetadata {
    gateway_v1::VaultSecretMetadata {
        scope: metadata.scope.to_string(),
        key: metadata.key.clone(),
        created_at_unix_ms: metadata.created_at_unix_ms,
        updated_at_unix_ms: metadata.updated_at_unix_ms,
        value_bytes: metadata.value_bytes as u32,
    }
}

pub(crate) fn memory_search_cache_key(request: &MemorySearchRequest) -> String {
    json!({
        "principal": request.principal,
        "channel": request.channel,
        "session_id": request.session_id,
        "query": request.query,
        "top_k": request.top_k,
        "min_score": request.min_score,
        "tags": request.tags,
        "sources": request.sources.iter().map(|source| source.as_str()).collect::<Vec<_>>(),
    })
    .to_string()
}

pub(crate) fn tool_approval_cache_key_prefix(context: &RequestContext, session_id: &str) -> String {
    format!(
        "principal={}|device_id={}|channel={}|session={}|",
        context.principal,
        context.device_id,
        context.channel.as_deref().unwrap_or_default(),
        session_id
    )
}

pub(crate) fn tool_approval_cache_key(
    context: &RequestContext,
    session_id: &str,
    subject_id: &str,
) -> String {
    format!("{}subject={subject_id}", tool_approval_cache_key_prefix(context, session_id))
}

pub(crate) fn tool_approval_outcome_from_record(
    record: &ApprovalRecord,
    fallback_decision: ApprovalDecision,
) -> ToolApprovalOutcome {
    let decision = record.decision.unwrap_or(fallback_decision);
    ToolApprovalOutcome {
        approval_id: record.approval_id.clone(),
        approved: matches!(decision, ApprovalDecision::Allow),
        reason: record.decision_reason.clone().unwrap_or_else(|| "approval resolved".to_owned()),
        decision,
        decision_scope: record.decision_scope.unwrap_or(ApprovalDecisionScope::Once),
        decision_scope_ttl_ms: record.decision_scope_ttl_ms,
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn require_supported_version(v: u32) -> Result<(), Status> {
    if v != CANONICAL_PROTOCOL_MAJOR {
        return Err(Status::failed_precondition("unsupported protocol major version"));
    }
    Ok(())
}

fn normalize_vault_ref_literal(scope: &VaultScope, key: &str) -> String {
    format!("{scope}/{key}").to_ascii_lowercase()
}

pub(crate) fn vault_get_requires_approval(
    scope: &VaultScope,
    key: &str,
    approval_required_refs: &[String],
) -> bool {
    if approval_required_refs.is_empty() {
        return false;
    }
    let candidate = normalize_vault_ref_literal(scope, key);
    approval_required_refs
        .iter()
        .any(|configured| configured.eq_ignore_ascii_case(candidate.as_str()))
}

#[allow(clippy::result_large_err)]
pub(crate) fn enforce_vault_get_approval_policy(
    principal: &str,
    scope: &VaultScope,
    key: &str,
    approval_required_refs: &[String],
    approval_granted: bool,
) -> Result<(), Status> {
    if !vault_get_requires_approval(scope, key, approval_required_refs) {
        return Ok(());
    }
    let evaluation = evaluate_with_config(
        &PolicyRequest {
            principal: principal.to_owned(),
            action: "vault.get".to_owned(),
            resource: format!("secrets:{scope}:{key}"),
        },
        &PolicyEvaluationConfig {
            allow_sensitive_tools: approval_granted,
            sensitive_actions: vec!["vault.get".to_owned()],
            ..PolicyEvaluationConfig::default()
        },
    )
    .map_err(|error| {
        Status::internal(format!("failed to evaluate vault approval policy: {error}"))
    })?;
    match evaluation.decision {
        PolicyDecision::Allow => Ok(()),
        PolicyDecision::DenyByDefault { reason } => Err(Status::permission_denied(format!(
            "vault read requires explicit approval for {scope}/{key}: {reason}"
        ))),
    }
}

#[allow(clippy::result_large_err)]
pub(crate) async fn read_vault_secret_for_context(
    runtime_state: &Arc<GatewayRuntimeState>,
    context: &RequestContext,
    scope: VaultScope,
    key: String,
    approval_granted: bool,
) -> Result<Vec<u8>, Status> {
    enforce_vault_scope_access(&scope, context)?;
    enforce_vault_get_approval_policy(
        context.principal.as_str(),
        &scope,
        key.as_str(),
        runtime_state.config.vault_get_approval_required_refs.as_slice(),
        approval_granted,
    )?;
    authorize_vault_action(
        context.principal.as_str(),
        "vault.get",
        format!("secrets:{scope}:{key}").as_str(),
    )?;
    let value = runtime_state.vault_get_secret(scope.clone(), key.clone()).await?;
    record_vault_journal_event(
        runtime_state,
        context,
        "secret.accessed",
        "vault.get",
        &scope,
        Some(key.as_str()),
        Some(value.len()),
    )
    .await?;
    Ok(value)
}

#[allow(clippy::result_large_err)]
pub(crate) async fn reveal_vault_secret_for_console(
    runtime_state: &Arc<GatewayRuntimeState>,
    context: &RequestContext,
    scope_literal: &str,
    key_literal: &str,
) -> Result<Vec<u8>, Status> {
    let scope = parse_vault_scope(scope_literal)?;
    let key = key_literal.trim().to_owned();
    read_vault_secret_for_context(runtime_state, context, scope, key, true).await
}
