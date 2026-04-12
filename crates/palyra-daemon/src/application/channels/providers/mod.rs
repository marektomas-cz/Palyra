//! Central provider dispatch for daemon channel application flows.
//!
//! Provider-specific behavior is delegated into submodules so generic handlers
//! do not accumulate scattered Discord branching over time.

use serde_json::{json, Value};

use crate::{app::state::AppState, journal::ApprovalRiskLevel, *};

pub(crate) mod discord;

pub(crate) fn build_channel_provider_operations_payload(
    connector_id: &str,
    connector: &palyra_connectors::ConnectorStatusSnapshot,
    runtime: Option<&Value>,
    recent_dead_letters: &[palyra_connectors::DeadLetterRecord],
) -> Value {
    match connector.kind {
        palyra_connectors::ConnectorKind::Discord => {
            discord::build_discord_channel_operations_payload(
                connector_id,
                connector,
                runtime,
                recent_dead_letters,
            )
        }
        _ => Value::Null,
    }
}

#[allow(clippy::result_large_err)]
pub(crate) async fn build_channel_provider_health_refresh_payload(
    state: &AppState,
    connector_id: &str,
    verify_channel_id: Option<String>,
) -> Result<Value, Response> {
    let connector = state.channels.status(connector_id).map_err(channel_platform_error_response)?;
    match connector.kind {
        palyra_connectors::ConnectorKind::Discord => {
            discord::build_discord_channel_health_refresh_payload(
                state,
                connector_id,
                verify_channel_id,
            )
            .await
        }
        _ => Ok(json!({
            "supported": false,
            "message": "health refresh is currently implemented for Discord connectors only",
        })),
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn classify_channel_message_mutation_governance(
    state: &AppState,
    connector_id: &str,
    preview: &palyra_connectors::ConnectorMessageRecord,
    operation: channels::DiscordMessageMutationKind,
) -> Result<channels::DiscordMessageMutationGovernance, Response> {
    let connector = state.channels.status(connector_id).map_err(channel_platform_error_response)?;
    match connector.kind {
        palyra_connectors::ConnectorKind::Discord => {
            let instance = state
                .channels
                .connector_instance(connector_id)
                .map_err(channel_platform_error_response)?;
            Ok(channels::classify_discord_message_mutation_governance(
                &instance,
                preview,
                operation,
                unix_ms_now().map_err(|error| {
                    runtime_status_response(tonic::Status::internal(sanitize_http_error_message(
                        error.to_string().as_str(),
                    )))
                })?,
            ))
        }
        _ => Ok(channels::DiscordMessageMutationGovernance {
            risk_level: ApprovalRiskLevel::High,
            approval_required: true,
            reason: "non-Discord connector mutation defaults to explicit approval".to_owned(),
        }),
    }
}

pub(crate) fn channel_message_policy_action(
    operation: channels::DiscordMessageMutationKind,
) -> &'static str {
    discord::channel_message_policy_action(operation)
}

pub(crate) fn channel_message_required_permissions(
    operation: channels::DiscordMessageMutationKind,
) -> Vec<String> {
    discord::channel_message_required_permissions(operation)
}

pub(crate) fn find_matching_message<'a, I>(messages: I, needles: &[&str]) -> Option<String>
where
    I: IntoIterator<Item = Option<&'a str>>,
{
    messages.into_iter().flatten().find_map(|message| {
        let normalized = message.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return None;
        }
        if needles.iter().any(|needle| normalized.contains(needle)) {
            Some(sanitize_http_error_message(message.trim()))
        } else {
            None
        }
    })
}
