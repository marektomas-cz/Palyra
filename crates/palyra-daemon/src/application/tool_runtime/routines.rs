use std::{collections::HashMap, sync::Arc};

use chrono::{TimeZone, Timelike};
use palyra_common::validate_canonical_id;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tonic::Status;
use ulid::Ulid;

use crate::{
    cron::{self, CronTimezoneMode},
    gateway::{
        current_unix_ms, GatewayRuntimeState, ToolRuntimeExecutionContext,
        ROUTINES_CONTROL_TOOL_NAME, ROUTINES_QUERY_TOOL_NAME,
    },
    journal::{
        ApprovalCreateRequest, ApprovalDecision, ApprovalDecisionScope, ApprovalPolicySnapshot,
        ApprovalPromptOption, ApprovalPromptRecord, ApprovalRiskLevel, ApprovalSubjectType,
        CronConcurrencyPolicy, CronJobCreateRequest, CronJobRecord, CronJobUpdatePatch,
        CronMisfirePolicy, CronRetryPolicy, CronRunFinalizeRequest, CronRunStartRequest,
        CronRunStatus,
    },
    routines::{
        default_outcome_from_cron_status, join_run_metadata, natural_language_schedule_preview,
        routine_delivery_preview, shadow_manual_schedule_payload_json,
        validate_routine_prompt_self_contained, RoutineApprovalMode, RoutineApprovalPolicy,
        RoutineDeliveryConfig, RoutineDeliveryMode, RoutineDispatchMode, RoutineExecutionConfig,
        RoutineExecutionPosture, RoutineMetadataRecord, RoutineMetadataUpsert, RoutineQuietHours,
        RoutineRegistry, RoutineRegistryError, RoutineRunMetadataUpsert, RoutineRunMode,
        RoutineRunOutcomeKind, RoutineSilentPolicy, RoutineTriggerKind,
    },
    tool_protocol::{ToolAttestation, ToolExecutionOutcome},
    transport::grpc::proto::palyra::cron::v1 as cron_v1,
};

const DEFAULT_ROUTINE_CHANNEL: &str = "system:routines";
const DEFAULT_ROUTINE_PAGE_LIMIT: usize = 100;
const MAX_ROUTINE_PAGE_LIMIT: usize = 500;
const ROUTINE_APPROVAL_TIMEOUT_SECONDS: u32 = 900;
const ROUTINE_APPROVAL_DEVICE_ID: &str = "system:routines";
const MAX_ROUTINES_QUERY_TOOL_INPUT_BYTES: usize = 64 * 1024;
const MAX_ROUTINES_CONTROL_TOOL_INPUT_BYTES: usize = 128 * 1024;

pub(crate) async fn execute_routines_tool(
    runtime_state: &Arc<GatewayRuntimeState>,
    context: ToolRuntimeExecutionContext<'_>,
    tool_name: &str,
    proposal_id: &str,
    input_json: &[u8],
) -> ToolExecutionOutcome {
    let max_input_bytes = if tool_name == ROUTINES_CONTROL_TOOL_NAME {
        MAX_ROUTINES_CONTROL_TOOL_INPUT_BYTES
    } else {
        MAX_ROUTINES_QUERY_TOOL_INPUT_BYTES
    };
    if input_json.len() > max_input_bytes {
        return routines_tool_execution_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!("{tool_name} input exceeds {max_input_bytes} bytes"),
        );
    }

    let payload = match parse_object_input(tool_name, input_json) {
        Ok(value) => value,
        Err(error) => {
            return routines_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                error,
            );
        }
    };
    let runtime = match runtime_state.routines_runtime_config() {
        Ok(config) => config,
        Err(error) => {
            return routines_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                sanitize_status_message(error),
            );
        }
    };

    let owned_context = RoutinesToolContext {
        principal: context.principal.to_owned(),
        channel: context.channel.map(str::to_owned),
        session_id: context.session_id.to_owned(),
        run_id: context.run_id.to_owned(),
    };

    let result = if tool_name == ROUTINES_QUERY_TOOL_NAME {
        execute_query_operation(
            runtime_state,
            &runtime.registry,
            runtime.timezone_mode,
            &owned_context,
            payload,
        )
        .await
    } else {
        execute_control_operation(runtime_state, &runtime, &owned_context, payload).await
    };

    match result {
        Ok(output) => match serde_json::to_vec(&output) {
            Ok(output_json) => routines_tool_execution_outcome(
                proposal_id,
                input_json,
                true,
                output_json,
                String::new(),
            ),
            Err(error) => routines_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!("failed to encode routines tool output: {error}"),
            ),
        },
        Err(error) => {
            routines_tool_execution_outcome(proposal_id, input_json, false, b"{}".to_vec(), error)
        }
    }
}

#[derive(Debug, Clone)]
struct RoutinesToolContext {
    principal: String,
    channel: Option<String>,
    session_id: String,
    run_id: String,
}

#[derive(Debug, Clone)]
struct RoutineParts {
    job: CronJobRecord,
    metadata: RoutineMetadataRecord,
}

#[derive(Debug, Clone)]
struct RoutineJobUpsert {
    routine_id: String,
    name: String,
    prompt: String,
    owner_principal: String,
    channel: String,
    session_key: Option<String>,
    session_label: Option<String>,
    schedule_type: crate::journal::CronScheduleType,
    schedule_payload_json: String,
    enabled: bool,
    concurrency_policy: CronConcurrencyPolicy,
    retry_policy: CronRetryPolicy,
    misfire_policy: CronMisfirePolicy,
    jitter_ms: u64,
    next_run_at_unix_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct ScheduleResolution {
    schedule_type: crate::journal::CronScheduleType,
    schedule_payload_json: String,
    next_run_at_unix_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct RoutineDispatchRequest {
    trigger_kind: RoutineTriggerKind,
    trigger_reason: Option<String>,
    trigger_payload: Value,
    trigger_dedupe_key: Option<String>,
    dispatch_mode: RoutineDispatchMode,
    source_run_id: Option<String>,
    execution_override: Option<RoutineExecutionConfig>,
    delivery_override: Option<RoutineDeliveryConfig>,
    approval_note: Option<String>,
    safety_note: Option<String>,
    bypass_operator_gates: bool,
}

struct TerminalRoutineRunRequest<'a> {
    routine_id: &'a str,
    trigger_kind: RoutineTriggerKind,
    trigger_reason: Option<String>,
    trigger_payload: &'a Value,
    trigger_dedupe_key: Option<String>,
    execution: RoutineExecutionConfig,
    delivery: RoutineDeliveryConfig,
    dispatch_mode: RoutineDispatchMode,
    source_run_id: Option<String>,
    status: CronRunStatus,
    outcome_override: RoutineRunOutcomeKind,
    message: &'a str,
    skip_reason: Option<String>,
    delivery_reason: Option<String>,
    approval_note: Option<String>,
    safety_note: Option<String>,
}

async fn execute_query_operation(
    runtime_state: &Arc<GatewayRuntimeState>,
    registry: &Arc<RoutineRegistry>,
    timezone_mode: CronTimezoneMode,
    context: &RoutinesToolContext,
    payload: Map<String, Value>,
) -> Result<Value, String> {
    let operation =
        optional_string_field(&payload, "operation").unwrap_or_else(|| "list".to_owned());
    match operation.as_str() {
        "list" => list_routines(runtime_state, registry, context, &payload).await,
        "get" => get_routine(runtime_state, registry, context, &payload).await,
        "list_runs" => list_routine_runs(runtime_state, registry, context, &payload).await,
        "schedule_preview" => preview_schedule(&payload, timezone_mode),
        _ => Err(
            "palyra.routines.query operation must be one of list|get|list_runs|schedule_preview"
                .to_owned(),
        ),
    }
}

async fn execute_control_operation(
    runtime_state: &Arc<GatewayRuntimeState>,
    runtime: &crate::gateway::RoutinesRuntimeConfig,
    context: &RoutinesToolContext,
    payload: Map<String, Value>,
) -> Result<Value, String> {
    let operation = required_string_field(&payload, "operation")?;
    match operation.as_str() {
        "upsert" => upsert_routine(runtime_state, runtime, context, &payload).await,
        "pause" => set_routine_enabled(runtime_state, runtime, context, &payload, false).await,
        "resume" => set_routine_enabled(runtime_state, runtime, context, &payload, true).await,
        "run_now" => run_routine_now(runtime_state, runtime, context, &payload).await,
        "test_run" => test_run_routine(runtime_state, runtime, context, &payload).await,
        _ => Err(
            "palyra.routines.control operation must be one of upsert|pause|resume|run_now|test_run"
                .to_owned(),
        ),
    }
}

async fn list_routines(
    runtime_state: &Arc<GatewayRuntimeState>,
    registry: &Arc<RoutineRegistry>,
    context: &RoutinesToolContext,
    payload: &Map<String, Value>,
) -> Result<Value, String> {
    let jobs = synchronize_schedule_routines(runtime_state, registry).await?;
    let mut filtered =
        routine_views_for_principal(runtime_state, registry, jobs, context.principal.as_str())
            .await?;
    let limit = optional_usize_field(payload, "limit")?
        .unwrap_or(DEFAULT_ROUTINE_PAGE_LIMIT)
        .clamp(1, MAX_ROUTINE_PAGE_LIMIT);
    let trigger_kind_filter =
        optional_string_field(payload, "trigger_kind").map(|value| value.to_ascii_lowercase());
    let enabled_filter = optional_bool_field(payload, "enabled")?;
    let channel_filter = optional_string_field(payload, "channel");
    let template_filter = optional_string_field(payload, "template_id");
    filtered.retain(|routine| {
        trigger_kind_filter.as_ref().is_none_or(|expected| {
            routine
                .get("trigger_kind")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(expected.as_str()))
        }) && enabled_filter.is_none_or(|enabled| {
            routine.get("enabled").and_then(Value::as_bool).unwrap_or(false) == enabled
        }) && channel_filter.as_ref().is_none_or(|expected| {
            routine
                .get("channel")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(expected.as_str()))
        }) && template_filter.as_ref().is_none_or(|expected| {
            routine
                .get("template_id")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(expected.as_str()))
        })
    });
    if let Some(after_routine_id) = optional_string_field(payload, "after_routine_id") {
        filtered.retain(|routine| {
            read_string_value(routine, "routine_id").as_str() > after_routine_id.as_str()
        });
    }
    let has_more = filtered.len() > limit;
    if has_more {
        filtered.truncate(limit);
    }
    let next_after_routine_id = if has_more {
        filtered
            .last()
            .and_then(|routine| routine.get("routine_id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    } else {
        None
    };
    Ok(json!({
        "operation": "list",
        "routines": filtered,
        "next_after_routine_id": next_after_routine_id,
    }))
}

async fn get_routine(
    runtime_state: &Arc<GatewayRuntimeState>,
    registry: &Arc<RoutineRegistry>,
    context: &RoutinesToolContext,
    payload: &Map<String, Value>,
) -> Result<Value, String> {
    let routine_id = required_string_field(payload, "routine_id")?;
    let routine = load_routine_view_for_owner(
        runtime_state,
        registry,
        routine_id.as_str(),
        context.principal.as_str(),
    )
    .await?;
    Ok(json!({
        "operation": "get",
        "routine": routine,
    }))
}

async fn list_routine_runs(
    runtime_state: &Arc<GatewayRuntimeState>,
    registry: &Arc<RoutineRegistry>,
    context: &RoutinesToolContext,
    payload: &Map<String, Value>,
) -> Result<Value, String> {
    let routine_id = required_string_field(payload, "routine_id")?;
    let routine = load_routine_parts_for_owner(
        runtime_state,
        registry,
        routine_id.as_str(),
        context.principal.as_str(),
    )
    .await?;
    let limit = optional_usize_field(payload, "limit")?
        .unwrap_or(DEFAULT_ROUTINE_PAGE_LIMIT)
        .clamp(1, MAX_ROUTINE_PAGE_LIMIT);
    let (runs, next_after_run_id) = runtime_state
        .list_cron_runs(
            Some(routine.job.job_id.clone()),
            optional_string_field(payload, "after_run_id"),
            Some(limit),
        )
        .await
        .map_err(sanitize_status_message)?;
    let mapped_runs = runs
        .iter()
        .map(|run| {
            let metadata =
                registry.find_run_metadata(run.run_id.as_str()).map_err(map_registry_error)?;
            Ok::<_, String>(join_run_metadata(
                routine.metadata.routine_id.as_str(),
                run,
                metadata.as_ref(),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "operation": "list_runs",
        "routine_id": routine.metadata.routine_id,
        "runs": mapped_runs,
        "next_after_run_id": next_after_run_id,
    }))
}

fn preview_schedule(
    payload: &Map<String, Value>,
    timezone_mode: CronTimezoneMode,
) -> Result<Value, String> {
    let phrase = required_string_field(payload, "phrase")?;
    let timezone = match optional_string_field(payload, "timezone").as_deref() {
        Some("utc") => CronTimezoneMode::Utc,
        Some("local") => CronTimezoneMode::Local,
        None => timezone_mode,
        Some(_) => return Err("timezone must be one of local|utc".to_owned()),
    };
    let preview =
        natural_language_schedule_preview(phrase.as_str(), timezone, unix_ms_now_string_safe()?)
            .map_err(map_registry_error)?;
    Ok(json!({
        "operation": "schedule_preview",
        "preview": preview,
    }))
}

async fn upsert_routine(
    runtime_state: &Arc<GatewayRuntimeState>,
    runtime: &crate::gateway::RoutinesRuntimeConfig,
    context: &RoutinesToolContext,
    payload: &Map<String, Value>,
) -> Result<Value, String> {
    let routine_id = match optional_string_field(payload, "routine_id") {
        Some(value) => {
            validate_canonical_id(value.as_str())
                .map_err(|_| "routine_id must be a canonical ULID when provided".to_owned())?;
            value
        }
        None => Ulid::new().to_string(),
    };
    let trigger_kind =
        parse_trigger_kind(required_string_field(payload, "trigger_kind")?.as_str())?;
    let owner_principal = normalize_owner_principal(
        optional_string_field(payload, "owner_principal").as_deref(),
        context.principal.as_str(),
    )?;
    let channel = normalize_channel(
        optional_string_field(payload, "channel").as_deref(),
        context.channel.as_deref(),
    );
    let enabled = optional_bool_field(payload, "enabled")?.unwrap_or(true);
    let existing_job =
        runtime_state.cron_job(routine_id.clone()).await.map_err(sanitize_status_message)?;
    if let Some(job) = existing_job.as_ref() {
        ensure_job_owner(job, context.principal.as_str())?;
    }

    let schedule = resolve_routine_schedule(payload, trigger_kind, runtime.timezone_mode)?;
    let prompt = required_string_field(payload, "prompt")?;
    let execution = parse_execution_config(
        optional_string_field(payload, "run_mode").as_deref(),
        optional_string_field(payload, "procedure_profile_id"),
        optional_string_field(payload, "skill_profile_id"),
        optional_string_field(payload, "provider_profile_id"),
        optional_string_field(payload, "execution_posture").as_deref(),
    )?;
    validate_routine_prompt_self_contained(prompt.as_str(), &execution)
        .map_err(map_registry_error)?;
    let delivery = parse_delivery(
        optional_string_field(payload, "delivery_mode").as_deref(),
        optional_string_field(payload, "delivery_channel"),
        optional_string_field(payload, "delivery_failure_mode").as_deref(),
        optional_string_field(payload, "delivery_failure_channel"),
        optional_string_field(payload, "silent_policy").as_deref(),
    )?;
    let quiet_hours = parse_quiet_hours(
        optional_string_field(payload, "quiet_hours_start").as_deref(),
        optional_string_field(payload, "quiet_hours_end").as_deref(),
        optional_string_field(payload, "quiet_hours_timezone"),
    )?;
    let approval_policy =
        parse_approval_policy(optional_string_field(payload, "approval_mode").as_deref())?;
    let concurrency_policy =
        parse_concurrency_policy(optional_string_field(payload, "concurrency_policy").as_deref())?;
    let retry_policy = parse_retry_policy(
        optional_u32_field(payload, "retry_max_attempts")?,
        optional_u64_field(payload, "retry_backoff_ms")?,
    )?;
    let misfire_policy =
        parse_misfire_policy(optional_string_field(payload, "misfire_policy").as_deref())?;
    let approval_required = enabled
        && approval_policy.mode == RoutineApprovalMode::BeforeEnable
        && !routine_approval_granted(
            runtime_state,
            routine_approval_subject_id(routine_id.as_str(), RoutineApprovalMode::BeforeEnable),
        )
        .await?;
    let job_record = persist_routine_job(
        runtime_state,
        existing_job,
        RoutineJobUpsert {
            routine_id: routine_id.clone(),
            name: required_string_field(payload, "name")?,
            prompt: prompt.clone(),
            owner_principal: owner_principal.clone(),
            channel: channel.clone(),
            session_key: optional_string_field(payload, "session_key"),
            session_label: optional_string_field(payload, "session_label"),
            schedule_type: schedule.schedule_type,
            schedule_payload_json: schedule.schedule_payload_json.clone(),
            enabled: enabled && !approval_required,
            concurrency_policy,
            retry_policy: retry_policy.clone(),
            misfire_policy,
            jitter_ms: optional_u64_field(payload, "jitter_ms")?.unwrap_or(0),
            next_run_at_unix_ms: schedule.next_run_at_unix_ms,
        },
    )
    .await?;
    runtime.scheduler_wake.notify_one();

    let trigger_payload_json = if trigger_kind == RoutineTriggerKind::Schedule {
        build_schedule_trigger_payload(&job_record)
    } else {
        serde_json::to_string(payload.get("trigger_payload").unwrap_or(&Value::Object(Map::new())))
            .map_err(|error| format!("trigger payload must be valid JSON: {error}"))?
    };
    let metadata = runtime
        .registry
        .upsert_routine(RoutineMetadataUpsert {
            routine_id: routine_id.clone(),
            trigger_kind,
            trigger_payload_json,
            execution,
            delivery,
            quiet_hours,
            cooldown_ms: optional_u64_field(payload, "cooldown_ms")?.unwrap_or(0),
            approval_policy,
            template_id: optional_string_field(payload, "template_id"),
        })
        .map_err(map_registry_error)?;
    let approval = if approval_required {
        Some(
            ensure_routine_approval_requested(
                runtime_state,
                context.principal.as_str(),
                Some(job_record.channel.as_str()),
                &job_record,
                &metadata,
                RoutineApprovalMode::BeforeEnable,
            )
            .await?,
        )
    } else {
        None
    };
    Ok(json!({
        "operation": "upsert",
        "routine": routine_view_from_parts(&job_record, &metadata),
        "approval": approval,
    }))
}

async fn set_routine_enabled(
    runtime_state: &Arc<GatewayRuntimeState>,
    runtime: &crate::gateway::RoutinesRuntimeConfig,
    context: &RoutinesToolContext,
    payload: &Map<String, Value>,
    enabled: bool,
) -> Result<Value, String> {
    let routine_id = required_string_field(payload, "routine_id")?;
    let routine = load_routine_parts_for_owner(
        runtime_state,
        &runtime.registry,
        routine_id.as_str(),
        context.principal.as_str(),
    )
    .await?;
    let approval_required = enabled
        && routine.metadata.approval_policy.mode == RoutineApprovalMode::BeforeEnable
        && !routine_approval_granted(
            runtime_state,
            routine_approval_subject_id(
                routine.metadata.routine_id.as_str(),
                RoutineApprovalMode::BeforeEnable,
            ),
        )
        .await?;
    let updated = runtime_state
        .update_cron_job(
            routine.job.job_id.clone(),
            CronJobUpdatePatch {
                enabled: Some(enabled && !approval_required),
                ..CronJobUpdatePatch::default()
            },
        )
        .await
        .map_err(sanitize_status_message)?;
    runtime.scheduler_wake.notify_one();
    let approval = if approval_required {
        Some(
            ensure_routine_approval_requested(
                runtime_state,
                context.principal.as_str(),
                context.channel.as_deref(),
                &updated,
                &routine.metadata,
                RoutineApprovalMode::BeforeEnable,
            )
            .await?,
        )
    } else {
        None
    };
    Ok(json!({
        "operation": if enabled { "resume" } else { "pause" },
        "routine": routine_view_from_parts(&updated, &routine.metadata),
        "approval": approval,
    }))
}

async fn run_routine_now(
    runtime_state: &Arc<GatewayRuntimeState>,
    runtime: &crate::gateway::RoutinesRuntimeConfig,
    context: &RoutinesToolContext,
    payload: &Map<String, Value>,
) -> Result<Value, String> {
    let routine_id = required_string_field(payload, "routine_id")?;
    dispatch_single_routine(
        runtime_state,
        runtime,
        context,
        routine_id.as_str(),
        RoutineDispatchRequest {
            trigger_kind: RoutineTriggerKind::Manual,
            trigger_reason: Some("manual run-now".to_owned()),
            trigger_payload: json!({
                "source": "manual_run_now",
                "requested_from_run_id": context.run_id,
                "requested_from_session_id": context.session_id,
            }),
            trigger_dedupe_key: None,
            dispatch_mode: RoutineDispatchMode::Normal,
            source_run_id: None,
            execution_override: None,
            delivery_override: None,
            approval_note: None,
            safety_note: None,
            bypass_operator_gates: false,
        },
    )
    .await
}

async fn test_run_routine(
    runtime_state: &Arc<GatewayRuntimeState>,
    runtime: &crate::gateway::RoutinesRuntimeConfig,
    context: &RoutinesToolContext,
    payload: &Map<String, Value>,
) -> Result<Value, String> {
    let routine_id = required_string_field(payload, "routine_id")?;
    let routine = load_routine_parts_for_owner(
        runtime_state,
        &runtime.registry,
        routine_id.as_str(),
        context.principal.as_str(),
    )
    .await?;
    let replay_source = match optional_string_field(payload, "source_run_id") {
        Some(run_id) => {
            let entry = runtime
                .registry
                .find_run_metadata(run_id.as_str())
                .map_err(map_registry_error)?
                .ok_or_else(|| "source run not found".to_owned())?;
            if entry.routine_id != routine.metadata.routine_id {
                return Err("source_run_id must belong to the selected routine".to_owned());
            }
            Some(entry)
        }
        None => None,
    };
    let trigger_payload = if let Some(value) = payload.get("trigger_payload") {
        value.clone()
    } else if let Some(source) = replay_source.as_ref() {
        serde_json::from_str(source.trigger_payload_json.as_str())
            .map_err(|error| format!("failed to parse archived trigger payload: {error}"))?
    } else {
        serde_json::from_str(routine.metadata.trigger_payload_json.as_str())
            .map_err(|error| format!("failed to parse routine trigger payload: {error}"))?
    };
    let dispatch_mode = if replay_source.is_some() {
        RoutineDispatchMode::Replay
    } else {
        RoutineDispatchMode::TestRun
    };
    let safety_note = Some(match dispatch_mode {
        RoutineDispatchMode::Replay => {
            "safe replay forced fresh_session and audit-only delivery".to_owned()
        }
        _ => "safe test-run forced fresh_session and audit-only delivery".to_owned(),
    });
    dispatch_single_routine(
        runtime_state,
        runtime,
        context,
        routine_id.as_str(),
        RoutineDispatchRequest {
            trigger_kind: replay_source
                .as_ref()
                .map(|entry| entry.trigger_kind)
                .unwrap_or(routine.metadata.trigger_kind),
            trigger_reason: optional_string_field(payload, "trigger_reason")
                .or_else(|| replay_source.as_ref().and_then(|entry| entry.trigger_reason.clone()))
                .or_else(|| Some("safe test-run".to_owned())),
            trigger_payload,
            trigger_dedupe_key: None,
            dispatch_mode,
            source_run_id: replay_source.as_ref().map(|entry| entry.run_id.clone()),
            execution_override: Some(RoutineExecutionConfig {
                run_mode: RoutineRunMode::FreshSession,
                ..routine.metadata.execution.clone()
            }),
            delivery_override: Some(build_safe_test_delivery()),
            approval_note: None,
            safety_note,
            bypass_operator_gates: true,
        },
    )
    .await
}

async fn dispatch_single_routine(
    runtime_state: &Arc<GatewayRuntimeState>,
    runtime: &crate::gateway::RoutinesRuntimeConfig,
    context: &RoutinesToolContext,
    routine_id: &str,
    request: RoutineDispatchRequest,
) -> Result<Value, String> {
    let routine = load_routine_parts_for_owner(
        runtime_state,
        &runtime.registry,
        routine_id,
        context.principal.as_str(),
    )
    .await?;
    let execution =
        request.execution_override.clone().unwrap_or_else(|| routine.metadata.execution.clone());
    let delivery =
        request.delivery_override.clone().unwrap_or_else(|| routine.metadata.delivery.clone());

    if !request.bypass_operator_gates {
        if !routine.job.enabled {
            return register_terminal_routine_run(
                runtime_state,
                &runtime.registry,
                TerminalRoutineRunRequest {
                    routine_id: routine.job.job_id.as_str(),
                    trigger_kind: request.trigger_kind,
                    trigger_reason: request.trigger_reason,
                    trigger_payload: &request.trigger_payload,
                    trigger_dedupe_key: request.trigger_dedupe_key,
                    execution,
                    delivery,
                    dispatch_mode: request.dispatch_mode,
                    source_run_id: request.source_run_id,
                    status: CronRunStatus::Skipped,
                    outcome_override: RoutineRunOutcomeKind::Skipped,
                    message: "routine is disabled",
                    skip_reason: Some("routine_disabled".to_owned()),
                    delivery_reason: None,
                    approval_note: request.approval_note,
                    safety_note: request.safety_note,
                },
            )
            .await;
        }
        if routine.metadata.approval_policy.mode == RoutineApprovalMode::BeforeFirstRun
            && !routine_approval_granted(
                runtime_state,
                routine_approval_subject_id(
                    routine.metadata.routine_id.as_str(),
                    RoutineApprovalMode::BeforeFirstRun,
                ),
            )
            .await?
        {
            let approval = ensure_routine_approval_requested(
                runtime_state,
                context.principal.as_str(),
                Some(routine.job.channel.as_str()),
                &routine.job,
                &routine.metadata,
                RoutineApprovalMode::BeforeFirstRun,
            )
            .await?;
            let mut response = register_terminal_routine_run(
                runtime_state,
                &runtime.registry,
                TerminalRoutineRunRequest {
                    routine_id: routine.job.job_id.as_str(),
                    trigger_kind: request.trigger_kind,
                    trigger_reason: request.trigger_reason,
                    trigger_payload: &request.trigger_payload,
                    trigger_dedupe_key: request.trigger_dedupe_key,
                    execution,
                    delivery,
                    dispatch_mode: request.dispatch_mode,
                    source_run_id: request.source_run_id,
                    status: CronRunStatus::Denied,
                    outcome_override: RoutineRunOutcomeKind::Denied,
                    message: "routine approval is required before the first run",
                    skip_reason: Some("approval_required".to_owned()),
                    delivery_reason: None,
                    approval_note: Some("before_first_run approval is still pending".to_owned()),
                    safety_note: request.safety_note,
                },
            )
            .await?;
            if let Some(object) = response.as_object_mut() {
                object.insert("approval".to_owned(), approval);
            }
            return Ok(response);
        }
        if routine.metadata.trigger_kind != request.trigger_kind
            && request.trigger_kind != RoutineTriggerKind::Manual
        {
            return Err("trigger_kind does not match routine definition".to_owned());
        }
        if let Some(dedupe_key) = request.trigger_dedupe_key.as_deref() {
            let seen = runtime
                .registry
                .seen_dedupe_key(routine.metadata.routine_id.as_str(), dedupe_key)
                .map_err(map_registry_error)?;
            if seen {
                return register_terminal_routine_run(
                    runtime_state,
                    &runtime.registry,
                    TerminalRoutineRunRequest {
                        routine_id: routine.job.job_id.as_str(),
                        trigger_kind: request.trigger_kind,
                        trigger_reason: request.trigger_reason,
                        trigger_payload: &request.trigger_payload,
                        trigger_dedupe_key: Some(dedupe_key.to_owned()),
                        execution,
                        delivery,
                        dispatch_mode: request.dispatch_mode,
                        source_run_id: request.source_run_id,
                        status: CronRunStatus::Skipped,
                        outcome_override: RoutineRunOutcomeKind::Throttled,
                        message: "duplicate trigger dedupe key already processed",
                        skip_reason: Some("duplicate_trigger_dedupe_key".to_owned()),
                        delivery_reason: None,
                        approval_note: request.approval_note,
                        safety_note: request.safety_note,
                    },
                )
                .await;
            }
        }

        let now_unix_ms = unix_ms_now_string_safe()?;
        if routine.metadata.cooldown_ms > 0 {
            let latest = runtime
                .registry
                .list_run_metadata(Some(routine.metadata.routine_id.as_str()), 1)
                .map_err(map_registry_error)?
                .into_iter()
                .last();
            if latest.as_ref().is_some_and(|entry| {
                entry.created_at_unix_ms + routine.metadata.cooldown_ms as i64 > now_unix_ms
            }) {
                return register_terminal_routine_run(
                    runtime_state,
                    &runtime.registry,
                    TerminalRoutineRunRequest {
                        routine_id: routine.job.job_id.as_str(),
                        trigger_kind: request.trigger_kind,
                        trigger_reason: request.trigger_reason,
                        trigger_payload: &request.trigger_payload,
                        trigger_dedupe_key: request.trigger_dedupe_key,
                        execution,
                        delivery,
                        dispatch_mode: request.dispatch_mode,
                        source_run_id: request.source_run_id,
                        status: CronRunStatus::Skipped,
                        outcome_override: RoutineRunOutcomeKind::Throttled,
                        message: "routine cooldown window is still active",
                        skip_reason: Some("cooldown_active".to_owned()),
                        delivery_reason: None,
                        approval_note: request.approval_note,
                        safety_note: request.safety_note,
                    },
                )
                .await;
            }
        }
        if is_in_quiet_hours(routine.metadata.quiet_hours.as_ref(), now_unix_ms)? {
            return register_terminal_routine_run(
                runtime_state,
                &runtime.registry,
                TerminalRoutineRunRequest {
                    routine_id: routine.job.job_id.as_str(),
                    trigger_kind: request.trigger_kind,
                    trigger_reason: request.trigger_reason,
                    trigger_payload: &request.trigger_payload,
                    trigger_dedupe_key: request.trigger_dedupe_key,
                    execution,
                    delivery,
                    dispatch_mode: request.dispatch_mode,
                    source_run_id: request.source_run_id,
                    status: CronRunStatus::Skipped,
                    outcome_override: RoutineRunOutcomeKind::Skipped,
                    message: "routine quiet hours suppress execution",
                    skip_reason: Some("quiet_hours".to_owned()),
                    delivery_reason: None,
                    approval_note: request.approval_note,
                    safety_note: request.safety_note,
                },
            )
            .await;
        }
    }

    let outcome = cron::trigger_job_now_with_options(
        Arc::clone(runtime_state),
        runtime.auth.clone(),
        runtime.grpc_url.clone(),
        routine.job.clone(),
        Arc::clone(&runtime.scheduler_wake),
        build_cron_trigger_options(
            &execution,
            request.dispatch_mode,
            request.source_run_id.as_deref(),
            request.safety_note.as_deref(),
        ),
    )
    .await
    .map_err(sanitize_status_message)?;
    if let Some(run_id) = outcome.run_id.as_ref() {
        let outcome_kind = default_outcome_from_cron_status(outcome.status);
        let _ = runtime
            .registry
            .upsert_run_metadata(RoutineRunMetadataUpsert {
                run_id: run_id.clone(),
                routine_id: routine.metadata.routine_id.clone(),
                trigger_kind: request.trigger_kind,
                trigger_reason: request.trigger_reason,
                trigger_payload_json: serde_json::to_string(&request.trigger_payload)
                    .map_err(|error| format!("failed to serialize trigger payload: {error}"))?,
                trigger_dedupe_key: request.trigger_dedupe_key,
                execution,
                delivery: delivery.clone(),
                dispatch_mode: request.dispatch_mode,
                source_run_id: request.source_run_id,
                outcome_override: Some(outcome_kind),
                outcome_message: Some(outcome.message.clone()),
                output_delivered: Some(output_delivered_for_outcome(&delivery, outcome_kind)),
                skip_reason: if matches!(outcome.status, CronRunStatus::Skipped) {
                    Some("scheduler_skipped".to_owned())
                } else {
                    None
                },
                delivery_reason: Some(delivery_reason_for_outcome(&delivery, outcome_kind)),
                approval_note: request.approval_note,
                safety_note: request.safety_note,
            })
            .map_err(map_registry_error)?;
    }
    Ok(json!({
        "operation": if request.dispatch_mode == RoutineDispatchMode::Normal { "run_now" } else { "test_run" },
        "routine_id": routine.metadata.routine_id,
        "run_id": outcome.run_id,
        "status": outcome.status.as_str(),
        "message": outcome.message,
        "dispatch_mode": request.dispatch_mode.as_str(),
        "delivery_preview": routine_delivery_preview(&delivery),
    }))
}

async fn register_terminal_routine_run(
    runtime_state: &Arc<GatewayRuntimeState>,
    registry: &Arc<RoutineRegistry>,
    request: TerminalRoutineRunRequest<'_>,
) -> Result<Value, String> {
    let run_id = Ulid::new().to_string();
    let delivery_preview = routine_delivery_preview(&request.delivery);
    runtime_state
        .start_cron_run(CronRunStartRequest {
            run_id: run_id.clone(),
            job_id: request.routine_id.to_owned(),
            attempt: 1,
            session_id: None,
            orchestrator_run_id: None,
            status: request.status,
            error_kind: Some("routine_gate".to_owned()),
            error_message_redacted: Some(request.message.to_owned()),
        })
        .await
        .map_err(sanitize_status_message)?;
    runtime_state
        .finalize_cron_run(CronRunFinalizeRequest {
            run_id: run_id.clone(),
            status: request.status,
            error_kind: Some("routine_gate".to_owned()),
            error_message_redacted: Some(request.message.to_owned()),
            model_tokens_in: 0,
            model_tokens_out: 0,
            tool_calls: 0,
            tool_denies: 0,
            orchestrator_run_id: None,
            session_id: None,
        })
        .await
        .map_err(sanitize_status_message)?;
    registry
        .upsert_run_metadata(RoutineRunMetadataUpsert {
            run_id: run_id.clone(),
            routine_id: request.routine_id.to_owned(),
            trigger_kind: request.trigger_kind,
            trigger_reason: request.trigger_reason,
            trigger_payload_json: serde_json::to_string(request.trigger_payload)
                .map_err(|error| format!("failed to serialize trigger payload: {error}"))?,
            trigger_dedupe_key: request.trigger_dedupe_key,
            execution: request.execution,
            delivery: request.delivery,
            dispatch_mode: request.dispatch_mode,
            source_run_id: request.source_run_id,
            outcome_override: Some(request.outcome_override),
            outcome_message: Some(request.message.to_owned()),
            output_delivered: Some(false),
            skip_reason: request.skip_reason,
            delivery_reason: request.delivery_reason,
            approval_note: request.approval_note,
            safety_note: request.safety_note,
        })
        .map_err(map_registry_error)?;
    Ok(json!({
        "run_id": run_id,
        "status": request.status.as_str(),
        "message": request.message,
        "dispatch_mode": request.dispatch_mode.as_str(),
        "delivery_preview": delivery_preview,
    }))
}

async fn routine_views_for_principal(
    runtime_state: &Arc<GatewayRuntimeState>,
    registry: &Arc<RoutineRegistry>,
    jobs: Vec<CronJobRecord>,
    principal: &str,
) -> Result<Vec<Value>, String> {
    let job_map = jobs.into_iter().map(|job| (job.job_id.clone(), job)).collect::<HashMap<_, _>>();
    let mut routines = Vec::new();
    for metadata in registry.list_routines().map_err(map_registry_error)?.into_iter() {
        let Some(job) = job_map.get(metadata.routine_id.as_str()) else {
            continue;
        };
        if job.owner_principal != principal {
            continue;
        }
        let view = enrich_routine_view_with_latest_run(
            runtime_state,
            registry,
            routine_view_from_parts(job, &metadata),
            metadata.routine_id.as_str(),
            job.job_id.as_str(),
        )
        .await?;
        routines.push(view);
    }
    routines.sort_by(|left, right| {
        read_string_value(left, "routine_id").cmp(&read_string_value(right, "routine_id"))
    });
    Ok(routines)
}

async fn load_routine_view_for_owner(
    runtime_state: &Arc<GatewayRuntimeState>,
    registry: &Arc<RoutineRegistry>,
    routine_id: &str,
    principal: &str,
) -> Result<Value, String> {
    let parts =
        load_routine_parts_for_owner(runtime_state, registry, routine_id, principal).await?;
    enrich_routine_view_with_latest_run(
        runtime_state,
        registry,
        routine_view_from_parts(&parts.job, &parts.metadata),
        parts.metadata.routine_id.as_str(),
        parts.job.job_id.as_str(),
    )
    .await
}

async fn load_routine_parts_for_owner(
    runtime_state: &Arc<GatewayRuntimeState>,
    registry: &Arc<RoutineRegistry>,
    routine_id: &str,
    principal: &str,
) -> Result<RoutineParts, String> {
    synchronize_schedule_routines(runtime_state, registry).await?;
    let metadata = registry
        .get_routine(routine_id)
        .map_err(map_registry_error)?
        .ok_or_else(|| "routine not found".to_owned())?;
    let job = runtime_state
        .cron_job(metadata.routine_id.clone())
        .await
        .map_err(sanitize_status_message)?
        .ok_or_else(|| "routine backing cron job not found".to_owned())?;
    ensure_job_owner(&job, principal)?;
    Ok(RoutineParts { job, metadata })
}

async fn persist_routine_job(
    runtime_state: &Arc<GatewayRuntimeState>,
    existing_job: Option<CronJobRecord>,
    request: RoutineJobUpsert,
) -> Result<CronJobRecord, String> {
    if existing_job.is_some() {
        runtime_state
            .update_cron_job(
                request.routine_id.clone(),
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
                    concurrency_policy: Some(request.concurrency_policy),
                    retry_policy: Some(request.retry_policy),
                    misfire_policy: Some(request.misfire_policy),
                    jitter_ms: Some(request.jitter_ms),
                    next_run_at_unix_ms: Some(request.next_run_at_unix_ms),
                    queued_run: Some(false),
                },
            )
            .await
            .map_err(sanitize_status_message)
    } else {
        runtime_state
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
                concurrency_policy: request.concurrency_policy,
                retry_policy: request.retry_policy,
                misfire_policy: request.misfire_policy,
                jitter_ms: request.jitter_ms,
                next_run_at_unix_ms: request.next_run_at_unix_ms,
            })
            .await
            .map_err(sanitize_status_message)
    }
}

async fn synchronize_schedule_routines(
    runtime_state: &Arc<GatewayRuntimeState>,
    registry: &Arc<RoutineRegistry>,
) -> Result<Vec<CronJobRecord>, String> {
    let jobs = list_all_cron_jobs(runtime_state).await?;
    registry.sync_schedule_routines(jobs.as_slice()).map_err(map_registry_error)?;
    Ok(jobs)
}

async fn list_all_cron_jobs(
    runtime_state: &Arc<GatewayRuntimeState>,
) -> Result<Vec<CronJobRecord>, String> {
    let mut jobs = Vec::new();
    let mut after_job_id = None::<String>;
    loop {
        let (mut page, next_after_job_id) = runtime_state
            .list_cron_jobs(after_job_id.clone(), Some(MAX_ROUTINE_PAGE_LIMIT), None, None, None)
            .await
            .map_err(sanitize_status_message)?;
        if page.is_empty() {
            break;
        }
        jobs.append(&mut page);
        let Some(next_after_job_id) = next_after_job_id else {
            break;
        };
        after_job_id = Some(next_after_job_id);
    }
    Ok(jobs)
}

async fn enrich_routine_view_with_latest_run(
    runtime_state: &Arc<GatewayRuntimeState>,
    registry: &Arc<RoutineRegistry>,
    mut view: Value,
    routine_id: &str,
    job_id: &str,
) -> Result<Value, String> {
    let (runs, _) = runtime_state
        .list_cron_runs(Some(job_id.to_owned()), None, Some(10))
        .await
        .map_err(sanitize_status_message)?;
    let Some(run) = runs.last() else {
        return Ok(view);
    };
    let metadata = registry.find_run_metadata(run.run_id.as_str()).map_err(map_registry_error)?;
    let latest_run = join_run_metadata(routine_id, run, metadata.as_ref());
    if let Some(object) = view.as_object_mut() {
        object.insert("last_run".to_owned(), latest_run.clone());
        object.insert(
            "last_outcome_kind".to_owned(),
            latest_run.get("outcome_kind").cloned().unwrap_or(Value::Null),
        );
        object.insert(
            "last_outcome_message".to_owned(),
            latest_run.get("outcome_message").cloned().unwrap_or(Value::Null),
        );
        let failure_count =
            runs.iter().filter(|entry| matches!(entry.status, CronRunStatus::Failed)).count();
        let skip_count =
            runs.iter().filter(|entry| matches!(entry.status, CronRunStatus::Skipped)).count();
        let denied_count =
            runs.iter().filter(|entry| matches!(entry.status, CronRunStatus::Denied)).count();
        object.insert(
            "troubleshooting".to_owned(),
            json!({
                "window_size": runs.len(),
                "failed_runs": failure_count,
                "skipped_runs": skip_count,
                "denied_runs": denied_count,
                "recommended_action": if failure_count > 0 {
                    "Inspect the latest run details, provider routing, and delivery preview before re-running."
                } else if denied_count > 0 {
                    "Resolve the approval or safety gate and use safe test-run before re-enabling production delivery."
                } else if skip_count > 0 {
                    "Review cooldown, quiet hours, or trigger dedupe rules that may be suppressing execution."
                } else {
                    "Routine is healthy; use safe test-run when you need a no-delivery diagnostic run."
                },
            }),
        );
    }
    Ok(view)
}

fn routine_view_from_parts(job: &CronJobRecord, metadata: &RoutineMetadataRecord) -> Value {
    json!({
        "routine_id": metadata.routine_id,
        "job_id": job.job_id,
        "name": job.name,
        "prompt": job.prompt,
        "owner_principal": job.owner_principal,
        "channel": job.channel,
        "session_key": job.session_key,
        "session_label": job.session_label,
        "enabled": job.enabled,
        "schedule_type": job.schedule_type.as_str(),
        "schedule_payload": serde_json::from_str::<Value>(job.schedule_payload_json.as_str()).unwrap_or_else(|_| json!({ "raw": job.schedule_payload_json })),
        "next_run_at_unix_ms": job.next_run_at_unix_ms,
        "last_run_at_unix_ms": job.last_run_at_unix_ms,
        "queued_run": job.queued_run,
        "concurrency_policy": job.concurrency_policy.as_str(),
        "retry_policy": {
            "max_attempts": job.retry_policy.max_attempts,
            "backoff_ms": job.retry_policy.backoff_ms,
        },
        "misfire_policy": job.misfire_policy.as_str(),
        "jitter_ms": job.jitter_ms,
        "trigger_kind": metadata.trigger_kind.as_str(),
        "trigger_payload": serde_json::from_str::<Value>(metadata.trigger_payload_json.as_str()).unwrap_or_else(|_| json!({ "raw": metadata.trigger_payload_json })),
        "run_mode": metadata.execution.run_mode.as_str(),
        "execution_posture": metadata.execution.execution_posture.as_str(),
        "procedure_profile_id": metadata.execution.procedure_profile_id,
        "skill_profile_id": metadata.execution.skill_profile_id,
        "provider_profile_id": metadata.execution.provider_profile_id,
        "delivery_mode": metadata.delivery.mode.as_str(),
        "delivery_channel": metadata.delivery.channel,
        "delivery_failure_mode": metadata.delivery.failure_mode.map(RoutineDeliveryMode::as_str),
        "delivery_failure_channel": metadata.delivery.failure_channel,
        "silent_policy": metadata.delivery.silent_policy.as_str(),
        "delivery_preview": routine_delivery_preview(&metadata.delivery),
        "quiet_hours": metadata.quiet_hours.as_ref().map(|quiet_hours| json!({
            "start_minute_of_day": quiet_hours.start_minute_of_day,
            "end_minute_of_day": quiet_hours.end_minute_of_day,
            "timezone": quiet_hours.timezone,
        })),
        "cooldown_ms": metadata.cooldown_ms,
        "approval_mode": metadata.approval_policy.mode.as_str(),
        "template_id": metadata.template_id,
        "created_at_unix_ms": metadata.created_at_unix_ms,
        "updated_at_unix_ms": metadata.updated_at_unix_ms,
    })
}

fn routine_approval_subject_id(routine_id: &str, mode: RoutineApprovalMode) -> String {
    format!("routine:{routine_id}:{}", mode.as_str())
}

async fn routine_approval_granted(
    runtime_state: &Arc<GatewayRuntimeState>,
    subject_id: String,
) -> Result<bool, String> {
    let (approvals, _) = runtime_state
        .list_approval_records(
            None,
            Some(25),
            None,
            None,
            Some(subject_id),
            None,
            Some(ApprovalDecision::Allow),
            Some(ApprovalSubjectType::Tool),
        )
        .await
        .map_err(sanitize_status_message)?;
    Ok(approvals
        .into_iter()
        .any(|approval| matches!(approval.decision, Some(ApprovalDecision::Allow))))
}

async fn ensure_routine_approval_requested(
    runtime_state: &Arc<GatewayRuntimeState>,
    principal: &str,
    channel: Option<&str>,
    job: &CronJobRecord,
    metadata: &RoutineMetadataRecord,
    mode: RoutineApprovalMode,
) -> Result<Value, String> {
    let subject_id = routine_approval_subject_id(metadata.routine_id.as_str(), mode);
    let (existing, _) = runtime_state
        .list_approval_records(
            None,
            Some(25),
            None,
            None,
            Some(subject_id.clone()),
            Some(principal.to_owned()),
            None,
            Some(ApprovalSubjectType::Tool),
        )
        .await
        .map_err(sanitize_status_message)?;
    if let Some(approval) = existing
        .into_iter()
        .rev()
        .find(|approval| approval.subject_id == subject_id && approval.decision.is_none())
    {
        return serde_json::to_value(approval)
            .map_err(|error| format!("failed to serialize routine approval record: {error}"));
    }

    let details_json = json!({
        "routine_id": metadata.routine_id,
        "name": job.name,
        "approval_mode": mode.as_str(),
        "trigger_kind": metadata.trigger_kind.as_str(),
        "delivery_mode": metadata.delivery.mode.as_str(),
        "channel": job.channel,
        "template_id": metadata.template_id,
    })
    .to_string();
    let prompt = ApprovalPromptRecord {
        title: format!("Approve routine {}", job.name),
        risk_level: ApprovalRiskLevel::High,
        subject_id: subject_id.clone(),
        summary: format!(
            "Routine `{}` requires explicit approval for `{}`.",
            job.name,
            mode.as_str()
        ),
        options: vec![
            ApprovalPromptOption {
                option_id: "allow_once".to_owned(),
                label: "Approve routine".to_owned(),
                description: "Allow the routine to proceed with the requested automation action."
                    .to_owned(),
                default_selected: true,
                decision_scope: ApprovalDecisionScope::Once,
                timebox_ttl_ms: None,
            },
            ApprovalPromptOption {
                option_id: "deny_once".to_owned(),
                label: "Keep blocked".to_owned(),
                description: "Leave the routine blocked until an operator approves it later."
                    .to_owned(),
                default_selected: false,
                decision_scope: ApprovalDecisionScope::Once,
                timebox_ttl_ms: None,
            },
        ],
        timeout_seconds: ROUTINE_APPROVAL_TIMEOUT_SECONDS,
        details_json: details_json.clone(),
        policy_explanation:
            "Routine approvals are explicit operator gates for sensitive automation activation."
                .to_owned(),
    };
    let policy_hash = hex::encode(Sha256::digest(details_json.as_bytes()));
    let record = runtime_state
        .create_approval_record(ApprovalCreateRequest {
            approval_id: Ulid::new().to_string(),
            session_id: Ulid::new().to_string(),
            run_id: Ulid::new().to_string(),
            principal: principal.to_owned(),
            device_id: ROUTINE_APPROVAL_DEVICE_ID.to_owned(),
            channel: channel.map(ToOwned::to_owned),
            subject_type: ApprovalSubjectType::Tool,
            subject_id: subject_id.clone(),
            request_summary: format!(
                "routine_id={} routine_name={} approval_mode={}",
                metadata.routine_id,
                job.name,
                mode.as_str()
            ),
            policy_snapshot: ApprovalPolicySnapshot {
                policy_id: "routine.approval.v1".to_owned(),
                policy_hash,
                evaluation_summary: format!(
                    "routine approval required mode={} trigger={} delivery={}",
                    mode.as_str(),
                    metadata.trigger_kind.as_str(),
                    metadata.delivery.mode.as_str()
                ),
            },
            prompt,
        })
        .await
        .map_err(sanitize_status_message)?;
    serde_json::to_value(record)
        .map_err(|error| format!("failed to serialize routine approval record: {error}"))
}

fn resolve_routine_schedule(
    payload: &Map<String, Value>,
    trigger_kind: RoutineTriggerKind,
    timezone_mode: CronTimezoneMode,
) -> Result<ScheduleResolution, String> {
    if trigger_kind != RoutineTriggerKind::Schedule {
        return Ok(ScheduleResolution {
            schedule_type: crate::journal::CronScheduleType::At,
            schedule_payload_json: shadow_manual_schedule_payload_json(),
            next_run_at_unix_ms: None,
        });
    }
    if let Some(phrase) = optional_string_field(payload, "natural_language_schedule") {
        let preview = natural_language_schedule_preview(
            phrase.as_str(),
            timezone_mode,
            unix_ms_now_string_safe()?,
        )
        .map_err(map_registry_error)?;
        return Ok(ScheduleResolution {
            schedule_type: parse_schedule_type(preview.schedule_type.as_str())?,
            schedule_payload_json: preview.schedule_payload_json,
            next_run_at_unix_ms: preview.next_run_at_unix_ms,
        });
    }
    let schedule = build_schedule(payload)?;
    let normalized =
        cron::normalize_schedule(Some(schedule), unix_ms_now_string_safe()?, timezone_mode)
            .map_err(sanitize_status_message)?;
    Ok(ScheduleResolution {
        schedule_type: normalized.schedule_type,
        schedule_payload_json: normalized.schedule_payload_json,
        next_run_at_unix_ms: normalized.next_run_at_unix_ms,
    })
}

fn build_schedule(payload: &Map<String, Value>) -> Result<cron_v1::Schedule, String> {
    let schedule_type = required_string_field(payload, "schedule_type")?;
    match schedule_type.as_str() {
        "cron" => {
            let expression = required_string_field(payload, "cron_expression")?;
            Ok(cron_v1::Schedule {
                r#type: cron_v1::ScheduleType::Cron as i32,
                spec: Some(cron_v1::schedule::Spec::Cron(cron_v1::CronSchedule { expression })),
            })
        }
        "every" => {
            let interval_ms = optional_u64_field(payload, "every_interval_ms")?
                .filter(|value| *value > 0)
                .ok_or_else(|| {
                    "every_interval_ms must be greater than zero for schedule_type=every".to_owned()
                })?;
            Ok(cron_v1::Schedule {
                r#type: cron_v1::ScheduleType::Every as i32,
                spec: Some(cron_v1::schedule::Spec::Every(cron_v1::EverySchedule { interval_ms })),
            })
        }
        "at" => {
            let timestamp_rfc3339 = required_string_field(payload, "at_timestamp_rfc3339")?;
            Ok(cron_v1::Schedule {
                r#type: cron_v1::ScheduleType::At as i32,
                spec: Some(cron_v1::schedule::Spec::At(cron_v1::AtSchedule { timestamp_rfc3339 })),
            })
        }
        _ => Err("schedule_type must be one of cron|every|at".to_owned()),
    }
}

fn build_schedule_trigger_payload(job: &CronJobRecord) -> String {
    json!({
        "schedule_type": job.schedule_type.as_str(),
        "schedule_payload": serde_json::from_str::<Value>(job.schedule_payload_json.as_str()).unwrap_or_else(|_| json!({ "raw": job.schedule_payload_json })),
    })
    .to_string()
}

fn build_safe_test_delivery() -> RoutineDeliveryConfig {
    RoutineDeliveryConfig {
        mode: RoutineDeliveryMode::LogsOnly,
        channel: None,
        failure_mode: Some(RoutineDeliveryMode::LogsOnly),
        failure_channel: None,
        silent_policy: RoutineSilentPolicy::AuditOnly,
    }
}

fn build_cron_trigger_options(
    execution: &RoutineExecutionConfig,
    dispatch_mode: RoutineDispatchMode,
    source_run_id: Option<&str>,
    safety_note: Option<&str>,
) -> cron::TriggerJobOptions {
    cron::TriggerJobOptions {
        force_run_mode: Some(execution.run_mode),
        allow_sensitive_tools: Some(
            execution.execution_posture == RoutineExecutionPosture::SensitiveTools,
        ),
        model_profile_override: execution.provider_profile_id.clone(),
        parameter_delta_json: Some(
            json!({
                "routine": {
                    "run_mode": execution.run_mode.as_str(),
                    "execution_posture": execution.execution_posture.as_str(),
                    "procedure_profile_id": execution.procedure_profile_id,
                    "skill_profile_id": execution.skill_profile_id,
                    "provider_profile_id": execution.provider_profile_id,
                },
                "dispatch_mode": dispatch_mode.as_str(),
                "source_run_id": source_run_id,
                "safety_note": safety_note,
            })
            .to_string(),
        ),
        origin_kind: Some(match dispatch_mode {
            RoutineDispatchMode::Normal => "manual".to_owned(),
            RoutineDispatchMode::TestRun => "routine_test_run".to_owned(),
            RoutineDispatchMode::Replay => "routine_replay".to_owned(),
        }),
    }
}

fn output_delivered_for_outcome(
    delivery: &RoutineDeliveryConfig,
    outcome_kind: RoutineRunOutcomeKind,
) -> bool {
    if delivery.silent_policy == RoutineSilentPolicy::AuditOnly {
        return false;
    }
    let failure_path =
        matches!(outcome_kind, RoutineRunOutcomeKind::Failed | RoutineRunOutcomeKind::Denied);
    if !failure_path && delivery.silent_policy == RoutineSilentPolicy::FailureOnly {
        return false;
    }
    let effective_mode =
        if failure_path { delivery.failure_mode.unwrap_or(delivery.mode) } else { delivery.mode };
    !matches!(effective_mode, RoutineDeliveryMode::LocalOnly | RoutineDeliveryMode::LogsOnly)
}

fn delivery_reason_for_outcome(
    delivery: &RoutineDeliveryConfig,
    outcome_kind: RoutineRunOutcomeKind,
) -> String {
    let preview = routine_delivery_preview(delivery);
    let key =
        if matches!(outcome_kind, RoutineRunOutcomeKind::Failed | RoutineRunOutcomeKind::Denied) {
            "failure"
        } else {
            "success"
        };
    preview
        .get(key)
        .and_then(Value::as_object)
        .and_then(|entry| entry.get("reason"))
        .and_then(Value::as_str)
        .unwrap_or("delivery preview unavailable")
        .to_owned()
}

fn parse_trigger_kind(value: &str) -> Result<RoutineTriggerKind, String> {
    RoutineTriggerKind::from_str(value).ok_or_else(|| {
        "trigger_kind must be one of schedule|hook|webhook|system_event|manual".to_owned()
    })
}

fn parse_schedule_type(value: &str) -> Result<crate::journal::CronScheduleType, String> {
    crate::journal::CronScheduleType::from_str(value)
        .ok_or_else(|| "schedule type must be one of cron|every|at".to_owned())
}

fn parse_concurrency_policy(value: Option<&str>) -> Result<CronConcurrencyPolicy, String> {
    let normalized = value.filter(|value| !value.is_empty()).unwrap_or("forbid");
    CronConcurrencyPolicy::from_str(normalized)
        .ok_or_else(|| "concurrency_policy must be one of forbid|replace|queue(1)".to_owned())
}

fn parse_retry_policy(
    max_attempts: Option<u32>,
    backoff_ms: Option<u64>,
) -> Result<CronRetryPolicy, String> {
    Ok(CronRetryPolicy {
        max_attempts: max_attempts.unwrap_or(1).max(1),
        backoff_ms: backoff_ms.unwrap_or(1_000).max(1),
    })
}

fn parse_misfire_policy(value: Option<&str>) -> Result<CronMisfirePolicy, String> {
    let normalized = value.filter(|value| !value.is_empty()).unwrap_or("skip");
    CronMisfirePolicy::from_str(normalized)
        .ok_or_else(|| "misfire_policy must be one of skip|catch_up".to_owned())
}

fn parse_execution_config(
    run_mode: Option<&str>,
    procedure_profile_id: Option<String>,
    skill_profile_id: Option<String>,
    provider_profile_id: Option<String>,
    execution_posture: Option<&str>,
) -> Result<RoutineExecutionConfig, String> {
    let run_mode = match run_mode.filter(|value| !value.is_empty()) {
        Some(value) => RoutineRunMode::from_str(value)
            .ok_or_else(|| "run_mode must be one of same_session|fresh_session".to_owned())?,
        None => RoutineRunMode::SameSession,
    };
    let execution_posture = match execution_posture.filter(|value| !value.is_empty()) {
        Some(value) => RoutineExecutionPosture::from_str(value).ok_or_else(|| {
            "execution_posture must be one of standard|sensitive_tools".to_owned()
        })?,
        None => RoutineExecutionPosture::Standard,
    };
    Ok(RoutineExecutionConfig {
        run_mode,
        procedure_profile_id,
        skill_profile_id,
        provider_profile_id,
        execution_posture,
    })
}

fn parse_delivery(
    mode: Option<&str>,
    channel: Option<String>,
    failure_mode: Option<&str>,
    failure_channel: Option<String>,
    silent_policy: Option<&str>,
) -> Result<RoutineDeliveryConfig, String> {
    let normalized_mode = mode.filter(|value| !value.is_empty()).unwrap_or("same_channel");
    let mode = RoutineDeliveryMode::from_str(normalized_mode).ok_or_else(|| {
        "delivery_mode must be one of same_channel|specific_channel|local_only|logs_only".to_owned()
    })?;
    let failure_mode = match failure_mode.filter(|value| !value.is_empty()) {
        Some(value) => Some(RoutineDeliveryMode::from_str(value).ok_or_else(|| {
            "delivery_failure_mode must be one of same_channel|specific_channel|local_only|logs_only".to_owned()
        })?),
        None => None,
    };
    let silent_policy = match silent_policy.filter(|value| !value.is_empty()) {
        Some(value) => RoutineSilentPolicy::from_str(value).ok_or_else(|| {
            "silent_policy must be one of noisy|failure_only|audit_only".to_owned()
        })?,
        None => RoutineSilentPolicy::Noisy,
    };
    if matches!(mode, RoutineDeliveryMode::SpecificChannel) && channel.is_none() {
        return Err("delivery_channel is required for delivery_mode=specific_channel".to_owned());
    }
    if matches!(failure_mode, Some(RoutineDeliveryMode::SpecificChannel))
        && failure_channel.as_ref().or(channel.as_ref()).is_none()
    {
        return Err(
            "delivery_failure_channel or delivery_channel is required for delivery_failure_mode=specific_channel"
                .to_owned(),
        );
    }
    Ok(RoutineDeliveryConfig { mode, channel, failure_mode, failure_channel, silent_policy })
}

fn parse_quiet_hours(
    start: Option<&str>,
    end: Option<&str>,
    timezone: Option<String>,
) -> Result<Option<RoutineQuietHours>, String> {
    let Some(start) = start.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let end = end.filter(|value| !value.is_empty()).ok_or_else(|| {
        "quiet_hours_end is required when quiet_hours_start is provided".to_owned()
    })?;
    let timezone = timezone
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    Ok(Some(RoutineQuietHours {
        start_minute_of_day: parse_time_of_day_to_minutes(start)?,
        end_minute_of_day: parse_time_of_day_to_minutes(end)?,
        timezone,
    }))
}

fn parse_time_of_day_to_minutes(value: &str) -> Result<u16, String> {
    let (hour, minute) = value
        .split_once(':')
        .ok_or_else(|| "quiet hour values must use HH:MM format".to_owned())?;
    let hour =
        hour.parse::<u16>().map_err(|_| "quiet hour hour component must be numeric".to_owned())?;
    let minute = minute
        .parse::<u16>()
        .map_err(|_| "quiet hour minute component must be numeric".to_owned())?;
    if hour > 23 || minute > 59 {
        return Err("quiet hour values must stay within 00:00-23:59".to_owned());
    }
    Ok(hour * 60 + minute)
}

fn parse_approval_policy(value: Option<&str>) -> Result<RoutineApprovalPolicy, String> {
    let normalized = value.filter(|value| !value.is_empty()).unwrap_or("none");
    let mode = RoutineApprovalMode::from_str(normalized).ok_or_else(|| {
        "approval_mode must be one of none|before_enable|before_first_run".to_owned()
    })?;
    Ok(RoutineApprovalPolicy { mode })
}

fn normalize_owner_principal(requested: Option<&str>, principal: &str) -> Result<String, String> {
    match requested.map(str::trim) {
        Some("") | None => Ok(principal.to_owned()),
        Some(owner_principal) if owner_principal == principal => Ok(owner_principal.to_owned()),
        Some(_) => Err("owner_principal must match the authenticated principal".to_owned()),
    }
}

fn normalize_channel(requested: Option<&str>, fallback_channel: Option<&str>) -> String {
    requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| fallback_channel.map(ToOwned::to_owned))
        .unwrap_or_else(|| DEFAULT_ROUTINE_CHANNEL.to_owned())
}

fn ensure_job_owner(job: &CronJobRecord, principal: &str) -> Result<(), String> {
    if job.owner_principal != principal {
        return Err("routine owner mismatch for authenticated principal".to_owned());
    }
    Ok(())
}

fn is_in_quiet_hours(
    quiet_hours: Option<&RoutineQuietHours>,
    now_unix_ms: i64,
) -> Result<bool, String> {
    let Some(quiet_hours) = quiet_hours else {
        return Ok(false);
    };
    let minute_of_day = match quiet_hours.timezone.as_deref().unwrap_or("local") {
        "utc" => {
            let value = chrono::Utc
                .timestamp_millis_opt(now_unix_ms)
                .single()
                .ok_or_else(|| "failed to resolve UTC quiet-hour timestamp".to_owned())?;
            value.hour() as u16 * 60 + value.minute() as u16
        }
        "local" => {
            let value = chrono::Local
                .timestamp_millis_opt(now_unix_ms)
                .single()
                .ok_or_else(|| "failed to resolve local quiet-hour timestamp".to_owned())?;
            value.hour() as u16 * 60 + value.minute() as u16
        }
        _ => return Err("quiet_hours timezone must be one of local|utc".to_owned()),
    };
    let start = quiet_hours.start_minute_of_day;
    let end = quiet_hours.end_minute_of_day;
    Ok(if start == end {
        true
    } else if start < end {
        minute_of_day >= start && minute_of_day < end
    } else {
        minute_of_day >= start || minute_of_day < end
    })
}

fn required_string_field(payload: &Map<String, Value>, key: &str) -> Result<String, String> {
    optional_string_field(payload, key)
        .ok_or_else(|| format!("{key} is required and must be a non-empty string"))
}

fn optional_string_field(payload: &Map<String, Value>, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn optional_bool_field(payload: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(format!("{key} must be a boolean")),
    }
}

fn optional_u32_field(payload: &Map<String, Value>, key: &str) -> Result<Option<u32>, String> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => {
            let raw =
                value.as_u64().ok_or_else(|| format!("{key} must be a non-negative integer"))?;
            let value = u32::try_from(raw)
                .map_err(|_| format!("{key} must fit into a 32-bit unsigned integer"))?;
            Ok(Some(value))
        }
        Some(_) => Err(format!("{key} must be a non-negative integer")),
    }
}

fn optional_u64_field(payload: &Map<String, Value>, key: &str) -> Result<Option<u64>, String> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => {
            value.as_u64().ok_or_else(|| format!("{key} must be a non-negative integer")).map(Some)
        }
        Some(_) => Err(format!("{key} must be a non-negative integer")),
    }
}

fn optional_usize_field(payload: &Map<String, Value>, key: &str) -> Result<Option<usize>, String> {
    optional_u64_field(payload, key).map(|value| value.map(|value| value as usize))
}

fn parse_object_input(tool_name: &str, input_json: &[u8]) -> Result<Map<String, Value>, String> {
    match serde_json::from_slice::<Value>(input_json) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(format!("{tool_name} requires JSON object input")),
        Err(error) => Err(format!("{tool_name} invalid JSON input: {error}")),
    }
}

fn read_string_value(record: &Value, key: &str) -> String {
    record.get(key).and_then(Value::as_str).unwrap_or_default().to_owned()
}

fn map_registry_error(error: RoutineRegistryError) -> String {
    match error {
        RoutineRegistryError::InvalidField { message, .. } => message,
        other => other.to_string(),
    }
}

fn sanitize_status_message(error: Status) -> String {
    let message = error.message().trim();
    if message.is_empty() {
        error.to_string()
    } else {
        message.to_owned()
    }
}

fn unix_ms_now_string_safe() -> Result<i64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .map_err(|error| error.to_string())
}

fn routines_tool_execution_outcome(
    proposal_id: &str,
    input_json: &[u8],
    success: bool,
    output_json: Vec<u8>,
    error: String,
) -> ToolExecutionOutcome {
    let executed_at_unix_ms = current_unix_ms();
    let mut hasher = Sha256::new();
    hasher.update(b"palyra.routines.attestation.v1");
    hasher.update((proposal_id.len() as u64).to_be_bytes());
    hasher.update(proposal_id.as_bytes());
    hasher.update((input_json.len() as u64).to_be_bytes());
    hasher.update(input_json);
    hasher.update([u8::from(success)]);
    hasher.update((output_json.len() as u64).to_be_bytes());
    hasher.update(output_json.as_slice());
    hasher.update((error.len() as u64).to_be_bytes());
    hasher.update(error.as_bytes());
    hasher.update(executed_at_unix_ms.to_be_bytes());
    let execution_sha256 = hex::encode(hasher.finalize());

    ToolExecutionOutcome {
        success,
        output_json,
        error,
        attestation: ToolAttestation {
            attestation_id: Ulid::new().to_string(),
            execution_sha256,
            executed_at_unix_ms,
            timed_out: false,
            executor: "routines_runtime".to_owned(),
            sandbox_enforcement: "none".to_owned(),
        },
    }
}
