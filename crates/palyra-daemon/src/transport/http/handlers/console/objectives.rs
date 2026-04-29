use std::sync::Arc;

use serde::Deserialize;

use super::diagnostics::{authorize_console_session, build_page_info};

use crate::{
    cron::{self, CronTimezoneMode},
    domain::workspace::{
        objective_workspace_document_path, sync_workspace_managed_block,
        WorkspaceManagedBlockUpdate, WorkspaceManagedEntry,
    },
    gateway::proto::palyra::cron::v1 as cron_v1,
    journal::{
        CronConcurrencyPolicy, CronJobCreateRequest, CronJobRecord, CronJobUpdatePatch,
        CronMisfirePolicy, CronRetryPolicy, CronScheduleType, OrchestratorCancelRequest,
        WorkspaceDocumentWriteRequest,
    },
    objectives::{
        ObjectiveApproachKind, ObjectiveApproachRecord, ObjectiveAttemptRecord,
        ObjectiveAutomationBinding, ObjectiveBudget, ObjectiveKind, ObjectiveLifecycleRecord,
        ObjectivePriority, ObjectiveRecord, ObjectiveRegistryError, ObjectiveState,
        ObjectiveUpsert, ObjectiveWorkspaceBinding,
    },
    routines::{
        default_outcome_from_cron_status, join_run_metadata, natural_language_schedule_preview,
        shadow_manual_schedule_payload_json, RoutineApprovalMode, RoutineApprovalPolicy,
        RoutineDeliveryConfig, RoutineDeliveryMode, RoutineDispatchMode, RoutineExecutionConfig,
        RoutineQuietHours, RoutineRunMetadataUpsert, RoutineSilentPolicy, RoutineTriggerKind,
    },
    *,
};

const DEFAULT_OBJECTIVE_CHANNEL: &str = "system:objectives";
const DEFAULT_OBJECTIVE_PAGE_LIMIT: usize = 100;
const MAX_OBJECTIVE_PAGE_LIMIT: usize = 500;
const OBJECTIVE_FOCUS_BLOCK_ID: &str = "objective-focus";
const OBJECTIVE_HEARTBEAT_BLOCK_ID: &str = "objective-heartbeats";
const OBJECTIVE_INBOX_BLOCK_ID: &str = "objective-inbox";

#[derive(Debug, Deserialize)]
pub(crate) struct ConsoleObjectiveListQuery {
    #[serde(default)]
    after_objective_id: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConsoleObjectiveBudgetPayload {
    #[serde(default)]
    max_runs: Option<u32>,
    #[serde(default)]
    max_tokens: Option<u64>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConsoleObjectiveUpsertRequest {
    #[serde(default)]
    objective_id: Option<String>,
    kind: String,
    name: String,
    prompt: String,
    #[serde(default)]
    owner_principal: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    session_key: Option<String>,
    #[serde(default)]
    session_label: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    budget: Option<ConsoleObjectiveBudgetPayload>,
    #[serde(default)]
    current_focus: Option<String>,
    #[serde(default)]
    success_criteria: Option<String>,
    #[serde(default)]
    exit_condition: Option<String>,
    #[serde(default)]
    next_recommended_step: Option<String>,
    #[serde(default)]
    standing_order: Option<String>,
    #[serde(default)]
    related_document_paths: Option<Vec<String>>,
    #[serde(default)]
    related_memory_ids: Option<Vec<String>>,
    #[serde(default)]
    related_session_ids: Option<Vec<String>>,
    #[serde(default)]
    linked_artifact_paths: Option<Vec<String>>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    natural_language_schedule: Option<String>,
    #[serde(default)]
    schedule_type: Option<String>,
    #[serde(default)]
    cron_expression: Option<String>,
    #[serde(default)]
    every_interval_ms: Option<u64>,
    #[serde(default)]
    at_timestamp_rfc3339: Option<String>,
    #[serde(default)]
    delivery_mode: Option<String>,
    #[serde(default)]
    delivery_channel: Option<String>,
    #[serde(default)]
    quiet_hours_start: Option<String>,
    #[serde(default)]
    quiet_hours_end: Option<String>,
    #[serde(default)]
    quiet_hours_timezone: Option<String>,
    #[serde(default)]
    cooldown_ms: Option<u64>,
    #[serde(default)]
    approval_mode: Option<String>,
    #[serde(default)]
    template_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConsoleObjectiveLifecycleRequest {
    action: String,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConsoleObjectiveAttemptRequest {
    status: String,
    summary: String,
    #[serde(default)]
    run_id: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    outcome_kind: Option<String>,
    #[serde(default)]
    learned: Option<String>,
    #[serde(default)]
    recommended_next_step: Option<String>,
    #[serde(default)]
    completed_at_unix_ms: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConsoleObjectiveApproachRequest {
    kind: String,
    summary: String,
    #[serde(default)]
    run_id: Option<String>,
}

pub(crate) async fn console_objectives_list_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConsoleObjectiveListQuery>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let limit =
        query.limit.unwrap_or(DEFAULT_OBJECTIVE_PAGE_LIMIT).clamp(1, MAX_OBJECTIVE_PAGE_LIMIT);
    let kind_filter = query
        .kind
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let state_filter = query
        .state
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let mut objectives = list_objective_views(
        &state,
        session.context.principal.as_str(),
        kind_filter.as_deref(),
        state_filter.as_deref(),
    )
    .await?;
    objectives.sort_by(|left, right| {
        read_string_value(left, "objective_id").cmp(&read_string_value(right, "objective_id"))
    });
    if let Some(after_objective_id) =
        query.after_objective_id.as_deref().map(str::trim).filter(|value| !value.is_empty())
    {
        objectives.retain(|objective| {
            read_string_value(objective, "objective_id").as_str() > after_objective_id
        });
    }
    let has_more = objectives.len() > limit;
    if has_more {
        objectives.truncate(limit);
    }
    let next_after_objective_id = if has_more {
        objectives
            .last()
            .and_then(|objective| objective.get("objective_id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    } else {
        None
    };
    Ok(Json(json!({
        "objectives": objectives,
        "next_after_objective_id": next_after_objective_id,
        "page": build_page_info(limit, objectives.len(), next_after_objective_id.clone()),
    })))
}

pub(crate) async fn console_objective_get_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(objective_id): Path<String>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let objective =
        load_objective_for_owner(&state, objective_id.as_str(), session.context.principal.as_str())
            .await?;
    Ok(Json(json!({ "objective": objective })))
}

pub(crate) async fn console_objective_upsert_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConsoleObjectiveUpsertRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let owner_principal =
        normalize_owner_principal(&payload.owner_principal, session.context.principal.as_str())?;
    let kind = parse_objective_kind(payload.kind.as_str())?;
    let priority = parse_objective_priority(payload.priority.as_deref())?;
    let channel = normalize_channel(payload.channel.as_deref(), session.context.channel.as_deref());
    let existing = if let Some(objective_id) = payload.objective_id.as_deref() {
        state.objectives.get_objective(objective_id).map_err(objective_registry_error_response)?
    } else {
        None
    };
    if let Some(existing_objective) = existing.as_ref() {
        ensure_objective_owner(existing_objective, owner_principal.as_str())?;
    }
    let objective_id = existing
        .as_ref()
        .map(|entry| entry.objective_id.clone())
        .unwrap_or_else(|| Ulid::new().to_string());
    let workspace_document_path = existing
        .as_ref()
        .map(|entry| entry.workspace.workspace_document_path.clone())
        .unwrap_or_else(|| {
            objective_workspace_document_path(objective_id.as_str())
                .expect("objective workspace path should normalize")
        });
    let schedule = resolve_objective_schedule(&payload, kind, state.cron_timezone_mode)?;
    let automation = ObjectiveAutomationBinding {
        routine_id: Some(
            existing
                .as_ref()
                .and_then(|entry| entry.automation.routine_id.clone())
                .unwrap_or_else(|| objective_id.clone()),
        ),
        enabled: payload.enabled.unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|entry| entry.automation.enabled)
                .unwrap_or(kind != ObjectiveKind::Heartbeat || schedule.is_scheduled)
        }),
        trigger_kind: schedule.trigger_kind,
        schedule_type: schedule.schedule_type.as_str().to_owned(),
        schedule_payload_json: schedule.schedule_payload_json.clone(),
        delivery: parse_delivery(
            payload.delivery_mode.as_deref(),
            payload.delivery_channel.clone(),
        )?,
        quiet_hours: parse_quiet_hours(
            payload.quiet_hours_start.as_deref(),
            payload.quiet_hours_end.as_deref(),
            payload.quiet_hours_timezone.clone(),
        )?,
        cooldown_ms: payload.cooldown_ms.unwrap_or_else(|| {
            existing.as_ref().map(|entry| entry.automation.cooldown_ms).unwrap_or(0)
        }),
        approval_policy: parse_approval_policy(payload.approval_mode.as_deref())?,
        template_id: payload.template_id.clone().or_else(|| {
            if kind == ObjectiveKind::Heartbeat {
                Some("heartbeat".to_owned())
            } else {
                existing.as_ref().and_then(|entry| entry.automation.template_id.clone())
            }
        }),
    };
    let routine_id = automation.routine_id.clone().ok_or_else(|| {
        runtime_status_response(tonic::Status::internal(
            "objective automation routine id should always exist",
        ))
    })?;
    let cron_job = persist_objective_job(
        &state,
        existing.as_ref().and_then(|entry| entry.automation.routine_id.clone()),
        ObjectiveJobUpsert {
            routine_id,
            name: payload.name.clone(),
            prompt: payload.prompt.clone(),
            owner_principal: owner_principal.clone(),
            channel: channel.clone(),
            session_key: payload.session_key.clone(),
            session_label: payload.session_label.clone(),
            schedule_type: schedule.schedule_type,
            schedule_payload_json: schedule.schedule_payload_json,
            enabled: automation.enabled,
            next_run_at_unix_ms: schedule.next_run_at_unix_ms,
        },
    )
    .await?;
    persist_objective_routine_metadata(&state, &automation)
        .map_err(routine_registry_error_response)?;
    let now_unix_ms = unix_ms_now().map_err(internal_console_error)?;
    let mut lifecycle_history =
        existing.as_ref().map(|entry| entry.lifecycle_history.clone()).unwrap_or_default();
    if existing.is_none() {
        lifecycle_history.push(ObjectiveLifecycleRecord {
            event_id: Ulid::new().to_string(),
            action: "created".to_owned(),
            from_state: None,
            to_state: ObjectiveState::Draft,
            reason: Some(format!("kind={}", kind.as_str())),
            run_id: None,
            occurred_at_unix_ms: now_unix_ms,
        });
    }
    let objective = state
        .objectives
        .upsert_objective(ObjectiveUpsert {
            record: ObjectiveRecord {
                objective_id: objective_id.clone(),
                kind,
                state: existing.as_ref().map(|entry| entry.state).unwrap_or(ObjectiveState::Draft),
                name: payload.name,
                prompt: payload.prompt,
                owner_principal: owner_principal.clone(),
                channel: Some(channel.clone()),
                priority,
                budget: normalize_budget(payload.budget, existing.as_ref()),
                current_focus: payload
                    .current_focus
                    .or_else(|| existing.as_ref().and_then(|entry| entry.current_focus.clone())),
                success_criteria: payload
                    .success_criteria
                    .or_else(|| existing.as_ref().and_then(|entry| entry.success_criteria.clone())),
                exit_condition: payload
                    .exit_condition
                    .or_else(|| existing.as_ref().and_then(|entry| entry.exit_condition.clone())),
                next_recommended_step: payload.next_recommended_step.or_else(|| {
                    existing.as_ref().and_then(|entry| entry.next_recommended_step.clone())
                }),
                standing_order: payload
                    .standing_order
                    .or_else(|| existing.as_ref().and_then(|entry| entry.standing_order.clone())),
                workspace: ObjectiveWorkspaceBinding {
                    workspace_document_path,
                    session_key: payload.session_key.or_else(|| {
                        existing.as_ref().and_then(|entry| entry.workspace.session_key.clone())
                    }),
                    session_label: payload.session_label.or_else(|| {
                        existing.as_ref().and_then(|entry| entry.workspace.session_label.clone())
                    }),
                    related_document_paths: payload.related_document_paths.unwrap_or_else(|| {
                        existing
                            .as_ref()
                            .map(|entry| entry.workspace.related_document_paths.clone())
                            .unwrap_or_default()
                    }),
                    related_memory_ids: payload.related_memory_ids.unwrap_or_else(|| {
                        existing
                            .as_ref()
                            .map(|entry| entry.workspace.related_memory_ids.clone())
                            .unwrap_or_default()
                    }),
                    related_session_ids: payload.related_session_ids.unwrap_or_else(|| {
                        existing
                            .as_ref()
                            .map(|entry| entry.workspace.related_session_ids.clone())
                            .unwrap_or_default()
                    }),
                },
                automation,
                last_attempt: existing.as_ref().and_then(|entry| entry.last_attempt.clone()),
                attempt_history: existing
                    .as_ref()
                    .map(|entry| entry.attempt_history.clone())
                    .unwrap_or_default(),
                approach_history: existing
                    .as_ref()
                    .map(|entry| entry.approach_history.clone())
                    .unwrap_or_default(),
                lifecycle_history,
                linked_run_ids: existing
                    .as_ref()
                    .map(|entry| entry.linked_run_ids.clone())
                    .unwrap_or_default(),
                linked_artifact_paths: payload.linked_artifact_paths.unwrap_or_else(|| {
                    existing
                        .as_ref()
                        .map(|entry| entry.linked_artifact_paths.clone())
                        .unwrap_or_default()
                }),
                created_at_unix_ms: existing
                    .as_ref()
                    .map(|entry| entry.created_at_unix_ms)
                    .unwrap_or(now_unix_ms),
                updated_at_unix_ms: now_unix_ms,
                archived_at_unix_ms: existing.and_then(|entry| entry.archived_at_unix_ms),
            },
        })
        .map_err(objective_registry_error_response)?;
    project_objective_workspace(
        &state,
        owner_principal.as_str(),
        Some(channel.as_str()),
        Some(cron_job.session_key.as_deref().unwrap_or_default()).filter(|value| !value.is_empty()),
        &objective,
    )
    .await?;
    let objective_view = build_objective_view(&state, objective).await?;
    Ok(Json(json!({ "objective": objective_view })))
}

pub(crate) async fn console_objective_lifecycle_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(objective_id): Path<String>,
    Json(payload): Json<ConsoleObjectiveLifecycleRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let mut objective =
        load_objective_record(&state, objective_id.as_str(), session.context.principal.as_str())?;
    let action = payload.action.trim().to_ascii_lowercase();
    let reason = normalize_lifecycle_reason(payload.reason)?;
    let mut preflight_objective = objective.clone();
    apply_lifecycle_workspace_projection(action.as_str(), &mut preflight_objective)
        .map_err(runtime_status_response)?;
    preflight_objective_workspace_projection(&state, &preflight_objective).await?;
    match action.as_str() {
        "fire" => apply_fire_action(&state, &mut objective, reason).await?,
        "pause" => apply_pause_action(&state, &mut objective, reason).await?,
        "resume" => apply_resume_action(&state, &mut objective, reason).await?,
        "cancel" => apply_cancel_action(&state, &mut objective, reason).await?,
        "archive" => apply_archive_action(&state, &mut objective, reason).await?,
        _ => {
            return Err(runtime_status_response(tonic::Status::invalid_argument(
                "action must be one of fire|pause|resume|cancel|archive",
            )));
        }
    }
    let updated = state
        .objectives
        .upsert_objective(ObjectiveUpsert { record: objective })
        .map_err(objective_registry_error_response)?;
    project_objective_workspace(
        &state,
        updated.owner_principal.as_str(),
        updated.channel.as_deref(),
        updated.workspace.session_key.as_deref(),
        &updated,
    )
    .await?;
    Ok(Json(json!({ "objective": build_objective_view(&state, updated).await? })))
}

pub(crate) async fn console_objective_attempt_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(objective_id): Path<String>,
    Json(payload): Json<ConsoleObjectiveAttemptRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let mut objective =
        load_objective_record(&state, objective_id.as_str(), session.context.principal.as_str())?;
    let now_unix_ms = unix_ms_now().map_err(internal_console_error)?;
    let attempt = ObjectiveAttemptRecord {
        attempt_id: Ulid::new().to_string(),
        run_id: normalize_optional_text(payload.run_id.as_deref()),
        session_id: normalize_optional_text(payload.session_id.as_deref()),
        status: payload.status,
        outcome_kind: payload.outcome_kind,
        summary: payload.summary,
        learned: payload.learned,
        recommended_next_step: payload
            .recommended_next_step
            .or_else(|| objective.next_recommended_step.clone()),
        created_at_unix_ms: now_unix_ms,
        completed_at_unix_ms: payload.completed_at_unix_ms,
    };
    if let Some(run_id) = attempt.run_id.as_ref() {
        objective.linked_run_ids.push(run_id.clone());
    }
    if let Some(next_step) = attempt.recommended_next_step.clone() {
        objective.next_recommended_step = Some(next_step);
    }
    objective.last_attempt = Some(attempt.clone());
    objective.attempt_history.push(attempt);
    let updated = state
        .objectives
        .upsert_objective(ObjectiveUpsert { record: objective })
        .map_err(objective_registry_error_response)?;
    project_objective_workspace(
        &state,
        updated.owner_principal.as_str(),
        updated.channel.as_deref(),
        updated.workspace.session_key.as_deref(),
        &updated,
    )
    .await?;
    Ok(Json(json!({ "objective": build_objective_view(&state, updated).await? })))
}

pub(crate) async fn console_objective_approach_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(objective_id): Path<String>,
    Json(payload): Json<ConsoleObjectiveApproachRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let mut objective =
        load_objective_record(&state, objective_id.as_str(), session.context.principal.as_str())?;
    objective.approach_history.push(ObjectiveApproachRecord {
        entry_id: Ulid::new().to_string(),
        kind: parse_approach_kind(payload.kind.as_str())?,
        summary: payload.summary,
        run_id: normalize_optional_text(payload.run_id.as_deref()),
        created_at_unix_ms: unix_ms_now().map_err(internal_console_error)?,
    });
    let updated = state
        .objectives
        .upsert_objective(ObjectiveUpsert { record: objective })
        .map_err(objective_registry_error_response)?;
    project_objective_workspace(
        &state,
        updated.owner_principal.as_str(),
        updated.channel.as_deref(),
        updated.workspace.session_key.as_deref(),
        &updated,
    )
    .await?;
    Ok(Json(json!({ "objective": build_objective_view(&state, updated).await? })))
}

pub(crate) async fn console_objective_summary_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(objective_id): Path<String>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let objective =
        load_objective_record(&state, objective_id.as_str(), session.context.principal.as_str())?;
    let view = build_objective_view(&state, objective.clone()).await?;
    Ok(Json(json!({
        "objective_id": objective.objective_id,
        "summary_markdown": render_objective_summary_markdown(&view),
        "summary": view,
    })))
}

#[derive(Debug, Clone)]
struct ObjectiveScheduleResolution {
    trigger_kind: RoutineTriggerKind,
    schedule_type: CronScheduleType,
    schedule_payload_json: String,
    next_run_at_unix_ms: Option<i64>,
    is_scheduled: bool,
}

#[derive(Debug, Clone)]
struct ObjectiveJobUpsert {
    routine_id: String,
    name: String,
    prompt: String,
    owner_principal: String,
    channel: String,
    session_key: Option<String>,
    session_label: Option<String>,
    schedule_type: CronScheduleType,
    schedule_payload_json: String,
    enabled: bool,
    next_run_at_unix_ms: Option<i64>,
}

async fn load_objective_for_owner(
    state: &AppState,
    objective_id: &str,
    owner_principal: &str,
) -> Result<Value, Response> {
    let objective = load_objective_record(state, objective_id, owner_principal)?;
    build_objective_view(state, objective).await
}

#[allow(clippy::result_large_err)]
fn load_objective_record(
    state: &AppState,
    objective_id: &str,
    owner_principal: &str,
) -> Result<ObjectiveRecord, Response> {
    let objective = state
        .objectives
        .get_objective(objective_id)
        .map_err(objective_registry_error_response)?
        .ok_or_else(|| runtime_status_response(tonic::Status::not_found("objective not found")))?;
    ensure_objective_owner(&objective, owner_principal)?;
    Ok(objective)
}

async fn list_objective_views(
    state: &AppState,
    principal: &str,
    kind_filter: Option<&str>,
    state_filter: Option<&str>,
) -> Result<Vec<Value>, Response> {
    let mut objectives = Vec::new();
    for objective in state
        .objectives
        .list_objectives()
        .map_err(objective_registry_error_response)?
        .into_iter()
        .filter(|entry| entry.owner_principal == principal)
        .filter(|entry| kind_filter.is_none_or(|expected| entry.kind.as_str() == expected))
        .filter(|entry| state_filter.is_none_or(|expected| entry.state.as_str() == expected))
    {
        objectives.push(build_objective_view(state, objective).await?);
    }
    Ok(objectives)
}

async fn build_objective_view(
    state: &AppState,
    objective: ObjectiveRecord,
) -> Result<Value, Response> {
    let routine = load_objective_routine(state, &objective).await?;
    let latest_run = latest_objective_run(state, &objective).await?;
    let health = compute_objective_health(&objective, routine.as_ref(), latest_run.as_ref());
    Ok(json!({
        "objective_id": objective.objective_id,
        "kind": objective.kind.as_str(),
        "state": objective.state.as_str(),
        "name": objective.name,
        "prompt": objective.prompt,
        "owner_principal": objective.owner_principal,
        "channel": objective.channel,
        "priority": objective.priority.as_str(),
        "budget": {
            "max_runs": objective.budget.max_runs,
            "max_tokens": objective.budget.max_tokens,
            "notes": objective.budget.notes,
        },
        "current_focus": objective.current_focus,
        "success_criteria": objective.success_criteria,
        "exit_condition": objective.exit_condition,
        "next_recommended_step": objective.next_recommended_step,
        "standing_order": objective.standing_order,
        "workspace": {
            "workspace_document_path": objective.workspace.workspace_document_path,
            "session_key": objective.workspace.session_key,
            "session_label": objective.workspace.session_label,
            "related_document_paths": objective.workspace.related_document_paths,
            "related_memory_ids": objective.workspace.related_memory_ids,
            "related_session_ids": objective.workspace.related_session_ids,
        },
        "automation": {
            "routine_id": objective.automation.routine_id,
            "enabled": objective.automation.enabled,
            "trigger_kind": objective.automation.trigger_kind.as_str(),
            "schedule_type": objective.automation.schedule_type,
            "schedule_payload": serde_json::from_str::<Value>(objective.automation.schedule_payload_json.as_str())
                .unwrap_or_else(|_| json!({ "raw": objective.automation.schedule_payload_json })),
            "delivery_mode": objective.automation.delivery.mode.as_str(),
            "delivery_channel": objective.automation.delivery.channel,
            "cooldown_ms": objective.automation.cooldown_ms,
            "approval_mode": objective.automation.approval_policy.mode.as_str(),
            "template_id": objective.automation.template_id,
        },
        "linked_run_ids": objective.linked_run_ids,
        "linked_artifact_paths": objective.linked_artifact_paths,
        "last_attempt": objective.last_attempt,
        "attempt_history": objective.attempt_history,
        "approach_history": objective.approach_history,
        "lifecycle_history": objective.lifecycle_history,
        "linked_routine": routine,
        "last_run": latest_run,
        "health": health,
        "created_at_unix_ms": objective.created_at_unix_ms,
        "updated_at_unix_ms": objective.updated_at_unix_ms,
        "archived_at_unix_ms": objective.archived_at_unix_ms,
    }))
}

async fn load_objective_routine(
    state: &AppState,
    objective: &ObjectiveRecord,
) -> Result<Option<Value>, Response> {
    let Some(routine_id) = objective.automation.routine_id.as_ref() else {
        return Ok(None);
    };
    let Some(job) =
        state.runtime.cron_job(routine_id.clone()).await.map_err(runtime_status_response)?
    else {
        return Ok(None);
    };
    Ok(Some(json!({
        "job_id": job.job_id,
        "name": job.name,
        "prompt": job.prompt,
        "enabled": job.enabled,
        "channel": job.channel,
        "session_key": job.session_key,
        "session_label": job.session_label,
        "schedule_type": job.schedule_type.as_str(),
        "schedule_payload": serde_json::from_str::<Value>(job.schedule_payload_json.as_str())
            .unwrap_or_else(|_| json!({ "raw": job.schedule_payload_json })),
        "next_run_at_unix_ms": job.next_run_at_unix_ms,
        "last_run_at_unix_ms": job.last_run_at_unix_ms,
        "queued_run": job.queued_run,
    })))
}

async fn latest_objective_run(
    state: &AppState,
    objective: &ObjectiveRecord,
) -> Result<Option<Value>, Response> {
    let Some(routine_id) = objective.automation.routine_id.as_ref() else {
        return Ok(None);
    };
    let (runs, _) = state
        .runtime
        .list_cron_runs(Some(routine_id.clone()), None, Some(1))
        .await
        .map_err(runtime_status_response)?;
    let Some(run) = runs.last() else {
        return Ok(None);
    };
    let metadata = state
        .routines
        .find_run_metadata(run.run_id.as_str())
        .map_err(routine_registry_error_response)?;
    Ok(Some(join_run_metadata(routine_id.as_str(), run, metadata.as_ref())))
}

fn compute_objective_health(
    objective: &ObjectiveRecord,
    routine: Option<&Value>,
    last_run: Option<&Value>,
) -> Value {
    let status = last_run
        .and_then(|entry| entry.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("never_run");
    let health = match objective.state {
        ObjectiveState::Archived => "archived",
        ObjectiveState::Cancelled => "cancelled",
        ObjectiveState::Paused => "paused",
        ObjectiveState::Draft => "draft",
        ObjectiveState::Active => match status {
            "succeeded" => "healthy",
            "failed" => "attention",
            "running" => "running",
            "queued" => "queued",
            "skipped" => "degraded",
            _ => "active",
        },
    };
    json!({
        "state": health,
        "last_run_status": status,
        "next_run_at_unix_ms": routine
            .and_then(|entry| entry.get("next_run_at_unix_ms"))
            .cloned()
            .unwrap_or(Value::Null),
    })
}

async fn persist_objective_job(
    state: &AppState,
    existing_routine_id: Option<String>,
    request: ObjectiveJobUpsert,
) -> Result<CronJobRecord, Response> {
    if let Some(routine_id) = existing_routine_id {
        state
            .runtime
            .update_cron_job(
                routine_id,
                CronJobUpdatePatch {
                    name: Some(request.name),
                    prompt: Some(request.prompt),
                    owner_principal: Some(request.owner_principal),
                    channel: Some(request.channel),
                    session_key: Some(request.session_key),
                    session_label: Some(request.session_label),
                    schedule_type: Some(request.schedule_type),
                    schedule_payload_json: Some(request.schedule_payload_json),
                    enabled: Some(request.enabled),
                    concurrency_policy: Some(CronConcurrencyPolicy::Forbid),
                    retry_policy: Some(CronRetryPolicy { max_attempts: 0, backoff_ms: 0 }),
                    misfire_policy: Some(CronMisfirePolicy::Skip),
                    jitter_ms: Some(0),
                    next_run_at_unix_ms: Some(request.next_run_at_unix_ms),
                    queued_run: Some(false),
                },
            )
            .await
            .map_err(runtime_status_response)
    } else {
        state
            .runtime
            .create_cron_job(CronJobCreateRequest {
                job_id: request.routine_id,
                name: request.name,
                prompt: request.prompt,
                owner_principal: request.owner_principal,
                channel: request.channel,
                session_key: request.session_key,
                session_label: request.session_label,
                schedule_type: request.schedule_type,
                schedule_payload_json: request.schedule_payload_json,
                enabled: request.enabled,
                concurrency_policy: CronConcurrencyPolicy::Forbid,
                retry_policy: CronRetryPolicy { max_attempts: 0, backoff_ms: 0 },
                misfire_policy: CronMisfirePolicy::Skip,
                jitter_ms: 0,
                next_run_at_unix_ms: request.next_run_at_unix_ms,
            })
            .await
            .map_err(runtime_status_response)
    }
}

fn persist_objective_routine_metadata(
    state: &AppState,
    automation: &ObjectiveAutomationBinding,
) -> Result<(), crate::routines::RoutineRegistryError> {
    let routine_id = automation
        .routine_id
        .clone()
        .expect("objective automation should always have a routine id");
    state.routines.upsert_routine(crate::routines::RoutineMetadataUpsert {
        routine_id,
        trigger_kind: automation.trigger_kind,
        trigger_payload_json: automation.schedule_payload_json.clone(),
        execution: RoutineExecutionConfig::default(),
        delivery: automation.delivery.clone(),
        quiet_hours: automation.quiet_hours.clone(),
        cooldown_ms: automation.cooldown_ms,
        approval_policy: automation.approval_policy.clone(),
        template_id: automation.template_id.clone(),
    })?;
    Ok(())
}

async fn set_objective_job_enabled(
    state: &AppState,
    objective: &ObjectiveRecord,
    enabled: bool,
) -> Result<(), Response> {
    let Some(routine_id) = objective.automation.routine_id.as_ref() else {
        return Ok(());
    };
    state
        .runtime
        .update_cron_job(
            routine_id.clone(),
            CronJobUpdatePatch { enabled: Some(enabled), ..CronJobUpdatePatch::default() },
        )
        .await
        .map_err(runtime_status_response)?;
    Ok(())
}

async fn trigger_objective_now(
    state: &AppState,
    objective: &ObjectiveRecord,
) -> Result<Value, Response> {
    let routine_id = objective.automation.routine_id.clone().ok_or_else(|| {
        runtime_status_response(tonic::Status::failed_precondition(
            "objective is missing linked automation",
        ))
    })?;
    let job = state
        .runtime
        .cron_job(routine_id.clone())
        .await
        .map_err(runtime_status_response)?
        .ok_or_else(|| {
            runtime_status_response(tonic::Status::not_found("linked routine job not found"))
        })?;
    let outcome = cron::trigger_job_now(
        Arc::clone(&state.runtime),
        state.auth.clone(),
        state.grpc_url.clone(),
        job,
        Arc::clone(&state.scheduler_wake),
    )
    .await
    .map_err(runtime_status_response)?;
    if let Some(run_id) = outcome.run_id.as_ref() {
        state
            .routines
            .upsert_run_metadata(RoutineRunMetadataUpsert {
                run_id: run_id.clone(),
                routine_id,
                trigger_kind: RoutineTriggerKind::Manual,
                trigger_reason: Some("objective fire".to_owned()),
                trigger_payload_json: json!({ "source": "objective" }).to_string(),
                trigger_dedupe_key: None,
                execution: RoutineExecutionConfig::default(),
                delivery: objective.automation.delivery.clone(),
                dispatch_mode: RoutineDispatchMode::Normal,
                source_run_id: None,
                outcome_override: Some(default_outcome_from_cron_status(outcome.status)),
                outcome_message: Some(outcome.message.clone()),
                output_delivered: Some(true),
                skip_reason: None,
                delivery_reason: None,
                approval_note: None,
                safety_note: None,
            })
            .map_err(routine_registry_error_response)?;
    }
    Ok(json!({
        "run_id": outcome.run_id,
        "status": outcome.status.as_str(),
        "message": outcome.message,
    }))
}

async fn apply_fire_action(
    state: &AppState,
    objective: &mut ObjectiveRecord,
    reason: Option<String>,
) -> Result<(), Response> {
    if matches!(objective.state, ObjectiveState::Cancelled | ObjectiveState::Archived) {
        return Err(runtime_status_response(tonic::Status::failed_precondition(
            "cancelled or archived objectives cannot be fired",
        )));
    }
    if !objective.automation.enabled {
        return Err(runtime_status_response(tonic::Status::failed_precondition(
            "objective automation is paused or disabled",
        )));
    }
    let from_state = objective.state;
    let now_unix_ms = unix_ms_now().map_err(internal_console_error)?;
    let outcome = trigger_objective_now(state, objective).await?;
    objective.state = ObjectiveState::Active;
    if let Some(run_id) = outcome.get("run_id").and_then(Value::as_str) {
        objective.linked_run_ids.push(run_id.to_owned());
        let attempt = ObjectiveAttemptRecord {
            attempt_id: Ulid::new().to_string(),
            run_id: Some(run_id.to_owned()),
            session_id: None,
            status: outcome.get("status").and_then(Value::as_str).unwrap_or("scheduled").to_owned(),
            outcome_kind: outcome.get("status").and_then(Value::as_str).map(ToOwned::to_owned),
            summary: outcome
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("objective dispatched")
                .to_owned(),
            learned: None,
            recommended_next_step: objective.next_recommended_step.clone(),
            created_at_unix_ms: now_unix_ms,
            completed_at_unix_ms: None,
        };
        objective.last_attempt = Some(attempt.clone());
        objective.attempt_history.push(attempt);
    }
    objective.lifecycle_history.push(ObjectiveLifecycleRecord {
        event_id: Ulid::new().to_string(),
        action: "fire".to_owned(),
        from_state: Some(from_state),
        to_state: ObjectiveState::Active,
        reason,
        run_id: outcome.get("run_id").and_then(Value::as_str).map(ToOwned::to_owned),
        occurred_at_unix_ms: now_unix_ms,
    });
    Ok(())
}

async fn apply_pause_action(
    state: &AppState,
    objective: &mut ObjectiveRecord,
    reason: Option<String>,
) -> Result<(), Response> {
    let from_state = objective.state;
    objective.state = ObjectiveState::Paused;
    objective.automation.enabled = false;
    set_objective_job_enabled(state, objective, false).await?;
    objective.lifecycle_history.push(ObjectiveLifecycleRecord {
        event_id: Ulid::new().to_string(),
        action: "pause".to_owned(),
        from_state: Some(from_state),
        to_state: ObjectiveState::Paused,
        reason,
        run_id: None,
        occurred_at_unix_ms: unix_ms_now().map_err(internal_console_error)?,
    });
    Ok(())
}

async fn apply_resume_action(
    state: &AppState,
    objective: &mut ObjectiveRecord,
    reason: Option<String>,
) -> Result<(), Response> {
    if objective.state == ObjectiveState::Archived {
        return Err(runtime_status_response(tonic::Status::failed_precondition(
            "archived objectives cannot be resumed",
        )));
    }
    let from_state = objective.state;
    objective.state = ObjectiveState::Active;
    objective.automation.enabled = true;
    set_objective_job_enabled(state, objective, true).await?;
    objective.lifecycle_history.push(ObjectiveLifecycleRecord {
        event_id: Ulid::new().to_string(),
        action: "resume".to_owned(),
        from_state: Some(from_state),
        to_state: ObjectiveState::Active,
        reason,
        run_id: None,
        occurred_at_unix_ms: unix_ms_now().map_err(internal_console_error)?,
    });
    Ok(())
}

async fn apply_cancel_action(
    state: &AppState,
    objective: &mut ObjectiveRecord,
    reason: Option<String>,
) -> Result<(), Response> {
    let from_state = objective.state;
    objective.state = ObjectiveState::Cancelled;
    objective.automation.enabled = false;
    set_objective_job_enabled(state, objective, false).await?;
    if let Some(run_id) = objective.linked_run_ids.last() {
        let _ = state
            .runtime
            .request_orchestrator_cancel(OrchestratorCancelRequest {
                run_id: run_id.clone(),
                reason: reason.clone().unwrap_or_else(|| "objective cancelled".to_owned()),
            })
            .await;
    }
    objective.lifecycle_history.push(ObjectiveLifecycleRecord {
        event_id: Ulid::new().to_string(),
        action: "cancel".to_owned(),
        from_state: Some(from_state),
        to_state: ObjectiveState::Cancelled,
        reason,
        run_id: objective.linked_run_ids.last().cloned(),
        occurred_at_unix_ms: unix_ms_now().map_err(internal_console_error)?,
    });
    Ok(())
}

async fn apply_archive_action(
    state: &AppState,
    objective: &mut ObjectiveRecord,
    reason: Option<String>,
) -> Result<(), Response> {
    let from_state = objective.state;
    objective.state = ObjectiveState::Archived;
    objective.automation.enabled = false;
    set_objective_job_enabled(state, objective, false).await?;
    objective.lifecycle_history.push(ObjectiveLifecycleRecord {
        event_id: Ulid::new().to_string(),
        action: "archive".to_owned(),
        from_state: Some(from_state),
        to_state: ObjectiveState::Archived,
        reason,
        run_id: None,
        occurred_at_unix_ms: unix_ms_now().map_err(internal_console_error)?,
    });
    Ok(())
}

async fn preflight_objective_workspace_projection(
    state: &AppState,
    objective: &ObjectiveRecord,
) -> Result<(), Response> {
    validate_objective_document_projection(state, objective).await?;
    validate_owner_objective_blocks_projection(state, objective).await
}

async fn validate_objective_document_projection(
    state: &AppState,
    objective: &ObjectiveRecord,
) -> Result<(), Response> {
    let current = state
        .runtime
        .workspace_document_by_path(
            objective.owner_principal.clone(),
            objective.channel.clone(),
            None,
            objective.workspace.workspace_document_path.clone(),
            false,
        )
        .await
        .map_err(runtime_status_response)?;
    let content = current
        .as_ref()
        .map(|entry| entry.content_text.as_str())
        .unwrap_or_else(|| objective_document_heading(objective.kind));
    sync_workspace_managed_block(content, &objective_record_block(objective))
        .map_err(objective_workspace_error_response)?;
    Ok(())
}

async fn validate_owner_objective_blocks_projection(
    state: &AppState,
    objective: &ObjectiveRecord,
) -> Result<(), Response> {
    let mut objectives =
        state.objectives.list_objectives().map_err(objective_registry_error_response)?;
    if let Some(existing) =
        objectives.iter_mut().find(|entry| entry.objective_id == objective.objective_id)
    {
        *existing = objective.clone();
    } else {
        objectives.push(objective.clone());
    }
    let owner_objectives = objectives
        .into_iter()
        .filter(|entry| entry.owner_principal == objective.owner_principal)
        .collect::<Vec<_>>();
    for (path, update) in owner_objective_block_updates(&owner_objectives) {
        let current = state
            .runtime
            .workspace_document_by_path(
                objective.owner_principal.clone(),
                objective.channel.clone(),
                None,
                path.to_owned(),
                false,
            )
            .await
            .map_err(runtime_status_response)?;
        let existing_content =
            current.as_ref().map(|entry| entry.content_text.as_str()).unwrap_or_default();
        sync_workspace_managed_block(existing_content, &update)
            .map_err(objective_workspace_error_response)?;
    }
    Ok(())
}

fn apply_lifecycle_workspace_projection(
    action: &str,
    objective: &mut ObjectiveRecord,
) -> Result<(), tonic::Status> {
    match action {
        "fire" => {
            if matches!(objective.state, ObjectiveState::Cancelled | ObjectiveState::Archived) {
                return Err(tonic::Status::failed_precondition(
                    "cancelled or archived objectives cannot be fired",
                ));
            }
            if !objective.automation.enabled {
                return Err(tonic::Status::failed_precondition(
                    "objective automation is paused or disabled",
                ));
            }
            objective.state = ObjectiveState::Active;
        }
        "pause" => {
            objective.state = ObjectiveState::Paused;
            objective.automation.enabled = false;
        }
        "resume" => {
            if objective.state == ObjectiveState::Archived {
                return Err(tonic::Status::failed_precondition(
                    "archived objectives cannot be resumed",
                ));
            }
            objective.state = ObjectiveState::Active;
            objective.automation.enabled = true;
        }
        "cancel" => {
            objective.state = ObjectiveState::Cancelled;
            objective.automation.enabled = false;
        }
        "archive" => {
            objective.state = ObjectiveState::Archived;
            objective.automation.enabled = false;
        }
        _ => {
            return Err(tonic::Status::invalid_argument(
                "action must be one of fire|pause|resume|cancel|archive",
            ));
        }
    }
    Ok(())
}

async fn project_objective_workspace(
    state: &AppState,
    owner_principal: &str,
    channel: Option<&str>,
    _session_key: Option<&str>,
    objective: &ObjectiveRecord,
) -> Result<(), Response> {
    write_objective_document(state, owner_principal, channel, objective).await?;
    sync_owner_objective_blocks(state, owner_principal, channel).await
}

async fn write_objective_document(
    state: &AppState,
    owner_principal: &str,
    channel: Option<&str>,
    objective: &ObjectiveRecord,
) -> Result<(), Response> {
    let current = state
        .runtime
        .workspace_document_by_path(
            owner_principal.to_owned(),
            channel.map(ToOwned::to_owned),
            None,
            objective.workspace.workspace_document_path.clone(),
            false,
        )
        .await
        .map_err(runtime_status_response)?;
    let content = current
        .as_ref()
        .map(|entry| entry.content_text.as_str())
        .unwrap_or_else(|| objective_document_heading(objective.kind));
    let next_content = sync_workspace_managed_block(content, &objective_record_block(objective))
        .map_err(objective_workspace_error_response)?
        .content_text;
    state
        .runtime
        .upsert_workspace_document(WorkspaceDocumentWriteRequest {
            document_id: current.as_ref().map(|entry| entry.document_id.clone()),
            principal: owner_principal.to_owned(),
            channel: channel.map(ToOwned::to_owned),
            agent_id: None,
            session_id: None,
            path: objective.workspace.workspace_document_path.clone(),
            title: Some(objective.name.clone()),
            content_text: next_content,
            template_id: None,
            template_version: None,
            template_content_hash: None,
            source_memory_id: None,
            manual_override: false,
        })
        .await
        .map_err(runtime_status_response)?;
    Ok(())
}

async fn sync_owner_objective_blocks(
    state: &AppState,
    owner_principal: &str,
    channel: Option<&str>,
) -> Result<(), Response> {
    let objectives = state
        .objectives
        .list_objectives()
        .map_err(objective_registry_error_response)?
        .into_iter()
        .filter(|entry| entry.owner_principal == owner_principal)
        .collect::<Vec<_>>();
    for (path, update) in owner_objective_block_updates(&objectives) {
        sync_owner_block(state, owner_principal, channel, path, update).await?;
    }
    Ok(())
}

async fn sync_owner_block(
    state: &AppState,
    owner_principal: &str,
    channel: Option<&str>,
    path: &str,
    update: WorkspaceManagedBlockUpdate,
) -> Result<(), Response> {
    let current = state
        .runtime
        .workspace_document_by_path(
            owner_principal.to_owned(),
            channel.map(ToOwned::to_owned),
            None,
            path.to_owned(),
            false,
        )
        .await
        .map_err(runtime_status_response)?;
    let existing_content =
        current.as_ref().map(|entry| entry.content_text.as_str()).unwrap_or_default();
    let next_content = sync_workspace_managed_block(existing_content, &update)
        .map_err(objective_workspace_error_response)?
        .content_text;
    state
        .runtime
        .upsert_workspace_document(WorkspaceDocumentWriteRequest {
            document_id: current.as_ref().map(|entry| entry.document_id.clone()),
            principal: owner_principal.to_owned(),
            channel: channel.map(ToOwned::to_owned),
            agent_id: None,
            session_id: None,
            path: path.to_owned(),
            title: None,
            content_text: next_content,
            template_id: None,
            template_version: None,
            template_content_hash: None,
            source_memory_id: None,
            manual_override: false,
        })
        .await
        .map_err(runtime_status_response)?;
    Ok(())
}

fn managed_entry(entry_id: &str, content: String) -> WorkspaceManagedEntry {
    WorkspaceManagedEntry { entry_id: entry_id.to_owned(), label: "objective".to_owned(), content }
}

fn objective_document_heading(kind: ObjectiveKind) -> &'static str {
    if kind == ObjectiveKind::Heartbeat {
        "# Heartbeat Objective\n\nManual notes outside the managed block are preserved.\n"
    } else {
        "# Objective\n\nManual notes outside the managed block are preserved.\n"
    }
}

fn objective_record_block(objective: &ObjectiveRecord) -> WorkspaceManagedBlockUpdate {
    WorkspaceManagedBlockUpdate {
        block_id: "objective-record".to_owned(),
        heading: format!("{} Summary", objective.name),
        entries: vec![
            managed_entry("objective", format!("[{}] {}", objective.kind.as_str(), objective.name)),
            managed_entry("state", format!("state: {}", objective.state.as_str())),
            managed_entry(
                "focus",
                objective
                    .current_focus
                    .clone()
                    .unwrap_or_else(|| "No current focus recorded.".to_owned()),
            ),
            managed_entry(
                "success",
                objective
                    .success_criteria
                    .clone()
                    .unwrap_or_else(|| "No success criteria recorded.".to_owned()),
            ),
            managed_entry(
                "next_step",
                objective
                    .next_recommended_step
                    .clone()
                    .unwrap_or_else(|| "No next recommended step recorded.".to_owned()),
            ),
        ],
    }
}

fn owner_objective_block_updates(
    objectives: &[ObjectiveRecord],
) -> Vec<(&'static str, WorkspaceManagedBlockUpdate)> {
    vec![
        (
            "context/current-focus.md",
            WorkspaceManagedBlockUpdate {
                block_id: OBJECTIVE_FOCUS_BLOCK_ID.to_owned(),
                heading: "Objective Focus".to_owned(),
                entries: objectives
                    .iter()
                    .filter(|entry| {
                        !matches!(entry.state, ObjectiveState::Archived | ObjectiveState::Cancelled)
                    })
                    .map(|entry| {
                        managed_entry(
                            entry.objective_id.as_str(),
                            format!(
                                "[{}] {}: {}",
                                entry.state.as_str(),
                                entry.name,
                                entry
                                    .current_focus
                                    .clone()
                                    .unwrap_or_else(|| "No current focus recorded.".to_owned())
                            ),
                        )
                    })
                    .collect(),
            },
        ),
        (
            "HEARTBEAT.md",
            WorkspaceManagedBlockUpdate {
                block_id: OBJECTIVE_HEARTBEAT_BLOCK_ID.to_owned(),
                heading: "Objective Heartbeats".to_owned(),
                entries: objectives
                    .iter()
                    .filter(|entry| entry.kind == ObjectiveKind::Heartbeat)
                    .map(|entry| {
                        managed_entry(
                            entry.objective_id.as_str(),
                            format!(
                                "[{}] {} -> next step: {}",
                                entry.state.as_str(),
                                entry.name,
                                entry
                                    .next_recommended_step
                                    .clone()
                                    .unwrap_or_else(|| "No next step recorded.".to_owned())
                            ),
                        )
                    })
                    .collect(),
            },
        ),
        (
            "projects/inbox.md",
            WorkspaceManagedBlockUpdate {
                block_id: OBJECTIVE_INBOX_BLOCK_ID.to_owned(),
                heading: "Objective Inbox".to_owned(),
                entries: objectives
                    .iter()
                    .filter(|entry| {
                        matches!(
                            entry.state,
                            ObjectiveState::Draft | ObjectiveState::Active | ObjectiveState::Paused
                        )
                    })
                    .map(|entry| {
                        managed_entry(
                            entry.objective_id.as_str(),
                            format!(
                                "{} -> {}",
                                entry.name,
                                entry
                                    .next_recommended_step
                                    .clone()
                                    .unwrap_or_else(|| "Review the objective summary.".to_owned())
                            ),
                        )
                    })
                    .collect(),
            },
        ),
    ]
}

#[allow(clippy::result_large_err)]
fn normalize_lifecycle_reason(reason: Option<String>) -> Result<Option<String>, Response> {
    let Some(reason) = reason else {
        return Ok(None);
    };
    let trimmed = reason.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > 500 {
        return Err(validation_error_response(
            "reason",
            "too_large",
            "reason must be at most 500 bytes",
        ));
    }
    Ok(Some(trimmed.to_owned()))
}

fn normalize_budget(
    budget: Option<ConsoleObjectiveBudgetPayload>,
    existing: Option<&ObjectiveRecord>,
) -> ObjectiveBudget {
    let mut normalized = existing.map(|entry| entry.budget.clone()).unwrap_or_default();
    if let Some(payload) = budget {
        normalized.max_runs = payload.max_runs.or(normalized.max_runs);
        normalized.max_tokens = payload.max_tokens.or(normalized.max_tokens);
        normalized.notes = payload.notes.or(normalized.notes);
    }
    normalized
}

#[allow(clippy::result_large_err)]
fn parse_objective_kind(value: &str) -> Result<ObjectiveKind, Response> {
    ObjectiveKind::from_str(value).ok_or_else(|| {
        runtime_status_response(tonic::Status::invalid_argument(
            "kind must be one of objective|heartbeat|standing_order|program",
        ))
    })
}

#[allow(clippy::result_large_err)]
fn parse_objective_priority(value: Option<&str>) -> Result<ObjectivePriority, Response> {
    match value.map(str::trim).filter(|entry| !entry.is_empty()) {
        None => Ok(ObjectivePriority::Normal),
        Some(value) => ObjectivePriority::from_str(value).ok_or_else(|| {
            runtime_status_response(tonic::Status::invalid_argument(
                "priority must be one of low|normal|high|critical",
            ))
        }),
    }
}

#[allow(clippy::result_large_err)]
fn parse_approach_kind(value: &str) -> Result<ObjectiveApproachKind, Response> {
    match value.trim().to_ascii_lowercase().as_str() {
        "attempted" => Ok(ObjectiveApproachKind::Attempted),
        "learned" => Ok(ObjectiveApproachKind::Learned),
        "failed_approach" => Ok(ObjectiveApproachKind::FailedApproach),
        "recommended_next_step" => Ok(ObjectiveApproachKind::RecommendedNextStep),
        "standing_order" => Ok(ObjectiveApproachKind::StandingOrder),
        _ => Err(runtime_status_response(tonic::Status::invalid_argument(
            "kind must be one of attempted|learned|failed_approach|recommended_next_step|standing_order",
        ))),
    }
}

#[allow(clippy::result_large_err)]
fn normalize_owner_principal(
    requested: &Option<String>,
    session_principal: &str,
) -> Result<String, Response> {
    match requested.as_deref().map(str::trim) {
        Some("") | None => Ok(session_principal.to_owned()),
        Some(owner_principal) if owner_principal == session_principal => {
            Ok(owner_principal.to_owned())
        }
        Some(_) => Err(runtime_status_response(tonic::Status::permission_denied(
            "owner_principal must match authenticated session principal",
        ))),
    }
}

fn normalize_channel(requested: Option<&str>, session_channel: Option<&str>) -> String {
    requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| session_channel.map(ToOwned::to_owned))
        .unwrap_or_else(|| DEFAULT_OBJECTIVE_CHANNEL.to_owned())
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|entry| !entry.is_empty()).map(ToOwned::to_owned)
}

#[allow(clippy::result_large_err)]
fn ensure_objective_owner(objective: &ObjectiveRecord, principal: &str) -> Result<(), Response> {
    if objective.owner_principal != principal {
        return Err(runtime_status_response(tonic::Status::permission_denied(
            "objective owner mismatch for authenticated principal",
        )));
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
fn resolve_objective_schedule(
    payload: &ConsoleObjectiveUpsertRequest,
    kind: ObjectiveKind,
    timezone_mode: CronTimezoneMode,
) -> Result<ObjectiveScheduleResolution, Response> {
    let natural_language_schedule = payload
        .natural_language_schedule
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(phrase) = natural_language_schedule {
        let preview = natural_language_schedule_preview(
            phrase,
            timezone_mode,
            unix_ms_now().map_err(internal_console_error)?,
        )
        .map_err(routine_registry_error_response)?;
        return Ok(ObjectiveScheduleResolution {
            trigger_kind: RoutineTriggerKind::Schedule,
            schedule_type: parse_schedule_type(preview.schedule_type.as_str())?,
            schedule_payload_json: preview.schedule_payload_json,
            next_run_at_unix_ms: preview.next_run_at_unix_ms,
            is_scheduled: true,
        });
    }
    let schedule_type =
        payload.schedule_type.as_deref().map(str::trim).filter(|value| !value.is_empty());
    if let Some(schedule_type) = schedule_type {
        let schedule = build_console_schedule(schedule_type, payload)?;
        let normalized = cron::normalize_schedule(
            Some(schedule),
            unix_ms_now().map_err(internal_console_error)?,
            timezone_mode,
        )
        .map_err(runtime_status_response)?;
        return Ok(ObjectiveScheduleResolution {
            trigger_kind: RoutineTriggerKind::Schedule,
            schedule_type: normalized.schedule_type,
            schedule_payload_json: normalized.schedule_payload_json,
            next_run_at_unix_ms: normalized.next_run_at_unix_ms,
            is_scheduled: true,
        });
    }
    if kind == ObjectiveKind::Heartbeat {
        let normalized = cron::normalize_schedule(
            Some(cron_v1::Schedule {
                r#type: cron_v1::ScheduleType::Every as i32,
                spec: Some(cron_v1::schedule::Spec::Every(cron_v1::EverySchedule {
                    interval_ms: payload.every_interval_ms.unwrap_or(60 * 60 * 1_000),
                })),
            }),
            unix_ms_now().map_err(internal_console_error)?,
            timezone_mode,
        )
        .map_err(runtime_status_response)?;
        return Ok(ObjectiveScheduleResolution {
            trigger_kind: RoutineTriggerKind::Schedule,
            schedule_type: normalized.schedule_type,
            schedule_payload_json: normalized.schedule_payload_json,
            next_run_at_unix_ms: normalized.next_run_at_unix_ms,
            is_scheduled: true,
        });
    }
    Ok(ObjectiveScheduleResolution {
        trigger_kind: RoutineTriggerKind::Manual,
        schedule_type: CronScheduleType::At,
        schedule_payload_json: shadow_manual_schedule_payload_json(),
        next_run_at_unix_ms: None,
        is_scheduled: false,
    })
}

#[allow(clippy::result_large_err)]
fn build_console_schedule(
    schedule_type_raw: &str,
    payload: &ConsoleObjectiveUpsertRequest,
) -> Result<cron_v1::Schedule, Response> {
    match schedule_type_raw.trim().to_ascii_lowercase().as_str() {
        "cron" => {
            let expression = payload
                .cron_expression
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    runtime_status_response(tonic::Status::invalid_argument(
                        "cron_expression is required for schedule_type=cron",
                    ))
                })?;
            Ok(cron_v1::Schedule {
                r#type: cron_v1::ScheduleType::Cron as i32,
                spec: Some(cron_v1::schedule::Spec::Cron(cron_v1::CronSchedule {
                    expression: expression.to_owned(),
                })),
            })
        }
        "every" => Ok(cron_v1::Schedule {
            r#type: cron_v1::ScheduleType::Every as i32,
            spec: Some(cron_v1::schedule::Spec::Every(cron_v1::EverySchedule {
                interval_ms: payload.every_interval_ms.ok_or_else(|| {
                    runtime_status_response(tonic::Status::invalid_argument(
                        "every_interval_ms is required for schedule_type=every",
                    ))
                })?,
            })),
        }),
        "at" => {
            let timestamp = payload
                .at_timestamp_rfc3339
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    runtime_status_response(tonic::Status::invalid_argument(
                        "at_timestamp_rfc3339 is required for schedule_type=at",
                    ))
                })?;
            Ok(cron_v1::Schedule {
                r#type: cron_v1::ScheduleType::At as i32,
                spec: Some(cron_v1::schedule::Spec::At(cron_v1::AtSchedule {
                    timestamp_rfc3339: timestamp.to_owned(),
                })),
            })
        }
        _ => Err(runtime_status_response(tonic::Status::invalid_argument(
            "schedule_type must be one of cron|every|at",
        ))),
    }
}

#[allow(clippy::result_large_err)]
fn parse_schedule_type(value: &str) -> Result<CronScheduleType, Response> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cron" => Ok(CronScheduleType::Cron),
        "every" => Ok(CronScheduleType::Every),
        "at" => Ok(CronScheduleType::At),
        _ => Err(runtime_status_response(tonic::Status::invalid_argument(
            "schedule_type must be one of cron|every|at",
        ))),
    }
}

#[allow(clippy::result_large_err)]
fn parse_delivery(
    mode: Option<&str>,
    channel: Option<String>,
) -> Result<RoutineDeliveryConfig, Response> {
    let mode =
        match mode.map(str::trim).filter(|value| !value.is_empty()) {
            None => RoutineDeliveryMode::SameChannel,
            Some("same_channel") => RoutineDeliveryMode::SameChannel,
            Some("specific_channel") => RoutineDeliveryMode::SpecificChannel,
            Some("local_only") => RoutineDeliveryMode::LocalOnly,
            Some("logs_only") => RoutineDeliveryMode::LogsOnly,
            Some(_) => return Err(runtime_status_response(tonic::Status::invalid_argument(
                "delivery_mode must be one of same_channel|specific_channel|local_only|logs_only",
            ))),
        };
    let delivery_channel =
        channel.map(|value| value.trim().to_owned()).filter(|value| !value.is_empty());
    if mode == RoutineDeliveryMode::SpecificChannel && delivery_channel.is_none() {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "delivery_channel is required for delivery_mode=specific_channel",
        )));
    }
    Ok(RoutineDeliveryConfig {
        mode,
        channel: delivery_channel,
        failure_mode: None,
        failure_channel: None,
        silent_policy: RoutineSilentPolicy::Noisy,
    })
}

#[allow(clippy::result_large_err)]
fn parse_quiet_hours(
    start: Option<&str>,
    end: Option<&str>,
    timezone: Option<String>,
) -> Result<Option<RoutineQuietHours>, Response> {
    let start = start.map(str::trim).filter(|value| !value.is_empty());
    let end = end.map(str::trim).filter(|value| !value.is_empty());
    match (start, end) {
        (None, None) => Ok(None),
        (Some(start), Some(end)) => Ok(Some(RoutineQuietHours {
            start_minute_of_day: parse_minute_of_day(start)?,
            end_minute_of_day: parse_minute_of_day(end)?,
            timezone: normalize_optional_text(timezone.as_deref()),
        })),
        _ => Err(runtime_status_response(tonic::Status::invalid_argument(
            "quiet_hours_start and quiet_hours_end must be provided together",
        ))),
    }
}

#[allow(clippy::result_large_err)]
fn parse_minute_of_day(value: &str) -> Result<u16, Response> {
    let (hour, minute) = value.split_once(':').ok_or_else(|| {
        runtime_status_response(tonic::Status::invalid_argument(
            "quiet hour values must use HH:MM format",
        ))
    })?;
    let hour = hour.parse::<u16>().map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument(
            "quiet hour hour component must be numeric",
        ))
    })?;
    let minute = minute.parse::<u16>().map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument(
            "quiet hour minute component must be numeric",
        ))
    })?;
    if hour > 23 || minute > 59 {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "quiet hour values must stay within 00:00-23:59",
        )));
    }
    Ok(hour * 60 + minute)
}

#[allow(clippy::result_large_err)]
fn parse_approval_policy(value: Option<&str>) -> Result<RoutineApprovalPolicy, Response> {
    let mode = match value.map(str::trim).filter(|entry| !entry.is_empty()) {
        None | Some("none") => RoutineApprovalMode::None,
        Some("before_enable") => RoutineApprovalMode::BeforeEnable,
        Some("before_first_run") => RoutineApprovalMode::BeforeFirstRun,
        Some(_) => {
            return Err(runtime_status_response(tonic::Status::invalid_argument(
                "approval_mode must be one of none|before_enable|before_first_run",
            )))
        }
    };
    Ok(RoutineApprovalPolicy { mode })
}

fn render_objective_summary_markdown(view: &Value) -> String {
    let objective_id = read_string_value(view, "objective_id");
    let kind = read_string_value(view, "kind");
    let name = read_string_value(view, "name");
    let state = read_string_value(view, "state");
    let current_focus =
        view.get("current_focus").and_then(Value::as_str).unwrap_or("No current focus recorded.");
    let success_criteria = view
        .get("success_criteria")
        .and_then(Value::as_str)
        .unwrap_or("No success criteria recorded.");
    let next_step = view
        .get("next_recommended_step")
        .and_then(Value::as_str)
        .unwrap_or("No next recommended step recorded.");
    let health = view
        .get("health")
        .and_then(|entry| entry.get("state"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!(
        "# {name}\n\n- objective_id: {objective_id}\n- kind: {kind}\n- state: {state}\n- health: {health}\n- current_focus: {current_focus}\n- success_criteria: {success_criteria}\n- next_recommended_step: {next_step}\n"
    )
}

fn read_string_value(record: &Value, key: &str) -> String {
    record.get(key).and_then(Value::as_str).unwrap_or_default().to_owned()
}

fn objective_registry_error_response(error: ObjectiveRegistryError) -> Response {
    match error {
        ObjectiveRegistryError::InvalidField { field, message } => {
            validation_error_response(field, "invalid", message.as_str())
        }
        other => runtime_status_response(tonic::Status::internal(other.to_string())),
    }
}

fn routine_registry_error_response(error: crate::routines::RoutineRegistryError) -> Response {
    match error {
        crate::routines::RoutineRegistryError::InvalidField { field, message } => {
            validation_error_response(field, "invalid", message.as_str())
        }
        other => runtime_status_response(tonic::Status::internal(other.to_string())),
    }
}

fn objective_workspace_error_response(
    error: crate::domain::workspace::WorkspaceManagedBlockError,
) -> Response {
    runtime_status_response(tonic::Status::failed_precondition(format!(
        "objective workspace projection blocked before lifecycle mutation: {error}. Repair the affected Palyra managed block by removing manual edits between PALYRA markers or delete the malformed managed block, then retry the objective command."
    )))
}

fn internal_console_error(error: anyhow::Error) -> Response {
    runtime_status_response(tonic::Status::internal(sanitize_http_error_message(
        error.to_string().as_str(),
    )))
}

#[cfg(test)]
mod tests {
    use super::{
        apply_lifecycle_workspace_projection, compute_objective_health, managed_entry,
        normalize_budget, normalize_lifecycle_reason, objective_record_block,
        owner_objective_block_updates, parse_objective_kind, parse_objective_priority,
        render_objective_summary_markdown, ConsoleObjectiveBudgetPayload,
    };
    use crate::domain::workspace::{
        sync_workspace_managed_block, WorkspaceManagedBlockError, WorkspaceManagedBlockUpdate,
    };
    use crate::objectives::{
        ObjectiveAutomationBinding, ObjectiveBudget, ObjectiveKind, ObjectivePriority,
        ObjectiveRecord, ObjectiveState, ObjectiveWorkspaceBinding,
    };
    use crate::routines::{
        shadow_manual_schedule_payload_json, RoutineApprovalPolicy, RoutineDeliveryConfig,
        RoutineTriggerKind,
    };
    use serde_json::json;
    use ulid::Ulid;

    fn sample_objective() -> ObjectiveRecord {
        ObjectiveRecord {
            objective_id: Ulid::new().to_string(),
            kind: ObjectiveKind::Heartbeat,
            state: ObjectiveState::Active,
            name: "Daily heartbeat".to_owned(),
            prompt: "Summarize the current focus every morning.".to_owned(),
            owner_principal: "user:ops".to_owned(),
            channel: Some("console".to_owned()),
            priority: ObjectivePriority::Normal,
            budget: ObjectiveBudget::default(),
            current_focus: Some("Keep the board current.".to_owned()),
            success_criteria: Some("Operators can see the latest summary.".to_owned()),
            exit_condition: None,
            next_recommended_step: Some("Review the newest summary.".to_owned()),
            standing_order: None,
            workspace: ObjectiveWorkspaceBinding {
                workspace_document_path: "projects/objectives/demo.md".to_owned(),
                session_key: None,
                session_label: None,
                related_document_paths: vec![],
                related_memory_ids: vec![],
                related_session_ids: vec![],
            },
            automation: ObjectiveAutomationBinding {
                routine_id: Some(Ulid::new().to_string()),
                enabled: true,
                trigger_kind: RoutineTriggerKind::Schedule,
                schedule_type: "every".to_owned(),
                schedule_payload_json: shadow_manual_schedule_payload_json(),
                delivery: RoutineDeliveryConfig::default(),
                quiet_hours: None,
                cooldown_ms: 0,
                approval_policy: RoutineApprovalPolicy::default(),
                template_id: Some("heartbeat".to_owned()),
            },
            last_attempt: None,
            attempt_history: vec![],
            approach_history: vec![],
            lifecycle_history: vec![],
            linked_run_ids: vec![],
            linked_artifact_paths: vec![],
            created_at_unix_ms: 1,
            updated_at_unix_ms: 1,
            archived_at_unix_ms: None,
        }
    }

    #[test]
    fn kind_parser_accepts_product_terms() {
        assert_eq!(
            parse_objective_kind("standing_order").expect("kind should parse"),
            ObjectiveKind::StandingOrder
        );
    }

    #[test]
    fn priority_parser_defaults_to_normal() {
        assert_eq!(
            parse_objective_priority(None).expect("priority should default"),
            ObjectivePriority::Normal
        );
    }

    #[test]
    fn budget_merge_overrides_existing_fields() {
        let existing = sample_objective();
        let merged = normalize_budget(
            Some(ConsoleObjectiveBudgetPayload {
                max_runs: Some(7),
                max_tokens: None,
                notes: Some("Watch cost.".to_owned()),
            }),
            Some(&existing),
        );
        assert_eq!(merged.max_runs, Some(7));
        assert_eq!(merged.notes.as_deref(), Some("Watch cost."));
    }

    #[test]
    fn objective_summary_markdown_surfaces_core_fields() {
        let markdown = render_objective_summary_markdown(&json!({
            "objective_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "kind": "heartbeat",
            "name": "Daily heartbeat",
            "state": "active",
            "current_focus": "Keep the board current.",
            "success_criteria": "Operators can see the latest summary.",
            "next_recommended_step": "Review the newest summary.",
            "health": { "state": "healthy" }
        }));
        assert!(markdown.contains("Daily heartbeat"));
        assert!(markdown.contains("health: healthy"));
    }

    #[test]
    fn health_uses_last_run_status_for_active_objectives() {
        let objective = sample_objective();
        let health = compute_objective_health(
            &objective,
            Some(&json!({ "next_run_at_unix_ms": 10 })),
            Some(&json!({ "status": "failed" })),
        );
        assert_eq!(health.get("state").and_then(serde_json::Value::as_str), Some("attention"));
    }

    #[test]
    fn managed_entry_uses_objective_label() {
        let entry = managed_entry("demo", "Track the next step".to_owned());
        assert_eq!(entry.label, "objective");
    }

    #[test]
    fn lifecycle_projection_is_pure_before_side_effects() {
        let mut objective = sample_objective();
        apply_lifecycle_workspace_projection("pause", &mut objective)
            .expect("pause projection should succeed");
        assert_eq!(objective.state, ObjectiveState::Paused);
        assert!(!objective.automation.enabled);
        assert!(objective.lifecycle_history.is_empty());
    }

    #[test]
    fn lifecycle_reason_is_normalized_before_mutation() {
        assert_eq!(
            normalize_lifecycle_reason(Some("  operator pause  ".to_owned()))
                .expect("reason should normalize")
                .as_deref(),
            Some("operator pause")
        );
        assert!(
            normalize_lifecycle_reason(Some("x".repeat(501))).is_err(),
            "oversized reasons must fail before lifecycle side effects"
        );
    }

    #[test]
    fn objective_projection_rejects_malformed_managed_block() {
        let objective = sample_objective();
        let malformed = "\
# Heartbeat Objective

## Daily heartbeat Summary
<!-- PALYRA:BEGIN objective-record -->
manual edit inside managed block
<!-- PALYRA:END objective-record -->
";
        let error = sync_workspace_managed_block(malformed, &objective_record_block(&objective))
            .expect_err("malformed managed content should block projection");
        assert!(matches!(error, WorkspaceManagedBlockError::MalformedItem { .. }));
    }

    #[test]
    fn owner_projection_blocks_cover_expected_workspace_docs() {
        let objective = sample_objective();
        let updates = owner_objective_block_updates(&[objective]);
        let paths = updates.iter().map(|(path, _)| *path).collect::<Vec<_>>();
        assert_eq!(paths, vec!["context/current-focus.md", "HEARTBEAT.md", "projects/inbox.md"]);
        assert!(
            updates.iter().all(|(_, update): &(&str, WorkspaceManagedBlockUpdate)| {
                !update.block_id.trim().is_empty()
            }),
            "all owner projection updates should target a managed block"
        );
    }
}
