use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use axum::{extract::State, response::IntoResponse, routing::get, Json, Router};
use clap::Parser;
use palyra_common::{
    build_metadata, health_response, parse_daemon_bind_socket, validate_canonical_id,
    HealthResponse, CANONICAL_PROTOCOL_MAJOR,
};
use reqwest::{redirect::Policy, Url};
use tokio::sync::Mutex;
use tokio::time::{interval, MissedTickBehavior};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{transport::Server, Request, Response, Status};
use tracing::info;
use tracing_subscriber::EnvFilter;
use ulid::Ulid;

pub mod proto {
    pub mod palyra {
        pub mod common {
            pub mod v1 {
                tonic::include_proto!("palyra.common.v1");
            }
        }

        pub mod browser {
            pub mod v1 {
                tonic::include_proto!("palyra.browser.v1");
            }
        }
    }
}

use proto::palyra::browser::v1 as browser_v1;

const DEFAULT_GRPC_PORT: u16 = 7543;
const DEFAULT_SESSION_IDLE_TTL_MS: u64 = 10 * 60 * 1_000;
const DEFAULT_MAX_SESSIONS: usize = 128;
const DEFAULT_MAX_NAVIGATION_TIMEOUT_MS: u64 = 15_000;
const DEFAULT_MAX_SESSION_LIFETIME_MS: u64 = 30 * 60 * 1_000;
const DEFAULT_MAX_SCREENSHOT_BYTES: u64 = 256 * 1024;
const DEFAULT_MAX_RESPONSE_BYTES: u64 = 512 * 1024;
const DEFAULT_MAX_TITLE_BYTES: u64 = 4 * 1024;
const DEFAULT_MAX_ACTION_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_MAX_TYPE_INPUT_BYTES: u64 = 4 * 1024;
const DEFAULT_MAX_ACTIONS_PER_SESSION: u64 = 256;
const DEFAULT_MAX_ACTIONS_PER_WINDOW: u64 = 20;
const DEFAULT_ACTION_RATE_WINDOW_MS: u64 = 1_000;
const DEFAULT_MAX_ACTION_LOG_ENTRIES: usize = 256;
const DEFAULT_MAX_OBSERVE_SNAPSHOT_BYTES: u64 = 64 * 1024;
const DEFAULT_MAX_VISIBLE_TEXT_BYTES: u64 = 16 * 1024;
const DEFAULT_MAX_NETWORK_LOG_ENTRIES: usize = 256;
const DEFAULT_MAX_NETWORK_LOG_BYTES: u64 = 64 * 1024;
const MAX_NETWORK_LOG_HEADER_COUNT: usize = 24;
const MAX_NETWORK_LOG_HEADER_VALUE_BYTES: usize = 256;
const MAX_NETWORK_LOG_URL_BYTES: usize = 2 * 1024;
const DEFAULT_ACTION_RETRY_INTERVAL_MS: u64 = 100;
const CLEANUP_INTERVAL_MS: u64 = 15_000;
const AUTHORIZATION_HEADER: &str = "authorization";
const ONE_BY_ONE_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 10, 73, 68, 65, 84, 120, 156, 99, 96, 0, 0, 0, 2, 0, 1, 229,
    39, 212, 138, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[derive(Debug, Clone, Parser)]
#[command(name = "palyra-browserd", about = "Palyra browser service v1")]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    bind: String,
    #[arg(long, default_value_t = 7143)]
    port: u16,
    #[arg(long, default_value = "127.0.0.1")]
    grpc_bind: String,
    #[arg(long, default_value_t = DEFAULT_GRPC_PORT)]
    grpc_port: u16,
    #[arg(long)]
    auth_token: Option<String>,
    #[arg(long, default_value_t = DEFAULT_SESSION_IDLE_TTL_MS)]
    session_idle_ttl_ms: u64,
    #[arg(long, default_value_t = DEFAULT_MAX_SESSIONS)]
    max_sessions: usize,
    #[arg(long, default_value_t = DEFAULT_MAX_NAVIGATION_TIMEOUT_MS)]
    max_navigation_timeout_ms: u64,
    #[arg(long, default_value_t = DEFAULT_MAX_SESSION_LIFETIME_MS)]
    max_session_lifetime_ms: u64,
    #[arg(long, default_value_t = DEFAULT_MAX_SCREENSHOT_BYTES)]
    max_screenshot_bytes: u64,
    #[arg(long, default_value_t = DEFAULT_MAX_RESPONSE_BYTES)]
    max_response_bytes: u64,
    #[arg(long, default_value_t = DEFAULT_MAX_TITLE_BYTES)]
    max_title_bytes: u64,
}

#[derive(Debug, Clone)]
struct SessionBudget {
    max_navigation_timeout_ms: u64,
    max_session_lifetime_ms: u64,
    max_screenshot_bytes: u64,
    max_response_bytes: u64,
    max_title_bytes: u64,
    max_action_timeout_ms: u64,
    max_type_input_bytes: u64,
    max_actions_per_session: u64,
    max_actions_per_window: u64,
    action_rate_window_ms: u64,
    max_action_log_entries: usize,
    max_observe_snapshot_bytes: u64,
    max_visible_text_bytes: u64,
    max_network_log_entries: usize,
    max_network_log_bytes: u64,
}

#[derive(Debug, Clone)]
struct BrowserSessionRecord {
    last_active: Instant,
    created_at: Instant,
    idle_ttl: Duration,
    budget: SessionBudget,
    allow_private_targets: bool,
    allow_downloads: bool,
    action_allowed_domains: Vec<String>,
    last_title: String,
    last_url: Option<String>,
    last_page_body: String,
    scroll_x: i64,
    scroll_y: i64,
    typed_inputs: HashMap<String, String>,
    action_count: u64,
    action_window: VecDeque<Instant>,
    action_log: VecDeque<BrowserActionLogEntryInternal>,
    network_log: VecDeque<NetworkLogEntryInternal>,
}

#[derive(Debug, Clone)]
struct BrowserActionLogEntryInternal {
    action_id: String,
    action_name: String,
    selector: String,
    success: bool,
    outcome: String,
    error: String,
    started_at_unix_ms: u64,
    completed_at_unix_ms: u64,
    attempts: u32,
    page_url: String,
}

#[derive(Debug, Clone)]
struct NetworkLogHeaderInternal {
    name: String,
    value: String,
}

#[derive(Debug, Clone)]
struct NetworkLogEntryInternal {
    request_url: String,
    status_code: u16,
    timing_bucket: String,
    latency_ms: u64,
    captured_at_unix_ms: u64,
    headers: Vec<NetworkLogHeaderInternal>,
}

#[derive(Debug)]
struct BrowserRuntimeState {
    started_at: Instant,
    auth_token: Option<String>,
    default_idle_ttl: Duration,
    default_budget: SessionBudget,
    max_sessions: usize,
    sessions: Mutex<HashMap<String, BrowserSessionRecord>>,
}

impl BrowserRuntimeState {
    fn new(args: &Args) -> Result<Self> {
        if args.session_idle_ttl_ms == 0 || args.max_sessions == 0 {
            anyhow::bail!("session_idle_ttl_ms and max_sessions must be greater than zero");
        }
        Ok(Self {
            started_at: Instant::now(),
            auth_token: args
                .auth_token
                .clone()
                .or_else(|| std::env::var("PALYRA_BROWSERD_AUTH_TOKEN").ok())
                .and_then(|value| {
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_owned())
                    }
                }),
            default_idle_ttl: Duration::from_millis(args.session_idle_ttl_ms),
            default_budget: SessionBudget {
                max_navigation_timeout_ms: args.max_navigation_timeout_ms.max(1),
                max_session_lifetime_ms: args.max_session_lifetime_ms.max(1),
                max_screenshot_bytes: args.max_screenshot_bytes.max(1),
                max_response_bytes: args.max_response_bytes.max(1),
                max_title_bytes: args.max_title_bytes.max(1),
                max_action_timeout_ms: DEFAULT_MAX_ACTION_TIMEOUT_MS,
                max_type_input_bytes: DEFAULT_MAX_TYPE_INPUT_BYTES,
                max_actions_per_session: DEFAULT_MAX_ACTIONS_PER_SESSION,
                max_actions_per_window: DEFAULT_MAX_ACTIONS_PER_WINDOW,
                action_rate_window_ms: DEFAULT_ACTION_RATE_WINDOW_MS,
                max_action_log_entries: DEFAULT_MAX_ACTION_LOG_ENTRIES,
                max_observe_snapshot_bytes: DEFAULT_MAX_OBSERVE_SNAPSHOT_BYTES,
                max_visible_text_bytes: DEFAULT_MAX_VISIBLE_TEXT_BYTES,
                max_network_log_entries: DEFAULT_MAX_NETWORK_LOG_ENTRIES,
                max_network_log_bytes: DEFAULT_MAX_NETWORK_LOG_BYTES,
            },
            max_sessions: args.max_sessions,
            sessions: Mutex::new(HashMap::new()),
        })
    }

    async fn authorize(&self, metadata: &tonic::metadata::MetadataMap) -> Result<(), Status> {
        let Some(expected_token) = self.auth_token.as_ref() else {
            return Ok(());
        };
        let supplied = metadata
            .get(AUTHORIZATION_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        let expected = format!("Bearer {expected_token}");
        if supplied.trim() != expected {
            return Err(Status::unauthenticated("missing or invalid browser service token"));
        }
        Ok(())
    }
}

#[derive(Clone)]
struct AppState {
    runtime: Arc<BrowserRuntimeState>,
}

#[derive(Clone)]
struct BrowserServiceImpl {
    runtime: Arc<BrowserRuntimeState>,
}

#[tonic::async_trait]
impl browser_v1::browser_service_server::BrowserService for BrowserServiceImpl {
    async fn health(
        &self,
        request: Request<browser_v1::BrowserHealthRequest>,
    ) -> Result<Response<browser_v1::BrowserHealthResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let active_sessions = self.runtime.sessions.lock().await.len();
        Ok(Response::new(browser_v1::BrowserHealthResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            status: "ok".to_owned(),
            uptime_seconds: self.runtime.started_at.elapsed().as_secs(),
            active_sessions: u32::try_from(active_sessions).unwrap_or(u32::MAX),
        }))
    }

    async fn create_session(
        &self,
        request: Request<browser_v1::CreateSessionRequest>,
    ) -> Result<Response<browser_v1::CreateSessionResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let payload = request.into_inner();
        if payload.principal.trim().is_empty() {
            return Err(Status::invalid_argument("principal is required"));
        }
        let mut sessions = self.runtime.sessions.lock().await;
        if sessions.len() >= self.runtime.max_sessions {
            return Err(Status::resource_exhausted("browser session capacity reached"));
        }
        let session_id = Ulid::new().to_string();
        let now = Instant::now();
        let idle_ttl = if payload.idle_ttl_ms == 0 {
            self.runtime.default_idle_ttl
        } else {
            Duration::from_millis(payload.idle_ttl_ms)
        };
        let budget = SessionBudget {
            max_navigation_timeout_ms: payload
                .budget
                .as_ref()
                .map(|value| value.max_navigation_timeout_ms)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_navigation_timeout_ms),
            max_session_lifetime_ms: payload
                .budget
                .as_ref()
                .map(|value| value.max_session_lifetime_ms)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_session_lifetime_ms),
            max_screenshot_bytes: payload
                .budget
                .as_ref()
                .map(|value| value.max_screenshot_bytes)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_screenshot_bytes),
            max_response_bytes: payload
                .budget
                .as_ref()
                .map(|value| value.max_response_bytes)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_response_bytes),
            max_action_timeout_ms: payload
                .budget
                .as_ref()
                .map(|value| value.max_action_timeout_ms)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_action_timeout_ms),
            max_type_input_bytes: payload
                .budget
                .as_ref()
                .map(|value| value.max_type_input_bytes)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_type_input_bytes),
            max_actions_per_session: payload
                .budget
                .as_ref()
                .map(|value| value.max_actions_per_session)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_actions_per_session),
            max_actions_per_window: payload
                .budget
                .as_ref()
                .map(|value| value.max_actions_per_window)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_actions_per_window),
            action_rate_window_ms: payload
                .budget
                .as_ref()
                .map(|value| value.action_rate_window_ms)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.action_rate_window_ms),
            max_action_log_entries: payload
                .budget
                .as_ref()
                .map(|value| value.max_action_log_entries)
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_action_log_entries),
            max_observe_snapshot_bytes: payload
                .budget
                .as_ref()
                .map(|value| value.max_observe_snapshot_bytes)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_observe_snapshot_bytes),
            max_visible_text_bytes: payload
                .budget
                .as_ref()
                .map(|value| value.max_visible_text_bytes)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_visible_text_bytes),
            max_network_log_entries: payload
                .budget
                .as_ref()
                .map(|value| value.max_network_log_entries)
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_network_log_entries),
            max_network_log_bytes: payload
                .budget
                .as_ref()
                .map(|value| value.max_network_log_bytes)
                .filter(|value| *value > 0)
                .unwrap_or(self.runtime.default_budget.max_network_log_bytes),
            max_title_bytes: self.runtime.default_budget.max_title_bytes,
        };
        let action_allowed_domains =
            normalize_action_allowed_domains(payload.action_allowed_domains.as_slice());
        sessions.insert(
            session_id.clone(),
            BrowserSessionRecord {
                last_active: now,
                created_at: now,
                idle_ttl,
                budget: budget.clone(),
                allow_private_targets: payload.allow_private_targets,
                allow_downloads: payload.allow_downloads,
                action_allowed_domains: action_allowed_domains.clone(),
                last_title: String::new(),
                last_url: None,
                last_page_body: String::new(),
                scroll_x: 0,
                scroll_y: 0,
                typed_inputs: HashMap::new(),
                action_count: 0,
                action_window: VecDeque::new(),
                action_log: VecDeque::new(),
                network_log: VecDeque::new(),
            },
        );

        Ok(Response::new(browser_v1::CreateSessionResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            session_id: Some(proto::palyra::common::v1::CanonicalId { ulid: session_id }),
            created_at_unix_ms: current_unix_ms(),
            effective_budget: Some(browser_v1::SessionBudget {
                max_navigation_timeout_ms: budget.max_navigation_timeout_ms,
                max_session_lifetime_ms: budget.max_session_lifetime_ms,
                max_screenshot_bytes: budget.max_screenshot_bytes,
                max_response_bytes: budget.max_response_bytes,
                max_action_timeout_ms: budget.max_action_timeout_ms,
                max_type_input_bytes: budget.max_type_input_bytes,
                max_actions_per_session: budget.max_actions_per_session,
                max_actions_per_window: budget.max_actions_per_window,
                action_rate_window_ms: budget.action_rate_window_ms,
                max_action_log_entries: budget.max_action_log_entries as u64,
                max_observe_snapshot_bytes: budget.max_observe_snapshot_bytes,
                max_visible_text_bytes: budget.max_visible_text_bytes,
                max_network_log_entries: budget.max_network_log_entries as u64,
                max_network_log_bytes: budget.max_network_log_bytes,
            }),
            downloads_enabled: payload.allow_downloads,
            action_allowed_domains,
        }))
    }

    async fn close_session(
        &self,
        request: Request<browser_v1::CloseSessionRequest>,
    ) -> Result<Response<browser_v1::CloseSessionResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let session_id = parse_session_id_from_proto(request.into_inner().session_id)
            .map_err(Status::invalid_argument)?;
        let removed = self.runtime.sessions.lock().await.remove(session_id.as_str());
        Ok(Response::new(browser_v1::CloseSessionResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            closed: removed.is_some(),
            reason: if removed.is_some() {
                "closed".to_owned()
            } else {
                "session_not_found".to_owned()
            },
        }))
    }

    async fn navigate(
        &self,
        request: Request<browser_v1::NavigateRequest>,
    ) -> Result<Response<browser_v1::NavigateResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let mut payload = request.into_inner();
        let session_id = parse_session_id_from_proto(payload.session_id.take())
            .map_err(Status::invalid_argument)?;
        let url = payload.url.trim().to_owned();
        if url.is_empty() {
            return Err(Status::invalid_argument("navigate requires non-empty url"));
        }
        let (timeout_ms, max_response_bytes, allow_private_targets) = {
            let mut sessions = self.runtime.sessions.lock().await;
            let Some(session) = sessions.get_mut(session_id.as_str()) else {
                return Err(Status::not_found("browser session not found"));
            };
            session.last_active = Instant::now();
            let timeout_ms =
                payload.timeout_ms.max(1).min(session.budget.max_navigation_timeout_ms);
            (
                timeout_ms,
                session.budget.max_response_bytes,
                payload.allow_private_targets || session.allow_private_targets,
            )
        };

        let outcome = navigate_with_guards(
            url.as_str(),
            timeout_ms,
            payload.allow_redirects,
            if payload.max_redirects == 0 { 3 } else { payload.max_redirects },
            allow_private_targets,
            max_response_bytes,
        )
        .await;
        let network_log_entries = outcome.network_log.clone();

        let mut sessions = self.runtime.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id.as_str()) {
            if outcome.success {
                session.last_title = outcome.title.clone();
                session.last_url = Some(outcome.final_url.clone());
                session.last_page_body = outcome.page_body.clone();
                session.scroll_x = 0;
                session.scroll_y = 0;
                session.typed_inputs.clear();
            }
            append_network_log_entries(session, network_log_entries.as_slice());
            session.last_active = Instant::now();
        }

        Ok(Response::new(browser_v1::NavigateResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            success: outcome.success,
            final_url: outcome.final_url,
            status_code: u32::from(outcome.status_code),
            title: truncate_utf8_bytes(
                outcome.title.as_str(),
                self.runtime.default_budget.max_title_bytes as usize,
            ),
            body_bytes: outcome.body_bytes,
            latency_ms: outcome.latency_ms,
            error: outcome.error,
        }))
    }

    async fn click(
        &self,
        request: Request<browser_v1::ClickRequest>,
    ) -> Result<Response<browser_v1::ClickResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let mut payload = request.into_inner();
        let session_id = parse_session_id_from_proto(payload.session_id.take())
            .map_err(Status::invalid_argument)?;
        let selector = payload.selector.trim();
        if selector.is_empty() {
            return Err(Status::invalid_argument("click requires non-empty selector"));
        }

        let context = match consume_action_budget_and_snapshot(
            self.runtime.as_ref(),
            session_id.as_str(),
            true,
        )
        .await
        {
            Ok(value) => value,
            Err(error) => {
                return Ok(Response::new(browser_v1::ClickResponse {
                    v: CANONICAL_PROTOCOL_MAJOR,
                    success: false,
                    error,
                    action_log: None,
                    failure_screenshot_bytes: Vec::new(),
                    failure_screenshot_mime_type: String::new(),
                }));
            }
        };

        let timeout_ms = payload.timeout_ms.max(1).min(context.budget.max_action_timeout_ms);
        let max_attempts = payload.max_retries.clamp(0, 16).saturating_add(1);
        let started_at = Instant::now();
        let started_at_unix_ms = current_unix_ms();
        let mut attempts = 0_u32;
        let mut success = false;
        let mut outcome = "selector_not_found".to_owned();
        let mut error = format!("selector '{}' was not found", selector);
        loop {
            attempts = attempts.saturating_add(1);
            if let Some(tag) = find_matching_html_tag(selector, context.page_body.as_str()) {
                if is_download_like_tag(tag.as_str()) && !context.allow_downloads {
                    outcome = "download_blocked".to_owned();
                    error =
                        "download-like click is blocked by session policy (allow_downloads=false)"
                            .to_owned();
                    break;
                }
                success = true;
                outcome = if is_download_like_tag(tag.as_str()) {
                    "download_allowed".to_owned()
                } else {
                    "clicked".to_owned()
                };
                error.clear();
                break;
            }
            if attempts >= max_attempts || started_at.elapsed() >= Duration::from_millis(timeout_ms)
            {
                break;
            }
            let remaining_ms = timeout_ms.saturating_sub(started_at.elapsed().as_millis() as u64);
            let sleep_ms = DEFAULT_ACTION_RETRY_INTERVAL_MS.min(remaining_ms.max(1));
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }

        let (action_log, failure_screenshot_bytes, failure_screenshot_mime_type) =
            finalize_session_action(
                self.runtime.as_ref(),
                session_id.as_str(),
                FinalizeActionRequest {
                    action_name: "click",
                    selector,
                    success,
                    outcome: outcome.as_str(),
                    error: error.as_str(),
                    started_at_unix_ms,
                    attempts,
                    capture_failure_screenshot: payload.capture_failure_screenshot,
                    max_failure_screenshot_bytes: payload.max_failure_screenshot_bytes,
                },
            )
            .await;

        Ok(Response::new(browser_v1::ClickResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            success,
            error,
            action_log,
            failure_screenshot_bytes,
            failure_screenshot_mime_type,
        }))
    }

    async fn r#type(
        &self,
        request: Request<browser_v1::TypeRequest>,
    ) -> Result<Response<browser_v1::TypeResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let mut payload = request.into_inner();
        let session_id = parse_session_id_from_proto(payload.session_id.take())
            .map_err(Status::invalid_argument)?;
        let selector = payload.selector.trim();
        if selector.is_empty() {
            return Err(Status::invalid_argument("type requires non-empty selector"));
        }

        let context = match consume_action_budget_and_snapshot(
            self.runtime.as_ref(),
            session_id.as_str(),
            true,
        )
        .await
        {
            Ok(value) => value,
            Err(error) => {
                return Ok(Response::new(browser_v1::TypeResponse {
                    v: CANONICAL_PROTOCOL_MAJOR,
                    success: false,
                    typed_bytes: 0,
                    error,
                    action_log: None,
                    failure_screenshot_bytes: Vec::new(),
                    failure_screenshot_mime_type: String::new(),
                }));
            }
        };

        let text = payload.text;
        if (text.len() as u64) > context.budget.max_type_input_bytes {
            let error = format!(
                "type input exceeds max_type_input_bytes ({} > {})",
                text.len(),
                context.budget.max_type_input_bytes
            );
            let (action_log, failure_screenshot_bytes, failure_screenshot_mime_type) =
                finalize_session_action(
                    self.runtime.as_ref(),
                    session_id.as_str(),
                    FinalizeActionRequest {
                        action_name: "type",
                        selector,
                        success: false,
                        outcome: "input_too_large",
                        error: error.as_str(),
                        started_at_unix_ms: current_unix_ms(),
                        attempts: 1,
                        capture_failure_screenshot: payload.capture_failure_screenshot,
                        max_failure_screenshot_bytes: payload.max_failure_screenshot_bytes,
                    },
                )
                .await;
            return Ok(Response::new(browser_v1::TypeResponse {
                v: CANONICAL_PROTOCOL_MAJOR,
                success: false,
                typed_bytes: 0,
                error,
                action_log,
                failure_screenshot_bytes,
                failure_screenshot_mime_type,
            }));
        }

        let timeout_ms = payload.timeout_ms.max(1).min(context.budget.max_action_timeout_ms);
        let started_at = Instant::now();
        let started_at_unix_ms = current_unix_ms();
        let mut attempts = 0_u32;
        let mut success = false;
        let mut outcome = "selector_not_found".to_owned();
        let mut error = format!("selector '{}' was not found", selector);
        loop {
            attempts = attempts.saturating_add(1);
            if let Some(tag) = find_matching_html_tag(selector, context.page_body.as_str()) {
                if !is_typable_tag(tag.as_str()) {
                    outcome = "selector_not_typable".to_owned();
                    error =
                        format!("selector '{}' does not target an input-like element", selector);
                    break;
                }
                success = true;
                outcome = "typed".to_owned();
                error.clear();
                break;
            }
            if started_at.elapsed() >= Duration::from_millis(timeout_ms) {
                break;
            }
            let remaining_ms = timeout_ms.saturating_sub(started_at.elapsed().as_millis() as u64);
            let sleep_ms = DEFAULT_ACTION_RETRY_INTERVAL_MS.min(remaining_ms.max(1));
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }

        if success {
            let mut sessions = self.runtime.sessions.lock().await;
            if let Some(session) = sessions.get_mut(session_id.as_str()) {
                let field = session.typed_inputs.entry(selector.to_owned()).or_default();
                if payload.clear_existing {
                    *field = text.clone();
                } else {
                    field.push_str(text.as_str());
                }
            }
        }

        let (action_log, failure_screenshot_bytes, failure_screenshot_mime_type) =
            finalize_session_action(
                self.runtime.as_ref(),
                session_id.as_str(),
                FinalizeActionRequest {
                    action_name: "type",
                    selector,
                    success,
                    outcome: outcome.as_str(),
                    error: error.as_str(),
                    started_at_unix_ms,
                    attempts,
                    capture_failure_screenshot: payload.capture_failure_screenshot,
                    max_failure_screenshot_bytes: payload.max_failure_screenshot_bytes,
                },
            )
            .await;

        Ok(Response::new(browser_v1::TypeResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            success,
            typed_bytes: if success { text.len() as u64 } else { 0 },
            error,
            action_log,
            failure_screenshot_bytes,
            failure_screenshot_mime_type,
        }))
    }

    async fn scroll(
        &self,
        request: Request<browser_v1::ScrollRequest>,
    ) -> Result<Response<browser_v1::ScrollResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let mut payload = request.into_inner();
        let session_id = parse_session_id_from_proto(payload.session_id.take())
            .map_err(Status::invalid_argument)?;

        let _context = match consume_action_budget_and_snapshot(
            self.runtime.as_ref(),
            session_id.as_str(),
            false,
        )
        .await
        {
            Ok(value) => value,
            Err(error) => {
                return Ok(Response::new(browser_v1::ScrollResponse {
                    v: CANONICAL_PROTOCOL_MAJOR,
                    success: false,
                    scroll_x: 0,
                    scroll_y: 0,
                    error,
                    action_log: None,
                    failure_screenshot_bytes: Vec::new(),
                    failure_screenshot_mime_type: String::new(),
                }));
            }
        };

        let mut scroll_x = 0_i64;
        let mut scroll_y = 0_i64;
        {
            let mut sessions = self.runtime.sessions.lock().await;
            if let Some(session) = sessions.get_mut(session_id.as_str()) {
                session.scroll_x = session.scroll_x.saturating_add(payload.delta_x);
                session.scroll_y = session.scroll_y.saturating_add(payload.delta_y);
                scroll_x = session.scroll_x;
                scroll_y = session.scroll_y;
            }
        }

        let (action_log, failure_screenshot_bytes, failure_screenshot_mime_type) =
            finalize_session_action(
                self.runtime.as_ref(),
                session_id.as_str(),
                FinalizeActionRequest {
                    action_name: "scroll",
                    selector: "",
                    success: true,
                    outcome: "scrolled",
                    error: "",
                    started_at_unix_ms: current_unix_ms(),
                    attempts: 1,
                    capture_failure_screenshot: payload.capture_failure_screenshot,
                    max_failure_screenshot_bytes: payload.max_failure_screenshot_bytes,
                },
            )
            .await;

        Ok(Response::new(browser_v1::ScrollResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            success: true,
            scroll_x,
            scroll_y,
            error: String::new(),
            action_log,
            failure_screenshot_bytes,
            failure_screenshot_mime_type,
        }))
    }

    async fn wait_for(
        &self,
        request: Request<browser_v1::WaitForRequest>,
    ) -> Result<Response<browser_v1::WaitForResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let mut payload = request.into_inner();
        let session_id = parse_session_id_from_proto(payload.session_id.take())
            .map_err(Status::invalid_argument)?;
        let selector = payload.selector.trim().to_owned();
        let text = payload.text;
        if selector.is_empty() && text.trim().is_empty() {
            return Err(Status::invalid_argument(
                "wait_for requires non-empty selector or non-empty text",
            ));
        }
        let context = match consume_action_budget_and_snapshot(
            self.runtime.as_ref(),
            session_id.as_str(),
            true,
        )
        .await
        {
            Ok(value) => value,
            Err(error) => {
                return Ok(Response::new(browser_v1::WaitForResponse {
                    v: CANONICAL_PROTOCOL_MAJOR,
                    success: false,
                    waited_ms: 0,
                    error,
                    action_log: None,
                    failure_screenshot_bytes: Vec::new(),
                    failure_screenshot_mime_type: String::new(),
                    matched_selector: String::new(),
                    matched_text: String::new(),
                }));
            }
        };

        let timeout_ms = payload.timeout_ms.max(1).min(context.budget.max_action_timeout_ms);
        let poll_interval_ms = payload.poll_interval_ms.clamp(25, 1_000);
        let started = Instant::now();
        let started_at_unix_ms = current_unix_ms();
        let mut attempts = 0_u32;
        let mut matched_selector = String::new();
        let mut matched_text = String::new();
        let mut success = false;
        loop {
            attempts = attempts.saturating_add(1);
            if !selector.is_empty()
                && find_matching_html_tag(selector.as_str(), context.page_body.as_str()).is_some()
            {
                matched_selector = selector.clone();
                success = true;
                break;
            }
            if !text.trim().is_empty() && context.page_body.contains(text.as_str()) {
                matched_text = text.clone();
                success = true;
                break;
            }
            if started.elapsed() >= Duration::from_millis(timeout_ms) {
                break;
            }
            let remaining_ms = timeout_ms.saturating_sub(started.elapsed().as_millis() as u64);
            let sleep_ms = poll_interval_ms.min(remaining_ms.max(1));
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }
        let waited_ms = started.elapsed().as_millis() as u64;
        let error = if success {
            String::new()
        } else {
            "wait_for condition was not satisfied before timeout".to_owned()
        };

        let (action_log, failure_screenshot_bytes, failure_screenshot_mime_type) =
            finalize_session_action(
                self.runtime.as_ref(),
                session_id.as_str(),
                FinalizeActionRequest {
                    action_name: "wait_for",
                    selector: selector.as_str(),
                    success,
                    outcome: if success { "condition_matched" } else { "condition_timeout" },
                    error: error.as_str(),
                    started_at_unix_ms,
                    attempts,
                    capture_failure_screenshot: payload.capture_failure_screenshot,
                    max_failure_screenshot_bytes: payload.max_failure_screenshot_bytes,
                },
            )
            .await;

        Ok(Response::new(browser_v1::WaitForResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            success,
            waited_ms,
            error,
            action_log,
            failure_screenshot_bytes,
            failure_screenshot_mime_type,
            matched_selector,
            matched_text,
        }))
    }

    async fn get_title(
        &self,
        request: Request<browser_v1::GetTitleRequest>,
    ) -> Result<Response<browser_v1::GetTitleResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let mut payload = request.into_inner();
        let session_id = parse_session_id_from_proto(payload.session_id.take())
            .map_err(Status::invalid_argument)?;
        let max_title_bytes = usize::try_from(payload.max_title_bytes)
            .ok()
            .filter(|value| *value > 0)
            .unwrap_or(self.runtime.default_budget.max_title_bytes as usize);
        let mut sessions = self.runtime.sessions.lock().await;
        let Some(session) = sessions.get_mut(session_id.as_str()) else {
            return Ok(Response::new(browser_v1::GetTitleResponse {
                v: CANONICAL_PROTOCOL_MAJOR,
                success: false,
                title: String::new(),
                error: "session_not_found".to_owned(),
            }));
        };
        session.last_active = Instant::now();
        Ok(Response::new(browser_v1::GetTitleResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            success: true,
            title: truncate_utf8_bytes(session.last_title.as_str(), max_title_bytes),
            error: String::new(),
        }))
    }

    async fn screenshot(
        &self,
        request: Request<browser_v1::ScreenshotRequest>,
    ) -> Result<Response<browser_v1::ScreenshotResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let mut payload = request.into_inner();
        let session_id = parse_session_id_from_proto(payload.session_id.take())
            .map_err(Status::invalid_argument)?;
        if !payload.format.trim().is_empty() && !payload.format.trim().eq_ignore_ascii_case("png") {
            return Err(Status::invalid_argument("screenshot format must be empty or 'png'"));
        }
        let mut sessions = self.runtime.sessions.lock().await;
        let Some(session) = sessions.get_mut(session_id.as_str()) else {
            return Ok(Response::new(browser_v1::ScreenshotResponse {
                v: CANONICAL_PROTOCOL_MAJOR,
                success: false,
                image_bytes: Vec::new(),
                mime_type: "image/png".to_owned(),
                error: "session_not_found".to_owned(),
            }));
        };
        session.last_active = Instant::now();
        let max_bytes = payload.max_bytes.max(1).min(session.budget.max_screenshot_bytes);
        if (ONE_BY_ONE_PNG.len() as u64) > max_bytes {
            return Ok(Response::new(browser_v1::ScreenshotResponse {
                v: CANONICAL_PROTOCOL_MAJOR,
                success: false,
                image_bytes: Vec::new(),
                mime_type: "image/png".to_owned(),
                error: format!(
                    "screenshot output exceeds max_bytes ({} > {max_bytes})",
                    ONE_BY_ONE_PNG.len()
                ),
            }));
        }
        Ok(Response::new(browser_v1::ScreenshotResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            success: true,
            image_bytes: ONE_BY_ONE_PNG.to_vec(),
            mime_type: "image/png".to_owned(),
            error: String::new(),
        }))
    }

    async fn observe(
        &self,
        request: Request<browser_v1::ObserveRequest>,
    ) -> Result<Response<browser_v1::ObserveResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let mut payload = request.into_inner();
        let session_id = parse_session_id_from_proto(payload.session_id.take())
            .map_err(Status::invalid_argument)?;
        let include_dom_snapshot = if payload.include_dom_snapshot
            || payload.include_accessibility_tree
            || payload.include_visible_text
        {
            payload.include_dom_snapshot
        } else {
            true
        };
        let include_accessibility_tree = if payload.include_dom_snapshot
            || payload.include_accessibility_tree
            || payload.include_visible_text
        {
            payload.include_accessibility_tree
        } else {
            true
        };
        let include_visible_text = payload.include_visible_text;

        let mut sessions = self.runtime.sessions.lock().await;
        let Some(session) = sessions.get_mut(session_id.as_str()) else {
            return Ok(Response::new(browser_v1::ObserveResponse {
                v: CANONICAL_PROTOCOL_MAJOR,
                success: false,
                dom_snapshot: String::new(),
                accessibility_tree: String::new(),
                visible_text: String::new(),
                dom_truncated: false,
                accessibility_tree_truncated: false,
                visible_text_truncated: false,
                page_url: String::new(),
                error: "session_not_found".to_owned(),
            }));
        };
        session.last_active = Instant::now();
        if session.last_page_body.trim().is_empty() {
            return Ok(Response::new(browser_v1::ObserveResponse {
                v: CANONICAL_PROTOCOL_MAJOR,
                success: false,
                dom_snapshot: String::new(),
                accessibility_tree: String::new(),
                visible_text: String::new(),
                dom_truncated: false,
                accessibility_tree_truncated: false,
                visible_text_truncated: false,
                page_url: String::new(),
                error: "navigate must succeed before observe".to_owned(),
            }));
        }

        let max_dom_snapshot_bytes =
            payload.max_dom_snapshot_bytes.max(1).min(session.budget.max_observe_snapshot_bytes)
                as usize;
        let max_accessibility_tree_bytes = payload
            .max_accessibility_tree_bytes
            .max(1)
            .min(session.budget.max_observe_snapshot_bytes)
            as usize;
        let max_visible_text_bytes =
            payload.max_visible_text_bytes.max(1).min(session.budget.max_visible_text_bytes)
                as usize;

        let (dom_snapshot, dom_truncated) = if include_dom_snapshot {
            build_dom_snapshot(session.last_page_body.as_str(), max_dom_snapshot_bytes)
        } else {
            (String::new(), false)
        };
        let (accessibility_tree, accessibility_tree_truncated) = if include_accessibility_tree {
            build_accessibility_tree_snapshot(
                session.last_page_body.as_str(),
                max_accessibility_tree_bytes,
            )
        } else {
            (String::new(), false)
        };
        let (visible_text, visible_text_truncated) = if include_visible_text {
            build_visible_text_snapshot(session.last_page_body.as_str(), max_visible_text_bytes)
        } else {
            (String::new(), false)
        };

        Ok(Response::new(browser_v1::ObserveResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            success: true,
            dom_snapshot,
            accessibility_tree,
            visible_text,
            dom_truncated,
            accessibility_tree_truncated,
            visible_text_truncated,
            page_url: normalize_url_with_redaction(session.last_url.as_deref().unwrap_or_default()),
            error: String::new(),
        }))
    }

    async fn network_log(
        &self,
        request: Request<browser_v1::NetworkLogRequest>,
    ) -> Result<Response<browser_v1::NetworkLogResponse>, Status> {
        self.runtime.authorize(request.metadata()).await?;
        let mut payload = request.into_inner();
        let session_id = parse_session_id_from_proto(payload.session_id.take())
            .map_err(Status::invalid_argument)?;
        let mut sessions = self.runtime.sessions.lock().await;
        let Some(session) = sessions.get_mut(session_id.as_str()) else {
            return Ok(Response::new(browser_v1::NetworkLogResponse {
                v: CANONICAL_PROTOCOL_MAJOR,
                success: false,
                entries: Vec::new(),
                truncated: false,
                error: "session_not_found".to_owned(),
            }));
        };
        session.last_active = Instant::now();
        let limit = if payload.limit == 0 {
            session.budget.max_network_log_entries
        } else {
            usize::try_from(payload.limit).unwrap_or(usize::MAX)
        }
        .min(session.budget.max_network_log_entries)
        .max(1);
        let max_payload_bytes =
            payload.max_payload_bytes.max(1).min(session.budget.max_network_log_bytes) as usize;

        let start = session.network_log.len().saturating_sub(limit);
        let mut truncated = start > 0;
        let mut entries = session
            .network_log
            .iter()
            .skip(start)
            .cloned()
            .map(|entry| network_log_entry_to_proto(entry, payload.include_headers))
            .collect::<Vec<_>>();
        truncated = truncate_network_log_payload(&mut entries, max_payload_bytes) || truncated;

        Ok(Response::new(browser_v1::NetworkLogResponse {
            v: CANONICAL_PROTOCOL_MAJOR,
            success: true,
            entries,
            truncated,
            error: String::new(),
        }))
    }
}

#[derive(Debug, Clone)]
struct NavigateOutcome {
    success: bool,
    final_url: String,
    status_code: u16,
    title: String,
    page_body: String,
    body_bytes: u64,
    latency_ms: u64,
    error: String,
    network_log: Vec<NetworkLogEntryInternal>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let args = Args::parse();
    let runtime = Arc::new(BrowserRuntimeState::new(&args)?);
    spawn_cleanup_loop(Arc::clone(&runtime));

    let build = build_metadata();
    info!(
        service = "palyra-browserd",
        version = build.version,
        git_hash = build.git_hash,
        build_profile = build.build_profile,
        bind_addr = %args.bind,
        port = args.port,
        grpc_bind_addr = %args.grpc_bind,
        grpc_port = args.grpc_port,
        auth_enabled = runtime.auth_token.is_some(),
        "browser service startup"
    );

    let app = Router::new()
        .route("/healthz", get(health_handler))
        .with_state(AppState { runtime: Arc::clone(&runtime) });

    let admin_address =
        parse_daemon_bind_socket(&args.bind, args.port).context("invalid bind address or port")?;
    let grpc_address = parse_daemon_bind_socket(&args.grpc_bind, args.grpc_port)
        .context("invalid gRPC bind address or port")?;
    let admin_listener = tokio::net::TcpListener::bind(admin_address)
        .await
        .context("failed to bind browserd health listener")?;
    let grpc_listener = tokio::net::TcpListener::bind(grpc_address)
        .await
        .context("failed to bind browserd gRPC listener")?;

    info!(
        listen_addr = %admin_listener.local_addr().context("health local_addr")?,
        "browserd health endpoint ready"
    );
    info!(
        grpc_listen_addr = %grpc_listener.local_addr().context("grpc local_addr")?,
        "browserd gRPC endpoint ready"
    );

    let http_server = axum::serve(admin_listener, app).with_graceful_shutdown(shutdown_signal());
    let grpc_server = Server::builder()
        .add_service(browser_v1::browser_service_server::BrowserServiceServer::new(
            BrowserServiceImpl { runtime: Arc::clone(&runtime) },
        ))
        .serve_with_incoming_shutdown(TcpListenerStream::new(grpc_listener), shutdown_signal());

    let (http_result, grpc_result) = tokio::join!(http_server, grpc_server);
    http_result.context("browserd health server failed")?;
    grpc_result.context("browserd gRPC server failed")?;
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().json().with_env_filter(filter).init();
}

async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json::<HealthResponse>(health_response("palyra-browserd", state.runtime.started_at))
}

fn spawn_cleanup_loop(runtime: Arc<BrowserRuntimeState>) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(CLEANUP_INTERVAL_MS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let now = Instant::now();
            runtime.sessions.lock().await.retain(|_, session| {
                let idle_alive =
                    now.saturating_duration_since(session.last_active) <= session.idle_ttl;
                let lifetime_alive = now.saturating_duration_since(session.created_at)
                    <= Duration::from_millis(session.budget.max_session_lifetime_ms);
                idle_alive && lifetime_alive
            });
        }
    });
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(error = %error, "failed to register Ctrl+C handler");
        std::future::pending::<()>().await;
    }
}

async fn navigate_with_guards(
    raw_url: &str,
    timeout_ms: u64,
    allow_redirects: bool,
    max_redirects: u32,
    allow_private_targets: bool,
    max_response_bytes: u64,
) -> NavigateOutcome {
    let started_at = Instant::now();
    let mut network_log = Vec::new();
    let mut current_url = match Url::parse(raw_url) {
        Ok(value) => value,
        Err(error) => {
            return NavigateOutcome {
                success: false,
                final_url: String::new(),
                status_code: 0,
                title: String::new(),
                page_body: String::new(),
                body_bytes: 0,
                latency_ms: started_at.elapsed().as_millis() as u64,
                error: format!("invalid URL: {error}"),
                network_log,
            }
        }
    };
    let client = match reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_millis(timeout_ms.max(1)))
        .build()
    {
        Ok(value) => value,
        Err(error) => {
            return NavigateOutcome {
                success: false,
                final_url: current_url.to_string(),
                status_code: 0,
                title: String::new(),
                page_body: String::new(),
                body_bytes: 0,
                latency_ms: started_at.elapsed().as_millis() as u64,
                error: format!("failed to build HTTP client: {error}"),
                network_log,
            }
        }
    };

    let redirect_limit = max_redirects.clamp(1, 10);
    let mut redirects = 0_u32;
    loop {
        if let Err(error) = validate_target_url(&current_url, allow_private_targets).await {
            return NavigateOutcome {
                success: false,
                final_url: current_url.to_string(),
                status_code: 0,
                title: String::new(),
                page_body: String::new(),
                body_bytes: 0,
                latency_ms: started_at.elapsed().as_millis() as u64,
                error,
                network_log,
            };
        }

        let request_started = Instant::now();
        let response = match client.get(current_url.clone()).send().await {
            Ok(value) => value,
            Err(error) => {
                return NavigateOutcome {
                    success: false,
                    final_url: current_url.to_string(),
                    status_code: 0,
                    title: String::new(),
                    page_body: String::new(),
                    body_bytes: 0,
                    latency_ms: started_at.elapsed().as_millis() as u64,
                    error: format!("request failed: {error}"),
                    network_log,
                }
            }
        };
        let request_latency_ms = request_started.elapsed().as_millis() as u64;
        network_log.push(NetworkLogEntryInternal {
            request_url: normalize_url_with_redaction(current_url.as_str()),
            status_code: response.status().as_u16(),
            timing_bucket: timing_bucket_for_latency(request_latency_ms).to_owned(),
            latency_ms: request_latency_ms,
            captured_at_unix_ms: current_unix_ms(),
            headers: sanitize_network_headers(response.headers()),
        });

        if response.status().is_redirection() {
            if !allow_redirects {
                return NavigateOutcome {
                    success: false,
                    final_url: current_url.to_string(),
                    status_code: response.status().as_u16(),
                    title: String::new(),
                    page_body: String::new(),
                    body_bytes: 0,
                    latency_ms: started_at.elapsed().as_millis() as u64,
                    error: "redirect response blocked by policy".to_owned(),
                    network_log,
                };
            }
            if redirects >= redirect_limit {
                return NavigateOutcome {
                    success: false,
                    final_url: current_url.to_string(),
                    status_code: response.status().as_u16(),
                    title: String::new(),
                    page_body: String::new(),
                    body_bytes: 0,
                    latency_ms: started_at.elapsed().as_millis() as u64,
                    error: format!("redirect limit exceeded ({redirect_limit})"),
                    network_log,
                };
            }
            let Some(location) = response.headers().get(reqwest::header::LOCATION) else {
                return NavigateOutcome {
                    success: false,
                    final_url: current_url.to_string(),
                    status_code: response.status().as_u16(),
                    title: String::new(),
                    page_body: String::new(),
                    body_bytes: 0,
                    latency_ms: started_at.elapsed().as_millis() as u64,
                    error: "redirect missing Location header".to_owned(),
                    network_log,
                };
            };
            let Ok(location_str) = location.to_str() else {
                return NavigateOutcome {
                    success: false,
                    final_url: current_url.to_string(),
                    status_code: response.status().as_u16(),
                    title: String::new(),
                    page_body: String::new(),
                    body_bytes: 0,
                    latency_ms: started_at.elapsed().as_millis() as u64,
                    error: "redirect location header contains invalid UTF-8".to_owned(),
                    network_log,
                };
            };
            current_url = match current_url.join(location_str) {
                Ok(value) => value,
                Err(error) => {
                    return NavigateOutcome {
                        success: false,
                        final_url: current_url.to_string(),
                        status_code: response.status().as_u16(),
                        title: String::new(),
                        page_body: String::new(),
                        body_bytes: 0,
                        latency_ms: started_at.elapsed().as_millis() as u64,
                        error: format!("invalid redirect target: {error}"),
                        network_log,
                    }
                }
            };
            redirects = redirects.saturating_add(1);
            continue;
        }

        let status_code = response.status().as_u16();
        let body = match response.bytes().await {
            Ok(value) => value,
            Err(error) => {
                return NavigateOutcome {
                    success: false,
                    final_url: current_url.to_string(),
                    status_code,
                    title: String::new(),
                    page_body: String::new(),
                    body_bytes: 0,
                    latency_ms: started_at.elapsed().as_millis() as u64,
                    error: format!("failed to read response body: {error}"),
                    network_log,
                }
            }
        };

        if (body.len() as u64) > max_response_bytes {
            return NavigateOutcome {
                success: false,
                final_url: current_url.to_string(),
                status_code,
                title: String::new(),
                page_body: String::new(),
                body_bytes: body.len() as u64,
                latency_ms: started_at.elapsed().as_millis() as u64,
                error: format!(
                    "response exceeds max_response_bytes ({} > {max_response_bytes})",
                    body.len()
                ),
                network_log,
            };
        }

        let page_body = String::from_utf8_lossy(body.as_ref()).to_string();

        return NavigateOutcome {
            success: (200..400).contains(&status_code),
            final_url: current_url.to_string(),
            status_code,
            title: extract_html_title(page_body.as_str()).unwrap_or_default().to_owned(),
            page_body,
            body_bytes: body.len() as u64,
            latency_ms: started_at.elapsed().as_millis() as u64,
            error: if status_code >= 400 {
                format!("navigation returned HTTP {status_code}")
            } else {
                String::new()
            },
            network_log,
        };
    }
}

async fn validate_target_url(url: &Url, allow_private_targets: bool) -> Result<(), String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!("blocked URL scheme '{}'", url.scheme()));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("URL credentials are not allowed".to_owned());
    }
    let host = url.host_str().ok_or_else(|| "URL host is required".to_owned())?;
    let port =
        url.port_or_known_default().ok_or_else(|| "URL port could not be resolved".to_owned())?;

    let addresses = if let Ok(address) = host.parse::<IpAddr>() {
        vec![address]
    } else {
        tokio::net::lookup_host((host, port))
            .await
            .map_err(|error| format!("DNS resolution failed for host '{host}': {error}"))?
            .map(|socket| socket.ip())
            .collect::<Vec<_>>()
    };

    if addresses.is_empty() {
        return Err(format!("DNS resolution returned no addresses for host '{host}'"));
    }
    if !allow_private_targets && addresses.iter().any(|address| is_private_or_local_ip(*address)) {
        return Err("target resolves to private/local address and is blocked by policy".to_owned());
    }
    Ok(())
}

fn is_private_or_local_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ipv4) => {
            ipv4.is_private() || ipv4.is_loopback() || ipv4.is_link_local() || ipv4.is_unspecified()
        }
        IpAddr::V6(ipv6) => {
            if let Some(mapped) = ipv6.to_ipv4_mapped() {
                return mapped.is_private()
                    || mapped.is_loopback()
                    || mapped.is_link_local()
                    || mapped.is_unspecified();
            }
            ipv6.is_loopback()
                || ipv6.is_unicast_link_local()
                || ipv6.is_unique_local()
                || ipv6.is_unspecified()
        }
    }
}

fn extract_html_title(body: &str) -> Option<&str> {
    let lower = body.to_ascii_lowercase();
    let start = lower.find("<title>")?;
    let end = lower[start + 7..].find("</title>")?;
    Some(body[start + 7..start + 7 + end].trim())
}

fn truncate_utf8_bytes_with_flag(raw: &str, max_bytes: usize) -> (String, bool) {
    let truncated = truncate_utf8_bytes(raw, max_bytes);
    let was_truncated = truncated.len() < raw.len();
    (truncated, was_truncated)
}

fn append_network_log_entries(
    session: &mut BrowserSessionRecord,
    entries: &[NetworkLogEntryInternal],
) {
    for entry in entries {
        session.network_log.push_back(entry.clone());
    }
    while session.network_log.len() > session.budget.max_network_log_entries {
        session.network_log.pop_front();
    }
    while session.network_log.iter().map(estimate_network_log_entry_internal_bytes).sum::<usize>()
        > session.budget.max_network_log_bytes as usize
    {
        if session.network_log.pop_front().is_none() {
            break;
        }
    }
}

fn estimate_network_log_entry_internal_bytes(entry: &NetworkLogEntryInternal) -> usize {
    let headers_bytes = entry
        .headers
        .iter()
        .map(|header| header.name.len() + header.value.len() + 8)
        .sum::<usize>();
    entry.request_url.len() + entry.timing_bucket.len() + headers_bytes + 64
}

fn network_log_entry_to_proto(
    entry: NetworkLogEntryInternal,
    include_headers: bool,
) -> browser_v1::NetworkLogEntry {
    let headers = if include_headers {
        entry
            .headers
            .into_iter()
            .map(|header| browser_v1::NetworkLogHeader {
                v: CANONICAL_PROTOCOL_MAJOR,
                name: header.name,
                value: header.value,
            })
            .collect()
    } else {
        Vec::new()
    };
    browser_v1::NetworkLogEntry {
        v: CANONICAL_PROTOCOL_MAJOR,
        request_url: entry.request_url,
        status_code: u32::from(entry.status_code),
        timing_bucket: entry.timing_bucket,
        latency_ms: entry.latency_ms,
        captured_at_unix_ms: entry.captured_at_unix_ms,
        headers,
    }
}

fn estimate_network_log_payload_bytes(entries: &[browser_v1::NetworkLogEntry]) -> usize {
    entries.iter().map(estimate_network_log_proto_entry_bytes).sum::<usize>() + 2
}

fn estimate_network_log_proto_entry_bytes(entry: &browser_v1::NetworkLogEntry) -> usize {
    let headers = entry.headers.iter().map(estimate_network_log_proto_header_bytes).sum::<usize>();
    entry.request_url.len() + entry.timing_bucket.len() + headers + 64
}

fn estimate_network_log_proto_header_bytes(header: &browser_v1::NetworkLogHeader) -> usize {
    header.name.len() + header.value.len() + 8
}

fn truncate_network_log_payload(
    entries: &mut Vec<browser_v1::NetworkLogEntry>,
    max_payload_bytes: usize,
) -> bool {
    let mut truncated = false;
    while !entries.is_empty()
        && estimate_network_log_payload_bytes(entries.as_slice()) > max_payload_bytes
    {
        entries.remove(0);
        truncated = true;
    }
    truncated
}

fn timing_bucket_for_latency(latency_ms: u64) -> &'static str {
    if latency_ms <= 100 {
        "lt_100ms"
    } else if latency_ms <= 500 {
        "100_500ms"
    } else if latency_ms <= 2_000 {
        "500ms_2s"
    } else {
        "gt_2s"
    }
}

fn sanitize_network_headers(headers: &reqwest::header::HeaderMap) -> Vec<NetworkLogHeaderInternal> {
    let mut output = headers
        .iter()
        .take(MAX_NETWORK_LOG_HEADER_COUNT)
        .map(|(name, value)| {
            let header_name = name.as_str().to_ascii_lowercase();
            let raw_value = value.to_str().unwrap_or("<non_utf8>");
            let sanitized = sanitize_single_network_header(header_name.as_str(), raw_value);
            NetworkLogHeaderInternal { name: header_name, value: sanitized }
        })
        .collect::<Vec<_>>();
    output.sort_by(|left, right| left.name.cmp(&right.name));
    output
}

fn sanitize_single_network_header(name: &str, raw_value: &str) -> String {
    if name.eq_ignore_ascii_case("location")
        || raw_value.starts_with("http://")
        || raw_value.starts_with("https://")
    {
        return normalize_url_with_redaction(raw_value);
    }
    if is_sensitive_header_name(name) || contains_sensitive_material(raw_value) {
        return "<redacted>".to_owned();
    }
    truncate_utf8_bytes(raw_value, MAX_NETWORK_LOG_HEADER_VALUE_BYTES)
}

fn is_sensitive_header_name(name: &str) -> bool {
    matches!(
        name,
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "set-cookie"
            | "x-api-key"
            | "x-auth-token"
            | "x-csrf-token"
    ) || name.contains("token")
        || name.contains("secret")
        || name.contains("password")
}

fn contains_sensitive_material(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    [
        "bearer ",
        "token=",
        "access_token=",
        "id_token=",
        "refresh_token=",
        "session=",
        "password=",
        "passwd=",
        "secret=",
        "api_key=",
        "apikey=",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn normalize_url_with_redaction(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Ok(parsed) = Url::parse(trimmed) {
        let Some(host) = parsed.host_str() else {
            return truncate_utf8_bytes(
                redact_query_from_raw(trimmed).as_str(),
                MAX_NETWORK_LOG_URL_BYTES,
            );
        };
        let mut output = format!("{}://{host}", parsed.scheme());
        if let Some(port) = parsed.port() {
            if !is_default_port(parsed.scheme(), port) {
                output.push(':');
                output.push_str(port.to_string().as_str());
            }
        }
        if parsed.path().is_empty() {
            output.push('/');
        } else {
            output.push_str(parsed.path());
        }
        if let Some(query) = parsed.query() {
            let redacted = redact_query_pairs(query);
            if !redacted.is_empty() {
                output.push('?');
                output.push_str(redacted.as_str());
            }
        }
        return truncate_utf8_bytes(output.as_str(), MAX_NETWORK_LOG_URL_BYTES);
    }
    truncate_utf8_bytes(redact_query_from_raw(trimmed).as_str(), MAX_NETWORK_LOG_URL_BYTES)
}

fn redact_query_from_raw(raw: &str) -> String {
    let without_fragment = raw.split('#').next().unwrap_or_default();
    let Some((base, query)) = without_fragment.split_once('?') else {
        return without_fragment.to_owned();
    };
    let redacted = redact_query_pairs(query);
    if redacted.is_empty() {
        base.to_owned()
    } else {
        format!("{base}?{redacted}")
    }
}

fn redact_query_pairs(query: &str) -> String {
    query
        .split('&')
        .filter(|pair| !pair.trim().is_empty())
        .map(|pair| {
            let (raw_key, raw_value_opt) = pair
                .split_once('=')
                .map(|(key, value)| (key.trim(), Some(value)))
                .unwrap_or_else(|| (pair.trim(), None));
            if raw_key.is_empty() {
                return String::new();
            }
            let value = raw_value_opt.unwrap_or_default();
            let sanitized = if is_sensitive_query_key(raw_key) || contains_sensitive_material(value)
            {
                "<redacted>".to_owned()
            } else {
                truncate_utf8_bytes(value, 128)
            };
            if raw_value_opt.is_some() {
                format!("{raw_key}={sanitized}")
            } else {
                raw_key.to_owned()
            }
        })
        .filter(|pair| !pair.is_empty())
        .collect::<Vec<_>>()
        .join("&")
}

fn is_sensitive_query_key(raw_key: &str) -> bool {
    let key = raw_key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "token"
            | "access_token"
            | "id_token"
            | "refresh_token"
            | "auth"
            | "authorization"
            | "api_key"
            | "apikey"
            | "password"
            | "passwd"
            | "secret"
            | "signature"
            | "sig"
            | "session"
            | "session_id"
            | "jwt"
    ) || key.contains("token")
        || key.contains("secret")
        || key.contains("password")
}

fn is_default_port(scheme: &str, port: u16) -> bool {
    matches!((scheme, port), ("http", 80) | ("https", 443))
}

fn build_dom_snapshot(page_body: &str, max_bytes: usize) -> (String, bool) {
    let lines = collect_opening_tags(page_body)
        .iter()
        .enumerate()
        .map(|(index, tag)| build_dom_line(index + 1, tag.as_str()))
        .collect::<Vec<_>>();
    let content = lines.join("\n");
    truncate_utf8_bytes_with_flag(content.as_str(), max_bytes)
}

fn build_dom_line(index: usize, tag: &str) -> String {
    let tag_lower = tag.to_ascii_lowercase();
    let name = html_tag_name(tag_lower.as_str()).unwrap_or("unknown");
    let mut attributes = Vec::new();
    for attr_name in [
        "id",
        "class",
        "name",
        "role",
        "aria-label",
        "type",
        "href",
        "src",
        "action",
        "title",
        "alt",
        "placeholder",
    ] {
        let Some(value) = extract_attr_value(tag_lower.as_str(), attr_name) else {
            continue;
        };
        let sanitized = sanitize_snapshot_attribute(attr_name, value.as_str());
        if sanitized.is_empty() {
            continue;
        }
        attributes.push(format!("{attr_name}=\"{sanitized}\""));
    }
    if attributes.is_empty() {
        format!("{index:04} <{name}>")
    } else {
        format!("{index:04} <{name} {}>", attributes.join(" "))
    }
}

fn sanitize_snapshot_attribute(attr_name: &str, raw_value: &str) -> String {
    if raw_value.trim().is_empty() {
        return String::new();
    }
    let lower = attr_name.to_ascii_lowercase();
    if matches!(lower.as_str(), "value" | "password" | "token") {
        return "<redacted>".to_owned();
    }
    if lower == "href" || lower == "src" || lower == "action" {
        return normalize_url_with_redaction(raw_value);
    }
    if contains_sensitive_material(raw_value) {
        return "<redacted>".to_owned();
    }
    truncate_utf8_bytes(raw_value, 128)
}

fn build_accessibility_tree_snapshot(page_body: &str, max_bytes: usize) -> (String, bool) {
    let mut lines = Vec::new();
    for (index, tag) in collect_opening_tags(page_body).iter().enumerate() {
        if let Some(line) = build_accessibility_line(index + 1, tag.as_str()) {
            lines.push(line);
        }
    }
    let content = lines.join("\n");
    truncate_utf8_bytes_with_flag(content.as_str(), max_bytes)
}

fn build_accessibility_line(index: usize, tag: &str) -> Option<String> {
    let tag_lower = tag.to_ascii_lowercase();
    let role = accessibility_role_for_tag(tag_lower.as_str())?;
    let tag_name = html_tag_name(tag_lower.as_str()).unwrap_or("unknown");
    let name = accessibility_name_for_tag(tag_lower.as_str());
    let selector = accessibility_selector_for_tag(tag_lower.as_str());
    Some(format!("{index:04} role={role}; name={name}; tag={tag_name}; selector={selector}"))
}

fn accessibility_role_for_tag(tag_lower: &str) -> Option<String> {
    if let Some(explicit_role) = extract_attr_value(tag_lower, "role")
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return Some(truncate_utf8_bytes(explicit_role.as_str(), 64));
    }
    let tag_name = html_tag_name(tag_lower)?;
    let inferred = match tag_name {
        "a" => "link",
        "button" => "button",
        "textarea" => "textbox",
        "select" => "combobox",
        "img" => "img",
        "form" => "form",
        "nav" => "navigation",
        "main" => "main",
        "header" => "banner",
        "footer" => "contentinfo",
        "ul" | "ol" => "list",
        "li" => "listitem",
        "table" => "table",
        "tr" => "row",
        "td" => "cell",
        "th" => "columnheader",
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => "heading",
        "input" => match extract_attr_value(tag_lower, "type")
            .unwrap_or_else(|| "text".to_owned())
            .as_str()
        {
            "checkbox" => "checkbox",
            "radio" => "radio",
            "submit" | "button" | "reset" => "button",
            "search" | "email" | "url" | "tel" | "text" | "password" => "textbox",
            _ => "input",
        },
        _ => return None,
    };
    Some(inferred.to_owned())
}

fn accessibility_name_for_tag(tag_lower: &str) -> String {
    for attr_name in ["aria-label", "title", "alt", "placeholder", "name", "id"] {
        if let Some(value) = extract_attr_value(tag_lower, attr_name)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
        {
            if contains_sensitive_material(value.as_str()) {
                return "<redacted>".to_owned();
            }
            return truncate_utf8_bytes(value.as_str(), 128);
        }
    }
    if let Some(href) = extract_attr_value(tag_lower, "href")
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return normalize_url_with_redaction(href.as_str());
    }
    "-".to_owned()
}

fn accessibility_selector_for_tag(tag_lower: &str) -> String {
    if let Some(id) = extract_attr_value(tag_lower, "id")
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return format!("#{}", truncate_utf8_bytes(id.as_str(), 96));
    }
    if let Some(name) = extract_attr_value(tag_lower, "name")
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return format!("[name={}]", truncate_utf8_bytes(name.as_str(), 96));
    }
    if let Some(class) = extract_attr_value(tag_lower, "class")
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        let first_class = class.split_ascii_whitespace().next().unwrap_or_default();
        if !first_class.is_empty() {
            return format!(".{}", truncate_utf8_bytes(first_class, 96));
        }
    }
    "-".to_owned()
}

fn build_visible_text_snapshot(page_body: &str, max_bytes: usize) -> (String, bool) {
    let without_scripts = strip_tag_block_case_insensitive(page_body, "script");
    let without_styles = strip_tag_block_case_insensitive(without_scripts.as_str(), "style");
    let without_comments = strip_html_comments(without_styles.as_str());
    let mut visible = String::new();
    let mut inside_tag = false;
    for character in without_comments.chars() {
        if character == '<' {
            inside_tag = true;
            visible.push(' ');
            continue;
        }
        if character == '>' {
            inside_tag = false;
            visible.push(' ');
            continue;
        }
        if !inside_tag {
            visible.push(character);
        }
    }
    let collapsed = visible.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_utf8_bytes_with_flag(collapsed.as_str(), max_bytes)
}

fn strip_tag_block_case_insensitive(input: &str, tag_name: &str) -> String {
    let mut output = String::new();
    let lower = input.to_ascii_lowercase();
    let open_pattern = format!("<{tag_name}");
    let close_pattern = format!("</{tag_name}>");
    let mut cursor = 0usize;
    while let Some(rel_open) = lower[cursor..].find(open_pattern.as_str()) {
        let open = cursor + rel_open;
        output.push_str(&input[cursor..open]);
        let Some(rel_close) = lower[open..].find(close_pattern.as_str()) else {
            cursor = input.len();
            break;
        };
        let close_start = open + rel_close;
        cursor = close_start + close_pattern.len();
    }
    if cursor < input.len() {
        output.push_str(&input[cursor..]);
    }
    output
}

fn strip_html_comments(input: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    while let Some(rel_start) = input[cursor..].find("<!--") {
        let start = cursor + rel_start;
        output.push_str(&input[cursor..start]);
        let Some(rel_end) = input[start + 4..].find("-->") else {
            cursor = input.len();
            break;
        };
        cursor = start + 4 + rel_end + 3;
    }
    if cursor < input.len() {
        output.push_str(&input[cursor..]);
    }
    output
}

fn collect_opening_tags(html: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel_start) = html[cursor..].find('<') {
        let start = cursor + rel_start;
        let Some(rel_end) = html[start..].find('>') else {
            break;
        };
        let end = start + rel_end;
        let tag = &html[start..=end];
        if tag.starts_with("</") || tag.starts_with("<!") || tag.starts_with("<?") {
            cursor = end.saturating_add(1);
            continue;
        }
        let tag_lower = tag.to_ascii_lowercase();
        if matches!(html_tag_name(tag_lower.as_str()), Some("script" | "style")) {
            cursor = end.saturating_add(1);
            continue;
        }
        tags.push(tag.to_owned());
        cursor = end.saturating_add(1);
    }
    tags
}

fn truncate_utf8_bytes(raw: &str, max_bytes: usize) -> String {
    if raw.len() <= max_bytes {
        return raw.to_owned();
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !raw.is_char_boundary(boundary) {
        boundary -= 1;
    }
    raw[..boundary].to_owned()
}

fn parse_session_id(raw: Option<&str>) -> Result<String, String> {
    let value = raw.unwrap_or_default().trim();
    if value.is_empty() {
        return Err("session_id is required".to_owned());
    }
    validate_canonical_id(value).map_err(|error| format!("invalid session_id: {error}"))?;
    Ok(value.to_owned())
}

fn parse_session_id_from_proto(
    raw: Option<proto::palyra::common::v1::CanonicalId>,
) -> Result<String, String> {
    match raw {
        Some(value) => parse_session_id(Some(value.ulid.as_str())),
        None => parse_session_id(None),
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone)]
struct ActionSessionSnapshot {
    budget: SessionBudget,
    page_body: String,
    allow_downloads: bool,
}

#[derive(Debug, Clone, Copy)]
struct FinalizeActionRequest<'a> {
    action_name: &'a str,
    selector: &'a str,
    success: bool,
    outcome: &'a str,
    error: &'a str,
    started_at_unix_ms: u64,
    attempts: u32,
    capture_failure_screenshot: bool,
    max_failure_screenshot_bytes: u64,
}

async fn consume_action_budget_and_snapshot(
    runtime: &BrowserRuntimeState,
    session_id: &str,
    require_page_body: bool,
) -> Result<ActionSessionSnapshot, String> {
    let mut sessions = runtime.sessions.lock().await;
    let Some(session) = sessions.get_mut(session_id) else {
        return Err("session_not_found".to_owned());
    };
    session.last_active = Instant::now();
    enforce_action_domain_allowlist(session)?;
    if require_page_body && session.last_page_body.trim().is_empty() {
        return Err("navigate must succeed before performing this browser action".to_owned());
    }

    let now = Instant::now();
    let rate_window = Duration::from_millis(session.budget.action_rate_window_ms.max(1));
    while let Some(front) = session.action_window.front().copied() {
        if now.saturating_duration_since(front) > rate_window {
            session.action_window.pop_front();
        } else {
            break;
        }
    }
    if session.action_count >= session.budget.max_actions_per_session {
        return Err(format!(
            "session action budget exceeded ({} >= {})",
            session.action_count, session.budget.max_actions_per_session
        ));
    }
    if session.action_window.len() as u64 >= session.budget.max_actions_per_window {
        return Err(format!(
            "session action rate limit exceeded ({} per {}ms)",
            session.budget.max_actions_per_window, session.budget.action_rate_window_ms
        ));
    }
    session.action_count = session.action_count.saturating_add(1);
    session.action_window.push_back(now);

    Ok(ActionSessionSnapshot {
        budget: session.budget.clone(),
        page_body: session.last_page_body.clone(),
        allow_downloads: session.allow_downloads,
    })
}

fn enforce_action_domain_allowlist(session: &BrowserSessionRecord) -> Result<(), String> {
    if session.action_allowed_domains.is_empty() {
        return Ok(());
    }
    let Some(current_url) = session.last_url.as_deref() else {
        return Err(
            "action domain allowlist is configured but session has no active URL".to_owned()
        );
    };
    let current_host = Url::parse(current_url)
        .ok()
        .and_then(|url| url.host_str().map(|value| value.to_ascii_lowercase()))
        .ok_or_else(|| "failed to resolve host for action domain allowlist check".to_owned())?;
    if session.action_allowed_domains.iter().any(|domain| {
        current_host == *domain || current_host.ends_with(format!(".{domain}").as_str())
    }) {
        return Ok(());
    }
    Err(format!("current page host '{current_host}' is blocked by action domain allowlist"))
}

fn normalize_action_allowed_domains(values: &[String]) -> Vec<String> {
    let mut domains = values
        .iter()
        .filter_map(|value| normalize_single_allowed_domain(value.as_str()))
        .collect::<Vec<_>>();
    domains.sort();
    domains.dedup();
    domains
}

fn normalize_single_allowed_domain(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let from_url = Url::parse(trimmed).ok().and_then(|url| url.host_str().map(str::to_owned));
    let value = from_url.unwrap_or_else(|| {
        trimmed
            .split('/')
            .next()
            .unwrap_or_default()
            .split(':')
            .next()
            .unwrap_or_default()
            .to_owned()
    });
    let normalized = value.trim().trim_matches('.').to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    if normalized.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-') {
        Some(normalized)
    } else {
        None
    }
}

async fn finalize_session_action(
    runtime: &BrowserRuntimeState,
    session_id: &str,
    request: FinalizeActionRequest<'_>,
) -> (Option<browser_v1::BrowserActionLogEntry>, Vec<u8>, String) {
    let mut sessions = runtime.sessions.lock().await;
    let Some(session) = sessions.get_mut(session_id) else {
        return (None, Vec::new(), String::new());
    };
    let entry = BrowserActionLogEntryInternal {
        action_id: Ulid::new().to_string(),
        action_name: request.action_name.to_owned(),
        selector: request.selector.to_owned(),
        success: request.success,
        outcome: request.outcome.to_owned(),
        error: request.error.to_owned(),
        started_at_unix_ms: request.started_at_unix_ms,
        completed_at_unix_ms: current_unix_ms(),
        attempts: request.attempts,
        page_url: session.last_url.clone().unwrap_or_default(),
    };
    session.last_active = Instant::now();
    session.action_log.push_back(entry.clone());
    while session.action_log.len() > session.budget.max_action_log_entries {
        session.action_log.pop_front();
    }
    let (failure_screenshot_bytes, failure_screenshot_mime_type) =
        if !request.success && request.capture_failure_screenshot {
            let max_bytes = if request.max_failure_screenshot_bytes == 0 {
                session.budget.max_screenshot_bytes
            } else {
                request.max_failure_screenshot_bytes.min(session.budget.max_screenshot_bytes)
            };
            if (ONE_BY_ONE_PNG.len() as u64) <= max_bytes {
                (ONE_BY_ONE_PNG.to_vec(), "image/png".to_owned())
            } else {
                (Vec::new(), String::new())
            }
        } else {
            (Vec::new(), String::new())
        };
    (
        Some(browser_v1::BrowserActionLogEntry {
            v: CANONICAL_PROTOCOL_MAJOR,
            action_id: entry.action_id,
            action_name: entry.action_name,
            selector: entry.selector,
            success: entry.success,
            outcome: entry.outcome,
            error: entry.error,
            started_at_unix_ms: entry.started_at_unix_ms,
            completed_at_unix_ms: entry.completed_at_unix_ms,
            attempts: entry.attempts,
            page_url: entry.page_url,
        }),
        failure_screenshot_bytes,
        failure_screenshot_mime_type,
    )
}

fn find_matching_html_tag(selector: &str, html: &str) -> Option<String> {
    let selector = selector.trim();
    if selector.is_empty() {
        return None;
    }
    let selector_lower = selector.to_ascii_lowercase();
    let mut cursor = 0usize;
    while let Some(rel_start) = html[cursor..].find('<') {
        let start = cursor + rel_start;
        let Some(rel_end) = html[start..].find('>') else {
            break;
        };
        let end = start + rel_end;
        let tag = &html[start..=end];
        if tag.starts_with("</") {
            cursor = end.saturating_add(1);
            continue;
        }
        if html_tag_matches_selector(tag, selector, selector_lower.as_str()) {
            return Some(tag.to_owned());
        }
        cursor = end.saturating_add(1);
    }
    None
}

fn html_tag_matches_selector(tag: &str, selector: &str, selector_lower: &str) -> bool {
    let tag_lower = tag.to_ascii_lowercase();
    if let Some(id) = selector.strip_prefix('#') {
        return has_attr_value(tag_lower.as_str(), "id", id.trim().to_ascii_lowercase().as_str());
    }
    if let Some(class) = selector.strip_prefix('.') {
        let class = class.trim().to_ascii_lowercase();
        let Some(value) = extract_attr_value(tag_lower.as_str(), "class") else {
            return false;
        };
        return value
            .split_ascii_whitespace()
            .any(|token| token.eq_ignore_ascii_case(class.as_str()));
    }
    if selector.starts_with('[') && selector.ends_with(']') {
        let inner = selector[1..selector.len().saturating_sub(1)].trim();
        if let Some(value) = inner.strip_prefix("name=") {
            let value = value.trim().trim_matches('"').trim_matches('\'').to_ascii_lowercase();
            return has_attr_value(tag_lower.as_str(), "name", value.as_str());
        }
        return false;
    }
    html_tag_name(tag_lower.as_str())
        .map(|name| name.eq_ignore_ascii_case(selector_lower))
        .unwrap_or(false)
}

fn has_attr_value(tag_lower: &str, attr_name: &str, expected_value_lower: &str) -> bool {
    extract_attr_value(tag_lower, attr_name)
        .map(|value| value.eq_ignore_ascii_case(expected_value_lower))
        .unwrap_or(false)
}

fn extract_attr_value(tag_lower: &str, attr_name: &str) -> Option<String> {
    let needle = format!("{attr_name}=");
    let start = tag_lower.find(needle.as_str())?;
    let mut value = &tag_lower[start + needle.len()..];
    value = value.trim_start();
    if let Some(stripped) = value.strip_prefix('"') {
        let end = stripped.find('"')?;
        return Some(stripped[..end].to_owned());
    }
    if let Some(stripped) = value.strip_prefix('\'') {
        let end = stripped.find('\'')?;
        return Some(stripped[..end].to_owned());
    }
    let end = value
        .find(|ch: char| ch.is_ascii_whitespace() || ch == '>' || ch == '/')
        .unwrap_or(value.len());
    Some(value[..end].to_owned())
}

fn html_tag_name(tag_lower: &str) -> Option<&str> {
    let trimmed = tag_lower.trim_start_matches('<').trim_start();
    let end = trimmed
        .find(|ch: char| ch.is_ascii_whitespace() || ch == '>' || ch == '/')
        .unwrap_or(trimmed.len());
    if end == 0 {
        None
    } else {
        Some(&trimmed[..end])
    }
}

fn is_typable_tag(tag: &str) -> bool {
    let tag_lower = tag.to_ascii_lowercase();
    matches!(html_tag_name(tag_lower.as_str()), Some("input" | "textarea"))
}

fn is_download_like_tag(tag: &str) -> bool {
    let tag_lower = tag.to_ascii_lowercase();
    if html_tag_name(tag_lower.as_str()) != Some("a") {
        return false;
    }
    if tag_lower.contains(" download")
        || tag_lower.contains(" download=")
        || tag_lower.ends_with("download>")
    {
        return true;
    }
    let Some(href) = extract_attr_value(tag_lower.as_str(), "href") else {
        return false;
    };
    let href = href.split('?').next().unwrap_or_default();
    let href = href.to_ascii_lowercase();
    [
        ".zip", ".gz", ".tar", ".7z", ".rar", ".pdf", ".csv", ".json", ".txt", ".doc", ".docx",
        ".xls", ".xlsx", ".ppt", ".pptx", ".exe", ".msi",
    ]
    .iter()
    .any(|suffix| href.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::{
        browser_v1, navigate_with_guards, Args, BrowserRuntimeState, BrowserServiceImpl,
        ONE_BY_ONE_PNG,
    };
    use crate::proto;
    use crate::proto::palyra::browser::v1::browser_service_server::BrowserService;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;
    use tonic::Request;

    #[tokio::test(flavor = "multi_thread")]
    async fn navigate_with_guards_blocks_file_scheme() {
        let outcome =
            navigate_with_guards("file:///tmp/index.html", 1_000, true, 3, false, 1024).await;
        assert!(!outcome.success, "file scheme must be blocked");
        assert!(
            outcome.error.contains("blocked URL scheme"),
            "error should explain blocked scheme: {}",
            outcome.error
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn navigate_with_guards_enforces_response_size_limit() {
        let (url, handle) = spawn_static_http_server(
            200,
            "<html><head><title>Oversized</title></head><body>very large</body></html>",
        );
        let outcome = navigate_with_guards(url.as_str(), 2_000, true, 3, true, 16).await;
        assert!(!outcome.success, "oversized payload must fail");
        assert!(
            outcome.error.contains("max_response_bytes"),
            "size limit error should be explicit: {}",
            outcome.error
        );
        handle.join().expect("test server thread should exit");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn navigate_with_guards_blocks_private_target_by_default() {
        let outcome =
            navigate_with_guards("http://127.0.0.1:8080/", 1_000, true, 3, false, 1024).await;
        assert!(!outcome.success, "private targets should be blocked by default");
        assert!(
            outcome.error.contains("private/local"),
            "error should explain private target block: {}",
            outcome.error
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn browser_service_roundtrip_navigate_and_screenshot() {
        let (url, handle) = spawn_static_http_server(
            200,
            "<html><head><title>Integration Title</title></head><body>ok</body></html>",
        );
        let runtime = std::sync::Arc::new(
            BrowserRuntimeState::new(&Args {
                bind: "127.0.0.1".to_owned(),
                port: 7143,
                grpc_bind: "127.0.0.1".to_owned(),
                grpc_port: 7543,
                auth_token: None,
                session_idle_ttl_ms: 60_000,
                max_sessions: 16,
                max_navigation_timeout_ms: 10_000,
                max_session_lifetime_ms: 60_000,
                max_screenshot_bytes: 128 * 1024,
                max_response_bytes: 128 * 1024,
                max_title_bytes: 4 * 1024,
            })
            .expect("runtime should initialize"),
        );
        let service = BrowserServiceImpl { runtime };

        let created = service
            .create_session(Request::new(browser_v1::CreateSessionRequest {
                v: 1,
                principal: "user:ops".to_owned(),
                idle_ttl_ms: 10_000,
                budget: None,
                allow_private_targets: true,
                allow_downloads: false,
                action_allowed_domains: Vec::new(),
            }))
            .await
            .expect("create_session should succeed")
            .into_inner();
        let session_id = created
            .session_id
            .as_ref()
            .map(|value| value.ulid.clone())
            .expect("session id should be present");

        let navigate = service
            .navigate(Request::new(browser_v1::NavigateRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId {
                    ulid: session_id.clone(),
                }),
                url,
                timeout_ms: 2_000,
                allow_redirects: true,
                max_redirects: 3,
                allow_private_targets: true,
            }))
            .await
            .expect("navigate should succeed")
            .into_inner();
        assert!(navigate.success, "navigation should succeed");
        assert_eq!(navigate.title, "Integration Title");

        let screenshot = service
            .screenshot(Request::new(browser_v1::ScreenshotRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId { ulid: session_id }),
                max_bytes: 1024,
                format: "png".to_owned(),
            }))
            .await
            .expect("screenshot should succeed")
            .into_inner();
        assert!(screenshot.success, "screenshot should succeed");
        assert_eq!(screenshot.image_bytes, ONE_BY_ONE_PNG);

        handle.join().expect("test server thread should exit");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn browser_service_click_type_and_wait_for_on_fixture_page() {
        let (url, handle) = spawn_static_http_server(
            200,
            "<html><head><title>Actions</title></head><body><input id=\"email\" name=\"email\" /><button id=\"submit\">Submit</button></body></html>",
        );
        let runtime = std::sync::Arc::new(
            BrowserRuntimeState::new(&Args {
                bind: "127.0.0.1".to_owned(),
                port: 7143,
                grpc_bind: "127.0.0.1".to_owned(),
                grpc_port: 7543,
                auth_token: None,
                session_idle_ttl_ms: 60_000,
                max_sessions: 16,
                max_navigation_timeout_ms: 10_000,
                max_session_lifetime_ms: 60_000,
                max_screenshot_bytes: 128 * 1024,
                max_response_bytes: 128 * 1024,
                max_title_bytes: 4 * 1024,
            })
            .expect("runtime should initialize"),
        );
        let service = BrowserServiceImpl { runtime };
        let created = service
            .create_session(Request::new(browser_v1::CreateSessionRequest {
                v: 1,
                principal: "user:ops".to_owned(),
                idle_ttl_ms: 10_000,
                budget: None,
                allow_private_targets: true,
                allow_downloads: false,
                action_allowed_domains: Vec::new(),
            }))
            .await
            .expect("create_session should succeed")
            .into_inner();
        let session_id = created
            .session_id
            .as_ref()
            .map(|value| value.ulid.clone())
            .expect("session id should be present");

        let navigate = service
            .navigate(Request::new(browser_v1::NavigateRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId {
                    ulid: session_id.clone(),
                }),
                url,
                timeout_ms: 2_000,
                allow_redirects: true,
                max_redirects: 3,
                allow_private_targets: true,
            }))
            .await
            .expect("navigate should succeed")
            .into_inner();
        assert!(navigate.success, "navigation should succeed");

        let click = service
            .click(Request::new(browser_v1::ClickRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId {
                    ulid: session_id.clone(),
                }),
                selector: "#submit".to_owned(),
                max_retries: 2,
                timeout_ms: 500,
                capture_failure_screenshot: true,
                max_failure_screenshot_bytes: 1024,
            }))
            .await
            .expect("click should execute")
            .into_inner();
        assert!(click.success, "click action should succeed");
        assert_eq!(
            click.action_log.as_ref().map(|value| value.action_name.as_str()),
            Some("click")
        );

        let typed = service
            .r#type(Request::new(browser_v1::TypeRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId {
                    ulid: session_id.clone(),
                }),
                selector: "#email".to_owned(),
                text: "agent@example.com".to_owned(),
                clear_existing: true,
                timeout_ms: 500,
                capture_failure_screenshot: true,
                max_failure_screenshot_bytes: 1024,
            }))
            .await
            .expect("type should execute")
            .into_inner();
        assert!(typed.success, "type action should succeed");
        assert_eq!(typed.typed_bytes, "agent@example.com".len() as u64);
        assert_eq!(typed.action_log.as_ref().map(|value| value.action_name.as_str()), Some("type"));

        let waited = service
            .wait_for(Request::new(browser_v1::WaitForRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId { ulid: session_id }),
                selector: "#submit".to_owned(),
                text: String::new(),
                timeout_ms: 300,
                poll_interval_ms: 25,
                capture_failure_screenshot: true,
                max_failure_screenshot_bytes: 1024,
            }))
            .await
            .expect("wait_for should execute")
            .into_inner();
        assert!(waited.success, "wait_for should match existing selector");
        assert_eq!(waited.matched_selector, "#submit");

        handle.join().expect("test server thread should exit");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn browser_service_rejects_oversized_type_input() {
        let (url, handle) = spawn_static_http_server(
            200,
            "<html><body><input id=\"name\" name=\"name\" /></body></html>",
        );
        let runtime = std::sync::Arc::new(
            BrowserRuntimeState::new(&Args {
                bind: "127.0.0.1".to_owned(),
                port: 7143,
                grpc_bind: "127.0.0.1".to_owned(),
                grpc_port: 7543,
                auth_token: None,
                session_idle_ttl_ms: 60_000,
                max_sessions: 16,
                max_navigation_timeout_ms: 10_000,
                max_session_lifetime_ms: 60_000,
                max_screenshot_bytes: 128 * 1024,
                max_response_bytes: 128 * 1024,
                max_title_bytes: 4 * 1024,
            })
            .expect("runtime should initialize"),
        );
        let service = BrowserServiceImpl { runtime };
        let created = service
            .create_session(Request::new(browser_v1::CreateSessionRequest {
                v: 1,
                principal: "user:ops".to_owned(),
                idle_ttl_ms: 10_000,
                budget: Some(browser_v1::SessionBudget {
                    max_navigation_timeout_ms: 0,
                    max_session_lifetime_ms: 0,
                    max_screenshot_bytes: 0,
                    max_response_bytes: 0,
                    max_action_timeout_ms: 0,
                    max_type_input_bytes: 4,
                    max_actions_per_session: 0,
                    max_actions_per_window: 0,
                    action_rate_window_ms: 0,
                    max_action_log_entries: 0,
                    max_observe_snapshot_bytes: 0,
                    max_visible_text_bytes: 0,
                    max_network_log_entries: 0,
                    max_network_log_bytes: 0,
                }),
                allow_private_targets: true,
                allow_downloads: false,
                action_allowed_domains: Vec::new(),
            }))
            .await
            .expect("create_session should succeed")
            .into_inner();
        let session_id = created
            .session_id
            .as_ref()
            .map(|value| value.ulid.clone())
            .expect("session id should be present");
        let navigate = service
            .navigate(Request::new(browser_v1::NavigateRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId {
                    ulid: session_id.clone(),
                }),
                url,
                timeout_ms: 2_000,
                allow_redirects: true,
                max_redirects: 3,
                allow_private_targets: true,
            }))
            .await
            .expect("navigate should succeed")
            .into_inner();
        assert!(navigate.success, "navigation should succeed");

        let typed = service
            .r#type(Request::new(browser_v1::TypeRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId { ulid: session_id }),
                selector: "#name".to_owned(),
                text: "abcdef".to_owned(),
                clear_existing: false,
                timeout_ms: 500,
                capture_failure_screenshot: true,
                max_failure_screenshot_bytes: 1024,
            }))
            .await
            .expect("type request should complete")
            .into_inner();
        assert!(!typed.success, "oversized type payload should fail");
        assert!(
            typed.error.contains("max_type_input_bytes"),
            "error should contain explicit budget context: {}",
            typed.error
        );

        handle.join().expect("test server thread should exit");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn browser_service_blocks_download_click_when_disabled() {
        let (url, handle) = spawn_static_http_server(
            200,
            "<html><body><a id=\"download-link\" href=\"/report.csv\" download>Download</a></body></html>",
        );
        let runtime = std::sync::Arc::new(
            BrowserRuntimeState::new(&Args {
                bind: "127.0.0.1".to_owned(),
                port: 7143,
                grpc_bind: "127.0.0.1".to_owned(),
                grpc_port: 7543,
                auth_token: None,
                session_idle_ttl_ms: 60_000,
                max_sessions: 16,
                max_navigation_timeout_ms: 10_000,
                max_session_lifetime_ms: 60_000,
                max_screenshot_bytes: 128 * 1024,
                max_response_bytes: 128 * 1024,
                max_title_bytes: 4 * 1024,
            })
            .expect("runtime should initialize"),
        );
        let service = BrowserServiceImpl { runtime };
        let created = service
            .create_session(Request::new(browser_v1::CreateSessionRequest {
                v: 1,
                principal: "user:ops".to_owned(),
                idle_ttl_ms: 10_000,
                budget: None,
                allow_private_targets: true,
                allow_downloads: false,
                action_allowed_domains: Vec::new(),
            }))
            .await
            .expect("create_session should succeed")
            .into_inner();
        let session_id = created
            .session_id
            .as_ref()
            .map(|value| value.ulid.clone())
            .expect("session id should be present");
        let navigate = service
            .navigate(Request::new(browser_v1::NavigateRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId {
                    ulid: session_id.clone(),
                }),
                url,
                timeout_ms: 2_000,
                allow_redirects: true,
                max_redirects: 3,
                allow_private_targets: true,
            }))
            .await
            .expect("navigate should succeed")
            .into_inner();
        assert!(navigate.success, "navigation should succeed");

        let click = service
            .click(Request::new(browser_v1::ClickRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId { ulid: session_id }),
                selector: "#download-link".to_owned(),
                max_retries: 0,
                timeout_ms: 500,
                capture_failure_screenshot: true,
                max_failure_screenshot_bytes: 1024,
            }))
            .await
            .expect("click request should complete")
            .into_inner();
        assert!(!click.success, "download-like click should be blocked by default");
        assert!(
            click.error.contains("allow_downloads=false"),
            "error should identify explicit download policy: {}",
            click.error
        );
        assert_eq!(
            click.failure_screenshot_bytes, ONE_BY_ONE_PNG,
            "blocked click should include bounded failure screenshot"
        );

        handle.join().expect("test server thread should exit");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn browser_service_observe_returns_stable_sanitized_snapshot() {
        let (url, handle) = spawn_static_http_server(
            200,
            "<html><head><title>Observe Fixture</title></head><body><main><h1>Portal</h1><form id=\"login\" action=\"/submit?token=secret&safe=ok\"><input id=\"email\" name=\"email\" type=\"email\" /><button id=\"send\" aria-label=\"Send\">Send</button></form><a id=\"docs\" href=\"https://example.com/docs?access_token=secret&lang=en\">Docs</a><script>window.token='abc'</script></main></body></html>",
        );
        let runtime = std::sync::Arc::new(
            BrowserRuntimeState::new(&Args {
                bind: "127.0.0.1".to_owned(),
                port: 7143,
                grpc_bind: "127.0.0.1".to_owned(),
                grpc_port: 7543,
                auth_token: None,
                session_idle_ttl_ms: 60_000,
                max_sessions: 16,
                max_navigation_timeout_ms: 10_000,
                max_session_lifetime_ms: 60_000,
                max_screenshot_bytes: 128 * 1024,
                max_response_bytes: 128 * 1024,
                max_title_bytes: 4 * 1024,
            })
            .expect("runtime should initialize"),
        );
        let service = BrowserServiceImpl { runtime };
        let created = service
            .create_session(Request::new(browser_v1::CreateSessionRequest {
                v: 1,
                principal: "user:ops".to_owned(),
                idle_ttl_ms: 10_000,
                budget: None,
                allow_private_targets: true,
                allow_downloads: false,
                action_allowed_domains: Vec::new(),
            }))
            .await
            .expect("create_session should succeed")
            .into_inner();
        let session_id = created
            .session_id
            .as_ref()
            .map(|value| value.ulid.clone())
            .expect("session id should be present");
        let navigate = service
            .navigate(Request::new(browser_v1::NavigateRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId {
                    ulid: session_id.clone(),
                }),
                url: format!("{url}?access_token=topsecret&lang=en"),
                timeout_ms: 2_000,
                allow_redirects: true,
                max_redirects: 3,
                allow_private_targets: true,
            }))
            .await
            .expect("navigate should succeed")
            .into_inner();
        assert!(navigate.success, "navigation should succeed");

        let observed = service
            .observe(Request::new(browser_v1::ObserveRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId { ulid: session_id }),
                include_dom_snapshot: true,
                include_accessibility_tree: true,
                include_visible_text: true,
                max_dom_snapshot_bytes: 8 * 1024,
                max_accessibility_tree_bytes: 8 * 1024,
                max_visible_text_bytes: 2 * 1024,
            }))
            .await
            .expect("observe should execute")
            .into_inner();
        assert!(observed.success, "observe should succeed");
        assert!(
            observed.dom_snapshot.contains("<form"),
            "dom snapshot should include structural elements"
        );
        assert!(
            observed.dom_snapshot.contains("token=<redacted>")
                || observed.dom_snapshot.contains("access_token=<redacted>"),
            "dom snapshot should redact sensitive URL query params: {}",
            observed.dom_snapshot
        );
        assert!(
            !observed.dom_snapshot.contains("topsecret"),
            "sensitive query values must be redacted from dom snapshot: {}",
            observed.dom_snapshot
        );
        assert!(
            observed.accessibility_tree.contains("role=button"),
            "accessibility tree should include semantic roles: {}",
            observed.accessibility_tree
        );
        assert!(
            observed.visible_text.contains("Portal"),
            "visible text extraction should include visible text content"
        );
        assert!(
            observed.page_url.contains("access_token=<redacted>"),
            "observed page URL should be redacted: {}",
            observed.page_url
        );

        handle.join().expect("test server thread should exit");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn browser_service_observe_truncates_deterministically_when_oversized() {
        let large_body = format!(
            "<html><body><main>{}</main></body></html>",
            (0..80)
                .map(|index| format!("<section id=\"section-{index}\"><button id=\"btn-{index}\">Run {index}</button></section>"))
                .collect::<String>()
        );
        let (url, handle) = spawn_static_http_server(200, large_body.as_str());
        let runtime = std::sync::Arc::new(
            BrowserRuntimeState::new(&Args {
                bind: "127.0.0.1".to_owned(),
                port: 7143,
                grpc_bind: "127.0.0.1".to_owned(),
                grpc_port: 7543,
                auth_token: None,
                session_idle_ttl_ms: 60_000,
                max_sessions: 16,
                max_navigation_timeout_ms: 10_000,
                max_session_lifetime_ms: 60_000,
                max_screenshot_bytes: 128 * 1024,
                max_response_bytes: 256 * 1024,
                max_title_bytes: 4 * 1024,
            })
            .expect("runtime should initialize"),
        );
        let service = BrowserServiceImpl { runtime };
        let created = service
            .create_session(Request::new(browser_v1::CreateSessionRequest {
                v: 1,
                principal: "user:ops".to_owned(),
                idle_ttl_ms: 10_000,
                budget: None,
                allow_private_targets: true,
                allow_downloads: false,
                action_allowed_domains: Vec::new(),
            }))
            .await
            .expect("create_session should succeed")
            .into_inner();
        let session_id = created
            .session_id
            .as_ref()
            .map(|value| value.ulid.clone())
            .expect("session id should be present");
        let navigate = service
            .navigate(Request::new(browser_v1::NavigateRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId {
                    ulid: session_id.clone(),
                }),
                url,
                timeout_ms: 2_000,
                allow_redirects: true,
                max_redirects: 3,
                allow_private_targets: true,
            }))
            .await
            .expect("navigate should succeed")
            .into_inner();
        assert!(navigate.success, "navigation should succeed");

        let request = browser_v1::ObserveRequest {
            v: 1,
            session_id: Some(proto::palyra::common::v1::CanonicalId { ulid: session_id }),
            include_dom_snapshot: true,
            include_accessibility_tree: true,
            include_visible_text: true,
            max_dom_snapshot_bytes: 64,
            max_accessibility_tree_bytes: 64,
            max_visible_text_bytes: 48,
        };
        let first = service
            .observe(Request::new(request.clone()))
            .await
            .expect("first observe should execute")
            .into_inner();
        let second = service
            .observe(Request::new(request))
            .await
            .expect("second observe should execute")
            .into_inner();
        assert!(
            first.dom_truncated
                && first.accessibility_tree_truncated
                && first.visible_text_truncated,
            "all observe channels should report truncation for oversized snapshots"
        );
        assert_eq!(first.dom_snapshot, second.dom_snapshot, "dom truncation must be deterministic");
        assert_eq!(
            first.accessibility_tree, second.accessibility_tree,
            "a11y truncation must be deterministic"
        );
        assert_eq!(
            first.visible_text, second.visible_text,
            "visible text truncation must be deterministic"
        );

        handle.join().expect("test server thread should exit");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn browser_service_network_log_redacts_sensitive_values() {
        let (url, handle) = spawn_static_http_server_with_headers(
            200,
            "<html><body>network log fixture</body></html>",
            &[
                ("Set-Cookie", "session=abc123; HttpOnly"),
                ("X-Api-Key", "secret-key"),
                ("Location", "https://example.com/redirect?token=secret&safe=1"),
            ],
        );
        let runtime = std::sync::Arc::new(
            BrowserRuntimeState::new(&Args {
                bind: "127.0.0.1".to_owned(),
                port: 7143,
                grpc_bind: "127.0.0.1".to_owned(),
                grpc_port: 7543,
                auth_token: None,
                session_idle_ttl_ms: 60_000,
                max_sessions: 16,
                max_navigation_timeout_ms: 10_000,
                max_session_lifetime_ms: 60_000,
                max_screenshot_bytes: 128 * 1024,
                max_response_bytes: 128 * 1024,
                max_title_bytes: 4 * 1024,
            })
            .expect("runtime should initialize"),
        );
        let service = BrowserServiceImpl { runtime };
        let created = service
            .create_session(Request::new(browser_v1::CreateSessionRequest {
                v: 1,
                principal: "user:ops".to_owned(),
                idle_ttl_ms: 10_000,
                budget: None,
                allow_private_targets: true,
                allow_downloads: false,
                action_allowed_domains: Vec::new(),
            }))
            .await
            .expect("create_session should succeed")
            .into_inner();
        let session_id = created
            .session_id
            .as_ref()
            .map(|value| value.ulid.clone())
            .expect("session id should be present");
        let navigate = service
            .navigate(Request::new(browser_v1::NavigateRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId {
                    ulid: session_id.clone(),
                }),
                url: format!("{url}?access_token=supersecret&safe=1"),
                timeout_ms: 2_000,
                allow_redirects: true,
                max_redirects: 3,
                allow_private_targets: true,
            }))
            .await
            .expect("navigate should succeed")
            .into_inner();
        assert!(navigate.success, "navigation should succeed");

        let without_headers = service
            .network_log(Request::new(browser_v1::NetworkLogRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId {
                    ulid: session_id.clone(),
                }),
                limit: 10,
                include_headers: false,
                max_payload_bytes: 8 * 1024,
            }))
            .await
            .expect("network_log without headers should execute")
            .into_inner();
        assert!(without_headers.success, "network log call should succeed");
        assert!(!without_headers.entries.is_empty(), "network log should contain entries");
        assert!(
            without_headers.entries.iter().all(|entry| entry.headers.is_empty()),
            "headers must be excluded unless explicitly requested"
        );

        let with_headers = service
            .network_log(Request::new(browser_v1::NetworkLogRequest {
                v: 1,
                session_id: Some(proto::palyra::common::v1::CanonicalId { ulid: session_id }),
                limit: 10,
                include_headers: true,
                max_payload_bytes: 8 * 1024,
            }))
            .await
            .expect("network_log with headers should execute")
            .into_inner();
        assert!(with_headers.success, "network log call should succeed");
        let entry =
            with_headers.entries.last().expect("network log should include at least one entry");
        assert!(
            entry.request_url.contains("access_token=<redacted>"),
            "network log URLs should redact sensitive query values: {}",
            entry.request_url
        );
        assert!(
            !entry.request_url.contains("supersecret"),
            "network log must not leak original sensitive URL values: {}",
            entry.request_url
        );
        assert!(
            entry
                .headers
                .iter()
                .any(|header| { header.name == "set-cookie" && header.value == "<redacted>" }),
            "set-cookie header should be redacted"
        );
        assert!(
            entry.headers.iter().any(|header| {
                header.name == "location" && header.value.contains("token=<redacted>")
            }),
            "location header URLs should be normalized and redacted"
        );

        handle.join().expect("test server thread should exit");
    }

    fn spawn_static_http_server(status_code: u16, body: &str) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = listener.local_addr().expect("listener local address should resolve");
        let body = body.to_owned();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("listener should accept request");
            read_http_request(&mut stream);
            let response = format!(
                "HTTP/1.1 {status_code} OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).expect("server should write response");
            stream.flush().expect("server should flush response");
        });
        (format!("http://{address}/"), handle)
    }

    fn spawn_static_http_server_with_headers(
        status_code: u16,
        body: &str,
        headers: &[(&str, &str)],
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = listener.local_addr().expect("listener local address should resolve");
        let body = body.to_owned();
        let headers = headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<Vec<_>>();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("listener should accept request");
            read_http_request(&mut stream);
            let mut response = format!(
                "HTTP/1.1 {status_code} OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n",
                body.len()
            );
            for (name, value) in headers {
                response.push_str(format!("{name}: {value}\r\n").as_str());
            }
            response.push_str("Connection: close\r\n\r\n");
            response.push_str(body.as_str());
            stream.write_all(response.as_bytes()).expect("server should write response");
            stream.flush().expect("server should flush response");
        });
        (format!("http://{address}/"), handle)
    }

    fn read_http_request(stream: &mut TcpStream) {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout should be configured");
        let mut buffer = [0_u8; 1024];
        let _ = stream.read(&mut buffer);
    }
}
