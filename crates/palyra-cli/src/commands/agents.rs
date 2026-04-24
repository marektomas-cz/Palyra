use std::path::{Path, PathBuf};

use anyhow::bail;

use crate::*;

pub(crate) fn run_agents(command: AgentsCommand) -> Result<()> {
    let root_context = app::current_root_context()
        .ok_or_else(|| anyhow!("CLI root context is unavailable for agents command"))?;
    let connection = root_context.resolve_grpc_connection(
        app::ConnectionOverrides::default(),
        app::ConnectionDefaults::ADMIN,
    )?;
    let runtime = build_runtime()?;
    runtime.block_on(run_agents_async(command, connection))
}

pub(crate) async fn run_agents_async(
    command: AgentsCommand,
    connection: AgentConnection,
) -> Result<()> {
    let json = match &command {
        AgentsCommand::List { json, .. }
        | AgentsCommand::Bindings { json, .. }
        | AgentsCommand::Show { json, .. }
        | AgentsCommand::Bind { json, .. }
        | AgentsCommand::Unbind { json, .. }
        | AgentsCommand::SetDefault { json, .. }
        | AgentsCommand::Create { json, .. }
        | AgentsCommand::Delete { json, .. }
        | AgentsCommand::Identity { json, .. } => output::preferred_json(*json),
    };
    let mut client = client::runtime::GatewayRuntimeClient::connect(connection.clone()).await?;

    match command {
        AgentsCommand::List { after, limit, json: _, ndjson } => {
            let ndjson = output::preferred_ndjson(json, ndjson);
            let response = client.list_agents(after, limit).await?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "agents": response.agents.iter().map(agent_to_json).collect::<Vec<_>>(),
                        "default_agent_id": empty_to_none(response.default_agent_id),
                        "next_after_agent_id": empty_to_none(response.next_after_agent_id),
                        "execution_backends": response.execution_backends.iter().map(execution_backend_inventory_to_json).collect::<Vec<_>>(),
                    }))?
                );
            } else if ndjson {
                for agent in &response.agents {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({
                            "type": "agent",
                            "agent": agent_to_json(agent),
                            "is_default": response.default_agent_id == agent.agent_id,
                        }))?
                    );
                }
            } else {
                println!(
                    "agents.list count={} default={} next_after={}",
                    response.agents.len(),
                    text_or_none(response.default_agent_id.as_str()),
                    text_or_none(response.next_after_agent_id.as_str())
                );
                print_execution_backend_inventory(response.execution_backends.as_slice());
                for agent in &response.agents {
                    println!(
                        "agent id={} name={} dir={} workspaces={} model_profile={} backend_preference={}",
                        agent.agent_id,
                        agent.display_name,
                        agent.agent_dir,
                        agent.workspace_roots.len(),
                        agent.default_model_profile,
                        text_or_none(agent.execution_backend_preference.as_str())
                    );
                }
            }
        }
        AgentsCommand::Bindings {
            agent_id,
            principal,
            channel,
            session_id,
            limit,
            json: _,
            ndjson,
        } => {
            let ndjson = output::preferred_ndjson(json, ndjson);
            let response = client
                .list_agent_bindings(AgentBindingsQueryInput {
                    agent_id: agent_id
                        .map(|value| normalize_agent_id_cli(value.as_str()))
                        .transpose()?
                        .unwrap_or_default(),
                    principal: principal.unwrap_or_default(),
                    channel: channel.unwrap_or_default(),
                    session_id: resolve_optional_canonical_id(session_id)?,
                    limit: limit.unwrap_or(250),
                })
                .await?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "bindings": response.bindings.iter().map(agent_binding_to_json).collect::<Vec<_>>(),
                    }))?
                );
            } else if ndjson {
                for binding in &response.bindings {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({
                            "type": "agent_binding",
                            "binding": agent_binding_to_json(binding),
                        }))?
                    );
                }
            } else {
                println!("agents.bindings count={}", response.bindings.len());
                for binding in &response.bindings {
                    println!(
                        "binding agent_id={} principal={} channel={} updated_at_unix_ms={}",
                        binding.agent_id,
                        binding.principal,
                        text_or_none(binding.channel.as_str()),
                        binding.updated_at_unix_ms
                    );
                }
            }
        }
        AgentsCommand::Show { agent_id, json: _ } => {
            let response = client.get_agent(normalize_agent_id_cli(agent_id.as_str())?).await?;
            let agent = response.agent.context("GetAgent returned empty agent payload")?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "agent": agent_to_json(&agent),
                        "is_default": response.is_default,
                        "resolved_execution_backend": empty_to_none(response.resolved_execution_backend),
                        "execution_backend_fallback_used": response.execution_backend_fallback_used,
                        "execution_backend_reason": response.execution_backend_reason,
                        "execution_backends": response.execution_backends.iter().map(execution_backend_inventory_to_json).collect::<Vec<_>>(),
                    }))?
                );
            } else {
                println!(
                    "agents.show id={} name={} dir={} default={} model_profile={} backend_preference={} resolved_backend={} fallback={}",
                    agent.agent_id,
                    agent.display_name,
                    agent.agent_dir,
                    response.is_default,
                    agent.default_model_profile,
                    text_or_none(agent.execution_backend_preference.as_str()),
                    text_or_none(response.resolved_execution_backend.as_str()),
                    response.execution_backend_fallback_used
                );
                println!("agents.show.backend_reason {}", response.execution_backend_reason);
                print_execution_backend_inventory(response.execution_backends.as_slice());
            }
        }
        AgentsCommand::Bind { agent_id, principal, channel, session_id, json: _ } => {
            let response = client
                .bind_agent_for_context(gateway_v1::BindAgentForContextRequest {
                    v: CANONICAL_PROTOCOL_MAJOR,
                    agent_id: normalize_agent_id_cli(agent_id.as_str())?,
                    principal: principal.unwrap_or_else(|| connection.principal.clone()),
                    channel: channel.unwrap_or_else(|| connection.channel.clone()),
                    session_id: Some(common_v1::CanonicalId {
                        ulid: resolve_or_generate_canonical_id(Some(session_id))?,
                    }),
                })
                .await?;
            let binding = response.binding.context("BindAgentForContext returned empty binding")?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "binding": agent_binding_to_json(&binding),
                        "created": response.created,
                    }))?
                );
            } else {
                println!(
                    "agents.bind agent_id={} principal={} channel={} created={}",
                    binding.agent_id,
                    binding.principal,
                    text_or_none(binding.channel.as_str()),
                    response.created
                );
            }
        }
        AgentsCommand::Unbind { principal, channel, session_id, json: _ } => {
            let response = client
                .unbind_agent_for_context(gateway_v1::UnbindAgentForContextRequest {
                    v: CANONICAL_PROTOCOL_MAJOR,
                    principal: principal.unwrap_or_else(|| connection.principal.clone()),
                    channel: channel.unwrap_or_else(|| connection.channel.clone()),
                    session_id: Some(common_v1::CanonicalId {
                        ulid: resolve_or_generate_canonical_id(Some(session_id))?,
                    }),
                })
                .await?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "removed": response.removed,
                        "removed_agent_id": empty_to_none(response.removed_agent_id),
                    }))?
                );
            } else {
                println!(
                    "agents.unbind removed={} removed_agent_id={}",
                    response.removed,
                    text_or_none(response.removed_agent_id.as_str())
                );
            }
        }
        AgentsCommand::SetDefault { agent_id, json: _ } => {
            let response =
                client.set_default_agent(normalize_agent_id_cli(agent_id.as_str())?).await?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "previous_agent_id": empty_to_none(response.previous_agent_id),
                        "default_agent_id": response.default_agent_id,
                    }))?
                );
            } else {
                println!(
                    "agents.set_default previous={} default={}",
                    text_or_none(response.previous_agent_id.as_str()),
                    response.default_agent_id
                );
            }
        }
        AgentsCommand::Create {
            agent_id,
            display_name,
            agent_dir,
            workspace_root,
            model_profile,
            execution_backend,
            tool_allow,
            skill_allow,
            set_default,
            allow_absolute_paths,
            json: _,
        } => {
            let current_dir = std::env::current_dir()
                .context("failed to resolve current directory for agent workspace roots")?;
            reject_unflagged_absolute_agent_dir(agent_dir.as_deref(), allow_absolute_paths)?;
            let (workspace_roots, allow_absolute_paths) = prepare_workspace_roots_for_create(
                workspace_root,
                allow_absolute_paths,
                &current_dir,
            )?;
            let response = client
                .create_agent(gateway_v1::CreateAgentRequest {
                    v: CANONICAL_PROTOCOL_MAJOR,
                    agent_id: normalize_agent_id_cli(agent_id.as_str())?,
                    display_name,
                    agent_dir: agent_dir.unwrap_or_default(),
                    workspace_roots,
                    default_model_profile: model_profile.unwrap_or_default(),
                    execution_backend_preference: execution_backend
                        .and_then(normalize_optional_text_arg)
                        .unwrap_or_default(),
                    default_tool_allowlist: tool_allow,
                    default_skill_allowlist: skill_allow,
                    set_default,
                    allow_absolute_paths,
                })
                .await?;
            let agent = response.agent.context("CreateAgent returned empty agent payload")?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "agent": agent_to_json(&agent),
                        "default_changed": response.default_changed,
                        "default_agent_id": empty_to_none(response.default_agent_id),
                        "resolved_execution_backend": empty_to_none(response.resolved_execution_backend),
                        "execution_backend_fallback_used": response.execution_backend_fallback_used,
                        "execution_backend_reason": response.execution_backend_reason,
                        "execution_backends": response.execution_backends.iter().map(execution_backend_inventory_to_json).collect::<Vec<_>>(),
                    }))?
                );
            } else {
                println!(
                    "agents.create id={} name={} default_changed={} default={} dir={} backend_preference={} resolved_backend={} fallback={}",
                    agent.agent_id,
                    agent.display_name,
                    response.default_changed,
                    text_or_none(response.default_agent_id.as_str()),
                    agent.agent_dir,
                    text_or_none(agent.execution_backend_preference.as_str()),
                    text_or_none(response.resolved_execution_backend.as_str()),
                    response.execution_backend_fallback_used
                );
                println!("agents.create.backend_reason {}", response.execution_backend_reason);
                print_execution_backend_inventory(response.execution_backends.as_slice());
            }
        }
        AgentsCommand::Delete { agent_id, dry_run, yes, json: _ } => {
            let normalized_agent_id = normalize_agent_id_cli(agent_id.as_str())?;
            let agent = client
                .get_agent(normalized_agent_id.clone())
                .await?
                .agent
                .context("GetAgent returned empty agent payload")?;
            let bindings = client
                .list_agent_bindings(AgentBindingsQueryInput {
                    agent_id: normalized_agent_id.clone(),
                    principal: String::new(),
                    channel: String::new(),
                    session_id: None,
                    limit: 1_000,
                })
                .await?
                .bindings;
            if dry_run {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({
                            "agent": agent_to_json(&agent),
                            "would_delete": true,
                            "binding_count": bindings.len(),
                            "bindings": bindings.iter().map(agent_binding_to_json).collect::<Vec<_>>(),
                            "agent_dir_retained": true,
                        }))?
                    );
                } else {
                    println!(
                        "agents.delete.dry_run agent_id={} binding_count={} agent_dir={} agent_dir_retained=true",
                        agent.agent_id,
                        bindings.len(),
                        agent.agent_dir
                    );
                }
            } else {
                if !yes {
                    anyhow::bail!(
                        "agents delete requires --yes; use --dry-run to inspect the impact first"
                    );
                }
                let response = client.delete_agent(normalized_agent_id).await?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({
                            "deleted": response.deleted,
                            "deleted_agent_id": response.deleted_agent_id,
                            "removed_bindings_count": response.removed_bindings_count,
                            "previous_default_agent_id": empty_to_none(response.previous_default_agent_id),
                            "default_agent_id": empty_to_none(response.default_agent_id),
                            "agent_dir": response.agent_dir,
                            "agent_dir_retained": true,
                        }))?
                    );
                } else {
                    println!(
                        "agents.delete deleted={} agent_id={} removed_bindings={} previous_default={} default={} agent_dir={} agent_dir_retained=true",
                        response.deleted,
                        response.deleted_agent_id,
                        response.removed_bindings_count,
                        text_or_none(response.previous_default_agent_id.as_str()),
                        text_or_none(response.default_agent_id.as_str()),
                        response.agent_dir
                    );
                }
            }
        }
        AgentsCommand::Identity {
            principal,
            channel,
            session_id,
            preferred_agent_id,
            persist_binding,
            json: _,
        } => {
            let response = client
                .resolve_agent_for_context(AgentContextResolveInput {
                    principal: principal.unwrap_or_else(|| connection.principal.clone()),
                    channel: channel.unwrap_or_else(|| connection.channel.clone()),
                    session_id: resolve_optional_canonical_id(session_id)?,
                    preferred_agent_id: preferred_agent_id
                        .map(|value| normalize_agent_id_cli(value.as_str()))
                        .transpose()?
                        .unwrap_or_default(),
                    persist_session_binding: persist_binding,
                })
                .await?;
            let agent =
                response.agent.context("ResolveAgentForContext returned empty agent payload")?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "agent": agent_to_json(&agent),
                        "source": agent_resolution_source_label(response.source),
                        "binding_created": response.binding_created,
                        "is_default": response.is_default,
                        "resolved_execution_backend": empty_to_none(response.resolved_execution_backend),
                        "execution_backend_fallback_used": response.execution_backend_fallback_used,
                        "execution_backend_reason": response.execution_backend_reason,
                        "execution_backends": response.execution_backends.iter().map(execution_backend_inventory_to_json).collect::<Vec<_>>(),
                    }))?
                );
            } else {
                println!(
                    "agents.identity agent_id={} source={} binding_created={} default={} backend_preference={} resolved_backend={} fallback={}",
                    agent.agent_id,
                    agent_resolution_source_label(response.source),
                    response.binding_created,
                    response.is_default,
                    text_or_none(agent.execution_backend_preference.as_str()),
                    text_or_none(response.resolved_execution_backend.as_str()),
                    response.execution_backend_fallback_used
                );
                println!("agents.identity.backend_reason {}", response.execution_backend_reason);
                print_execution_backend_inventory(response.execution_backends.as_slice());
            }
        }
    }

    std::io::stdout().flush().context("stdout flush failed")
}

fn agent_binding_to_json(binding: &gateway_v1::AgentBinding) -> Value {
    json!({
        "agent_id": binding.agent_id,
        "principal": binding.principal,
        "channel": empty_to_none(binding.channel.clone()),
        "session_id": if binding.session_id.is_some() { Value::String(REDACTED.to_owned()) } else { Value::Null },
        "updated_at_unix_ms": binding.updated_at_unix_ms,
    })
}

fn execution_backend_inventory_to_json(backend: &gateway_v1::ExecutionBackendInventory) -> Value {
    json!({
        "backend_id": backend.backend_id,
        "label": backend.label,
        "state": backend.state,
        "selectable": backend.selectable,
        "selected_by_default": backend.selected_by_default,
        "description": backend.description,
        "operator_summary": backend.operator_summary,
        "executor_label": empty_to_none(backend.executor_label.clone()),
        "rollout_flag": empty_to_none(backend.rollout_flag.clone()),
        "rollout_enabled": backend.rollout_enabled,
        "capabilities": backend.capabilities,
        "tradeoffs": backend.tradeoffs,
        "active_node_count": backend.active_node_count,
        "total_node_count": backend.total_node_count,
    })
}

fn reject_unflagged_absolute_agent_dir(
    agent_dir: Option<&str>,
    allow_absolute_paths: bool,
) -> Result<()> {
    let Some(agent_dir) = agent_dir.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    if is_cli_absolute_path(agent_dir) && !allow_absolute_paths {
        bail!("absolute agent directory `{agent_dir}` requires --allow-absolute-paths");
    }
    Ok(())
}

fn prepare_workspace_roots_for_create(
    workspace_roots: Vec<String>,
    allow_absolute_paths: bool,
    current_dir: &Path,
) -> Result<(Vec<String>, bool)> {
    let mut normalized = Vec::with_capacity(workspace_roots.len());
    let mut absolutized_relative_root = false;
    for raw in workspace_roots {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            bail!("workspace root cannot be empty");
        }
        if is_cli_absolute_path(trimmed) {
            if !allow_absolute_paths {
                bail!("absolute workspace root `{trimmed}` requires --allow-absolute-paths");
            }
            normalized.push(trimmed.to_owned());
        } else {
            let resolved = current_dir.join(PathBuf::from(trimmed));
            normalized.push(resolved.to_string_lossy().into_owned());
            absolutized_relative_root = true;
        }
    }
    Ok((normalized, allow_absolute_paths || absolutized_relative_root))
}

fn is_cli_absolute_path(value: &str) -> bool {
    Path::new(value).is_absolute()
        || value.starts_with(r"\\")
        || value.as_bytes().get(1).is_some_and(|byte| *byte == b':')
}

fn print_execution_backend_inventory(backends: &[gateway_v1::ExecutionBackendInventory]) {
    if backends.is_empty() {
        return;
    }
    for backend in backends {
        println!(
            "backend id={} state={} selectable={} rollout_enabled={} active_nodes={}/{}",
            backend.backend_id,
            text_or_none(backend.state.as_str()),
            backend.selectable,
            backend.rollout_enabled,
            backend.active_node_count,
            backend.total_node_count
        );
    }
}

fn text_or_none(value: &str) -> &str {
    if value.trim().is_empty() {
        "none"
    } else {
        value
    }
}

fn agent_resolution_source_label(raw: i32) -> &'static str {
    match gateway_v1::AgentResolutionSource::try_from(raw)
        .unwrap_or(gateway_v1::AgentResolutionSource::Unspecified)
    {
        gateway_v1::AgentResolutionSource::SessionBinding => "session_binding",
        gateway_v1::AgentResolutionSource::Default => "default",
        gateway_v1::AgentResolutionSource::Fallback => "fallback",
        gateway_v1::AgentResolutionSource::Unspecified => "unspecified",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn prepare_workspace_roots_resolves_relative_values_against_invocation_dir() -> Result<()> {
        let temp = tempdir()?;
        let (workspace_roots, allow_absolute_paths) =
            prepare_workspace_roots_for_create(vec![".".to_owned()], false, temp.path())?;

        assert_eq!(workspace_roots.len(), 1);
        assert!(Path::new(workspace_roots[0].as_str()).is_absolute());
        assert!(
            workspace_roots[0].contains(temp.path().file_name().unwrap().to_str().unwrap()),
            "resolved root should stay under invocation directory: {}",
            workspace_roots[0]
        );
        assert!(allow_absolute_paths);
        Ok(())
    }

    #[test]
    fn prepare_workspace_roots_requires_flag_for_user_supplied_absolute_values() -> Result<()> {
        let temp = tempdir()?;
        let absolute_root = temp.path().to_string_lossy().into_owned();

        let error = prepare_workspace_roots_for_create(vec![absolute_root], false, temp.path())
            .expect_err("absolute root without flag should fail");

        assert!(format!("{error:#}").contains("--allow-absolute-paths"));
        Ok(())
    }
}
