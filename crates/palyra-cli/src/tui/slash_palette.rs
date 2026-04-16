use serde_json::Value;

use crate::shared_chat_commands::{
    find_shared_chat_command, resolve_shared_chat_command_name, shared_chat_commands_for_surface,
    SharedChatCommandDefinition, SharedChatCommandSurface,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct TuiSlashEntityCatalog {
    pub(crate) sessions: Vec<TuiSlashSessionRecord>,
    pub(crate) objectives: Vec<TuiSlashObjectiveRecord>,
    pub(crate) auth_profiles: Vec<TuiSlashAuthProfileRecord>,
    pub(crate) browser_profiles: Vec<TuiSlashBrowserProfileRecord>,
    pub(crate) browser_sessions: Vec<TuiSlashBrowserSessionRecord>,
    pub(crate) checkpoints: Vec<TuiSlashCheckpointRecord>,
    pub(crate) workspace_artifacts: Vec<TuiSlashWorkspaceArtifactRecord>,
    pub(crate) workspace_checkpoints: Vec<TuiSlashWorkspaceCheckpointRecord>,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashSessionRelative {
    pub(crate) session_id: String,
    pub(crate) title: String,
    pub(crate) branch_state: String,
    pub(crate) relation: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashSessionRecord {
    pub(crate) session_id: String,
    pub(crate) title: String,
    pub(crate) session_key: String,
    pub(crate) archived: bool,
    pub(crate) preview: String,
    pub(crate) root_title: String,
    pub(crate) last_summary: String,
    pub(crate) branch_state: String,
    pub(crate) family_sequence: u64,
    pub(crate) family_size: usize,
    pub(crate) parent_session_id: Option<String>,
    pub(crate) parent_title: Option<String>,
    pub(crate) pending_approvals: usize,
    pub(crate) artifact_count: usize,
    pub(crate) active_context_files: usize,
    pub(crate) relatives: Vec<TuiSlashSessionRelative>,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashObjectiveRecord {
    pub(crate) objective_id: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) focus: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashAuthProfileRecord {
    pub(crate) profile_id: String,
    pub(crate) profile_name: String,
    pub(crate) provider_kind: String,
    pub(crate) scope_kind: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashBrowserProfileRecord {
    pub(crate) profile_id: String,
    pub(crate) name: String,
    pub(crate) persistence_enabled: bool,
    pub(crate) private_profile: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashBrowserSessionRecord {
    pub(crate) session_id: String,
    pub(crate) title: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashCheckpointRecord {
    pub(crate) checkpoint_id: String,
    pub(crate) name: String,
    pub(crate) note: String,
    pub(crate) created_at_unix_ms: i64,
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashWorkspaceArtifactRecord {
    pub(crate) artifact_id: String,
    pub(crate) path: String,
    pub(crate) display_path: String,
    pub(crate) change_kind: String,
    pub(crate) latest_checkpoint_id: String,
    pub(crate) preview_kind: String,
    pub(crate) size_bytes: Option<u64>,
    pub(crate) deleted: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashWorkspaceCheckpointRecord {
    pub(crate) checkpoint_id: String,
    pub(crate) source_label: String,
    pub(crate) summary_text: String,
    pub(crate) restore_count: u64,
    pub(crate) created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TuiUxMetrics {
    pub(crate) slash_commands: u64,
    pub(crate) palette_accepts: u64,
    pub(crate) keyboard_accepts: u64,
    pub(crate) undo: u64,
    pub(crate) interrupt: u64,
    pub(crate) errors: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TuiUxMetricKey {
    SlashCommands,
    PaletteAccepts,
    KeyboardAccepts,
    Undo,
    Interrupt,
    Errors,
}

impl TuiUxMetrics {
    pub(crate) fn record(&mut self, key: TuiUxMetricKey) {
        match key {
            TuiUxMetricKey::SlashCommands => self.slash_commands += 1,
            TuiUxMetricKey::PaletteAccepts => self.palette_accepts += 1,
            TuiUxMetricKey::KeyboardAccepts => self.keyboard_accepts += 1,
            TuiUxMetricKey::Undo => self.undo += 1,
            TuiUxMetricKey::Interrupt => self.interrupt += 1,
            TuiUxMetricKey::Errors => self.errors += 1,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashSuggestion {
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) detail: String,
    pub(crate) example: String,
    pub(crate) replacement: String,
    pub(crate) badge: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashPaletteState {
    pub(crate) active_command: Option<&'static SharedChatCommandDefinition>,
    pub(crate) suggestions: Vec<TuiSlashSuggestion>,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiSlashPreview {
    pub(crate) title: String,
    pub(crate) subtitle: String,
    pub(crate) detail: String,
    pub(crate) example: String,
    pub(crate) badge: String,
}

pub(crate) struct BuildTuiSlashPaletteArgs<'a> {
    pub(crate) input: &'a str,
    pub(crate) catalog: &'a TuiSlashEntityCatalog,
    pub(crate) streaming: bool,
    pub(crate) delegation_profiles: &'a [&'a str],
    pub(crate) delegation_templates: &'a [&'a str],
}

pub(crate) fn build_tui_slash_palette(
    args: BuildTuiSlashPaletteArgs<'_>,
) -> Option<TuiSlashPaletteState> {
    let trimmed = args.input.trim_start();
    if !trimmed.starts_with('/') {
        return None;
    }

    let body = trimmed.trim_start_matches('/');
    let has_trailing_whitespace = trimmed.chars().last().map(char::is_whitespace).unwrap_or(false);
    let raw_parts = body.split_whitespace().collect::<Vec<_>>();
    let raw_command_token = raw_parts.first().copied().unwrap_or_default();
    let normalized_command_token = raw_command_token.trim().to_ascii_lowercase();
    let normalized_command_name = resolve_shared_chat_command_name(
        normalized_command_token.as_str(),
        SharedChatCommandSurface::Tui,
    )
    .unwrap_or(normalized_command_token.as_str());
    let active_command = if normalized_command_name.is_empty() {
        None
    } else {
        find_shared_chat_command(normalized_command_name, SharedChatCommandSurface::Tui)
    };

    if raw_command_token.trim().is_empty() || active_command.is_none() {
        return Some(TuiSlashPaletteState {
            active_command,
            suggestions: build_command_name_suggestions(normalized_command_token.as_str()),
        });
    }

    let normalized_rest = raw_parts.iter().skip(1).copied().collect::<Vec<_>>().join(" ");
    let active_token = if has_trailing_whitespace || normalized_rest.trim().is_empty() {
        String::new()
    } else {
        normalized_rest.split_whitespace().last().unwrap_or_default().trim().to_ascii_lowercase()
    };
    Some(TuiSlashPaletteState {
        active_command,
        suggestions: build_entity_suggestions(
            active_command
                .expect("active command must be present when building entity suggestions"),
            args.catalog,
            normalized_rest.trim(),
            active_token.as_str(),
            args.streaming,
            args.delegation_profiles,
            args.delegation_templates,
        ),
    })
}

pub(crate) fn preview_for_selection(
    palette: &TuiSlashPaletteState,
    selected: usize,
) -> Option<TuiSlashPreview> {
    if let Some(suggestion) = palette.suggestions.get(selected) {
        return Some(TuiSlashPreview {
            title: suggestion.title.clone(),
            subtitle: suggestion.subtitle.clone(),
            detail: suggestion.detail.clone(),
            example: suggestion.example.clone(),
            badge: suggestion.badge.clone(),
        });
    }
    palette.active_command.map(|command| TuiSlashPreview {
        title: command.synopsis.clone(),
        subtitle: command.description.clone(),
        detail: command.example.clone(),
        example: command.example.clone(),
        badge: command.category.clone(),
    })
}

pub(crate) fn select_undo_checkpoint(
    checkpoints: &[TuiSlashCheckpointRecord],
) -> Option<&TuiSlashCheckpointRecord> {
    let undo_checkpoint = checkpoints
        .iter()
        .filter(|checkpoint| checkpoint_has_tag(checkpoint, "undo_safe"))
        .max_by_key(|checkpoint| checkpoint.created_at_unix_ms);
    if undo_checkpoint.is_some() {
        return undo_checkpoint;
    }
    checkpoints.iter().max_by_key(|checkpoint| checkpoint.created_at_unix_ms)
}

pub(crate) fn checkpoint_has_tag(checkpoint: &TuiSlashCheckpointRecord, tag: &str) -> bool {
    let normalized = tag.trim().to_ascii_lowercase();
    !normalized.is_empty()
        && checkpoint.tags.iter().any(|entry| entry.trim().to_ascii_lowercase() == normalized)
}

pub(crate) fn read_json_string(value: &Value, pointer: &str) -> String {
    value.pointer(pointer).and_then(Value::as_str).unwrap_or_default().to_owned()
}

pub(crate) fn read_json_i64(value: &Value, pointer: &str) -> i64 {
    value.pointer(pointer).and_then(Value::as_i64).unwrap_or_default()
}

pub(crate) fn read_json_bool(value: &Value, pointer: &str) -> bool {
    value.pointer(pointer).and_then(Value::as_bool).unwrap_or(false)
}

pub(crate) fn read_json_tags(value: &Value, pointer: &str) -> Vec<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn build_command_name_suggestions(query: &str) -> Vec<TuiSlashSuggestion> {
    let normalized_query = query.trim().to_ascii_lowercase();
    shared_chat_commands_for_surface(SharedChatCommandSurface::Tui)
        .into_iter()
        .filter(|command| {
            if normalized_query.is_empty() {
                return true;
            }
            command.name.contains(normalized_query.as_str())
                || command.aliases.iter().any(|alias| alias.contains(normalized_query.as_str()))
                || command
                    .keywords
                    .iter()
                    .any(|keyword| keyword.contains(normalized_query.as_str()))
        })
        .take(8)
        .map(|command| {
            let replacement = if command.synopsis.contains(' ') {
                format!("/{} ", command.name)
            } else {
                format!("/{}", command.name)
            };
            TuiSlashSuggestion {
                title: command.synopsis.clone(),
                subtitle: command.description.clone(),
                detail: command.example.clone(),
                example: command.example.clone(),
                replacement,
                badge: command.category.clone(),
            }
        })
        .collect()
}

fn build_entity_suggestions(
    command: &'static SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    normalized_rest: &str,
    active_token: &str,
    streaming: bool,
    delegation_profiles: &[&str],
    delegation_templates: &[&str],
) -> Vec<TuiSlashSuggestion> {
    match command.name.as_str() {
        "resume" | "history" => build_session_suggestions(command, catalog, active_token),
        "objective" => build_objective_suggestions(command, catalog, active_token),
        "profile" => build_profile_suggestions(command, catalog, active_token),
        "browser" => {
            let mut suggestions = build_browser_profile_suggestions(command, catalog, active_token);
            suggestions.extend(build_browser_session_suggestions(command, catalog, active_token));
            suggestions.truncate(8);
            suggestions
        }
        "delegate" => build_delegation_suggestions(
            command,
            active_token,
            delegation_profiles,
            delegation_templates,
        ),
        "checkpoint" => {
            build_checkpoint_suggestions(command, catalog, normalized_rest, active_token)
        }
        "rollback" => {
            build_workspace_rollback_suggestions(command, catalog, normalized_rest, active_token)
        }
        "undo" => build_undo_suggestions(command, catalog, active_token),
        "workspace" => build_workspace_suggestions(command, catalog, normalized_rest, active_token),
        "interrupt" => build_interrupt_suggestions(command, active_token, streaming),
        "doctor" => build_doctor_suggestions(command, active_token),
        "compact" => {
            build_static_suggestions(command, active_token, &["preview", "apply", "history"])
        }
        "tools" | "thinking" | "verbose" => {
            build_static_suggestions(command, active_token, &["on", "off", "default"])
        }
        "shell" => build_static_suggestions(command, active_token, &["on", "off"]),
        "export" => build_static_suggestions(command, active_token, &["json", "markdown"]),
        _ => Vec::new(),
    }
}

fn build_session_suggestions(
    command: &SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    active_token: &str,
) -> Vec<TuiSlashSuggestion> {
    let query = active_token.trim().to_ascii_lowercase();
    catalog
        .sessions
        .iter()
        .filter(|session| {
            query.is_empty()
                || session.session_id.to_ascii_lowercase().contains(query.as_str())
                || session.title.to_ascii_lowercase().contains(query.as_str())
                || session.session_key.to_ascii_lowercase().contains(query.as_str())
                || session.root_title.to_ascii_lowercase().contains(query.as_str())
                || session.last_summary.to_ascii_lowercase().contains(query.as_str())
                || session
                    .parent_title
                    .as_ref()
                    .map(|title| title.to_ascii_lowercase().contains(query.as_str()))
                    .unwrap_or(false)
                || session.relatives.iter().any(|relative| {
                    relative.title.to_ascii_lowercase().contains(query.as_str())
                        || relative.relation.to_ascii_lowercase().contains(query.as_str())
                })
        })
        .take(6)
        .map(|session| {
            let mut detail_parts = Vec::new();
            if !session.last_summary.is_empty() {
                detail_parts.push(session.last_summary.clone());
            } else if !session.preview.is_empty() {
                detail_parts.push(session.preview.clone());
            } else if !session.root_title.is_empty() && session.root_title != session.title {
                detail_parts.push(format!("Family root: {}", session.root_title));
            } else {
                detail_parts.push("Resume this session context.".to_owned());
            }
            detail_parts.push(format!(
                "approvals {} · artifacts {} · context {}",
                session.pending_approvals, session.artifact_count, session.active_context_files
            ));
            if let Some(parent_session_id) = session.parent_session_id.as_ref() {
                detail_parts.push(format!(
                    "parent {}",
                    session
                        .parent_title
                        .clone()
                        .unwrap_or_else(|| shorten_entity_id(parent_session_id.as_str()))
                ));
            }
            if !session.relatives.is_empty() {
                let family_preview = session
                    .relatives
                    .iter()
                    .take(2)
                    .map(|relative| {
                        let lineage_state = if relative.branch_state.is_empty() {
                            shorten_entity_id(relative.session_id.as_str())
                        } else {
                            relative.branch_state.clone()
                        };
                        format!("{} {} ({lineage_state})", relative.relation, relative.title)
                    })
                    .collect::<Vec<_>>()
                    .join(" · ");
                detail_parts.push(format!("family {family_preview}"));
            }
            TuiSlashSuggestion {
                title: session.title.clone(),
                subtitle: if session.session_key.is_empty() {
                    session.session_id.clone()
                } else {
                    match session.family_size {
                        0 | 1 => session.session_key.clone(),
                        size => format!(
                            "{} · family {}/{}",
                            session.session_key, session.family_sequence, size
                        ),
                    }
                },
                detail: detail_parts.join(" · "),
                example: format!("/{} {}", command.name, session.session_id),
                replacement: if command.name == "history" {
                    format!("/{} {}", command.name, session.title)
                } else {
                    format!("/{} {}", command.name, session.session_id)
                },
                badge: if session.archived {
                    "archived".to_owned()
                } else if session.branch_state.eq_ignore_ascii_case("active_branch") {
                    "branch".to_owned()
                } else {
                    "session".to_owned()
                },
            }
        })
        .collect()
}

fn build_objective_suggestions(
    command: &SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    active_token: &str,
) -> Vec<TuiSlashSuggestion> {
    let query = active_token.trim().to_ascii_lowercase();
    catalog
        .objectives
        .iter()
        .filter(|objective| {
            query.is_empty()
                || objective.objective_id.to_ascii_lowercase().contains(query.as_str())
                || objective.name.to_ascii_lowercase().contains(query.as_str())
                || objective.kind.to_ascii_lowercase().contains(query.as_str())
        })
        .take(6)
        .map(|objective| TuiSlashSuggestion {
            title: format!("{} · {}", objective.kind.replace('_', " "), objective.name),
            subtitle: objective.objective_id.clone(),
            detail: if objective.focus.is_empty() {
                "No focus recorded.".to_owned()
            } else {
                objective.focus.clone()
            },
            example: format!("/{} {}", command.name, objective.objective_id),
            replacement: format!("/{} {}", command.name, objective.objective_id),
            badge: objective.kind.clone(),
        })
        .collect()
}

fn build_profile_suggestions(
    command: &SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    active_token: &str,
) -> Vec<TuiSlashSuggestion> {
    let query = active_token.trim().to_ascii_lowercase();
    catalog
        .auth_profiles
        .iter()
        .filter(|profile| {
            query.is_empty()
                || profile.profile_id.to_ascii_lowercase().contains(query.as_str())
                || profile.profile_name.to_ascii_lowercase().contains(query.as_str())
                || profile.provider_kind.to_ascii_lowercase().contains(query.as_str())
        })
        .take(6)
        .map(|profile| TuiSlashSuggestion {
            title: profile.profile_name.clone(),
            subtitle: format!("{} · {}", profile.provider_kind, profile.scope_kind),
            detail: profile.profile_id.clone(),
            example: format!("/{} {}", command.name, profile.profile_id),
            replacement: format!("/{} {}", command.name, profile.profile_id),
            badge: "profile".to_owned(),
        })
        .collect()
}

fn build_browser_profile_suggestions(
    command: &SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    active_token: &str,
) -> Vec<TuiSlashSuggestion> {
    let query = active_token.trim().to_ascii_lowercase();
    catalog
        .browser_profiles
        .iter()
        .filter(|profile| {
            query.is_empty()
                || profile.profile_id.to_ascii_lowercase().contains(query.as_str())
                || profile.name.to_ascii_lowercase().contains(query.as_str())
        })
        .take(4)
        .map(|profile| TuiSlashSuggestion {
            title: profile.name.clone(),
            subtitle: profile.profile_id.clone(),
            detail: format!(
                "{} · {}",
                if profile.persistence_enabled { "persistent" } else { "ephemeral" },
                if profile.private_profile { "private" } else { "shared" }
            ),
            example: format!("/{} {}", command.name, profile.profile_id),
            replacement: format!("/{} {}", command.name, profile.profile_id),
            badge: "browser profile".to_owned(),
        })
        .collect()
}

fn build_browser_session_suggestions(
    command: &SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    active_token: &str,
) -> Vec<TuiSlashSuggestion> {
    let query = active_token.trim().to_ascii_lowercase();
    catalog
        .browser_sessions
        .iter()
        .filter(|session| {
            query.is_empty()
                || session.session_id.to_ascii_lowercase().contains(query.as_str())
                || session.title.to_ascii_lowercase().contains(query.as_str())
        })
        .take(4)
        .map(|session| TuiSlashSuggestion {
            title: session.title.clone(),
            subtitle: session.session_id.clone(),
            detail: "Inspect browser session detail in the transcript.".to_owned(),
            example: format!("/{} {}", command.name, session.session_id),
            replacement: format!("/{} {}", command.name, session.session_id),
            badge: "browser session".to_owned(),
        })
        .collect()
}

fn build_delegation_suggestions(
    command: &SharedChatCommandDefinition,
    active_token: &str,
    delegation_profiles: &[&str],
    delegation_templates: &[&str],
) -> Vec<TuiSlashSuggestion> {
    let query = active_token.trim().to_ascii_lowercase();
    delegation_templates
        .iter()
        .map(|template| (*template, "template"))
        .chain(delegation_profiles.iter().map(|profile| (*profile, "profile")))
        .filter(|(value, _)| query.is_empty() || value.contains(&query))
        .take(8)
        .map(|(value, badge)| TuiSlashSuggestion {
            title: value.to_owned(),
            subtitle: badge.to_owned(),
            detail: "Complete the command with a delegated task prompt.".to_owned(),
            example: format!("/{} {} Summarize the latest operator findings.", command.name, value),
            replacement: format!("/{} {} ", command.name, value),
            badge: badge.to_owned(),
        })
        .collect()
}

fn build_checkpoint_suggestions(
    command: &SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    normalized_rest: &str,
    active_token: &str,
) -> Vec<TuiSlashSuggestion> {
    if !normalized_rest.starts_with("restore") {
        return build_static_suggestions(command, active_token, &["list", "restore", "save"]);
    }
    let query = active_token.trim().to_ascii_lowercase();
    catalog
        .checkpoints
        .iter()
        .filter(|checkpoint| {
            query.is_empty()
                || checkpoint.checkpoint_id.to_ascii_lowercase().contains(query.as_str())
                || checkpoint.name.to_ascii_lowercase().contains(query.as_str())
        })
        .take(6)
        .map(|checkpoint| TuiSlashSuggestion {
            title: checkpoint.name.clone(),
            subtitle: checkpoint.checkpoint_id.clone(),
            detail: if checkpoint.note.is_empty() {
                "Restore checkpoint into a new branch.".to_owned()
            } else {
                checkpoint.note.clone()
            },
            example: format!("/{} restore {}", command.name, checkpoint.checkpoint_id),
            replacement: format!("/{} restore {}", command.name, checkpoint.checkpoint_id),
            badge: if checkpoint_has_tag(checkpoint, "undo_safe") {
                "undo-safe".to_owned()
            } else {
                "checkpoint".to_owned()
            },
        })
        .collect()
}

fn build_workspace_rollback_suggestions(
    command: &SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    normalized_rest: &str,
    active_token: &str,
) -> Vec<TuiSlashSuggestion> {
    if normalized_rest.starts_with("diff") {
        return build_workspace_checkpoint_suggestions(command, catalog, active_token, "diff");
    }
    if normalized_rest.starts_with("restore") {
        return build_workspace_checkpoint_suggestions(command, catalog, active_token, "restore");
    }
    vec![
        TuiSlashSuggestion {
            title: "Workspace rollback summary".to_owned(),
            subtitle: command.synopsis.clone(),
            detail: "List workspace checkpoints and restore guidance for the latest run."
                .to_owned(),
            example: format!("/{}", command.name),
            replacement: format!("/{}", command.name),
            badge: command.category.clone(),
        },
        TuiSlashSuggestion {
            title: "Preview rollback diff".to_owned(),
            subtitle: "diff".to_owned(),
            detail: "Compare the current run against a workspace checkpoint before restoring."
                .to_owned(),
            example: format!("/{} diff ", command.name),
            replacement: format!("/{} diff ", command.name),
            badge: "diff".to_owned(),
        },
        TuiSlashSuggestion {
            title: "Restore workspace checkpoint".to_owned(),
            subtitle: "restore".to_owned(),
            detail: "Require `--confirm` before mutating the tracked workspace.".to_owned(),
            example: format!("/{} restore ", command.name),
            replacement: format!("/{} restore ", command.name),
            badge: "restore".to_owned(),
        },
    ]
}

fn build_workspace_suggestions(
    command: &SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    normalized_rest: &str,
    active_token: &str,
) -> Vec<TuiSlashSuggestion> {
    if normalized_rest.starts_with("show") {
        return build_workspace_artifact_suggestions(command, catalog, active_token, "show");
    }
    if normalized_rest.starts_with("open") {
        return build_workspace_artifact_suggestions(command, catalog, active_token, "open");
    }
    if normalized_rest.starts_with("handoff") {
        return vec![
            TuiSlashSuggestion {
                title: "Workspace handoff path".to_owned(),
                subtitle: "handoff".to_owned(),
                detail: "Print the matching web workspace handoff path for the current run."
                    .to_owned(),
                example: format!("/{} handoff", command.name),
                replacement: format!("/{} handoff", command.name),
                badge: command.category.clone(),
            },
            TuiSlashSuggestion {
                title: "Open workspace handoff".to_owned(),
                subtitle: "handoff open".to_owned(),
                detail: "Open the workspace handoff directly in the default browser.".to_owned(),
                example: format!("/{} handoff open", command.name),
                replacement: format!("/{} handoff open", command.name),
                badge: "browser".to_owned(),
            },
        ];
    }
    vec![
        TuiSlashSuggestion {
            title: "Workspace summary".to_owned(),
            subtitle: command.synopsis.clone(),
            detail: "List changed paths, checkpoints, and handoff options for the latest run."
                .to_owned(),
            example: format!("/{}", command.name),
            replacement: format!("/{}", command.name),
            badge: command.category.clone(),
        },
        TuiSlashSuggestion {
            title: "Changed workspace paths".to_owned(),
            subtitle: "changed".to_owned(),
            detail: "Show the latest changed files without opening artifact detail.".to_owned(),
            example: format!("/{} changed", command.name),
            replacement: format!("/{} changed", command.name),
            badge: "workspace".to_owned(),
        },
        TuiSlashSuggestion {
            title: "Inspect workspace artifact".to_owned(),
            subtitle: "show".to_owned(),
            detail: "Resolve an artifact id or list index to preview content in the transcript."
                .to_owned(),
            example: format!("/{} show ", command.name),
            replacement: format!("/{} show ", command.name),
            badge: "artifact".to_owned(),
        },
        TuiSlashSuggestion {
            title: "Open workspace artifact externally".to_owned(),
            subtitle: "open".to_owned(),
            detail: "Open a changed file with the platform default application.".to_owned(),
            example: format!("/{} open ", command.name),
            replacement: format!("/{} open ", command.name),
            badge: "external".to_owned(),
        },
        TuiSlashSuggestion {
            title: "Workspace handoff".to_owned(),
            subtitle: "handoff".to_owned(),
            detail: "Send the current workspace context to the web console for richer preview."
                .to_owned(),
            example: format!("/{} handoff", command.name),
            replacement: format!("/{} handoff", command.name),
            badge: "handoff".to_owned(),
        },
    ]
}

fn build_workspace_artifact_suggestions(
    command: &SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    active_token: &str,
    action: &str,
) -> Vec<TuiSlashSuggestion> {
    let query = active_token.trim().to_ascii_lowercase();
    catalog
        .workspace_artifacts
        .iter()
        .filter(|artifact| {
            query.is_empty()
                || artifact.artifact_id.to_ascii_lowercase().contains(query.as_str())
                || artifact.display_path.to_ascii_lowercase().contains(query.as_str())
                || artifact.path.to_ascii_lowercase().contains(query.as_str())
                || artifact.change_kind.to_ascii_lowercase().contains(query.as_str())
        })
        .take(6)
        .map(|artifact| TuiSlashSuggestion {
            title: artifact.display_path.clone(),
            subtitle: artifact.artifact_id.clone(),
            detail: format!(
                "{} · {} · cp {} · {}{}",
                artifact.change_kind,
                artifact.preview_kind,
                shorten_entity_id(artifact.latest_checkpoint_id.as_str()),
                artifact
                    .size_bytes
                    .map(|value| format!("{value} bytes"))
                    .unwrap_or_else(|| "size unknown".to_owned()),
                if artifact.deleted { " · deleted" } else { "" }
            ),
            example: format!("/{} {} {}", command.name, action, artifact.artifact_id),
            replacement: format!("/{} {} {}", command.name, action, artifact.artifact_id),
            badge: if artifact.deleted { "deleted".to_owned() } else { "artifact".to_owned() },
        })
        .collect()
}

fn build_workspace_checkpoint_suggestions(
    command: &SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    active_token: &str,
    action: &str,
) -> Vec<TuiSlashSuggestion> {
    let query = active_token.trim().to_ascii_lowercase();
    let mut checkpoints = catalog.workspace_checkpoints.iter().collect::<Vec<_>>();
    checkpoints.sort_by_key(|checkpoint| std::cmp::Reverse(checkpoint.created_at_unix_ms));
    checkpoints
        .into_iter()
        .filter(|checkpoint| {
            query.is_empty()
                || checkpoint.checkpoint_id.to_ascii_lowercase().contains(query.as_str())
                || checkpoint.source_label.to_ascii_lowercase().contains(query.as_str())
                || checkpoint.summary_text.to_ascii_lowercase().contains(query.as_str())
        })
        .take(6)
        .map(|checkpoint| TuiSlashSuggestion {
            title: checkpoint.source_label.clone(),
            subtitle: checkpoint.checkpoint_id.clone(),
            detail: format!(
                "{} · restores {}",
                if checkpoint.summary_text.is_empty() {
                    "Workspace checkpoint".to_owned()
                } else {
                    checkpoint.summary_text.clone()
                },
                checkpoint.restore_count
            ),
            example: if action == "restore" {
                format!("/{} restore {} --confirm", command.name, checkpoint.checkpoint_id)
            } else {
                format!("/{} diff {}", command.name, checkpoint.checkpoint_id)
            },
            replacement: if action == "restore" {
                format!("/{} restore {} --confirm", command.name, checkpoint.checkpoint_id)
            } else {
                format!("/{} diff {}", command.name, checkpoint.checkpoint_id)
            },
            badge: "workspace checkpoint".to_owned(),
        })
        .collect()
}

fn build_undo_suggestions(
    command: &SharedChatCommandDefinition,
    catalog: &TuiSlashEntityCatalog,
    active_token: &str,
) -> Vec<TuiSlashSuggestion> {
    let mut suggestions = Vec::new();
    if active_token.trim().is_empty() {
        if let Some(checkpoint) = select_undo_checkpoint(catalog.checkpoints.as_slice()) {
            suggestions.push(TuiSlashSuggestion {
                title: "Undo last turn".to_owned(),
                subtitle: checkpoint.name.clone(),
                detail: if checkpoint.note.is_empty() {
                    "Restore the latest safe checkpoint.".to_owned()
                } else {
                    checkpoint.note.clone()
                },
                example: format!("/{}", command.name),
                replacement: format!("/{}", command.name),
                badge: if checkpoint_has_tag(checkpoint, "undo_safe") {
                    "undo-safe".to_owned()
                } else {
                    "checkpoint".to_owned()
                },
            });
        }
    }
    let query = active_token.trim().to_ascii_lowercase();
    suggestions.extend(
        catalog
            .checkpoints
            .iter()
            .filter(|checkpoint| {
                query.is_empty()
                    || checkpoint.checkpoint_id.to_ascii_lowercase().contains(query.as_str())
                    || checkpoint.name.to_ascii_lowercase().contains(query.as_str())
            })
            .take(6)
            .map(|checkpoint| TuiSlashSuggestion {
                title: checkpoint.name.clone(),
                subtitle: checkpoint.checkpoint_id.clone(),
                detail: if checkpoint.note.is_empty() {
                    "Restore this checkpoint as the new active branch.".to_owned()
                } else {
                    checkpoint.note.clone()
                },
                example: format!("/{} {}", command.name, checkpoint.checkpoint_id),
                replacement: format!("/{} {}", command.name, checkpoint.checkpoint_id),
                badge: if checkpoint_has_tag(checkpoint, "undo_safe") {
                    "undo-safe".to_owned()
                } else {
                    "checkpoint".to_owned()
                },
            }),
    );
    suggestions.truncate(8);
    suggestions
}

fn build_interrupt_suggestions(
    command: &SharedChatCommandDefinition,
    active_token: &str,
    streaming: bool,
) -> Vec<TuiSlashSuggestion> {
    ["soft", "force"]
        .into_iter()
        .filter(|candidate| active_token.is_empty() || candidate.contains(active_token))
        .map(|candidate| TuiSlashSuggestion {
            title: if candidate == "soft" {
                "Soft interrupt".to_owned()
            } else {
                "Force interrupt".to_owned()
            },
            subtitle: if streaming {
                "Current run is active".to_owned()
            } else {
                "Prepare redirect before the next send".to_owned()
            },
            detail: if candidate == "soft" {
                "Wait for the runtime to honor a normal cancellation request.".to_owned()
            } else {
                "Escalate wording only when a normal interrupt already failed.".to_owned()
            },
            example: format!("/{} {} Summarize the failures instead.", command.name, candidate),
            replacement: format!("/{} {} ", command.name, candidate),
            badge: candidate.to_owned(),
        })
        .collect()
}

fn build_doctor_suggestions(
    command: &SharedChatCommandDefinition,
    active_token: &str,
) -> Vec<TuiSlashSuggestion> {
    [
        ("jobs", "Recent jobs", "List the latest doctor recovery jobs."),
        ("run", "Dry-run doctor", "Queue a non-mutating doctor recovery pass."),
        ("repair", "Repair doctor", "Queue a repair-enabled doctor recovery pass."),
    ]
    .into_iter()
    .filter(|(value, title, _)| {
        active_token.is_empty()
            || value.contains(active_token)
            || title.to_ascii_lowercase().contains(active_token)
    })
    .map(|(value, title, detail)| TuiSlashSuggestion {
        title: title.to_owned(),
        subtitle: value.to_owned(),
        detail: detail.to_owned(),
        example: format!("/{} {}", command.name, value),
        replacement: format!("/{} {}", command.name, value),
        badge: "doctor".to_owned(),
    })
    .collect()
}

fn build_static_suggestions(
    command: &SharedChatCommandDefinition,
    active_token: &str,
    values: &[&str],
) -> Vec<TuiSlashSuggestion> {
    values
        .iter()
        .copied()
        .filter(|value| active_token.is_empty() || value.contains(active_token))
        .map(|value| TuiSlashSuggestion {
            title: format!("{} {}", command.name, value),
            subtitle: command.synopsis.clone(),
            detail: command.description.clone(),
            example: format!("/{} {}", command.name, value),
            replacement: format!("/{} {}", command.name, value),
            badge: command.category.clone(),
        })
        .collect()
}

fn shorten_entity_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 12 {
        trimmed.to_owned()
    } else {
        format!("{}…{}", &trimmed[..6], &trimmed[trimmed.len().saturating_sub(4)..])
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tui_slash_palette, checkpoint_has_tag, preview_for_selection, read_json_tags,
        select_undo_checkpoint, BuildTuiSlashPaletteArgs, TuiSlashCheckpointRecord,
        TuiSlashEntityCatalog, TuiSlashSessionRecord, TuiSlashWorkspaceArtifactRecord,
        TuiSlashWorkspaceCheckpointRecord,
    };
    use serde_json::json;

    #[test]
    fn slash_palette_offers_command_matches_for_partial_input() {
        let palette = build_tui_slash_palette(BuildTuiSlashPaletteArgs {
            input: "/int",
            catalog: &TuiSlashEntityCatalog::default(),
            streaming: false,
            delegation_profiles: &[],
            delegation_templates: &[],
        })
        .expect("slash palette should be available");
        assert!(palette
            .suggestions
            .iter()
            .any(|suggestion| suggestion.replacement.starts_with("/interrupt")));
    }

    #[test]
    fn slash_palette_uses_real_session_catalog_for_resume() {
        let palette = build_tui_slash_palette(BuildTuiSlashPaletteArgs {
            input: "/resume ops",
            catalog: &TuiSlashEntityCatalog {
                sessions: vec![TuiSlashSessionRecord {
                    session_id: "01ARZ3NDEKTSV4RRFFQ69G5FAW".to_owned(),
                    title: "Ops Triage".to_owned(),
                    session_key: "ops:triage".to_owned(),
                    archived: false,
                    preview: "Investigate deploy failures".to_owned(),
                    root_title: "Ops Triage".to_owned(),
                    last_summary: "Investigate deploy failures".to_owned(),
                    branch_state: "root".to_owned(),
                    family_sequence: 1,
                    family_size: 1,
                    parent_session_id: None,
                    parent_title: None,
                    pending_approvals: 0,
                    artifact_count: 0,
                    active_context_files: 0,
                    relatives: Vec::new(),
                }],
                ..TuiSlashEntityCatalog::default()
            },
            streaming: false,
            delegation_profiles: &[],
            delegation_templates: &[],
        })
        .expect("resume palette should be available");
        assert_eq!(palette.suggestions.len(), 1);
        assert_eq!(palette.suggestions[0].replacement, "/resume 01ARZ3NDEKTSV4RRFFQ69G5FAW");
    }

    #[test]
    fn undo_selection_prefers_undo_safe_checkpoint() {
        let checkpoints = vec![
            TuiSlashCheckpointRecord {
                checkpoint_id: "older".to_owned(),
                name: "Older".to_owned(),
                note: String::new(),
                created_at_unix_ms: 10,
                tags: vec!["manual".to_owned()],
            },
            TuiSlashCheckpointRecord {
                checkpoint_id: "latest".to_owned(),
                name: "Latest".to_owned(),
                note: String::new(),
                created_at_unix_ms: 20,
                tags: vec!["undo_safe".to_owned()],
            },
        ];
        assert_eq!(
            select_undo_checkpoint(checkpoints.as_slice())
                .map(|checkpoint| checkpoint.checkpoint_id.as_str()),
            Some("latest")
        );
    }

    #[test]
    fn checkpoint_tag_matching_is_case_insensitive() {
        let checkpoint = TuiSlashCheckpointRecord {
            checkpoint_id: "latest".to_owned(),
            name: "Latest".to_owned(),
            note: String::new(),
            created_at_unix_ms: 20,
            tags: vec!["Undo_Safe".to_owned()],
        };
        assert!(checkpoint_has_tag(&checkpoint, "undo_safe"));
    }

    #[test]
    fn preview_falls_back_to_active_command_when_no_suggestions_exist() {
        let palette = build_tui_slash_palette(BuildTuiSlashPaletteArgs {
            input: "/help",
            catalog: &TuiSlashEntityCatalog::default(),
            streaming: false,
            delegation_profiles: &[],
            delegation_templates: &[],
        })
        .expect("help palette should be available");
        let preview = preview_for_selection(&palette, 0).expect("preview should resolve");
        assert!(preview.title.starts_with("/help"));
    }

    #[test]
    fn json_tag_reader_extracts_string_values_only() {
        let value = json!({ "tags": ["undo_safe", 4, "manual"] });
        assert_eq!(
            read_json_tags(&value, "/tags"),
            vec!["undo_safe".to_owned(), "manual".to_owned()]
        );
    }

    #[test]
    fn workspace_palette_offers_artifact_show_suggestions() {
        let palette = build_tui_slash_palette(BuildTuiSlashPaletteArgs {
            input: "/workspace show src",
            catalog: &TuiSlashEntityCatalog {
                workspace_artifacts: vec![TuiSlashWorkspaceArtifactRecord {
                    artifact_id: "artifact-1".to_owned(),
                    path: "src/lib.rs".to_owned(),
                    display_path: "src/lib.rs".to_owned(),
                    change_kind: "modified".to_owned(),
                    latest_checkpoint_id: "checkpoint-1".to_owned(),
                    preview_kind: "text".to_owned(),
                    size_bytes: Some(1024),
                    deleted: false,
                }],
                ..TuiSlashEntityCatalog::default()
            },
            streaming: false,
            delegation_profiles: &[],
            delegation_templates: &[],
        })
        .expect("workspace palette should be available");
        assert_eq!(palette.suggestions.len(), 1);
        assert_eq!(palette.suggestions[0].replacement, "/workspace show artifact-1");
    }

    #[test]
    fn rollback_palette_offers_workspace_restore_confirmation_suggestions() {
        let palette = build_tui_slash_palette(BuildTuiSlashPaletteArgs {
            input: "/rollback restore work",
            catalog: &TuiSlashEntityCatalog {
                workspace_checkpoints: vec![TuiSlashWorkspaceCheckpointRecord {
                    checkpoint_id: "workspace-1".to_owned(),
                    source_label: "filesystem_write".to_owned(),
                    summary_text: "src/lib.rs changed".to_owned(),
                    restore_count: 0,
                    created_at_unix_ms: 42,
                }],
                ..TuiSlashEntityCatalog::default()
            },
            streaming: false,
            delegation_profiles: &[],
            delegation_templates: &[],
        })
        .expect("rollback palette should be available");
        assert_eq!(palette.suggestions.len(), 1);
        assert_eq!(palette.suggestions[0].replacement, "/rollback restore workspace-1 --confirm");
    }
}
