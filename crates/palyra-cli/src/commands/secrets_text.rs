use palyra_control_plane as control_plane;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConfiguredSecretInventoryTextSummary {
    pub(super) snapshot_generation: u64,
    pub(super) entries: Vec<ConfiguredSecretInventoryTextEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConfiguredSecretInventoryTextEntry {
    pub(super) status: &'static str,
    pub(super) reload_action: &'static str,
    pub(super) source_kind: &'static str,
    pub(super) resolution_scope: &'static str,
    pub(super) required: bool,
    pub(super) affected_component_count: usize,
    pub(super) last_error_kind: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConfiguredSecretExplainTextSummary {
    pub(super) status: &'static str,
    pub(super) reload_action: &'static str,
    pub(super) resolution_scope: &'static str,
    pub(super) required: bool,
    pub(super) affected_component_count: usize,
    pub(super) source_kind: &'static str,
    pub(super) refresh_policy: &'static str,
    pub(super) snapshot_policy: &'static str,
    pub(super) last_error_kind: &'static str,
    pub(super) last_error_present: bool,
}

pub(super) fn summarize_configured_secret_inventory(
    envelope: &control_plane::ConfiguredSecretListEnvelope,
) -> ConfiguredSecretInventoryTextSummary {
    ConfiguredSecretInventoryTextSummary {
        snapshot_generation: envelope.snapshot_generation,
        entries: envelope
            .secrets
            .iter()
            .map(|secret| ConfiguredSecretInventoryTextEntry {
                status: allowlisted_status_label(secret.status.as_str()),
                reload_action: allowlisted_reload_action_label(secret.reload_action.as_str()),
                source_kind: allowlisted_source_kind_label(secret.source.kind.as_str()),
                resolution_scope: allowlisted_resolution_scope_label(
                    secret.resolution_scope.as_str(),
                ),
                required: secret.source.required,
                affected_component_count: secret.affected_components.len(),
                last_error_kind: secret
                    .last_error_kind
                    .as_deref()
                    .map(allowlisted_error_kind_label)
                    .unwrap_or("none"),
            })
            .collect(),
    }
}

pub(super) fn render_configured_secret_inventory_lines(
    summary: &ConfiguredSecretInventoryTextSummary,
) -> Vec<String> {
    let mut lines = vec![format!(
        "configured_secrets snapshot_generation={} count={}",
        summary.snapshot_generation,
        summary.entries.len()
    )];
    for (index, secret) in summary.entries.iter().enumerate() {
        lines.push(format!(
            "secret index={} status={} reload_action={} source={} scope={} required={} affected_components={} error_kind={}",
            index + 1,
            secret.status,
            secret.reload_action,
            secret.source_kind,
            secret.resolution_scope,
            secret.required,
            secret.affected_component_count,
            secret.last_error_kind
        ));
    }
    lines
}

pub(super) fn summarize_configured_secret_explain(
    secret: &control_plane::ConfiguredSecretRecord,
) -> ConfiguredSecretExplainTextSummary {
    ConfiguredSecretExplainTextSummary {
        status: allowlisted_status_label(secret.status.as_str()),
        reload_action: allowlisted_reload_action_label(secret.reload_action.as_str()),
        resolution_scope: allowlisted_resolution_scope_label(secret.resolution_scope.as_str()),
        required: secret.source.required,
        affected_component_count: secret.affected_components.len(),
        source_kind: allowlisted_source_kind_label(secret.source.kind.as_str()),
        refresh_policy: allowlisted_refresh_policy_label(secret.source.refresh_policy.as_str()),
        snapshot_policy: allowlisted_snapshot_policy_label(secret.source.snapshot_policy.as_str()),
        last_error_kind: secret
            .last_error_kind
            .as_deref()
            .map(allowlisted_error_kind_label)
            .unwrap_or("none"),
        last_error_present: secret.last_error.is_some(),
    }
}

pub(super) fn render_configured_secret_explain_lines(
    summary: &ConfiguredSecretExplainTextSummary,
) -> Vec<String> {
    let mut lines = vec![format!(
        "secret.explain status={} reload_action={} scope={} required={} affected_components={}",
        summary.status,
        summary.reload_action,
        summary.resolution_scope,
        summary.required,
        summary.affected_component_count
    )];
    lines.push(format!(
        "source kind={} refresh_policy={} snapshot_policy={} error_kind={}",
        summary.source_kind,
        summary.refresh_policy,
        summary.snapshot_policy,
        summary.last_error_kind
    ));
    if summary.last_error_present {
        lines.push("last_error_present=true use --json for structured diagnostics".to_owned());
    }
    lines
}

// Text mode only emits allowlisted labels so arbitrary secret-derived strings
// never reach stdout in human-readable output.
fn allowlisted_status_label(candidate: &str) -> &'static str {
    match candidate {
        "healthy" => "healthy",
        "missing" => "missing",
        "blocked" => "blocked",
        "failed" => "failed",
        _ => "other",
    }
}

fn allowlisted_reload_action_label(candidate: &str) -> &'static str {
    match candidate {
        "blocked_while_runs_active" => "blocked_while_runs_active",
        "restart_required" => "restart_required",
        "daemon_restart_required" => "daemon_restart_required",
        "browserd_restart_required" => "browserd_restart_required",
        "live_refresh_supported" => "live_refresh_supported",
        "live_reference" => "live_reference",
        "manual_review" => "manual_review",
        _ => "other",
    }
}

fn allowlisted_source_kind_label(candidate: &str) -> &'static str {
    match candidate {
        "vault" => "vault",
        "env" => "env",
        "file" => "file",
        "exec" => "exec",
        _ => "other",
    }
}

fn allowlisted_resolution_scope_label(candidate: &str) -> &'static str {
    match candidate {
        "startup" => "startup",
        "reload" => "reload",
        "runtime" => "runtime",
        _ => "other",
    }
}

fn allowlisted_refresh_policy_label(candidate: &str) -> &'static str {
    match candidate {
        "on_startup" => "on_startup",
        "startup_only" => "startup_only",
        "on_reload" => "on_reload",
        "per_run" => "per_run",
        "per_use" => "per_use",
        _ => "other",
    }
}

fn allowlisted_snapshot_policy_label(candidate: &str) -> &'static str {
    match candidate {
        "freeze_until_reload" => "freeze_until_reload",
        "runtime_snapshot" => "runtime_snapshot",
        "refresh_per_run" => "refresh_per_run",
        "refresh_per_use" => "refresh_per_use",
        _ => "other",
    }
}

fn allowlisted_error_kind_label(candidate: &str) -> &'static str {
    match candidate {
        "none" => "none",
        "missing" => "missing",
        "invalid_reference" => "invalid_reference",
        "policy_blocked" => "policy_blocked",
        "io" => "io",
        "too_large" => "too_large",
        "timeout" => "timeout",
        "exec_failed" => "exec_failed",
        "decode_failed" => "decode_failed",
        "auth_invalid" => "auth_invalid",
        _ => "other",
    }
}
