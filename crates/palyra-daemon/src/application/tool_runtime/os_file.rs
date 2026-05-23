use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::UNIX_EPOCH,
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use palyra_safety::{
    redact_text_for_export, SafetyContentKind, SafetyFindingCategory, SafetySourceKind, TrustLabel,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    agents::AgentResolveRequest,
    gateway::{GatewayRuntimeState, ToolRuntimeExecutionContext, OS_FILE_TOOL_NAME},
    tool_protocol::{build_tool_execution_outcome, ToolExecutionOutcome},
};

const MAX_OS_FILE_READ_BYTES: u64 = 128 * 1024;
const MAX_OS_FILE_TOOL_INPUT_BYTES: usize = 384 * 1024;
const MAX_OS_FILE_WRITE_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize)]
struct OsFileInput {
    operation: OsFileOperation,
    path: String,
    #[serde(default)]
    target_path: Option<String>,
    #[serde(default)]
    content_text: Option<String>,
    #[serde(default)]
    bytes_base64: Option<String>,
    #[serde(default)]
    create_parent_dirs: Option<bool>,
    #[serde(default)]
    overwrite: Option<bool>,
    #[serde(default)]
    dry_run: Option<bool>,
    #[serde(default)]
    offset_bytes: Option<u64>,
    #[serde(default)]
    max_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum OsFileOperation {
    Stat,
    Read,
    Write,
    Copy,
    Move,
    DeleteFile,
    Mkdir,
}

#[derive(Debug, Clone)]
struct OsFilePolicy {
    workspace_roots: Vec<PathBuf>,
    user_os_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct ResolvedOsPath {
    requested_path: PathBuf,
    resolved_path: PathBuf,
    existed: bool,
}

pub(crate) async fn execute_os_file_tool(
    runtime_state: &Arc<GatewayRuntimeState>,
    context: ToolRuntimeExecutionContext<'_>,
    proposal_id: &str,
    input_json: &[u8],
) -> ToolExecutionOutcome {
    if input_json.len() > MAX_OS_FILE_TOOL_INPUT_BYTES {
        return os_file_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!("{OS_FILE_TOOL_NAME} input exceeds {MAX_OS_FILE_TOOL_INPUT_BYTES} bytes"),
        );
    }
    let input = match serde_json::from_slice::<OsFileInput>(input_json) {
        Ok(input) => input,
        Err(error) => {
            return os_file_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!("{OS_FILE_TOOL_NAME} input must match OS file schema: {error}"),
            );
        }
    };
    let policy = match resolve_os_file_policy(runtime_state, context).await {
        Ok(policy) => policy,
        Err(error) => {
            return os_file_outcome(proposal_id, input_json, false, b"{}".to_vec(), error);
        }
    };
    let output = match execute_os_file_operation(&policy, &input) {
        Ok(output) => output,
        Err(error) => {
            return os_file_outcome(proposal_id, input_json, false, b"{}".to_vec(), error);
        }
    };
    match serde_json::to_vec(&output) {
        Ok(output_json) => {
            os_file_outcome(proposal_id, input_json, true, output_json, String::new())
        }
        Err(error) => os_file_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!("{OS_FILE_TOOL_NAME} failed to serialize output: {error}"),
        ),
    }
}

async fn resolve_os_file_policy(
    runtime_state: &Arc<GatewayRuntimeState>,
    context: ToolRuntimeExecutionContext<'_>,
) -> Result<OsFilePolicy, String> {
    let agent_outcome = runtime_state
        .resolve_agent_for_context(AgentResolveRequest {
            principal: context.principal.to_owned(),
            channel: context.channel.map(str::to_owned),
            session_id: Some(context.session_id.to_owned()),
            preferred_agent_id: None,
            persist_session_binding: false,
        })
        .await
        .map_err(|error| {
            format!(
                "{OS_FILE_TOOL_NAME} failed to resolve agent OS file policy: {}",
                error.message()
            )
        })?;
    let workspace_roots = agent_outcome
        .agent
        .workspace_roots
        .iter()
        .filter_map(|root| canonicalize_existing_dir(Path::new(root)).ok())
        .collect::<Vec<_>>();
    let user_os_roots = user_owned_os_roots();
    Ok(OsFilePolicy { workspace_roots, user_os_roots })
}

fn execute_os_file_operation(policy: &OsFilePolicy, input: &OsFileInput) -> Result<Value, String> {
    match input.operation {
        OsFileOperation::Stat => stat_path(policy, input),
        OsFileOperation::Read => read_path(policy, input),
        OsFileOperation::Write => write_path(policy, input),
        OsFileOperation::Copy => copy_path(policy, input),
        OsFileOperation::Move => move_path(policy, input),
        OsFileOperation::DeleteFile => delete_file_path(policy, input),
        OsFileOperation::Mkdir => mkdir_path(policy, input),
    }
}

fn stat_path(policy: &OsFilePolicy, input: &OsFileInput) -> Result<Value, String> {
    let path = resolve_existing_os_path(input.path.as_str())?;
    ensure_os_path_allowed(policy, &path)?;
    let metadata = fs::metadata(path.resolved_path.as_path()).map_err(|error| {
        format!("{OS_FILE_TOOL_NAME} failed to inspect {}: {error}", input.path.trim())
    })?;
    Ok(json!({
        "operation": "stat",
        "path": display_path(path.requested_path.as_path()),
        "resolved_path": display_path(path.resolved_path.as_path()),
        "kind": metadata_kind(&metadata),
        "size_bytes": metadata.len(),
        "readonly": metadata.permissions().readonly(),
        "modified_unix_ms": metadata_modified_unix_ms(&metadata),
        "dry_run": false,
    }))
}

fn read_path(policy: &OsFilePolicy, input: &OsFileInput) -> Result<Value, String> {
    let path = resolve_existing_os_path(input.path.as_str())?;
    ensure_os_path_allowed(policy, &path)?;
    let mut file = File::open(path.resolved_path.as_path()).map_err(|error| {
        format!("{OS_FILE_TOOL_NAME} failed to open {}: {error}", input.path.trim())
    })?;
    let size_bytes = file
        .metadata()
        .map_err(|error| {
            format!("{OS_FILE_TOOL_NAME} failed to inspect {}: {error}", input.path.trim())
        })?
        .len();
    let offset_bytes = input.offset_bytes.unwrap_or(0);
    file.seek(SeekFrom::Start(offset_bytes)).map_err(|error| {
        format!("{OS_FILE_TOOL_NAME} failed to seek {}: {error}", input.path.trim())
    })?;
    let requested_max = input.max_bytes.unwrap_or(MAX_OS_FILE_READ_BYTES);
    let read_limit = requested_max.min(MAX_OS_FILE_READ_BYTES);
    let mut buffer = Vec::with_capacity(usize::try_from(read_limit.min(8192)).unwrap_or(8192));
    file.take(read_limit).read_to_end(&mut buffer).map_err(|error| {
        format!("{OS_FILE_TOOL_NAME} failed to read {}: {error}", input.path.trim())
    })?;
    let returned_bytes = u64::try_from(buffer.len()).expect("OS file read size must fit u64");
    let eof = offset_bytes.saturating_add(returned_bytes) >= size_bytes;
    let chunk_sha256 = hex::encode(Sha256::digest(buffer.as_slice()));
    let (text, bytes_base64, redacted) = visible_file_content(buffer);
    Ok(json!({
        "operation": "read",
        "path": display_path(path.requested_path.as_path()),
        "resolved_path": display_path(path.resolved_path.as_path()),
        "offset_bytes": offset_bytes,
        "returned_bytes": returned_bytes,
        "size_bytes": size_bytes,
        "eof": eof,
        "chunk_sha256": chunk_sha256,
        "text": text,
        "bytes_base64": bytes_base64,
        "redacted": redacted,
        "dry_run": false,
    }))
}

fn write_path(policy: &OsFilePolicy, input: &OsFileInput) -> Result<Value, String> {
    let bytes = input_write_bytes(input)?;
    let path = resolve_target_os_path(input.path.as_str())?;
    ensure_os_path_allowed(policy, &path)?;
    let dry_run = input.dry_run.unwrap_or(false);
    let create_parent_dirs = input.create_parent_dirs.unwrap_or(true);
    let overwrite = input.overwrite.unwrap_or(true);
    let parent = path.resolved_path.parent().ok_or_else(|| {
        format!("{OS_FILE_TOOL_NAME} write target has no parent: {}", input.path.trim())
    })?;
    let existed_before = path.resolved_path.exists();
    let parent_existed_before = parent.exists();
    if existed_before && !overwrite {
        return Err(format!(
            "{OS_FILE_TOOL_NAME} refusing to overwrite existing file {}",
            input.path.trim()
        ));
    }
    if !parent_existed_before && !create_parent_dirs {
        return Err(format!(
            "{OS_FILE_TOOL_NAME} parent directory does not exist for {}",
            input.path.trim()
        ));
    }
    let content_sha256 = hex::encode(Sha256::digest(bytes.as_slice()));
    if !dry_run {
        if create_parent_dirs {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "{OS_FILE_TOOL_NAME} failed to create parent directories for {}: {error}",
                    input.path.trim()
                )
            })?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path.resolved_path.as_path())
            .map_err(|error| {
                format!(
                    "{OS_FILE_TOOL_NAME} failed to open write target {}: {error}",
                    input.path.trim()
                )
            })?;
        file.write_all(bytes.as_slice()).map_err(|error| {
            format!("{OS_FILE_TOOL_NAME} failed to write {}: {error}", input.path.trim())
        })?;
        file.sync_all().map_err(|error| {
            format!("{OS_FILE_TOOL_NAME} failed to sync {}: {error}", input.path.trim())
        })?;
    }
    Ok(json!({
        "operation": "write",
        "path": display_path(path.requested_path.as_path()),
        "resolved_path": display_path(path.resolved_path.as_path()),
        "bytes_written": bytes.len(),
        "content_sha256": content_sha256,
        "existed_before": existed_before,
        "created_parent_dirs": create_parent_dirs && !parent_existed_before,
        "dry_run": dry_run,
    }))
}

fn copy_path(policy: &OsFilePolicy, input: &OsFileInput) -> Result<Value, String> {
    let source = resolve_existing_os_path(input.path.as_str())?;
    let target = resolve_target_os_path(required_target_path(input)?)?;
    ensure_os_path_allowed(policy, &source)?;
    ensure_os_path_allowed(policy, &target)?;
    let dry_run = input.dry_run.unwrap_or(false);
    let create_parent_dirs = input.create_parent_dirs.unwrap_or(true);
    let overwrite = input.overwrite.unwrap_or(true);
    prepare_target_parent(&target, create_parent_dirs, overwrite, dry_run, "copy")?;
    let source_size = fs::metadata(source.resolved_path.as_path())
        .map_err(|error| format!("{OS_FILE_TOOL_NAME} failed to inspect source: {error}"))?
        .len();
    if !dry_run {
        fs::copy(source.resolved_path.as_path(), target.resolved_path.as_path()).map_err(
            |error| format!("{OS_FILE_TOOL_NAME} failed to copy {}: {error}", input.path.trim()),
        )?;
    }
    Ok(json!({
        "operation": "copy",
        "path": display_path(source.requested_path.as_path()),
        "resolved_path": display_path(source.resolved_path.as_path()),
        "target_path": display_path(target.requested_path.as_path()),
        "resolved_target_path": display_path(target.resolved_path.as_path()),
        "source_size_bytes": source_size,
        "target_existed_before": target.existed,
        "dry_run": dry_run,
    }))
}

fn move_path(policy: &OsFilePolicy, input: &OsFileInput) -> Result<Value, String> {
    let source = resolve_existing_os_path(input.path.as_str())?;
    let target = resolve_target_os_path(required_target_path(input)?)?;
    ensure_os_path_allowed(policy, &source)?;
    ensure_os_path_allowed(policy, &target)?;
    let dry_run = input.dry_run.unwrap_or(false);
    let create_parent_dirs = input.create_parent_dirs.unwrap_or(true);
    let overwrite = input.overwrite.unwrap_or(true);
    prepare_target_parent(&target, create_parent_dirs, overwrite, dry_run, "move")?;
    let source_size = fs::metadata(source.resolved_path.as_path())
        .map_err(|error| format!("{OS_FILE_TOOL_NAME} failed to inspect source: {error}"))?
        .len();
    if !dry_run {
        if target.existed {
            fs::remove_file(target.resolved_path.as_path()).map_err(|error| {
                format!("{OS_FILE_TOOL_NAME} failed to replace target before move: {error}")
            })?;
        }
        fs::rename(source.resolved_path.as_path(), target.resolved_path.as_path()).map_err(
            |error| format!("{OS_FILE_TOOL_NAME} failed to move {}: {error}", input.path.trim()),
        )?;
    }
    Ok(json!({
        "operation": "move",
        "path": display_path(source.requested_path.as_path()),
        "resolved_path": display_path(source.resolved_path.as_path()),
        "target_path": display_path(target.requested_path.as_path()),
        "resolved_target_path": display_path(target.resolved_path.as_path()),
        "source_size_bytes": source_size,
        "target_existed_before": target.existed,
        "dry_run": dry_run,
    }))
}

fn delete_file_path(policy: &OsFilePolicy, input: &OsFileInput) -> Result<Value, String> {
    let path = resolve_existing_os_path(input.path.as_str())?;
    ensure_os_path_allowed(policy, &path)?;
    let metadata = fs::metadata(path.resolved_path.as_path()).map_err(|error| {
        format!("{OS_FILE_TOOL_NAME} failed to inspect {}: {error}", input.path.trim())
    })?;
    if !metadata.is_file() {
        return Err(format!("{OS_FILE_TOOL_NAME} delete_file only removes regular files"));
    }
    let dry_run = input.dry_run.unwrap_or(false);
    if !dry_run {
        fs::remove_file(path.resolved_path.as_path()).map_err(|error| {
            format!("{OS_FILE_TOOL_NAME} failed to delete {}: {error}", input.path.trim())
        })?;
    }
    Ok(json!({
        "operation": "delete_file",
        "path": display_path(path.requested_path.as_path()),
        "resolved_path": display_path(path.resolved_path.as_path()),
        "size_bytes": metadata.len(),
        "dry_run": dry_run,
    }))
}

fn mkdir_path(policy: &OsFilePolicy, input: &OsFileInput) -> Result<Value, String> {
    let path = resolve_target_os_path(input.path.as_str())?;
    ensure_os_path_allowed(policy, &path)?;
    let dry_run = input.dry_run.unwrap_or(false);
    if !dry_run {
        fs::create_dir_all(path.resolved_path.as_path()).map_err(|error| {
            format!("{OS_FILE_TOOL_NAME} failed to create directory {}: {error}", input.path.trim())
        })?;
    }
    Ok(json!({
        "operation": "mkdir",
        "path": display_path(path.requested_path.as_path()),
        "resolved_path": display_path(path.resolved_path.as_path()),
        "existed_before": path.existed,
        "dry_run": dry_run,
    }))
}

fn prepare_target_parent(
    target: &ResolvedOsPath,
    create_parent_dirs: bool,
    overwrite: bool,
    dry_run: bool,
    operation: &str,
) -> Result<(), String> {
    if target.existed && !overwrite {
        return Err(format!(
            "{OS_FILE_TOOL_NAME} refusing to overwrite existing {operation} target"
        ));
    }
    let parent = target
        .resolved_path
        .parent()
        .ok_or_else(|| format!("{OS_FILE_TOOL_NAME} {operation} target has no parent directory"))?;
    if !parent.exists() && !create_parent_dirs {
        return Err(format!("{OS_FILE_TOOL_NAME} {operation} target parent does not exist"));
    }
    if !dry_run && create_parent_dirs {
        fs::create_dir_all(parent).map_err(|error| {
            format!("{OS_FILE_TOOL_NAME} failed to create {operation} target parent: {error}")
        })?;
    }
    Ok(())
}

fn input_write_bytes(input: &OsFileInput) -> Result<Vec<u8>, String> {
    match (input.content_text.as_ref(), input.bytes_base64.as_ref()) {
        (Some(_), Some(_)) => Err(format!(
            "{OS_FILE_TOOL_NAME} write accepts either content_text or bytes_base64, not both"
        )),
        (Some(text), None) => {
            if text.len() > MAX_OS_FILE_WRITE_BYTES {
                return Err(format!(
                    "{OS_FILE_TOOL_NAME} content_text exceeds {MAX_OS_FILE_WRITE_BYTES} bytes"
                ));
            }
            Ok(text.as_bytes().to_vec())
        }
        (None, Some(bytes_base64)) => {
            let decoded = BASE64_STANDARD.decode(bytes_base64.as_bytes()).map_err(|error| {
                format!("{OS_FILE_TOOL_NAME} bytes_base64 must be valid base64: {error}")
            })?;
            if decoded.len() > MAX_OS_FILE_WRITE_BYTES {
                return Err(format!(
                    "{OS_FILE_TOOL_NAME} bytes_base64 decoded payload exceeds {MAX_OS_FILE_WRITE_BYTES} bytes"
                ));
            }
            Ok(decoded)
        }
        (None, None) => {
            Err(format!("{OS_FILE_TOOL_NAME} write requires content_text or bytes_base64"))
        }
    }
}

fn visible_file_content(buffer: Vec<u8>) -> (Option<String>, Option<String>, bool) {
    match String::from_utf8(buffer) {
        Ok(text) => {
            let redaction = redact_text_for_export(
                text.as_str(),
                SafetySourceKind::Workspace,
                SafetyContentKind::WorkspaceDocument,
                TrustLabel::TrustedLocal,
            );
            let redacted = redaction.scan.has_category(SafetyFindingCategory::SecretLeak);
            let visible_text = if redacted { redaction.redacted_text } else { text };
            (Some(visible_text), None, redacted)
        }
        Err(error) => (None, Some(BASE64_STANDARD.encode(error.into_bytes())), false),
    }
}

fn required_target_path(input: &OsFileInput) -> Result<&str, String> {
    input
        .target_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{OS_FILE_TOOL_NAME} operation requires non-empty target_path"))
}

fn resolve_existing_os_path(path: &str) -> Result<ResolvedOsPath, String> {
    let requested_path = parse_absolute_os_path(path)?;
    let resolved_path = fs::canonicalize(requested_path.as_path()).map_err(|error| {
        format!("{OS_FILE_TOOL_NAME} path does not resolve to an existing OS file target: {error}")
    })?;
    Ok(ResolvedOsPath { requested_path, resolved_path, existed: true })
}

fn resolve_target_os_path(path: &str) -> Result<ResolvedOsPath, String> {
    let requested_path = parse_absolute_os_path(path)?;
    if requested_path.exists() {
        let resolved_path = fs::canonicalize(requested_path.as_path()).map_err(|error| {
            format!("{OS_FILE_TOOL_NAME} failed to resolve existing target: {error}")
        })?;
        return Ok(ResolvedOsPath { requested_path, resolved_path, existed: true });
    }
    let (existing_ancestor, missing_suffix) = nearest_existing_ancestor(requested_path.as_path())?;
    let canonical_ancestor = fs::canonicalize(existing_ancestor.as_path()).map_err(|error| {
        format!("{OS_FILE_TOOL_NAME} failed to resolve target ancestor: {error}")
    })?;
    let resolved_path = canonical_ancestor.join(missing_suffix);
    Ok(ResolvedOsPath { requested_path, resolved_path, existed: false })
}

fn parse_absolute_os_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(format!("{OS_FILE_TOOL_NAME} path must be non-empty"));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(format!("{OS_FILE_TOOL_NAME} path contains unsupported characters"));
    }
    let parsed = PathBuf::from(trimmed);
    if !parsed.is_absolute() {
        return Err(format!("{OS_FILE_TOOL_NAME} path must be an absolute OS path"));
    }
    for component in parsed.components() {
        if matches!(component, Component::CurDir | Component::ParentDir) {
            return Err(format!(
                "{OS_FILE_TOOL_NAME} path must not contain '.' or '..' components"
            ));
        }
    }
    Ok(parsed)
}

fn nearest_existing_ancestor(path: &Path) -> Result<(PathBuf, PathBuf), String> {
    let mut cursor = path.to_path_buf();
    while !cursor.exists() {
        if !cursor.pop() {
            return Err(format!("{OS_FILE_TOOL_NAME} target path has no existing ancestor"));
        }
    }
    if !cursor.is_dir() {
        let Some(parent) = cursor.parent() else {
            return Err(format!("{OS_FILE_TOOL_NAME} target ancestor has no parent directory"));
        };
        cursor = parent.to_path_buf();
    }
    let suffix = path.strip_prefix(cursor.as_path()).map_err(|_| {
        format!("{OS_FILE_TOOL_NAME} failed to resolve target path relative to existing ancestor")
    })?;
    Ok((cursor, suffix.to_path_buf()))
}

fn ensure_os_path_allowed(policy: &OsFilePolicy, path: &ResolvedOsPath) -> Result<(), String> {
    if protected_os_path(path.resolved_path.as_path()) {
        return Err(format!(
            "{OS_FILE_TOOL_NAME} denied protected OS path {}",
            display_path(path.resolved_path.as_path())
        ));
    }
    if policy
        .workspace_roots
        .iter()
        .any(|root| path_starts_with(path.resolved_path.as_path(), root.as_path()))
    {
        return Ok(());
    }
    if policy
        .user_os_roots
        .iter()
        .any(|root| path_starts_with(path.resolved_path.as_path(), root.as_path()))
    {
        return Ok(());
    }
    Err(format!(
        "{OS_FILE_TOOL_NAME} path {} is outside agent workspace roots and approved user-owned OS roots",
        display_path(path.resolved_path.as_path())
    ))
}

fn user_owned_os_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for key in ["USERPROFILE", "HOME"] {
        if let Some(value) = std::env::var_os(key) {
            push_canonical_root(&mut roots, PathBuf::from(value));
        }
    }
    push_canonical_root(&mut roots, std::env::temp_dir());
    #[cfg(unix)]
    {
        push_canonical_root(&mut roots, PathBuf::from("/var/tmp"));
    }
    roots
}

fn push_canonical_root(roots: &mut Vec<PathBuf>, root: PathBuf) {
    if let Ok(canonical) = canonicalize_existing_dir(root.as_path()) {
        if !roots.iter().any(|existing| same_path(existing.as_path(), canonical.as_path())) {
            roots.push(canonical);
        }
    }
}

fn canonicalize_existing_dir(path: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        format!("{OS_FILE_TOOL_NAME} failed to resolve OS root {}: {error}", display_path(path))
    })?;
    if !canonical.is_dir() {
        return Err(format!("{OS_FILE_TOOL_NAME} OS root is not a directory"));
    }
    Ok(canonical)
}

fn protected_os_path(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let normalized = path.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
        return normalized.ends_with(":/")
            || normalized.contains(":/windows")
            || normalized.contains(":/program files")
            || normalized.contains(":/program files (x86)")
            || normalized.contains(":/system volume information");
    }
    #[cfg(not(windows))]
    {
        let normalized = path.to_string_lossy().replace('\\', "/");
        if normalized == "/" {
            return true;
        }
        for prefix in ["/etc", "/bin", "/sbin", "/usr", "/lib", "/lib64", "/System", "/Library"] {
            if normalized == prefix || normalized.starts_with(format!("{prefix}/").as_str()) {
                return true;
            }
        }
        false
    }
}

fn path_starts_with(path: &Path, root: &Path) -> bool {
    if path.starts_with(root) {
        return true;
    }
    #[cfg(windows)]
    {
        let path = path.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
        let root = root.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
        path == root || path.starts_with(format!("{root}/").as_str())
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .replace('\\', "/")
            .eq_ignore_ascii_case(&right.to_string_lossy().replace('\\', "/"))
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn metadata_kind(metadata: &fs::Metadata) -> &'static str {
    if metadata.is_dir() {
        "directory"
    } else if metadata.is_file() {
        "file"
    } else {
        "other"
    }
}

fn metadata_modified_unix_ms(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn os_file_outcome(
    proposal_id: &str,
    input_json: &[u8],
    success: bool,
    output_json: Vec<u8>,
    error: String,
) -> ToolExecutionOutcome {
    build_tool_execution_outcome(
        proposal_id,
        OS_FILE_TOOL_NAME,
        input_json,
        success,
        output_json,
        error,
        false,
        "os_file".to_owned(),
        "approved_os_paths".to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_policy(root: &Path) -> OsFilePolicy {
        OsFilePolicy {
            workspace_roots: vec![fs::canonicalize(root).expect("root should canonicalize")],
            user_os_roots: vec![fs::canonicalize(root).expect("root should canonicalize")],
        }
    }

    #[test]
    fn os_file_write_and_read_absolute_user_path() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let policy = test_policy(tempdir.path());
        let target = tempdir.path().join("os-level").join("reports").join("outside-report.md");

        let write = execute_os_file_operation(
            &policy,
            &OsFileInput {
                operation: OsFileOperation::Write,
                path: target.to_string_lossy().into_owned(),
                target_path: None,
                content_text: Some("palyra-os-level-ok\n".to_owned()),
                bytes_base64: None,
                create_parent_dirs: Some(true),
                overwrite: Some(true),
                dry_run: Some(false),
                offset_bytes: None,
                max_bytes: None,
            },
        )
        .expect("absolute user path write should succeed");

        assert_eq!(write.get("operation").and_then(Value::as_str), Some("write"));
        assert!(target.is_file());

        let read = execute_os_file_operation(
            &policy,
            &OsFileInput {
                operation: OsFileOperation::Read,
                path: target.to_string_lossy().into_owned(),
                target_path: None,
                content_text: None,
                bytes_base64: None,
                create_parent_dirs: None,
                overwrite: None,
                dry_run: None,
                offset_bytes: None,
                max_bytes: None,
            },
        )
        .expect("absolute user path read should succeed");

        assert_eq!(read.get("text").and_then(Value::as_str), Some("palyra-os-level-ok\n"));
        assert!(read.get("resolved_path").and_then(Value::as_str).is_some());
    }

    #[test]
    fn os_file_rejects_path_outside_workspace_and_user_roots() {
        let allowed_root = tempfile::tempdir().expect("allowed root should be created");
        let outside_root = tempfile::tempdir().expect("outside root should be created");
        let workspace = allowed_root.path().join("workspace");
        let outside = outside_root.path().join("outside").join("report.md");
        fs::create_dir_all(workspace.as_path()).expect("workspace should exist");
        fs::create_dir_all(outside.parent().expect("outside parent")).expect("outside parent");
        let policy = OsFilePolicy {
            workspace_roots: vec![fs::canonicalize(workspace).expect("workspace canonical")],
            user_os_roots: vec![
                fs::canonicalize(allowed_root.path()).expect("allowed root canonical")
            ],
        };

        let error = execute_os_file_operation(
            &policy,
            &OsFileInput {
                operation: OsFileOperation::Write,
                path: outside.to_string_lossy().into_owned(),
                target_path: None,
                content_text: Some("outside\n".to_owned()),
                bytes_base64: None,
                create_parent_dirs: Some(true),
                overwrite: Some(true),
                dry_run: Some(false),
                offset_bytes: None,
                max_bytes: None,
            },
        )
        .expect_err("outside path should require an approved root");

        assert!(error.contains("approved user-owned OS roots"), "unexpected error: {error}");
    }

    #[test]
    fn os_file_dry_run_does_not_write() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let policy = test_policy(tempdir.path());
        let target = tempdir.path().join("reports").join("dry-run.md");

        execute_os_file_operation(
            &policy,
            &OsFileInput {
                operation: OsFileOperation::Write,
                path: target.to_string_lossy().into_owned(),
                target_path: None,
                content_text: Some("dry-run\n".to_owned()),
                bytes_base64: None,
                create_parent_dirs: Some(true),
                overwrite: Some(true),
                dry_run: Some(true),
                offset_bytes: None,
                max_bytes: None,
            },
        )
        .expect("dry-run write should validate");

        assert!(!target.exists());
    }
}
