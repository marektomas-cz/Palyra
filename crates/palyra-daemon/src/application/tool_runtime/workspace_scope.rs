use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use crate::gateway::GatewayRuntimeState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveWorkspaceRoot {
    pub(crate) root: PathBuf,
    pub(crate) relative_path: String,
}

pub(crate) async fn session_active_workspace_root(
    runtime_state: &Arc<GatewayRuntimeState>,
    session_id: &str,
    workspace_roots: &[PathBuf],
) -> Result<Option<ActiveWorkspaceRoot>, String> {
    let state = runtime_state.session_project_context_state(session_id.to_owned()).await.map_err(
        |status| format!("failed to load session project workspace focus: {}", status.message()),
    )?;
    let Some(state) = state else {
        return Ok(None);
    };
    Ok(active_workspace_root_from_focus_paths(workspace_roots, state.focus_paths.as_slice()))
}

pub(crate) fn active_workspace_root_from_focus_paths(
    workspace_roots: &[PathBuf],
    focus_paths: &[String],
) -> Option<ActiveWorkspaceRoot> {
    let canonical_roots = canonicalize_workspace_roots(workspace_roots);
    if canonical_roots.is_empty() {
        return None;
    }

    for focus_path in focus_paths {
        let Some(focus_path) = normalize_relative_workspace_path(focus_path) else {
            continue;
        };
        if focus_path == "." {
            continue;
        }
        for root in &canonical_roots {
            let candidate = root.join(focus_path.as_str());
            let Some(directory) = nearest_existing_directory(candidate.as_path(), root) else {
                continue;
            };
            if directory == *root || !directory.starts_with(root) {
                continue;
            }
            let relative_path = directory
                .strip_prefix(root)
                .ok()
                .and_then(|relative| normalize_relative_workspace_path(&relative.to_string_lossy()))
                .unwrap_or_else(|| ".".to_owned());
            if relative_path == "." {
                continue;
            }
            return Some(ActiveWorkspaceRoot { root: directory, relative_path });
        }
    }
    None
}

pub(crate) fn relative_path_already_targets_active_root(
    path: &str,
    active: &ActiveWorkspaceRoot,
) -> bool {
    let Some(path) = normalize_relative_workspace_path(path) else {
        return false;
    };
    path == active.relative_path || path.starts_with(format!("{}/", active.relative_path).as_str())
}

fn canonicalize_workspace_roots(workspace_roots: &[PathBuf]) -> Vec<PathBuf> {
    workspace_roots
        .iter()
        .filter_map(|root| fs::canonicalize(root).ok().filter(|path| path.is_dir()))
        .collect()
}

fn nearest_existing_directory(candidate: &Path, workspace_root: &Path) -> Option<PathBuf> {
    let mut cursor = candidate.to_path_buf();
    loop {
        if cursor.exists() {
            if cursor.is_dir() {
                return Some(cursor);
            }
            return cursor.parent().map(Path::to_path_buf);
        }
        if cursor == workspace_root || !cursor.pop() {
            return None;
        }
    }
}

fn normalize_relative_workspace_path(path: &str) -> Option<String> {
    let normalized = path.trim().replace('\\', "/");
    let without_workspace_alias = normalized
        .strip_prefix("/workspace/")
        .or_else(|| normalized.strip_prefix("workspace/"))
        .unwrap_or(normalized.as_str());
    let trimmed = without_workspace_alias.trim_start_matches("./").trim_matches('/');
    if trimmed.is_empty() || trimmed == "." {
        return Some(".".to_owned());
    }

    let parsed = Path::new(trimmed);
    if parsed.is_absolute() {
        return None;
    }
    let mut components = Vec::new();
    for component in parsed.components() {
        match component {
            Component::Normal(value) => components.push(value.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if components.is_empty() {
        Some(".".to_owned())
    } else {
        Some(components.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        active_workspace_root_from_focus_paths, relative_path_already_targets_active_root,
    };
    use std::fs;

    #[test]
    fn active_workspace_root_uses_existing_session_focus_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let project = tempdir.path().join("scenario-s002-notes-api");
        fs::create_dir_all(project.as_path()).expect("project directory should exist");

        let active = active_workspace_root_from_focus_paths(
            &[tempdir.path().to_path_buf()],
            &["scenario-s002-notes-api".to_owned()],
        )
        .expect("active workspace root should resolve");

        assert_eq!(active.root, fs::canonicalize(project).expect("project should canonicalize"));
        assert_eq!(active.relative_path, "scenario-s002-notes-api");
        assert!(relative_path_already_targets_active_root(
            "scenario-s002-notes-api/package.json",
            &active
        ));
        assert!(!relative_path_already_targets_active_root("package.json", &active));
    }

    #[test]
    fn active_workspace_root_uses_nearest_existing_parent_for_file_focus() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let project = tempdir.path().join("scenario-s027-routine");
        fs::create_dir_all(project.as_path()).expect("project directory should exist");

        let active = active_workspace_root_from_focus_paths(
            &[tempdir.path().to_path_buf()],
            &["scenario-s027-routine/reports/cron-edit.log".to_owned()],
        )
        .expect("active workspace root should resolve to nearest existing parent");

        assert_eq!(active.root, fs::canonicalize(project).expect("project should canonicalize"));
        assert_eq!(active.relative_path, "scenario-s027-routine");
    }
}
