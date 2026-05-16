mod checkpoint_flow;

use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use palyra_common::workspace_patch::{
    apply_workspace_patch, compute_patch_sha256, redact_patch_preview, WorkspacePatchError,
    WorkspacePatchLimits, WorkspacePatchOutcome, WorkspacePatchRedactionPolicy,
    WorkspacePatchRequest,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::warn;
use ulid::Ulid;

use crate::{
    agents::AgentResolveRequest,
    gateway::{
        current_unix_ms, GatewayRuntimeState, MAX_PATCH_TOOL_MARKER_BYTES,
        MAX_PATCH_TOOL_PATTERN_BYTES, MAX_PATCH_TOOL_REDACTION_PATTERNS,
        MAX_PATCH_TOOL_SECRET_FILE_MARKERS, MAX_WORKSPACE_PATCH_TOOL_INPUT_BYTES,
    },
    tool_protocol::{ToolAttestation, ToolExecutionOutcome},
};

use checkpoint_flow::WorkspacePatchMutationRequest;

const WORKSPACE_PATCH_GRAMMAR_HINT: &str = "Use a complete Palyra patch document: begin with exactly '*** Begin Patch', then operation headers like '*** Add File: path', '*** Replace File: path', or '*** Update File: path', end with exactly one '*** End Patch'. Never send a partial or truncated patch. For large file creation or multi-file changes, split work into multiple smaller complete apply_patch calls. Add-file and replace-file content lines may start with '+'. Use Add File only for missing files. If an Update File hunk fails with context not found, read the current file and retry with Replace File plus the full intended file content. Update-file hunks must start with '@@' and hunk lines must start with ' ', '+', or '-'. JSON files are validated after patch planning; if JSON validation fails, retry with the complete valid JSON file content.";

pub(crate) struct WorkspacePatchToolRequest<'a> {
    pub(crate) principal: &'a str,
    pub(crate) device_id: &'a str,
    pub(crate) channel: Option<&'a str>,
    pub(crate) session_id: &'a str,
    pub(crate) run_id: &'a str,
    pub(crate) proposal_id: &'a str,
    pub(crate) input_json: &'a [u8],
}

impl<'a> WorkspacePatchToolRequest<'a> {
    pub(crate) fn from_runtime_context(
        context: crate::gateway::ToolRuntimeExecutionContext<'a>,
        proposal_id: &'a str,
        input_json: &'a [u8],
    ) -> Self {
        Self {
            principal: context.principal,
            device_id: context.device_id,
            channel: context.channel,
            session_id: context.session_id,
            run_id: context.run_id,
            proposal_id,
            input_json,
        }
    }
}

pub(crate) async fn execute_workspace_patch_tool(
    runtime_state: &Arc<GatewayRuntimeState>,
    request: WorkspacePatchToolRequest<'_>,
) -> ToolExecutionOutcome {
    let WorkspacePatchToolRequest {
        principal,
        device_id,
        channel,
        session_id,
        run_id,
        proposal_id,
        input_json,
    } = request;
    if input_json.len() > MAX_WORKSPACE_PATCH_TOOL_INPUT_BYTES {
        return workspace_patch_tool_execution_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!(
                "palyra.fs.apply_patch input exceeds {MAX_WORKSPACE_PATCH_TOOL_INPUT_BYTES} bytes"
            ),
        );
    }

    let parsed = match serde_json::from_slice::<Value>(input_json) {
        Ok(Value::Object(map)) => map,
        Ok(_) => {
            return workspace_patch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.fs.apply_patch requires JSON object input".to_owned(),
            );
        }
        Err(error) => {
            return workspace_patch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!("palyra.fs.apply_patch invalid JSON input: {error}"),
            );
        }
    };

    let patch = match parsed.get("patch").and_then(Value::as_str) {
        Some(value) if !value.trim().is_empty() => value.to_owned(),
        _ => {
            return workspace_patch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.fs.apply_patch requires non-empty string field 'patch'".to_owned(),
            );
        }
    };

    let dry_run = match parsed.get("dry_run") {
        Some(Value::Bool(value)) => *value,
        Some(_) => {
            return workspace_patch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.fs.apply_patch dry_run must be a boolean".to_owned(),
            );
        }
        None => false,
    };

    let mut redaction_policy = WorkspacePatchRedactionPolicy::default();
    match parse_patch_string_array_field(
        &parsed,
        "redaction_patterns",
        MAX_PATCH_TOOL_REDACTION_PATTERNS,
        MAX_PATCH_TOOL_PATTERN_BYTES,
    ) {
        Ok(Some(patterns)) => {
            extend_patch_string_defaults(&mut redaction_policy.redaction_patterns, patterns);
        }
        Ok(None) => {}
        Err(message) => {
            return workspace_patch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                message,
            );
        }
    }
    match parse_patch_string_array_field(
        &parsed,
        "secret_file_markers",
        MAX_PATCH_TOOL_SECRET_FILE_MARKERS,
        MAX_PATCH_TOOL_MARKER_BYTES,
    ) {
        Ok(Some(markers)) => {
            extend_patch_string_defaults(&mut redaction_policy.secret_file_markers, markers);
        }
        Ok(None) => {}
        Err(message) => {
            return workspace_patch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                message,
            );
        }
    }

    let agent_outcome = match runtime_state
        .resolve_agent_for_context(AgentResolveRequest {
            principal: principal.to_owned(),
            channel: channel.map(str::to_owned),
            session_id: Some(session_id.to_owned()),
            preferred_agent_id: None,
            persist_session_binding: false,
        })
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            return workspace_patch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!(
                    "palyra.fs.apply_patch failed to resolve agent workspace: {}",
                    error.message()
                ),
            );
        }
    };
    let agent_workspace_roots =
        agent_outcome.agent.workspace_roots.iter().map(PathBuf::from).collect::<Vec<_>>();
    let workspace_roots =
        match resolve_workspace_patch_roots(&parsed, agent_workspace_roots.as_slice()) {
            Ok(workspace_roots) => workspace_roots,
            Err(message) => {
                return workspace_patch_tool_execution_outcome(
                    proposal_id,
                    input_json,
                    false,
                    b"{}".to_vec(),
                    message,
                );
            }
        };
    let limits = WorkspacePatchLimits::default();
    let planning_request = WorkspacePatchRequest {
        patch: patch.clone(),
        dry_run: true,
        redaction_policy: redaction_policy.clone(),
    };

    let planned_outcome =
        match apply_workspace_patch(workspace_roots.as_slice(), &planning_request, &limits) {
            Ok(outcome) => outcome,
            Err(error) => {
                return workspace_patch_error_outcome(
                    proposal_id,
                    input_json,
                    dry_run,
                    patch.as_str(),
                    &redaction_policy,
                    &limits,
                    &error,
                );
            }
        };

    if dry_run {
        return serialize_workspace_patch_success(proposal_id, input_json, &planned_outcome);
    }

    checkpoint_flow::execute_workspace_patch_mutation(
        runtime_state,
        WorkspacePatchMutationRequest {
            principal,
            device_id,
            channel,
            session_id,
            run_id,
            proposal_id,
            input_json,
            patch: patch.as_str(),
            redaction_policy: &redaction_policy,
            limits: &limits,
            workspace_roots: workspace_roots.as_slice(),
            planned_outcome,
        },
    )
    .await
}

fn resolve_workspace_patch_roots(
    parsed: &serde_json::Map<String, Value>,
    agent_workspace_roots: &[PathBuf],
) -> Result<Vec<PathBuf>, String> {
    let Some(value) = parsed.get("workspace_root") else {
        return Ok(agent_workspace_roots.to_vec());
    };
    let Some(raw_workspace_root) = value.as_str() else {
        return Err("palyra.fs.apply_patch workspace_root must be a string".to_owned());
    };
    let workspace_root = raw_workspace_root.trim();
    if workspace_root.is_empty() {
        return Ok(agent_workspace_roots.to_vec());
    }
    resolve_workspace_root_override(agent_workspace_roots, workspace_root).map(|root| vec![root])
}

fn resolve_workspace_root_override(
    agent_workspace_roots: &[PathBuf],
    workspace_root: &str,
) -> Result<PathBuf, String> {
    if workspace_root.chars().any(char::is_control) {
        return Err(
            "palyra.fs.apply_patch workspace_root contains unsupported characters".to_owned()
        );
    }

    let canonical_roots = canonicalize_agent_workspace_roots(agent_workspace_roots)?;
    if canonical_roots.is_empty() {
        return Err("palyra.fs.apply_patch agent has no accessible workspace roots".to_owned());
    }

    let requested = Path::new(workspace_root);
    if requested.is_absolute() {
        return canonicalize_workspace_root_override(requested, &canonical_roots, workspace_root);
    }
    validate_relative_workspace_root_override(requested, workspace_root)?;
    for canonical_root in &canonical_roots {
        let candidate = canonical_root.join(requested);
        match canonicalize_workspace_root_override(
            candidate.as_path(),
            &canonical_roots,
            workspace_root,
        ) {
            Ok(path) => return Ok(path),
            Err(error) if error.contains("does not exist") => {}
            Err(error) => return Err(error),
        }
    }
    Err(format!(
        "palyra.fs.apply_patch workspace_root does not exist inside agent workspace roots: {workspace_root}"
    ))
}

fn canonicalize_agent_workspace_roots(
    agent_workspace_roots: &[PathBuf],
) -> Result<Vec<PathBuf>, String> {
    let mut canonical_roots = Vec::with_capacity(agent_workspace_roots.len());
    for root in agent_workspace_roots {
        match fs::canonicalize(root) {
            Ok(canonical_root) if canonical_root.is_dir() => canonical_roots.push(canonical_root),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "palyra.fs.apply_patch failed to resolve agent workspace root {}: {error}",
                    root.display()
                ));
            }
        }
    }
    Ok(canonical_roots)
}

fn canonicalize_workspace_root_override(
    candidate: &Path,
    canonical_roots: &[PathBuf],
    workspace_root: &str,
) -> Result<PathBuf, String> {
    let canonical_candidate = fs::canonicalize(candidate).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "palyra.fs.apply_patch workspace_root does not exist inside agent workspace roots: {workspace_root}"
            )
        } else {
            format!("palyra.fs.apply_patch failed to resolve workspace_root {workspace_root}: {error}")
        }
    })?;
    if !canonical_candidate.is_dir() {
        return Err(format!(
            "palyra.fs.apply_patch workspace_root is not a directory: {workspace_root}"
        ));
    }
    if canonical_roots.iter().any(|root| canonical_candidate.starts_with(root)) {
        return Ok(canonical_candidate);
    }
    Err(format!(
        "palyra.fs.apply_patch workspace_root escapes agent workspace roots: {workspace_root}"
    ))
}

fn validate_relative_workspace_root_override(
    path: &Path,
    raw_workspace_root: &str,
) -> Result<(), String> {
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "palyra.fs.apply_patch workspace_root must stay inside agent workspace roots: {raw_workspace_root}"
                ));
            }
        }
    }
    Ok(())
}

fn serialize_workspace_patch_success(
    proposal_id: &str,
    input_json: &[u8],
    outcome: &WorkspacePatchOutcome,
) -> ToolExecutionOutcome {
    match serde_json::to_vec(outcome) {
        Ok(output_json) => workspace_patch_tool_execution_outcome(
            proposal_id,
            input_json,
            true,
            output_json,
            String::new(),
        ),
        Err(error) => workspace_patch_tool_execution_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!("palyra.fs.apply_patch failed to serialize output: {error}"),
        ),
    }
}

fn workspace_patch_error_outcome(
    proposal_id: &str,
    input_json: &[u8],
    dry_run: bool,
    patch: &str,
    redaction_policy: &WorkspacePatchRedactionPolicy,
    limits: &WorkspacePatchLimits,
    error: &WorkspacePatchError,
) -> ToolExecutionOutcome {
    if let Some((line, column)) = error.parse_location() {
        warn!(
            proposal_id = %proposal_id,
            line,
            column,
            error = %error,
            "workspace patch parse failed"
        );
    } else {
        warn!(
            proposal_id = %proposal_id,
            error = %error,
            "workspace patch execution failed"
        );
    }
    let failure_payload = json!({
        "patch_sha256": compute_patch_sha256(patch),
        "dry_run": dry_run,
        "files_touched": [],
        "rollback_performed": error.rollback_performed(),
        "redacted_preview": redact_patch_preview(
            patch,
            redaction_policy,
            limits.max_preview_bytes
        ),
        "parse_error": error
            .parse_location()
            .map(|(line, column)| json!({ "line": line, "column": column })),
        "recovery_hint": workspace_patch_recovery_hint(error),
        "grammar_hint": WORKSPACE_PATCH_GRAMMAR_HINT,
        "error": error.to_string(),
    });
    let output_json = serde_json::to_vec(&failure_payload).unwrap_or_else(|_| b"{}".to_vec());
    workspace_patch_tool_execution_outcome(
        proposal_id,
        input_json,
        false,
        output_json,
        format!(
            "palyra.fs.apply_patch failed: {error}. {} {WORKSPACE_PATCH_GRAMMAR_HINT}",
            workspace_patch_recovery_hint(error)
        ),
    )
}

fn workspace_patch_recovery_hint(error: &WorkspacePatchError) -> &'static str {
    match error {
        WorkspacePatchError::Parse { message, .. }
            if message.contains("unexpected content after '*** End Patch'") =>
        {
            "Remove any duplicate terminator or text after the final '*** End Patch', then retry with one complete patch."
        }
        WorkspacePatchError::Parse { message, .. }
            if message.contains("expected '*** Begin Patch'") =>
        {
            "Start the patch with exactly '*** Begin Patch' on its own line, not a Markdown-decorated variant."
        }
        WorkspacePatchError::InvalidJsonFile { .. } => {
            "Read or reconstruct the intended JSON and retry with Replace File or Add File containing complete valid JSON only."
        }
        WorkspacePatchError::HunkApplyFailed { .. } => {
            "Read the current file and retry with either fresh context hunks or Replace File containing the full intended file content."
        }
        _ => "Inspect the patch error and retry with a smaller complete patch that preserves workspace-relative paths.",
    }
}

pub(crate) fn extend_patch_string_defaults(defaults: &mut Vec<String>, additions: Vec<String>) {
    for addition in additions {
        if !defaults.iter().any(|existing| existing == &addition) {
            defaults.push(addition);
        }
    }
}

pub(crate) fn parse_patch_string_array_field(
    payload: &serde_json::Map<String, Value>,
    field_name: &str,
    max_items: usize,
    max_item_bytes: usize,
) -> Result<Option<Vec<String>>, String> {
    let Some(value) = payload.get(field_name) else {
        return Ok(None);
    };
    let Value::Array(values) = value else {
        return Err(format!("palyra.fs.apply_patch {field_name} must be an array of strings"));
    };
    if values.len() > max_items {
        return Err(format!("palyra.fs.apply_patch {field_name} exceeds limit ({max_items})"));
    }
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let Some(raw) = value.as_str() else {
            return Err(format!("palyra.fs.apply_patch {field_name} must be an array of strings"));
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() > max_item_bytes {
            return Err(format!(
                "palyra.fs.apply_patch {field_name} entries must be <= {max_item_bytes} bytes"
            ));
        }
        parsed.push(trimmed.to_owned());
    }
    Ok(Some(parsed))
}

fn workspace_patch_tool_execution_outcome(
    proposal_id: &str,
    input_json: &[u8],
    success: bool,
    output_json: Vec<u8>,
    error: String,
) -> ToolExecutionOutcome {
    let executed_at_unix_ms = current_unix_ms();
    let mut hasher = Sha256::new();
    hasher.update(b"palyra.fs.apply_patch.attestation.v1");
    hasher.update((proposal_id.len() as u64).to_be_bytes());
    hasher.update(proposal_id.as_bytes());
    hasher.update((input_json.len() as u64).to_be_bytes());
    hasher.update(input_json);
    hasher.update([u8::from(success)]);
    hasher.update((output_json.len() as u64).to_be_bytes());
    hasher.update(output_json.as_slice());
    hasher.update((error.len() as u64).to_be_bytes());
    hasher.update(error.as_bytes());
    hasher.update(executed_at_unix_ms.to_be_bytes());
    let execution_sha256 = hex::encode(hasher.finalize());

    ToolExecutionOutcome {
        success,
        output_json,
        error,
        attestation: ToolAttestation {
            attestation_id: Ulid::new().to_string(),
            execution_sha256,
            executed_at_unix_ms,
            timed_out: false,
            executor: "workspace_patch".to_owned(),
            sandbox_enforcement: "workspace_roots".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_workspace_patch_roots, workspace_patch_error_outcome,
        workspace_patch_recovery_hint, WORKSPACE_PATCH_GRAMMAR_HINT,
    };
    use palyra_common::workspace_patch::{
        apply_workspace_patch, WorkspacePatchError, WorkspacePatchLimits, WorkspacePatchRequest,
    };
    use serde_json::{json, Value};

    #[test]
    fn workspace_root_override_targets_existing_subdirectory() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let workspace = tempdir.path().join("workspace");
        let project = workspace.join("e2e-cli").join("file-tool-smoke");
        std::fs::create_dir_all(&project).expect("project directory should exist");

        let parsed = json!({ "workspace_root": "e2e-cli/file-tool-smoke" })
            .as_object()
            .expect("json object")
            .clone();
        let roots = resolve_workspace_patch_roots(&parsed, std::slice::from_ref(&workspace))
            .expect("workspace root override should resolve");
        let patch = "*** Begin Patch\n*** Add File: calc.js\n+export const add = (a, b) => a + b;\n*** End Patch\n";

        apply_workspace_patch(
            roots.as_slice(),
            &WorkspacePatchRequest {
                patch: patch.to_owned(),
                dry_run: false,
                redaction_policy: Default::default(),
            },
            &WorkspacePatchLimits::default(),
        )
        .expect("patch should apply inside project root");

        assert!(project.join("calc.js").is_file());
        assert!(!workspace.join("calc.js").exists());
    }

    #[test]
    fn workspace_root_override_rejects_outside_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let workspace = tempdir.path().join("workspace");
        let outside = tempdir.path().join("outside");
        std::fs::create_dir_all(&workspace).expect("workspace directory should exist");
        std::fs::create_dir_all(&outside).expect("outside directory should exist");

        let parsed = json!({ "workspace_root": outside.to_string_lossy() })
            .as_object()
            .expect("json object")
            .clone();
        let error = resolve_workspace_patch_roots(&parsed, &[workspace])
            .expect_err("outside workspace_root should be rejected");

        assert!(error.contains("escapes agent workspace roots"), "unexpected error: {error}");
    }

    #[test]
    fn workspace_root_override_rejects_host_directory_even_when_near_workspace_root() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let workspace = tempdir.path().join("workspace");
        let outside = tempdir.path().join("outside");
        std::fs::create_dir_all(&workspace).expect("workspace directory should exist");
        std::fs::create_dir_all(&outside).expect("outside directory should exist");

        let parsed = json!({ "workspace_root": outside.to_string_lossy() })
            .as_object()
            .expect("json object")
            .clone();
        let error = resolve_workspace_patch_roots(&parsed, &[workspace])
            .expect_err("host workspace_root should be rejected");

        assert!(error.contains("escapes agent workspace roots"), "unexpected error: {error}");
    }

    #[test]
    fn parse_failure_result_includes_repairable_patch_grammar_hint() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let workspace = tempdir.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("workspace directory should exist");
        let limits = WorkspacePatchLimits::default();
        let request = WorkspacePatchRequest {
            patch: "function sum(a, b) { return a + b; }".to_owned(),
            dry_run: true,
            redaction_policy: Default::default(),
        };
        let error = apply_workspace_patch(std::slice::from_ref(&workspace), &request, &limits)
            .expect_err("raw file contents should fail patch parsing");

        let outcome = workspace_patch_error_outcome(
            "01ARZ3NDEKTSV4RRFFQ69G5FAW",
            br#"{"patch":"function sum(a, b) { return a + b; }"}"#,
            true,
            request.patch.as_str(),
            &request.redaction_policy,
            &limits,
            &error,
        );

        assert!(!outcome.success);
        assert!(outcome.error.contains(WORKSPACE_PATCH_GRAMMAR_HINT));
        let payload: Value =
            serde_json::from_slice(outcome.output_json.as_slice()).expect("valid failure json");
        assert_eq!(
            payload.get("grammar_hint").and_then(Value::as_str),
            Some(WORKSPACE_PATCH_GRAMMAR_HINT)
        );
        assert_eq!(payload.pointer("/parse_error/line").and_then(Value::as_u64), Some(1));
    }

    #[test]
    fn json_patch_failure_result_includes_specific_recovery_hint() {
        let error = WorkspacePatchError::InvalidJsonFile {
            path: "reports/seen.json".to_owned(),
            message: "expected value at line 1 column 1".to_owned(),
        };

        let outcome = workspace_patch_error_outcome(
            "01ARZ3NDEKTSV4RRFFQ69G5FAW",
            br#"{"patch":"*** Begin Patch\n*** Add File: reports/seen.json\n+***\n*** End Patch\n"}"#,
            false,
            "*** Begin Patch\n*** Add File: reports/seen.json\n+***\n*** End Patch\n",
            &Default::default(),
            &WorkspacePatchLimits::default(),
            &error,
        );

        let expected_hint = workspace_patch_recovery_hint(&error);

        assert!(!outcome.success);
        assert!(
            outcome.error.contains(expected_hint),
            "expected error to include recovery hint: {}",
            outcome.error
        );
        let payload: Value =
            serde_json::from_slice(outcome.output_json.as_slice()).expect("valid failure json");
        assert_eq!(payload.get("recovery_hint").and_then(Value::as_str), Some(expected_hint));
    }
}
