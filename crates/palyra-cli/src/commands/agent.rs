use crate::*;

fn interactive_session_started_message() -> &'static str {
    "agent.interactive=session_started exit_hint=/exit"
}

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
            let started_message = interactive_session_started_message();
            if ndjson {
                eprintln!("{started_message}");
                std::io::stderr().flush().context("stderr flush failed")?;
            } else {
                println!("{started_message}");
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

#[cfg(test)]
mod tests {
    use super::interactive_session_started_message;

    #[test]
    fn interactive_session_started_message_omits_session_identifier() {
        let banner = interactive_session_started_message();
        assert!(banner.contains("agent.interactive=session_started"));
        assert!(banner.contains("exit_hint=/exit"));
        assert!(!banner.contains("session_id="));
    }
}
