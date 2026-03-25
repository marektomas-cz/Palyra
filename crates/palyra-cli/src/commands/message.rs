use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::Write;

use crate::{
    args::MessageCommand, normalize_optional_text_arg, normalize_required_text_arg, output,
};

use super::channels;

const SUPPORTED_MESSAGE_ACTIONS: &[&str] = &["send", "thread"];
const UNSUPPORTED_MESSAGE_ACTIONS: &[&str] =
    &["read", "search", "edit", "delete", "react"];

pub(crate) fn run_message(command: MessageCommand) -> Result<()> {
    match command {
        MessageCommand::Capabilities { connector_id, url, token, principal, device_id, channel, json } => {
            let status = channels::get_connector_status(
                connector_id.as_str(),
                url,
                token,
                principal,
                device_id,
                channel,
                "failed to call channels status endpoint for message capabilities",
            )?;
            emit_capabilities(connector_id.as_str(), &status, output::preferred_json(json))?;
        }
        MessageCommand::Send {
            connector_id,
            to,
            text,
            confirm,
            auto_reaction,
            thread_id,
            url,
            token,
            principal,
            device_id,
            channel,
            json,
        } => {
            let text = normalize_required_text_arg(text, "text")?;
            let target = normalize_required_text_arg(to, "to")?;
            ensure_discord_send_supported(
                connector_id.as_str(),
                url.clone(),
                token.clone(),
                principal.clone(),
                device_id.clone(),
                channel.clone(),
            )?;
            let response = channels::post_connector_action(
                connector_id.as_str(),
                "/test-send",
                Some(json!({
                    "target": target,
                    "text": text,
                    "confirm": confirm,
                    "auto_reaction": auto_reaction.and_then(normalize_optional_text_arg),
                    "thread_id": thread_id.and_then(normalize_optional_text_arg),
                })),
                url,
                token,
                principal,
                device_id,
                channel,
                "failed to call message send endpoint",
            )?;
            emit_send("send", connector_id.as_str(), response, output::preferred_json(json))?;
        }
        MessageCommand::Thread {
            connector_id,
            to,
            text,
            thread_id,
            confirm,
            auto_reaction,
            url,
            token,
            principal,
            device_id,
            channel,
            json,
        } => {
            let text = normalize_required_text_arg(text, "text")?;
            let target = normalize_required_text_arg(to, "to")?;
            let thread_id = normalize_required_text_arg(thread_id, "thread_id")?;
            ensure_discord_send_supported(
                connector_id.as_str(),
                url.clone(),
                token.clone(),
                principal.clone(),
                device_id.clone(),
                channel.clone(),
            )?;
            let response = channels::post_connector_action(
                connector_id.as_str(),
                "/test-send",
                Some(json!({
                    "target": target,
                    "text": text,
                    "confirm": confirm,
                    "auto_reaction": auto_reaction.and_then(normalize_optional_text_arg),
                    "thread_id": thread_id,
                })),
                url,
                token,
                principal,
                device_id,
                channel,
                "failed to call message thread endpoint",
            )?;
            emit_send("thread", connector_id.as_str(), response, output::preferred_json(json))?;
        }
        MessageCommand::Read { connector_id, message_id, json } => {
            unsupported_message_action("read", connector_id.as_str(), Some(message_id.as_str()), json)?
        }
        MessageCommand::Search { connector_id, query, json } => {
            unsupported_message_action("search", connector_id.as_str(), Some(query.as_str()), json)?
        }
        MessageCommand::Edit { connector_id, message_id, text, json } => {
            let detail = format!("message_id={message_id} text={text}");
            unsupported_message_action("edit", connector_id.as_str(), Some(detail.as_str()), json)?
        }
        MessageCommand::Delete { connector_id, message_id, json } => {
            unsupported_message_action("delete", connector_id.as_str(), Some(message_id.as_str()), json)?
        }
        MessageCommand::React { connector_id, message_id, emoji, remove, json } => {
            let detail = format!("message_id={message_id} emoji={emoji} remove={remove}");
            unsupported_message_action("react", connector_id.as_str(), Some(detail.as_str()), json)?
        }
    }

    std::io::stdout().flush().context("stdout flush failed")
}

fn ensure_discord_send_supported(
    connector_id: &str,
    url: Option<String>,
    token: Option<String>,
    principal: String,
    device_id: String,
    channel: Option<String>,
) -> Result<()> {
    let status = channels::get_connector_status(
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

fn connector_kind(payload: &Value) -> Option<&str> {
    payload
        .get("connector")
        .unwrap_or(payload)
        .get("kind")
        .and_then(Value::as_str)
}

fn emit_capabilities(connector_id: &str, status: &Value, json_output: bool) -> Result<()> {
    let provider_kind = connector_kind(status).unwrap_or("unknown");
    let supported_actions: &[&str] = if provider_kind.eq_ignore_ascii_case("discord") {
        SUPPORTED_MESSAGE_ACTIONS
    } else {
        &[]
    };
    let payload = json!({
        "connector_id": connector_id,
        "provider_kind": provider_kind,
        "supported_actions": supported_actions,
        "unsupported_actions": UNSUPPORTED_MESSAGE_ACTIONS,
        "notes": [
            "send and thread reuse the authenticated admin channels transport",
            "direct outbound delivery currently maps to the existing discord test-send backend",
            "read/search/edit/delete/react remain capability-gated until connector CRUD APIs exist"
        ],
    });
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .context("failed to encode message capabilities payload as JSON")?
        );
    } else {
        println!(
            "message.capabilities connector_id={} provider_kind={} supported={} unsupported={}",
            connector_id,
            provider_kind,
            if supported_actions.is_empty() { "none".to_owned() } else { supported_actions.join(",") },
            UNSUPPORTED_MESSAGE_ACTIONS.join(",")
        );
    }
    Ok(())
}

fn emit_send(action: &str, connector_id: &str, response: Value, json_output: bool) -> Result<()> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "action": action,
                "connector_id": connector_id,
                "response": response,
            }))
            .context("failed to encode message send payload as JSON")?
        );
    } else {
        let dispatch = response.get("dispatch").unwrap_or(&response);
        println!(
            "message.{} connector_id={} target={} delivered={} retried={} dead_lettered={}",
            action,
            connector_id,
            dispatch.get("target").and_then(Value::as_str).unwrap_or("unknown"),
            dispatch.get("delivered").and_then(Value::as_u64).unwrap_or(0),
            dispatch.get("retried").and_then(Value::as_u64).unwrap_or(0),
            dispatch.get("dead_lettered").and_then(Value::as_u64).unwrap_or(0)
        );
    }
    Ok(())
}

fn unsupported_message_action(
    action: &str,
    connector_id: &str,
    detail: Option<&str>,
    _json_output: bool,
) -> Result<()> {
    let detail_suffix = detail.filter(|value| !value.trim().is_empty()).unwrap_or("none");
    bail!(
        "unsupported message capability '{action}' for connector '{connector_id}'; supported actions today: {}; detail={detail_suffix}",
        SUPPORTED_MESSAGE_ACTIONS.join(", "),
    )
}
