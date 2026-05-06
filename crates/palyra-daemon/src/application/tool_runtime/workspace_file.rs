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
        GatewayRuntimeState, ToolRuntimeExecutionContext, MAX_WORKSPACE_LIST_DIR_TOOL_INPUT_BYTES,
        MAX_WORKSPACE_READ_FILE_BYTES, MAX_WORKSPACE_READ_FILE_TOOL_INPUT_BYTES,
        WORKSPACE_LIST_DIR_TOOL_NAME, WORKSPACE_READ_FILE_TOOL_NAME,
    },
    sandbox_runner::process_runner_allows_host_access,
    tool_protocol::{build_tool_execution_outcome, ToolExecutionOutcome},
};

const WORKSPACE_LIST_DIR_DEFAULT_ENTRIES: usize = 128;
const WORKSPACE_LIST_DIR_MAX_ENTRIES: usize = 512;

#[derive(Debug, Deserialize)]
struct WorkspaceReadFileInput {
    path: String,
    #[serde(default)]
    offset_bytes: u64,
    #[serde(default)]
    max_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceListDirInput {
    #[serde(default)]
    path: String,
    #[serde(default)]
    max_entries: Option<u64>,
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

#[derive(Debug, Serialize)]
struct WorkspaceListDirOutput {
    path: String,
    workspace_root_index: usize,
    entries: Vec<WorkspaceListDirEntry>,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct WorkspaceListDirEntry {
    name: String,
    path: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_bytes: Option<u64>,
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
    let host_root =
        process_runner_allows_host_access(&runtime_state.config.tool_call.process_runner)
            .then_some(runtime_state.config.tool_call.process_runner.workspace_root.as_path());
    let read = match read_workspace_file_from_roots(workspace_roots.as_slice(), &input, host_root) {
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

pub(crate) async fn execute_workspace_list_dir_tool(
    runtime_state: &Arc<GatewayRuntimeState>,
    context: ToolRuntimeExecutionContext<'_>,
    proposal_id: &str,
    input_json: &[u8],
) -> ToolExecutionOutcome {
    let input = match parse_workspace_list_dir_input(input_json) {
        Ok(input) => input,
        Err(error) => {
            return workspace_list_dir_outcome(
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
            return workspace_list_dir_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!(
                    "{WORKSPACE_LIST_DIR_TOOL_NAME} failed to resolve agent workspace: {}",
                    error.message()
                ),
            );
        }
    };

    let workspace_roots =
        agent_outcome.agent.workspace_roots.iter().map(PathBuf::from).collect::<Vec<_>>();
    let host_root =
        process_runner_allows_host_access(&runtime_state.config.tool_call.process_runner)
            .then_some(runtime_state.config.tool_call.process_runner.workspace_root.as_path());
    let listing = match list_workspace_dir_from_roots(workspace_roots.as_slice(), &input, host_root)
    {
        Ok(listing) => listing,
        Err(error) => {
            return workspace_list_dir_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                error,
            );
        }
    };

    match serde_json::to_vec(&listing) {
        Ok(output_json) => {
            workspace_list_dir_outcome(proposal_id, input_json, true, output_json, String::new())
        }
        Err(error) => workspace_list_dir_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!("{WORKSPACE_LIST_DIR_TOOL_NAME} failed to serialize output: {error}"),
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
    input.path = normalize_workspace_path_input(input.path.as_str());
    validate_workspace_path_syntax(input.path.as_str(), WORKSPACE_READ_FILE_TOOL_NAME)?;
    Ok(input)
}

fn parse_workspace_list_dir_input(input_json: &[u8]) -> Result<WorkspaceListDirInput, String> {
    if input_json.len() > MAX_WORKSPACE_LIST_DIR_TOOL_INPUT_BYTES {
        return Err(format!(
            "{WORKSPACE_LIST_DIR_TOOL_NAME} input exceeds {MAX_WORKSPACE_LIST_DIR_TOOL_INPUT_BYTES} bytes"
        ));
    }

    let mut input =
        serde_json::from_slice::<WorkspaceListDirInput>(input_json).map_err(|error| {
            format!(
                "{WORKSPACE_LIST_DIR_TOOL_NAME} input must match directory listing schema: {error}"
            )
        })?;
    if matches!(input.max_entries, Some(0)) {
        return Err(format!("{WORKSPACE_LIST_DIR_TOOL_NAME} max_entries must be >= 1"));
    }
    input.path = normalize_workspace_path_input(input.path.as_str());
    validate_workspace_path_syntax(input.path.as_str(), WORKSPACE_LIST_DIR_TOOL_NAME)?;
    Ok(input)
}

fn normalize_workspace_path_input(path: &str) -> String {
    let normalized = path.trim().replace('\\', "/");
    let without_current = normalized.strip_prefix("./").unwrap_or(normalized.as_str());
    match without_current {
        "." | "/workspace" | "/workspace/" | "workspace" | "workspace/" => String::new(),
        _ => without_current
            .strip_prefix("/workspace/")
            .or_else(|| without_current.strip_prefix("workspace/"))
            .unwrap_or(without_current)
            .to_owned(),
    }
}

fn validate_workspace_path_syntax(path: &str, tool_name: &str) -> Result<(), String> {
    if path.chars().any(char::is_control) {
        return Err(format!("{tool_name} path contains unsupported characters"));
    }
    if path.contains(':') && !looks_like_windows_drive_path(path) {
        return Err(format!("{tool_name} path contains unsupported characters"));
    }
    if path.is_empty() {
        return Ok(());
    }

    let parsed = Path::new(path);
    if parsed.is_absolute() {
        return Ok(());
    }
    if !parsed.components().all(|component| matches!(component, Component::Normal(_))) {
        return Err(format!(
            "{tool_name} path must not contain root, prefix, '.', or '..' components"
        ));
    }
    Ok(())
}

fn looks_like_windows_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

fn read_workspace_file_from_roots(
    workspace_roots: &[PathBuf],
    input: &WorkspaceReadFileInput,
    host_root: Option<&Path>,
) -> Result<WorkspaceReadFileOutput, String> {
    let canonical_roots =
        canonicalize_workspace_roots(workspace_roots, WORKSPACE_READ_FILE_TOOL_NAME)?;
    let canonical_host_root = canonicalize_host_root(host_root, WORKSPACE_READ_FILE_TOOL_NAME)?;
    if canonical_roots.is_empty() && canonical_host_root.is_none() {
        return Err(format!(
            "{WORKSPACE_READ_FILE_TOOL_NAME} agent has no accessible workspace roots"
        ));
    }

    let requested = Path::new(input.path.as_str());
    if requested.is_absolute() {
        if canonical_host_root.is_some() {
            return read_absolute_host_file(requested, input);
        }
        let (workspace_root_index, canonical_target, display_path) =
            resolve_absolute_workspace_file(canonical_roots.as_slice(), requested, input)?;
        return read_workspace_file_chunk(
            workspace_root_index,
            canonical_target,
            display_path,
            input,
        );
    }

    for (workspace_root_index, canonical_root) in &canonical_roots {
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
            return Err(read_file_not_regular_file_error(input.path.as_str()));
        }

        let display_path = canonical_target
            .strip_prefix(canonical_root)
            .map(normalize_relative_path_display)
            .unwrap_or_else(|_| input.path.clone());
        return read_workspace_file_chunk(
            *workspace_root_index,
            canonical_target,
            display_path,
            input,
        );
    }

    if let Some(host_root) = canonical_host_root.as_ref() {
        let candidate = host_root.join(Path::new(input.path.as_str()));
        if let Ok(canonical_target) = fs::canonicalize(candidate.as_path()) {
            if !canonical_target.is_file() {
                return Err(read_file_not_regular_file_error(input.path.as_str()));
            }
            let display_path = canonical_target.to_string_lossy().into_owned();
            return read_workspace_file_chunk(0, canonical_target, display_path, input);
        }
    }

    Err(format!(
        "{WORKSPACE_READ_FILE_TOOL_NAME} file not found in agent workspace roots: {}",
        display_requested_path(input.path.as_str())
    ))
}

fn canonicalize_workspace_roots(
    workspace_roots: &[PathBuf],
    tool_name: &str,
) -> Result<Vec<(usize, PathBuf)>, String> {
    let mut canonical_roots = Vec::with_capacity(workspace_roots.len());
    for (workspace_root_index, workspace_root) in workspace_roots.iter().enumerate() {
        match fs::canonicalize(workspace_root) {
            Ok(path) if path.is_dir() => canonical_roots.push((workspace_root_index, path)),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "{tool_name} failed to resolve workspace root {workspace_root_index}: {error}"
                ));
            }
        }
    }
    Ok(canonical_roots)
}

fn canonicalize_host_root(
    host_root: Option<&Path>,
    tool_name: &str,
) -> Result<Option<PathBuf>, String> {
    let Some(host_root) = host_root else {
        return Ok(None);
    };
    if host_root.as_os_str().is_empty() {
        return std::env::current_dir().map(Some).map_err(|error| {
            format!("{tool_name} failed to resolve host current directory: {error}")
        });
    }
    match fs::canonicalize(host_root) {
        Ok(path) if path.is_dir() => Ok(Some(path)),
        Ok(_) => std::env::current_dir().map(Some).map_err(|error| {
            format!("{tool_name} failed to resolve host current directory: {error}")
        }),
        Err(_) => std::env::current_dir().map(Some).map_err(|error| {
            format!("{tool_name} failed to resolve host current directory: {error}")
        }),
    }
}

fn resolve_absolute_workspace_file(
    canonical_roots: &[(usize, PathBuf)],
    requested: &Path,
    input: &WorkspaceReadFileInput,
) -> Result<(usize, PathBuf, String), String> {
    let canonical_target = fs::canonicalize(requested).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "{WORKSPACE_READ_FILE_TOOL_NAME} file not found in agent workspace roots: {}",
                input.path
            )
        } else {
            format!("{WORKSPACE_READ_FILE_TOOL_NAME} failed to resolve path: {error}")
        }
    })?;
    for (workspace_root_index, canonical_root) in canonical_roots {
        if canonical_target.starts_with(canonical_root) {
            if !canonical_target.is_file() {
                return Err(read_file_not_regular_file_error(input.path.as_str()));
            }
            let display_path = canonical_target
                .strip_prefix(canonical_root)
                .map(normalize_relative_path_display)
                .unwrap_or_else(|_| input.path.clone());
            return Ok((*workspace_root_index, canonical_target, display_path));
        }
    }
    Err(format!("{WORKSPACE_READ_FILE_TOOL_NAME} path escapes agent workspace roots"))
}

fn read_absolute_host_file(
    requested: &Path,
    input: &WorkspaceReadFileInput,
) -> Result<WorkspaceReadFileOutput, String> {
    let canonical_target = fs::canonicalize(requested).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "{WORKSPACE_READ_FILE_TOOL_NAME} host file not found: {}",
                display_requested_path(input.path.as_str())
            )
        } else {
            format!("{WORKSPACE_READ_FILE_TOOL_NAME} failed to resolve host path: {error}")
        }
    })?;
    if !canonical_target.is_file() {
        return Err(read_file_not_regular_file_error(input.path.as_str()));
    }
    read_workspace_file_chunk(
        0,
        canonical_target.clone(),
        canonical_target.to_string_lossy().into_owned(),
        input,
    )
}

fn read_file_not_regular_file_error(path: &str) -> String {
    format!(
        "{WORKSPACE_READ_FILE_TOOL_NAME} target is not a regular file: {}; use {WORKSPACE_LIST_DIR_TOOL_NAME} to inspect workspace directories",
        display_requested_path(path)
    )
}

fn list_workspace_dir_from_roots(
    workspace_roots: &[PathBuf],
    input: &WorkspaceListDirInput,
    host_root: Option<&Path>,
) -> Result<WorkspaceListDirOutput, String> {
    let canonical_roots =
        canonicalize_workspace_roots(workspace_roots, WORKSPACE_LIST_DIR_TOOL_NAME)?;
    let canonical_host_root = canonicalize_host_root(host_root, WORKSPACE_LIST_DIR_TOOL_NAME)?;
    if canonical_roots.is_empty() && canonical_host_root.is_none() {
        return Err(format!(
            "{WORKSPACE_LIST_DIR_TOOL_NAME} agent has no accessible workspace roots"
        ));
    }

    let requested = Path::new(input.path.as_str());
    if requested.is_absolute() {
        if canonical_host_root.is_some() {
            return list_absolute_host_dir(requested, input);
        }
        let (workspace_root_index, canonical_target, display_path) =
            resolve_absolute_workspace_dir(canonical_roots.as_slice(), requested, input)?;
        return list_workspace_directory(
            workspace_root_index,
            canonical_roots
                .iter()
                .find_map(|(index, root)| {
                    (*index == workspace_root_index).then_some(root.as_path())
                })
                .ok_or_else(|| {
                    format!(
                        "{WORKSPACE_LIST_DIR_TOOL_NAME} failed to resolve workspace root for directory listing"
                    )
                })?,
            canonical_target,
            display_path,
            input,
        );
    }

    for (workspace_root_index, canonical_root) in &canonical_roots {
        let candidate = canonical_root.join(Path::new(input.path.as_str()));
        let canonical_target = match fs::canonicalize(&candidate) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "{WORKSPACE_LIST_DIR_TOOL_NAME} failed to resolve path in workspace root {workspace_root_index}: {error}"
                ));
            }
        };
        if !canonical_target.starts_with(canonical_root.as_path()) {
            return Err(format!(
                "{WORKSPACE_LIST_DIR_TOOL_NAME} path escapes agent workspace roots"
            ));
        }
        if !canonical_target.is_dir() {
            return Err(format!(
                "{WORKSPACE_LIST_DIR_TOOL_NAME} target is not a directory: {}",
                display_requested_path(input.path.as_str())
            ));
        }

        let display_path = canonical_target
            .strip_prefix(canonical_root)
            .map(normalize_relative_path_display)
            .unwrap_or_else(|_| display_requested_path(input.path.as_str()).to_owned());
        return list_workspace_directory(
            *workspace_root_index,
            canonical_root.as_path(),
            canonical_target,
            display_path,
            input,
        );
    }

    if let Some(host_root) = canonical_host_root.as_ref() {
        let candidate = host_root.join(Path::new(input.path.as_str()));
        if let Ok(canonical_target) = fs::canonicalize(candidate.as_path()) {
            if !canonical_target.is_dir() {
                return Err(format!(
                    "{WORKSPACE_LIST_DIR_TOOL_NAME} target is not a directory: {}",
                    display_requested_path(input.path.as_str())
                ));
            }
            let display_path = canonical_target.to_string_lossy().into_owned();
            return list_workspace_directory(
                0,
                host_root.as_path(),
                canonical_target,
                display_path,
                input,
            );
        }
    }

    Err(format!(
        "{WORKSPACE_LIST_DIR_TOOL_NAME} directory not found in agent workspace roots: {}",
        display_requested_path(input.path.as_str())
    ))
}

fn list_absolute_host_dir(
    requested: &Path,
    input: &WorkspaceListDirInput,
) -> Result<WorkspaceListDirOutput, String> {
    let canonical_target = fs::canonicalize(requested).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "{WORKSPACE_LIST_DIR_TOOL_NAME} host directory not found: {}",
                display_requested_path(input.path.as_str())
            )
        } else {
            format!("{WORKSPACE_LIST_DIR_TOOL_NAME} failed to resolve host path: {error}")
        }
    })?;
    if !canonical_target.is_dir() {
        return Err(format!(
            "{WORKSPACE_LIST_DIR_TOOL_NAME} target is not a directory: {}",
            display_requested_path(input.path.as_str())
        ));
    }
    let parent_root = canonical_target.parent().unwrap_or(canonical_target.as_path());
    list_workspace_directory(
        0,
        parent_root,
        canonical_target.clone(),
        canonical_target.to_string_lossy().into_owned(),
        input,
    )
}

fn resolve_absolute_workspace_dir(
    canonical_roots: &[(usize, PathBuf)],
    requested: &Path,
    input: &WorkspaceListDirInput,
) -> Result<(usize, PathBuf, String), String> {
    let canonical_target = fs::canonicalize(requested).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "{WORKSPACE_LIST_DIR_TOOL_NAME} directory not found in agent workspace roots: {}",
                display_requested_path(input.path.as_str())
            )
        } else {
            format!("{WORKSPACE_LIST_DIR_TOOL_NAME} failed to resolve path: {error}")
        }
    })?;
    for (workspace_root_index, canonical_root) in canonical_roots {
        if canonical_target.starts_with(canonical_root) {
            if !canonical_target.is_dir() {
                return Err(format!(
                    "{WORKSPACE_LIST_DIR_TOOL_NAME} target is not a directory: {}",
                    display_requested_path(input.path.as_str())
                ));
            }
            let display_path = canonical_target
                .strip_prefix(canonical_root)
                .map(normalize_relative_path_display)
                .unwrap_or_else(|_| display_requested_path(input.path.as_str()).to_owned());
            return Ok((*workspace_root_index, canonical_target, display_path));
        }
    }
    Err(format!("{WORKSPACE_LIST_DIR_TOOL_NAME} path escapes agent workspace roots"))
}

fn list_workspace_directory(
    workspace_root_index: usize,
    canonical_root: &Path,
    path: PathBuf,
    display_path: String,
    input: &WorkspaceListDirInput,
) -> Result<WorkspaceListDirOutput, String> {
    let max_entries = input
        .max_entries
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(WORKSPACE_LIST_DIR_DEFAULT_ENTRIES)
        .min(WORKSPACE_LIST_DIR_MAX_ENTRIES);
    let mut entries = Vec::new();
    for entry_result in fs::read_dir(path.as_path()).map_err(|error| {
        format!(
            "{WORKSPACE_LIST_DIR_TOOL_NAME} failed to read workspace directory {}: {error}",
            display_requested_path(input.path.as_str())
        )
    })? {
        let entry = entry_result.map_err(|error| {
            format!(
                "{WORKSPACE_LIST_DIR_TOOL_NAME} failed to read directory entry for {}: {error}",
                display_requested_path(input.path.as_str())
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "{WORKSPACE_LIST_DIR_TOOL_NAME} failed to inspect directory entry for {}: {error}",
                display_requested_path(input.path.as_str())
            )
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let raw_entry_path = entry.path();
        let path = raw_entry_path
            .strip_prefix(canonical_root)
            .map(normalize_relative_path_display)
            .unwrap_or_else(|_| {
                if display_path == "." {
                    name.clone()
                } else {
                    format!("{display_path}/{name}")
                }
            });
        let size_bytes = if file_type.is_file() {
            entry.metadata().ok().map(|metadata| metadata.len())
        } else {
            None
        };
        let kind = if file_type.is_dir() {
            "directory"
        } else if file_type.is_file() {
            "file"
        } else if file_type.is_symlink() {
            "symlink"
        } else {
            "other"
        };
        entries.push(WorkspaceListDirEntry { name, path, kind: kind.to_owned(), size_bytes });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    let truncated = entries.len() > max_entries;
    entries.truncate(max_entries);

    Ok(WorkspaceListDirOutput { path: display_path, workspace_root_index, entries, truncated })
}

fn read_workspace_file_chunk(
    workspace_root_index: usize,
    path: PathBuf,
    display_path: String,
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
        path: display_path,
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

fn normalize_relative_path_display(path: &Path) -> String {
    let mut rendered = Vec::new();
    for component in path.components() {
        if let Component::Normal(value) = component {
            rendered.push(value.to_string_lossy().into_owned());
        }
    }
    if rendered.is_empty() {
        ".".to_owned()
    } else {
        rendered.join("/")
    }
}

fn display_requested_path(path: &str) -> &str {
    if path.is_empty() {
        "."
    } else {
        path
    }
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

fn workspace_list_dir_outcome(
    proposal_id: &str,
    input_json: &[u8],
    success: bool,
    output_json: Vec<u8>,
    error: String,
) -> ToolExecutionOutcome {
    build_tool_execution_outcome(
        proposal_id,
        WORKSPACE_LIST_DIR_TOOL_NAME,
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

        let output = read_workspace_file_from_roots(&[tempdir.path().to_path_buf()], &input, None)
            .expect("workspace file should be readable");

        assert_eq!(output.text.as_deref(), Some(contents));
        assert_eq!(output.path, "agent-e2e-tool-test.js");
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

        let output = read_workspace_file_from_roots(&[tempdir.path().to_path_buf()], &input, None)
            .expect("workspace file chunk should be readable");

        assert_eq!(output.text.as_deref(), Some("cde"));
        assert_eq!(output.returned_bytes, 3);
        assert!(!output.eof);
    }

    #[test]
    fn read_workspace_file_accepts_absolute_path_inside_root() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let workspace = tempdir.path().join("workspace");
        fs::create_dir_all(workspace.join("nested")).expect("workspace should exist");
        let file_path = workspace.join("nested").join("calc.js");
        fs::write(&file_path, "export const add = (a, b) => a + b;\n")
            .expect("workspace file should be written");
        let input = WorkspaceReadFileInput {
            path: file_path.to_string_lossy().into_owned(),
            offset_bytes: 0,
            max_bytes: None,
        };

        let output = read_workspace_file_from_roots(&[workspace], &input, None)
            .expect("absolute workspace file should be readable");

        assert_eq!(output.path, "nested/calc.js");
        assert_eq!(output.text.as_deref(), Some("export const add = (a, b) => a + b;\n"));
    }

    #[test]
    fn read_workspace_file_accepts_workspace_virtual_absolute_alias() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::create_dir_all(tempdir.path().join("nested")).expect("nested dir should exist");
        fs::write(tempdir.path().join("nested").join("calc.js"), "export const answer = 42;\n")
            .expect("workspace file should be written");
        let mut input = parse_workspace_read_file_input(
            br#"{"path":"/workspace/nested/calc.js","offset_bytes":0}"#,
        )
        .expect("virtual workspace path should parse");

        assert_eq!(input.path, "nested/calc.js");
        input.max_bytes = None;
        let output = read_workspace_file_from_roots(&[tempdir.path().to_path_buf()], &input, None)
            .expect("virtual workspace path should be readable");

        assert_eq!(output.path, "nested/calc.js");
        assert_eq!(output.text.as_deref(), Some("export const answer = 42;\n"));
    }

    #[test]
    fn read_workspace_file_accepts_workspace_prefix_alias() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::create_dir_all(tempdir.path().join("scenarios")).expect("scenarios dir should exist");
        fs::write(tempdir.path().join("scenarios").join("app.js"), "console.log('ok');\n")
            .expect("workspace file should be written");
        let input = parse_workspace_read_file_input(br#"{"path":"workspace/scenarios/app.js"}"#)
            .expect("workspace alias path should parse");

        let output = read_workspace_file_from_roots(&[tempdir.path().to_path_buf()], &input, None)
            .expect("workspace alias path should be readable");

        assert_eq!(output.path, "scenarios/app.js");
        assert_eq!(output.text.as_deref(), Some("console.log('ok');\n"));
    }

    #[test]
    fn read_workspace_file_rejects_absolute_path_outside_root() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let workspace = tempdir.path().join("workspace");
        let outside = tempdir.path().join("outside");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::create_dir_all(&outside).expect("outside directory should exist");
        let outside_file = outside.join("secret.txt");
        fs::write(&outside_file, "secret").expect("outside file should be written");
        let input = WorkspaceReadFileInput {
            path: outside_file.to_string_lossy().into_owned(),
            offset_bytes: 0,
            max_bytes: None,
        };

        let error = read_workspace_file_from_roots(&[workspace], &input, None)
            .expect_err("outside absolute path should be rejected");

        assert!(error.contains("escapes agent workspace roots"), "unexpected error: {error}");
    }

    #[test]
    fn read_workspace_file_allows_absolute_host_path_when_host_access_enabled() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let workspace = tempdir.path().join("workspace");
        let outside = tempdir.path().join("outside");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::create_dir_all(&outside).expect("outside directory should exist");
        let outside_file = outside.join("notes.txt");
        fs::write(&outside_file, "host note\n").expect("outside file should be written");
        let input = WorkspaceReadFileInput {
            path: outside_file.to_string_lossy().into_owned(),
            offset_bytes: 0,
            max_bytes: None,
        };

        let output = read_workspace_file_from_roots(&[workspace], &input, Some(tempdir.path()))
            .expect("host access should allow absolute file reads outside workspace roots");

        assert_eq!(output.text.as_deref(), Some("host note\n"));
        assert_eq!(
            output.path,
            outside_file.canonicalize().expect("file should resolve").display().to_string()
        );
    }

    #[test]
    fn read_workspace_file_rejects_parent_traversal() {
        let error =
            parse_workspace_read_file_input(br#"{"path":"../outside.txt"}"#).expect_err("path");

        assert!(error.contains("must not contain"), "unexpected validation error: {error}");
    }

    #[test]
    fn read_workspace_file_directory_error_points_to_list_dir() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::create_dir_all(tempdir.path().join("scenarios")).expect("scenarios dir should exist");
        let input = parse_workspace_read_file_input(br#"{"path":"workspace/scenarios"}"#)
            .expect("workspace alias directory should parse");

        let error = read_workspace_file_from_roots(&[tempdir.path().to_path_buf()], &input, None)
            .expect_err("directory read should fail");

        assert!(error.contains(WORKSPACE_LIST_DIR_TOOL_NAME), "unexpected error: {error}");
    }

    #[test]
    fn list_workspace_dir_returns_sorted_entries_for_workspace_alias() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::create_dir_all(tempdir.path().join("scenarios").join("nested"))
            .expect("nested dir should exist");
        fs::write(tempdir.path().join("scenarios").join("b.txt"), "bravo")
            .expect("workspace file should be written");
        fs::write(tempdir.path().join("scenarios").join("a.txt"), "alpha")
            .expect("workspace file should be written");
        let input =
            parse_workspace_list_dir_input(br#"{"path":"/workspace/scenarios","max_entries":10}"#)
                .expect("workspace alias list input should parse");

        let output = list_workspace_dir_from_roots(&[tempdir.path().to_path_buf()], &input, None)
            .expect("workspace directory should be listed");

        assert_eq!(output.path, "scenarios");
        assert_eq!(output.workspace_root_index, 0);
        assert!(!output.truncated);
        assert_eq!(
            output.entries.iter().map(|entry| entry.path.as_str()).collect::<Vec<_>>(),
            vec!["scenarios/a.txt", "scenarios/b.txt", "scenarios/nested"]
        );
        assert_eq!(output.entries[0].kind, "file");
        assert_eq!(output.entries[0].size_bytes, Some(5));
        assert_eq!(output.entries[2].kind, "directory");
        assert_eq!(output.entries[2].size_bytes, None);
    }

    #[test]
    fn list_workspace_dir_allows_absolute_host_path_when_host_access_enabled() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let workspace = tempdir.path().join("workspace");
        let outside = tempdir.path().join("outside");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::create_dir_all(&outside).expect("outside directory should exist");
        fs::write(outside.join("notes.txt"), "host note\n")
            .expect("outside file should be written");
        let input = WorkspaceListDirInput {
            path: outside.to_string_lossy().into_owned(),
            max_entries: None,
        };

        let output = list_workspace_dir_from_roots(&[workspace], &input, Some(tempdir.path()))
            .expect("host access should allow absolute directory listings outside workspace roots");

        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].name, "notes.txt");
        assert_eq!(output.entries[0].kind, "file");
    }

    #[test]
    fn list_workspace_dir_rejects_parent_traversal() {
        let error = parse_workspace_list_dir_input(br#"{"path":"../outside"}"#).expect_err("path");

        assert!(error.contains("must not contain"), "unexpected validation error: {error}");
    }
}
