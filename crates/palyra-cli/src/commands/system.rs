use crate::*;
use palyra_control_plane as control_plane;

#[derive(Debug, Serialize)]
struct SystemPresenceEntry {
    subsystem: String,
    state: String,
    detail: String,
}

pub(crate) fn run_system(command: SystemCommand) -> Result<()> {
    match command {
        SystemCommand::Event { limit, json } => {
            run_system_events(limit, output::preferred_json(json))
        }
        other => {
            let runtime = build_runtime()?;
            runtime.block_on(run_system_async(other))
        }
    }
}

async fn run_system_async(command: SystemCommand) -> Result<()> {
    let context =
        client::control_plane::connect_admin_console(app::ConnectionOverrides::default()).await?;
    let diagnostics = context.client.get_diagnostics().await?;
    let deployment = context.client.get_deployment_posture().await?;

    match command {
        SystemCommand::Heartbeat { json } => {
            emit_system_heartbeat(&diagnostics, &deployment, output::preferred_json(json))
        }
        SystemCommand::Presence { json } => {
            emit_system_presence(&diagnostics, &deployment, output::preferred_json(json))
        }
        SystemCommand::Event { .. } => unreachable!("system event is handled synchronously"),
    }
}

fn run_system_events(limit: Option<usize>, json_output: bool) -> Result<()> {
    let root_context = app::current_root_context()
        .ok_or_else(|| anyhow!("CLI root context is unavailable for system command"))?;
    let connection = root_context.resolve_http_connection(
        app::ConnectionOverrides::default(),
        app::ConnectionDefaults::ADMIN,
    )?;
    let endpoint = format!("{}/admin/v1/journal/recent", connection.base_url.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .context("failed to build HTTP client")?;
    let mut request = client
        .get(endpoint)
        .header("x-palyra-principal", connection.principal.clone())
        .header("x-palyra-device-id", connection.device_id.clone())
        .header("x-palyra-channel", connection.channel.clone())
        .header("x-palyra-trace-id", connection.trace_id.clone());
    if let Some(token) = connection.token.as_ref() {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    if let Some(limit) = limit {
        request = request.query(&[("limit", limit)]);
    }

    let response: JournalRecentResponse = request
        .send()
        .context("failed to call daemon journal recent endpoint")?
        .error_for_status()
        .context("daemon journal recent endpoint returned non-success status")?
        .json()
        .context("failed to parse daemon journal recent payload")?;

    if json_output {
        return output::print_json_pretty(
            &json!({
                "total_events": response.total_events,
                "hash_chain_enabled": response.hash_chain_enabled,
                "events": response.events,
            }),
            "failed to encode system event payload as JSON",
        );
    }

    println!(
        "system.event total_events={} hash_chain_enabled={} returned_events={}",
        response.total_events,
        response.hash_chain_enabled,
        response.events.len()
    );
    for event in response.events {
        println!(
            "system.event.entry event_id={} kind={} actor={} redacted={} timestamp_unix_ms={} hash_present={}",
            event.event_id,
            event.kind,
            event.actor,
            event.redacted,
            event.timestamp_unix_ms,
            event.hash.is_some()
        );
    }
    std::io::stdout().flush().context("stdout flush failed")
}

fn emit_system_heartbeat(
    diagnostics: &Value,
    deployment: &control_plane::DeploymentPostureSummary,
    json_output: bool,
) -> Result<()> {
    let generated_at_unix_ms =
        diagnostics.get("generated_at_unix_ms").and_then(Value::as_i64).unwrap_or_default();
    let auth_state =
        diagnostics.pointer("/auth_profiles/state").and_then(Value::as_str).unwrap_or("unknown");
    let browser_state = diagnostics
        .pointer("/browserd/health/status")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            diagnostics.pointer("/browserd/enabled").and_then(Value::as_bool).map(|enabled| {
                if enabled {
                    "configured".to_owned()
                } else {
                    "disabled".to_owned()
                }
            })
        })
        .unwrap_or_else(|| "unknown".to_owned());
    let browser_sessions =
        diagnostics.pointer("/browserd/sessions/active").and_then(Value::as_u64).unwrap_or(0);
    let degraded_connectors = diagnostics
        .pointer("/observability/connector/degraded_connectors")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let queue_depth = diagnostics
        .pointer("/observability/connector/queue_depth")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let recent_failures = diagnostics
        .pointer("/observability/recent_failures")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let support_bundle_failures = diagnostics
        .pointer("/observability/support_bundle/failures")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let memory_entries =
        diagnostics.pointer("/memory/usage/entries").and_then(Value::as_u64).unwrap_or(0);
    let overall_status = system_heartbeat_status(
        auth_state,
        browser_state.as_str(),
        degraded_connectors,
        recent_failures,
        deployment.warnings.len(),
    );

    let payload = json!({
        "status": overall_status,
        "generated_at_unix_ms": generated_at_unix_ms,
        "deployment": {
            "mode": deployment.mode,
            "bind_profile": deployment.bind_profile,
            "remote_bind_detected": deployment.remote_bind_detected,
            "warnings": deployment.warnings,
        },
        "subsystems": {
            "auth_state": auth_state,
            "browser_state": browser_state,
            "browser_sessions": browser_sessions,
            "connector_degraded": degraded_connectors,
            "connector_queue_depth": queue_depth,
            "memory_entries": memory_entries,
            "recent_failures": recent_failures,
            "support_bundle_failures": support_bundle_failures,
        },
    });

    if json_output {
        return output::print_json_pretty(&payload, "failed to encode system heartbeat as JSON");
    }

    println!(
        "system.heartbeat status={} generated_at_unix_ms={} deployment_mode={} bind_profile={} remote_bind_detected={} warnings={} recent_failures={} support_bundle_failures={}",
        overall_status,
        generated_at_unix_ms,
        deployment.mode,
        deployment.bind_profile,
        deployment.remote_bind_detected,
        deployment.warnings.len(),
        recent_failures,
        support_bundle_failures
    );
    println!(
        "system.heartbeat.subsystems auth_state={} browser_state={} browser_sessions={} connector_degraded={} connector_queue_depth={} memory_entries={}",
        auth_state,
        browser_state,
        browser_sessions,
        degraded_connectors,
        queue_depth,
        memory_entries
    );
    for warning in &deployment.warnings {
        println!("system.heartbeat.warning={warning}");
    }
    std::io::stdout().flush().context("stdout flush failed")
}

fn emit_system_presence(
    diagnostics: &Value,
    deployment: &control_plane::DeploymentPostureSummary,
    json_output: bool,
) -> Result<()> {
    let generated_at_unix_ms =
        diagnostics.get("generated_at_unix_ms").and_then(Value::as_i64).unwrap_or_default();
    let auth_state =
        diagnostics.pointer("/auth_profiles/state").and_then(Value::as_str).unwrap_or("unknown");
    let browser_enabled =
        diagnostics.pointer("/browserd/enabled").and_then(Value::as_bool).unwrap_or(false);
    let browser_state = diagnostics
        .pointer("/browserd/health/status")
        .and_then(Value::as_str)
        .unwrap_or(if browser_enabled { "configured" } else { "disabled" });
    let browser_sessions =
        diagnostics.pointer("/browserd/sessions/active").and_then(Value::as_u64).unwrap_or(0);
    let webhooks_total =
        diagnostics.pointer("/webhooks/total").and_then(Value::as_u64).unwrap_or(0);
    let webhooks_ready =
        diagnostics.pointer("/webhooks/ready").and_then(Value::as_u64).unwrap_or(0);
    let degraded_connectors = diagnostics
        .pointer("/observability/connector/degraded_connectors")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let queue_depth = diagnostics
        .pointer("/observability/connector/queue_depth")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let memory_entries =
        diagnostics.pointer("/memory/usage/entries").and_then(Value::as_u64).unwrap_or(0);
    let memory_bytes =
        diagnostics.pointer("/memory/usage/approx_bytes").and_then(Value::as_u64).unwrap_or(0);
    let support_bundle_failures = diagnostics
        .pointer("/observability/support_bundle/failures")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let subsystems = vec![
        SystemPresenceEntry {
            subsystem: "deployment".to_owned(),
            state: if deployment.warnings.is_empty() {
                "ok".to_owned()
            } else {
                "degraded".to_owned()
            },
            detail: format!(
                "mode={} bind_profile={} remote_bind_detected={} warnings={}",
                deployment.mode,
                deployment.bind_profile,
                deployment.remote_bind_detected,
                deployment.warnings.len()
            ),
        },
        SystemPresenceEntry {
            subsystem: "auth_profiles".to_owned(),
            state: auth_state.to_owned(),
            detail: format!(
                "provider_auth_state={}",
                diagnostics
                    .pointer("/observability/provider_auth/state")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ),
        },
        SystemPresenceEntry {
            subsystem: "browserd".to_owned(),
            state: browser_state.to_owned(),
            detail: format!("enabled={} active_sessions={}", browser_enabled, browser_sessions),
        },
        SystemPresenceEntry {
            subsystem: "connectors".to_owned(),
            state: if degraded_connectors > 0 || queue_depth > 0 {
                "degraded".to_owned()
            } else {
                "ok".to_owned()
            },
            detail: format!(
                "degraded_connectors={} queue_depth={}",
                degraded_connectors, queue_depth
            ),
        },
        SystemPresenceEntry {
            subsystem: "webhooks".to_owned(),
            state: if webhooks_total == 0 {
                "idle".to_owned()
            } else if webhooks_ready == webhooks_total {
                "ok".to_owned()
            } else {
                "degraded".to_owned()
            },
            detail: format!("ready={} total={}", webhooks_ready, webhooks_total),
        },
        SystemPresenceEntry {
            subsystem: "memory".to_owned(),
            state: "ok".to_owned(),
            detail: format!("entries={} approx_bytes={}", memory_entries, memory_bytes),
        },
        SystemPresenceEntry {
            subsystem: "support_bundle".to_owned(),
            state: if support_bundle_failures > 0 {
                "degraded".to_owned()
            } else {
                "ok".to_owned()
            },
            detail: format!("failures={support_bundle_failures}"),
        },
    ];

    if json_output {
        return output::print_json_pretty(
            &json!({
                "generated_at_unix_ms": generated_at_unix_ms,
                "subsystems": subsystems,
            }),
            "failed to encode system presence as JSON",
        );
    }

    println!(
        "system.presence generated_at_unix_ms={} subsystems={} degraded={}",
        generated_at_unix_ms,
        subsystems.len(),
        subsystems.iter().filter(|entry| entry.state == "degraded").count()
    );
    for entry in subsystems {
        println!(
            "system.presence.entry subsystem={} state={} detail={}",
            entry.subsystem, entry.state, entry.detail
        );
    }
    std::io::stdout().flush().context("stdout flush failed")
}

fn system_heartbeat_status(
    auth_state: &str,
    browser_state: &str,
    degraded_connectors: u64,
    recent_failures: usize,
    deployment_warnings: usize,
) -> &'static str {
    let auth_ok = matches!(auth_state, "ok" | "static");
    let browser_ok = matches!(browser_state, "ok" | "disabled" | "configured");
    if auth_ok
        && browser_ok
        && degraded_connectors == 0
        && recent_failures == 0
        && deployment_warnings == 0
    {
        "ok"
    } else {
        "degraded"
    }
}

#[cfg(test)]
mod tests {
    use super::system_heartbeat_status;

    #[test]
    fn heartbeat_status_is_ok_when_all_subsystems_are_stable() {
        assert_eq!(system_heartbeat_status("ok", "disabled", 0, 0, 0), "ok");
        assert_eq!(system_heartbeat_status("static", "ok", 0, 0, 0), "ok");
    }

    #[test]
    fn heartbeat_status_degrades_on_warnings_or_failures() {
        assert_eq!(system_heartbeat_status("missing", "ok", 0, 0, 0), "degraded");
        assert_eq!(system_heartbeat_status("ok", "degraded", 0, 0, 0), "degraded");
        assert_eq!(system_heartbeat_status("ok", "ok", 1, 0, 0), "degraded");
        assert_eq!(system_heartbeat_status("ok", "ok", 0, 1, 0), "degraded");
        assert_eq!(system_heartbeat_status("ok", "ok", 0, 0, 1), "degraded");
    }
}
