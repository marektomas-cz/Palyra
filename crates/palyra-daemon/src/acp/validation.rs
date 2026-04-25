use std::path::{Component, Path, PathBuf};

use super::{normalize_text, AcpRuntimeError, AcpRuntimeResult};

const MAX_ACP_SCOPE_COUNT: usize = 128;

pub(super) fn normalize_state_root(root: &Path) -> AcpRuntimeResult<PathBuf> {
    if root.as_os_str().is_empty() {
        return Err(AcpRuntimeError::InvalidField {
            field: "state_root",
            message: "ACP state root cannot be empty".to_owned(),
        });
    }
    if root.components().any(|component| matches!(component, Component::ParentDir)) {
        return Err(AcpRuntimeError::InvalidField {
            field: "state_root",
            message: "ACP state root cannot contain parent directory traversal components"
                .to_owned(),
        });
    }
    if root.is_absolute() {
        return Ok(root.to_path_buf());
    }
    let current_dir = std::env::current_dir().map_err(|source| AcpRuntimeError::Io {
        operation: "resolve_current_dir",
        path: root.to_path_buf(),
        source,
    })?;
    Ok(current_dir.join(root))
}

pub(super) fn normalize_scope_strings(scopes: Vec<String>) -> AcpRuntimeResult<Vec<String>> {
    if scopes.len() > MAX_ACP_SCOPE_COUNT {
        return Err(AcpRuntimeError::InvalidField {
            field: "scopes",
            message: format!("scope list exceeds {MAX_ACP_SCOPE_COUNT} entries"),
        });
    }
    let mut normalized = Vec::new();
    for scope in scopes {
        normalized.push(normalize_text(scope.as_str(), "scope", 128)?);
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}
