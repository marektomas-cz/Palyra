use palyra_control_plane as control_plane;

pub(super) fn render_configured_secret_inventory_lines(
    envelope: &control_plane::ConfiguredSecretListEnvelope,
) -> Vec<String> {
    let mut lines = vec![format!(
        "configured_secrets snapshot_generation={} count={}",
        envelope.snapshot_generation,
        envelope.secrets.len()
    )];
    for (index, secret) in envelope.secrets.iter().enumerate() {
        lines.push(format!(
            "secret index={} status={} reload_action={} source={} scope={} required={} affected_components={} error_kind={}",
            index + 1,
            secret.status,
            secret.reload_action,
            secret.source.kind,
            secret.resolution_scope,
            secret.source.required,
            secret.affected_components.len(),
            secret.last_error_kind.as_deref().unwrap_or("none")
        ));
    }
    lines
}

pub(super) fn render_configured_secret_explain_lines(
    secret: &control_plane::ConfiguredSecretRecord,
) -> Vec<String> {
    let mut lines = vec![format!(
        "secret.explain status={} reload_action={} scope={} required={} affected_components={} value_bytes={}",
        secret.status,
        secret.reload_action,
        secret.resolution_scope,
        secret.source.required,
        secret.affected_components.len(),
        secret
            .value_bytes
            .map_or_else(|| "unknown".to_owned(), |value| value.to_string())
    )];
    lines.push(format!(
        "source kind={} refresh_policy={} snapshot_policy={} error_kind={}",
        secret.source.kind,
        secret.source.refresh_policy,
        secret.source.snapshot_policy,
        secret.last_error_kind.as_deref().unwrap_or("none")
    ));
    if secret.last_error.is_some() {
        lines.push("last_error_present=true use --json for structured diagnostics".to_owned());
    }
    lines
}
