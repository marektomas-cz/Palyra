use palyra_control_plane as control_plane;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConfiguredSecretInventoryTextSummary {
    pub(super) snapshot_generation: String,
    pub(super) entries: Vec<ConfiguredSecretInventoryTextEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConfiguredSecretInventoryTextEntry {
    pub(super) status: String,
    pub(super) reload_action: String,
    pub(super) source_kind: String,
    pub(super) resolution_scope: String,
    pub(super) required: bool,
    pub(super) affected_component_count: usize,
    pub(super) last_error_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConfiguredSecretExplainTextSummary {
    pub(super) status: String,
    pub(super) reload_action: String,
    pub(super) resolution_scope: String,
    pub(super) required: bool,
    pub(super) affected_component_count: usize,
    pub(super) value_bytes: String,
    pub(super) source_kind: String,
    pub(super) refresh_policy: String,
    pub(super) snapshot_policy: String,
    pub(super) last_error_kind: String,
    pub(super) last_error_present: bool,
}

pub(super) fn summarize_configured_secret_inventory(
    envelope: &control_plane::ConfiguredSecretListEnvelope,
) -> ConfiguredSecretInventoryTextSummary {
    ConfiguredSecretInventoryTextSummary {
        snapshot_generation: envelope.snapshot_generation.to_string(),
        entries: envelope
            .secrets
            .iter()
            .map(|secret| ConfiguredSecretInventoryTextEntry {
                status: secret.status.clone(),
                reload_action: secret.reload_action.clone(),
                source_kind: secret.source.kind.clone(),
                resolution_scope: secret.resolution_scope.clone(),
                required: secret.source.required,
                affected_component_count: secret.affected_components.len(),
                last_error_kind: secret
                    .last_error_kind
                    .clone()
                    .unwrap_or_else(|| "none".to_owned()),
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
        status: secret.status.clone(),
        reload_action: secret.reload_action.clone(),
        resolution_scope: secret.resolution_scope.clone(),
        required: secret.source.required,
        affected_component_count: secret.affected_components.len(),
        value_bytes: secret
            .value_bytes
            .map_or_else(|| "unknown".to_owned(), |value| value.to_string()),
        source_kind: secret.source.kind.clone(),
        refresh_policy: secret.source.refresh_policy.clone(),
        snapshot_policy: secret.source.snapshot_policy.clone(),
        last_error_kind: secret.last_error_kind.clone().unwrap_or_else(|| "none".to_owned()),
        last_error_present: secret.last_error.is_some(),
    }
}

pub(super) fn render_configured_secret_explain_lines(
    summary: &ConfiguredSecretExplainTextSummary,
) -> Vec<String> {
    let mut lines = vec![format!(
        "secret.explain status={} reload_action={} scope={} required={} affected_components={} value_bytes={}",
        summary.status,
        summary.reload_action,
        summary.resolution_scope,
        summary.required,
        summary.affected_component_count,
        summary.value_bytes
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
