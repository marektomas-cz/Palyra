use super::types::ToolCallRejectionKind;
use super::{
    build_model_visible_tool_catalog_snapshot, provider_tools_from_catalog_snapshot,
    snapshot_to_provider_request_value, validate_tool_call_against_catalog_snapshot,
    ToolCatalogBuildRequest, ToolExposureSurface, ToolSchemaDialect,
};
use crate::{
    sandbox_runner::{EgressEnforcementMode, SandboxProcessRunnerPolicy, SandboxProcessRunnerTier},
    tool_protocol::{ToolCallConfig, ToolRequestContext},
    wasm_plugin_runner::WasmPluginRunnerPolicy,
};

fn config(allowed_tools: &[&str]) -> ToolCallConfig {
    ToolCallConfig {
        allowed_tools: allowed_tools.iter().map(|tool| (*tool).to_owned()).collect(),
        max_calls_per_run: 4,
        execution_timeout_ms: 1_000,
        process_runner: SandboxProcessRunnerPolicy {
            enabled: false,
            tier: SandboxProcessRunnerTier::B,
            workspace_root: ".".into(),
            allowed_executables: Vec::new(),
            allow_interpreters: false,
            egress_enforcement_mode: EgressEnforcementMode::Strict,
            allowed_egress_hosts: Vec::new(),
            allowed_dns_suffixes: Vec::new(),
            cpu_time_limit_ms: 1_000,
            memory_limit_bytes: 128 * 1024 * 1024,
            max_output_bytes: 64 * 1024,
        },
        wasm_runtime: WasmPluginRunnerPolicy {
            enabled: false,
            allow_inline_modules: false,
            max_module_size_bytes: 256 * 1024,
            fuel_budget: 10_000_000,
            max_memory_bytes: 64 * 1024 * 1024,
            max_table_elements: 100_000,
            max_instances: 256,
            allowed_http_hosts: Vec::new(),
            allowed_secrets: Vec::new(),
            allowed_storage_prefixes: Vec::new(),
            allowed_channels: Vec::new(),
        },
    }
}

fn request_context() -> ToolRequestContext {
    ToolRequestContext {
        principal: "user:test".to_owned(),
        device_id: Some("device:test".to_owned()),
        channel: Some("console".to_owned()),
        session_id: Some("session".to_owned()),
        run_id: Some("run".to_owned()),
        skill_id: None,
    }
}

#[test]
fn catalog_snapshot_exposes_allowlisted_tools_with_schema_hashes() {
    let config = config(&["palyra.echo", "palyra.sleep"]);
    let snapshot = build_model_visible_tool_catalog_snapshot(ToolCatalogBuildRequest {
        config: &config,
        browser_service_enabled: false,
        request_context: &request_context(),
        provider_kind: "openai_compatible",
        provider_model_id: Some("gpt-test"),
        surface: ToolExposureSurface::RunStream,
        remaining_tool_budget: 2,
        created_at_unix_ms: 42,
    });

    assert_eq!(snapshot.tools.len(), 2);
    assert!(snapshot.tools.iter().all(|tool| !tool.internal_schema_hash.is_empty()));
    assert!(snapshot.filtered_tools.iter().any(|tool| tool.name == "palyra.process.run"));
    assert!(snapshot.snapshot_id.starts_with("toolcat_"));
}

#[test]
fn provider_payload_projects_native_openai_tools() {
    let config = config(&["palyra.echo"]);
    let snapshot = build_model_visible_tool_catalog_snapshot(ToolCatalogBuildRequest {
        config: &config,
        browser_service_enabled: false,
        request_context: &request_context(),
        provider_kind: "openai_compatible",
        provider_model_id: None,
        surface: ToolExposureSurface::RunStream,
        remaining_tool_budget: 1,
        created_at_unix_ms: 42,
    });
    let payload = snapshot_to_provider_request_value(&snapshot);
    let tools = provider_tools_from_catalog_snapshot(&payload, ToolSchemaDialect::OpenAiCompatible);

    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["function"]["name"], "palyra.echo");
    assert_eq!(tools[0]["function"]["parameters"]["type"], "object");
}

#[test]
fn intake_normalizes_safe_scalar_arguments() {
    let config = config(&["palyra.sleep"]);
    let snapshot = build_model_visible_tool_catalog_snapshot(ToolCatalogBuildRequest {
        config: &config,
        browser_service_enabled: false,
        request_context: &request_context(),
        provider_kind: "openai_compatible",
        provider_model_id: None,
        surface: ToolExposureSurface::RunStream,
        remaining_tool_budget: 1,
        created_at_unix_ms: 42,
    });

    let normalized = validate_tool_call_against_catalog_snapshot(
        &snapshot,
        "palyra.sleep",
        br#"{"duration_ms":"25"}"#,
    )
    .expect("duration string should safely normalize to integer");
    let normalized_json: serde_json::Value =
        serde_json::from_slice(normalized.input_json.as_slice()).expect("valid json");
    assert_eq!(normalized_json["duration_ms"], 25);
    assert_eq!(normalized.audit.steps.len(), 1);
}

#[test]
fn intake_rejects_runtime_unavailable_tool() {
    let config = config(&["palyra.process.run"]);
    let snapshot = build_model_visible_tool_catalog_snapshot(ToolCatalogBuildRequest {
        config: &config,
        browser_service_enabled: false,
        request_context: &request_context(),
        provider_kind: "openai_compatible",
        provider_model_id: None,
        surface: ToolExposureSurface::RunStream,
        remaining_tool_budget: 1,
        created_at_unix_ms: 42,
    });
    let rejection = validate_tool_call_against_catalog_snapshot(
        &snapshot,
        "palyra.process.run",
        br#"{"command":"echo","args":[]}"#,
    )
    .expect_err("process runner is disabled");

    assert_eq!(rejection.kind, ToolCallRejectionKind::UnavailableTool);
}

#[test]
fn intake_rejects_command_scalar_coercion() {
    let mut config = config(&["palyra.process.run"]);
    config.process_runner.enabled = true;
    let snapshot = build_model_visible_tool_catalog_snapshot(ToolCatalogBuildRequest {
        config: &config,
        browser_service_enabled: false,
        request_context: &request_context(),
        provider_kind: "openai_compatible",
        provider_model_id: None,
        surface: ToolExposureSurface::RunStream,
        remaining_tool_budget: 1,
        created_at_unix_ms: 42,
    });
    let rejection = validate_tool_call_against_catalog_snapshot(
        &snapshot,
        "palyra.process.run",
        br#"{"command":123,"args":[]}"#,
    )
    .expect_err("command must not be coerced");

    assert_eq!(rejection.kind, ToolCallRejectionKind::MalformedArguments);
}
