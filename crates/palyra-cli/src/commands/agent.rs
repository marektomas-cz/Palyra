use crate::*;

fn interactive_session_started_message(session: &gateway_v1::SessionSummary) -> String {
    let session_key = session.session_key.trim();
    let session_label = session.session_label.trim();
    let mut parts =
        vec!["agent.interactive=session_started".to_owned(), "exit_hint=/exit".to_owned()];
    if !session_key.is_empty() {
        parts.push(format!("session_key={session_key}"));
    }
    if !session_label.is_empty() {
        parts.push(format!("session_label={session_label}"));
    }
    parts.push("help_hint=/help".to_owned());
    parts.join(" ")
}

pub(crate) fn run_agent(command: AgentCommand) -> Result<()> {
    let root_context = app::current_root_context()
        .ok_or_else(|| anyhow!("CLI root context is unavailable for agent command"))?;
    match command {
        AgentCommand::Run {
            grpc_url,
            token,
            principal,
            device_id,
            channel,
            session_id,
            session_key,
            session_label,
            require_existing,
            reset_session,
            run_id,
            prompt,
            prompt_stdin,
            allow_sensitive_tools,
            ndjson,
        } => {
            let input_prompt = resolve_prompt_input(prompt, prompt_stdin)?;
            let connection = root_context.resolve_grpc_connection(
                app::ConnectionOverrides {
                    grpc_url,
                    token,
                    principal,
                    device_id,
                    channel,
                    daemon_url: None,
                },
                app::ConnectionDefaults::USER,
            )?;
            let request = build_agent_run_input(
                session_id,
                session_key,
                session_label,
                require_existing,
                reset_session,
                run_id,
                input_prompt,
                allow_sensitive_tools,
            )?;
            execute_agent_stream(connection, request, output::preferred_ndjson(false, ndjson))
        }
        AgentCommand::Interactive {
            grpc_url,
            token,
            principal,
            device_id,
            channel,
            session_id,
            session_key,
            session_label,
            require_existing,
            allow_sensitive_tools,
            ndjson,
        } => {
            let connection = root_context.resolve_grpc_connection(
                app::ConnectionOverrides {
                    grpc_url,
                    token,
                    principal,
                    device_id,
                    channel,
                    daemon_url: None,
                },
                app::ConnectionDefaults::USER,
            )?;
            let runtime = build_runtime()?;
            runtime.block_on(run_agent_interactive_async(
                connection,
                session_id,
                session_key,
                session_label,
                require_existing,
                allow_sensitive_tools,
                ndjson,
            ))
        }
        AgentCommand::AcpShim {
            grpc_url,
            token,
            principal,
            device_id,
            channel,
            session_id,
            session_key,
            session_label,
            require_existing,
            reset_session,
            run_id,
            prompt,
            prompt_stdin,
            allow_sensitive_tools,
            ndjson_stdin,
        } => {
            let connection = root_context.resolve_grpc_connection(
                app::ConnectionOverrides {
                    grpc_url,
                    token,
                    principal,
                    device_id,
                    channel,
                    daemon_url: None,
                },
                app::ConnectionDefaults::USER,
            )?;
            if ndjson_stdin {
                return run_acp_shim_from_stdin(connection, allow_sensitive_tools);
            }

            let input_prompt = resolve_prompt_input(prompt, prompt_stdin)?;
            let request = build_agent_run_input(
                session_id,
                session_key,
                session_label,
                require_existing,
                reset_session,
                run_id,
                input_prompt,
                allow_sensitive_tools,
            )?;
            run_agent_stream_as_acp(connection, request)
        }
        AgentCommand::Acp {
            grpc_url,
            token,
            principal,
            device_id,
            channel,
            allow_sensitive_tools,
        } => {
            let connection = root_context.resolve_grpc_connection(
                app::ConnectionOverrides {
                    grpc_url,
                    token,
                    principal,
                    device_id,
                    channel,
                    daemon_url: None,
                },
                app::ConnectionDefaults::USER,
            )?;
            acp_bridge::run_agent_acp_bridge(connection, allow_sensitive_tools)
        }
    }
}

async fn run_agent_interactive_async(
    connection: AgentConnection,
    session_id: Option<String>,
    session_key: Option<String>,
    session_label: Option<String>,
    require_existing: bool,
    allow_sensitive_tools: bool,
    ndjson: bool,
) -> Result<()> {
    let mut client = client::runtime::GatewayRuntimeClient::connect(connection.clone()).await?;
    let mut session = client
        .resolve_session(gateway_v1::ResolveSessionRequest {
            v: CANONICAL_PROTOCOL_MAJOR,
            session_id: session_id
                .map(|value| resolve_or_generate_canonical_id(Some(value)))
                .transpose()?
                .map(|ulid| common_v1::CanonicalId { ulid }),
            session_key: session_key.unwrap_or_default(),
            session_label: session_label.unwrap_or_default(),
            require_existing,
            reset_session: false,
        })
        .await?
        .session
        .context("ResolveSession returned empty session payload")?;
    let started_message = interactive_session_started_message(&session);
    eprintln!("{started_message}");
    std::io::stderr().flush().context("stderr flush failed")?;

    let stdin = std::io::stdin();
    let mut last_run_id = None::<String>;
    for line in stdin.lock().lines() {
        let prompt = line.context("failed to read interactive prompt from stdin")?;
        let prompt = prompt.trim();
        if prompt.is_empty() {
            continue;
        }
        if prompt.eq_ignore_ascii_case("/exit") {
            break;
        }
        if prompt.eq_ignore_ascii_case("/help") {
            eprintln!(
                "agent.interactive.commands /help /session /reset /abort [run_id] /exit"
            );
            std::io::stderr().flush().context("stderr flush failed")?;
            continue;
        }
        if prompt.eq_ignore_ascii_case("/session") {
            session = client
                .resolve_session(gateway_v1::ResolveSessionRequest {
                    v: CANONICAL_PROTOCOL_MAJOR,
                    session_id: session.session_id.clone(),
                    session_key: String::new(),
                    session_label: String::new(),
                    require_existing: true,
                    reset_session: false,
                })
                .await?
                .session
                .context("ResolveSession returned empty session payload")?;
            eprintln!(
                "agent.interactive.session key={} label={} updated_at_unix_ms={} last_run_id={}",
                if session.session_key.trim().is_empty() {
                    "none"
                } else {
                    session.session_key.as_str()
                },
                if session.session_label.trim().is_empty() {
                    "none"
                } else {
                    session.session_label.as_str()
                },
                session.updated_at_unix_ms,
                session
                    .last_run_id
                    .as_ref()
                    .map(|value| value.ulid.as_str())
                    .unwrap_or("none")
            );
            std::io::stderr().flush().context("stderr flush failed")?;
            continue;
        }
        if prompt.eq_ignore_ascii_case("/reset") {
            session = client
                .resolve_session(gateway_v1::ResolveSessionRequest {
                    v: CANONICAL_PROTOCOL_MAJOR,
                    session_id: session.session_id.clone(),
                    session_key: String::new(),
                    session_label: String::new(),
                    require_existing: true,
                    reset_session: true,
                })
                .await?
                .session
                .context("ResolveSession returned empty session payload")?;
            eprintln!("agent.interactive.reset session_reset=true");
            std::io::stderr().flush().context("stderr flush failed")?;
            continue;
        }
        if let Some(run_id) = prompt.strip_prefix("/abort").map(str::trim) {
            let run_id = if run_id.is_empty() {
                last_run_id
                    .clone()
                    .context("/abort without explicit run_id requires a previous run")?
            } else {
                resolve_or_generate_canonical_id(Some(run_id.to_owned()))?
            };
            let response = client.abort_run(run_id.clone(), Some("interactive_abort".to_owned())).await?;
            eprintln!(
                "agent.interactive.abort run_id={} cancel_requested={} reason={}",
                response.run_id.as_ref().map(|value| value.ulid.as_str()).unwrap_or(run_id.as_str()),
                response.cancel_requested,
                if response.reason.trim().is_empty() {
                    "none"
                } else {
                    response.reason.as_str()
                }
            );
            std::io::stderr().flush().context("stderr flush failed")?;
            continue;
        }

        let request = build_agent_run_input(
            session.session_id.as_ref().map(|value| value.ulid.clone()),
            None,
            None,
            true,
            false,
            None,
            prompt.to_owned(),
            allow_sensitive_tools,
        )?;
        last_run_id = Some(request.run_id.clone());
        execute_agent_stream(connection.clone(), request, ndjson)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::interactive_session_started_message;
    use crate::proto::palyra::{common::v1 as common_v1, gateway::v1 as gateway_v1};

    #[test]
    fn interactive_session_started_message_omits_session_identifier() {
        let banner = interactive_session_started_message(&gateway_v1::SessionSummary {
            session_id: Some(common_v1::CanonicalId {
                ulid: "01ARZ3NDEKTSV4RRFFQ69G5FAW".to_owned(),
            }),
            session_key: "ops:triage".to_owned(),
            session_label: "Ops Triage".to_owned(),
            created_at_unix_ms: 0,
            updated_at_unix_ms: 0,
            last_run_id: None,
            archived_at_unix_ms: 0,
        });
        assert!(banner.contains("agent.interactive=session_started"));
        assert!(banner.contains("exit_hint=/exit"));
        assert!(banner.contains("session_key=ops:triage"));
        assert!(!banner.contains("session_id="));
    }
}
