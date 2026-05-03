use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    agents::AgentResolveRequest,
    gateway::{
        GatewayRuntimeState, ToolRuntimeExecutionContext, MAX_WORKSPACE_READ_FILE_BYTES,
        MAX_WORKSPACE_READ_FILE_TOOL_INPUT_BYTES, WORKSPACE_READ_FILE_TOOL_NAME,
    },
    tool_protocol::{build_tool_execution_outcome, ToolExecutionOutcome},
};

#[derive(Debug, Deserialize)]
struct WorkspaceReadFileInput {
    path: String,
    #[serde(default)]
    offset_bytes: u64,
    #[serde(default)]
    max_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
struct WorkspaceReadFileOutput {
    path: String,
    workspace_root_index: usize,
    offset_bytes: u64,
    returned_bytes: u64,
    size_bytes: u64,
    eof: bool,
    chunk_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes_base64: Option<String>,
}

pub(crate) async fn execute_workspace_read_file_tool(
    runtime_state: &Arc<GatewayRuntimeState>,
    context: ToolRuntimeExecutionContext<'_>,
    proposal_id: &str,
    input_json: &[u8],
) -> ToolExecutionOutcome {
    let input = match parse_workspace_read_file_input(input_json) {
        Ok(input) => input,
        Err(error) => {
            return workspace_read_file_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                error,
            );
        }
    };

    let agent_outcome = match runtime_state
        .resolve_agent_for_context(AgentResolveRequest {
            principal: context.principal.to_owned(),
            channel: context.channel.map(str::to_owned),
            session_id: Some(context.session_id.to_owned()),
            preferred_agent_id: None,
            persist_session_binding: false,
        })
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            return workspace_read_file_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!(
                    "{WORKSPACE_READ_FILE_TOOL_NAME} failed to resolve agent workspace: {}",
                    error.message()
                ),
            );
        }
    };

    let workspace_roots =
        agent_outcome.agent.workspace_roots.iter().map(PathBuf::from).collect::<Vec<_>>();
    let read = match read_workspace_file_from_roots(workspace_roots.as_slice(), &input) {
        Ok(read) => read,
        Err(error) => {
            return workspace_read_file_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                error,
            );
        }
    };

    match serde_json::to_vec(&read) {
        Ok(output_json) => {
            workspace_read_file_outcome(proposal_id, input_json, true, output_json, String::new())
        }
        Err(error) => workspace_read_file_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!("{WORKSPACE_READ_FILE_TOOL_NAME} failed to serialize output: {error}"),
        ),
    }
}

fn parse_workspace_read_file_input(input_json: &[u8]) -> Result<WorkspaceReadFileInput, String> {
    if input_json.len() > MAX_WORKSPACE_READ_FILE_TOOL_INPUT_BYTES {
        return Err(format!(
            "{WORKSPACE_READ_FILE_TOOL_NAME} input exceeds {MAX_WORKSPACE_READ_FILE_TOOL_INPUT_BYTES} bytes"
        ));
    }

    let mut input =
        serde_json::from_slice::<WorkspaceReadFileInput>(input_json).map_err(|error| {
            format!("{WORKSPACE_READ_FILE_TOOL_NAME} input must match file read schema: {error}")
        })?;
    input.path = input.path.trim().to_owned();
    if input.path.is_empty() {
        return Err(format!(
            "{WORKSPACE_READ_FILE_TOOL_NAME} requires non-empty string field 'path'"
        ));
    }
    if matches!(input.max_bytes, Some(0)) {
        return Err(format!("{WORKSPACE_READ_FILE_TOOL_NAME} max_bytes must be >= 1"));
    }
    validate_workspace_relative_path(input.path.as_str())?;
    Ok(input)
}

fn validate_workspace_relative_path(path: &str) -> Result<(), String> {
    if path.contains('\\') {
        return Err(format!("{WORKSPACE_READ_FILE_TOOL_NAME} path must use '/' separators"));
    }
    if path.contains(':') || path.chars().any(char::is_control) {
        return Err(format!(
            "{WORKSPACE_READ_FILE_TOOL_NAME} path contains unsupported characters"
        ));
    }

    let parsed = Path::new(path);
    if parsed.is_absolute() {
        return Err(format!(
            "{WORKSPACE_READ_FILE_TOOL_NAME} path must be relative to an agent workspace root"
        ));
    }
    if !parsed.components().all(|component| matches!(component, Component::Normal(_))) {
        return Err(format!(
            "{WORKSPACE_READ_FILE_TOOL_NAME} path must not contain root, prefix, '.', or '..' components"
        ));
    }
    Ok(())
}

fn read_workspace_file_from_roots(
    workspace_roots: &[PathBuf],
    input: &WorkspaceReadFileInput,
) -> Result<WorkspaceReadFileOutput, String> {
    if workspace_roots.is_empty() {
        return Err(format!(
            "{WORKSPACE_READ_FILE_TOOL_NAME} agent has no workspace roots configured"
        ));
    }

    let mut saw_accessible_root = false;
    for (workspace_root_index, workspace_root) in workspace_roots.iter().enumerate() {
        let Ok(canonical_root) = fs::canonicalize(workspace_root) else {
            continue;
        };
        if !canonical_root.is_dir() {
            continue;
        }
        saw_accessible_root = true;

        let candidate = canonical_root.join(Path::new(input.path.as_str()));
        let canonical_target = match fs::canonicalize(&candidate) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "{WORKSPACE_READ_FILE_TOOL_NAME} failed to resolve path in workspace root {workspace_root_index}: {error}"
                ));
            }
        };
        if !canonical_target.starts_with(canonical_root.as_path()) {
            return Err(format!(
                "{WORKSPACE_READ_FILE_TOOL_NAME} path escapes agent workspace roots"
            ));
        }
        if !canonical_target.is_file() {
            return Err(format!(
                "{WORKSPACE_READ_FILE_TOOL_NAME} target is not a regular file: {}",
                input.path
            ));
        }

        return read_workspace_file_chunk(workspace_root_index, canonical_target, input);
    }

    if saw_accessible_root {
        Err(format!(
            "{WORKSPACE_READ_FILE_TOOL_NAME} file not found in agent workspace roots: {}",
            input.path
        ))
    } else {
        Err(format!("{WORKSPACE_READ_FILE_TOOL_NAME} agent has no accessible workspace roots"))
    }
}

fn read_workspace_file_chunk(
    workspace_root_index: usize,
    path: PathBuf,
    input: &WorkspaceReadFileInput,
) -> Result<WorkspaceReadFileOutput, String> {
    let mut file = File::open(path.as_path()).map_err(|error| {
        format!(
            "{WORKSPACE_READ_FILE_TOOL_NAME} failed to open workspace file {}: {error}",
            input.path
        )
    })?;
    let size_bytes = file
        .metadata()
        .map_err(|error| {
            format!(
                "{WORKSPACE_READ_FILE_TOOL_NAME} failed to inspect workspace file {}: {error}",
                input.path
            )
        })?
        .len();
    file.seek(SeekFrom::Start(input.offset_bytes)).map_err(|error| {
        format!(
            "{WORKSPACE_READ_FILE_TOOL_NAME} failed to seek workspace file {}: {error}",
            input.path
        )
    })?;

    let max_bytes = input.max_bytes.unwrap_or(MAX_WORKSPACE_READ_FILE_BYTES);
    let read_limit = usize::try_from(max_bytes.min(MAX_WORKSPACE_READ_FILE_BYTES))
        .expect("workspace read cap must fit usize");
    let mut buffer = Vec::with_capacity(read_limit.min(8192));
    file.take(read_limit as u64).read_to_end(&mut buffer).map_err(|error| {
        format!(
            "{WORKSPACE_READ_FILE_TOOL_NAME} failed to read workspace file {}: {error}",
            input.path
        )
    })?;

    let returned_bytes =
        u64::try_from(buffer.len()).expect("returned workspace file chunk size must fit u64");
    let eof = input.offset_bytes.saturating_add(returned_bytes) >= size_bytes;
    let chunk_sha256 = hex::encode(Sha256::digest(buffer.as_slice()));
    let (text, bytes_base64) = match String::from_utf8(buffer) {
        Ok(text) => (Some(text), None),
        Err(error) => (None, Some(BASE64_STANDARD.encode(error.into_bytes()))),
    };

    Ok(WorkspaceReadFileOutput {
        path: input.path.clone(),
        workspace_root_index,
        offset_bytes: input.offset_bytes,
        returned_bytes,
        size_bytes,
        eof,
        chunk_sha256,
        text,
        bytes_base64,
    })
}

fn workspace_read_file_outcome(
    proposal_id: &str,
    input_json: &[u8],
    success: bool,
    output_json: Vec<u8>,
    error: String,
) -> ToolExecutionOutcome {
    build_tool_execution_outcome(
        proposal_id,
        WORKSPACE_READ_FILE_TOOL_NAME,
        input_json,
        success,
        output_json,
        error,
        false,
        "workspace_file".to_owned(),
        "workspace_roots".to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_workspace_file_returns_utf8_text() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let file_path = tempdir.path().join("agent-e2e-tool-test.js");
        let contents = "export function add(a, b) { return a + b; }\nexport const meaning = 42;\n";
        fs::write(file_path, contents).expect("workspace file should be written");
        let input = WorkspaceReadFileInput {
            path: "agent-e2e-tool-test.js".to_owned(),
            offset_bytes: 0,
            max_bytes: None,
        };

        let output = read_workspace_file_from_roots(&[tempdir.path().to_path_buf()], &input)
            .expect("workspace file should be readable");

        assert_eq!(output.text.as_deref(), Some(contents));
        assert_eq!(output.bytes_base64, None);
        assert_eq!(output.returned_bytes, contents.len() as u64);
        assert!(output.eof);
        assert_eq!(output.workspace_root_index, 0);
    }

    #[test]
    fn read_workspace_file_returns_bounded_chunk() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("chunk.txt"), "abcdef").expect("workspace file should exist");
        let input = WorkspaceReadFileInput {
            path: "chunk.txt".to_owned(),
            offset_bytes: 2,
            max_bytes: Some(3),
        };

        let output = read_workspace_file_from_roots(&[tempdir.path().to_path_buf()], &input)
            .expect("workspace file chunk should be readable");

        assert_eq!(output.text.as_deref(), Some("cde"));
        assert_eq!(output.returned_bytes, 3);
        assert!(!output.eof);
    }

    #[test]
    fn read_workspace_file_rejects_parent_traversal() {
        let error =
            parse_workspace_read_file_input(br#"{"path":"../outside.txt"}"#).expect_err("path");

        assert!(error.contains("must not contain"), "unexpected validation error: {error}");
    }
}
