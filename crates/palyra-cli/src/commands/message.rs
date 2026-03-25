use anyhow::{Context, Result};
use serde_json::Value;
use std::io::Write;

use crate::{
    app,
    args::MessageCommand,
    build_runtime,
    client::{message, operator::OperatorRuntime},
    normalize_optional_text_arg, normalize_required_text_arg, output,
};

pub(crate) fn run_message(command: MessageCommand) -> Result<()> {
    let root_context = app::current_root_context()
        .ok_or_else(|| anyhow::anyhow!("CLI root context is unavailable for message command"))?;
    let connection = root_context.resolve_grpc_connection(
        app::ConnectionOverrides::default(),
        app::ConnectionDefaults::USER,
    )?;
    let runtime = build_runtime()?;
    runtime.block_on(run_message_async(command, OperatorRuntime::new(connection)))
}

async fn run_message_async(command: MessageCommand, runtime: OperatorRuntime) -> Result<()> {
    match command {
        MessageCommand::Capabilities {
            connector_id,
            url,
            token,
            principal,
            device_id,
            channel,
            json,
        } => {
            let capabilities = runtime
                .message_capabilities(
                    connector_id.clone(),
                    url,
                    token,
                    principal,
                    device_id,
                    channel,
                )
                .await?;
            emit_capabilities(connector_id.as_str(), &capabilities, output::preferred_json(json))?;
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
            let response = runtime
                .send_message(message::MessageDispatchOptions {
                    connector_id: connector_id.clone(),
                    target,
                    text,
                    confirm,
                    auto_reaction: auto_reaction.and_then(normalize_optional_text_arg),
                    thread_id: thread_id.and_then(normalize_optional_text_arg),
                    url,
                    token,
                    principal,
                    device_id,
                    channel,
                })
                .await?;
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
            let response = runtime
                .send_message(message::MessageDispatchOptions {
                    connector_id: connector_id.clone(),
                    target,
                    text,
                    confirm,
                    auto_reaction: auto_reaction.and_then(normalize_optional_text_arg),
                    thread_id: Some(thread_id),
                    url,
                    token,
                    principal,
                    device_id,
                    channel,
                })
                .await?;
            emit_send("thread", connector_id.as_str(), response, output::preferred_json(json))?;
        }
        MessageCommand::Read { connector_id, message_id, json } => unsupported_message_action(
            "read",
            connector_id.as_str(),
            Some(message_id.as_str()),
            json,
        )?,
        MessageCommand::Search { connector_id, query, json } => {
            unsupported_message_action("search", connector_id.as_str(), Some(query.as_str()), json)?
        }
        MessageCommand::Edit { connector_id, message_id, text, json } => {
            let detail = format!("message_id={message_id} text={text}");
            unsupported_message_action("edit", connector_id.as_str(), Some(detail.as_str()), json)?
        }
        MessageCommand::Delete { connector_id, message_id, json } => unsupported_message_action(
            "delete",
            connector_id.as_str(),
            Some(message_id.as_str()),
            json,
        )?,
        MessageCommand::React { connector_id, message_id, emoji, remove, json } => {
            let detail = format!("message_id={message_id} emoji={emoji} remove={remove}");
            unsupported_message_action("react", connector_id.as_str(), Some(detail.as_str()), json)?
        }
    }

    std::io::stdout().flush().context("stdout flush failed")
}

fn emit_capabilities(
    connector_id: &str,
    capabilities: &message::MessageCapabilities,
    json_output: bool,
) -> Result<()> {
    let payload = message::encode_capabilities_json(connector_id, capabilities);
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
            capabilities.provider_kind,
            if capabilities.supported_actions.is_empty() {
                "none".to_owned()
            } else {
                capabilities.supported_actions.join(",")
            },
            capabilities.unsupported_actions.join(",")
        );
    }
    Ok(())
}

fn emit_send(action: &str, connector_id: &str, response: Value, json_output: bool) -> Result<()> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&message::encode_dispatch_json(
                action,
                connector_id,
                response.clone(),
            ))
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
    json_output: bool,
) -> Result<()> {
    let _ = json_output;
    message::unsupported_message_action(action, connector_id, detail)
}
