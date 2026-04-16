use std::{
    collections::BTreeMap,
    fs,
    io::{self, Stdout},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use palyra_control_plane::{SessionCatalogRecord, SessionQuickControlsUpdateRequest};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};

mod composer;
mod handoff;
mod rollback;
mod slash_palette;
mod text;
mod workspace;

use composer::{TuiComposer, TuiComposerView};
use handoff::{build_console_handoff_path, TuiCrossSurfaceHandoff};
use slash_palette::{
    build_tui_slash_palette, checkpoint_has_tag, preview_for_selection, read_json_bool,
    read_json_i64, read_json_string, read_json_tags, select_undo_checkpoint,
    BuildTuiSlashPaletteArgs, TuiSlashAuthProfileRecord, TuiSlashBrowserProfileRecord,
    TuiSlashBrowserSessionRecord, TuiSlashCheckpointRecord, TuiSlashEntityCatalog,
    TuiSlashObjectiveRecord, TuiSlashPaletteState, TuiSlashSessionRecord, TuiSlashSessionRelative,
    TuiSlashWorkspaceArtifactRecord, TuiSlashWorkspaceCheckpointRecord, TuiUxMetricKey,
    TuiUxMetrics,
};
use text::{resolve_tui_locale, TuiLocale};

use crate::{
    client::operator::{ManagedRunStream, OperatorRuntime},
    commands::models::ModelsListPayload,
    shared_chat_commands::{
        render_shared_chat_command_synopsis_lines, resolve_shared_chat_command_name,
        SharedChatCommandSurface,
    },
    *,
};

#[derive(Clone)]
pub(crate) struct LaunchOptions {
    pub(crate) connection: AgentConnection,
    pub(crate) session_id: Option<common_v1::CanonicalId>,
    pub(crate) session_key: Option<String>,
    pub(crate) session_label: Option<String>,
    pub(crate) require_existing: bool,
    pub(crate) allow_sensitive_tools: bool,
    pub(crate) include_archived_sessions: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Transcript,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Chat,
    Help,
    Picker(PickerKind),
    Settings,
    Approval,
    ShellConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickerKind {
    Agent,
    Session,
    Model,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsItem {
    ShowTools,
    ShowThinking,
    ShowVerbose,
    LocalShell,
}

#[derive(Debug, Clone)]
struct PickerItem {
    id: String,
    title: String,
    detail: String,
}

#[derive(Debug, Clone)]
struct PickerState {
    kind: PickerKind,
    title: String,
    items: Vec<PickerItem>,
    selected: usize,
}

#[derive(Debug, Clone)]
enum EntryKind {
    User,
    Assistant,
    Tool,
    Thinking,
    System,
    Shell,
}

#[derive(Debug, Clone)]
struct TranscriptEntry {
    kind: EntryKind,
    title: String,
    body: String,
}

#[derive(Debug, Clone)]
struct ShellResult {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Clone, Default)]
struct ComposerDraftState {
    composer: TuiComposer,
    attachments: Vec<TuiPendingAttachment>,
}

#[derive(Debug, Clone)]
struct TuiPendingAttachment {
    local_id: String,
    artifact_id: String,
    attachment_id: String,
    filename: String,
    declared_content_type: String,
    content_hash: String,
    size_bytes: u64,
    width_px: Option<u64>,
    height_px: Option<u64>,
    kind: String,
    budget_tokens: u64,
    derived_artifacts: usize,
}

#[derive(Debug, Clone, Default)]
struct SessionRuntimeSnapshot {
    session_total_tokens: u64,
    session_runs: usize,
    average_latency_ms: Option<f64>,
    estimated_cost_usd: Option<f64>,
    latest_started_at_unix_ms: Option<i64>,
    latest_completed_at_unix_ms: Option<i64>,
    latest_run_total_tokens: u64,
    attachment_count: usize,
    background_task_count: usize,
    active_background_task_count: usize,
}

#[derive(Debug, Clone)]
struct ContextBudgetSummary {
    draft_tokens: u64,
    project_context_tokens: u64,
    attachment_tokens: u64,
    estimated_total_tokens: u64,
    limit_tokens: u64,
    ratio: f64,
    tone: StatusTone,
    label: String,
    warning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum StatusTone {
    #[default]
    Default,
    Warning,
    Danger,
}

struct App {
    runtime: OperatorRuntime,
    session: gateway_v1::SessionSummary,
    current_session_catalog: Option<SessionCatalogRecord>,
    current_agent: Option<gateway_v1::Agent>,
    current_agent_source: &'static str,
    models: Option<ModelsListPayload>,
    composer: TuiComposer,
    draft_cache: BTreeMap<String, ComposerDraftState>,
    pending_attachments: Vec<TuiPendingAttachment>,
    session_runtime: SessionRuntimeSnapshot,
    transcript: Vec<TranscriptEntry>,
    active_stream: Option<ManagedRunStream>,
    pending_approval: Option<common_v1::ToolApprovalRequest>,
    pending_shell_command: Option<String>,
    pending_picker: Option<PickerState>,
    pending_slash_palette: Option<TuiSlashPaletteState>,
    slash_palette_selected: usize,
    slash_palette_dismissed: bool,
    slash_entity_catalog: TuiSlashEntityCatalog,
    pending_redirect_prompt: Option<PendingRedirectPrompt>,
    focus: Focus,
    mode: Mode,
    show_tools: bool,
    show_thinking: bool,
    show_verbose: bool,
    local_shell_enabled: bool,
    allow_sensitive_tools: bool,
    include_archived_sessions: bool,
    last_run_id: Option<String>,
    selected_objective_id: Option<String>,
    locale: TuiLocale,
    ux_metrics: TuiUxMetrics,
    scroll_offset: u16,
    status_line: String,
    settings_selected: usize,
    show_tips: bool,
}

#[derive(Debug, Clone)]
struct PendingRedirectPrompt {
    prompt: String,
    mode: String,
    interrupted_run_id: String,
}

const BUILT_IN_DELEGATION_PROFILES: &[&str] =
    &["research", "synthesis", "review", "patching", "triage"];
const BUILT_IN_DELEGATION_TEMPLATES: &[&str] =
    &["compare_variants", "research_then_synthesize", "review_and_patch", "multi_source_triage"];
const CONTEXT_BUDGET_SOFT_LIMIT: u64 = 12_000;
const CONTEXT_BUDGET_HARD_LIMIT: u64 = 16_000;
const MAX_COMPOSER_VISIBLE_LINES: usize = 6;

pub(crate) fn run(options: LaunchOptions) -> Result<()> {
    let runtime = build_runtime()?;
    runtime.block_on(async move {
        let mut app = App::bootstrap(options).await?;
        let mut terminal = setup_terminal()?;
        let result = run_loop(&mut terminal, &mut app).await;
        restore_terminal(&mut terminal)?;
        result
    })
}

async fn run_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        app.drain_stream_events().await?;
        terminal.draw(|frame| render(frame, app))?;
        if app.should_exit() {
            return Ok(());
        }
        if event::poll(Duration::from_millis(50)).context("failed to poll terminal events")? {
            let event = event::read().context("failed to read terminal event")?;
            match event {
                CEvent::Key(key) => {
                    if handle_key(app, key).await? {
                        return Ok(());
                    }
                }
                CEvent::Paste(text)
                    if matches!(app.mode, Mode::Chat) && matches!(app.focus, Focus::Input) =>
                {
                    app.composer.insert_text(text.as_str());
                    app.sync_composer_after_edit();
                    app.status_line = "Pasted text into the composer".to_owned();
                }
                _ => {}
            }
        }
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("failed to enable raw terminal mode")?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen).context("failed to enter alternate screen")?;
    Terminal::new(CrosstermBackend::new(stdout)).context("failed to initialize terminal backend")
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("failed to disable raw terminal mode")?;
    terminal
        .backend_mut()
        .execute(LeaveAlternateScreen)
        .context("failed to leave alternate screen")?;
    terminal.show_cursor().context("failed to restore terminal cursor")
}

impl App {
    async fn bootstrap(options: LaunchOptions) -> Result<Self> {
        let runtime = OperatorRuntime::new(options.connection.clone());
        let response = runtime
            .resolve_session(SessionResolveInput {
                session_id: options.session_id,
                session_key: options.session_key.unwrap_or_default(),
                session_label: options.session_label.unwrap_or_default(),
                require_existing: options.require_existing,
                reset_session: false,
            })
            .await?;
        let session = response
            .session
            .context("ResolveSession returned empty session payload for tui bootstrap")?;
        let locale = resolve_tui_locale();
        let mut app = Self {
            runtime,
            session,
            current_session_catalog: None,
            current_agent: None,
            current_agent_source: "unresolved",
            models: None,
            composer: TuiComposer::default(),
            draft_cache: BTreeMap::new(),
            pending_attachments: Vec::new(),
            session_runtime: SessionRuntimeSnapshot::default(),
            transcript: Vec::new(),
            active_stream: None,
            pending_approval: None,
            pending_shell_command: None,
            pending_picker: None,
            pending_slash_palette: None,
            slash_palette_selected: 0,
            slash_palette_dismissed: false,
            slash_entity_catalog: TuiSlashEntityCatalog::default(),
            pending_redirect_prompt: None,
            focus: Focus::Input,
            mode: Mode::Chat,
            show_tools: true,
            show_thinking: true,
            show_verbose: true,
            local_shell_enabled: false,
            allow_sensitive_tools: options.allow_sensitive_tools,
            include_archived_sessions: options.include_archived_sessions,
            last_run_id: None,
            selected_objective_id: None,
            locale,
            ux_metrics: TuiUxMetrics::default(),
            scroll_offset: 0,
            status_line: text::connected(locale),
            settings_selected: 0,
            show_tips: true,
        };
        app.refresh_agent_identity(None, false).await?;
        match app.runtime.list_models(None) {
            Ok(models) => app.models = Some(models),
            Err(error) => {
                app.status_line = sanitize_terminal_text(
                    text::connected_model_catalog_unavailable(
                        app.locale,
                        error.to_string().as_str(),
                    )
                    .as_str(),
                )
            }
        }
        if let Err(error) = app.refresh_slash_entity_catalogs().await {
            app.status_line = sanitize_terminal_text(
                text::connected_slash_catalog_unavailable(app.locale, error.to_string().as_str())
                    .as_str(),
            );
        }
        if let Err(error) = app.refresh_session_runtime_snapshot().await {
            app.status_line = sanitize_terminal_text(
                format!("Connected; runtime metadata unavailable: {error}").as_str(),
            );
        }
        app.sync_slash_palette();
        app.push_entry(EntryKind::System, "Session", text::connected_entry(app.locale));
        Ok(app)
    }

    fn should_exit(&self) -> bool {
        matches!(self.mode, Mode::Chat) && self.status_line == "__exit__"
    }

    async fn drain_stream_events(&mut self) -> Result<()> {
        loop {
            let next = {
                let Some(stream) = self.active_stream.as_mut() else {
                    break;
                };
                tokio::time::timeout(Duration::from_millis(1), stream.next_event()).await
            };
            match next {
                Ok(Ok(Some(event))) => self.handle_stream_event(event)?,
                Ok(Ok(None)) => {
                    self.active_stream = None;
                    if let Ok(now) = now_unix_ms_i64() {
                        self.session_runtime.latest_completed_at_unix_ms = Some(now);
                    }
                    let _ = self.refresh_slash_entity_catalogs().await;
                    let _ = self.refresh_session_runtime_snapshot().await;
                    self.status_line = text::run_completed(self.locale);
                    if let Some(redirect) = self.pending_redirect_prompt.take() {
                        self.push_entry(
                            EntryKind::System,
                            "Redirect",
                            format!(
                                "{} interrupt completed for {}. Starting redirected prompt.\nAny external side effects already emitted remain in the world state.",
                                redirect.mode,
                                shorten_id(redirect.interrupted_run_id.as_str())
                            ),
                        );
                        self.status_line = text::starting_redirected_prompt(self.locale);
                        self.start_prompt_run(
                            redirect.prompt,
                            Some("interrupt_redirect".to_owned()),
                            Some(redirect.interrupted_run_id),
                            None,
                            Vec::new(),
                        )
                        .await?;
                    }
                    break;
                }
                Ok(Err(error)) => {
                    self.active_stream = None;
                    if let Ok(now) = now_unix_ms_i64() {
                        self.session_runtime.latest_completed_at_unix_ms = Some(now);
                    }
                    let _ = self.refresh_slash_entity_catalogs().await;
                    let _ = self.refresh_session_runtime_snapshot().await;
                    self.status_line =
                        sanitize_terminal_text(format!("Run failed: {error}").as_str());
                    self.push_entry(EntryKind::System, "Run error", error.to_string());
                    self.pending_redirect_prompt = None;
                    break;
                }
                Err(_) => break,
            }
        }
        Ok(())
    }

    fn handle_stream_event(&mut self, event: common_v1::RunStreamEvent) -> Result<()> {
        let run_id =
            event.run_id.as_ref().map(|value| value.ulid.as_str()).unwrap_or("unknown").to_owned();
        match event.body {
            Some(common_v1::run_stream_event::Body::ModelToken(token)) => {
                self.append_assistant_token(run_id.as_str(), token.token.as_str());
                if token.is_final {
                    self.status_line = text::assistant_response_completed(self.locale);
                }
            }
            Some(common_v1::run_stream_event::Body::Status(status)) => {
                self.status_line = sanitize_terminal_text(status.message.as_str());
                if self.show_thinking {
                    self.push_entry(
                        EntryKind::Thinking,
                        format!("Status ({})", stream_status_kind_to_text(status.kind)),
                        status.message,
                    );
                }
            }
            Some(common_v1::run_stream_event::Body::ToolProposal(proposal)) => {
                if self.show_tools {
                    self.push_entry(
                        EntryKind::Tool,
                        format!("Tool proposal: {}", proposal.tool_name),
                        format!(
                            "proposal_id={} approval_required={}",
                            proposal
                                .proposal_id
                                .as_ref()
                                .map(|value| value.ulid.as_str())
                                .unwrap_or("unknown"),
                            proposal.approval_required
                        ),
                    );
                }
            }
            Some(common_v1::run_stream_event::Body::ToolDecision(decision)) => {
                if self.show_tools {
                    self.push_entry(
                        EntryKind::Tool,
                        format!("Tool decision: {}", tool_decision_kind_to_text(decision.kind)),
                        decision.reason,
                    );
                }
            }
            Some(common_v1::run_stream_event::Body::ToolApprovalRequest(mut approval)) => {
                approval.tool_name = sanitize_terminal_text(approval.tool_name.as_str());
                approval.request_summary =
                    sanitize_terminal_text(approval.request_summary.as_str());
                if let Some(prompt) = approval.prompt.as_mut() {
                    prompt.title = sanitize_terminal_text(prompt.title.as_str());
                    prompt.subject_id = sanitize_terminal_text(prompt.subject_id.as_str());
                    prompt.summary = sanitize_terminal_text(prompt.summary.as_str());
                    prompt.policy_explanation =
                        sanitize_terminal_text(prompt.policy_explanation.as_str());
                }
                self.status_line = sanitize_terminal_text(
                    text::approval_required(
                        self.locale,
                        if approval.tool_name.trim().is_empty() {
                            "tool"
                        } else {
                            approval.tool_name.as_str()
                        },
                    )
                    .as_str(),
                );
                if self.show_tools {
                    self.push_entry(
                        EntryKind::Tool,
                        format!("Approval requested: {}", approval.tool_name),
                        approval.request_summary.clone(),
                    );
                }
                self.pending_approval = Some(approval);
                self.mode = Mode::Approval;
            }
            Some(common_v1::run_stream_event::Body::ToolApprovalResponse(response)) => {
                if self.show_tools {
                    self.push_entry(
                        EntryKind::Tool,
                        "Approval response",
                        format!(
                            "approved={} scope={} reason={}",
                            response.approved,
                            approval_scope_to_text(response.decision_scope),
                            response.reason
                        ),
                    );
                }
            }
            Some(common_v1::run_stream_event::Body::ToolResult(result)) => {
                if self.show_tools {
                    self.push_entry(
                        EntryKind::Tool,
                        format!(
                            "Tool result ({})",
                            if result.success { "success" } else { "error" }
                        ),
                        if result.error.trim().is_empty() {
                            truncate_text(
                                String::from_utf8_lossy(result.output_json.as_slice()).to_string(),
                                600,
                            )
                        } else {
                            result.error
                        },
                    );
                }
            }
            Some(common_v1::run_stream_event::Body::ToolAttestation(attestation)) => {
                if self.show_tools {
                    self.push_entry(
                        EntryKind::Tool,
                        "Tool attestation",
                        format!(
                            "executor={} timed_out={} proposal_id={}",
                            attestation.executor,
                            attestation.timed_out,
                            attestation
                                .proposal_id
                                .as_ref()
                                .map(|value| value.ulid.as_str())
                                .unwrap_or("unknown")
                        ),
                    );
                }
            }
            Some(common_v1::run_stream_event::Body::A2uiUpdate(update)) => {
                self.push_entry(
                    EntryKind::System,
                    "A2UI update",
                    format!("surface={} version={}", update.surface, update.v),
                );
            }
            Some(common_v1::run_stream_event::Body::JournalEvent(journal_event)) => {
                self.push_entry(
                    EntryKind::System,
                    "Journal event",
                    format!(
                        "{} ({})",
                        journal_event.kind,
                        journal_event
                            .event_id
                            .as_ref()
                            .map(|value| value.ulid.as_str())
                            .unwrap_or("unknown")
                    ),
                );
            }
            None => {
                self.push_entry(EntryKind::System, "Event", "Received empty run-stream event");
            }
        }
        Ok(())
    }

    fn append_assistant_token(&mut self, run_id: &str, token: &str) {
        let title = format!("Assistant ({})", shorten_id(run_id));
        let token = sanitize_terminal_text(token);
        if let Some(last) = self.transcript.last_mut() {
            if matches!(last.kind, EntryKind::Assistant) && last.title == title {
                last.body.push_str(token.as_str());
                return;
            }
        }
        self.transcript.push(TranscriptEntry { kind: EntryKind::Assistant, title, body: token });
    }

    fn push_entry<T: AsRef<str>, U: AsRef<str>>(&mut self, kind: EntryKind, title: T, body: U) {
        self.transcript.push(TranscriptEntry {
            kind,
            title: sanitize_terminal_text(title.as_ref()),
            body: sanitize_terminal_text(body.as_ref()),
        });
    }

    fn current_draft_state(&self) -> ComposerDraftState {
        ComposerDraftState {
            composer: self.composer.clone(),
            attachments: self.pending_attachments.clone(),
        }
    }

    fn stash_current_draft(&mut self) {
        let Ok(session_id) = self.active_session_id() else {
            return;
        };
        if self.composer.trimmed_text().is_empty() && self.pending_attachments.is_empty() {
            self.draft_cache.remove(session_id.as_str());
            return;
        }
        self.draft_cache.insert(session_id, self.current_draft_state());
    }

    fn restore_current_draft(&mut self) {
        let Ok(session_id) = self.active_session_id() else {
            self.composer.clear();
            self.pending_attachments.clear();
            return;
        };
        if let Some(draft) = self.draft_cache.get(session_id.as_str()).cloned() {
            self.composer = draft.composer;
            self.pending_attachments = draft.attachments;
        } else {
            self.composer.clear();
            self.pending_attachments.clear();
        }
    }

    fn clear_current_draft(&mut self) {
        self.composer.clear();
        self.pending_attachments.clear();
        if let Ok(session_id) = self.active_session_id() {
            self.draft_cache.remove(session_id.as_str());
        }
    }

    fn sync_composer_after_edit(&mut self) {
        self.slash_palette_dismissed = false;
        self.sync_slash_palette();
        self.stash_current_draft();
    }

    fn dismiss_slash_palette(&mut self) {
        self.slash_palette_dismissed = true;
        self.pending_slash_palette = None;
        self.slash_palette_selected = 0;
    }

    fn prompt_attachments(&self) -> Vec<common_v1::MessageAttachment> {
        self.pending_attachments
            .iter()
            .map(|attachment| common_v1::MessageAttachment {
                kind: attachment_kind_to_proto(attachment.kind.as_str()) as i32,
                artifact_id: Some(common_v1::CanonicalId { ulid: attachment.artifact_id.clone() }),
                size_bytes: attachment.size_bytes,
                attachment_id: attachment.attachment_id.clone(),
                filename: attachment.filename.clone(),
                declared_content_type: attachment.declared_content_type.clone(),
                source_url: String::new(),
                content_hash: attachment.content_hash.clone(),
                origin: "tui_attachment_upload".to_owned(),
                policy_context: "attachment.upload.allowed".to_owned(),
                inline_bytes: Vec::new(),
                upload_requested: false,
                width_px: attachment.width_px.unwrap_or_default() as u32,
                height_px: attachment.height_px.unwrap_or_default() as u32,
            })
            .collect()
    }

    async fn start_prompt_run(
        &mut self,
        prompt: String,
        origin_kind: Option<String>,
        origin_run_id: Option<String>,
        parameter_delta_json: Option<String>,
        attachments: Vec<common_v1::MessageAttachment>,
    ) -> Result<()> {
        let mut request = build_agent_run_input(AgentRunInputArgs {
            session_id: self.session.session_id.clone(),
            session_key: None,
            session_label: None,
            require_existing: true,
            reset_session: false,
            run_id: None,
            prompt,
            allow_sensitive_tools: self.allow_sensitive_tools,
            origin_kind,
            origin_run_id,
            parameter_delta_json,
        })?;
        request.attachments = attachments;
        let stream = self.runtime.start_run_stream(request).await?;
        self.last_run_id = Some(stream.run_id().to_owned());
        if let Ok(now) = now_unix_ms_i64() {
            self.session_runtime.latest_started_at_unix_ms = Some(now);
            self.session_runtime.latest_completed_at_unix_ms = None;
        }
        self.active_stream = Some(stream);
        self.scroll_offset = 0;
        Ok(())
    }

    async fn submit_input(&mut self) -> Result<()> {
        let value = self.composer.trimmed_text().to_owned();
        if value.is_empty() {
            if !self.pending_attachments.is_empty() {
                self.status_line =
                    "Add a short prompt before sending attachments from the TUI composer."
                        .to_owned();
            }
            return Ok(());
        }
        if let Some(command) = value.strip_prefix('/') {
            return self.handle_slash_command(command).await;
        }
        if self.active_stream.is_some() {
            self.status_line = text::run_already_in_progress(self.locale);
            return Ok(());
        }
        if let Some(shell_command) = value.strip_prefix('!') {
            return self.handle_shell_request(shell_command.trim().to_owned()).await;
        }
        let draft_before_submit = self.current_draft_state();
        let attachments = self.prompt_attachments();
        self.create_undo_checkpoint("send").await?;
        let mut transcript_body = value.clone();
        if !self.pending_attachments.is_empty() {
            let attachment_lines = self
                .pending_attachments
                .iter()
                .map(|attachment| {
                    format!(
                        "  - {} · {} · {}",
                        attachment.filename,
                        format_size_bytes(attachment.size_bytes),
                        format_approx_tokens(attachment.budget_tokens)
                    )
                })
                .collect::<Vec<_>>();
            transcript_body.push_str("\n\nAttachments:\n");
            transcript_body.push_str(attachment_lines.join("\n").as_str());
        }
        let user_title = if self.pending_attachments.is_empty() {
            "You".to_owned()
        } else {
            format!(
                "You · {} attachment{}",
                self.pending_attachments.len(),
                if self.pending_attachments.len() == 1 { "" } else { "s" }
            )
        };
        self.status_line = text::running_prompt(self.locale);
        self.emit_ux_event(
            "ux.chat.prompt_submitted",
            Some("TUI submitted a chat prompt".to_owned()),
            serde_json::json!({
                "section": "chat",
                "sessionId": self.active_session_id().ok(),
                "objectiveId": self.selected_objective_id.clone(),
            }),
        )
        .await;
        self.clear_current_draft();
        self.slash_palette_dismissed = false;
        self.pending_slash_palette = None;
        self.slash_palette_selected = 0;
        if let Err(error) =
            self.start_prompt_run(value.clone(), None, None, None, attachments).await
        {
            self.composer = draft_before_submit.composer;
            self.pending_attachments = draft_before_submit.attachments;
            self.stash_current_draft();
            return Err(error);
        }
        self.push_entry(EntryKind::User, user_title, transcript_body);
        Ok(())
    }

    async fn handle_shell_request(&mut self, command: String) -> Result<()> {
        if command.is_empty() {
            self.status_line = text::shell_command_empty(self.locale);
            return Ok(());
        }
        if strict_profile_blocks_local_shell() {
            self.pending_shell_command = None;
            self.mode = Mode::Chat;
            self.status_line = text::local_shell_blocked(self.locale);
            return Ok(());
        }
        if !self.local_shell_enabled {
            self.pending_shell_command = Some(command);
            self.mode = Mode::ShellConfirm;
            self.status_line = text::local_shell_requires_opt_in(self.locale);
            return Ok(());
        }
        let result = run_local_shell(command.clone()).await?;
        self.push_entry(
            EntryKind::Shell,
            format!("Shell: {}", command),
            format_shell_result(&result),
        );
        self.status_line = text::shell_finished(self.locale, result.exit_code);
        Ok(())
    }

    async fn handle_slash_command(&mut self, command: &str) -> Result<()> {
        let mut parts = command.split_whitespace();
        let Some(raw_name) = parts.next() else {
            return Ok(());
        };
        let raw_arguments =
            command.trim().strip_prefix(raw_name).map(str::trim).unwrap_or_default();
        let Some(name) = resolve_shared_chat_command_name(raw_name, SharedChatCommandSurface::Tui)
        else {
            self.status_line = format!("Unknown slash command: /{raw_name}");
            return Ok(());
        };
        if self.active_stream.is_some()
            && !matches!(name, "help" | "status" | "usage" | "queue" | "interrupt")
        {
            self.status_line =
                format!("/{name} is unavailable while a run is active. Use /interrupt or /queue.");
            return Ok(());
        }
        self.ux_metrics.record(TuiUxMetricKey::SlashCommands);
        match name {
            "help" => self.mode = Mode::Help,
            "status" => {
                let detail = raw_arguments.eq_ignore_ascii_case("detail");
                self.push_entry(
                    EntryKind::System,
                    "Status",
                    if detail { self.status_detail_summary() } else { self.status_summary() },
                );
                self.status_line = text::status_refreshed(self.locale);
            }
            "new" => {
                let label = normalize_optional_text(parts.collect::<Vec<_>>().join(" "));
                self.create_session(label).await?;
            }
            "agent" => {
                if let Some(agent_id) = parts.next() {
                    self.switch_agent(agent_id.to_owned()).await?;
                } else {
                    self.open_picker(PickerKind::Agent).await?;
                }
            }
            "session" => {
                if let Some(reference) = parts.next() {
                    self.switch_session(reference.to_owned()).await?;
                } else {
                    self.open_picker(PickerKind::Session).await?;
                }
            }
            "objective" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.handle_objective_command(None, arguments).await?;
            }
            "heartbeat" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.handle_objective_command(Some("heartbeat"), arguments).await?;
            }
            "standing-order" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.handle_objective_command(Some("standing_order"), arguments).await?;
            }
            "program" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.handle_objective_command(Some("program"), arguments).await?;
            }
            "history" => {
                let query = if raw_arguments.trim().eq_ignore_ascii_case("family") {
                    self.current_session_catalog
                        .as_ref()
                        .map(|session| session.family.root_title.clone())
                } else {
                    normalize_optional_text(raw_arguments.to_owned())
                };
                self.open_session_history_picker(query).await?;
            }
            "resume" => {
                self.handle_resume_command(raw_arguments).await?;
            }
            "title" => {
                let label = normalize_optional_text(parts.collect::<Vec<_>>().join(" "));
                self.rename_current_session(label).await?;
            }
            "model" => {
                if let Some(model_id) = parts.next() {
                    self.set_model(model_id.to_owned()).await?;
                } else {
                    self.open_picker(PickerKind::Model).await?;
                }
            }
            "undo" => {
                let explicit_checkpoint_id = parts.next().map(ToOwned::to_owned);
                self.undo_last_turn(explicit_checkpoint_id).await?;
            }
            "interrupt" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.interrupt_run(arguments).await?;
            }
            "reset" => self.reset_session().await?,
            "retry" => self.retry_last_turn().await?,
            "branch" => {
                let label = normalize_optional_text(parts.collect::<Vec<_>>().join(" "));
                self.branch_from_current_session(label).await?;
            }
            "queue" => {
                let queued_text = normalize_optional_text(parts.collect::<Vec<_>>().join(" "));
                self.queue_follow_up(queued_text).await?;
            }
            "delegate" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.delegate_background_run(arguments).await?;
            }
            "checkpoint" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.handle_checkpoint_command(arguments).await?;
            }
            "rollback" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                rollback::handle_rollback_command(self, arguments).await?;
            }
            "workspace" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                workspace::handle_workspace_command(self, arguments).await?;
            }
            "background" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.handle_background_command(arguments).await?;
            }
            "usage" => {
                let budget = self.context_budget_summary();
                self.push_entry(
                    EntryKind::System,
                    "Usage",
                    format!(
                        "Budget {}\ndraft={} · project_context={} · attachments={}\nactive attachments={} · stored attachments={} · background tasks={}\nSlash commands={} · palette accepts={} · keyboard accepts={} · undo={} · interrupts={} · errors={}",
                        budget.label,
                        format_approx_tokens(budget.draft_tokens),
                        format_approx_tokens(budget.project_context_tokens),
                        format_approx_tokens(budget.attachment_tokens),
                        self.pending_attachments.len(),
                        self.session_runtime.attachment_count,
                        self.session_runtime.active_background_task_count,
                        self.ux_metrics.slash_commands,
                        self.ux_metrics.palette_accepts,
                        self.ux_metrics.keyboard_accepts,
                        self.ux_metrics.undo,
                        self.ux_metrics.interrupt,
                        self.ux_metrics.errors,
                    ),
                );
                self.status_line = text::usage_summary_refreshed(self.locale);
            }
            "compact" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.handle_compaction_command(arguments).await?;
            }
            "attach" => {
                self.handle_attach_command(raw_arguments).await?;
            }
            "profile" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.handle_profile_command(arguments).await?;
            }
            "browser" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.handle_browser_command(arguments).await?;
            }
            "doctor" => {
                let arguments = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
                self.handle_doctor_command(arguments).await?;
            }
            "settings" => self.mode = Mode::Settings,
            "tools" => {
                let next = parts.next();
                if next.is_some_and(quick_control_reset_requested) {
                    self.clear_trace_override().await?;
                } else {
                    self.set_trace_enabled(parse_toggle(next, self.show_tools)?).await?;
                }
            }
            "thinking" => {
                let next = parts.next();
                if next.is_some_and(quick_control_reset_requested) {
                    self.clear_thinking_override().await?;
                } else {
                    self.set_thinking_enabled(parse_toggle(next, self.show_thinking)?).await?;
                }
            }
            "verbose" => {
                let next = parts.next();
                if next.is_some_and(quick_control_reset_requested) {
                    self.clear_verbose_override().await?;
                } else {
                    self.set_verbose_enabled(parse_toggle(next, self.show_verbose)?).await?;
                }
            }
            "shell" => {
                let enabled = parse_toggle(parts.next(), self.local_shell_enabled)?;
                if enabled && !self.local_shell_enabled {
                    self.mode = Mode::ShellConfirm;
                    self.pending_shell_command = None;
                    self.status_line = text::confirm_local_shell_opt_in(self.locale);
                } else {
                    self.local_shell_enabled = enabled;
                    self.status_line = if enabled {
                        text::local_shell_enabled(self.locale)
                    } else {
                        text::local_shell_disabled(self.locale)
                    };
                }
            }
            "exit" | "quit" => self.status_line = "__exit__".to_owned(),
            other => anyhow::bail!(
                "shared chat command registry contains an unmapped TUI command `{other}`"
            ),
        }
        Ok(())
    }

    async fn switch_agent(&mut self, agent_id: String) -> Result<()> {
        let response = self
            .runtime
            .resolve_agent_for_context(AgentContextResolveInput {
                principal: self.runtime.connection().principal.clone(),
                channel: self.runtime.connection().channel.clone(),
                session_id: self.session.session_id.clone(),
                preferred_agent_id: normalize_agent_id_cli(agent_id.as_str())?,
                persist_session_binding: true,
            })
            .await?;
        let agent = response
            .agent
            .context("ResolveAgentForContext returned empty agent payload for tui switch")?;
        let source = agent_resolution_source_label(response.source);
        self.current_agent = Some(agent.clone());
        self.current_agent_source = source;
        self.refresh_current_session_catalog().await?;
        self.push_entry(
            EntryKind::System,
            "Agent",
            format!("Switched agent to {} ({source}).", agent.agent_id),
        );
        self.status_line = format!("Agent switched to {}", agent.display_name);
        self.mode = Mode::Chat;
        Ok(())
    }

    fn resolve_loaded_session_reference(&self, reference: &str) -> Option<TuiSlashSessionRecord> {
        let trimmed = reference.trim();
        self.slash_entity_catalog
            .sessions
            .iter()
            .find(|session| session_reference_matches(session, trimmed))
            .cloned()
    }

    async fn switch_session(&mut self, reference: String) -> Result<()> {
        if self.active_stream.is_some() {
            self.status_line = "Cannot switch sessions while a run is active".to_owned();
            return Ok(());
        }
        self.stash_current_draft();
        let trimmed = reference.trim();
        let mut recap_session = self.resolve_loaded_session_reference(trimmed);
        let request = if looks_like_canonical_ulid(trimmed) {
            SessionResolveInput {
                session_id: Some(resolve_required_canonical_id(trimmed.to_owned())?),
                session_key: String::new(),
                session_label: String::new(),
                require_existing: true,
                reset_session: false,
            }
        } else {
            if recap_session.is_none() {
                let response = self
                    .runtime
                    .list_session_catalog(vec![
                        ("limit", Some("12".to_owned())),
                        ("q", Some(trimmed.to_owned())),
                        (
                            "include_archived",
                            self.include_archived_sessions.then(|| "true".to_owned()),
                        ),
                    ])
                    .await?;
                recap_session = response
                    .sessions
                    .into_iter()
                    .find(|session| {
                        session_reference_equals(session.session_id.as_str(), trimmed)
                            || session_reference_equals(session.session_key.as_str(), trimmed)
                            || session_reference_equals(session.title.as_str(), trimmed)
                            || session_reference_equals(session.family.root_title.as_str(), trimmed)
                            || session.family.relatives.iter().any(|relative| {
                                session_reference_equals(relative.title.as_str(), trimmed)
                            })
                    })
                    .map(map_session_catalog_record);
            }

            if let Some(session) = recap_session.as_ref() {
                SessionResolveInput {
                    session_id: Some(resolve_required_canonical_id(session.session_id.clone())?),
                    session_key: String::new(),
                    session_label: String::new(),
                    require_existing: true,
                    reset_session: false,
                }
            } else {
                SessionResolveInput {
                    session_id: None,
                    session_key: trimmed.to_owned(),
                    session_label: String::new(),
                    require_existing: true,
                    reset_session: false,
                }
            }
        };
        let response = self.runtime.resolve_session(request).await?;
        let session = response
            .session
            .context("ResolveSession returned empty session payload for tui switch")?;
        self.session = session;
        self.last_run_id = None;
        self.pending_approval = None;
        self.transcript.clear();
        self.push_entry(EntryKind::System, "Session", "Session switched.");
        self.refresh_agent_identity(None, false).await?;
        self.refresh_slash_entity_catalogs().await?;
        let _ = self.refresh_session_runtime_snapshot().await;
        self.restore_current_draft();
        self.push_session_recap(recap_session.take());
        self.sync_slash_palette();
        self.status_line = text::session_switched(self.locale);
        self.emit_ux_event(
            "ux.session.resumed",
            Some("TUI resumed an existing session".to_owned()),
            serde_json::json!({
                "section": "chat",
                "sessionId": self.active_session_id().ok(),
            }),
        )
        .await;
        self.mode = Mode::Chat;
        Ok(())
    }

    async fn handle_resume_command(&mut self, raw_arguments: &str) -> Result<()> {
        let trimmed = raw_arguments.trim();
        if trimmed.is_empty() {
            return self.open_session_history_picker(None).await;
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "parent" => self.resume_family_parent().await,
            "sibling" | "siblings" => self.open_family_relation_picker("sibling").await,
            "child" | "children" => self.open_family_relation_picker("child").await,
            "family" => {
                let query = self
                    .current_session_catalog
                    .as_ref()
                    .map(|session| session.family.root_title.clone());
                self.open_session_history_picker(query).await
            }
            _ => self.switch_session(trimmed.to_owned()).await,
        }
    }

    async fn resume_family_parent(&mut self) -> Result<()> {
        let Some(parent_session_id) = self
            .current_session_catalog
            .as_ref()
            .and_then(|session| session.family.parent_session_id.clone())
        else {
            self.status_line = "The current session does not have a parent branch.".to_owned();
            return Ok(());
        };
        self.switch_session(parent_session_id).await
    }

    async fn open_family_relation_picker(&mut self, relation: &str) -> Result<()> {
        let Some(session) = self.current_session_catalog.as_ref() else {
            self.status_line = "Session family metadata is not loaded yet.".to_owned();
            return Ok(());
        };
        let mut items = session
            .family
            .relatives
            .iter()
            .filter(|relative| relative.relation.eq_ignore_ascii_case(relation))
            .map(|relative| PickerItem {
                id: relative.session_id.clone(),
                title: relative.title.clone(),
                detail: format!(
                    "{} · {}",
                    relative.relation,
                    describe_branch_state_label(relative.branch_state.as_str())
                ),
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            self.status_line =
                format!("No {} session is available from the current family.", relation);
            return Ok(());
        }
        if items.len() == 1 {
            return self.switch_session(items.remove(0).id).await;
        }
        self.pending_picker = Some(PickerState {
            kind: PickerKind::Session,
            title: format!("{} branches", relation),
            items,
            selected: 0,
        });
        self.mode = Mode::Picker(PickerKind::Session);
        Ok(())
    }

    async fn reset_session(&mut self) -> Result<()> {
        if self.active_stream.is_some() {
            self.status_line =
                "Cannot reset an active session while a run is in progress".to_owned();
            return Ok(());
        }
        self.stash_current_draft();
        let response = self
            .runtime
            .resolve_session(SessionResolveInput {
                session_id: self.session.session_id.clone(),
                session_key: String::new(),
                session_label: String::new(),
                require_existing: true,
                reset_session: true,
            })
            .await?;
        self.session = response
            .session
            .context("ResolveSession returned empty session payload for tui reset")?;
        self.transcript.clear();
        self.last_run_id = None;
        self.pending_approval = None;
        self.push_entry(EntryKind::System, "Session", "Session history reset.");
        self.refresh_agent_identity(None, false).await?;
        self.refresh_slash_entity_catalogs().await?;
        let _ = self.refresh_session_runtime_snapshot().await;
        self.restore_current_draft();
        self.sync_slash_palette();
        self.status_line = text::session_reset(self.locale);
        Ok(())
    }

    async fn rename_current_session(&mut self, requested_label: Option<String>) -> Result<()> {
        let session_id = self.active_session_id()?;
        let context = self.connect_admin_console().await?;
        context
            .client
            .post_json_value(
                format!(
                    "console/v1/chat/sessions/{}/rename",
                    percent_encode_component(session_id.as_str())
                ),
                &serde_json::json!({
                    "session_label": requested_label,
                    "manual_title_locked": requested_label.is_some(),
                }),
            )
            .await?;
        self.refresh_slash_entity_catalogs().await?;
        self.sync_slash_palette();
        let message = if requested_label.is_some() {
            "Session title updated."
        } else {
            "Session title returned to automatic mode."
        };
        self.push_entry(EntryKind::System, "Title", message);
        self.status_line = message.to_owned();
        Ok(())
    }

    async fn abort_run(
        &mut self,
        explicit_run_id: Option<String>,
        reason: Option<String>,
    ) -> Result<()> {
        let run_id = if let Some(run_id) = explicit_run_id {
            resolve_or_generate_canonical_id(Some(run_id))?
        } else {
            self.last_run_id
                .clone()
                .context("/abort without explicit run_id requires a previous run")?
        };
        let response = self
            .runtime
            .abort_run(run_id.clone(), reason.or(Some("tui_interrupt".to_owned())))
            .await?;
        self.push_entry(
            EntryKind::System,
            "Interrupt",
            format!(
                "cancel_requested={} run_id={}",
                response.cancel_requested,
                redacted_optional_identifier_for_output(
                    response
                        .run_id
                        .as_ref()
                        .map(|value| value.ulid.as_str())
                        .or(Some(run_id.as_str())),
                )
            ),
        );
        self.status_line = "Interrupt requested".to_owned();
        Ok(())
    }

    async fn create_undo_checkpoint(&mut self, source: &'static str) -> Result<()> {
        let session_id = self.active_session_id()?;
        if self.transcript.is_empty() && self.last_run_id.is_none() {
            return Ok(());
        }
        let context = self.connect_admin_console().await?;
        let note = match source {
            "retry" => {
                "Created automatically before retry so /undo can restore the prior conversational state."
            }
            "redirect" => {
                "Created automatically before interrupt redirect so /undo can restore the prior conversational state."
            }
            _ => {
                "Created automatically before a new chat run so /undo can restore the prior conversational state."
            }
        };
        let result = context
            .client
            .post_json_value(
                format!(
                    "console/v1/chat/sessions/{}/checkpoints",
                    percent_encode_component(session_id.as_str())
                ),
                &serde_json::json!({
                    "name": format!("Undo checkpoint before {source}"),
                    "note": note,
                    "tags": ["undo_safe", source],
                }),
            )
            .await;
        match result {
            Ok(_) => {
                self.refresh_checkpoint_catalog().await?;
                Ok(())
            }
            Err(error) => {
                self.ux_metrics.record(TuiUxMetricKey::Errors);
                self.status_line =
                    sanitize_terminal_text(format!("Undo checkpoint skipped: {error}").as_str());
                Ok(())
            }
        }
    }

    async fn undo_last_turn(&mut self, explicit_checkpoint_id: Option<String>) -> Result<()> {
        let checkpoint = if let Some(explicit_checkpoint_id) = explicit_checkpoint_id {
            self.slash_entity_catalog
                .checkpoints
                .iter()
                .find(|checkpoint| checkpoint.checkpoint_id == explicit_checkpoint_id)
        } else {
            select_undo_checkpoint(self.slash_entity_catalog.checkpoints.as_slice())
        };
        let Some(checkpoint) = checkpoint else {
            self.status_line = "No safe undo checkpoint is available yet. Send another turn first or restore a checkpoint explicitly.".to_owned();
            self.ux_metrics.record(TuiUxMetricKey::Errors);
            return Ok(());
        };
        self.ux_metrics.record(TuiUxMetricKey::Undo);
        self.restore_checkpoint_with_notice(checkpoint.checkpoint_id.clone(), "undo").await
    }

    async fn interrupt_run(&mut self, arguments: Vec<String>) -> Result<()> {
        let Some(active_run_id) =
            self.active_stream.as_ref().map(|stream| stream.run_id().to_owned())
        else {
            self.status_line = "No run is available for interruption.".to_owned();
            self.ux_metrics.record(TuiUxMetricKey::Errors);
            return Ok(());
        };
        let trimmed = arguments.join(" ");
        let mut parts = trimmed.split_whitespace();
        let first = parts.next().unwrap_or_default();
        let (mode, redirect_prompt) = if matches!(first, "soft" | "force") {
            (first, parts.collect::<Vec<_>>().join(" ").trim().to_owned())
        } else {
            ("soft", trimmed.trim().to_owned())
        };
        if !redirect_prompt.is_empty() {
            self.create_undo_checkpoint("redirect").await?;
            self.pending_redirect_prompt = Some(PendingRedirectPrompt {
                prompt: redirect_prompt,
                mode: mode.to_owned(),
                interrupted_run_id: active_run_id.clone(),
            });
        } else {
            self.pending_redirect_prompt = None;
        }
        self.ux_metrics.record(TuiUxMetricKey::Interrupt);
        self.abort_run(Some(active_run_id.clone()), Some(format!("tui_interrupt_{mode}"))).await?;
        self.push_entry(
            EntryKind::System,
            "Interrupt",
            if self.pending_redirect_prompt.is_some() {
                format!(
                    "{} interrupt requested for {}. The redirected prompt will start after the run closes cleanly.",
                    mode,
                    shorten_id(active_run_id.as_str())
                )
            } else {
                format!(
                    "{} interrupt requested for {}.\nAny external side effects already emitted remain in the world state.",
                    mode,
                    shorten_id(active_run_id.as_str())
                )
            },
        );
        self.status_line = if self.pending_redirect_prompt.is_some() {
            "Interrupt requested; redirect queued".to_owned()
        } else {
            "Interrupt requested".to_owned()
        };
        Ok(())
    }

    async fn create_session(&mut self, session_label: Option<String>) -> Result<()> {
        if self.active_stream.is_some() {
            self.status_line = "Cannot create a new session while a run is active".to_owned();
            return Ok(());
        }
        self.stash_current_draft();
        let response = self
            .runtime
            .resolve_session(SessionResolveInput {
                session_id: None,
                session_key: String::new(),
                session_label: session_label.unwrap_or_default(),
                require_existing: false,
                reset_session: false,
            })
            .await?;
        self.session = response
            .session
            .context("ResolveSession returned empty session payload for tui create")?;
        self.transcript.clear();
        self.last_run_id = None;
        self.pending_approval = None;
        self.push_entry(EntryKind::System, "Session", "Created a new session.");
        self.refresh_agent_identity(None, false).await?;
        self.refresh_slash_entity_catalogs().await?;
        let _ = self.refresh_session_runtime_snapshot().await;
        self.restore_current_draft();
        self.sync_slash_palette();
        self.status_line = "Session created".to_owned();
        Ok(())
    }

    async fn retry_last_turn(&mut self) -> Result<()> {
        if self.active_stream.is_some() {
            self.status_line = "Cannot retry while a run is active".to_owned();
            return Ok(());
        }
        let session_id = self
            .session
            .session_id
            .as_ref()
            .map(|value| value.ulid.clone())
            .context("active TUI session is missing a session_id")?;
        self.create_undo_checkpoint("retry").await?;
        let context = client::control_plane::connect_admin_console(app::ConnectionOverrides {
            grpc_url: Some(self.runtime.connection().grpc_url.clone()),
            daemon_url: None,
            token: self.runtime.connection().token.clone(),
            principal: Some(self.runtime.connection().principal.clone()),
            device_id: Some(self.runtime.connection().device_id.clone()),
            channel: Some(self.runtime.connection().channel.clone()),
        })
        .await?;
        let payload = context
            .client
            .post_json_value(
                format!(
                    "console/v1/chat/sessions/{}/retry",
                    percent_encode_component(session_id.as_str())
                ),
                &serde_json::json!({}),
            )
            .await?;
        let prompt = payload
            .pointer("/text")
            .and_then(serde_json::Value::as_str)
            .context("retry response is missing text")?
            .to_owned();
        let origin_kind = payload
            .pointer("/origin_kind")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        let origin_run_id = payload
            .pointer("/origin_run_id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        let parameter_delta_json = payload
            .pointer("/parameter_delta")
            .filter(|value| !value.is_null())
            .map(serde_json::to_string)
            .transpose()?;
        self.push_entry(EntryKind::System, "Retry", "Replaying the latest turn as a new run.");
        self.status_line = "Retrying latest turn".to_owned();
        self.start_prompt_run(prompt, origin_kind, origin_run_id, parameter_delta_json, Vec::new())
            .await
    }

    async fn branch_from_current_session(&mut self, session_label: Option<String>) -> Result<()> {
        if self.active_stream.is_some() {
            self.status_line = "Cannot branch while a run is active".to_owned();
            return Ok(());
        }
        let session_id = self
            .session
            .session_id
            .as_ref()
            .map(|value| value.ulid.clone())
            .context("active TUI session is missing a session_id")?;
        let context = client::control_plane::connect_admin_console(app::ConnectionOverrides {
            grpc_url: Some(self.runtime.connection().grpc_url.clone()),
            daemon_url: None,
            token: self.runtime.connection().token.clone(),
            principal: Some(self.runtime.connection().principal.clone()),
            device_id: Some(self.runtime.connection().device_id.clone()),
            channel: Some(self.runtime.connection().channel.clone()),
        })
        .await?;
        let payload = context
            .client
            .post_json_value(
                format!(
                    "console/v1/chat/sessions/{}/branch",
                    percent_encode_component(session_id.as_str())
                ),
                &serde_json::json!({ "session_label": session_label }),
            )
            .await?;
        let next_session_id = payload
            .pointer("/session/session_id")
            .and_then(serde_json::Value::as_str)
            .context("branch response is missing child session_id")?
            .to_owned();
        let suggested_session_label = payload
            .pointer("/suggested_session_label")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        self.switch_session(next_session_id).await?;
        self.push_entry(
            EntryKind::System,
            "Branch",
            match suggested_session_label {
                Some(label) => format!(
                    "Created a new active branch from the latest terminal run.\nSuggested title: {label}"
                ),
                None => "Created a new active branch from the latest terminal run.".to_owned(),
            },
        );
        self.refresh_slash_entity_catalogs().await?;
        self.sync_slash_palette();
        self.status_line = "Branched into a new session".to_owned();
        Ok(())
    }

    async fn queue_follow_up(&mut self, queued_text: Option<String>) -> Result<()> {
        let text = match queued_text {
            Some(text) => text,
            None => {
                self.status_line = "Usage: /queue <follow-up text>".to_owned();
                return Ok(());
            }
        };
        let Some(run_id) = self.active_stream.as_ref().map(|stream| stream.run_id().to_owned())
        else {
            self.status_line = "Queued follow-up requires an active run".to_owned();
            return Ok(());
        };
        let context = client::control_plane::connect_admin_console(app::ConnectionOverrides {
            grpc_url: Some(self.runtime.connection().grpc_url.clone()),
            daemon_url: None,
            token: self.runtime.connection().token.clone(),
            principal: Some(self.runtime.connection().principal.clone()),
            device_id: Some(self.runtime.connection().device_id.clone()),
            channel: Some(self.runtime.connection().channel.clone()),
        })
        .await?;
        let payload = context
            .client
            .post_json_value(
                format!("console/v1/chat/runs/{}/queue", percent_encode_component(run_id.as_str())),
                &serde_json::json!({ "text": text }),
            )
            .await?;
        let queued_input_id = payload
            .pointer("/queued_input/queued_input_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        self.push_entry(
            EntryKind::System,
            "Queue",
            format!("Queued follow-up {} for the active run.", shorten_id(queued_input_id)),
        );
        self.status_line = "Queued follow-up recorded".to_owned();
        Ok(())
    }

    async fn handle_attach_command(&mut self, raw_arguments: &str) -> Result<()> {
        let trimmed = raw_arguments.trim();
        if trimmed.is_empty() {
            if self.pending_attachments.is_empty() {
                self.push_entry(
                    EntryKind::System,
                    "Attachments",
                    "Usage: /attach <path>\nShortcuts: Ctrl+O seeds `/attach ` in the composer.\nClipboard attach is not available in the TUI yet; use a filesystem path instead.",
                );
                self.status_line =
                    "Type `/attach <path>` or press Ctrl+O to attach a file".to_owned();
            } else {
                self.list_pending_attachments();
                self.status_line = "Pending attachments listed".to_owned();
            }
            return Ok(());
        }
        let lower = trimmed.to_ascii_lowercase();
        if matches!(lower.as_str(), "list" | "ls" | "show") {
            self.list_pending_attachments();
            self.status_line = "Pending attachments listed".to_owned();
            return Ok(());
        }
        if matches!(lower.as_str(), "clipboard" | "paste") {
            self.push_entry(
                EntryKind::System,
                "Attachments",
                "Clipboard image/file attach is not available in the TUI yet. Use `/attach <path>` for a local file or the desktop/web surface for richer clipboard flows.",
            );
            self.status_line = "Clipboard attach remains desktop/web-first".to_owned();
            return Ok(());
        }
        if matches!(lower.as_str(), "clear" | "reset") {
            self.pending_attachments.clear();
            self.stash_current_draft();
            self.status_line = "Pending attachments cleared".to_owned();
            return Ok(());
        }
        if let Some(target) = lower.strip_prefix("remove ").or_else(|| lower.strip_prefix("rm ")) {
            let target = target.trim();
            let removed = if let Ok(index) = target.parse::<usize>() {
                if index == 0 || index > self.pending_attachments.len() {
                    None
                } else {
                    Some(self.pending_attachments.remove(index - 1))
                }
            } else {
                self.pending_attachments
                    .iter()
                    .position(|attachment| {
                        attachment.local_id.eq_ignore_ascii_case(target)
                            || attachment.artifact_id.eq_ignore_ascii_case(target)
                            || attachment.filename.eq_ignore_ascii_case(target)
                    })
                    .map(|index| self.pending_attachments.remove(index))
            };
            if let Some(attachment) = removed {
                self.stash_current_draft();
                self.status_line = format!("Removed attachment {}", attachment.filename);
            } else {
                self.status_line =
                    "Attachment not found. Use `/attach list` to inspect the current queue."
                        .to_owned();
            }
            return Ok(());
        }
        self.upload_attachment_from_path(trimmed).await
    }

    fn list_pending_attachments(&mut self) {
        if self.pending_attachments.is_empty() {
            self.push_entry(EntryKind::System, "Attachments", "No pending attachments.");
            return;
        }
        let mut lines = vec![format!(
            "{} attachment{} ready for the next turn:",
            self.pending_attachments.len(),
            if self.pending_attachments.len() == 1 { "" } else { "s" }
        )];
        lines.extend(self.pending_attachments.iter().enumerate().map(|(index, attachment)| {
            format!(
                "{}. {} · {} · {} · {}{}",
                index + 1,
                attachment.filename,
                attachment.kind,
                format_size_bytes(attachment.size_bytes),
                format_approx_tokens(attachment.budget_tokens),
                if attachment.derived_artifacts > 0 {
                    format!(" · derived {}", attachment.derived_artifacts)
                } else {
                    String::new()
                }
            )
        }));
        lines.push("Use `/attach remove <index>` to drop one item before sending.".to_owned());
        self.push_entry(EntryKind::System, "Attachments", lines.join("\n"));
    }

    async fn upload_attachment_from_path(&mut self, raw_path: &str) -> Result<()> {
        let normalized = raw_path.trim().trim_matches('"').trim_matches('\'');
        if normalized.is_empty() {
            self.status_line = "Attachment path cannot be empty".to_owned();
            return Ok(());
        }
        let path = PathBuf::from(normalized);
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .context("failed to resolve the current working directory")?
                .join(path)
        };
        let metadata = fs::metadata(path.as_path())
            .with_context(|| format!("failed to stat attachment {}", path.display()))?;
        if !metadata.is_file() {
            anyhow::bail!("attachment path is not a regular file: {}", path.display());
        }
        let bytes = fs::read(path.as_path())
            .with_context(|| format!("failed to read {}", path.display()))?;
        let filename = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| path.display().to_string());
        let content_type = guess_content_type(path.as_path());
        let session_id = self.active_session_id()?;
        let context = self.connect_admin_console().await?;
        let payload = context
            .client
            .post_json_value(
                format!(
                    "console/v1/chat/sessions/{}/attachments",
                    percent_encode_component(session_id.as_str())
                ),
                &serde_json::json!({
                    "filename": filename,
                    "content_type": content_type,
                    "bytes_base64": BASE64_STANDARD.encode(bytes.as_slice()),
                }),
            )
            .await?;
        let declared_content_type = read_json_string(&payload, "/attachment/declared_content_type");
        let kind = {
            let reported = read_json_string(&payload, "/attachment/kind");
            if reported.is_empty() {
                attachment_kind_label(declared_content_type.as_str()).to_owned()
            } else {
                reported
            }
        };
        let attachment = TuiPendingAttachment {
            local_id: format!(
                "{}-{}",
                read_json_string(&payload, "/attachment/artifact_id"),
                now_unix_ms_i64().unwrap_or_default()
            ),
            artifact_id: read_json_string(&payload, "/attachment/artifact_id"),
            attachment_id: read_json_string(&payload, "/attachment/attachment_id"),
            filename: read_json_string(&payload, "/attachment/filename"),
            declared_content_type,
            content_hash: read_json_string(&payload, "/attachment/content_hash"),
            size_bytes: read_json_u64(&payload, "/attachment/size_bytes"),
            width_px: read_json_optional_i64(&payload, "/attachment/width_px")
                .map(|value| value as u64),
            height_px: read_json_optional_i64(&payload, "/attachment/height_px")
                .map(|value| value as u64),
            kind,
            budget_tokens: read_json_u64(&payload, "/attachment/budget_tokens"),
            derived_artifacts: payload
                .pointer("/derived_artifacts")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len)
                .unwrap_or_default(),
        };
        self.pending_attachments.push(attachment.clone());
        self.session_runtime.attachment_count =
            self.session_runtime.attachment_count.saturating_add(1);
        self.stash_current_draft();
        self.status_line = format!(
            "Attached {} ({}, {}{})",
            attachment.filename,
            format_size_bytes(attachment.size_bytes),
            format_approx_tokens(attachment.budget_tokens),
            if attachment.derived_artifacts > 0 {
                format!(", derived {}", attachment.derived_artifacts)
            } else {
                String::new()
            }
        );
        Ok(())
    }

    async fn connect_admin_console(&self) -> Result<client::control_plane::AdminConsoleContext> {
        client::control_plane::connect_admin_console(app::ConnectionOverrides {
            grpc_url: Some(self.runtime.connection().grpc_url.clone()),
            daemon_url: None,
            token: self.runtime.connection().token.clone(),
            principal: Some(self.runtime.connection().principal.clone()),
            device_id: Some(self.runtime.connection().device_id.clone()),
            channel: Some(self.runtime.connection().channel.clone()),
        })
        .await
    }

    async fn emit_ux_event(&self, name: &str, summary: Option<String>, details: Value) {
        let Ok(context) = self.connect_admin_console().await else {
            return;
        };
        let mut details = match details {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        details.insert("surface".to_owned(), Value::String("tui".to_owned()));
        details.insert("locale".to_owned(), Value::String(self.locale.as_str().to_owned()));
        let _ = context
            .client
            .post_json_value(
                "console/v1/system/events/emit",
                &serde_json::json!({
                    "name": name,
                    "summary": summary,
                    "details": Value::Object(details),
                }),
            )
            .await;
    }

    fn active_session_id(&self) -> Result<String> {
        self.session
            .session_id
            .as_ref()
            .map(|value| value.ulid.clone())
            .context("active TUI session is missing a session_id")
    }

    fn sync_slash_palette(&mut self) {
        if !self.composer.text().trim_start().starts_with('/') {
            self.pending_slash_palette = None;
            self.slash_palette_dismissed = false;
            self.slash_palette_selected = 0;
            return;
        }
        if self.slash_palette_dismissed {
            self.pending_slash_palette = None;
            return;
        }
        self.pending_slash_palette = build_tui_slash_palette(BuildTuiSlashPaletteArgs {
            input: self.composer.text(),
            catalog: &self.slash_entity_catalog,
            streaming: self.active_stream.is_some(),
            delegation_profiles: BUILT_IN_DELEGATION_PROFILES,
            delegation_templates: BUILT_IN_DELEGATION_TEMPLATES,
        });
        let suggestion_count = self
            .pending_slash_palette
            .as_ref()
            .map(|palette| palette.suggestions.len())
            .unwrap_or(0);
        self.slash_palette_selected =
            self.slash_palette_selected.min(suggestion_count.saturating_sub(1));
    }

    fn apply_selected_slash_suggestion(&mut self, accepted_with_keyboard: bool) {
        let Some(palette) = self.pending_slash_palette.as_ref() else {
            return;
        };
        let Some(suggestion) = palette.suggestions.get(self.slash_palette_selected) else {
            return;
        };
        self.composer.set_text(suggestion.replacement.clone());
        self.slash_palette_dismissed = false;
        self.pending_slash_palette = None;
        self.slash_palette_selected = 0;
        self.ux_metrics.record(TuiUxMetricKey::PaletteAccepts);
        if accepted_with_keyboard {
            self.ux_metrics.record(TuiUxMetricKey::KeyboardAccepts);
        }
        self.sync_composer_after_edit();
    }

    async fn refresh_slash_entity_catalogs(&mut self) -> Result<()> {
        self.refresh_session_catalog().await?;
        self.refresh_objective_catalog().await?;
        self.refresh_auth_profile_catalog().await?;
        self.refresh_browser_catalog().await?;
        self.refresh_checkpoint_catalog().await?;
        self.refresh_workspace_catalog().await?;
        Ok(())
    }

    async fn refresh_session_catalog(&mut self) -> Result<()> {
        let response = self
            .runtime
            .list_session_catalog(vec![
                ("limit", Some("32".to_owned())),
                ("sort", Some("updated_desc".to_owned())),
                ("include_archived", self.include_archived_sessions.then(|| "true".to_owned())),
            ])
            .await?;
        let active_session_id = self.active_session_id().ok();
        let mut current_session_catalog = None;
        self.slash_entity_catalog.sessions = response
            .sessions
            .into_iter()
            .map(|session| {
                if active_session_id.as_deref() == Some(session.session_id.as_str()) {
                    current_session_catalog = Some(session.clone());
                }
                map_session_catalog_record(session)
            })
            .collect();
        if let Some(session) = current_session_catalog {
            self.current_session_catalog = Some(session);
            self.sync_session_quick_controls_from_catalog();
        } else if active_session_id.is_some() {
            self.refresh_current_session_catalog().await?;
        } else {
            self.current_session_catalog = None;
        }
        Ok(())
    }

    async fn refresh_objective_catalog(&mut self) -> Result<()> {
        let context = self.connect_admin_console().await?;
        let payload = crate::commands::objectives::list_objectives_value(
            &context.client,
            None,
            Some(32),
            None,
            None,
        )
        .await?;
        self.slash_entity_catalog.objectives = payload
            .pointer("/objectives")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|value| TuiSlashObjectiveRecord {
                objective_id: read_json_string(&value, "/objective_id"),
                name: read_json_string(&value, "/name"),
                kind: read_json_string(&value, "/kind"),
                focus: read_json_string(&value, "/current_focus"),
            })
            .filter(|record| !record.objective_id.is_empty())
            .collect();
        Ok(())
    }

    async fn refresh_auth_profile_catalog(&mut self) -> Result<()> {
        let context = self.connect_admin_console().await?;
        let payload =
            context.client.get_json_value("console/v1/auth/profiles?limit=32".to_owned()).await?;
        self.slash_entity_catalog.auth_profiles = payload
            .pointer("/profiles")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|value| TuiSlashAuthProfileRecord {
                profile_id: read_json_string(&value, "/profile_id"),
                profile_name: read_json_string(&value, "/profile_name"),
                provider_kind: read_json_string(&value, "/provider/kind"),
                scope_kind: read_json_string(&value, "/scope/kind"),
            })
            .filter(|record| !record.profile_id.is_empty())
            .collect();
        Ok(())
    }

    async fn refresh_browser_catalog(&mut self) -> Result<()> {
        let context = self.connect_admin_console().await?;
        let profiles_payload = context
            .client
            .get_json_value("console/v1/browser/profiles?limit=16".to_owned())
            .await?;
        self.slash_entity_catalog.browser_profiles = profiles_payload
            .pointer("/profiles")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|value| TuiSlashBrowserProfileRecord {
                profile_id: read_json_string(&value, "/profile_id"),
                name: read_json_string(&value, "/name"),
                persistence_enabled: read_json_bool(&value, "/persistence_enabled")
                    || read_json_bool(&value, "/persistence"),
                private_profile: read_json_bool(&value, "/private_profile"),
            })
            .filter(|record| !record.profile_id.is_empty())
            .collect();
        let sessions_payload = context
            .client
            .get_json_value("console/v1/browser/sessions?limit=16".to_owned())
            .await?;
        self.slash_entity_catalog.browser_sessions = sessions_payload
            .pointer("/sessions")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|value| {
                let page_title = read_json_string(&value, "/page_title");
                let target_url = read_json_string(&value, "/target_url");
                let channel = read_json_string(&value, "/channel");
                let title = if !page_title.is_empty() {
                    page_title
                } else if !target_url.is_empty() {
                    target_url
                } else if !channel.is_empty() {
                    channel
                } else {
                    "Browser session".to_owned()
                };
                TuiSlashBrowserSessionRecord {
                    session_id: read_json_string(&value, "/session_id"),
                    title,
                }
            })
            .filter(|record| !record.session_id.is_empty())
            .collect();
        Ok(())
    }

    async fn refresh_checkpoint_catalog(&mut self) -> Result<()> {
        let session_id = self.active_session_id()?;
        let context = self.connect_admin_console().await?;
        let payload = context
            .client
            .get_json_value(format!(
                "console/v1/chat/sessions/{}/transcript",
                percent_encode_component(session_id.as_str())
            ))
            .await?;
        self.slash_entity_catalog.checkpoints = payload
            .pointer("/checkpoints")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|value| {
                let tags_json = read_json_string(&value, "/tags_json");
                let parsed_tags =
                    serde_json::from_str::<serde_json::Value>(tags_json.as_str()).ok();
                TuiSlashCheckpointRecord {
                    checkpoint_id: read_json_string(&value, "/checkpoint_id"),
                    name: read_json_string(&value, "/name"),
                    note: read_json_string(&value, "/note"),
                    created_at_unix_ms: read_json_i64(&value, "/created_at_unix_ms"),
                    tags: parsed_tags
                        .as_ref()
                        .map(|parsed| read_json_tags(parsed, ""))
                        .unwrap_or_default(),
                }
            })
            .filter(|record| !record.checkpoint_id.is_empty())
            .collect();
        Ok(())
    }

    async fn refresh_workspace_catalog(&mut self) -> Result<()> {
        self.slash_entity_catalog.workspace_artifacts.clear();
        self.slash_entity_catalog.workspace_checkpoints.clear();
        let run_id = self
            .last_run_id
            .clone()
            .or_else(|| {
                self.current_session_catalog
                    .as_ref()
                    .and_then(|session| session.last_run_id.clone())
            })
            .or_else(|| self.session.last_run_id.as_ref().map(|value| value.ulid.clone()));
        let Some(run_id) = run_id else {
            return Ok(());
        };
        let context = self.connect_admin_console().await?;
        let payload = match context
            .client
            .get_json_value(format!(
                "console/v1/chat/runs/{}/workspace?limit=24",
                percent_encode_component(run_id.as_str())
            ))
            .await
        {
            Ok(payload) => payload,
            Err(_) => return Ok(()),
        };
        self.slash_entity_catalog.workspace_artifacts = payload
            .pointer("/workspace/artifacts")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|value| TuiSlashWorkspaceArtifactRecord {
                artifact_id: read_json_string(&value, "/artifact_id"),
                path: read_json_string(&value, "/path"),
                display_path: read_json_string(&value, "/display_path"),
                change_kind: read_json_string(&value, "/change_kind"),
                latest_checkpoint_id: read_json_string(&value, "/latest_checkpoint_id"),
                preview_kind: read_json_string(&value, "/preview_kind"),
                size_bytes: value.pointer("/size_bytes").and_then(serde_json::Value::as_u64),
                deleted: read_json_bool(&value, "/deleted"),
            })
            .filter(|record| !record.artifact_id.is_empty())
            .collect();
        self.slash_entity_catalog.workspace_checkpoints = payload
            .pointer("/workspace/workspace_checkpoints")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|value| TuiSlashWorkspaceCheckpointRecord {
                checkpoint_id: read_json_string(&value, "/checkpoint_id"),
                source_label: read_json_string(&value, "/source_label"),
                summary_text: read_json_string(&value, "/summary_text"),
                restore_count: value
                    .pointer("/restore_count")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default(),
                created_at_unix_ms: read_json_i64(&value, "/created_at_unix_ms"),
            })
            .filter(|record| !record.checkpoint_id.is_empty())
            .collect();
        Ok(())
    }

    async fn restore_checkpoint_with_notice(
        &mut self,
        checkpoint_id: String,
        source: &'static str,
    ) -> Result<()> {
        let context = self.connect_admin_console().await?;
        let payload = context
            .client
            .post_json_value(
                format!(
                    "console/v1/chat/checkpoints/{}/restore",
                    percent_encode_component(checkpoint_id.as_str())
                ),
                &serde_json::json!({}),
            )
            .await?;
        let next_session_id = payload
            .pointer("/session/session_id")
            .and_then(serde_json::Value::as_str)
            .context("checkpoint restore response is missing session_id")?
            .to_owned();
        let suggested_session_label = payload
            .pointer("/suggested_session_label")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        self.switch_session(next_session_id).await?;
        let warning = if source == "undo" {
            "Conversation state was restored from a checkpoint. Any external side effects already emitted remain in the world state."
        } else {
            "Checkpoint restored into a new branch. Any external side effects already emitted remain in the world state."
        };
        self.push_entry(
            EntryKind::System,
            if source == "undo" { "Undo" } else { "Checkpoint restore" },
            match suggested_session_label {
                Some(label) => format!(
                    "{warning}\ncheckpoint={}\nsuggested title={label}",
                    shorten_id(checkpoint_id.as_str())
                ),
                None => format!("{warning}\ncheckpoint={}", shorten_id(checkpoint_id.as_str())),
            },
        );
        self.status_line = if source == "undo" {
            "Undo restore completed".to_owned()
        } else {
            "Checkpoint restored as a new branch".to_owned()
        };
        Ok(())
    }

    async fn handle_profile_command(&mut self, arguments: Vec<String>) -> Result<()> {
        if self.slash_entity_catalog.auth_profiles.is_empty() {
            self.refresh_auth_profile_catalog().await?;
        }
        let target = normalize_optional_text(arguments.join(" "));
        if let Some(target) = target {
            let target = target.to_ascii_lowercase();
            let matched = self
                .slash_entity_catalog
                .auth_profiles
                .iter()
                .find(|profile| {
                    profile.profile_id.to_ascii_lowercase() == target
                        || profile.profile_name.to_ascii_lowercase() == target
                })
                .cloned();
            let Some(profile) = matched else {
                self.status_line = "Auth profile not found in the loaded catalog.".to_owned();
                self.ux_metrics.record(TuiUxMetricKey::Errors);
                return Ok(());
            };
            self.push_entry(
                EntryKind::System,
                "Profile",
                format!(
                    "{}\nprofile_id={}\nprovider={} scope={}\nUse `palyra auth profiles show {}` or the web console Auth section for full posture detail.",
                    profile.profile_name,
                    profile.profile_id,
                    profile.provider_kind,
                    profile.scope_kind,
                    profile.profile_id
                ),
            );
            self.status_line = format!("Loaded profile {}", profile.profile_name);
            return Ok(());
        }
        if self.slash_entity_catalog.auth_profiles.is_empty() {
            self.push_entry(
                EntryKind::System,
                "Profile",
                "No auth profiles are currently visible.",
            );
        } else {
            let body = self
                .slash_entity_catalog
                .auth_profiles
                .iter()
                .take(8)
                .map(|profile| {
                    format!(
                        "{} · {} · {} · {}",
                        profile.profile_name,
                        profile.profile_id,
                        profile.provider_kind,
                        profile.scope_kind
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            self.push_entry(EntryKind::System, "Profiles", body);
        }
        self.status_line = "Auth profile catalog refreshed".to_owned();
        Ok(())
    }

    async fn handle_browser_command(&mut self, arguments: Vec<String>) -> Result<()> {
        if self.slash_entity_catalog.browser_profiles.is_empty()
            && self.slash_entity_catalog.browser_sessions.is_empty()
        {
            self.refresh_browser_catalog().await?;
        }
        let target = normalize_optional_text(arguments.join(" "));
        if let Some(target) = target {
            let target = target.to_ascii_lowercase();
            if let Some(session) = self
                .slash_entity_catalog
                .browser_sessions
                .iter()
                .find(|session| session.session_id.to_ascii_lowercase() == target)
                .cloned()
            {
                let context = self.connect_admin_console().await?;
                let payload = context
                    .client
                    .get_json_value(format!(
                        "console/v1/browser/sessions/{}",
                        percent_encode_component(session.session_id.as_str())
                    ))
                    .await?;
                self.push_entry(
                    EntryKind::System,
                    "Browser session",
                    serde_json::to_string_pretty(&payload)?,
                );
                self.status_line = "Browser session detail loaded".to_owned();
                return Ok(());
            }
            if let Some(profile) = self
                .slash_entity_catalog
                .browser_profiles
                .iter()
                .find(|profile| {
                    profile.profile_id.to_ascii_lowercase() == target
                        || profile.name.to_ascii_lowercase() == target
                })
                .cloned()
            {
                self.push_entry(
                    EntryKind::System,
                    "Browser profile",
                    format!(
                        "{}\nprofile_id={}\npersistence={} private={}\nUse `palyra browser profiles list` or the web browser workbench for richer actions.",
                        profile.name,
                        profile.profile_id,
                        profile.persistence_enabled,
                        profile.private_profile
                    ),
                );
                self.status_line = format!("Loaded browser profile {}", profile.name);
                return Ok(());
            }
            self.status_line =
                "Browser profile or session not found in the loaded catalog.".to_owned();
            self.ux_metrics.record(TuiUxMetricKey::Errors);
            return Ok(());
        }
        let mut lines = Vec::new();
        if self.slash_entity_catalog.browser_profiles.is_empty() {
            lines.push("No browser profiles visible.".to_owned());
        } else {
            lines.push("Profiles:".to_owned());
            lines.extend(self.slash_entity_catalog.browser_profiles.iter().take(6).map(
                |profile| {
                    format!(
                        "  {} · {} · {}",
                        profile.name,
                        profile.profile_id,
                        if profile.persistence_enabled { "persistent" } else { "ephemeral" }
                    )
                },
            ));
        }
        if self.slash_entity_catalog.browser_sessions.is_empty() {
            lines.push("No browser sessions visible.".to_owned());
        } else {
            lines.push("Sessions:".to_owned());
            lines.extend(
                self.slash_entity_catalog
                    .browser_sessions
                    .iter()
                    .take(6)
                    .map(|session| format!("  {} · {}", session.title, session.session_id)),
            );
        }
        self.push_entry(EntryKind::System, "Browser", lines.join("\n"));
        self.status_line = "Browser catalog refreshed".to_owned();
        Ok(())
    }

    async fn handle_doctor_command(&mut self, arguments: Vec<String>) -> Result<()> {
        let action = arguments.first().map(String::as_str).unwrap_or("jobs");
        let context = self.connect_admin_console().await?;
        match action {
            "jobs" => {
                let payload = context
                    .client
                    .get_json_value("console/v1/doctor/jobs?limit=8".to_owned())
                    .await?;
                let jobs = payload
                    .pointer("/jobs")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if jobs.is_empty() {
                    self.push_entry(
                        EntryKind::System,
                        "Doctor",
                        "No doctor recovery jobs recorded.",
                    );
                } else {
                    let body = jobs
                        .iter()
                        .map(|job| {
                            format!(
                                "{} · {} · {}",
                                read_json_string(job, "/job_id"),
                                read_json_string(job, "/state"),
                                read_json_string(job, "/command_output")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.push_entry(EntryKind::System, "Doctor jobs", body);
                }
                self.status_line = "Doctor jobs refreshed".to_owned();
            }
            "run" | "repair" => {
                let payload = context
                    .client
                    .post_json_value(
                        "console/v1/doctor/jobs".to_owned(),
                        &serde_json::json!({
                            "dry_run": action == "run",
                            "repair": action == "repair",
                        }),
                    )
                    .await?;
                let job_id = read_json_string(&payload, "/job/job_id");
                let state = read_json_string(&payload, "/job/state");
                self.push_entry(
                    EntryKind::System,
                    "Doctor job",
                    format!(
                        "Queued doctor {action} job {} ({state}).",
                        shorten_id(job_id.as_str())
                    ),
                );
                self.status_line = format!("Doctor {action} job queued");
            }
            _ => {
                self.status_line = "Usage: /doctor [jobs|run|repair]".to_owned();
                self.ux_metrics.record(TuiUxMetricKey::Errors);
            }
        }
        Ok(())
    }

    async fn handle_objective_command(
        &mut self,
        fixed_kind: Option<&'static str>,
        arguments: Vec<String>,
    ) -> Result<()> {
        let context = self.connect_admin_console().await?;
        let Some(command) = arguments.first().map(String::as_str) else {
            let label = fixed_kind.unwrap_or("objective");
            self.status_line =
                format!("Usage: /{label} list|show|select|fire|pause|resume|archive|create");
            return Ok(());
        };
        match command {
            "list" => {
                let payload = crate::commands::objectives::list_objectives_value(
                    &context.client,
                    None,
                    Some(10),
                    fixed_kind,
                    None,
                )
                .await?;
                let objectives = payload
                    .pointer("/objectives")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut lines = Vec::new();
                if objectives.is_empty() {
                    lines.push("No objectives found.".to_owned());
                } else {
                    for objective in objectives.iter().take(10) {
                        let objective_id = objective
                            .pointer("/objective_id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("unknown");
                        let kind = objective
                            .pointer("/kind")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("objective");
                        let state = objective
                            .pointer("/state")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("unknown");
                        let name = objective
                            .pointer("/name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("Untitled");
                        let focus = objective
                            .pointer("/current_focus")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("No focus recorded.");
                        lines.push(format!("{objective_id} [{kind}/{state}] {name}"));
                        lines.push(format!("  focus: {focus}"));
                    }
                }
                self.push_entry(EntryKind::System, "Objectives", lines.join("\n"));
                self.status_line = "Objective list refreshed".to_owned();
            }
            "select" => {
                let objective_id =
                    arguments.get(1).cloned().context("Usage: /objective select <objective_id>")?;
                let payload = crate::commands::objectives::get_objective_value(
                    &context.client,
                    objective_id.as_str(),
                )
                .await?;
                let resolved_id = payload
                    .pointer("/objective/objective_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(objective_id.as_str())
                    .to_owned();
                self.selected_objective_id = Some(resolved_id.clone());
                self.push_entry(
                    EntryKind::System,
                    "Objective selected",
                    format!("Selected objective {resolved_id}."),
                );
                self.status_line = format!("Objective selected: {resolved_id}");
            }
            "show" => {
                let objective_id = resolve_tui_objective_reference(
                    arguments.get(1).cloned(),
                    self.selected_objective_id.as_ref(),
                )?;
                let payload = crate::commands::objectives::get_objective_summary_value(
                    &context.client,
                    objective_id.as_str(),
                )
                .await?;
                let markdown = payload
                    .pointer("/summary_markdown")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Objective summary is unavailable.");
                self.selected_objective_id = Some(objective_id.clone());
                self.push_entry(EntryKind::System, "Objective summary", markdown);
                self.status_line = format!("Objective summary loaded: {objective_id}");
            }
            "fire" | "pause" | "resume" | "archive" => {
                let objective_id = resolve_tui_objective_reference(
                    arguments.get(1).cloned(),
                    self.selected_objective_id.as_ref(),
                )?;
                let payload = crate::commands::objectives::objective_lifecycle_value(
                    &context.client,
                    objective_id.as_str(),
                    command,
                    None,
                )
                .await?;
                let objective = payload.pointer("/objective").cloned().unwrap_or(payload);
                let state = objective
                    .pointer("/state")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown");
                let name = objective
                    .pointer("/name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Untitled objective");
                self.selected_objective_id = Some(objective_id.clone());
                self.push_entry(
                    EntryKind::System,
                    "Objective lifecycle",
                    format!("{objective_id} [{state}] {name}"),
                );
                self.status_line = format!("Objective {command}: {objective_id}");
            }
            "create" => {
                let spec = parse_tui_objective_create_spec(fixed_kind, arguments.as_slice())?;
                let payload = crate::commands::objectives::upsert_objective_value(
                    &context.client,
                    &serde_json::Map::from_iter([
                        ("kind".to_owned(), serde_json::Value::String(spec.kind.to_owned())),
                        ("name".to_owned(), serde_json::Value::String(spec.name.clone())),
                        ("prompt".to_owned(), serde_json::Value::String(spec.prompt.clone())),
                        ("enabled".to_owned(), serde_json::Value::Bool(true)),
                        (
                            "delivery_mode".to_owned(),
                            serde_json::Value::String("same_channel".to_owned()),
                        ),
                        (
                            "approval_mode".to_owned(),
                            serde_json::Value::String(
                                if spec.kind == "standing_order" || spec.kind == "program" {
                                    "before_first_run"
                                } else {
                                    "none"
                                }
                                .to_owned(),
                            ),
                        ),
                        (
                            "template_id".to_owned(),
                            if spec.kind == "heartbeat" {
                                serde_json::Value::String("heartbeat".to_owned())
                            } else {
                                serde_json::Value::Null
                            },
                        ),
                        (
                            "natural_language_schedule".to_owned(),
                            if spec.kind == "heartbeat" {
                                serde_json::Value::String("every weekday at 9".to_owned())
                            } else {
                                serde_json::Value::Null
                            },
                        ),
                    ]),
                )
                .await?;
                let objective = payload.pointer("/objective").cloned().unwrap_or(payload);
                let objective_id = objective
                    .pointer("/objective_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .to_owned();
                let state = objective
                    .pointer("/state")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown");
                self.selected_objective_id = Some(objective_id.clone());
                self.push_entry(
                    EntryKind::System,
                    "Objective created",
                    format!("{objective_id} [{state}] {}", spec.name),
                );
                self.status_line = format!("Objective created: {objective_id}");
            }
            other => {
                self.status_line = format!("Unknown objective subcommand: {other}");
            }
        }
        Ok(())
    }

    async fn handle_compaction_command(&mut self, arguments: Vec<String>) -> Result<()> {
        let session_id = self.active_session_id()?;
        let context = self.connect_admin_console().await?;
        let spec = parse_tui_compaction_arguments(arguments.as_slice())?;
        if spec.history {
            let payload = context
                .client
                .get_json_value(format!(
                    "console/v1/chat/sessions/{}/transcript",
                    percent_encode_component(session_id.as_str())
                ))
                .await?;
            let compactions = payload
                .pointer("/compactions")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let checkpoints = payload
                .pointer("/checkpoints")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut lines = Vec::new();
            if compactions.is_empty() {
                lines.push("Compactions: none".to_owned());
            } else {
                lines.push("Compactions:".to_owned());
                for artifact in compactions.iter().rev().take(3) {
                    let artifact_id = artifact
                        .pointer("/artifact_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown");
                    let summary = artifact
                        .pointer("/summary_json")
                        .and_then(serde_json::Value::as_str)
                        .and_then(parse_tui_json_string);
                    let lifecycle = summary
                        .as_ref()
                        .and_then(|value| value.pointer("/lifecycle_state"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("stored");
                    let review_count = summary
                        .as_ref()
                        .and_then(|value| value.pointer("/planner/review_candidate_count"))
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or_default();
                    lines.push(format!(
                        "- {} {} review={} {}",
                        shorten_id(artifact_id),
                        lifecycle,
                        review_count,
                        artifact
                            .pointer("/summary_preview")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("No compaction preview.")
                    ));
                }
            }
            if checkpoints.is_empty() {
                lines.push("Checkpoints: none".to_owned());
            } else {
                lines.push("Checkpoints:".to_owned());
                for checkpoint in checkpoints.iter().rev().take(3) {
                    let checkpoint_id = checkpoint
                        .pointer("/checkpoint_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown");
                    lines.push(format!(
                        "- {} {} restores={} {}",
                        shorten_id(checkpoint_id),
                        checkpoint
                            .pointer("/name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("Unnamed checkpoint"),
                        checkpoint
                            .pointer("/restore_count")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or_default(),
                        checkpoint
                            .pointer("/note")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("No note recorded.")
                    ));
                }
            }
            self.push_entry(EntryKind::System, "Compaction history", lines.join("\n"));
            self.status_line = "Compaction history loaded".to_owned();
            return Ok(());
        }

        let path = if spec.apply {
            format!(
                "console/v1/chat/sessions/{}/compactions",
                percent_encode_component(session_id.as_str())
            )
        } else {
            format!(
                "console/v1/chat/sessions/{}/compactions/preview",
                percent_encode_component(session_id.as_str())
            )
        };
        let payload = context
            .client
            .post_json_value(
                path,
                &serde_json::json!({
                    "accept_candidate_ids": spec.accept_candidate_ids,
                    "reject_candidate_ids": spec.reject_candidate_ids,
                }),
            )
            .await?;
        let preview = payload.pointer("/preview").cloned().unwrap_or_else(|| serde_json::json!({}));
        let summary_preview = preview
            .pointer("/summary_preview")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("No compaction preview available.");
        let token_delta =
            preview.pointer("/token_delta").and_then(serde_json::Value::as_u64).unwrap_or_default();
        let review_count = preview
            .pointer("/summary/planner/review_candidate_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        let write_count = preview
            .pointer("/summary/writes")
            .and_then(serde_json::Value::as_array)
            .map(|writes| writes.len())
            .unwrap_or_default();
        if spec.apply {
            self.refresh_checkpoint_catalog().await?;
            let artifact_id = payload
                .pointer("/artifact/artifact_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let checkpoint_id = payload
                .pointer("/checkpoint/checkpoint_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            self.push_entry(
                EntryKind::System,
                "Compaction",
                format!(
                    "Created compaction artifact {} and checkpoint {} with estimated token delta {}.\nPlanned writes: {} · Review candidates: {}\n{}",
                    shorten_id(artifact_id),
                    shorten_id(checkpoint_id),
                    token_delta,
                    write_count,
                    review_count,
                    summary_preview
                ),
            );
            self.status_line = "Compaction artifact created".to_owned();
        } else {
            self.push_entry(
                EntryKind::System,
                "Compaction preview",
                format!(
                    "Estimated token delta {}.\nPlanned writes: {} · Review candidates: {}\n{}",
                    token_delta, write_count, review_count, summary_preview
                ),
            );
            self.status_line = "Compaction preview loaded".to_owned();
        }
        Ok(())
    }

    async fn handle_checkpoint_command(&mut self, arguments: Vec<String>) -> Result<()> {
        let Some(action) = arguments.first().map(String::as_str) else {
            self.status_line =
                "Usage: /checkpoint save <name> | list | restore <checkpoint_id>".to_owned();
            return Ok(());
        };
        match action {
            "save" => {
                let name = normalize_optional_text(
                    arguments.iter().skip(1).cloned().collect::<Vec<_>>().join(" "),
                );
                let Some(name) = name else {
                    self.status_line = "Usage: /checkpoint save <name>".to_owned();
                    return Ok(());
                };
                let session_id = self.active_session_id()?;
                let context = self.connect_admin_console().await?;
                let payload = context
                    .client
                    .post_json_value(
                        format!(
                            "console/v1/chat/sessions/{}/checkpoints",
                            percent_encode_component(session_id.as_str())
                        ),
                        &serde_json::json!({
                            "name": name,
                            "tags": Vec::<String>::new(),
                        }),
                    )
                    .await?;
                self.refresh_checkpoint_catalog().await?;
                let checkpoint_id = payload
                    .pointer("/checkpoint/checkpoint_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown");
                self.push_entry(
                    EntryKind::System,
                    "Checkpoint",
                    format!("Saved checkpoint {}.", shorten_id(checkpoint_id)),
                );
                self.status_line = "Checkpoint created".to_owned();
            }
            "list" => {
                self.refresh_checkpoint_catalog().await?;
                if self.slash_entity_catalog.checkpoints.is_empty() {
                    self.push_entry(
                        EntryKind::System,
                        "Checkpoint",
                        "No checkpoints stored for this session.",
                    );
                } else {
                    let body = self
                        .slash_entity_catalog
                        .checkpoints
                        .iter()
                        .map(|checkpoint| {
                            format!(
                                "{} · {}{}",
                                shorten_id(checkpoint.checkpoint_id.as_str()),
                                if checkpoint.name.is_empty() {
                                    "unnamed".to_owned()
                                } else {
                                    checkpoint.name.clone()
                                },
                                if checkpoint_has_tag(checkpoint, "undo_safe") {
                                    " · undo-safe".to_owned()
                                } else {
                                    String::new()
                                }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.push_entry(EntryKind::System, "Checkpoint list", body);
                }
                self.status_line = "Checkpoint list refreshed".to_owned();
            }
            "restore" => {
                let Some(checkpoint_id) = arguments.get(1) else {
                    self.status_line = "Usage: /checkpoint restore <checkpoint_id>".to_owned();
                    return Ok(());
                };
                self.restore_checkpoint_with_notice(checkpoint_id.clone(), "checkpoint").await?;
            }
            _ => {
                self.status_line =
                    "Usage: /checkpoint save <name> | list | restore <checkpoint_id>".to_owned();
            }
        }
        Ok(())
    }

    async fn handle_background_command(&mut self, arguments: Vec<String>) -> Result<()> {
        let Some(action) = arguments.first().map(String::as_str) else {
            self.status_line =
                "Usage: /background list | add <text> | show|pause|resume|retry|cancel <task_id>"
                    .to_owned();
            return Ok(());
        };
        let context = self.connect_admin_console().await?;
        match action {
            "list" => {
                let session_id = self.active_session_id()?;
                let payload = context
                    .client
                    .get_json_value(format!(
                        "console/v1/chat/background-tasks?session_id={}&include_completed=true",
                        percent_encode_component(session_id.as_str())
                    ))
                    .await?;
                let tasks = payload
                    .pointer("/tasks")
                    .and_then(serde_json::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if tasks.is_empty() {
                    self.push_entry(
                        EntryKind::System,
                        "Background",
                        "No background tasks recorded.",
                    );
                } else {
                    let body = tasks
                        .iter()
                        .map(|task| {
                            format!(
                                "{} · {} · {}",
                                task.pointer("/task_id")
                                    .and_then(serde_json::Value::as_str)
                                    .map(shorten_id)
                                    .unwrap_or_else(|| "unknown".to_owned()),
                                task.pointer("/state")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("unknown"),
                                task.pointer("/input_text")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.push_entry(EntryKind::System, "Background list", body);
                }
                self.status_line = "Background tasks refreshed".to_owned();
            }
            "add" => {
                let text = normalize_optional_text(
                    arguments.iter().skip(1).cloned().collect::<Vec<_>>().join(" "),
                );
                let Some(text) = text else {
                    self.status_line = "Usage: /background add <text>".to_owned();
                    return Ok(());
                };
                let session_id = self.active_session_id()?;
                let payload = context
                    .client
                    .post_json_value(
                        format!(
                            "console/v1/chat/sessions/{}/background-tasks",
                            percent_encode_component(session_id.as_str())
                        ),
                        &serde_json::json!({ "text": text }),
                    )
                    .await?;
                let task_id = payload
                    .pointer("/task/task_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown");
                self.push_entry(
                    EntryKind::System,
                    "Background task",
                    format!("Queued background task {}.", shorten_id(task_id)),
                );
                self.status_line = "Background task queued".to_owned();
            }
            "show" | "pause" | "resume" | "retry" | "cancel" => {
                let Some(task_id) = arguments.get(1) else {
                    self.status_line = format!("Usage: /background {action} <task_id>");
                    return Ok(());
                };
                if action == "show" {
                    let payload = context
                        .client
                        .get_json_value(format!(
                            "console/v1/chat/background-tasks/{}",
                            percent_encode_component(task_id.as_str())
                        ))
                        .await?;
                    self.push_entry(
                        EntryKind::System,
                        "Background task",
                        serde_json::to_string_pretty(&payload)?,
                    );
                    self.status_line = "Background task detail loaded".to_owned();
                } else {
                    let payload = context
                        .client
                        .post_json_value(
                            format!(
                                "console/v1/chat/background-tasks/{}/{}",
                                percent_encode_component(task_id.as_str()),
                                action
                            ),
                            &serde_json::json!({}),
                        )
                        .await?;
                    let state = payload
                        .pointer("/task/state")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown");
                    self.push_entry(
                        EntryKind::System,
                        "Background task",
                        format!("Action {action} applied to {} -> {state}.", shorten_id(task_id)),
                    );
                    self.status_line = format!("Background task {action} queued");
                }
            }
            _ => {
                self.status_line =
                    "Usage: /background list | add <text> | show|pause|resume|retry|cancel <task_id>"
                        .to_owned();
            }
        }
        Ok(())
    }

    async fn delegate_background_run(&mut self, arguments: Vec<String>) -> Result<()> {
        let Some(selector) = arguments.first().map(String::as_str) else {
            self.status_line = format!(
                "Usage: /delegate <profile-or-template> <text> (profiles: {} | templates: {})",
                BUILT_IN_DELEGATION_PROFILES.join(", "),
                BUILT_IN_DELEGATION_TEMPLATES.join(", ")
            );
            return Ok(());
        };
        let text = normalize_optional_text(
            arguments.iter().skip(1).cloned().collect::<Vec<_>>().join(" "),
        );
        let Some(text) = text else {
            self.status_line = "Usage: /delegate <profile-or-template> <text>".to_owned();
            return Ok(());
        };
        let selector = selector.trim().to_ascii_lowercase();
        let delegation = if BUILT_IN_DELEGATION_TEMPLATES.contains(&selector.as_str()) {
            serde_json::json!({ "template_id": selector })
        } else if BUILT_IN_DELEGATION_PROFILES.contains(&selector.as_str()) {
            serde_json::json!({ "profile_id": selector })
        } else {
            self.status_line = format!(
                "Unknown delegation selector '{}'. Profiles: {}. Templates: {}.",
                selector,
                BUILT_IN_DELEGATION_PROFILES.join(", "),
                BUILT_IN_DELEGATION_TEMPLATES.join(", ")
            );
            return Ok(());
        };
        let session_id = self.active_session_id()?;
        let context = self.connect_admin_console().await?;
        let payload = context
            .client
            .post_json_value(
                format!(
                    "console/v1/chat/sessions/{}/background-tasks",
                    percent_encode_component(session_id.as_str())
                ),
                &serde_json::json!({
                    "text": text,
                    "delegation": delegation,
                }),
            )
            .await?;
        let task_id = payload
            .pointer("/task/task_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        self.push_entry(
            EntryKind::System,
            "Delegation",
            format!("Queued delegated child task {} via {}.", shorten_id(task_id), selector),
        );
        self.status_line = "Delegated background task queued".to_owned();
        Ok(())
    }

    async fn open_picker(&mut self, kind: PickerKind) -> Result<()> {
        if matches!(kind, PickerKind::Session) {
            return self.open_session_history_picker(None).await;
        }
        let picker = match kind {
            PickerKind::Agent => {
                let response = self.runtime.list_agents(None, Some(100)).await?;
                let selected_agent =
                    self.current_agent.as_ref().map(|agent| agent.agent_id.as_str());
                let items = response
                    .agents
                    .into_iter()
                    .map(|agent| PickerItem {
                        id: agent.agent_id.clone(),
                        title: format!(
                            "{}{}",
                            agent.display_name,
                            if response.default_agent_id == agent.agent_id {
                                " [default]"
                            } else {
                                ""
                            }
                        ),
                        detail: format!(
                            "{} | model={} | workspaces={}",
                            agent.agent_id,
                            agent.default_model_profile,
                            agent.workspace_roots.len()
                        ),
                    })
                    .collect::<Vec<_>>();
                PickerState {
                    kind,
                    title: "Agent picker".to_owned(),
                    selected: selected_agent
                        .and_then(|id| items.iter().position(|item| item.id == id))
                        .unwrap_or(0),
                    items,
                }
            }
            PickerKind::Session => unreachable!(),
            PickerKind::Model => {
                let models = self.runtime.list_models(None)?;
                let current_model = self
                    .current_session_catalog
                    .as_ref()
                    .and_then(|session| session.quick_controls.model.value.clone())
                    .or_else(|| models.status.text_model.clone());
                let items = models
                    .models
                    .iter()
                    .filter(|entry| entry.target == "text")
                    .map(|entry| PickerItem {
                        id: entry.id.clone(),
                        title: entry.id.clone(),
                        detail: format!(
                            "configured={} preferred={} source={}",
                            entry.configured, entry.preferred, entry.source
                        ),
                    })
                    .collect::<Vec<_>>();
                self.models = Some(models);
                PickerState {
                    kind,
                    title: "Model picker".to_owned(),
                    selected: current_model
                        .as_deref()
                        .and_then(|id| items.iter().position(|item| item.id == id))
                        .unwrap_or(0),
                    items,
                }
            }
        };
        self.mode = Mode::Picker(kind);
        self.pending_picker = Some(picker);
        Ok(())
    }

    async fn open_session_history_picker(&mut self, query: Option<String>) -> Result<()> {
        let response = self
            .runtime
            .list_session_catalog(vec![
                ("limit", Some("100".to_owned())),
                ("sort", Some("updated_desc".to_owned())),
                ("q", query.clone()),
                ("include_archived", self.include_archived_sessions.then(|| "true".to_owned())),
            ])
            .await?;
        let current_session_id =
            self.session.session_id.as_ref().map(|value| value.ulid.as_str()).unwrap_or_default();
        let items = response
            .sessions
            .into_iter()
            .map(|session| PickerItem {
                id: session.session_id.clone(),
                title: session.title,
                detail: {
                    let title_state = if session.manual_title_locked {
                        "manual"
                    } else {
                        session.title_generation_state.as_str()
                    };
                    let family_hint = if session.family.family_size > 1 {
                        format!("family {}/{}", session.family.sequence, session.family.family_size)
                    } else {
                        describe_branch_state_label(session.branch_state.as_str())
                    };
                    let workspace_hint = match session.recap.project_context.as_ref() {
                        Some(project_context) if project_context.active_entries > 0 => format!(
                            "{} project rule{}{}",
                            project_context.active_entries,
                            if project_context.active_entries == 1 { "" } else { "s" },
                            if project_context.warnings.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    " · {} warning{}",
                                    project_context.warnings.len(),
                                    if project_context.warnings.len() == 1 { "" } else { "s" }
                                )
                            }
                        ),
                        Some(project_context) if project_context.blocked_entries > 0 => format!(
                            "{} blocked project rule{}",
                            project_context.blocked_entries,
                            if project_context.blocked_entries == 1 { "" } else { "s" }
                        ),
                        _ if session.has_context_files => format!(
                            "{} context file{}",
                            session.recap.active_context_files.len(),
                            if session.recap.active_context_files.len() == 1 { "" } else { "s" }
                        ),
                        _ => "no project rules".to_owned(),
                    };
                    let summary = session
                        .preview
                        .or(session.last_summary)
                        .unwrap_or_else(|| "no preview".to_owned());
                    format!(
                        "{title_state} | {family_hint} | {workspace_hint} | updated={} | {summary}",
                        session.updated_at_unix_ms
                    )
                },
            })
            .collect::<Vec<_>>();
        let selected = items.iter().position(|item| item.id == current_session_id).unwrap_or(0);
        self.mode = Mode::Picker(PickerKind::Session);
        self.pending_picker = Some(PickerState {
            kind: PickerKind::Session,
            title: match query.as_deref() {
                Some(value) if !value.trim().is_empty() => format!("Session history: {value}"),
                _ => "Session history".to_owned(),
            },
            items,
            selected,
        });
        self.status_line = "Session history ready".to_owned();
        Ok(())
    }

    async fn apply_picker_selection(&mut self) -> Result<()> {
        let Some(picker) = self.pending_picker.clone() else {
            self.mode = Mode::Chat;
            return Ok(());
        };
        let Some(item) = picker.items.get(picker.selected) else {
            self.mode = Mode::Chat;
            return Ok(());
        };
        match picker.kind {
            PickerKind::Agent => self.switch_agent(item.id.clone()).await?,
            PickerKind::Session => self.switch_session(item.id.clone()).await?,
            PickerKind::Model => self.set_model(item.id.clone()).await?,
        }
        self.pending_picker = None;
        self.mode = Mode::Chat;
        Ok(())
    }

    async fn set_model(&mut self, model_id: String) -> Result<()> {
        let (request, message) = if quick_control_reset_requested(model_id.as_str()) {
            (
                SessionQuickControlsUpdateRequest {
                    model_profile: Some(None),
                    ..SessionQuickControlsUpdateRequest::default()
                },
                "Session model returned to the inherited default.".to_owned(),
            )
        } else {
            (
                SessionQuickControlsUpdateRequest {
                    model_profile: Some(Some(model_id.clone())),
                    ..SessionQuickControlsUpdateRequest::default()
                },
                format!("Session model override set to {model_id}."),
            )
        };
        self.update_current_session_quick_controls(request).await?;
        self.models = Some(self.runtime.list_models(None)?);
        self.push_entry(EntryKind::System, "Model", message.clone());
        self.status_line = message;
        Ok(())
    }

    async fn set_thinking_enabled(&mut self, enabled: bool) -> Result<()> {
        self.update_current_session_quick_controls(SessionQuickControlsUpdateRequest {
            thinking: Some(Some(enabled)),
            ..SessionQuickControlsUpdateRequest::default()
        })
        .await?;
        let message = format!("Session thinking {}.", if enabled { "enabled" } else { "disabled" });
        self.push_entry(EntryKind::System, "Thinking", message.clone());
        self.status_line = message;
        Ok(())
    }

    async fn clear_thinking_override(&mut self) -> Result<()> {
        self.update_current_session_quick_controls(SessionQuickControlsUpdateRequest {
            thinking: Some(None),
            ..SessionQuickControlsUpdateRequest::default()
        })
        .await?;
        let message = "Session thinking returned to the inherited default.".to_owned();
        self.push_entry(EntryKind::System, "Thinking", message.clone());
        self.status_line = message;
        Ok(())
    }

    async fn set_trace_enabled(&mut self, enabled: bool) -> Result<()> {
        self.update_current_session_quick_controls(SessionQuickControlsUpdateRequest {
            trace: Some(Some(enabled)),
            ..SessionQuickControlsUpdateRequest::default()
        })
        .await?;
        let message = format!("Session trace {}.", if enabled { "enabled" } else { "disabled" });
        self.push_entry(EntryKind::System, "Trace", message.clone());
        self.status_line = message;
        Ok(())
    }

    async fn clear_trace_override(&mut self) -> Result<()> {
        self.update_current_session_quick_controls(SessionQuickControlsUpdateRequest {
            trace: Some(None),
            ..SessionQuickControlsUpdateRequest::default()
        })
        .await?;
        let message = "Session trace returned to the inherited default.".to_owned();
        self.push_entry(EntryKind::System, "Trace", message.clone());
        self.status_line = message;
        Ok(())
    }

    async fn set_verbose_enabled(&mut self, enabled: bool) -> Result<()> {
        self.update_current_session_quick_controls(SessionQuickControlsUpdateRequest {
            verbose: Some(Some(enabled)),
            ..SessionQuickControlsUpdateRequest::default()
        })
        .await?;
        let message = format!("Session verbose {}.", if enabled { "enabled" } else { "disabled" });
        self.push_entry(EntryKind::System, "Verbose", message.clone());
        self.status_line = message;
        Ok(())
    }

    async fn clear_verbose_override(&mut self) -> Result<()> {
        self.update_current_session_quick_controls(SessionQuickControlsUpdateRequest {
            verbose: Some(None),
            ..SessionQuickControlsUpdateRequest::default()
        })
        .await?;
        let message = "Session verbose returned to the inherited default.".to_owned();
        self.push_entry(EntryKind::System, "Verbose", message.clone());
        self.status_line = message;
        Ok(())
    }

    async fn update_current_session_quick_controls(
        &mut self,
        request: SessionQuickControlsUpdateRequest,
    ) -> Result<()> {
        let session_id = self.active_session_id()?;
        let refresh_agent_identity =
            request.agent_id.is_some() || request.reset_to_default == Some(true);
        let response =
            self.runtime.update_session_quick_controls(session_id.as_str(), &request).await?;
        self.current_session_catalog = Some(response.session);
        self.sync_session_quick_controls_from_catalog();
        if refresh_agent_identity {
            self.refresh_agent_identity(None, false).await?;
        }
        Ok(())
    }

    async fn refresh_current_session_catalog(&mut self) -> Result<()> {
        let session_id = self.active_session_id()?;
        let response = self.runtime.get_session_catalog_entry(session_id.as_str()).await?;
        self.current_session_catalog = Some(response.session);
        self.sync_session_quick_controls_from_catalog();
        Ok(())
    }

    async fn refresh_session_runtime_snapshot(&mut self) -> Result<()> {
        let session_id = self.active_session_id()?;
        let context = self.connect_admin_console().await?;
        let mut snapshot = SessionRuntimeSnapshot {
            session_total_tokens: self
                .current_session_catalog
                .as_ref()
                .map(|session| session.total_tokens)
                .unwrap_or_default(),
            latest_started_at_unix_ms: self
                .current_session_catalog
                .as_ref()
                .and_then(|session| session.last_run_started_at_unix_ms),
            ..SessionRuntimeSnapshot::default()
        };
        let usage_path = format!(
            "console/v1/usage/sessions/{}?run_limit=8",
            percent_encode_component(session_id.as_str())
        );
        let usage_result = context.client.get_json_value(usage_path).await;
        let transcript_path = format!(
            "console/v1/chat/sessions/{}/transcript",
            percent_encode_component(session_id.as_str())
        );
        let transcript_result = context.client.get_json_value(transcript_path).await;

        if let Ok(usage) = usage_result.as_ref() {
            snapshot.session_total_tokens = read_json_u64(usage, "/session/total_tokens")
                .max(read_json_u64(usage, "/totals/total_tokens"))
                .max(snapshot.session_total_tokens);
            snapshot.session_runs = read_json_u64(usage, "/session/runs") as usize;
            snapshot.average_latency_ms = read_json_f64(usage, "/session/average_latency_ms")
                .or_else(|| read_json_f64(usage, "/totals/average_latency_ms"));
            snapshot.estimated_cost_usd = read_json_f64(usage, "/session/estimated_cost_usd")
                .or_else(|| read_json_f64(usage, "/totals/estimated_cost_usd"));
            snapshot.latest_started_at_unix_ms =
                read_json_optional_i64(usage, "/session/latest_started_at_unix_ms")
                    .or(snapshot.latest_started_at_unix_ms);
            if let Some(latest_run) = usage
                .pointer("/runs")
                .and_then(serde_json::Value::as_array)
                .and_then(|runs| runs.first())
            {
                snapshot.latest_run_total_tokens = read_json_u64(latest_run, "/total_tokens");
                snapshot.latest_started_at_unix_ms =
                    read_json_optional_i64(latest_run, "/started_at_unix_ms")
                        .or(snapshot.latest_started_at_unix_ms);
                snapshot.latest_completed_at_unix_ms =
                    read_json_optional_i64(latest_run, "/completed_at_unix_ms");
            }
        }

        if let Ok(transcript) = transcript_result.as_ref() {
            snapshot.attachment_count = transcript
                .pointer("/attachments")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len)
                .unwrap_or_default();
            let tasks = transcript
                .pointer("/background_tasks")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            snapshot.background_task_count = tasks.len();
            snapshot.active_background_task_count = tasks
                .iter()
                .filter(|task| background_task_is_active(read_json_string(task, "/state").as_str()))
                .count();
            if snapshot.latest_completed_at_unix_ms.is_none() {
                snapshot.latest_completed_at_unix_ms = transcript
                    .pointer("/runs")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|runs| runs.first())
                    .and_then(|run| read_json_optional_i64(run, "/completed_at_unix_ms"));
            }
        }

        self.session_runtime = snapshot;
        match (usage_result.err(), transcript_result.err()) {
            (Some(usage_error), Some(transcript_error)) => Err(anyhow!(
                "usage metadata unavailable: {usage_error}; transcript metadata unavailable: {transcript_error}"
            )),
            (Some(error), None) => Err(anyhow!("usage metadata unavailable: {error}")),
            (None, Some(error)) => Err(anyhow!("transcript metadata unavailable: {error}")),
            (None, None) => Ok(()),
        }
    }

    fn context_budget_summary(&self) -> ContextBudgetSummary {
        let baseline_tokens = self
            .current_session_catalog
            .as_ref()
            .map(|session| session.total_tokens)
            .unwrap_or_default()
            .max(self.session_runtime.session_total_tokens);
        let draft_tokens = estimate_text_tokens(self.composer.text());
        let project_context_tokens = self
            .current_session_catalog
            .as_ref()
            .and_then(|session| session.recap.project_context.as_ref())
            .map(|project_context| project_context.active_estimated_tokens as u64)
            .unwrap_or_default();
        let attachment_tokens =
            self.pending_attachments.iter().map(|attachment| attachment.budget_tokens).sum::<u64>();
        let estimated_total_tokens =
            baseline_tokens + draft_tokens + project_context_tokens + attachment_tokens;
        let ratio = if CONTEXT_BUDGET_HARD_LIMIT == 0 {
            0.0
        } else {
            (estimated_total_tokens as f64) / (CONTEXT_BUDGET_HARD_LIMIT as f64)
        };
        let tone = if estimated_total_tokens >= CONTEXT_BUDGET_HARD_LIMIT {
            StatusTone::Danger
        } else if estimated_total_tokens >= CONTEXT_BUDGET_SOFT_LIMIT {
            StatusTone::Warning
        } else {
            StatusTone::Default
        };
        let warning = match tone {
            StatusTone::Danger => Some(
                "Estimated context is above the safe working budget. Branch or compact before another heavy turn."
                    .to_owned(),
            ),
            StatusTone::Warning => Some(
                "Estimated context is approaching the budget. Keep the next turn compact."
                    .to_owned(),
            ),
            StatusTone::Default => None,
        };
        ContextBudgetSummary {
            draft_tokens,
            project_context_tokens,
            attachment_tokens,
            estimated_total_tokens,
            limit_tokens: CONTEXT_BUDGET_HARD_LIMIT,
            ratio,
            tone,
            label: format!(
                "{} / {} est.",
                format_approx_tokens(estimated_total_tokens),
                format_approx_tokens(CONTEXT_BUDGET_HARD_LIMIT)
            ),
            warning,
        }
    }

    fn push_session_recap(&mut self, fallback: Option<TuiSlashSessionRecord>) {
        if let Some(session) = self.current_session_catalog.as_ref() {
            let mut lines = Vec::new();
            let workspace_checkpoint_count = self.slash_entity_catalog.workspace_checkpoints.len();
            let summary =
                session.last_summary.clone().or_else(|| session.preview.clone()).unwrap_or_else(
                    || "No stored recap is available for this session yet.".to_owned(),
                );
            lines.push(summary);
            lines.push(format!(
                "Approvals {} · Artifacts {} · Context {} · Rollback {} · Family {}/{}",
                session.pending_approvals,
                session.artifact_count,
                session.recap.active_context_files.len(),
                workspace_checkpoint_count,
                session.family.sequence,
                session.family.family_size
            ));
            if !session.recap.recent_artifacts.is_empty() {
                lines.push(format!(
                    "Recent artifacts: {}",
                    session
                        .recap
                        .recent_artifacts
                        .iter()
                        .take(3)
                        .map(|artifact| artifact.label.as_str())
                        .collect::<Vec<_>>()
                        .join(" · ")
                ));
            }
            if !session.recap.touched_files.is_empty() {
                lines.push(format!(
                    "Touched: {}",
                    session
                        .recap
                        .touched_files
                        .iter()
                        .take(3)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" · ")
                ));
            }
            if let Some(workspace_checkpoint) =
                self.slash_entity_catalog.workspace_checkpoints.first()
            {
                lines.push(format!(
                    "Workspace: /workspace · /workspace show <artifact> · /rollback diff {}",
                    workspace_checkpoint.checkpoint_id
                ));
            } else if session.artifact_count > 0 {
                lines.push("Workspace: /workspace lists changed files and `/workspace handoff` jumps into the web inspector.".to_owned());
            }
            if !session.recap.ctas.is_empty() {
                lines.push(format!("Quick actions: {}", session.recap.ctas.join(" · ")));
            }
            if session.family.parent_session_id.is_some() || !session.family.relatives.is_empty() {
                let mut family_actions = Vec::new();
                if session.family.parent_session_id.is_some() {
                    family_actions.push("/resume parent".to_owned());
                }
                if session.family.relatives.iter().any(|relative| relative.relation == "sibling") {
                    family_actions.push("/resume sibling".to_owned());
                }
                if session.family.relatives.iter().any(|relative| relative.relation == "child") {
                    family_actions.push("/resume child".to_owned());
                }
                family_actions.push(format!("/history {}", session.family.root_title));
                lines.push(format!("Family: {}", family_actions.join(" · ")));
            }
            self.push_entry(EntryKind::System, "Recap", lines.join("\n"));
            return;
        }
        if let Some(recap_session) = fallback {
            let recap = if !recap_session.last_summary.is_empty() {
                recap_session.last_summary
            } else if !recap_session.preview.is_empty() {
                recap_session.preview
            } else if recap_session.root_title != recap_session.title {
                recap_session.root_title
            } else {
                "No stored recap is available for this session yet.".to_owned()
            };
            self.push_entry(EntryKind::System, "Recap", recap);
        }
    }

    fn sync_session_quick_controls_from_catalog(&mut self) {
        let Some(session) = self.current_session_catalog.as_ref() else {
            return;
        };
        self.show_tools = session.quick_controls.trace.value;
        self.show_thinking = session.quick_controls.thinking.value;
        self.show_verbose = session.quick_controls.verbose.value;
    }

    fn current_session_model_display(&self) -> String {
        self.current_session_catalog
            .as_ref()
            .map(|session| session.quick_controls.model.display_value.trim())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                self.models
                    .as_ref()
                    .and_then(|models| models.status.text_model.as_deref())
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| "none".to_owned())
    }

    fn current_session_agent_display(&self) -> String {
        self.current_session_catalog
            .as_ref()
            .map(|session| session.quick_controls.agent.display_value.trim())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| self.current_agent.as_ref().map(|agent| agent.agent_id.clone()))
            .unwrap_or_else(|| "none".to_owned())
    }

    fn current_session_model_override_active(&self) -> bool {
        self.current_session_catalog
            .as_ref()
            .map(|session| session.quick_controls.model.override_active)
            .unwrap_or(false)
    }

    fn current_session_agent_override_active(&self) -> bool {
        self.current_session_catalog
            .as_ref()
            .map(|session| session.quick_controls.agent.override_active)
            .unwrap_or(false)
    }

    async fn refresh_agent_identity(
        &mut self,
        preferred_agent_id: Option<String>,
        persist_binding: bool,
    ) -> Result<()> {
        let response = self
            .runtime
            .resolve_agent_for_context(AgentContextResolveInput {
                principal: self.runtime.connection().principal.clone(),
                channel: self.runtime.connection().channel.clone(),
                session_id: self.session.session_id.clone(),
                preferred_agent_id: preferred_agent_id.unwrap_or_default(),
                persist_session_binding: persist_binding,
            })
            .await?;
        self.current_agent = response.agent;
        self.current_agent_source = agent_resolution_source_label(response.source);
        Ok(())
    }

    async fn reload_runtime_state(&mut self) -> Result<()> {
        self.refresh_agent_identity(None, false).await?;
        self.models = Some(self.runtime.list_models(None)?);
        self.refresh_slash_entity_catalogs().await?;
        let _ = self.refresh_session_runtime_snapshot().await;
        self.sync_slash_palette();
        self.status_line = "Runtime state reloaded".to_owned();
        Ok(())
    }

    fn status_summary(&self) -> String {
        let handoff = build_console_handoff_path(&TuiCrossSurfaceHandoff {
            section: "chat".to_owned(),
            session_id: self.active_session_id().ok(),
            run_id: self.last_run_id.clone(),
            objective_id: self.selected_objective_id.clone(),
            source: Some("tui".to_owned()),
            ..TuiCrossSurfaceHandoff::default()
        });
        let profile =
            app::current_root_context().and_then(|context| context.active_profile_context());
        let budget = self.context_budget_summary();
        let workspace_artifacts = self.slash_entity_catalog.workspace_artifacts.len().max(
            self.current_session_catalog
                .as_ref()
                .map(|session| session.artifact_count)
                .unwrap_or_default(),
        );
        let workspace_checkpoints = self.slash_entity_catalog.workspace_checkpoints.len();
        let duration = current_run_duration_ms(self)
            .map(format_duration_ms)
            .unwrap_or_else(|| "idle".to_owned());
        [
            format!(
                "Session {}\nBranch {} · Agent {} ({}) [{}] · Model {} [{}]",
                display_session_identity(&self.session),
                if self.session.branch_state.trim().is_empty() {
                    "none"
                } else {
                    self.session.branch_state.as_str()
                },
                self.current_session_agent_display(),
                self.current_agent_source,
                if self.current_session_agent_override_active() {
                    "session"
                } else {
                    "inherited"
                },
                self.current_session_model_display(),
                if self.current_session_model_override_active() {
                    "session"
                } else {
                    "inherited"
                },
            ),
            format!(
                "Budget {} · Attach {} · Context {} · Approvals {} · Background {} · Workspace {}/{}",
                budget.label,
                self.pending_attachments.len(),
                self.current_session_catalog
                    .as_ref()
                    .map(|session| session.recap.active_context_files.len())
                    .unwrap_or_default(),
                self.current_session_catalog
                    .as_ref()
                    .map(|session| session.pending_approvals)
                    .unwrap_or_default(),
                self.session_runtime.active_background_task_count,
                workspace_artifacts,
                workspace_checkpoints,
            ),
            format!(
                "Run {} · Tokens {} · Cost {} · Connection {}",
                duration,
                format_approx_tokens(self.session_runtime.latest_run_total_tokens),
                self.session_runtime
                    .estimated_cost_usd
                    .map(format_cost_usd)
                    .unwrap_or_else(|| "n/a".to_owned()),
                connection_posture_label(self.runtime.connection().grpc_url.as_str()),
            ),
            format!(
                "Profile {} · Env {} · Risk {} · Strict {} · Tools {} · Thinking {} · Verbose {} · Shell {}",
                profile.as_ref().map(|value| value.label.as_str()).unwrap_or("none"),
                profile.as_ref().map(|value| value.environment.as_str()).unwrap_or("none"),
                profile.as_ref().map(|value| value.risk_level.as_str()).unwrap_or("none"),
                profile.as_ref().map(|value| if value.strict_mode { "on" } else { "off" }).unwrap_or("off"),
                self.show_tools,
                self.show_thinking,
                self.show_verbose,
                self.local_shell_enabled,
            ),
            format!("Handoff {handoff}"),
        ]
        .join("\n")
    }

    fn status_detail_summary(&self) -> String {
        let budget = self.context_budget_summary();
        let workspace_artifacts = self.slash_entity_catalog.workspace_artifacts.len().max(
            self.current_session_catalog
                .as_ref()
                .map(|session| session.artifact_count)
                .unwrap_or_default(),
        );
        let workspace_checkpoints = self.slash_entity_catalog.workspace_checkpoints.len();
        let latest_workspace_artifact = self
            .slash_entity_catalog
            .workspace_artifacts
            .first()
            .map(|artifact| artifact.display_path.as_str())
            .unwrap_or("none");
        let latest_workspace_checkpoint = self
            .slash_entity_catalog
            .workspace_checkpoints
            .first()
            .map(|checkpoint| checkpoint.checkpoint_id.as_str())
            .unwrap_or("none");
        [
            self.status_summary(),
            format!(
                "Context detail total={} / {} · draft={} · project_context={} · attachments={}",
                format_approx_tokens(budget.estimated_total_tokens),
                format_approx_tokens(budget.limit_tokens),
                format_approx_tokens(budget.draft_tokens),
                format_approx_tokens(budget.project_context_tokens),
                format_approx_tokens(budget.attachment_tokens),
            ),
            format!(
                "Workspace detail artifacts={} · checkpoints={} · latest artifact={} · latest rollback={}",
                workspace_artifacts,
                workspace_checkpoints,
                latest_workspace_artifact,
                if latest_workspace_checkpoint == "none" {
                    "none".to_owned()
                } else {
                    shorten_id(latest_workspace_checkpoint)
                },
            ),
            format!(
                "Session attachments stored={} · background queued={} active={}",
                self.session_runtime.attachment_count,
                self.session_runtime.background_task_count,
                self.session_runtime.active_background_task_count,
            ),
        ]
        .join("\n")
    }
}

async fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
        return Ok(true);
    }
    match app.mode {
        Mode::Help => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => app.mode = Mode::Chat,
            _ => {}
        },
        Mode::Approval => handle_approval_key(app, key).await?,
        Mode::ShellConfirm => handle_shell_confirm_key(app, key).await?,
        Mode::Settings => handle_settings_key(app, key).await?,
        Mode::Picker(_) => handle_picker_key(app, key).await?,
        Mode::Chat => handle_chat_key(app, key).await?,
    }
    Ok(false)
}

async fn handle_chat_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Tab => {
            if matches!(app.focus, Focus::Input)
                && app
                    .pending_slash_palette
                    .as_ref()
                    .map(|palette| !palette.suggestions.is_empty())
                    .unwrap_or(false)
            {
                app.apply_selected_slash_suggestion(true);
            } else {
                if matches!(app.focus, Focus::Input) {
                    app.stash_current_draft();
                }
                app.focus = match app.focus {
                    Focus::Transcript => Focus::Input,
                    Focus::Input => Focus::Transcript,
                };
            }
        }
        KeyCode::Esc
            if matches!(app.focus, Focus::Input) && app.pending_slash_palette.is_some() =>
        {
            app.dismiss_slash_palette();
            app.status_line = "Slash palette dismissed".to_owned();
        }
        KeyCode::Esc if matches!(app.focus, Focus::Input) && app.composer.has_selection() => {
            app.composer.clear_selection();
            app.status_line = "Composer selection cleared".to_owned();
        }
        KeyCode::Char('?') => app.mode = Mode::Help,
        KeyCode::F(2) => app.open_picker(PickerKind::Agent).await?,
        KeyCode::F(3) => app.open_picker(PickerKind::Session).await?,
        KeyCode::F(4) => app.open_picker(PickerKind::Model).await?,
        KeyCode::F(5) => app.mode = Mode::Settings,
        KeyCode::F(8) => {
            app.show_tips = !app.show_tips;
            app.status_line = if app.show_tips {
                "TUI tips are visible again".to_owned()
            } else {
                "TUI tips hidden; press F8 to re-open them".to_owned()
            };
        }
        KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
            app.reload_runtime_state().await?;
        }
        KeyCode::Char('o')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(app.focus, Focus::Input) =>
        {
            if app.composer.trimmed_text().is_empty() {
                app.composer.set_text("/attach ");
                app.sync_composer_after_edit();
                app.status_line =
                    "Attachment flow primed. Type a path and press Enter to upload it.".to_owned();
            } else {
                app.status_line =
                    "Use `/attach <path>` directly or clear the current draft before Ctrl+O."
                        .to_owned();
            }
        }
        KeyCode::Enter
            if matches!(app.focus, Focus::Input) && key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.composer.insert_newline();
            app.sync_composer_after_edit();
        }
        KeyCode::Char('j')
            if matches!(app.focus, Focus::Input)
                && key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            app.composer.insert_newline();
            app.sync_composer_after_edit();
        }
        KeyCode::Enter if matches!(app.focus, Focus::Input) => app.submit_input().await?,
        KeyCode::Backspace if matches!(app.focus, Focus::Input) => {
            app.composer.backspace();
            app.sync_composer_after_edit();
        }
        KeyCode::Delete if matches!(app.focus, Focus::Input) => {
            app.composer.delete();
            app.sync_composer_after_edit();
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if matches!(app.focus, Focus::Input) {
                app.composer.clear();
                app.pending_attachments.clear();
                app.clear_current_draft();
                app.sync_slash_palette();
            }
        }
        KeyCode::Char('a')
            if matches!(app.focus, Focus::Input)
                && key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            app.composer.select_all();
        }
        KeyCode::Char(ch) if matches!(app.focus, Focus::Input) && key.modifiers.is_empty() => {
            app.composer.insert_text(ch.to_string().as_str());
            app.sync_composer_after_edit();
        }
        KeyCode::Up => {
            if matches!(app.focus, Focus::Input) && app.pending_slash_palette.is_some() {
                app.slash_palette_selected = app.slash_palette_selected.saturating_sub(1);
            } else if matches!(app.focus, Focus::Input) {
                app.composer.move_vertical(-1, key.modifiers.contains(KeyModifiers::SHIFT));
            } else if matches!(app.focus, Focus::Transcript) {
                app.scroll_offset = app.scroll_offset.saturating_add(1);
            }
        }
        KeyCode::Down => {
            if matches!(app.focus, Focus::Input) {
                if let Some(palette) = app.pending_slash_palette.as_ref() {
                    app.slash_palette_selected = (app.slash_palette_selected + 1)
                        .min(palette.suggestions.len().saturating_sub(1));
                } else {
                    app.composer.move_vertical(1, key.modifiers.contains(KeyModifiers::SHIFT));
                }
            } else if matches!(app.focus, Focus::Transcript) {
                app.scroll_offset = app.scroll_offset.saturating_sub(1);
            }
        }
        KeyCode::Left if matches!(app.focus, Focus::Input) => {
            app.composer.move_left(
                key.modifiers.contains(KeyModifiers::SHIFT),
                key.modifiers.contains(KeyModifiers::CONTROL),
            );
        }
        KeyCode::Right if matches!(app.focus, Focus::Input) => {
            app.composer.move_right(
                key.modifiers.contains(KeyModifiers::SHIFT),
                key.modifiers.contains(KeyModifiers::CONTROL),
            );
        }
        KeyCode::Home if matches!(app.focus, Focus::Input) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                app.composer.move_to_start(key.modifiers.contains(KeyModifiers::SHIFT));
            } else {
                app.composer.move_to_line_start(key.modifiers.contains(KeyModifiers::SHIFT));
            }
        }
        KeyCode::End if matches!(app.focus, Focus::Input) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                app.composer.move_to_end(key.modifiers.contains(KeyModifiers::SHIFT));
            } else {
                app.composer.move_to_line_end(key.modifiers.contains(KeyModifiers::SHIFT));
            }
        }
        KeyCode::PageUp => app.scroll_offset = app.scroll_offset.saturating_add(8),
        KeyCode::PageDown => app.scroll_offset = app.scroll_offset.saturating_sub(8),
        KeyCode::Char('q') if app.composer.is_empty() => app.status_line = "__exit__".to_owned(),
        _ => {}
    }
    Ok(())
}

async fn handle_approval_key(app: &mut App, key: KeyEvent) -> Result<()> {
    let Some(approval) = app.pending_approval.clone() else {
        app.mode = Mode::Chat;
        return Ok(());
    };
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => {
            if let Some(stream) = app.active_stream.as_ref() {
                stream.send_tool_approval_decision(
                    &approval,
                    true,
                    "approved_by_tui".to_owned(),
                    common_v1::ApprovalDecisionScope::Once as i32,
                    0,
                )?;
            }
            app.push_entry(
                EntryKind::Tool,
                "Approval response",
                format!("Approved {}", approval.tool_name),
            );
            app.pending_approval = None;
            app.mode = Mode::Chat;
            app.status_line = text::approval_granted_once(app.locale);
            app.emit_ux_event(
                "ux.approval.resolved",
                Some("TUI approved a pending action".to_owned()),
                serde_json::json!({
                    "section": "approvals",
                    "outcome": "approved",
                    "toolName": approval.tool_name.clone(),
                    "runId": app.last_run_id.clone(),
                }),
            )
            .await;
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            if let Some(stream) = app.active_stream.as_ref() {
                stream.send_tool_approval_decision(
                    &approval,
                    false,
                    "denied_by_tui".to_owned(),
                    common_v1::ApprovalDecisionScope::Once as i32,
                    0,
                )?;
            }
            app.push_entry(
                EntryKind::Tool,
                "Approval response",
                format!("Denied {}", approval.tool_name),
            );
            app.pending_approval = None;
            app.mode = Mode::Chat;
            app.status_line = text::approval_denied(app.locale);
            app.emit_ux_event(
                "ux.approval.resolved",
                Some("TUI denied a pending action".to_owned()),
                serde_json::json!({
                    "section": "approvals",
                    "outcome": "denied",
                    "toolName": approval.tool_name.clone(),
                    "runId": app.last_run_id.clone(),
                }),
            )
            .await;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_shell_confirm_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => {
            if strict_profile_blocks_local_shell() {
                app.pending_shell_command = None;
                app.mode = Mode::Chat;
                app.status_line = text::local_shell_blocked(app.locale);
                return Ok(());
            }
            app.local_shell_enabled = true;
            app.mode = Mode::Chat;
            app.status_line = text::local_shell_enabled_for_session(app.locale);
            if let Some(command) = app.pending_shell_command.take() {
                app.handle_shell_request(command).await?;
            }
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            app.pending_shell_command = None;
            app.mode = Mode::Chat;
            app.status_line = text::local_shell_remains_disabled(app.locale);
        }
        _ => {}
    }
    Ok(())
}

async fn handle_settings_key(app: &mut App, key: KeyEvent) -> Result<()> {
    let items = settings_items();
    match key.code {
        KeyCode::Esc => app.mode = Mode::Chat,
        KeyCode::Up => app.settings_selected = app.settings_selected.saturating_sub(1),
        KeyCode::Down => {
            app.settings_selected = (app.settings_selected + 1).min(items.len().saturating_sub(1));
        }
        KeyCode::Enter | KeyCode::Char(' ') => match items[app.settings_selected] {
            SettingsItem::ShowTools => app.set_trace_enabled(!app.show_tools).await?,
            SettingsItem::ShowThinking => app.set_thinking_enabled(!app.show_thinking).await?,
            SettingsItem::ShowVerbose => app.set_verbose_enabled(!app.show_verbose).await?,
            SettingsItem::LocalShell => {
                if strict_profile_blocks_local_shell() {
                    app.status_line = text::local_shell_blocked(app.locale);
                } else if app.local_shell_enabled {
                    app.local_shell_enabled = false;
                    app.status_line = text::local_shell_disabled(app.locale);
                } else {
                    app.mode = Mode::ShellConfirm;
                    app.pending_shell_command = None;
                    app.status_line = text::confirm_local_shell_opt_in(app.locale);
                }
            }
        },
        _ => {}
    }
    Ok(())
}

async fn handle_picker_key(app: &mut App, key: KeyEvent) -> Result<()> {
    let Some(picker) = app.pending_picker.as_mut() else {
        app.mode = Mode::Chat;
        return Ok(());
    };
    match key.code {
        KeyCode::Esc => {
            app.pending_picker = None;
            app.mode = Mode::Chat;
        }
        KeyCode::Up => picker.selected = picker.selected.saturating_sub(1),
        KeyCode::Down => {
            picker.selected = (picker.selected + 1).min(picker.items.len().saturating_sub(1));
        }
        KeyCode::Enter => app.apply_picker_selection().await?,
        _ => {}
    }
    Ok(())
}

fn render(frame: &mut Frame<'_>, app: &App) {
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

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
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

fn render_transcript(frame: &mut Frame<'_>, area: Rect, app: &App) {
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

fn render_input(frame: &mut Frame<'_>, area: Rect, app: &App, composer_view: &TuiComposerView) {
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

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
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

fn strict_profile_blocks_local_shell() -> bool {
    app::current_root_context()
        .map(|context| context.strict_profile_mode() && !context.allow_strict_profile_actions)
        .unwrap_or(false)
}

fn estimate_input_height(app: &App, composer_view: &TuiComposerView) -> u16 {
    let mut height = 2 + composer_view.lines.len() as u16 + 1;
    if !app.pending_attachments.is_empty() {
        height += 1;
    }
    if app.context_budget_summary().warning.is_some() {
        height += 1;
    }
    height.clamp(4, 10)
}

fn compact_status_bar_line(app: &App, width: usize) -> String {
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

fn compact_shortcut_line(app: &App, width: usize) -> String {
    let slash_hint =
        if app.pending_slash_palette.is_some() { "slash Up/Down + Tab" } else { "/ commands" };
    let line = format!(
        "Enter send · Alt+Enter/Ctrl+J newline · Ctrl+O attach · /status detail · /workspace · /rollback diff · F8 tips · {slash_hint}"
    );
    truncate_text(line, width.max(8))
}

fn discoverability_tip_line(app: &App, width: usize) -> String {
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

fn current_run_duration_ms(app: &App) -> Option<i64> {
    let started_at = app.session_runtime.latest_started_at_unix_ms?;
    let finished_at = if app.active_stream.is_some() {
        now_unix_ms_i64().ok()
    } else {
        app.session_runtime.latest_completed_at_unix_ms
    };
    Some(finished_at.unwrap_or(started_at) - started_at)
}

fn connection_posture_label(grpc_url: &str) -> &'static str {
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

fn fit_segments_to_width(width: usize, segments: Vec<String>) -> String {
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
struct TuiCompactionCommand {
    apply: bool,
    history: bool,
    accept_candidate_ids: Vec<String>,
    reject_candidate_ids: Vec<String>,
}

fn parse_tui_compaction_arguments(arguments: &[String]) -> Result<TuiCompactionCommand> {
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

fn parse_tui_json_string(value: &str) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(value).ok()
}

fn estimate_text_tokens(text: &str) -> u64 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    ((trimmed.len() as u64) + 3) / 4
}

fn format_approx_tokens(value: u64) -> String {
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

fn format_size_bytes(value: u64) -> String {
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

fn format_duration_ms(value: i64) -> String {
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

fn format_cost_usd(value: f64) -> String {
    if value >= 1.0 {
        format!("${value:.2}")
    } else {
        format!("${value:.4}")
    }
}

fn guess_content_type(path: &Path) -> &'static str {
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

fn attachment_kind_label(content_type: &str) -> &'static str {
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

fn attachment_kind_to_proto(kind: &str) -> common_v1::message_attachment::AttachmentKind {
    match kind.to_ascii_lowercase().as_str() {
        "image" => common_v1::message_attachment::AttachmentKind::Image,
        "audio" => common_v1::message_attachment::AttachmentKind::Audio,
        "video" => common_v1::message_attachment::AttachmentKind::Video,
        "file" => common_v1::message_attachment::AttachmentKind::File,
        _ => common_v1::message_attachment::AttachmentKind::Unspecified,
    }
}

fn read_json_optional_i64(value: &serde_json::Value, pointer: &str) -> Option<i64> {
    value.pointer(pointer).and_then(serde_json::Value::as_i64)
}

fn read_json_u64(value: &serde_json::Value, pointer: &str) -> u64 {
    value.pointer(pointer).and_then(serde_json::Value::as_u64).unwrap_or_default()
}

fn read_json_f64(value: &serde_json::Value, pointer: &str) -> Option<f64> {
    value.pointer(pointer).and_then(|entry| {
        entry
            .as_f64()
            .or_else(|| entry.as_i64().map(|number| number as f64))
            .or_else(|| entry.as_u64().map(|number| number as f64))
    })
}

fn background_task_is_active(state: &str) -> bool {
    !matches!(
        state.trim().to_ascii_lowercase().as_str(),
        "" | "completed" | "failed" | "cancelled" | "canceled" | "succeeded"
    )
}

fn map_session_catalog_record(session: SessionCatalogRecord) -> TuiSlashSessionRecord {
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

struct TuiObjectiveCreateSpec {
    kind: &'static str,
    name: String,
    prompt: String,
}

fn resolve_tui_objective_reference(
    explicit: Option<String>,
    selected: Option<&String>,
) -> Result<String> {
    explicit
        .or_else(|| selected.cloned())
        .context("Select an objective first or pass an explicit objective_id")
}

fn parse_tui_objective_create_spec(
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

fn parse_tui_objective_kind(value: &str) -> Result<&'static str> {
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

fn render_help_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
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

fn percent_encode_component(value: &str) -> String {
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

fn render_approval_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
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

fn approval_risk_level_to_text(raw: i32) -> &'static str {
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

fn render_picker_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
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

fn render_slash_palette_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
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

fn render_settings_popup(frame: &mut Frame<'_>, area: Rect, app: &App) {
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

fn render_shell_confirm_popup(frame: &mut Frame<'_>, area: Rect) {
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

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
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

fn settings_items() -> [SettingsItem; 4] {
    [
        SettingsItem::ShowTools,
        SettingsItem::ShowThinking,
        SettingsItem::ShowVerbose,
        SettingsItem::LocalShell,
    ]
}

fn entry_style(kind: &EntryKind) -> Style {
    match kind {
        EntryKind::User => Style::default().fg(Color::Cyan),
        EntryKind::Assistant => Style::default().fg(Color::White),
        EntryKind::Tool => Style::default().fg(Color::Yellow),
        EntryKind::Thinking => Style::default().fg(Color::DarkGray),
        EntryKind::System => Style::default().fg(Color::Green),
        EntryKind::Shell => Style::default().fg(Color::Magenta),
    }
}

fn display_session_identity(session: &gateway_v1::SessionSummary) -> String {
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

fn normalize_optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn shorten_id(value: &str) -> String {
    redacted_identifier_for_output(value)
}

fn parse_toggle(value: Option<&str>, current: bool) -> Result<bool> {
    match value.unwrap_or("toggle") {
        "toggle" => Ok(!current),
        "on" | "true" | "yes" => Ok(true),
        "off" | "false" | "no" => Ok(false),
        other => Err(anyhow!("unsupported toggle value: {other}")),
    }
}

fn quick_control_reset_requested(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "default" | "reset" | "inherit")
}

fn looks_like_canonical_ulid(value: &str) -> bool {
    value.len() == 26 && value.chars().all(|ch| ch.is_ascii_alphanumeric())
}

fn session_reference_equals(candidate: &str, reference: &str) -> bool {
    !candidate.trim().is_empty() && candidate.eq_ignore_ascii_case(reference)
}

fn session_reference_matches(session: &TuiSlashSessionRecord, reference: &str) -> bool {
    session_reference_equals(session.session_id.as_str(), reference)
        || session_reference_equals(session.session_key.as_str(), reference)
        || session_reference_equals(session.title.as_str(), reference)
        || session_reference_equals(session.root_title.as_str(), reference)
}

fn describe_branch_state_label(branch_state: &str) -> String {
    match branch_state {
        "root" => "root".to_owned(),
        "active_branch" | "branched" => "branch".to_owned(),
        "branch_source" => "branch source".to_owned(),
        "missing" => "no lineage".to_owned(),
        other => other.replace('_', " "),
    }
}

fn agent_resolution_source_label(raw: i32) -> &'static str {
    match gateway_v1::AgentResolutionSource::try_from(raw)
        .unwrap_or(gateway_v1::AgentResolutionSource::Unspecified)
    {
        gateway_v1::AgentResolutionSource::SessionBinding => "session_binding",
        gateway_v1::AgentResolutionSource::Default => "default",
        gateway_v1::AgentResolutionSource::Fallback => "fallback",
        gateway_v1::AgentResolutionSource::Unspecified => "unspecified",
    }
}

async fn run_local_shell(command: String) -> Result<ShellResult> {
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

fn truncate_text(mut value: String, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value;
    }
    value = value.chars().take(limit).collect::<String>();
    value.push_str("...");
    value
}

fn sanitize_terminal_text(value: &str) -> String {
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

fn format_shell_result(result: &ShellResult) -> String {
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

#[cfg(test)]
mod tests {
    use super::{
        display_session_identity, parse_toggle, parse_tui_objective_create_spec,
        parse_tui_objective_kind, quick_control_reset_requested, sanitize_terminal_text, App,
        Focus, Mode, SessionRuntimeSnapshot, TuiComposer, TuiLocale, TuiSlashEntityCatalog,
        TuiUxMetrics,
    };
    use crate::proto::palyra::{common::v1 as common_v1, gateway::v1 as gateway_v1};
    use std::collections::BTreeMap;

    fn test_app() -> App {
        App {
            runtime: crate::client::operator::OperatorRuntime::new(crate::AgentConnection {
                grpc_url: "http://127.0.0.1:7142".to_owned(),
                token: None,
                principal: "tester".to_owned(),
                device_id: "dev-1".to_owned(),
                channel: "cli".to_owned(),
                trace_id: "trace-1".to_owned(),
            }),
            session: gateway_v1::SessionSummary::default(),
            current_session_catalog: None,
            current_agent: None,
            current_agent_source: "test",
            models: None,
            composer: TuiComposer::default(),
            draft_cache: BTreeMap::new(),
            pending_attachments: Vec::new(),
            session_runtime: SessionRuntimeSnapshot::default(),
            transcript: Vec::new(),
            active_stream: None,
            pending_approval: None,
            pending_shell_command: None,
            pending_picker: None,
            pending_slash_palette: None,
            slash_palette_selected: 0,
            slash_palette_dismissed: false,
            slash_entity_catalog: TuiSlashEntityCatalog::default(),
            pending_redirect_prompt: None,
            focus: Focus::Input,
            mode: Mode::Chat,
            show_tools: true,
            show_thinking: true,
            show_verbose: true,
            local_shell_enabled: false,
            allow_sensitive_tools: false,
            include_archived_sessions: false,
            last_run_id: None,
            selected_objective_id: None,
            locale: TuiLocale::En,
            ux_metrics: TuiUxMetrics::default(),
            scroll_offset: 0,
            status_line: String::new(),
            settings_selected: 0,
            show_tips: true,
        }
    }

    #[test]
    fn sanitize_terminal_text_replaces_control_characters() {
        let sanitized = sanitize_terminal_text("ok\x1b[31mwarn\x07\r\nnext\tline");
        assert_eq!(sanitized, "ok<ESC>[31mwarn<U+0007>\nnext<U+0009>line");
    }

    #[test]
    fn parse_toggle_accepts_explicit_values() {
        assert!(parse_toggle(Some("on"), false).expect("on should parse"));
        assert!(!parse_toggle(Some("off"), true).expect("off should parse"));
    }

    #[test]
    fn quick_control_reset_requested_accepts_default_synonyms() {
        assert!(quick_control_reset_requested("default"));
        assert!(quick_control_reset_requested("reset"));
        assert!(quick_control_reset_requested("inherit"));
        assert!(!quick_control_reset_requested("on"));
    }

    #[test]
    fn display_session_identity_prefers_label() {
        let summary = gateway_v1::SessionSummary {
            session_id: Some(common_v1::CanonicalId {
                ulid: "01ARZ3NDEKTSV4RRFFQ69G5FAW".to_owned(),
            }),
            session_key: "ops:triage".to_owned(),
            session_label: "Ops Triage".to_owned(),
            title: "Auto title".to_owned(),
            created_at_unix_ms: 0,
            updated_at_unix_ms: 0,
            last_run_id: None,
            archived_at_unix_ms: 0,
            ..Default::default()
        };
        let display = display_session_identity(&summary);
        assert_eq!(display, "Auto title");
    }

    #[test]
    fn handle_stream_event_sanitizes_status_and_assistant_output() {
        let mut app = test_app();
        app.handle_stream_event(common_v1::RunStreamEvent {
            run_id: Some(common_v1::CanonicalId { ulid: "01ARZ3NDEKTSV4RRFFQ69G5FAW".to_owned() }),
            body: Some(common_v1::run_stream_event::Body::Status(common_v1::StreamStatus {
                kind: 0,
                message: "phase\x1b[2J\r\nnext".to_owned(),
            })),
            ..Default::default()
        })
        .expect("status event should be accepted");
        app.handle_stream_event(common_v1::RunStreamEvent {
            run_id: Some(common_v1::CanonicalId { ulid: "01ARZ3NDEKTSV4RRFFQ69G5FAW".to_owned() }),
            body: Some(common_v1::run_stream_event::Body::ModelToken(common_v1::ModelToken {
                token: "hello\x1b]52;c;ZXZpbA==\x07".to_owned(),
                is_final: false,
            })),
            ..Default::default()
        })
        .expect("token event should be accepted");

        assert_eq!(app.status_line, "phase<ESC>[2J\nnext");
        assert_eq!(app.transcript.len(), 2);
        assert_eq!(app.transcript[0].body, "phase<ESC>[2J\nnext");
        assert_eq!(app.transcript[1].body, "hello<ESC>]52;c;ZXZpbA==<U+0007>");
        assert!(app
            .transcript
            .iter()
            .flat_map(|entry| entry.title.chars().chain(entry.body.chars()))
            .all(|ch| !ch.is_control() || ch == '\n'));
    }

    #[test]
    fn handle_stream_event_sanitizes_approval_requests_before_storage() {
        let mut app = test_app();
        app.handle_stream_event(common_v1::RunStreamEvent {
            run_id: Some(common_v1::CanonicalId { ulid: "01ARZ3NDEKTSV4RRFFQ69G5FAW".to_owned() }),
            body: Some(common_v1::run_stream_event::Body::ToolApprovalRequest(
                common_v1::ToolApprovalRequest {
                    tool_name: "shell\x1b[31m".to_owned(),
                    request_summary: "run\x07 dangerous\rcommand".to_owned(),
                    ..Default::default()
                },
            )),
            ..Default::default()
        })
        .expect("approval event should be accepted");

        let approval = app.pending_approval.as_ref().expect("approval should be stored");
        assert_eq!(app.status_line, "Approval required for shell<ESC>[31m");
        assert_eq!(approval.tool_name, "shell<ESC>[31m");
        assert_eq!(approval.request_summary, "run<U+0007> dangerous\ncommand");
        assert_eq!(app.transcript.len(), 1);
        assert_eq!(app.transcript[0].title, "Approval requested: shell<ESC>[31m");
        assert_eq!(app.transcript[0].body, "run<U+0007> dangerous\ncommand");
    }

    #[test]
    fn parse_tui_objective_kind_accepts_product_aliases() {
        assert_eq!(parse_tui_objective_kind("objective").unwrap(), "objective");
        assert_eq!(parse_tui_objective_kind("heartbeat").unwrap(), "heartbeat");
        assert_eq!(parse_tui_objective_kind("standing-order").unwrap(), "standing_order");
        assert_eq!(parse_tui_objective_kind("program").unwrap(), "program");
    }

    #[test]
    fn parse_tui_objective_create_spec_supports_inline_prompt_separator() {
        let spec = parse_tui_objective_create_spec(
            None,
            &[
                "create".to_owned(),
                "heartbeat".to_owned(),
                "Ops".to_owned(),
                "status".to_owned(),
                "::".to_owned(),
                "Summarize".to_owned(),
                "current".to_owned(),
                "state".to_owned(),
            ],
        )
        .unwrap();
        assert_eq!(spec.kind, "heartbeat");
        assert_eq!(spec.name, "Ops status");
        assert_eq!(spec.prompt, "Summarize current state");
    }
}
