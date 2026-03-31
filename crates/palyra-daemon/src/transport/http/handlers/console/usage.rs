use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::http::{header::CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::diagnostics::{build_page_info, contract_descriptor};
use crate::agents::{AgentBindingQuery, AgentRecord, SessionAgentBinding};
use crate::journal::{self, OrchestratorUsageQuery};
use crate::*;

const DEFAULT_USAGE_LOOKBACK_MS: i64 = 30 * 24 * 60 * 60 * 1_000;
const MAX_USAGE_LOOKBACK_MS: i64 = 366 * 24 * 60 * 60 * 1_000;
const HOUR_BUCKET_MS: i64 = 60 * 60 * 1_000;
const DAY_BUCKET_MS: i64 = 24 * 60 * 60 * 1_000;
const DEFAULT_USAGE_BREAKDOWN_LIMIT: usize = 10;
const MAX_USAGE_BREAKDOWN_LIMIT: usize = 50;
const DEFAULT_USAGE_RUN_LIMIT: usize = 12;
const MAX_USAGE_RUN_LIMIT: usize = 25;

#[derive(Debug, Deserialize)]
pub(crate) struct ConsoleUsageSummaryQuery {
    #[serde(default)]
    start_at_unix_ms: Option<i64>,
    #[serde(default)]
    end_at_unix_ms: Option<i64>,
    #[serde(default)]
    bucket: Option<String>,
    #[serde(default)]
    include_archived: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConsoleUsageBreakdownQuery {
    #[serde(default)]
    start_at_unix_ms: Option<i64>,
    #[serde(default)]
    end_at_unix_ms: Option<i64>,
    #[serde(default)]
    bucket: Option<String>,
    #[serde(default)]
    include_archived: Option<bool>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConsoleUsageSessionDetailQuery {
    #[serde(default)]
    start_at_unix_ms: Option<i64>,
    #[serde(default)]
    end_at_unix_ms: Option<i64>,
    #[serde(default)]
    bucket: Option<String>,
    #[serde(default)]
    include_archived: Option<bool>,
    #[serde(default)]
    run_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConsoleUsageExportQuery {
    dataset: String,
    format: String,
    #[serde(default)]
    start_at_unix_ms: Option<i64>,
    #[serde(default)]
    end_at_unix_ms: Option<i64>,
    #[serde(default)]
    bucket: Option<String>,
    #[serde(default)]
    include_archived: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
struct UsageQueryEcho {
    start_at_unix_ms: i64,
    end_at_unix_ms: i64,
    bucket: String,
    bucket_width_ms: i64,
    include_archived: bool,
}

#[derive(Debug, Clone, Serialize)]
struct UsagePaginationQueryEcho {
    start_at_unix_ms: i64,
    end_at_unix_ms: i64,
    bucket: String,
    bucket_width_ms: i64,
    include_archived: bool,
    limit: usize,
    cursor: usize,
}

#[derive(Debug, Clone, Serialize)]
struct UsageSessionDetailQueryEcho {
    start_at_unix_ms: i64,
    end_at_unix_ms: i64,
    bucket: String,
    bucket_width_ms: i64,
    include_archived: bool,
    run_limit: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct UsageSummaryEnvelope {
    contract: control_plane::ContractDescriptor,
    query: UsageQueryEcho,
    totals: journal::OrchestratorUsageTotals,
    timeline: Vec<journal::OrchestratorUsageTimelineBucket>,
    cost_tracking_available: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct UsageSessionsEnvelope {
    contract: control_plane::ContractDescriptor,
    query: UsagePaginationQueryEcho,
    sessions: Vec<journal::OrchestratorUsageSessionRecord>,
    page: control_plane::PageInfo,
    cost_tracking_available: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct UsageSessionDetailEnvelope {
    contract: control_plane::ContractDescriptor,
    query: UsageSessionDetailQueryEcho,
    session: journal::OrchestratorUsageSessionRecord,
    totals: journal::OrchestratorUsageTotals,
    timeline: Vec<journal::OrchestratorUsageTimelineBucket>,
    runs: Vec<journal::OrchestratorUsageRunRecord>,
    cost_tracking_available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UsageAgentRecord {
    agent_id: String,
    display_name: String,
    binding_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_model_profile: Option<String>,
    session_count: u64,
    runs: u64,
    active_runs: u64,
    completed_runs: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    average_latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_started_at_unix_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    estimated_cost_usd: Option<f64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UsageAgentsEnvelope {
    contract: control_plane::ContractDescriptor,
    query: UsagePaginationQueryEcho,
    agents: Vec<UsageAgentRecord>,
    page: control_plane::PageInfo,
    cost_tracking_available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UsageModelRecord {
    model_id: String,
    display_name: String,
    model_source: String,
    agent_count: u64,
    session_count: u64,
    runs: u64,
    active_runs: u64,
    completed_runs: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    average_latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_started_at_unix_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    estimated_cost_usd: Option<f64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UsageModelsEnvelope {
    contract: control_plane::ContractDescriptor,
    query: UsagePaginationQueryEcho,
    models: Vec<UsageModelRecord>,
    page: control_plane::PageInfo,
    cost_tracking_available: bool,
}

pub(crate) async fn console_usage_summary_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConsoleUsageSummaryQuery>,
) -> Result<Json<UsageSummaryEnvelope>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let resolved = resolve_usage_query(
        query.start_at_unix_ms,
        query.end_at_unix_ms,
        query.bucket.as_deref(),
        query.include_archived.unwrap_or(false),
        &session.context,
        None,
    )?;
    let summary = state
        .runtime
        .summarize_orchestrator_usage(resolved.query.clone())
        .await
        .map_err(runtime_status_response)?;
    Ok(Json(UsageSummaryEnvelope {
        contract: contract_descriptor(),
        query: resolved.echo,
        totals: summary.totals,
        timeline: summary.timeline,
        cost_tracking_available: summary.cost_tracking_available,
    }))
}

pub(crate) async fn console_usage_sessions_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConsoleUsageBreakdownQuery>,
) -> Result<Json<UsageSessionsEnvelope>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let limit =
        query.limit.unwrap_or(DEFAULT_USAGE_BREAKDOWN_LIMIT).clamp(1, MAX_USAGE_BREAKDOWN_LIMIT);
    let cursor = parse_usage_cursor(query.cursor.as_deref())?;
    let resolved = resolve_usage_query(
        query.start_at_unix_ms,
        query.end_at_unix_ms,
        query.bucket.as_deref(),
        query.include_archived.unwrap_or(false),
        &session.context,
        None,
    )?;
    let sessions = state
        .runtime
        .list_orchestrator_usage_sessions(resolved.query.clone())
        .await
        .map_err(runtime_status_response)?;
    let next_cursor =
        (cursor.saturating_add(limit) < sessions.len()).then(|| (cursor + limit).to_string());
    let page =
        build_page_info(limit, sessions.len().saturating_sub(cursor).min(limit), next_cursor);
    let sessions = sessions.into_iter().skip(cursor).take(limit).collect::<Vec<_>>();

    Ok(Json(UsageSessionsEnvelope {
        contract: contract_descriptor(),
        query: UsagePaginationQueryEcho {
            start_at_unix_ms: resolved.echo.start_at_unix_ms,
            end_at_unix_ms: resolved.echo.end_at_unix_ms,
            bucket: resolved.echo.bucket,
            bucket_width_ms: resolved.echo.bucket_width_ms,
            include_archived: resolved.echo.include_archived,
            limit,
            cursor,
        },
        sessions,
        page,
        cost_tracking_available: false,
    }))
}

pub(crate) async fn console_usage_session_detail_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Query(query): Query<ConsoleUsageSessionDetailQuery>,
) -> Result<Json<UsageSessionDetailEnvelope>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    validate_canonical_id(session_id.as_str()).map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument(
            "session_id must be a canonical ULID",
        ))
    })?;
    let run_limit =
        query.run_limit.unwrap_or(DEFAULT_USAGE_RUN_LIMIT).clamp(1, MAX_USAGE_RUN_LIMIT);
    let resolved = resolve_usage_query(
        query.start_at_unix_ms,
        query.end_at_unix_ms,
        query.bucket.as_deref(),
        query.include_archived.unwrap_or(false),
        &session.context,
        Some(session_id.clone()),
    )?;
    let summary = state
        .runtime
        .summarize_orchestrator_usage(resolved.query.clone())
        .await
        .map_err(runtime_status_response)?;
    let detail = state
        .runtime
        .get_orchestrator_usage_session(resolved.query.clone(), session_id, run_limit)
        .await
        .map_err(runtime_status_response)?
        .ok_or_else(|| {
            runtime_status_response(tonic::Status::not_found("session was not found"))
        })?;

    Ok(Json(UsageSessionDetailEnvelope {
        contract: contract_descriptor(),
        query: UsageSessionDetailQueryEcho {
            start_at_unix_ms: resolved.echo.start_at_unix_ms,
            end_at_unix_ms: resolved.echo.end_at_unix_ms,
            bucket: resolved.echo.bucket,
            bucket_width_ms: resolved.echo.bucket_width_ms,
            include_archived: resolved.echo.include_archived,
            run_limit,
        },
        session: detail.0,
        totals: summary.totals,
        timeline: summary.timeline,
        runs: detail.1,
        cost_tracking_available: summary.cost_tracking_available,
    }))
}

pub(crate) async fn console_usage_agents_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConsoleUsageBreakdownQuery>,
) -> Result<Json<UsageAgentsEnvelope>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let limit =
        query.limit.unwrap_or(DEFAULT_USAGE_BREAKDOWN_LIMIT).clamp(1, MAX_USAGE_BREAKDOWN_LIMIT);
    let cursor = parse_usage_cursor(query.cursor.as_deref())?;
    let resolved = resolve_usage_query(
        query.start_at_unix_ms,
        query.end_at_unix_ms,
        query.bucket.as_deref(),
        query.include_archived.unwrap_or(false),
        &session.context,
        None,
    )?;
    let sessions = state
        .runtime
        .list_orchestrator_usage_sessions(resolved.query.clone())
        .await
        .map_err(runtime_status_response)?;
    let usage_metadata = load_usage_metadata(&state, &session.context).await?;
    let rows = build_usage_agent_rows(sessions.as_slice(), &usage_metadata);
    let next_cursor =
        (cursor.saturating_add(limit) < rows.len()).then(|| (cursor + limit).to_string());
    let page = build_page_info(limit, rows.len().saturating_sub(cursor).min(limit), next_cursor);
    let agents = rows.into_iter().skip(cursor).take(limit).collect::<Vec<_>>();

    Ok(Json(UsageAgentsEnvelope {
        contract: contract_descriptor(),
        query: UsagePaginationQueryEcho {
            start_at_unix_ms: resolved.echo.start_at_unix_ms,
            end_at_unix_ms: resolved.echo.end_at_unix_ms,
            bucket: resolved.echo.bucket,
            bucket_width_ms: resolved.echo.bucket_width_ms,
            include_archived: resolved.echo.include_archived,
            limit,
            cursor,
        },
        agents,
        page,
        cost_tracking_available: false,
    }))
}

pub(crate) async fn console_usage_models_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConsoleUsageBreakdownQuery>,
) -> Result<Json<UsageModelsEnvelope>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let limit =
        query.limit.unwrap_or(DEFAULT_USAGE_BREAKDOWN_LIMIT).clamp(1, MAX_USAGE_BREAKDOWN_LIMIT);
    let cursor = parse_usage_cursor(query.cursor.as_deref())?;
    let resolved = resolve_usage_query(
        query.start_at_unix_ms,
        query.end_at_unix_ms,
        query.bucket.as_deref(),
        query.include_archived.unwrap_or(false),
        &session.context,
        None,
    )?;
    let sessions = state
        .runtime
        .list_orchestrator_usage_sessions(resolved.query.clone())
        .await
        .map_err(runtime_status_response)?;
    let usage_metadata = load_usage_metadata(&state, &session.context).await?;
    let rows = build_usage_model_rows(sessions.as_slice(), &usage_metadata);
    let next_cursor =
        (cursor.saturating_add(limit) < rows.len()).then(|| (cursor + limit).to_string());
    let page = build_page_info(limit, rows.len().saturating_sub(cursor).min(limit), next_cursor);
    let models = rows.into_iter().skip(cursor).take(limit).collect::<Vec<_>>();

    Ok(Json(UsageModelsEnvelope {
        contract: contract_descriptor(),
        query: UsagePaginationQueryEcho {
            start_at_unix_ms: resolved.echo.start_at_unix_ms,
            end_at_unix_ms: resolved.echo.end_at_unix_ms,
            bucket: resolved.echo.bucket,
            bucket_width_ms: resolved.echo.bucket_width_ms,
            include_archived: resolved.echo.include_archived,
            limit,
            cursor,
        },
        models,
        page,
        cost_tracking_available: false,
    }))
}

pub(crate) async fn console_usage_export_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConsoleUsageExportQuery>,
) -> Result<Response, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let resolved = resolve_usage_query(
        query.start_at_unix_ms,
        query.end_at_unix_ms,
        query.bucket.as_deref(),
        query.include_archived.unwrap_or(false),
        &session.context,
        None,
    )?;
    let dataset = normalize_usage_export_dataset(query.dataset.as_str())?;
    let format = normalize_usage_export_format(query.format.as_str())?;

    let response = match dataset {
        UsageExportDataset::Timeline => {
            let summary = state
                .runtime
                .summarize_orchestrator_usage(resolved.query.clone())
                .await
                .map_err(runtime_status_response)?;
            match format {
                UsageExportFormat::Json => usage_json_export_response(
                    "timeline",
                    json!({
                        "contract": contract_descriptor(),
                        "query": resolved.echo,
                        "rows": summary.timeline,
                        "cost_tracking_available": summary.cost_tracking_available,
                    }),
                )?,
                UsageExportFormat::Csv => usage_csv_export_response(
                    "timeline",
                    build_timeline_csv(summary.timeline.as_slice()),
                )?,
            }
        }
        UsageExportDataset::Sessions => {
            let sessions = state
                .runtime
                .list_orchestrator_usage_sessions(resolved.query.clone())
                .await
                .map_err(runtime_status_response)?;
            match format {
                UsageExportFormat::Json => usage_json_export_response(
                    "sessions",
                    json!({
                        "contract": contract_descriptor(),
                        "query": resolved.echo,
                        "rows": sessions,
                        "cost_tracking_available": false,
                    }),
                )?,
                UsageExportFormat::Csv => {
                    usage_csv_export_response("sessions", build_sessions_csv(sessions.as_slice()))?
                }
            }
        }
        UsageExportDataset::Agents => {
            let sessions = state
                .runtime
                .list_orchestrator_usage_sessions(resolved.query.clone())
                .await
                .map_err(runtime_status_response)?;
            let usage_metadata = load_usage_metadata(&state, &session.context).await?;
            let agents = build_usage_agent_rows(sessions.as_slice(), &usage_metadata);
            match format {
                UsageExportFormat::Json => usage_json_export_response(
                    "agents",
                    json!({
                        "contract": contract_descriptor(),
                        "query": resolved.echo,
                        "rows": agents,
                        "cost_tracking_available": false,
                    }),
                )?,
                UsageExportFormat::Csv => {
                    usage_csv_export_response("agents", build_agents_csv(agents.as_slice()))?
                }
            }
        }
        UsageExportDataset::Models => {
            let sessions = state
                .runtime
                .list_orchestrator_usage_sessions(resolved.query.clone())
                .await
                .map_err(runtime_status_response)?;
            let usage_metadata = load_usage_metadata(&state, &session.context).await?;
            let models = build_usage_model_rows(sessions.as_slice(), &usage_metadata);
            match format {
                UsageExportFormat::Json => usage_json_export_response(
                    "models",
                    json!({
                        "contract": contract_descriptor(),
                        "query": resolved.echo,
                        "rows": models,
                        "cost_tracking_available": false,
                    }),
                )?,
                UsageExportFormat::Csv => {
                    usage_csv_export_response("models", build_models_csv(models.as_slice()))?
                }
            }
        }
    };

    Ok(response)
}

#[derive(Debug, Clone)]
struct ResolvedUsageQuery {
    query: OrchestratorUsageQuery,
    echo: UsageQueryEcho,
}

#[derive(Debug, Clone)]
struct UsageMetadata {
    bindings_by_session: HashMap<String, SessionAgentBinding>,
    agents_by_id: HashMap<String, AgentRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UsageExportDataset {
    Timeline,
    Sessions,
    Agents,
    Models,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UsageExportFormat {
    Json,
    Csv,
}

#[allow(clippy::result_large_err)]
fn resolve_usage_query(
    start_at_unix_ms: Option<i64>,
    end_at_unix_ms: Option<i64>,
    bucket: Option<&str>,
    include_archived: bool,
    context: &gateway::RequestContext,
    session_id: Option<String>,
) -> Result<ResolvedUsageQuery, Response> {
    let now = current_unix_ms().map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to resolve current time for usage query: {error}"
        )))
    })?;
    let end_at_unix_ms = end_at_unix_ms.unwrap_or(now);
    let start_at_unix_ms =
        start_at_unix_ms.unwrap_or(end_at_unix_ms.saturating_sub(DEFAULT_USAGE_LOOKBACK_MS));
    if end_at_unix_ms <= start_at_unix_ms {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "end_at_unix_ms must be greater than start_at_unix_ms",
        )));
    }
    if end_at_unix_ms.saturating_sub(start_at_unix_ms) > MAX_USAGE_LOOKBACK_MS {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "usage query lookback exceeds the maximum supported range",
        )));
    }
    let (bucket_label, bucket_width_ms) =
        normalize_usage_bucket(bucket, start_at_unix_ms, end_at_unix_ms)?;
    Ok(ResolvedUsageQuery {
        query: OrchestratorUsageQuery {
            start_at_unix_ms,
            end_at_unix_ms,
            bucket_width_ms,
            principal: context.principal.clone(),
            device_id: context.device_id.clone(),
            channel: context.channel.clone(),
            include_archived,
            session_id,
        },
        echo: UsageQueryEcho {
            start_at_unix_ms,
            end_at_unix_ms,
            bucket: bucket_label.to_owned(),
            bucket_width_ms,
            include_archived,
        },
    })
}

#[allow(clippy::result_large_err)]
fn normalize_usage_bucket(
    raw: Option<&str>,
    start_at_unix_ms: i64,
    end_at_unix_ms: i64,
) -> Result<(&'static str, i64), Response> {
    let lookback = end_at_unix_ms.saturating_sub(start_at_unix_ms);
    match raw.unwrap_or("auto").trim() {
        "" | "auto" => {
            if lookback <= 72 * HOUR_BUCKET_MS {
                Ok(("hour", HOUR_BUCKET_MS))
            } else {
                Ok(("day", DAY_BUCKET_MS))
            }
        }
        "hour" => Ok(("hour", HOUR_BUCKET_MS)),
        "day" => Ok(("day", DAY_BUCKET_MS)),
        _ => Err(runtime_status_response(tonic::Status::invalid_argument(
            "bucket must be one of auto|hour|day",
        ))),
    }
}

#[allow(clippy::result_large_err)]
fn normalize_usage_export_dataset(raw: &str) -> Result<UsageExportDataset, Response> {
    match raw.trim() {
        "timeline" => Ok(UsageExportDataset::Timeline),
        "sessions" => Ok(UsageExportDataset::Sessions),
        "agents" => Ok(UsageExportDataset::Agents),
        "models" => Ok(UsageExportDataset::Models),
        _ => Err(runtime_status_response(tonic::Status::invalid_argument(
            "dataset must be one of timeline|sessions|agents|models",
        ))),
    }
}

#[allow(clippy::result_large_err)]
fn normalize_usage_export_format(raw: &str) -> Result<UsageExportFormat, Response> {
    match raw.trim() {
        "json" => Ok(UsageExportFormat::Json),
        "csv" => Ok(UsageExportFormat::Csv),
        _ => Err(runtime_status_response(tonic::Status::invalid_argument(
            "format must be one of json|csv",
        ))),
    }
}

#[allow(clippy::result_large_err)]
fn parse_usage_cursor(raw: Option<&str>) -> Result<usize, Response> {
    let Some(raw) = raw.map(str::trim) else {
        return Ok(0);
    };
    if raw.is_empty() {
        return Ok(0);
    }
    raw.parse::<usize>().map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument(
            "cursor must be an unsigned integer offset",
        ))
    })
}

async fn load_usage_metadata(
    state: &AppState,
    context: &gateway::RequestContext,
) -> Result<UsageMetadata, Response> {
    let bindings = state
        .runtime
        .list_agent_bindings(AgentBindingQuery {
            agent_id: None,
            principal: Some(context.principal.clone()),
            channel: context.channel.clone(),
            session_id: None,
            limit: Some(1_000),
        })
        .await
        .map_err(runtime_status_response)?;
    let mut agents = Vec::new();
    let mut after_agent_id = None::<String>;
    loop {
        let page = state
            .runtime
            .list_agents(after_agent_id.clone(), Some(100))
            .await
            .map_err(runtime_status_response)?;
        agents.extend(page.agents);
        let Some(next_after) = page.next_after_agent_id else {
            break;
        };
        after_agent_id = Some(next_after);
    }

    Ok(UsageMetadata {
        bindings_by_session: bindings
            .into_iter()
            .map(|binding| (binding.session_id.clone(), binding))
            .collect(),
        agents_by_id: agents.into_iter().map(|agent| (agent.agent_id.clone(), agent)).collect(),
    })
}

fn build_usage_agent_rows(
    sessions: &[journal::OrchestratorUsageSessionRecord],
    metadata: &UsageMetadata,
) -> Vec<UsageAgentRecord> {
    let mut aggregates = HashMap::<String, UsageAgentAccumulator>::new();
    for session in sessions {
        let (agent_id, display_name, binding_source, default_model_profile) =
            resolve_usage_agent_identity(session, metadata);
        let entry = aggregates.entry(agent_id.clone()).or_insert_with(|| UsageAgentAccumulator {
            record: UsageAgentRecord {
                agent_id,
                display_name,
                binding_source,
                default_model_profile,
                session_count: 0,
                runs: 0,
                active_runs: 0,
                completed_runs: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                average_latency_ms: None,
                latest_started_at_unix_ms: None,
                estimated_cost_usd: None,
            },
            latency_weighted_total_ms: 0,
        });
        entry.record.session_count += 1;
        entry.record.runs += session.runs;
        entry.record.active_runs += session.active_runs;
        entry.record.completed_runs += session.completed_runs;
        entry.record.prompt_tokens += session.prompt_tokens;
        entry.record.completion_tokens += session.completion_tokens;
        entry.record.total_tokens += session.total_tokens;
        entry.record.latest_started_at_unix_ms = latest_unix_ms(
            entry.record.latest_started_at_unix_ms,
            session.latest_started_at_unix_ms,
        );
        if let Some(average_latency_ms) = session.average_latency_ms {
            entry.latency_weighted_total_ms +=
                u128::from(average_latency_ms) * u128::from(session.completed_runs);
        }
    }

    let mut rows = aggregates.into_values().map(finalize_agent_accumulator).collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .total_tokens
            .cmp(&left.total_tokens)
            .then_with(|| right.runs.cmp(&left.runs))
            .then_with(|| left.agent_id.cmp(&right.agent_id))
    });
    rows
}

fn build_usage_model_rows(
    sessions: &[journal::OrchestratorUsageSessionRecord],
    metadata: &UsageMetadata,
) -> Vec<UsageModelRecord> {
    let mut aggregates = HashMap::<String, UsageModelAccumulator>::new();
    for session in sessions {
        let (model_id, display_name, model_source, agent_id) =
            resolve_usage_model_identity(session, metadata);
        let entry = aggregates.entry(model_id.clone()).or_insert_with(|| UsageModelAccumulator {
            record: UsageModelRecord {
                model_id,
                display_name,
                model_source,
                agent_count: 0,
                session_count: 0,
                runs: 0,
                active_runs: 0,
                completed_runs: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                average_latency_ms: None,
                latest_started_at_unix_ms: None,
                estimated_cost_usd: None,
            },
            latency_weighted_total_ms: 0,
            agent_ids: HashSet::new(),
        });
        entry.record.session_count += 1;
        entry.record.runs += session.runs;
        entry.record.active_runs += session.active_runs;
        entry.record.completed_runs += session.completed_runs;
        entry.record.prompt_tokens += session.prompt_tokens;
        entry.record.completion_tokens += session.completion_tokens;
        entry.record.total_tokens += session.total_tokens;
        entry.record.latest_started_at_unix_ms = latest_unix_ms(
            entry.record.latest_started_at_unix_ms,
            session.latest_started_at_unix_ms,
        );
        if let Some(agent_id) = agent_id {
            entry.agent_ids.insert(agent_id);
        }
        if let Some(average_latency_ms) = session.average_latency_ms {
            entry.latency_weighted_total_ms +=
                u128::from(average_latency_ms) * u128::from(session.completed_runs);
        }
    }

    let mut rows = aggregates.into_values().map(finalize_model_accumulator).collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .total_tokens
            .cmp(&left.total_tokens)
            .then_with(|| right.runs.cmp(&left.runs))
            .then_with(|| left.model_id.cmp(&right.model_id))
    });
    rows
}

fn resolve_usage_agent_identity(
    session: &journal::OrchestratorUsageSessionRecord,
    metadata: &UsageMetadata,
) -> (String, String, String, Option<String>) {
    let binding = metadata.bindings_by_session.get(session.session_id.as_str());
    let agent = binding.and_then(|record| metadata.agents_by_id.get(record.agent_id.as_str()));
    match (binding, agent) {
        (Some(binding), Some(agent)) => (
            binding.agent_id.clone(),
            agent.display_name.clone(),
            "session_binding".to_owned(),
            Some(agent.default_model_profile.clone()),
        ),
        (Some(binding), None) => {
            (binding.agent_id.clone(), binding.agent_id.clone(), "session_binding".to_owned(), None)
        }
        (None, _) => {
            ("unassigned".to_owned(), "Unassigned".to_owned(), "unassigned".to_owned(), None)
        }
    }
}

fn resolve_usage_model_identity(
    session: &journal::OrchestratorUsageSessionRecord,
    metadata: &UsageMetadata,
) -> (String, String, String, Option<String>) {
    let binding = metadata.bindings_by_session.get(session.session_id.as_str());
    let agent = binding.and_then(|record| metadata.agents_by_id.get(record.agent_id.as_str()));
    match (binding, agent) {
        (Some(binding), Some(agent)) => (
            agent.default_model_profile.clone(),
            agent.default_model_profile.clone(),
            "agent_default_model_profile".to_owned(),
            Some(binding.agent_id.clone()),
        ),
        _ => ("unassigned".to_owned(), "Unassigned".to_owned(), "unassigned".to_owned(), None),
    }
}

#[derive(Debug)]
struct UsageAgentAccumulator {
    record: UsageAgentRecord,
    latency_weighted_total_ms: u128,
}

#[derive(Debug)]
struct UsageModelAccumulator {
    record: UsageModelRecord,
    latency_weighted_total_ms: u128,
    agent_ids: HashSet<String>,
}

fn finalize_agent_accumulator(mut aggregate: UsageAgentAccumulator) -> UsageAgentRecord {
    aggregate.record.average_latency_ms =
        weighted_latency(aggregate.latency_weighted_total_ms, aggregate.record.completed_runs);
    aggregate.record
}

fn finalize_model_accumulator(mut aggregate: UsageModelAccumulator) -> UsageModelRecord {
    aggregate.record.agent_count = aggregate.agent_ids.len() as u64;
    aggregate.record.average_latency_ms =
        weighted_latency(aggregate.latency_weighted_total_ms, aggregate.record.completed_runs);
    aggregate.record
}

fn weighted_latency(weighted_total_ms: u128, completed_runs: u64) -> Option<u64> {
    if completed_runs == 0 {
        return None;
    }
    Some((weighted_total_ms / u128::from(completed_runs)) as u64)
}

fn latest_unix_ms(current: Option<i64>, candidate: Option<i64>) -> Option<i64> {
    match (current, candidate) {
        (Some(current), Some(candidate)) => Some(current.max(candidate)),
        (Some(current), None) => Some(current),
        (None, Some(candidate)) => Some(candidate),
        (None, None) => None,
    }
}

#[allow(clippy::result_large_err)]
fn usage_json_export_response(
    dataset: &str,
    payload: serde_json::Value,
) -> Result<Response, Response> {
    let filename = format!("usage-{dataset}.json");
    let body = serde_json::to_string_pretty(&payload).map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to serialize usage export JSON: {error}"
        )))
    })?;
    Ok((
        [
            (CONTENT_TYPE, HeaderValue::from_static("application/json; charset=utf-8")),
            (
                axum::http::header::CONTENT_DISPOSITION,
                HeaderValue::from_str(format!("attachment; filename=\"{filename}\"").as_str())
                    .map_err(|_| {
                        runtime_status_response(tonic::Status::internal(
                            "failed to build usage export content-disposition header",
                        ))
                    })?,
            ),
        ],
        body,
    )
        .into_response())
}

#[allow(clippy::result_large_err)]
fn usage_csv_export_response(dataset: &str, body: String) -> Result<Response, Response> {
    let filename = format!("usage-{dataset}.csv");
    Ok((
        [
            (CONTENT_TYPE, HeaderValue::from_static("text/csv; charset=utf-8")),
            (
                axum::http::header::CONTENT_DISPOSITION,
                HeaderValue::from_str(format!("attachment; filename=\"{filename}\"").as_str())
                    .map_err(|_| {
                        runtime_status_response(tonic::Status::internal(
                            "failed to build usage export content-disposition header",
                        ))
                    })?,
            ),
        ],
        body,
    )
        .into_response())
}

fn build_timeline_csv(rows: &[journal::OrchestratorUsageTimelineBucket]) -> String {
    let mut csv = String::from(
        "bucket_start_unix_ms,bucket_end_unix_ms,runs,session_count,active_runs,completed_runs,prompt_tokens,completion_tokens,total_tokens,average_latency_ms,estimated_cost_usd\n",
    );
    for row in rows {
        push_csv_row(
            &mut csv,
            &[
                row.bucket_start_unix_ms.to_string(),
                row.bucket_end_unix_ms.to_string(),
                row.runs.to_string(),
                row.session_count.to_string(),
                row.active_runs.to_string(),
                row.completed_runs.to_string(),
                row.prompt_tokens.to_string(),
                row.completion_tokens.to_string(),
                row.total_tokens.to_string(),
                optional_u64(row.average_latency_ms),
                optional_f64(row.estimated_cost_usd),
            ],
        );
    }
    csv
}

fn build_sessions_csv(rows: &[journal::OrchestratorUsageSessionRecord]) -> String {
    let mut csv = String::from(
        "session_id,session_key,session_label,archived,archived_at_unix_ms,last_run_id,runs,active_runs,completed_runs,prompt_tokens,completion_tokens,total_tokens,average_latency_ms,latest_started_at_unix_ms,estimated_cost_usd\n",
    );
    for row in rows {
        push_csv_row(
            &mut csv,
            &[
                row.session_id.clone(),
                row.session_key.clone(),
                row.session_label.clone().unwrap_or_default(),
                row.archived.to_string(),
                optional_i64(row.archived_at_unix_ms),
                row.last_run_id.clone().unwrap_or_default(),
                row.runs.to_string(),
                row.active_runs.to_string(),
                row.completed_runs.to_string(),
                row.prompt_tokens.to_string(),
                row.completion_tokens.to_string(),
                row.total_tokens.to_string(),
                optional_u64(row.average_latency_ms),
                optional_i64(row.latest_started_at_unix_ms),
                optional_f64(row.estimated_cost_usd),
            ],
        );
    }
    csv
}

fn build_agents_csv(rows: &[UsageAgentRecord]) -> String {
    let mut csv = String::from(
        "agent_id,display_name,binding_source,default_model_profile,session_count,runs,active_runs,completed_runs,prompt_tokens,completion_tokens,total_tokens,average_latency_ms,latest_started_at_unix_ms,estimated_cost_usd\n",
    );
    for row in rows {
        push_csv_row(
            &mut csv,
            &[
                row.agent_id.clone(),
                row.display_name.clone(),
                row.binding_source.clone(),
                row.default_model_profile.clone().unwrap_or_default(),
                row.session_count.to_string(),
                row.runs.to_string(),
                row.active_runs.to_string(),
                row.completed_runs.to_string(),
                row.prompt_tokens.to_string(),
                row.completion_tokens.to_string(),
                row.total_tokens.to_string(),
                optional_u64(row.average_latency_ms),
                optional_i64(row.latest_started_at_unix_ms),
                optional_f64(row.estimated_cost_usd),
            ],
        );
    }
    csv
}

fn build_models_csv(rows: &[UsageModelRecord]) -> String {
    let mut csv = String::from(
        "model_id,display_name,model_source,agent_count,session_count,runs,active_runs,completed_runs,prompt_tokens,completion_tokens,total_tokens,average_latency_ms,latest_started_at_unix_ms,estimated_cost_usd\n",
    );
    for row in rows {
        push_csv_row(
            &mut csv,
            &[
                row.model_id.clone(),
                row.display_name.clone(),
                row.model_source.clone(),
                row.agent_count.to_string(),
                row.session_count.to_string(),
                row.runs.to_string(),
                row.active_runs.to_string(),
                row.completed_runs.to_string(),
                row.prompt_tokens.to_string(),
                row.completion_tokens.to_string(),
                row.total_tokens.to_string(),
                optional_u64(row.average_latency_ms),
                optional_i64(row.latest_started_at_unix_ms),
                optional_f64(row.estimated_cost_usd),
            ],
        );
    }
    csv
}

fn push_csv_row(buffer: &mut String, values: &[String]) {
    let encoded = values.iter().map(|value| csv_escape(value.as_str())).collect::<Vec<_>>();
    buffer.push_str(encoded.join(",").as_str());
    buffer.push('\n');
}

fn csv_escape(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

fn optional_u64(value: Option<u64>) -> String {
    value.map(|entry| entry.to_string()).unwrap_or_default()
}

fn optional_i64(value: Option<i64>) -> String {
    value.map(|entry| entry.to_string()).unwrap_or_default()
}

fn optional_f64(value: Option<f64>) -> String {
    value.map(|entry| format!("{entry:.6}")).unwrap_or_default()
}

fn current_unix_ms() -> Result<i64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64)
}
