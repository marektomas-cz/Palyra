use super::*;
use palyra_control_plane::{ContractDescriptor, PageInfo, CONTROL_PLANE_CONTRACT_VERSION};

fn sample_configured_secret_record() -> control_plane::ConfiguredSecretRecord {
    control_plane::ConfiguredSecretRecord {
        secret_id: "model_provider.openai_api_key_secret_ref:super-secret-fingerprint".to_owned(),
        component: "model_provider".to_owned(),
        config_path: "model_provider.openai_api_key_secret_ref".to_owned(),
        status: "healthy".to_owned(),
        resolution_scope: "startup".to_owned(),
        reload_action: "daemon_restart_required".to_owned(),
        snapshot_generation: 7,
        source: control_plane::ConfiguredSecretSourceView {
            kind: "vault".to_owned(),
            fingerprint: "super-secret-fingerprint".to_owned(),
            required: true,
            refresh_policy: "startup_only".to_owned(),
            snapshot_policy: "runtime_snapshot".to_owned(),
            description: "OpenAI credential".to_owned(),
            display_name: Some("OpenAI API key".to_owned()),
            redaction_label: Some("api_key".to_owned()),
            max_bytes: Some(512),
            exec_timeout_ms: None,
            trusted_dir_count: None,
            inherited_env_count: None,
            allow_symlinks: None,
        },
        affected_components: vec!["model_provider".to_owned()],
        last_resolved_at_unix_ms: Some(1_700_000_000_000),
        last_error_kind: Some("auth_invalid".to_owned()),
        last_error: Some("Bearer super-secret-token".to_owned()),
        value_bytes: Some(42),
    }
}

#[test]
fn configured_secret_inventory_text_lines_hide_sensitive_identifiers() {
    let envelope = control_plane::ConfiguredSecretListEnvelope {
        contract: ContractDescriptor {
            contract_version: CONTROL_PLANE_CONTRACT_VERSION.to_owned(),
        },
        generated_at_unix_ms: 1_700_000_000_000,
        snapshot_generation: 7,
        secrets: vec![sample_configured_secret_record()],
        page: PageInfo { limit: 1, returned: 1, next_cursor: None, has_more: false },
    };
    let rendered = super::secrets_text::render_configured_secret_inventory_lines(
        &super::secrets_text::summarize_configured_secret_inventory(&envelope),
    )
    .join("\n");
    assert!(rendered.contains("status=healthy"));
    assert!(rendered.contains("source=vault"));
    assert!(!rendered.contains("super-secret-fingerprint"));
    assert!(!rendered.contains("model_provider.openai_api_key_secret_ref"));
}

#[test]
fn configured_secret_explain_text_lines_hide_sensitive_detail() {
    let rendered = super::secrets_text::render_configured_secret_explain_lines(
        &super::secrets_text::summarize_configured_secret_explain(
            &sample_configured_secret_record(),
        ),
    )
    .join("\n");
    assert!(rendered.contains("status=healthy"));
    assert!(rendered.contains("error_kind=auth_invalid"));
    assert!(rendered.contains("last_error_present=true"));
    assert!(!rendered.contains("super-secret-fingerprint"));
    assert!(!rendered.contains("Bearer super-secret-token"));
    assert!(!rendered.contains("model_provider.openai_api_key_secret_ref"));
}
