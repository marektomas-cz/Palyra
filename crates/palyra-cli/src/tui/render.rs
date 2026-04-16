use super::*;

pub(super) fn render(frame: &mut Frame<'_>, app: &App) {
    let composer_view =
        app.composer.render(MAX_COMPOSER_VISIBLE_LINES, matches!(app.focus, Focus::Input));
    let input_height = estimate_input_height(app, &composer_view);
    let footer_height = if app.show_tips { 3 } else { 2 };
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(input_height),
            Constraint::Length(footer_height),
        ])
        .split(frame.area());
    let header = areas[0];
    let main = areas[1];
    let input = areas[2];
    let footer = areas[3];

    render_header(frame, header, app);
    render_transcript(frame, main, app);
    render_input(frame, input, app, &composer_view);
    render_footer(frame, footer, app);

    match app.mode {
        Mode::Help => render_help_popup(frame, frame.area(), app),
        Mode::Approval => render_approval_popup(frame, frame.area(), app),
        Mode::Picker(_) => render_picker_popup(frame, frame.area(), app),
        Mode::Settings => render_settings_popup(frame, frame.area(), app),
        Mode::ShellConfirm => render_shell_confirm_popup(frame, frame.area()),
        Mode::Chat => {
            if app.pending_slash_palette.is_some() {
                render_slash_palette_popup(frame, frame.area(), app);
            }
        }
    }
}

pub(super) fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let profile = app::current_root_context().and_then(|context| context.active_profile_context());
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(area);
    let top = rows[0];
    let bottom = rows[1];
    let banner = rows[2];
    let connection_line = Line::from(vec![
        Span::styled("Gateway ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(app.runtime.connection().grpc_url.as_str()),
        Span::raw("  "),
        Span::styled("Session ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(display_session_identity(&app.session)),
    ]);
    let status_line = Line::from(vec![
        Span::styled("Agent ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(app.current_session_agent_display()),
        Span::raw("  "),
        Span::styled("Model ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::raw(app.current_session_model_display()),
        Span::raw("  "),
        Span::styled("Status ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        Span::raw(sanitize_terminal_text(app.status_line.as_str())),
    ]);
    let profile_line = if let Some(profile) = profile {
        Line::from(vec![
            Span::styled(
                "Profile ",
                Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD),
            ),
            Span::raw(profile.label),
            Span::raw("  "),
            Span::styled(
                "Env ",
                Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD),
            ),
            Span::raw(profile.environment),
            Span::raw("  "),
            Span::styled(
                "Risk ",
                Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD),
            ),
            Span::raw(profile.risk_level),
            Span::raw("  "),
            Span::styled(
                "Strict ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw(if profile.strict_mode { "on" } else { "off" }),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                "Profile ",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
            ),
            Span::raw("none"),
        ])
    };
    frame.render_widget(Paragraph::new(connection_line), top);
    frame.render_widget(Paragraph::new(status_line), bottom);
    frame.render_widget(Paragraph::new(profile_line), banner);
}

pub(super) fn render_transcript(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = Vec::new();
    for entry in &app.transcript {
        if matches!(entry.kind, EntryKind::Tool) && !app.show_tools {
            continue;
        }
        if matches!(entry.kind, EntryKind::Thinking) && !app.show_thinking {
            continue;
        }
        if matches!(entry.kind, EntryKind::System) && !app.show_verbose {
            continue;
        }
        let style = entry_style(&entry.kind);
        lines.push(Line::from(Span::styled(
            format!("[{}]", entry.title),
            style.add_modifier(Modifier::BOLD),
        )));
        for chunk in entry.body.lines() {
            lines.push(Line::from(Span::styled(format!("  {}", chunk), style)));
        }
        lines.push(Line::default());
    }
    let transcript = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(
            if matches!(app.focus, Focus::Transcript) {
                "Transcript [focus]"
            } else {
                "Transcript"
            },
        ))
        .scroll((app.scroll_offset, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(transcript, area);
}

pub(super) fn render_input(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    composer_view: &TuiComposerView,
) {
    let budget = app.context_budget_summary();
    let workspace_checkpoint_count = app.slash_entity_catalog.workspace_checkpoints.len();
    let block =
        Block::default().borders(Borders::ALL).title(if matches!(app.focus, Focus::Input) {
            format!(
                "Composer [focus · {} line{} · ws {}]",
                composer_view.total_lines,
                if composer_view.total_lines == 1 { "" } else { "s" },
                workspace_checkpoint_count
            )
        } else {
            format!(
                "Composer [{} line{} · ws {}]",
                composer_view.total_lines,
                if composer_view.total_lines == 1 { "" } else { "s" },
                workspace_checkpoint_count
            )
        });
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines = vec![Line::from(vec![
        Span::styled("Budget ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(
            budget.label.clone(),
            match budget.tone {
                StatusTone::Default => Style::default(),
                StatusTone::Warning => Style::default().fg(Color::Yellow),
                StatusTone::Danger => Style::default().fg(Color::LightRed),
            },
        ),
        Span::raw("  "),
        Span::styled("Context ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(
            app.current_session_catalog
                .as_ref()
                .map(|session| session.recap.active_context_files.len().to_string())
                .unwrap_or_else(|| "0".to_owned()),
        ),
        Span::raw("  "),
        Span::styled("Approvals ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(
            app.current_session_catalog
                .as_ref()
                .map(|session| session.pending_approvals.to_string())
                .unwrap_or_else(|| "0".to_owned()),
        ),
        Span::raw("  "),
        Span::styled("Attach ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::raw(app.pending_attachments.len().to_string()),
    ])];
    if !app.pending_attachments.is_empty() {
        let preview = app
            .pending_attachments
            .iter()
            .take(2)
            .enumerate()
            .map(|(index, attachment)| {
                format!(
                    "{}={} · {} · {}",
                    index + 1,
                    attachment.filename,
                    format_size_bytes(attachment.size_bytes),
                    format_approx_tokens(attachment.budget_tokens)
                )
            })
            .collect::<Vec<_>>()
            .join("  ");
        lines.push(Line::from(format!(
            "Pending attachments: {}{}",
            preview,
            if app.pending_attachments.len() > 2 {
                format!("  +{}", app.pending_attachments.len() - 2)
            } else {
                String::new()
            }
        )));
    }
    lines.extend(composer_view.lines.clone());
    if let Some(warning) = budget.warning.as_ref() {
        lines.push(Line::from(Span::styled(
            warning.clone(),
            match budget.tone {
                StatusTone::Danger => Style::default().fg(Color::LightRed),
                StatusTone::Warning => Style::default().fg(Color::Yellow),
                StatusTone::Default => Style::default(),
            },
        )));
    }
    frame.render_widget(Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }), inner);
    if matches!(app.focus, Focus::Input) && matches!(app.mode, Mode::Chat) {
        let cursor_y = inner
            .y
            .saturating_add(1)
            .saturating_add(u16::from(!app.pending_attachments.is_empty()))
            .saturating_add(composer_view.cursor_y);
        frame.set_cursor_position((inner.x + composer_view.cursor_x, cursor_y));
    }
}

pub(super) fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            std::iter::repeat(Constraint::Length(1)).take(area.height as usize).collect::<Vec<_>>(),
        )
        .split(area);
    let width = area.width as usize;
    if let Some(row) = rows.first().copied() {
        frame.render_widget(Paragraph::new(compact_status_bar_line(app, width)), row);
    }
    if let Some(row) = rows.get(1).copied() {
        frame.render_widget(Paragraph::new(compact_shortcut_line(app, width)), row);
    }
    if app.show_tips {
        if let Some(row) = rows.get(2).copied() {
            frame.render_widget(Paragraph::new(discoverability_tip_line(app, width)), row);
        }
    }
}

pub(super) fn strict_profile_blocks_local_shell() -> bool {
    app::current_root_context()
        .map(|context| context.strict_profile_mode() && !context.allow_strict_profile_actions)
        .unwrap_or(false)
}

pub(super) fn estimate_input_height(app: &App, composer_view: &TuiComposerView) -> u16 {
    let mut height = 2 + composer_view.lines.len() as u16 + 1;
    if !app.pending_attachments.is_empty() {
        height += 1;
    }
    if app.context_budget_summary().warning.is_some() {
        height += 1;
    }
    height.clamp(4, 10)
}

pub(super) fn compact_status_bar_line(app: &App, width: usize) -> String {
    let budget = app.context_budget_summary();
    let context_fill = (budget.ratio * 100.0).round().clamp(0.0, 999.0);
    let run_duration = current_run_duration_ms(app).map(format_duration_ms);
    let connection_posture = connection_posture_label(app.runtime.connection().grpc_url.as_str());
    let checkpoint_count = app.slash_entity_catalog.workspace_checkpoints.len();
    let workspace_artifact_count = app.slash_entity_catalog.workspace_artifacts.len().max(
        app.current_session_catalog
            .as_ref()
            .map(|session| session.artifact_count)
            .unwrap_or_default(),
    );
    let segments = vec![
        format!("ctx {:>3.0}%", context_fill),
        format!("tok {}", format_approx_tokens(app.session_runtime.latest_run_total_tokens)),
        format!("budget {}", budget.label),
        app.session_runtime
            .estimated_cost_usd
            .map(format_cost_usd)
            .map(|value| format!("cost {value}"))
            .unwrap_or_else(|| "cost n/a".to_owned()),
        run_duration.map(|value| format!("run {value}")).unwrap_or_else(|| "run idle".to_owned()),
        format!(
            "approvals {}",
            app.current_session_catalog
                .as_ref()
                .map(|session| session.pending_approvals)
                .unwrap_or_default()
        ),
        format!("bg {}", app.session_runtime.active_background_task_count),
        format!("ws {workspace_artifact_count}/{checkpoint_count}"),
        format!("agent {}", app.current_session_agent_display()),
        format!("model {}", app.current_session_model_display()),
        format!("conn {}", connection_posture),
    ];
    fit_segments_to_width(width, segments)
}

pub(super) fn compact_shortcut_line(app: &App, width: usize) -> String {
    let slash_hint =
        if app.pending_slash_palette.is_some() { "slash Up/Down + Tab" } else { "/ commands" };
    let line = format!(
        "Enter send · Alt+Enter/Ctrl+J newline · Ctrl+O attach · /status detail · /workspace · /rollback diff · F8 tips · {slash_hint}"
    );
    truncate_text(line, width.max(8))
}

pub(super) fn discoverability_tip_line(app: &App, width: usize) -> String {
    let raw = if app.pending_approval.is_some() {
        "Tip: a pending approval is waiting; use y / n or Esc to resolve it."
    } else if !app.pending_attachments.is_empty() {
        "Tip: attachments are queued for the next turn; use /attach remove <index> before sending."
    } else if app
        .current_session_catalog
        .as_ref()
        .map(|session| session.family.family_size > 1)
        .unwrap_or(false)
    {
        "Tip: use /resume parent, /resume sibling, /resume child, or /history <family> to move around the current title family."
    } else if !app.slash_entity_catalog.workspace_checkpoints.is_empty() {
        "Tip: /rollback diff <checkpoint-id> previews recoverable workspace changes before restore."
    } else if !app.slash_entity_catalog.checkpoints.is_empty() {
        "Tip: /undo restores the latest safe conversation checkpoint, and /checkpoint list shows the rest."
    } else {
        "Tip: Alt+Enter/Ctrl+J adds a newline, Ctrl+O primes attachment upload, and F8 hides or re-shows these hints. Voice remains desktop-first."
    };
    truncate_text(raw.to_owned(), width.max(8))
}

pub(super) fn current_run_duration_ms(app: &App) -> Option<i64> {
    let started_at = app.session_runtime.latest_started_at_unix_ms?;
    let finished_at = if app.active_stream.is_some() {
        now_unix_ms_i64().ok()
    } else {
        app.session_runtime.latest_completed_at_unix_ms
    };
    Some(finished_at.unwrap_or(started_at) - started_at)
}

pub(super) fn connection_posture_label(grpc_url: &str) -> &'static str {
    let normalized = grpc_url.to_ascii_lowercase();
    if normalized.contains("127.0.0.1")
        || normalized.contains("localhost")
        || normalized.contains("[::1]")
    {
        "local"
    } else {
        "remote"
    }
}

pub(super) fn fit_segments_to_width(width: usize, segments: Vec<String>) -> String {
    let mut line = String::new();
    for segment in segments.into_iter().filter(|segment| !segment.trim().is_empty()) {
        let candidate = if line.is_empty() { segment } else { format!("{line}  {segment}") };
        if candidate.chars().count() > width {
            break;
        }
        line = candidate;
    }
    if line.is_empty() {
        String::new()
    } else {
        line
    }
}

#[derive(Debug, Default)]
pub(super) struct TuiCompactionCommand {
    pub(super) apply: bool,
    pub(super) history: bool,
    pub(super) accept_candidate_ids: Vec<String>,
    pub(super) reject_candidate_ids: Vec<String>,
}

pub(super) fn parse_tui_compaction_arguments(arguments: &[String]) -> Result<TuiCompactionCommand> {
    let mut command = TuiCompactionCommand::default();
    let mut index = 0usize;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "preview" => {}
            "apply" => command.apply = true,
            "history" => command.history = true,
            "--accept" => {
                let candidate_id = arguments
                    .get(index + 1)
                    .cloned()
                    .context("Usage: /compact [preview|apply|history] [--accept <candidate_id>] [--reject <candidate_id>]")?;
                command.accept_candidate_ids.push(candidate_id);
                index += 1;
            }
            "--reject" => {
                let candidate_id = arguments
                    .get(index + 1)
                    .cloned()
                    .context("Usage: /compact [preview|apply|history] [--accept <candidate_id>] [--reject <candidate_id>]")?;
                command.reject_candidate_ids.push(candidate_id);
                index += 1;
            }
            other => anyhow::bail!(
                "unknown /compact argument `{other}`; use preview, apply, history, --accept, or --reject"
            ),
        }
        index += 1;
    }
    if command.history
        && (command.apply
            || !command.accept_candidate_ids.is_empty()
            || !command.reject_candidate_ids.is_empty())
    {
        anyhow::bail!("history cannot be combined with apply or candidate review flags");
    }
    Ok(command)
}

pub(super) fn parse_tui_json_string(value: &str) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(value).ok()
}

pub(super) fn estimate_text_tokens(text: &str) -> u64 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    ((trimmed.len() as u64) + 3) / 4
}

pub(super) fn format_approx_tokens(value: u64) -> String {
    if value >= 1_000 {
        if value >= 10_000 {
            format!("{}k tok", value / 1_000)
        } else {
            format!("{:.1}k tok", (value as f64) / 1_000.0)
        }
    } else {
        format!("{value} tok")
    }
}

pub(super) fn format_size_bytes(value: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value_f64 = value as f64;
    if value_f64 >= GIB {
        format!("{:.1} GiB", value_f64 / GIB)
    } else if value_f64 >= MIB {
        format!("{:.1} MiB", value_f64 / MIB)
    } else if value_f64 >= KIB {
        format!("{:.1} KiB", value_f64 / KIB)
    } else {
        format!("{value} B")
    }
}

pub(super) fn format_duration_ms(value: i64) -> String {
    let value = value.max(0);
    if value >= 60_000 {
        let minutes = value / 60_000;
        let seconds = (value % 60_000) / 1_000;
        format!("{minutes}m {seconds:02}s")
    } else if value >= 1_000 {
        format!("{:.1}s", (value as f64) / 1_000.0)
    } else {
        format!("{value}ms")
    }
}

pub(super) fn format_cost_usd(value: f64) -> String {
    if value >= 1.0 {
        format!("${value:.2}")
    } else {
        format!("${value:.4}")
    }
}

pub(super) fn guess_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "txt" | "md" | "rs" | "toml" | "json" | "yaml" | "yml" | "ts" | "tsx" | "js" | "jsx"
        | "py" | "sh" | "ps1" => "text/plain",
        "pdf" => "application/pdf",
        "csv" => "text/csv",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

pub(super) fn attachment_kind_label(content_type: &str) -> &'static str {
    if content_type.starts_with("image/") {
        "image"
    } else if content_type.starts_with("audio/") {
        "audio"
    } else if content_type.starts_with("video/") {
        "video"
    } else {
        "file"
    }
}

pub(super) fn attachment_kind_to_proto(
    kind: &str,
) -> common_v1::message_attachment::AttachmentKind {
    match kind.to_ascii_lowercase().as_str() {
        "image" => common_v1::message_attachment::AttachmentKind::Image,
        "audio" => common_v1::message_attachment::AttachmentKind::Audio,
        "video" => common_v1::message_attachment::AttachmentKind::Video,
        "file" => common_v1::message_attachment::AttachmentKind::File,
        _ => common_v1::message_attachment::AttachmentKind::Unspecified,
    }
}

pub(super) fn read_json_optional_i64(value: &serde_json::Value, pointer: &str) -> Option<i64> {
    value.pointer(pointer).and_then(serde_json::Value::as_i64)
}

pub(super) fn read_json_u64(value: &serde_json::Value, pointer: &str) -> u64 {
    value.pointer(pointer).and_then(serde_json::Value::as_u64).unwrap_or_default()
}

pub(super) fn read_json_f64(value: &serde_json::Value, pointer: &str) -> Option<f64> {
    value.pointer(pointer).and_then(|entry| {
        entry
            .as_f64()
            .or_else(|| entry.as_i64().map(|number| number as f64))
            .or_else(|| entry.as_u64().map(|number| number as f64))
    })
}

pub(super) fn background_task_is_active(state: &str) -> bool {
    !matches!(
        state.trim().to_ascii_lowercase().as_str(),
        "" | "completed" | "failed" | "cancelled" | "canceled" | "succeeded"
    )
}

pub(super) fn map_session_catalog_record(session: SessionCatalogRecord) -> TuiSlashSessionRecord {
    TuiSlashSessionRecord {
        session_id: session.session_id,
        title: session.title,
        session_key: session.session_key,
        archived: session.archived,
        preview: session.preview.unwrap_or_default(),
        root_title: session.family.root_title,
        last_summary: session.last_summary.unwrap_or_default(),
        branch_state: session.branch_state,
        family_sequence: session.family.sequence,
        family_size: session.family.family_size,
        parent_session_id: session.family.parent_session_id,
        parent_title: session.family.parent_title,
        pending_approvals: session.pending_approvals,
        artifact_count: session.artifact_count,
        active_context_files: session.recap.active_context_files.len(),
        relatives: session
            .family
            .relatives
            .into_iter()
            .map(|relative| TuiSlashSessionRelative {
                session_id: relative.session_id,
                title: relative.title,
                branch_state: relative.branch_state,
                relation: relative.relation,
            })
            .collect(),
    }
}

pub(super) struct TuiObjectiveCreateSpec {
    pub(super) kind: &'static str,
    pub(super) name: String,
    pub(super) prompt: String,
}

pub(super) fn resolve_tui_objective_reference(
    explicit: Option<String>,
    selected: Option<&String>,
) -> Result<String> {
    explicit
        .or_else(|| selected.cloned())
        .context("Select an objective first or pass an explicit objective_id")
}

pub(super) fn parse_tui_objective_create_spec(
    fixed_kind: Option<&'static str>,
    arguments: &[String],
) -> Result<TuiObjectiveCreateSpec> {
    let create_arguments =
        arguments.get(1..).context("Usage: /objective create <kind> <name> :: <prompt>")?;
    let joined = create_arguments.join(" ");
    let (head, prompt) =
        joined.split_once("::").context("Usage: /objective create <kind> <name> :: <prompt>")?;
    let prompt = prompt.trim();
    if prompt.is_empty() {
        anyhow::bail!("objective prompt cannot be empty");
    }
    let mut head_parts = head.split_whitespace();
    let kind = match fixed_kind {
        Some(kind) => kind,
        None => parse_tui_objective_kind(
            head_parts.next().context("Usage: /objective create <kind> <name> :: <prompt>")?,
        )?,
    };
    let name = head_parts.collect::<Vec<_>>().join(" ").trim().to_owned();
    if name.is_empty() {
        anyhow::bail!("objective name cannot be empty");
    }
    Ok(TuiObjectiveCreateSpec { kind, name, prompt: prompt.to_owned() })
}

pub(super) fn parse_tui_objective_kind(value: &str) -> Result<&'static str> {
    match value.to_ascii_lowercase().as_str() {
        "objective" => Ok("objective"),
        "heartbeat" => Ok("heartbeat"),
        "standing-order" | "standing_order" => Ok("standing_order"),
        "program" => Ok("program"),
        other => anyhow::bail!(
            "unknown objective kind `{other}`; use objective, heartbeat, standing-order, or program"
        ),
    }
}

pub(super) fn render_help_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = vec![Line::from("Slash commands")];
    for line in render_shared_chat_command_synopsis_lines(SharedChatCommandSurface::Tui, 84) {
        lines.push(Line::from(line));
    }
    lines.extend([
        Line::default(),
        Line::from("Recommended now"),
        Line::from(discoverability_tip_line(app, 88)),
        Line::default(),
        Line::from("Multiline composer"),
        Line::from("  Enter send  Alt+Enter or Ctrl+J newline"),
        Line::from("  Ctrl+A select all  Ctrl+O seed /attach  Tab switch focus"),
    ]);
    lines.extend([
        Line::default(),
        Line::from("Daily-driver examples"),
        Line::from("  /attach ./logs/failure.txt   /resume parent   /status detail"),
        Line::from("  /workspace   /workspace show 1   /workspace handoff open"),
        Line::from("  /rollback diff <checkpoint>   /rollback restore <checkpoint> --confirm"),
        Line::default(),
        Line::from("Context references"),
        Line::from(
            "  @file:PATH @folder:PATH @diff[:PATH] @staged[:PATH] @url:URL @memory:\"query\"",
        ),
        Line::from("  Escape a literal at-sign with @@"),
        Line::default(),
        Line::from("Pickers"),
        Line::from("  F2 agent  F3 session  F4 model"),
        Line::from("  F5 settings  Ctrl+R reload runtime state  F8 toggle tips"),
        Line::default(),
        Line::from("Controls"),
        Line::from("  Tab focus or accept slash suggestion  Up/Down navigate palette or composer"),
        Line::from("  q quit when the composer is empty  Esc close overlay  /status detail expands context"),
        Line::default(),
        Line::from(
            "Retry, undo, attachments, recap family navigation, workspace explorer, rollback, and background tasks all reuse the console HTTP contracts.",
        ),
        Line::from("Voice remains desktop-first until the CLI path is fully stable."),
    ]);
    let popup_height = area.height.saturating_sub(2).min(lines.len() as u16 + 2).max(14);
    let popup = centered_rect(88, popup_height, area);
    let text = Text::from(lines);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
        popup,
    );
}

pub(super) fn percent_encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(*byte));
            }
            other => {
                encoded.push('%');
                encoded.push_str(format!("{other:02X}").as_str());
            }
        }
    }
    encoded
}

pub(super) fn render_approval_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered_rect(88, 14, area);
    let body = if let Some(approval) = app.pending_approval.as_ref() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    "Tool ",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::raw(sanitize_terminal_text(approval.tool_name.as_str())),
            ]),
            Line::default(),
            Line::from(sanitize_terminal_text(approval.request_summary.as_str())),
        ];
        if let Some(prompt) = approval.prompt.as_ref() {
            lines.push(Line::from(text::approval_risk(
                app.locale,
                approval_risk_level_to_text(prompt.risk_level),
            )));
            if !prompt.policy_explanation.trim().is_empty() {
                lines.push(Line::from(text::approval_policy(
                    app.locale,
                    prompt.policy_explanation.as_str(),
                )));
            }
            if !prompt.summary.trim().is_empty() {
                lines.push(Line::from(prompt.summary.clone()));
            }
        }
        lines.push(Line::default());
        lines.push(Line::from(text::approval_manage_posture_hint(app.locale)));
        lines.push(Line::default());
        lines.push(Line::from(text::approval_allow_once_hint(app.locale)));
        lines.push(Line::from(text::approval_deny_hint(app.locale)));
        Text::from(lines)
    } else {
        Text::from(text::approval_request_unavailable(app.locale))
    };
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(body)
            .block(Block::default().borders(Borders::ALL).title("Approval Required"))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

pub(super) fn approval_risk_level_to_text(raw: i32) -> &'static str {
    match common_v1::ApprovalRiskLevel::try_from(raw)
        .unwrap_or(common_v1::ApprovalRiskLevel::Unspecified)
    {
        common_v1::ApprovalRiskLevel::Low => "low",
        common_v1::ApprovalRiskLevel::Medium => "medium",
        common_v1::ApprovalRiskLevel::High => "high",
        common_v1::ApprovalRiskLevel::Critical => "critical",
        common_v1::ApprovalRiskLevel::Unspecified => "unspecified",
    }
}

pub(super) fn render_picker_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered_rect(88, 18, area);
    frame.render_widget(Clear, popup);
    if let Some(picker) = app.pending_picker.as_ref() {
        let mut lines = Vec::new();
        for (index, item) in picker.items.iter().enumerate() {
            let prefix = if index == picker.selected { ">" } else { " " };
            lines.push(Line::from(Span::styled(
                format!("{prefix} {}", item.title),
                if index == picker.selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            )));
            lines.push(Line::from(format!("  {}", item.detail)));
            lines.push(Line::default());
        }
        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .block(Block::default().borders(Borders::ALL).title(picker.title.as_str()))
                .wrap(Wrap { trim: false }),
            popup,
        );
    }
}

pub(super) fn render_slash_palette_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(palette) = app.pending_slash_palette.as_ref() else {
        return;
    };
    let preview = preview_for_selection(palette, app.slash_palette_selected);
    let preview_lines = if let Some(preview) = preview {
        vec![
            Line::from(vec![
                Span::styled(
                    preview.badge,
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::raw(preview.title),
            ]),
            Line::from(preview.subtitle),
            Line::from(preview.detail),
            Line::from(format!("Example: {}", preview.example)),
            Line::default(),
        ]
    } else {
        vec![
            Line::from("Slash command palette"),
            Line::from("Type a slash command, then use Up/Down and Tab."),
            Line::default(),
        ]
    };
    let mut lines = preview_lines;
    if palette.suggestions.is_empty() {
        lines.push(Line::from("No suggestions for the current token."));
    } else {
        for (index, suggestion) in palette.suggestions.iter().enumerate() {
            let selected = index == app.slash_palette_selected;
            let prefix = if selected { ">" } else { " " };
            lines.push(Line::from(Span::styled(
                format!("{prefix} [{}] {}", suggestion.badge, suggestion.title),
                if selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            )));
            lines.push(Line::from(format!("  {}", suggestion.subtitle)));
            lines.push(Line::from(format!("  {}", suggestion.detail)));
            lines.push(Line::default());
        }
    }
    let popup = centered_rect(92, 18, area);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title("Slash palette"))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

pub(super) fn render_settings_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered_rect(56, 12, area);
    let items = settings_items();
    let mut lines = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let selected = index == app.settings_selected;
        let prefix = if selected { ">" } else { " " };
        let (label, enabled) = match item {
            SettingsItem::ShowTools => ("Show tool cards", app.show_tools),
            SettingsItem::ShowThinking => ("Show thinking/status lines", app.show_thinking),
            SettingsItem::ShowVerbose => ("Show verbose/system lines", app.show_verbose),
            SettingsItem::LocalShell => ("Enable local shell", app.local_shell_enabled),
        };
        lines.push(Line::from(Span::styled(
            format!("{prefix} [{enabled}] {label}"),
            if selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )));
    }
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title("Settings"))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

pub(super) fn render_shell_confirm_popup(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered_rect(64, 8, area);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(Text::from(vec![
            Line::from("Local shell is disabled by default."),
            Line::default(),
            Line::from("Press y / Enter to enable it for this TUI session."),
            Line::from("Press n / Esc to keep it disabled."),
        ]))
        .block(Block::default().borders(Borders::ALL).title("Local Shell Opt-In"))
        .wrap(Wrap { trim: false }),
        popup,
    );
}

pub(super) fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(width.min(area.width.saturating_sub(2))),
            Constraint::Percentage(50),
        ])
        .split(area);
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(height.min(area.height.saturating_sub(2))),
            Constraint::Percentage(50),
        ])
        .split(horizontal[1]);
    vertical[1]
}

pub(super) fn settings_items() -> [SettingsItem; 4] {
    [
        SettingsItem::ShowTools,
        SettingsItem::ShowThinking,
        SettingsItem::ShowVerbose,
        SettingsItem::LocalShell,
    ]
}

pub(super) fn entry_style(kind: &EntryKind) -> Style {
    match kind {
        EntryKind::User => Style::default().fg(Color::Cyan),
        EntryKind::Assistant => Style::default().fg(Color::White),
        EntryKind::Tool => Style::default().fg(Color::Yellow),
        EntryKind::Thinking => Style::default().fg(Color::DarkGray),
        EntryKind::System => Style::default().fg(Color::Green),
        EntryKind::Shell => Style::default().fg(Color::Magenta),
    }
}

pub(super) fn display_session_identity(session: &gateway_v1::SessionSummary) -> String {
    if !session.title.trim().is_empty() {
        return session.title.clone();
    }
    if !session.session_label.trim().is_empty() {
        return session.session_label.clone();
    }
    if !session.session_key.trim().is_empty() {
        return session.session_key.clone();
    }
    if session.session_id.is_some() {
        "session".to_owned()
    } else {
        "unknown session".to_owned()
    }
}

pub(super) fn normalize_owned_optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub(super) fn shorten_id(value: &str) -> String {
    redacted_identifier_for_output(value)
}

pub(super) fn parse_toggle(value: Option<&str>, current: bool) -> Result<bool> {
    match value.unwrap_or("toggle") {
        "toggle" => Ok(!current),
        "on" | "true" | "yes" => Ok(true),
        "off" | "false" | "no" => Ok(false),
        other => Err(anyhow!("unsupported toggle value: {other}")),
    }
}

pub(super) fn quick_control_reset_requested(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "default" | "reset" | "inherit")
}

pub(super) fn looks_like_canonical_ulid(value: &str) -> bool {
    value.len() == 26 && value.chars().all(|ch| ch.is_ascii_alphanumeric())
}

pub(super) fn session_reference_equals(candidate: &str, reference: &str) -> bool {
    !candidate.trim().is_empty() && candidate.eq_ignore_ascii_case(reference)
}

pub(super) fn session_reference_matches(session: &TuiSlashSessionRecord, reference: &str) -> bool {
    session_reference_equals(session.session_id.as_str(), reference)
        || session_reference_equals(session.session_key.as_str(), reference)
        || session_reference_equals(session.title.as_str(), reference)
        || session_reference_equals(session.root_title.as_str(), reference)
}

pub(super) fn describe_branch_state_label(branch_state: &str) -> String {
    match branch_state {
        "root" => "root".to_owned(),
        "active_branch" | "branched" => "branch".to_owned(),
        "branch_source" => "branch source".to_owned(),
        "missing" => "no lineage".to_owned(),
        other => other.replace('_', " "),
    }
}

pub(super) fn agent_resolution_source_label(raw: i32) -> &'static str {
    match gateway_v1::AgentResolutionSource::try_from(raw)
        .unwrap_or(gateway_v1::AgentResolutionSource::Unspecified)
    {
        gateway_v1::AgentResolutionSource::SessionBinding => "session_binding",
        gateway_v1::AgentResolutionSource::Default => "default",
        gateway_v1::AgentResolutionSource::Fallback => "fallback",
        gateway_v1::AgentResolutionSource::Unspecified => "unspecified",
    }
}

pub(super) async fn run_local_shell(command: String) -> Result<ShellResult> {
    tokio::task::spawn_blocking(move || {
        #[cfg(windows)]
        let output = {
            let shell = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_owned());
            std::process::Command::new(shell).arg("/C").arg(command.as_str()).output()
        };
        #[cfg(not(windows))]
        let output = std::process::Command::new("sh").arg("-lc").arg(command.as_str()).output();

        let output = output.context("failed to execute local shell command")?;
        Ok::<ShellResult, anyhow::Error>(ShellResult {
            exit_code: output.status.code(),
            stdout: truncate_text(
                String::from_utf8_lossy(output.stdout.as_slice()).to_string(),
                1_500,
            ),
            stderr: truncate_text(
                String::from_utf8_lossy(output.stderr.as_slice()).to_string(),
                1_500,
            ),
        })
    })
    .await
    .context("local shell worker failed")?
}

pub(super) fn truncate_text(mut value: String, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value;
    }
    value = value.chars().take(limit).collect::<String>();
    value.push_str("...");
    value
}

pub(super) fn sanitize_terminal_text(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut just_saw_carriage_return = false;
    for ch in value.chars() {
        match ch {
            '\n' => {
                if !just_saw_carriage_return {
                    sanitized.push('\n');
                }
                just_saw_carriage_return = false;
            }
            '\r' => {
                if !sanitized.ends_with('\n') {
                    sanitized.push('\n');
                }
                just_saw_carriage_return = true;
            }
            '\u{1b}' => {
                sanitized.push_str("<ESC>");
                just_saw_carriage_return = false;
            }
            ch if ch.is_control() => {
                sanitized.push_str(format!("<U+{:04X}>", ch as u32).as_str());
                just_saw_carriage_return = false;
            }
            ch => {
                sanitized.push(ch);
                just_saw_carriage_return = false;
            }
        }
    }
    sanitized
}

pub(super) fn format_shell_result(result: &ShellResult) -> String {
    let mut body = format!(
        "exit_code={}\n",
        result.exit_code.map(|value| value.to_string()).unwrap_or_else(|| "unknown".to_owned())
    );
    if !result.stdout.trim().is_empty() {
        body.push_str("stdout:\n");
        body.push_str(result.stdout.trim());
        body.push('\n');
    }
    if !result.stderr.trim().is_empty() {
        body.push_str("stderr:\n");
        body.push_str(result.stderr.trim());
    }
    if body.trim().is_empty() {
        "no output".to_owned()
    } else {
        body.trim_end().to_owned()
    }
}
