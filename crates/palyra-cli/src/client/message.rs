use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::commands::channels::{post_connector_action, resolve_connector_status};

pub(crate) const SUPPORTED_MESSAGE_ACTIONS: &[&str] = &["send", "thread"];
pub(crate) const UNSUPPORTED_MESSAGE_ACTIONS: &[&str] =
    &["read", "search", "edit", "delete", "react"];

#[derive(Debug, Clone)]
pub(crate) struct MessageDispatchOptions {
    pub connector_id: String,
    pub target: String,
    pub text: String,
    pub confirm: bool,
    pub auto_reaction: Option<String>,
    pub thread_id: Option<String>,
    pub url: Option<String>,
    pub token: Option<String>,
    pub principal: String,
    pub device_id: String,
    pub channel: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct MessageCapabilities {
    pub provider_kind: String,
    pub supported_actions: Vec<&'static str>,
    pub unsupported_actions: Vec<&'static str>,
}

pub(crate) fn connector_kind(payload: &Value) -> Option<&str> {
    payload.get("connector").unwrap_or(payload).get("kind").and_then(Value::as_str)
}

pub(crate) fn load_capabilities(
    connector_id: &str,
    url: Option<String>,
    token: Option<String>,
    principal: String,
    device_id: String,
    channel: Option<String>,
) -> Result<MessageCapabilities> {
    let status = resolve_connector_status(
        connector_id,
        url,
        token,
        principal,
        device_id,
        channel,
        "failed to call channels status endpoint for message capabilities",
    )?;
    let provider_kind = connector_kind(&status).unwrap_or("unknown").to_owned();
    let supported_actions = if provider_kind.eq_ignore_ascii_case("discord") {
        SUPPORTED_MESSAGE_ACTIONS.to_vec()
    } else {
        Vec::new()
    };
    Ok(MessageCapabilities {
        provider_kind,
        supported_actions,
        unsupported_actions: UNSUPPORTED_MESSAGE_ACTIONS.to_vec(),
    })
}

pub(crate) fn send_message(options: MessageDispatchOptions) -> Result<Value> {
    ensure_discord_send_supported(
        options.connector_id.as_str(),
        options.url.clone(),
        options.token.clone(),
        options.principal.clone(),
        options.device_id.clone(),
        options.channel.clone(),
    )?;
    post_connector_action(
        options.connector_id.as_str(),
        "/test-send",
        Some(json!({
            "target": options.target,
            "text": options.text,
            "confirm": options.confirm,
            "auto_reaction": options.auto_reaction,
            "thread_id": options.thread_id,
        })),
        options.url,
        options.token,
        options.principal,
        options.device_id,
        options.channel,
        "failed to call message send endpoint",
    )
}

pub(crate) fn ensure_discord_send_supported(
    connector_id: &str,
    url: Option<String>,
    token: Option<String>,
    principal: String,
    device_id: String,
    channel: Option<String>,
) -> Result<()> {
    let status = resolve_connector_status(
        connector_id,
        url,
        token,
        principal,
        device_id,
        channel,
        "failed to call channels status endpoint for message send",
    )?;
    let kind = connector_kind(&status).unwrap_or("unknown");
    if !kind.eq_ignore_ascii_case("discord") {
        bail!(
            "unsupported message capability for connector '{connector_id}': direct send/thread currently requires a discord connector (detected kind={kind})"
        );
    }
    Ok(())
}

pub(crate) fn unsupported_message_action(
    action: &str,
    connector_id: &str,
    detail: Option<&str>,
) -> Result<()> {
    let detail_suffix = detail.filter(|value| !value.trim().is_empty()).unwrap_or("none");
    bail!(
        "unsupported message capability '{action}' for connector '{connector_id}'; supported actions today: {}; detail={detail_suffix}",
        SUPPORTED_MESSAGE_ACTIONS.join(", "),
    )
}

pub(crate) fn encode_capabilities_json(
    connector_id: &str,
    capabilities: &MessageCapabilities,
) -> Value {
    json!({
        "connector_id": connector_id,
        "provider_kind": capabilities.provider_kind,
        "supported_actions": capabilities.supported_actions,
        "unsupported_actions": capabilities.unsupported_actions,
        "notes": [
            "send and thread reuse the authenticated admin channels transport",
            "direct outbound delivery currently maps to the existing discord test-send backend",
            "read/search/edit/delete/react remain capability-gated until connector CRUD APIs exist"
        ],
    })
}

pub(crate) fn encode_dispatch_json(action: &str, connector_id: &str, response: Value) -> Value {
    json!({
        "action": action,
        "connector_id": connector_id,
        "response": response,
    })
}
