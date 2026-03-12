use palyra_policy::{
    evaluate_with_config, evaluate_with_context, PolicyDecision, PolicyEvaluationConfig,
    PolicyRequest, PolicyRequestContext,
};
use tonic::Status;

#[derive(Clone, Copy)]
pub(crate) enum SensitiveServiceRole {
    AdminOnly,
    AdminOrSystem,
}

#[must_use]
pub(crate) fn principal_has_sensitive_service_role(
    principal: &str,
    role: SensitiveServiceRole,
) -> bool {
    let normalized_principal = principal.to_ascii_lowercase();
    match role {
        SensitiveServiceRole::AdminOnly => normalized_principal.starts_with("admin:"),
        SensitiveServiceRole::AdminOrSystem => {
            normalized_principal.starts_with("admin:")
                || normalized_principal.starts_with("system:")
        }
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn authorize_cron_action(
    principal: &str,
    action: &str,
    resource: &str,
) -> Result<(), Status> {
    authorize_policy_action(principal, action, resource, "cron")
}

#[allow(clippy::result_large_err)]
pub(crate) fn authorize_message_action(
    principal: &str,
    action: &str,
    resource: &str,
    channel: Option<&str>,
    _session_id: Option<&str>,
    _run_id: Option<&str>,
) -> Result<(), Status> {
    let evaluation = evaluate_with_context(
        &PolicyRequest {
            principal: principal.to_owned(),
            action: action.to_owned(),
            resource: resource.to_owned(),
        },
        &PolicyRequestContext {
            channel: channel.map(str::to_owned),
            ..PolicyRequestContext::default()
        },
        &PolicyEvaluationConfig::default(),
    )
    .map_err(|error| {
        Status::internal(format!("failed to evaluate message routing policy: {error}"))
    })?;
    map_policy_decision(action, resource, evaluation.decision)
}

#[allow(clippy::result_large_err)]
pub(crate) fn authorize_memory_action(
    principal: &str,
    action: &str,
    resource: &str,
) -> Result<(), Status> {
    authorize_policy_action(principal, action, resource, "memory")
}

#[allow(clippy::result_large_err)]
pub(crate) fn authorize_vault_action(
    principal: &str,
    action: &str,
    resource: &str,
) -> Result<(), Status> {
    authorize_policy_action(principal, action, resource, "vault")
}

#[allow(clippy::result_large_err)]
pub(crate) fn authorize_agent_management_action(
    principal: &str,
    action: &str,
    resource: &str,
) -> Result<(), Status> {
    authorize_sensitive_service_action(
        principal,
        action,
        resource,
        "agent",
        SensitiveServiceRole::AdminOnly,
        "agent management requires admin principal prefix 'admin:'",
    )
}

#[allow(clippy::result_large_err)]
pub(crate) fn authorize_auth_profile_action(
    principal: &str,
    action: &str,
    resource: &str,
) -> Result<(), Status> {
    authorize_sensitive_service_action(
        principal,
        action,
        resource,
        "auth profile",
        SensitiveServiceRole::AdminOrSystem,
        "auth profile management requires admin/system principal prefix",
    )
}

#[allow(clippy::result_large_err)]
pub(crate) fn authorize_approvals_action(
    principal: &str,
    action: &str,
    resource: &str,
) -> Result<(), Status> {
    authorize_sensitive_service_action(
        principal,
        action,
        resource,
        "approvals",
        SensitiveServiceRole::AdminOrSystem,
        "approvals APIs require admin/system principal prefix",
    )
}

#[allow(clippy::result_large_err)]
fn authorize_policy_action(
    principal: &str,
    action: &str,
    resource: &str,
    surface: &str,
) -> Result<(), Status> {
    let evaluation = evaluate_with_config(
        &PolicyRequest {
            principal: principal.to_owned(),
            action: action.to_owned(),
            resource: resource.to_owned(),
        },
        &PolicyEvaluationConfig::default(),
    )
    .map_err(|error| Status::internal(format!("failed to evaluate {surface} policy: {error}")))?;
    map_policy_decision(action, resource, evaluation.decision)
}

#[allow(clippy::result_large_err)]
fn authorize_sensitive_service_action(
    principal: &str,
    action: &str,
    resource: &str,
    surface: &str,
    role: SensitiveServiceRole,
    allow_reason: &str,
) -> Result<(), Status> {
    let evaluation = evaluate_with_config(
        &PolicyRequest {
            principal: principal.to_owned(),
            action: action.to_owned(),
            resource: resource.to_owned(),
        },
        &PolicyEvaluationConfig::default(),
    )
    .map_err(|error| Status::internal(format!("failed to evaluate {surface} policy: {error}")))?;
    if principal_has_sensitive_service_role(principal, role) {
        return Ok(());
    }
    let reason = match evaluation.decision {
        PolicyDecision::Allow => allow_reason.to_owned(),
        PolicyDecision::DenyByDefault { reason } => reason,
    };
    Err(Status::permission_denied(format!(
        "policy denied action '{action}' on '{resource}': {reason}"
    )))
}

#[allow(clippy::result_large_err)]
fn map_policy_decision(
    action: &str,
    resource: &str,
    decision: PolicyDecision,
) -> Result<(), Status> {
    match decision {
        PolicyDecision::Allow => Ok(()),
        PolicyDecision::DenyByDefault { reason } => Err(Status::permission_denied(format!(
            "policy denied action '{action}' on '{resource}': {reason}"
        ))),
    }
}
