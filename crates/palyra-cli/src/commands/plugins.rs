use palyra_control_plane as control_plane;
use serde_json::Value;

use crate::*;

pub(crate) fn run_plugins(command: PluginsCommand) -> Result<()> {
    let runtime = build_runtime()?;
    runtime.block_on(run_plugins_async(command))
}

async fn run_plugins_async(command: PluginsCommand) -> Result<()> {
    let context =
        client::control_plane::connect_admin_console(app::ConnectionOverrides::default()).await?;
    match command {
        PluginsCommand::List { plugin_id, skill_id, enabled_only, ready_only, json } => {
            let envelope = context
                .client
                .list_plugins(&control_plane::PluginBindingsQuery { plugin_id, skill_id })
                .await?;
            emit_plugin_list(envelope, enabled_only, ready_only, json)
        }
        PluginsCommand::Info { plugin_id, json } => {
            let envelope = context.client.get_plugin(plugin_id.as_str()).await?;
            emit_plugin_envelope("plugins.info", &envelope, json)
        }
        PluginsCommand::Check { plugin_id, json } => {
            let envelope = context.client.check_plugin(plugin_id.as_str()).await?;
            emit_plugin_envelope("plugins.check", &envelope, json)
        }
        PluginsCommand::Install {
            plugin_id,
            skill_id,
            skill_version,
            artifact_path,
            tool_id,
            module_path,
            entrypoint,
            capability_http_hosts,
            capability_secrets,
            capability_storage_prefixes,
            capability_channels,
            display_name,
            notes,
            owner_principal,
            tags,
            disabled,
            allow_tofu,
            allow_untrusted,
            json,
        } => {
            let envelope = context
                .client
                .upsert_plugin(&control_plane::PluginBindingUpsertRequest {
                    plugin_id,
                    skill_id,
                    skill_version,
                    artifact_path,
                    tool_id,
                    module_path,
                    entrypoint,
                    enabled: Some(!disabled),
                    allow_tofu: allow_tofu.then_some(true),
                    allow_untrusted: allow_untrusted.then_some(true),
                    capability_profile: Some(control_plane::PluginCapabilityProfile {
                        http_hosts: capability_http_hosts,
                        secrets: capability_secrets,
                        storage_prefixes: capability_storage_prefixes,
                        channels: capability_channels,
                    }),
                    operator: Some(control_plane::PluginOperatorMetadata {
                        display_name,
                        notes,
                        owner_principal,
                        updated_by: None,
                        tags,
                    }),
                })
                .await?;
            emit_plugin_envelope("plugins.install", &envelope, json)
        }
        PluginsCommand::Update {
            plugin_id,
            skill_id,
            skill_version,
            artifact_path,
            tool_id,
            module_path,
            entrypoint,
            capability_http_hosts,
            capability_secrets,
            capability_storage_prefixes,
            capability_channels,
            display_name,
            notes,
            owner_principal,
            tags,
            disabled,
            allow_tofu,
            allow_untrusted,
            json,
        } => {
            let envelope = context
                .client
                .upsert_plugin(&control_plane::PluginBindingUpsertRequest {
                    plugin_id,
                    skill_id,
                    skill_version,
                    artifact_path,
                    tool_id,
                    module_path,
                    entrypoint,
                    enabled: Some(!disabled),
                    allow_tofu: allow_tofu.then_some(true),
                    allow_untrusted: allow_untrusted.then_some(true),
                    capability_profile: Some(control_plane::PluginCapabilityProfile {
                        http_hosts: capability_http_hosts,
                        secrets: capability_secrets,
                        storage_prefixes: capability_storage_prefixes,
                        channels: capability_channels,
                    }),
                    operator: Some(control_plane::PluginOperatorMetadata {
                        display_name,
                        notes,
                        owner_principal,
                        updated_by: None,
                        tags,
                    }),
                })
                .await?;
            emit_plugin_envelope("plugins.update", &envelope, json)
        }
        PluginsCommand::Enable { plugin_id, json } => {
            let envelope = context.client.enable_plugin(plugin_id.as_str()).await?;
            emit_plugin_envelope("plugins.enable", &envelope, json)
        }
        PluginsCommand::Disable { plugin_id, json } => {
            let envelope = context.client.disable_plugin(plugin_id.as_str()).await?;
            emit_plugin_envelope("plugins.disable", &envelope, json)
        }
        PluginsCommand::Remove { plugin_id, json } => {
            let envelope = context.client.delete_plugin(plugin_id.as_str()).await?;
            if json {
                output::print_json_pretty(
                    &envelope,
                    "failed to encode plugin delete output as JSON",
                )?;
            } else {
                println!(
                    "plugins.remove plugin_id={} deleted={} skill_id={} enabled={}",
                    envelope.binding.plugin_id,
                    envelope.deleted,
                    envelope.binding.skill_id,
                    envelope.binding.enabled
                );
                std::io::stdout().flush().context("stdout flush failed")?;
            }
            Ok(())
        }
    }
}

fn emit_plugin_list(
    mut envelope: control_plane::PluginBindingListEnvelope,
    enabled_only: bool,
    ready_only: bool,
    json: bool,
) -> Result<()> {
    if enabled_only {
        envelope.entries.retain(|entry| entry.binding.enabled);
    }
    if ready_only {
        envelope.entries.retain(|entry| json_bool(entry.check.as_object(), "ready"));
    }

    if json {
        return output::print_json_pretty(&envelope, "failed to encode plugin list as JSON");
    }

    println!("plugins.list root={} count={}", envelope.plugins_root, envelope.entries.len());
    for entry in &envelope.entries {
        println!(
            "plugins.entry plugin_id={} enabled={} skill_id={} skill_version={} ready={} tool_id={} module_path={}",
            entry.binding.plugin_id,
            entry.binding.enabled,
            entry.binding.skill_id,
            entry.binding.skill_version.as_deref().unwrap_or("current"),
            json_bool(entry.check.as_object(), "ready"),
            entry.binding.tool_id.as_deref().unwrap_or("default"),
            entry.binding.module_path.as_deref().unwrap_or("auto"),
        );
        if let Some(reasons) = json_string_array(entry.check.as_object(), "reasons") {
            if !reasons.is_empty() {
                println!(
                    "plugins.entry.reasons plugin_id={} {}",
                    entry.binding.plugin_id,
                    reasons.join(" | ")
                );
            }
        }
    }
    std::io::stdout().flush().context("stdout flush failed")
}

fn emit_plugin_envelope(
    event: &str,
    envelope: &control_plane::PluginBindingEnvelope,
    json: bool,
) -> Result<()> {
    if json {
        return output::print_json_pretty(envelope, "failed to encode plugin output as JSON");
    }

    println!(
        "{event} plugin_id={} enabled={} skill_id={} skill_version={} ready={} tool_id={} module_path={} entrypoint={}",
        envelope.binding.plugin_id,
        envelope.binding.enabled,
        envelope.binding.skill_id,
        envelope.binding.skill_version.as_deref().unwrap_or("current"),
        json_bool(envelope.check.as_object(), "ready"),
        envelope.binding.tool_id.as_deref().unwrap_or("default"),
        envelope.binding.module_path.as_deref().unwrap_or("auto"),
        envelope.binding.entrypoint.as_deref().unwrap_or("run"),
    );
    if let Some(reasons) = json_string_array(envelope.check.as_object(), "reasons") {
        if !reasons.is_empty() {
            println!("{event}.reasons {}", reasons.join(" | "));
        }
    }
    std::io::stdout().flush().context("stdout flush failed")
}

fn json_bool(object: Option<&serde_json::Map<String, Value>>, key: &str) -> bool {
    object.and_then(|value| value.get(key)).and_then(Value::as_bool).unwrap_or(false)
}

fn json_string_array(
    object: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<Vec<String>> {
    object
        .and_then(|value| value.get(key))
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>())
}
