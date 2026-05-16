use serde_json::{json, Map, Value};

use crate::tool_protocol::{tool_metadata, tool_requires_approval};

use super::hashing::stable_hash_value;
use super::types::{
    ToolApprovalPosture, ToolExposureSurface, ToolParallelismPolicy, ToolRegistryEntry,
    ToolResultProjectionPolicy, TOOL_REGISTRY_ENTRY_VERSION,
};

#[must_use]
pub(crate) fn registry_entries() -> Vec<ToolRegistryEntry> {
    let mut entries = vec![
        entry(
            "palyra.echo",
            "Echo safe text for connectivity and tool-flow checks.",
            object_schema(
                &["text"],
                vec![("text", json!({"type":"string","maxLength":4096}))],
                false,
            ),
            ToolParallelismPolicy::ReadOnly,
            ToolResultProjectionPolicy::InlineUnlessLarge,
        ),
        entry(
            "palyra.sleep",
            "Wait for a bounded number of milliseconds.",
            object_schema(
                &["duration_ms"],
                vec![("duration_ms", json!({"type":"integer","minimum":0,"maximum":5000}))],
                false,
            ),
            ToolParallelismPolicy::Idempotent,
            ToolResultProjectionPolicy::InlineUnlessLarge,
        ),
        entry(
            "palyra.memory.search",
            "Search scoped Palyra memory and return redacted hits.",
            object_schema(
                &["query"],
                vec![
                    ("query", json!({"type":"string","maxLength":8192})),
                    (
                        "scope",
                        json!({"type":"string","enum":["principal","session","channel"],"description":"Defaults to principal for cross-session recall. Use session only for the current session, and channel only for authenticated channel memory."}),
                    ),
                    ("top_k", json!({"type":"integer","minimum":1,"maximum":20})),
                    ("min_score", json!({"type":"number","minimum":0.0,"maximum":1.0})),
                    ("tags", json!({"type":"array","items":{"type":"string"},"maxItems":16})),
                    (
                        "sources",
                        json!({"type":"array","items":{"type":"string","enum":["manual","summary","import","tape:user_message","tape:tool_result"]},"maxItems":16}),
                    ),
                ],
                false,
            ),
            ToolParallelismPolicy::ReadOnly,
            ToolResultProjectionPolicy::InlineUnlessLarge,
        ),
        entry(
            "palyra.memory.recall",
            "Build a scoped recall preview from memory, workspace and run evidence.",
            object_schema(
                &["query"],
                vec![
                    ("query", json!({"type":"string","maxLength":8192})),
                    ("channel", json!({"type":"string"})),
                    (
                        "session_id",
                        json!({"type":"string","description":"Optional exact session id. Do not ask users for this for 'previous session' or 'last time'; omit it for principal cross-session recall."}),
                    ),
                    ("agent_id", json!({"type":"string"})),
                    ("memory_top_k", json!({"type":"integer","minimum":0,"maximum":16})),
                    ("workspace_top_k", json!({"type":"integer","minimum":0,"maximum":16})),
                    ("min_score", json!({"type":"number","minimum":0.0,"maximum":1.0})),
                    ("max_candidates", json!({"type":"integer","minimum":0,"maximum":12})),
                    (
                        "prompt_budget_tokens",
                        json!({"type":"integer","minimum":512,"maximum":4096}),
                    ),
                    ("workspace_prefix", json!({"type":"string"})),
                    ("include_workspace_historical", json!({"type":"boolean"})),
                    ("include_workspace_quarantined", json!({"type":"boolean"})),
                ],
                false,
            ),
            ToolParallelismPolicy::ReadOnly,
            ToolResultProjectionPolicy::InlineUnlessLarge,
        ),
        entry(
            "palyra.memory.session_search",
            "Search prior session transcripts for facts that were not stored as durable memory.",
            object_schema(
                &["query"],
                vec![
                    ("query", json!({"type":"string","maxLength":8192,"description":"Search text for prior-session transcript recall. Use this for previous session, last time, earlier conversation, or temporary facts explicitly not saved as memory."})),
                    ("channel", json!({"type":"string"})),
                    ("top_k", json!({"type":"integer","minimum":1,"maximum":24})),
                    ("min_score", json!({"type":"number","minimum":0.0,"maximum":1.0})),
                    ("window_before", json!({"type":"integer","minimum":0,"maximum":8})),
                    ("window_after", json!({"type":"integer","minimum":0,"maximum":8})),
                    (
                        "max_windows_per_session",
                        json!({"type":"integer","minimum":1,"maximum":8}),
                    ),
                    ("include_archived", json!({"type":"boolean"})),
                ],
                false,
            ),
            ToolParallelismPolicy::ReadOnly,
            ToolResultProjectionPolicy::InlineUnlessLarge,
        ),
        entry(
            "palyra.memory.retain",
            "Write a reviewable scoped memory item with provenance.",
            object_schema(
                &["content_text"],
                vec![
                    ("content_text", json!({"type":"string","maxLength":8192})),
                    ("scope", json!({"type":"string","enum":["session","principal","workspace"]})),
                    (
                        "source",
                        json!({"type":"string","enum":["manual","summary","import","tape:user_message","tape:tool_result"]}),
                    ),
                    ("tags", json!({"type":"array","items":{"type":"string"},"maxItems":16})),
                    ("confidence", json!({"type":"number","minimum":0.0,"maximum":1.0})),
                    ("ttl_ms", json!({"type":"integer","minimum":0})),
                    ("ttl_unix_ms", json!({"type":"integer","minimum":0})),
                    (
                        "provenance",
                        json!({"type":"object","properties":{},"additionalProperties":true}),
                    ),
                ],
                false,
            ),
            ToolParallelismPolicy::Exclusive,
            ToolResultProjectionPolicy::InlineUnlessLarge,
        ),
        entry(
            "palyra.memory.reflect",
            "Extract bounded memory reflection candidates from run context.",
            object_schema(
                &["content_text"],
                vec![
                    ("content_text", json!({"type":"string","maxLength":8192})),
                    (
                        "category",
                        json!({"type":"string","enum":["durable_fact","preference","procedure"]}),
                    ),
                    ("confidence", json!({"type":"number","minimum":0.0,"maximum":1.0})),
                ],
                false,
            ),
            ToolParallelismPolicy::ReadOnly,
            ToolResultProjectionPolicy::InlineUnlessLarge,
        ),
        entry(
            "palyra.routines.query",
            "Inspect routine definitions, run history, and schedule previews. Use operation=schedule_preview with phrase such as 'every 40 seconds' before creating scheduled monitors.",
            object_schema(
                &[],
                vec![
                    (
                        "operation",
                        json!({"type":"string","enum":["list","get","list_runs","schedule_preview"]}),
                    ),
                    ("routine_id", json!({"type":"string"})),
                    ("phrase", json!({"type":"string"})),
                    ("timezone", json!({"type":"string","enum":["local","utc"]})),
                    ("limit", json!({"type":"integer","minimum":1,"maximum":500})),
                ],
                true,
            ),
            ToolParallelismPolicy::ReadOnly,
            ToolResultProjectionPolicy::SummarizeAndArtifact,
        ),
        entry(
            "palyra.routines.control",
            "Create, update, pause, resume, or manually dispatch routines through the approval-aware runtime. For new reminders and monitors, omit routine_id and use operation=upsert with trigger_kind=schedule, name, prompt, and natural_language_schedule such as 'every 30 minutes' or 'every 40 seconds'.",
            object_schema(
                &["operation"],
                vec![
                    (
                        "operation",
                        json!({"type":"string","enum":["upsert","pause","resume","run_now","test_run"]}),
                    ),
                    (
                        "routine_id",
                        json!({"type":"string","description":"Canonical ULID returned by a previous successful routine operation. Omit this when creating a new routine; do not put human slugs here."}),
                    ),
                    ("name", json!({"type":"string"})),
                    ("prompt", json!({"type":"string"})),
                    (
                        "trigger_kind",
                        json!({"type":"string","enum":["schedule","hook","webhook","system_event","manual"]}),
                    ),
                    ("natural_language_schedule", json!({"type":"string"})),
                    ("schedule_type", json!({"type":"string","enum":["cron","every","at"]})),
                    (
                        "every_interval_ms",
                        json!({"type":"integer","minimum":30000,"description":"Minimum 30000 ms for durable routines; use palyra.sleep for shorter bounded in-session polling."}),
                    ),
                    ("cron_expression", json!({"type":"string"})),
                    ("at_timestamp_rfc3339", json!({"type":"string"})),
                    (
                        "delivery_mode",
                        json!({"type":"string","enum":["same_channel","specific_channel","local_only","logs_only"]}),
                    ),
                    (
                        "execution_posture",
                        json!({"type":"string","enum":["standard","sensitive_tools"]}),
                    ),
                    ("enabled", json!({"type":"boolean"})),
                ],
                true,
            ),
            ToolParallelismPolicy::Exclusive,
            ToolResultProjectionPolicy::RedactedPreviewAndArtifact,
        ),
        entry(
            "palyra.artifact.read",
            "Read a bounded scoped chunk from a tool-result artifact.",
            object_schema(
                &["artifact_id"],
                vec![
                    ("artifact_id", json!({"type":"string"})),
                    ("expected_digest_sha256", json!({"type":"string"})),
                    ("offset_bytes", json!({"type":"integer","minimum":0})),
                    ("max_bytes", json!({"type":"integer","minimum":1})),
                    ("text_preview", json!({"type":"boolean"})),
                ],
                false,
            ),
            ToolParallelismPolicy::ReadOnly,
            ToolResultProjectionPolicy::InlineUnlessLarge,
        ),
        entry(
            "palyra.fs.read_file",
            "Read a bounded chunk from a file inside the current agent workspace root. Accepts relative paths and virtual workspace aliases such as /workspace/file.txt.",
            object_schema(
                &["path"],
                vec![
                    ("path", json!({"type":"string"})),
                    ("offset_bytes", json!({"type":"integer","minimum":0})),
                    ("max_bytes", json!({"type":"integer","minimum":1})),
                ],
                false,
            ),
            ToolParallelismPolicy::ReadOnly,
            ToolResultProjectionPolicy::InlineUnlessLarge,
        ),
        entry(
            "palyra.fs.list_dir",
            "List entries in a directory inside the current agent workspace root. Use this for discovery instead of process.run find, grep, cat, or shell commands.",
            object_schema(
                &[],
                vec![
                    (
                        "path",
                        json!({"type":"string","description":"Directory path relative to the workspace root. Omit for the workspace root. /workspace and workspace/ are accepted virtual workspace aliases."}),
                    ),
                    ("max_entries", json!({"type":"integer","minimum":1,"maximum":512})),
                ],
                false,
            ),
            ToolParallelismPolicy::ReadOnly,
            ToolResultProjectionPolicy::InlineUnlessLarge,
        ),
        entry(
            "palyra.delegation.query",
            "Inspect delegated child tasks, child run status and merge previews in the current scope.",
            object_schema(
                &["operation"],
                vec![
                    (
                        "operation",
                        json!({"type":"string","enum":["list","status","merge_preview"]}),
                    ),
                    ("session_id", json!({"type":"string"})),
                    ("parent_run_id", json!({"type":"string"})),
                    ("task_id", json!({"type":"string"})),
                    ("run_id", json!({"type":"string"})),
                    ("include_completed", json!({"type":"boolean"})),
                ],
                false,
            ),
            ToolParallelismPolicy::ReadOnly,
            ToolResultProjectionPolicy::InlineUnlessLarge,
        ),
        entry(
            "palyra.delegation.control",
            "Create or interrupt bounded delegated child runs through the journaled runtime.",
            object_schema(
                &["operation"],
                vec![
                    ("operation", json!({"type":"string","enum":["delegate","interrupt"]})),
                    ("objective", json!({"type":"string","maxLength":8192})),
                    ("profile_id", json!({"type":"string"})),
                    ("template_id", json!({"type":"string"})),
                    ("parent_run_id", json!({"type":"string"})),
                    ("task_id", json!({"type":"string"})),
                    ("run_id", json!({"type":"string"})),
                    ("reason", json!({"type":"string","maxLength":2048})),
                    ("priority", json!({"type":"integer","minimum":-10,"maximum":10})),
                    ("budget_tokens", json!({"type":"integer","minimum":1})),
                    ("max_attempts", json!({"type":"integer","minimum":1,"maximum":16})),
                    ("execution_mode", json!({"type":"string","enum":["serial","parallel"]})),
                    ("group_id", json!({"type":"string"})),
                    ("model_profile", json!({"type":"string"})),
                    ("memory_scope", json!({"type":"string","enum":["none","parent_session","parent_session_and_workspace","workspace_only"]})),
                    ("tool_allowlist", json!({"type":"array","items":{"type":"string"},"maxItems":64})),
                    ("skill_allowlist", json!({"type":"array","items":{"type":"string"},"maxItems":64})),
                    ("approval_required", json!({"type":"boolean"})),
                    ("max_concurrent_children", json!({"type":"integer","minimum":1})),
                    ("max_children_per_parent", json!({"type":"integer","minimum":1})),
                    ("max_total_children", json!({"type":"integer","minimum":1})),
                    ("max_parallel_groups", json!({"type":"integer","minimum":1})),
                    ("max_depth", json!({"type":"integer","minimum":1})),
                    ("max_budget_share_bps", json!({"type":"integer","minimum":1,"maximum":10000})),
                    ("child_timeout_ms", json!({"type":"integer","minimum":1000})),
                ],
                false,
            ),
            ToolParallelismPolicy::Exclusive,
            ToolResultProjectionPolicy::RedactedPreviewAndArtifact,
        ),
        entry(
            "palyra.http.fetch",
            "Fetch an HTTP(S) URL through Palyra SSRF, header and content-type guardrails.",
            object_schema(
                &["url"],
                vec![
                    ("url", json!({"type":"string"})),
                    ("method", json!({"type":"string","enum":["GET","HEAD","POST"]})),
                    ("body", json!({"type":"string"})),
                    (
                        "headers",
                        json!({"type":"object","properties":{},"additionalProperties":{"type":"string"}}),
                    ),
                    ("allow_redirects", json!({"type":"boolean"})),
                    ("max_redirects", json!({"type":"integer","minimum":1,"maximum":20})),
                    ("allow_private_targets", json!({"type":"boolean"})),
                    ("max_response_bytes", json!({"type":"integer","minimum":1})),
                    ("cache", json!({"type":"boolean"})),
                    ("cache_ttl_ms", json!({"type":"integer","minimum":1})),
                    (
                        "allowed_content_types",
                        json!({"type":"array","items":{"type":"string"},"maxItems":32}),
                    ),
                    (
                        "credential_bindings",
                        json!({"type":"array","items":{"type":"object","properties":{},"additionalProperties":true}}),
                    ),
                ],
                false,
            ),
            ToolParallelismPolicy::Idempotent,
            ToolResultProjectionPolicy::RedactedPreviewAndArtifact,
        ),
        entry(
            "palyra.process.run",
            "Run a local process using the configured process posture. Local desktop defaults may allow host-wide execution; restrictive deployments can use executable allowlists, egress controls, and workspace scoping. Use palyra.fs.apply_patch, not this tool, for file writes.",
            object_schema(
                &["command"],
                vec![
                    (
                        "command",
                        json!({
                            "type":"string",
                            "maxLength":128,
                            "description":"Bare executable or portable builtin name only. When process_runner.allowed_executables contains '*', any PATH-resolvable executable is permitted by policy. Do not include arguments, shell syntax, or repeat this value in args."
                        }),
                    ),
                    (
                        "args",
                        json!({
                            "type":"array",
                            "items":{"type":"string"},
                            "maxItems":64,
                            "description":"Command arguments only. For `echo hello`, use command='echo' and args=['hello'], not args=['echo hello']. For `node script.js`, use command='node' and args=['script.js'], not args=['node','script.js']; the runtime normalizes a duplicated leading command token, but callers should not rely on that. Do not use mkdir, touch, echo redirection, or interpreter eval for file writes; use palyra.fs.apply_patch first, then use this tool only to verify."
                        }),
                    ),
                    (
                        "cwd",
                        json!({"type":"string","description":"Workspace-confined working directory. Omit for the workspace root, or use /workspace and /workspace/subdir as virtual workspace aliases."}),
                    ),
                    (
                        "background",
                        json!({
                            "type":"boolean",
                            "description":"Start an allowlisted long-running local process and return immediately. Use this instead of shell background syntax or nohup for temporary dev servers. For local browser verification, bind to 127.0.0.1 with an explicit port and set timeout_ms to a bounded verification window such as 60000."
                        }),
                    ),
                    (
                        "requested_egress_hosts",
                        json!({"type":"array","items":{"type":"string"},"maxItems":64}),
                    ),
                    ("timeout_ms", json!({"type":"integer","minimum":1})),
                ],
                false,
            ),
            ToolParallelismPolicy::Exclusive,
            ToolResultProjectionPolicy::RedactedPreviewAndArtifact,
        ),
        entry(
            "palyra.tool_program.run",
            "Execute a bounded ToolProgram DAG through nested tool policy gates.",
            object_schema(
                &["schema_version", "program_id", "granted_tools", "steps"],
                vec![
                    ("schema_version", json!({"type":"integer","enum":[1]})),
                    ("program_id", json!({"type":"string","maxLength":128})),
                    (
                        "granted_tools",
                        json!({"type":"array","items":{"type":"string","maxLength":256},"minItems":1,"maxItems":64}),
                    ),
                    (
                        "budgets",
                        json!({"type":"object","properties":{},"additionalProperties":true}),
                    ),
                    (
                        "safety_policy",
                        json!({"type":"object","properties":{},"additionalProperties":true}),
                    ),
                    (
                        "steps",
                        json!({"type":"array","items":{"type":"object","properties":{},"additionalProperties":true},"maxItems":32}),
                    ),
                ],
                false,
            ),
            ToolParallelismPolicy::Exclusive,
            ToolResultProjectionPolicy::RedactedPreviewAndArtifact,
        ),
        entry(
            "palyra.fs.apply_patch",
            "Create, update, or delete workspace files by applying a strict workspace-confined Palyra patch document with attestation. This is the primary file-write tool.",
            object_schema(
                &["patch"],
                vec![
                    (
                        "patch",
                        json!({
                            "type":"string",
                            "description":"A complete Palyra patch document. It must start with '*** Begin Patch', contain one or more '*** Add File:', '*** Replace File:', '*** Update File:', or '*** Delete File:' operations, and end with '*** End Patch'. The final non-whitespace line must be exactly '*** End Patch'; never send a partial or truncated patch. For large file creation or multi-file changes, split work into multiple smaller complete apply_patch calls. Add-file and replace-file body lines may start with '+'. For Add File, missing parent directories are created automatically; Replace File requires the target to exist and is the deterministic fallback after reading a file when update hunk context cannot be matched. Update-file operations require '@@' hunks whose lines start with ' ', '+', or '-'. Use forward-slash relative paths only, such as reports/report.md; never use host absolute paths."
                        }),
                    ),
                    (
                        "workspace_root",
                        json!({
                            "type":"string",
                            "description":"Optional existing workspace subdirectory to treat as the patch root. Omit for the agent workspace root."
                        }),
                    ),
                    ("dry_run", json!({"type":"boolean"})),
                ],
                false,
            ),
            ToolParallelismPolicy::Exclusive,
            ToolResultProjectionPolicy::RedactedPreviewAndArtifact,
        ),
        entry(
            "palyra.plugin.run",
            "Run a verified Palyra skill or bounded inline WASM module.",
            object_schema(
                &[],
                vec![
                    ("skill_id", json!({"type":"string"})),
                    ("skill_version", json!({"type":"string"})),
                    ("module_path", json!({"type":"string"})),
                    ("tool_id", json!({"type":"string"})),
                    ("module_wat", json!({"type":"string"})),
                    ("module_base64", json!({"type":"string"})),
                    ("entrypoint", json!({"type":"string"})),
                    (
                        "capabilities",
                        json!({"type":"object","properties":{},"additionalProperties":true}),
                    ),
                ],
                false,
            ),
            ToolParallelismPolicy::Exclusive,
            ToolResultProjectionPolicy::RedactedPreviewAndArtifact,
        ),
    ];

    for browser_tool in browser_tool_names() {
        let projection_policy = if *browser_tool == "palyra.browser.session.create" {
            ToolResultProjectionPolicy::InlineUnlessLarge
        } else {
            ToolResultProjectionPolicy::RedactedPreviewAndArtifact
        };
        entries.push(entry(
            browser_tool,
            browser_tool_description(browser_tool),
            browser_tool_schema(browser_tool),
            ToolParallelismPolicy::Exclusive,
            projection_policy,
        ));
    }

    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
}

pub(crate) fn registry_entry(tool_name: &str) -> Option<ToolRegistryEntry> {
    registry_entries().into_iter().find(|entry| entry.name == tool_name)
}

fn entry(
    name: &str,
    description: &str,
    input_schema: Value,
    parallelism_policy: ToolParallelismPolicy,
    projection_policy: ToolResultProjectionPolicy,
) -> ToolRegistryEntry {
    let capabilities = tool_metadata(name)
        .map(|metadata| {
            metadata
                .capabilities
                .iter()
                .map(|capability| capability.policy_name().to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    ToolRegistryEntry {
        name: name.to_owned(),
        description: description.to_owned(),
        version: TOOL_REGISTRY_ENTRY_VERSION,
        provenance: "builtin:palyra-daemon".to_owned(),
        schema_hash: stable_hash_value(&input_schema),
        input_schema,
        capabilities,
        approval_posture: if tool_requires_approval(name) {
            ToolApprovalPosture::ApprovalRequired
        } else {
            ToolApprovalPosture::Safe
        },
        projection_policy,
        parallelism_policy,
        target_surfaces: vec![ToolExposureSurface::RunStream, ToolExposureSurface::RouteMessage],
    }
}

fn object_schema(required: &[&str], properties: Vec<(&str, Value)>, additional: bool) -> Value {
    let mut property_map = Map::new();
    for (name, schema) in properties {
        property_map.insert(name.to_owned(), schema);
    }
    json!({
        "type": "object",
        "properties": property_map,
        "required": required,
        "additionalProperties": additional,
    })
}

fn browser_tool_names() -> &'static [&'static str] {
    &[
        "palyra.browser.session.create",
        "palyra.browser.session.close",
        "palyra.browser.navigate",
        "palyra.browser.click",
        "palyra.browser.type",
        "palyra.browser.press",
        "palyra.browser.select",
        "palyra.browser.highlight",
        "palyra.browser.scroll",
        "palyra.browser.wait_for",
        "palyra.browser.title",
        "palyra.browser.screenshot",
        "palyra.browser.pdf",
        "palyra.browser.observe",
        "palyra.browser.network_log",
        "palyra.browser.console_log",
        "palyra.browser.reset_state",
        "palyra.browser.tabs.list",
        "palyra.browser.tabs.open",
        "palyra.browser.tabs.switch",
        "palyra.browser.tabs.close",
        "palyra.browser.permissions.get",
        "palyra.browser.permissions.set",
    ]
}

fn browser_tool_description(tool_name: &str) -> &'static str {
    match tool_name {
        "palyra.browser.session.create" => "Create a brokered browser session.",
        "palyra.browser.session.close" => "Close a brokered browser session.",
        "palyra.browser.navigate" => "Navigate a brokered browser session to a URL.",
        "palyra.browser.click" => "Click an element in a brokered browser session.",
        "palyra.browser.type" => "Type text in a brokered browser session.",
        "palyra.browser.press" => "Press a key in a brokered browser session.",
        "palyra.browser.select" => "Select an option in a brokered browser session.",
        "palyra.browser.highlight" => "Highlight an element in a brokered browser session.",
        "palyra.browser.scroll" => "Scroll a brokered browser session.",
        "palyra.browser.wait_for" => "Wait for a browser condition.",
        "palyra.browser.title" => "Read the current browser title.",
        "palyra.browser.screenshot" => {
            "Capture a bounded browser screenshot; do not use it alone as visible text evidence."
        }
        "palyra.browser.pdf" => "Capture a bounded browser PDF.",
        "palyra.browser.observe" => {
            "Observe visible browser state including visible text for page-content claims."
        }
        "palyra.browser.network_log" => "Read bounded browser network logs.",
        "palyra.browser.console_log" => "Read bounded browser console logs.",
        "palyra.browser.reset_state" => "Reset browser session state.",
        "palyra.browser.tabs.list" => "List browser tabs.",
        "palyra.browser.tabs.open" => "Open a browser tab.",
        "palyra.browser.tabs.switch" => "Switch the active browser tab.",
        "palyra.browser.tabs.close" => "Close a browser tab.",
        "palyra.browser.permissions.get" => "Read browser permission state.",
        "palyra.browser.permissions.set" => "Update browser permission state.",
        _ => "Operate a brokered browser session.",
    }
}

fn browser_tool_schema(tool_name: &str) -> Value {
    let mut properties = vec![
        (
            "session_id",
            json!({
                "type":"string",
                "description":"Required for every browser tool except palyra.browser.session.create. Copy the exact 26-character session_id returned by palyra.browser.session.create; do not invent, shorten, or reuse a URL/label as the session_id."
            }),
        ),
        ("timeout_ms", json!({"type":"integer","minimum":1})),
    ];
    let mut required = if tool_name == "palyra.browser.session.create" {
        Vec::<&str>::new()
    } else {
        vec!["session_id"]
    };
    match tool_name {
        "palyra.browser.navigate" | "palyra.browser.tabs.open" => {
            properties.push((
                "url",
                json!({
                    "type":"string",
                    "description":"Target URL. For localhost, 127.0.0.1, or other private targets, set allow_private_targets=true on both session.create and this navigation/open call."
                }),
            ));
            properties.push((
                "allow_private_targets",
                json!({
                    "type":"boolean",
                    "description":"Required when url targets localhost, 127.0.0.1, RFC1918/private IPs, or other private network hosts."
                }),
            ));
            required.push("url");
        }
        "palyra.browser.click" | "palyra.browser.highlight" => {
            properties.push(("selector", json!({"type":"string"})));
            required.push("selector");
        }
        "palyra.browser.type" => {
            properties.push(("selector", json!({"type":"string"})));
            properties.push(("text", json!({"type":"string"})));
            required.extend(["selector", "text"]);
        }
        "palyra.browser.press" => {
            properties.push(("key", json!({"type":"string"})));
            required.push("key");
        }
        "palyra.browser.select" => {
            properties.push(("selector", json!({"type":"string"})));
            properties.push(("value", json!({"type":"string"})));
            required.extend(["selector", "value"]);
        }
        "palyra.browser.scroll" => {
            properties.push(("delta_x", json!({"type":"integer"})));
            properties.push(("delta_y", json!({"type":"integer"})));
        }
        "palyra.browser.session.create" => {
            properties.push(("profile_id", json!({"type":"string"})));
            properties.push(("private_profile", json!({"type":"boolean"})));
            properties.push(("allow_private_targets", json!({"type":"boolean"})));
            properties.push(("allow_downloads", json!({"type":"boolean"})));
            properties.push((
                "budget",
                json!({"type":"object","properties":{},"additionalProperties":true}),
            ));
        }
        "palyra.browser.observe" => {
            properties.push(("include_dom_snapshot", json!({"type":"boolean","default":true})));
            properties
                .push(("include_accessibility_tree", json!({"type":"boolean","default":true})));
            properties.push(("include_visible_text", json!({"type":"boolean","default":true})));
            properties.push(("max_dom_snapshot_bytes", json!({"type":"integer","minimum":0})));
            properties
                .push(("max_accessibility_tree_bytes", json!({"type":"integer","minimum":0})));
            properties.push(("max_visible_text_bytes", json!({"type":"integer","minimum":0})));
        }
        _ => {}
    }
    object_schema(required.as_slice(), properties, true)
}

#[cfg(test)]
mod tests {
    use super::registry_entry;

    #[test]
    fn process_runner_registry_steers_file_writes_to_patch_tool() {
        let entry = registry_entry("palyra.process.run").expect("process runner entry exists");
        assert!(entry.description.contains("not this tool"));

        let args_description = entry
            .input_schema
            .pointer("/properties/args/description")
            .and_then(serde_json::Value::as_str)
            .expect("args description should be visible to models");
        assert!(args_description.contains("Do not use mkdir, touch"));
        assert!(args_description.contains("palyra.fs.apply_patch first"));
        assert!(args_description.contains("not args=['node','script.js']"));

        let cwd_description = entry
            .input_schema
            .pointer("/properties/cwd/description")
            .and_then(serde_json::Value::as_str)
            .expect("cwd description should be visible to models");
        assert!(cwd_description.contains("/workspace/subdir"));
    }

    #[test]
    fn apply_patch_registry_explains_workspace_report_file_creation() {
        let entry = registry_entry("palyra.fs.apply_patch").expect("patch entry exists");
        assert!(entry.description.contains("primary file-write tool"));

        let patch_description = entry
            .input_schema
            .pointer("/properties/patch/description")
            .and_then(serde_json::Value::as_str)
            .expect("patch description should be visible to models");
        assert!(patch_description.contains("*** Replace File:"));
        assert!(patch_description.contains("final non-whitespace line"));
        assert!(patch_description.contains("multiple smaller complete apply_patch calls"));
        assert!(patch_description.contains("context cannot be matched"));
        assert!(patch_description.contains("missing parent directories"));
        assert!(patch_description.contains("reports/report.md"));
        assert!(patch_description.contains("never use host absolute paths"));
    }

    #[test]
    fn browser_registry_marks_observe_as_visible_text_evidence() {
        let observe = registry_entry("palyra.browser.observe").expect("observe entry exists");
        assert!(observe.description.contains("visible text"));
        assert_eq!(
            observe.input_schema.pointer("/required/0").and_then(serde_json::Value::as_str),
            Some("session_id")
        );

        let navigate = registry_entry("palyra.browser.navigate").expect("navigate entry exists");
        let required = navigate
            .input_schema
            .pointer("/required")
            .and_then(serde_json::Value::as_array)
            .expect("navigate required fields should be present");
        assert!(required.iter().any(|value| value.as_str() == Some("session_id")));
        assert!(required.iter().any(|value| value.as_str() == Some("url")));
        let session_description = navigate
            .input_schema
            .pointer("/properties/session_id/description")
            .and_then(serde_json::Value::as_str)
            .expect("browser session_id description should be visible to models");
        assert!(session_description.contains("26-character session_id"));
        let url_description = navigate
            .input_schema
            .pointer("/properties/url/description")
            .and_then(serde_json::Value::as_str)
            .expect("browser url description should be visible to models");
        assert!(url_description.contains("localhost"));

        let screenshot =
            registry_entry("palyra.browser.screenshot").expect("screenshot entry exists");
        assert!(screenshot.description.contains("do not use it alone"));
    }
}
