use super::builtin::registry_entry;
use super::types::ToolCallRejectionKind;
use super::{
    build_model_visible_tool_catalog_snapshot, projection_policy_for_tool,
    provider_tools_from_catalog_snapshot, snapshot_to_provider_request_value,
    validate_tool_call_against_catalog_snapshot, ToolCatalogBuildRequest, ToolExposureSurface,
    ToolResultProjectionPolicy, ToolSchemaDialect,
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
fn anthropic_catalog_exposes_http_fetch_with_boolean_additional_properties() {
    let config = config(&["palyra.http.fetch"]);
    let snapshot = build_model_visible_tool_catalog_snapshot(ToolCatalogBuildRequest {
        config: &config,
        browser_service_enabled: false,
        request_context: &request_context(),
        provider_kind: "anthropic",
        provider_model_id: Some("minimax-m2.7"),
        surface: ToolExposureSurface::RunStream,
        remaining_tool_budget: 1,
        created_at_unix_ms: 42,
    });

    let http_fetch = snapshot
        .tools
        .iter()
        .find(|tool| tool.name == "palyra.http.fetch")
        .expect("http fetch should stay visible for Anthropic-compatible providers");
    assert!(
        !snapshot.filtered_tools.iter().any(|tool| {
            tool.name == "palyra.http.fetch"
                && tool.reason_code.as_str() == "provider_schema_incompatible"
        }),
        "http fetch must not be filtered for schema dialect incompatibility"
    );
    assert_eq!(
        http_fetch.provider_schema["properties"]["headers"]["additionalProperties"],
        serde_json::Value::Bool(true)
    );

    let payload = snapshot_to_provider_request_value(&snapshot);
    let tools = provider_tools_from_catalog_snapshot(&payload, ToolSchemaDialect::Anthropic);
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "palyra.http.fetch");
    assert_eq!(
        tools[0]["input_schema"]["properties"]["headers"]["additionalProperties"],
        serde_json::Value::Bool(true)
    );
}

#[test]
fn browser_session_create_returns_model_visible_handle() {
    assert_eq!(
        projection_policy_for_tool("palyra.browser.session.create"),
        ToolResultProjectionPolicy::InlineUnlessLarge
    );
    assert_eq!(
        projection_policy_for_tool("palyra.fs.read_file"),
        ToolResultProjectionPolicy::InlineUnlessLarge
    );
    assert_eq!(
        projection_policy_for_tool("palyra.fs.list_dir"),
        ToolResultProjectionPolicy::InlineUnlessLarge
    );
    assert_eq!(
        projection_policy_for_tool("palyra.browser.observe"),
        ToolResultProjectionPolicy::RedactedPreviewAndArtifact
    );
}

#[test]
fn workspace_file_schemas_accept_workspace_root_override() {
    let read_file = registry_entry("palyra.fs.read_file").expect("read file entry exists");
    let list_dir = registry_entry("palyra.fs.list_dir").expect("list dir entry exists");

    assert_eq!(read_file.input_schema["properties"]["workspace_root"]["type"], "string");
    assert_eq!(list_dir.input_schema["properties"]["workspace_root"]["type"], "string");
    assert!(read_file.input_schema["properties"]["workspace_root"]["description"]
        .as_str()
        .unwrap_or_default()
        .contains("prior apply_patch"));
    assert!(list_dir.input_schema["properties"]["workspace_root"]["description"]
        .as_str()
        .unwrap_or_default()
        .contains("nested project"));
}

#[test]
fn browser_observe_schema_exposes_visible_text_default() {
    let entry = registry_entry("palyra.browser.observe").expect("browser observe tool entry");

    assert_eq!(entry.input_schema["properties"]["include_visible_text"]["type"], "boolean");
    assert_eq!(entry.input_schema["properties"]["include_visible_text"]["default"], true);
    assert_eq!(entry.input_schema["properties"]["max_visible_text_bytes"]["minimum"], 0);
}

#[test]
fn routines_control_schema_discourages_slug_ids_and_short_intervals() {
    let entry = registry_entry("palyra.routines.control").expect("routines control tool entry");

    assert!(
        entry.description.contains("omit routine_id"),
        "description should tell models not to invent human routine ids"
    );
    assert!(entry.input_schema["properties"]["routine_id"]["description"]
        .as_str()
        .unwrap_or_default()
        .contains("do not put human slugs here"));
    assert_eq!(entry.input_schema["properties"]["every_interval_ms"]["minimum"], 30_000);
    assert!(entry.input_schema["properties"]["every_interval_ms"]["description"]
        .as_str()
        .unwrap_or_default()
        .contains("palyra.sleep"));
}

#[test]
fn memory_session_search_schema_targets_prior_transcripts() {
    let entry = registry_entry("palyra.memory.session_search").expect("session search tool entry");
    let alias = registry_entry("palyra.session_search").expect("session search alias tool entry");

    assert!(entry.description.contains("prior session transcripts"));
    assert!(alias.description.contains("Compatibility alias"));
    assert_eq!(alias.input_schema["required"][0], "query");
    assert_eq!(entry.input_schema["required"][0], "query");
    assert!(entry.input_schema["properties"]["query"]["description"]
        .as_str()
        .unwrap_or_default()
        .contains("previous session"));
    assert_eq!(entry.input_schema["properties"]["top_k"]["maximum"], 24);
    assert_eq!(entry.input_schema["properties"]["window_before"]["maximum"], 8);
    assert_eq!(entry.projection_policy, ToolResultProjectionPolicy::InlineUnlessLarge);
}

#[test]
fn memory_retain_schema_explains_principal_scope_for_corrections() {
    let entry = registry_entry("palyra.memory.retain").expect("retain tool entry");

    assert!(entry.description.contains("scope=principal"));
    assert_eq!(
        entry.input_schema["properties"]["scope"]["enum"],
        serde_json::json!(["session", "channel", "principal"])
    );
    assert!(entry.input_schema["properties"]["scope"]["description"]
        .as_str()
        .unwrap_or_default()
        .contains("future sessions"));
    assert!(entry.input_schema["properties"]["content_text"]["description"]
        .as_str()
        .unwrap_or_default()
        .contains("old value"));
}

#[test]
fn sleep_schema_allows_short_heartbeat_waits() {
    let entry = registry_entry("palyra.sleep").expect("sleep should be registered");
    assert_eq!(entry.input_schema["properties"]["duration_ms"]["maximum"], 30_000);
}

#[test]
fn artifact_read_schema_defaults_to_text_preview() {
    let entry = registry_entry("palyra.artifact.read").expect("artifact read should be registered");

    assert_eq!(entry.input_schema["properties"]["text_preview"]["default"], true);
    assert!(entry.input_schema["properties"]["text_preview"]["description"]
        .as_str()
        .unwrap_or_default()
        .contains("bounded redacted text preview"));
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
