use crate::*;
use crate::{
    gateway::current_unix_ms,
    journal::{
        ToolJobAttachRequest, ToolJobRecord, ToolJobRetryRequest, ToolJobState,
        ToolJobTailReadRequest, ToolJobTransitionRequest, ToolJobsListFilter,
    },
};
use serde::Deserialize;
use serde_json::{json, Value};
use tonic::Status;

#[derive(Debug, Deserialize)]
pub(crate) struct ConsoleJobsListQuery {
    pub(crate) limit: Option<usize>,
    pub(crate) session_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) include_terminal: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConsoleJobTailQuery {
    pub(crate) offset: Option<i64>,
    pub(crate) limit: Option<usize>,
    pub(crate) max_bytes: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConsoleJobActionRequest {
    pub(crate) reason: Option<String>,
    pub(crate) idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConsoleJobSweepRequest {
    pub(crate) now_unix_ms: Option<i64>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConsoleJobRecoverRequest {
    pub(crate) now_unix_ms: Option<i64>,
    pub(crate) stale_after_ms: Option<i64>,
    pub(crate) limit: Option<usize>,
}

pub(crate) async fn console_jobs_list_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConsoleJobsListQuery>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let jobs = state
        .runtime
        .list_tool_jobs(ToolJobsListFilter {
            owner_principal: Some(session.context.principal.clone()),
            session_id: query.session_id,
            run_id: query.run_id,
            include_terminal: query.include_terminal.unwrap_or(false),
            limit: query.limit.unwrap_or(50),
        })
        .await
        .map_err(runtime_status_response)?;
    let next_cursor = jobs.last().map(|job| job.job_id.clone());
    Ok(Json(json!({
        "contract": contract_descriptor(),
        "page": build_page_info(query.limit.unwrap_or(50), jobs.len(), next_cursor),
        "jobs": jobs,
    })))
}

pub(crate) async fn console_job_get_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let job = load_authorized_job(&state, job_id, session.context.principal.as_str()).await?;
    Ok(Json(job_envelope(job)))
}

pub(crate) async fn console_job_tail_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
    Query(query): Query<ConsoleJobTailQuery>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let page = state
        .runtime
        .tail_tool_job(ToolJobTailReadRequest {
            job_id: normalize_non_empty_field(job_id, "job_id")?,
            owner_principal: Some(session.context.principal.clone()),
            offset: query.offset.unwrap_or(0).max(0),
            limit: query.limit.unwrap_or(100),
            max_bytes: query.max_bytes.unwrap_or(16 * 1024),
        })
        .await
        .map_err(runtime_status_response)?;
    Ok(Json(json!({
        "contract": contract_descriptor(),
        "tail": page,
    })))
}

pub(crate) async fn console_job_cancel_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
    Json(payload): Json<ConsoleJobActionRequest>,
) -> Result<Json<Value>, Response> {
    transition_authorized_job(
        state,
        headers,
        job_id,
        ToolJobState::Cancelling,
        payload.reason.unwrap_or_else(|| "operator_cancel".to_owned()),
        None,
    )
    .await
}

pub(crate) async fn console_job_drain_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
    Json(payload): Json<ConsoleJobActionRequest>,
) -> Result<Json<Value>, Response> {
    transition_authorized_job(
        state,
        headers,
        job_id,
        ToolJobState::Draining,
        payload.reason.unwrap_or_else(|| "operator_drain".to_owned()),
        None,
    )
    .await
}

pub(crate) async fn console_job_resume_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
    Json(payload): Json<ConsoleJobActionRequest>,
) -> Result<Json<Value>, Response> {
    transition_authorized_job(
        state,
        headers,
        job_id,
        ToolJobState::Queued,
        payload.reason.unwrap_or_else(|| "operator_resume".to_owned()),
        None,
    )
    .await
}

pub(crate) async fn console_job_retry_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
    Json(payload): Json<ConsoleJobActionRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, true)?;
    let job = state
        .runtime
        .retry_tool_job(ToolJobRetryRequest {
            job_id: normalize_non_empty_field(job_id, "job_id")?,
            owner_principal: Some(session.context.principal.clone()),
            idempotency_key: payload.idempotency_key,
            reason: payload.reason.unwrap_or_else(|| "operator_retry".to_owned()),
        })
        .await
        .map_err(runtime_status_response)?;
    Ok(Json(job_envelope(job)))
}

pub(crate) async fn console_job_attach_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let job = state
        .runtime
        .attach_tool_job(ToolJobAttachRequest {
            job_id: normalize_non_empty_field(job_id, "job_id")?,
            owner_principal: Some(session.context.principal.clone()),
        })
        .await
        .map_err(runtime_status_response)?;
    Ok(Json(job_envelope(job)))
}

pub(crate) async fn console_job_release_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let job_id = normalize_non_empty_field(job_id, "job_id")?;
    let _job =
        load_authorized_job(&state, job_id.clone(), session.context.principal.as_str()).await?;
    let job =
        state.runtime.release_tool_job_attachment(job_id).await.map_err(runtime_status_response)?;
    Ok(Json(job_envelope(job)))
}

pub(crate) async fn console_jobs_sweep_expired_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConsoleJobSweepRequest>,
) -> Result<Json<Value>, Response> {
    let _session = authorize_console_session(&state, &headers, true)?;
    let jobs = state
        .runtime
        .sweep_expired_tool_jobs(
            payload.now_unix_ms.unwrap_or_else(current_unix_ms),
            payload.limit.unwrap_or(100),
        )
        .await
        .map_err(runtime_status_response)?;
    Ok(Json(json!({
        "contract": contract_descriptor(),
        "jobs": jobs,
    })))
}

pub(crate) async fn console_jobs_recover_stale_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConsoleJobRecoverRequest>,
) -> Result<Json<Value>, Response> {
    let _session = authorize_console_session(&state, &headers, true)?;
    let jobs = state
        .runtime
        .recover_stale_tool_jobs(
            payload.now_unix_ms.unwrap_or_else(current_unix_ms),
            payload.stale_after_ms.unwrap_or(5 * 60 * 1_000),
            payload.limit.unwrap_or(100),
        )
        .await
        .map_err(runtime_status_response)?;
    Ok(Json(json!({
        "contract": contract_descriptor(),
        "jobs": jobs,
    })))
}

async fn transition_authorized_job(
    state: AppState,
    headers: HeaderMap,
    job_id: String,
    next_state: ToolJobState,
    reason: String,
    last_error: Option<String>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, true)?;
    let job_id = normalize_non_empty_field(job_id, "job_id")?;
    let _job =
        load_authorized_job(&state, job_id.clone(), session.context.principal.as_str()).await?;
    let job = state
        .runtime
        .transition_tool_job(ToolJobTransitionRequest {
            job_id,
            expected_state: None,
            next_state,
            reason,
            last_error,
            heartbeat_at_unix_ms: Some(current_unix_ms()),
            lease_expires_at_unix_ms: None,
        })
        .await
        .map_err(runtime_status_response)?;
    Ok(Json(job_envelope(job)))
}

async fn load_authorized_job(
    state: &AppState,
    job_id: String,
    principal: &str,
) -> Result<ToolJobRecord, Response> {
    let job_id = normalize_non_empty_field(job_id, "job_id")?;
    let job = state
        .runtime
        .get_tool_job(job_id.clone())
        .await
        .map_err(runtime_status_response)?
        .ok_or_else(|| runtime_status_response(Status::not_found("tool job not found")))?;
    if job.owner_principal != principal {
        return Err(runtime_status_response(Status::permission_denied(
            "tool job is outside the current console principal scope",
        )));
    }
    Ok(job)
}

fn job_envelope(job: ToolJobRecord) -> Value {
    json!({
        "contract": contract_descriptor(),
        "job": job,
    })
}
