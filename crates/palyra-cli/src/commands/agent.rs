use crate::*;

pub(crate) fn run_agent(command: AgentCommand) -> Result<()> {
    match command {
        AgentCommand::Run {
            grpc_url,
            token,
            principal,
            device_id,
            channel,
            session_id,
            run_id,
            prompt,
            prompt_stdin,
            allow_sensitive_tools,
            ndjson,
        } => {
            let input_prompt = resolve_prompt_input(prompt, prompt_stdin)?;
            let connection = AgentConnection {
                grpc_url: resolve_grpc_url(grpc_url)?,
                token: token.or_else(|| env::var("PALYRA_ADMIN_TOKEN").ok()),
                principal,
                device_id,
                channel,
            };
            let request =
                build_agent_run_input(session_id, run_id, input_prompt, allow_sensitive_tools)?;
            execute_agent_stream(connection, request, ndjson)
        }
        AgentCommand::Interactive {
            grpc_url,
            token,
            principal,
            device_id,
            channel,
            session_id,
            allow_sensitive_tools,
            ndjson,
        } => {
            let connection = AgentConnection {
                grpc_url: resolve_grpc_url(grpc_url)?,
                token: token.or_else(|| env::var("PALYRA_ADMIN_TOKEN").ok()),
                principal,
                device_id,
                channel,
            };
            let session_id = resolve_or_generate_canonical_id(session_id)?;
            if ndjson {
                eprintln!(
                    "agent.interactive=session_started session_id={} exit_hint=/exit",
                    session_id
                );
                std::io::stderr().flush().context("stderr flush failed")?;
            } else {
                println!(
                    "agent.interactive=session_started session_id={} exit_hint=/exit",
                    session_id
                );
                std::io::stdout().flush().context("stdout flush failed")?;
            }

            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                let prompt = line.context("failed to read interactive prompt from stdin")?;
                let prompt = prompt.trim();
                if prompt.is_empty() {
                    continue;
                }
                if prompt.eq_ignore_ascii_case("/exit") {
                    break;
                }
                let request = AgentRunInput {
                    session_id: session_id.clone(),
                    run_id: generate_canonical_ulid(),
                    prompt: prompt.to_owned(),
                    allow_sensitive_tools,
                };
                execute_agent_stream(connection.clone(), request, ndjson)?;
            }
            Ok(())
        }
        AgentCommand::AcpShim {
            grpc_url,
            token,
            principal,
            device_id,
            channel,
            session_id,
            run_id,
            prompt,
            prompt_stdin,
            allow_sensitive_tools,
            ndjson_stdin,
        } => {
            let connection = AgentConnection {
                grpc_url: resolve_grpc_url(grpc_url)?,
                token: token.or_else(|| env::var("PALYRA_ADMIN_TOKEN").ok()),
                principal,
                device_id,
                channel,
            };
            if ndjson_stdin {
                return run_acp_shim_from_stdin(connection, allow_sensitive_tools);
            }

            let input_prompt = resolve_prompt_input(prompt, prompt_stdin)?;
            let request =
                build_agent_run_input(session_id, run_id, input_prompt, allow_sensitive_tools)?;
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
            let connection = AgentConnection {
                grpc_url: resolve_grpc_url(grpc_url)?,
                token: token.or_else(|| env::var("PALYRA_ADMIN_TOKEN").ok()),
                principal,
                device_id,
                channel,
            };
            acp_bridge::run_agent_acp_bridge(connection, allow_sensitive_tools)
        }
    }
}
