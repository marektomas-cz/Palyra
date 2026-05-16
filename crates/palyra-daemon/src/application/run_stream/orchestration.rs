use std::{sync::Arc, time::Duration};

use serde_json::{json, Value};
use tokio::{
    sync::mpsc,
    time::{interval, interval_at, Instant as TokioInstant, MissedTickBehavior},
};
use tonic::{Status, Streaming};
use tracing::{warn, Instrument};

use crate::{
    application::learning::schedule_post_run_reflection,
    application::provider_events::{
        process_run_stream_provider_events, RunStreamProviderEventsOutcome,
        RunStreamToolResultForModel,
    },
    application::provider_input::{
        build_provider_image_inputs, prepare_model_provider_input, MemoryPromptFailureMode,
        PrepareModelProviderInputRequest,
    },
    application::tool_registry::{
        build_model_visible_tool_catalog_snapshot, snapshot_to_provider_request_value,
        tool_catalog_tape_payload, ModelVisibleToolCatalogSnapshot, ToolCatalogBuildRequest,
        ToolExposureSurface,
    },
    delegation::DelegationSnapshot,
    gateway::{
        canonical_id, current_unix_ms, ingest_memory_best_effort, non_empty,
        record_message_router_journal_event, security_requests_json_mode, truncate_with_ellipsis,
        GatewayRuntimeState,
    },
    journal::{
        MemorySource, OrchestratorCancelRequest, OrchestratorRunMetadataUpdateRequest,
        OrchestratorRunStartRequest, OrchestratorSessionResolveRequest,
        OrchestratorTapeAppendRequest, OrchestratorUsageDelta,
    },
    model_provider::{
        ProviderFinishReason, ProviderMessage, ProviderRequest, ProviderResponse,
        ProviderTurnOutput,
    },
    orchestrator::{is_cancel_command, RunLifecycleState, RunStateMachine, RunTransition},
    provider_leases::ProviderLeaseExecutionContext,
    self_healing::{WorkHeartbeatKind, WorkHeartbeatUpdate},
    tool_protocol::ToolRequestContext,
    transport::grpc::{auth::RequestContext, proto::palyra::common::v1 as common_v1},
    usage_governance::{
        plan_usage_routing, resolve_provider_binding_for_model, RoutingTaskClass,
        UsageRoutingPlanRequest,
    },
};

use super::{
    agent_loop::{
        AgentLoopTerminationReason, AgentRunLoopState, DEFAULT_AGENT_LOOP_WALL_CLOCK_BUDGET_MS,
    },
    cancellation::transition_run_stream_to_cancelled,
    tape::{maybe_compact_context_after_tool_results, send_status_with_tape},
};

const PROVIDER_PROGRESS_HEARTBEAT_MS: u64 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunStreamPostProviderOutcome {
    Completed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub(crate) enum RunStreamProviderRequestOutcome {
    Completed(Box<ProviderResponse>),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunStreamProviderResponseOutcome {
    Completed {
        tool_result_messages: Vec<ProviderMessage>,
        provider_trace_ref: Option<String>,
        final_reply_text: Option<String>,
    },
    Failed {
        message: String,
        provider_trace_ref: Option<String>,
        reason: AgentLoopTerminationReason,
    },
    Cancelled,
}

fn run_stream_attachment_metadata(attachments: &[common_v1::MessageAttachment]) -> Vec<Value> {
    attachments
        .iter()
        .map(|attachment| {
            let kind =
                match common_v1::message_attachment::AttachmentKind::try_from(attachment.kind).ok()
                {
                    Some(common_v1::message_attachment::AttachmentKind::Image) => "image",
                    Some(common_v1::message_attachment::AttachmentKind::File) => "file",
                    Some(common_v1::message_attachment::AttachmentKind::Audio) => "audio",
                    Some(common_v1::message_attachment::AttachmentKind::Video) => "video",
                    _ => "unspecified",
                };
            json!({
                "kind": kind,
                "artifact_id": attachment
                    .artifact_id
                    .as_ref()
                    .map(|value| value.ulid.clone()),
                "size_bytes": if attachment.size_bytes > 0 {
                    Some(attachment.size_bytes)
                } else {
                    None
                },
            })
        })
        .collect()
}

struct RunStreamUserMessage<'a> {
    run_id: &'a str,
    request_context: &'a RequestContext,
    envelope_id: Option<&'a common_v1::CanonicalId>,
    input_content: &'a common_v1::MessageContent,
    session_key: &'a str,
    json_mode_requested: bool,
}

#[allow(clippy::result_large_err)]
async fn append_run_stream_user_message(
    runtime_state: &Arc<GatewayRuntimeState>,
    tape_seq: &mut i64,
    message: RunStreamUserMessage<'_>,
) -> Result<(), Status> {
    if message.input_content.text.trim().is_empty() && message.input_content.attachments.is_empty()
    {
        return Ok(());
    }

    runtime_state
        .append_orchestrator_tape_event(OrchestratorTapeAppendRequest {
            run_id: message.run_id.to_owned(),
            seq: *tape_seq,
            event_type: "message.received".to_owned(),
            payload_json: json!({
                "envelope_id": message.envelope_id.map(|value| value.ulid.clone()),
                "text": message.input_content.text.clone(),
                "channel": message.request_context.channel.clone(),
                "session_key": non_empty(message.session_key.to_owned()),
                "json_mode_requested": message.json_mode_requested,
                "attachments": run_stream_attachment_metadata(message.input_content.attachments.as_slice()),
            })
            .to_string(),
        })
        .await?;
    *tape_seq = tape_seq.saturating_add(1);
    Ok(())
}

async fn persist_run_stream_delegation_metadata(
    runtime_state: &Arc<GatewayRuntimeState>,
    run_id: &str,
    origin_run_id: Option<&common_v1::CanonicalId>,
    parameter_delta_json: Option<&str>,
) -> Result<(), Status> {
    let Some(parameter_delta_json) = parameter_delta_json else {
        return Ok(());
    };
    let parsed = match serde_json::from_str::<Value>(parameter_delta_json) {
        Ok(value) => value,
        Err(error) => {
            warn!(
                run_id = %run_id,
                error = %error,
                "ignoring non-JSON parameter_delta while inspecting delegation metadata"
            );
            return Ok(());
        }
    };
    let Some(delegation_json) = parsed.get("delegation") else {
        return Ok(());
    };
    let delegation = match serde_json::from_value::<DelegationSnapshot>(delegation_json.clone()) {
        Ok(value) => value,
        Err(error) => {
            warn!(
                run_id = %run_id,
                error = %error,
                "ignoring invalid delegation snapshot inside parameter_delta"
            );
            return Ok(());
        }
    };
    runtime_state
        .update_orchestrator_run_metadata(OrchestratorRunMetadataUpdateRequest {
            run_id: run_id.to_owned(),
            parent_run_id: Some(origin_run_id.map(|value| value.ulid.clone())),
            delegation: Some(Some(delegation)),
            merge_result: None,
        })
        .await
}

#[allow(clippy::result_large_err)]
pub(crate) async fn finalize_run_stream_after_provider_response(
    sender: &mpsc::Sender<Result<common_v1::RunStreamEvent, Status>>,
    runtime_state: &Arc<GatewayRuntimeState>,
    run_state: &mut RunStateMachine,
    run_id: &str,
    tape_seq: &mut i64,
) -> Result<RunStreamPostProviderOutcome, Status> {
    match runtime_state.is_orchestrator_cancel_requested(run_id.to_owned()).await {
        Ok(true) => {
            transition_run_stream_to_cancelled(sender, runtime_state, run_state, run_id, tape_seq)
                .await?;
            return Ok(RunStreamPostProviderOutcome::Cancelled);
        }
        Ok(false) => {}
        Err(error) => return Err(error),
    }

    if run_state.state() == RunLifecycleState::InProgress {
        run_state
            .transition(RunTransition::Complete)
            .map_err(|error| Status::internal(error.to_string()))?;
        runtime_state
            .update_orchestrator_run_state(run_id.to_owned(), RunLifecycleState::Done, None)
            .await?;
        runtime_state.clear_self_healing_heartbeat(WorkHeartbeatKind::Run, run_id);
        send_status_with_tape(
            sender,
            runtime_state,
            run_id,
            tape_seq,
            common_v1::stream_status::StatusKind::Done,
            "completed",
        )
        .await?;
    }

    Ok(RunStreamPostProviderOutcome::Completed)
}

#[allow(clippy::result_large_err)]
async fn execute_run_stream_provider_request(
    sender: &mpsc::Sender<Result<common_v1::RunStreamEvent, Status>>,
    runtime_state: &Arc<GatewayRuntimeState>,
    run_state: &mut RunStateMachine,
    run_id: &str,
    provider_request: ProviderRequest,
    lease_context: ProviderLeaseExecutionContext,
    tape_seq: &mut i64,
) -> Result<RunStreamProviderRequestOutcome, Status> {
    let provider_span = tracing::info_span!(
        "provider.call",
        run_id = %run_id,
        trace_id = provider_request.context_trace_id.as_deref().unwrap_or("none"),
        has_tool_catalog = provider_request.tool_catalog_snapshot.is_some(),
        json_mode = provider_request.json_mode,
        status = tracing::field::Empty,
    );
    let mut provider_future = Box::pin(
        runtime_state
            .execute_model_provider_with_lease(provider_request, lease_context)
            .instrument(provider_span),
    );
    let mut cancel_poll = interval(Duration::from_millis(100));
    cancel_poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut progress_heartbeat = interval_at(
        TokioInstant::now() + Duration::from_millis(PROVIDER_PROGRESS_HEARTBEAT_MS),
        Duration::from_millis(PROVIDER_PROGRESS_HEARTBEAT_MS),
    );
    progress_heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            provider_result = &mut provider_future => {
                return provider_result
                    .map(Box::new)
                    .map(RunStreamProviderRequestOutcome::Completed);
            }
            _ = cancel_poll.tick() => {
                match runtime_state.is_orchestrator_cancel_requested(run_id.to_owned()).await {
                    Ok(true) => {
                        transition_run_stream_to_cancelled(
                            sender,
                            runtime_state,
                            run_state,
                            run_id,
                            tape_seq,
                        )
                        .await?;
                        return Ok(RunStreamProviderRequestOutcome::Cancelled);
                    }
                    Ok(false) => {}
                    Err(error) => return Err(error),
                }
            }
            _ = progress_heartbeat.tick() => {
                if run_state.state() == RunLifecycleState::InProgress {
                    send_status_with_tape(
                        sender,
                        runtime_state,
                        run_id,
                        tape_seq,
                        common_v1::stream_status::StatusKind::InProgress,
                        "streaming heartbeat",
                    )
                    .await?;
                }
            }
        }
    }
}

#[allow(clippy::result_large_err)]
async fn ensure_run_stream_in_progress(
    sender: &mpsc::Sender<Result<common_v1::RunStreamEvent, Status>>,
    runtime_state: &Arc<GatewayRuntimeState>,
    run_state: &mut RunStateMachine,
    run_id: &str,
    in_progress_emitted: &mut bool,
    tape_seq: &mut i64,
) -> Result<(), Status> {
    if *in_progress_emitted {
        return Ok(());
    }

    run_state
        .transition(RunTransition::StartStreaming)
        .map_err(|error| Status::internal(error.to_string()))?;
    runtime_state
        .update_orchestrator_run_state(run_id.to_owned(), RunLifecycleState::InProgress, None)
        .await?;
    send_status_with_tape(
        sender,
        runtime_state,
        run_id,
        tape_seq,
        common_v1::stream_status::StatusKind::InProgress,
        "streaming",
    )
    .await?;
    *in_progress_emitted = true;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn build_and_record_run_stream_tool_catalog_snapshot(
    runtime_state: &Arc<GatewayRuntimeState>,
    request_context: &RequestContext,
    session_id: &str,
    run_id: &str,
    provider_kind: &str,
    provider_model_id: Option<&str>,
    remaining_tool_budget: u32,
    tape_seq: &mut i64,
) -> Result<ModelVisibleToolCatalogSnapshot, Status> {
    let snapshot = build_model_visible_tool_catalog_snapshot(ToolCatalogBuildRequest {
        config: &runtime_state.config.tool_call,
        browser_service_enabled: runtime_state.config.browser_service.enabled,
        request_context: &ToolRequestContext {
            principal: request_context.principal.clone(),
            device_id: Some(request_context.device_id.clone()),
            channel: request_context.channel.clone(),
            session_id: Some(session_id.to_owned()),
            run_id: Some(run_id.to_owned()),
            skill_id: None,
        },
        provider_kind,
        provider_model_id,
        surface: ToolExposureSurface::RunStream,
        remaining_tool_budget,
        created_at_unix_ms: current_unix_ms(),
    });
    runtime_state
        .append_orchestrator_tape_event(OrchestratorTapeAppendRequest {
            run_id: run_id.to_owned(),
            seq: *tape_seq,
            event_type: "tool_catalog_snapshot".to_owned(),
            payload_json: tool_catalog_tape_payload(&snapshot),
        })
        .await?;
    *tape_seq = (*tape_seq).saturating_add(1);
    Ok(snapshot)
}

#[allow(clippy::result_large_err)]
async fn append_agent_loop_tape_event(
    runtime_state: &Arc<GatewayRuntimeState>,
    run_id: &str,
    tape_seq: &mut i64,
    event_type: &str,
    payload_json: String,
) -> Result<(), Status> {
    runtime_state
        .append_orchestrator_tape_event(OrchestratorTapeAppendRequest {
            run_id: run_id.to_owned(),
            seq: *tape_seq,
            event_type: event_type.to_owned(),
            payload_json,
        })
        .await?;
    *tape_seq = (*tape_seq).saturating_add(1);
    Ok(())
}

#[allow(clippy::result_large_err)]
#[allow(clippy::too_many_arguments)]
async fn terminate_run_stream_with_agent_loop_reason(
    sender: &mpsc::Sender<Result<common_v1::RunStreamEvent, Status>>,
    runtime_state: &Arc<GatewayRuntimeState>,
    run_state: &mut RunStateMachine,
    run_id: &str,
    tape_seq: &mut i64,
    loop_state: &AgentRunLoopState,
    reason: AgentLoopTerminationReason,
    message: &str,
    provider_trace_ref: Option<String>,
) -> Result<(), Status> {
    let message = loop_state.message_with_cleanup_guidance(message);
    append_agent_loop_tape_event(
        runtime_state,
        run_id,
        tape_seq,
        "agent_loop.terminated",
        loop_state.termination_payload(run_id, reason, message.as_str(), provider_trace_ref),
    )
    .await?;
    run_state
        .transition(RunTransition::Fail)
        .map_err(|error| Status::internal(error.to_string()))?;
    runtime_state
        .update_orchestrator_run_state(
            run_id.to_owned(),
            RunLifecycleState::Failed,
            Some(message.clone()),
        )
        .await?;
    runtime_state.clear_self_healing_heartbeat(WorkHeartbeatKind::Run, run_id);
    send_status_with_tape(
        sender,
        runtime_state,
        run_id,
        tape_seq,
        common_v1::stream_status::StatusKind::Failed,
        message.as_str(),
    )
    .await
}

#[allow(clippy::result_large_err)]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn process_run_stream_message(
    sender: &mpsc::Sender<Result<common_v1::RunStreamEvent, Status>>,
    stream: &mut Streaming<common_v1::RunStreamRequest>,
    runtime_state: &Arc<GatewayRuntimeState>,
    request_context: &RequestContext,
    active_session_id: &mut Option<String>,
    active_run_id: &mut Option<String>,
    run_state: &mut RunStateMachine,
    tape_seq: &mut i64,
    model_token_tape_events: &mut usize,
    model_token_compaction_emitted: &mut bool,
    in_progress_emitted: &mut bool,
    remaining_tool_budget: &mut u32,
    previous_session_run_id: &mut Option<String>,
    message: common_v1::RunStreamRequest,
) -> Result<(), Status> {
    let session_id = canonical_id(message.session_id, "session_id")?;
    let run_id = canonical_id(message.run_id, "run_id")?;

    if let Some(expected_session) = active_session_id.as_ref() {
        if expected_session != &session_id {
            return Err(Status::invalid_argument("run stream cannot switch session_id mid-stream"));
        }
    }
    if let Some(expected_run) = active_run_id.as_ref() {
        if expected_run != &run_id {
            return Err(Status::invalid_argument("run stream cannot switch run_id mid-stream"));
        }
    }

    let parameter_delta_json = (!message.parameter_delta_json.is_empty())
        .then(|| String::from_utf8_lossy(message.parameter_delta_json.as_slice()).into_owned());
    if active_run_id.is_none() {
        run_state
            .transition(RunTransition::Accept)
            .map_err(|error| Status::internal(error.to_string()))?;
        let resolved_session = runtime_state
            .resolve_orchestrator_session(OrchestratorSessionResolveRequest {
                session_id: Some(session_id.clone()),
                session_key: non_empty(message.session_key.clone()),
                session_label: non_empty(message.session_label.clone()),
                principal: request_context.principal.clone(),
                device_id: request_context.device_id.clone(),
                channel: request_context.channel.clone(),
                require_existing: message.require_existing,
                reset_session: message.reset_session,
            })
            .await?;
        if message.reset_session {
            runtime_state
                .clear_tool_approval_cache_for_session(request_context, session_id.as_str());
        }
        *previous_session_run_id = resolved_session
            .session
            .last_run_id
            .clone()
            .or(resolved_session.session.branch_origin_run_id.clone());
        if resolved_session.session.session_id != session_id {
            return Err(Status::failed_precondition(
                "resolved session_id does not match RunStream session_id",
            ));
        }
        runtime_state
            .start_orchestrator_run(OrchestratorRunStartRequest {
                run_id: run_id.clone(),
                session_id: session_id.clone(),
                origin_kind: non_empty(message.origin_kind.clone())
                    .unwrap_or_else(|| "manual".to_owned()),
                origin_run_id: message.origin_run_id.as_ref().map(|value| value.ulid.clone()),
                triggered_by_principal: Some(request_context.principal.clone()),
                parameter_delta_json: parameter_delta_json.clone(),
            })
            .await?;
        persist_run_stream_delegation_metadata(
            runtime_state,
            run_id.as_str(),
            message.origin_run_id.as_ref(),
            parameter_delta_json.as_deref(),
        )
        .await?;

        *active_session_id = Some(session_id.clone());
        *active_run_id = Some(run_id.clone());
        runtime_state.record_self_healing_heartbeat(WorkHeartbeatUpdate {
            kind: WorkHeartbeatKind::Run,
            object_id: run_id.clone(),
            summary: format!("run {run_id} for session {session_id}"),
        });

        let accepted_message =
            format!("accepted session={session_id} principal={}", request_context.principal);
        send_status_with_tape(
            sender,
            runtime_state,
            run_id.as_str(),
            tape_seq,
            common_v1::stream_status::StatusKind::Accepted,
            accepted_message.as_str(),
        )
        .await?;
    }

    let input_envelope = message.input.unwrap_or_default();
    let input_content = input_envelope.content.unwrap_or_default();
    let input_text = input_content.text.clone();
    let json_mode_requested = security_requests_json_mode(input_envelope.security.as_ref());
    let session_id_for_message = active_session_id
        .as_deref()
        .ok_or_else(|| {
            Status::internal(
                "run stream internal invariant violated: missing session_id for message",
            )
        })?
        .to_owned();
    runtime_state.record_self_healing_heartbeat(WorkHeartbeatUpdate {
        kind: WorkHeartbeatKind::Run,
        object_id: run_id.clone(),
        summary: format!("run {run_id} for session {session_id_for_message}"),
    });

    if is_cancel_command(input_text.as_str()) {
        runtime_state
            .request_orchestrator_cancel(OrchestratorCancelRequest {
                run_id: run_id.clone(),
                reason: "stream_cancel_command".to_owned(),
            })
            .await?;
    }

    match runtime_state.is_orchestrator_cancel_requested(run_id.clone()).await {
        Ok(true) => {
            transition_run_stream_to_cancelled(
                sender,
                runtime_state,
                run_state,
                run_id.as_str(),
                tape_seq,
            )
            .await?;
            return Ok(());
        }
        Ok(false) => {}
        Err(error) => return Err(error),
    }

    ensure_run_stream_in_progress(
        sender,
        runtime_state,
        run_state,
        run_id.as_str(),
        in_progress_emitted,
        tape_seq,
    )
    .await?;

    append_run_stream_user_message(
        runtime_state,
        tape_seq,
        RunStreamUserMessage {
            run_id: run_id.as_str(),
            request_context,
            envelope_id: input_envelope.envelope_id.as_ref(),
            input_content: &input_content,
            session_key: message.session_key.as_str(),
            json_mode_requested,
        },
    )
    .await?;

    let provider_snapshot = runtime_state.model_provider_status_snapshot();
    let routing_vision_inputs = build_provider_image_inputs(
        input_content.attachments.as_slice(),
        &runtime_state.config.media,
    )
    .len();
    let routing_decision = plan_usage_routing(UsageRoutingPlanRequest {
        runtime_state,
        request_context,
        run_id: run_id.as_str(),
        session_id: session_id.as_str(),
        parameter_delta_json: parameter_delta_json.as_deref(),
        prompt_text: input_text.as_str(),
        json_mode: json_mode_requested,
        vision_inputs: routing_vision_inputs,
        scope_kind: "session",
        scope_id: session_id_for_message.as_str(),
        task_class: RoutingTaskClass::PrimaryInteractive,
        provider_snapshot: &provider_snapshot,
    })
    .await?;

    let session_model_override = if routing_decision.mode == "enforced" {
        None
    } else {
        runtime_state
            .orchestrator_session_by_id(session_id.clone())
            .await?
            .and_then(|session| session.model_profile_override)
    };

    let provider_model_override = if routing_decision.mode == "enforced" {
        Some(routing_decision.actual_model_id.clone())
    } else {
        session_model_override
    };
    let (lease_provider_id, lease_provider_kind, lease_credential_id) =
        provider_model_override.as_deref().map_or_else(
            || {
                (
                    routing_decision.provider_id.clone(),
                    routing_decision.provider_kind.clone(),
                    routing_decision.credential_id.clone(),
                )
            },
            |model_id| resolve_provider_binding_for_model(&provider_snapshot, model_id),
        );
    let mut first_turn_tool_catalog_snapshot = Some(
        build_and_record_run_stream_tool_catalog_snapshot(
            runtime_state,
            request_context,
            session_id_for_message.as_str(),
            run_id.as_str(),
            lease_provider_kind.as_str(),
            provider_model_override.as_deref().or(Some(routing_decision.actual_model_id.as_str())),
            *remaining_tool_budget,
            tape_seq,
        )
        .await?,
    );
    let previous_run_id_for_context = previous_session_run_id.take();
    let prepared_provider_input = prepare_model_provider_input(
        runtime_state,
        request_context,
        PrepareModelProviderInputRequest {
            run_id: run_id.as_str(),
            tape_seq,
            session_id: session_id_for_message.as_str(),
            previous_run_id: previous_run_id_for_context.as_deref(),
            parameter_delta_json: parameter_delta_json.as_deref(),
            input_text: input_text.as_str(),
            attachments: input_content.attachments.as_slice(),
            provider_kind_hint: Some(lease_provider_kind.as_str()),
            provider_model_id_hint: provider_model_override
                .as_deref()
                .or(Some(routing_decision.actual_model_id.as_str())),
            tool_catalog_snapshot: first_turn_tool_catalog_snapshot.as_ref(),
            memory_ingest_reason: "run_stream_user_input",
            memory_prompt_failure_mode: MemoryPromptFailureMode::Fail,
            channel_for_log: request_context.channel.as_deref().unwrap_or("n/a"),
        },
    )
    .await?;
    let mut base_provider_request = ProviderRequest::from_input_text(
        prepared_provider_input.provider_input_text,
        json_mode_requested,
        prepared_provider_input.vision_inputs,
        provider_model_override.clone(),
    );
    base_provider_request.instruction_hash = prepared_provider_input.instruction_hash.clone();
    base_provider_request.context_trace_id = prepared_provider_input.context_trace_id.clone();
    base_provider_request.budget_profile = prepared_provider_input.budget_profile.clone();
    if !prepared_provider_input.provider_messages.is_empty() {
        let mut messages = prepared_provider_input.provider_messages.clone();
        messages.push(ProviderMessage::user_text(base_provider_request.input_text.clone()));
        base_provider_request.messages = messages;
    }
    let mut loop_state = AgentRunLoopState::new(
        base_provider_request.effective_messages(),
        AgentRunLoopState::default_model_turn_budget(
            runtime_state.config.tool_call.max_calls_per_run,
        ),
        *remaining_tool_budget,
        DEFAULT_AGENT_LOOP_WALL_CLOCK_BUDGET_MS,
    );
    append_agent_loop_tape_event(
        runtime_state,
        run_id.as_str(),
        tape_seq,
        "agent_loop.started",
        loop_state.start_payload(run_id.as_str()),
    )
    .await?;

    loop {
        let _turn_id = match loop_state.start_model_turn() {
            Ok(turn_id) => turn_id,
            Err(reason) => {
                let message = match reason {
                    AgentLoopTerminationReason::MaxTurns => "agent loop model turn limit reached",
                    AgentLoopTerminationReason::WallClock => {
                        "agent loop wall-clock budget exhausted"
                    }
                    _ => "agent loop budget exhausted",
                };
                terminate_run_stream_with_agent_loop_reason(
                    sender,
                    runtime_state,
                    run_state,
                    run_id.as_str(),
                    tape_seq,
                    &loop_state,
                    reason,
                    message,
                    None,
                )
                .await?;
                return Ok(());
            }
        };
        append_agent_loop_tape_event(
            runtime_state,
            run_id.as_str(),
            tape_seq,
            "agent_loop.turn_started",
            loop_state.turn_payload(run_id.as_str(), "agent_loop.turn_started"),
        )
        .await?;

        let tool_catalog_snapshot = if let Some(snapshot) = first_turn_tool_catalog_snapshot.take()
        {
            snapshot
        } else {
            build_and_record_run_stream_tool_catalog_snapshot(
                runtime_state,
                request_context,
                session_id_for_message.as_str(),
                run_id.as_str(),
                lease_provider_kind.as_str(),
                provider_model_override
                    .as_deref()
                    .or(Some(routing_decision.actual_model_id.as_str())),
                loop_state.remaining_tool_calls(),
                tape_seq,
            )
            .await?
        };
        let mut provider_request = ProviderRequest::from_input_text(
            base_provider_request.input_text.clone(),
            base_provider_request.json_mode,
            base_provider_request.vision_inputs.clone(),
            base_provider_request.model_override.clone(),
        );
        provider_request.messages = loop_state.messages();
        provider_request.tool_catalog_snapshot =
            Some(snapshot_to_provider_request_value(&tool_catalog_snapshot));
        provider_request.instruction_hash = base_provider_request.instruction_hash.clone();
        provider_request.context_trace_id = base_provider_request.context_trace_id.clone();
        provider_request.budget_profile = base_provider_request.budget_profile.clone();
        let provider_response = match execute_run_stream_provider_request(
            sender,
            runtime_state,
            run_state,
            run_id.as_str(),
            provider_request,
            ProviderLeaseExecutionContext {
                provider_id: lease_provider_id.clone(),
                credential_id: lease_credential_id.clone(),
                priority: RoutingTaskClass::PrimaryInteractive.lease_priority(),
                task_label: RoutingTaskClass::PrimaryInteractive.as_str().to_owned(),
                max_wait_ms: RoutingTaskClass::PrimaryInteractive.max_lease_wait_ms(),
                session_id: Some(session_id_for_message.clone()),
                run_id: Some(run_id.clone()),
            },
            tape_seq,
        )
        .await
        {
            Ok(RunStreamProviderRequestOutcome::Completed(response)) => *response,
            Ok(RunStreamProviderRequestOutcome::Cancelled) => {
                return Ok(());
            }
            Err(error) => {
                terminate_run_stream_with_agent_loop_reason(
                    sender,
                    runtime_state,
                    run_state,
                    run_id.as_str(),
                    tape_seq,
                    &loop_state,
                    AgentLoopTerminationReason::ProviderError,
                    error.message(),
                    None,
                )
                .await?;
                return Err(error);
            }
        };
        loop_state.record_provider_response(&provider_response);
        let provider_output = provider_response.output.clone();

        let response_outcome = process_run_stream_provider_response(
            sender,
            stream,
            runtime_state,
            request_context,
            active_session_id.as_deref(),
            run_state,
            session_id.as_str(),
            run_id.as_str(),
            session_id_for_message.as_str(),
            provider_response,
            &tool_catalog_snapshot,
            remaining_tool_budget,
            message.allow_sensitive_tools,
            tape_seq,
            model_token_tape_events,
            model_token_compaction_emitted,
        )
        .await?;
        loop_state.sync_remaining_tool_calls(*remaining_tool_budget);
        loop_state.append_assistant_turn(&provider_output);

        match response_outcome {
            RunStreamProviderResponseOutcome::Completed {
                tool_result_messages,
                provider_trace_ref,
                final_reply_text,
            } => {
                let should_refeed_tool_results = !tool_result_messages.is_empty()
                    && provider_output.finish_reason == ProviderFinishReason::ToolCalls;
                if !should_refeed_tool_results {
                    if let Some(message) =
                        incomplete_terminal_final_answer(final_reply_text.as_deref(), &loop_state)
                    {
                        terminate_run_stream_with_agent_loop_reason(
                            sender,
                            runtime_state,
                            run_state,
                            run_id.as_str(),
                            tape_seq,
                            &loop_state,
                            AgentLoopTerminationReason::IncompleteFinalAnswer,
                            message.as_str(),
                            provider_trace_ref,
                        )
                        .await?;
                        return Ok(());
                    }
                    append_agent_loop_tape_event(
                        runtime_state,
                        run_id.as_str(),
                        tape_seq,
                        "agent_loop.terminated",
                        loop_state.termination_payload(
                            run_id.as_str(),
                            AgentLoopTerminationReason::FinalAnswer,
                            final_reply_text.as_deref().unwrap_or("completed"),
                            provider_trace_ref,
                        ),
                    )
                    .await?;
                    finalize_run_stream_after_provider_response(
                        sender,
                        runtime_state,
                        run_state,
                        run_id.as_str(),
                        tape_seq,
                    )
                    .await?;
                    return Ok(());
                }

                let tool_result_count = tool_result_messages.len();
                loop_state.append_tool_result_messages(tool_result_messages);
                maybe_compact_context_after_tool_results(
                    runtime_state,
                    request_context,
                    session_id.as_str(),
                    run_id.as_str(),
                    tape_seq,
                    tool_result_count,
                )
                .await?;
                append_agent_loop_tape_event(
                    runtime_state,
                    run_id.as_str(),
                    tape_seq,
                    "agent_loop.turn_completed",
                    loop_state.turn_payload(run_id.as_str(), "agent_loop.turn_completed"),
                )
                .await?;
            }
            RunStreamProviderResponseOutcome::Failed { message, provider_trace_ref, reason } => {
                terminate_run_stream_with_agent_loop_reason(
                    sender,
                    runtime_state,
                    run_state,
                    run_id.as_str(),
                    tape_seq,
                    &loop_state,
                    reason,
                    message.as_str(),
                    provider_trace_ref,
                )
                .await?;
                return Ok(());
            }
            RunStreamProviderResponseOutcome::Cancelled => {
                return Ok(());
            }
        }
    }
}

#[allow(clippy::result_large_err)]
#[allow(clippy::too_many_arguments)]
async fn process_run_stream_provider_response(
    sender: &mpsc::Sender<Result<common_v1::RunStreamEvent, Status>>,
    stream: &mut Streaming<common_v1::RunStreamRequest>,
    runtime_state: &Arc<GatewayRuntimeState>,
    request_context: &RequestContext,
    active_session_id: Option<&str>,
    run_state: &mut RunStateMachine,
    session_id: &str,
    run_id: &str,
    session_id_for_message: &str,
    provider_response: ProviderResponse,
    tool_catalog_snapshot: &ModelVisibleToolCatalogSnapshot,
    remaining_tool_budget: &mut u32,
    allow_sensitive_tools: bool,
    tape_seq: &mut i64,
    model_token_tape_events: &mut usize,
    model_token_compaction_emitted: &mut bool,
) -> Result<RunStreamProviderResponseOutcome, Status> {
    let provider_output = provider_response.output.clone();
    runtime_state
        .add_orchestrator_usage(OrchestratorUsageDelta {
            run_id: run_id.to_owned(),
            prompt_tokens_delta: provider_response.prompt_tokens,
            completion_tokens_delta: 0,
        })
        .await?;

    let (summary_tokens, tool_results) = match process_run_stream_provider_events(
        sender,
        stream,
        runtime_state,
        request_context,
        active_session_id,
        run_state,
        session_id,
        run_id,
        provider_response.events,
        tool_catalog_snapshot,
        remaining_tool_budget,
        allow_sensitive_tools,
        tape_seq,
        model_token_tape_events,
        model_token_compaction_emitted,
    )
    .await?
    {
        RunStreamProviderEventsOutcome::Completed { summary_tokens, tool_results } => {
            (summary_tokens, tool_results)
        }
        RunStreamProviderEventsOutcome::Cancelled => {
            return Ok(RunStreamProviderResponseOutcome::Cancelled);
        }
    };
    persist_run_stream_provider_turn_output(runtime_state, run_id, tape_seq, &provider_output)
        .await?;
    let terminal_tool_failure = tool_results.iter().find_map(terminal_tool_authorization_failure);
    let tool_result_messages = if terminal_tool_failure.is_some() {
        Vec::new()
    } else {
        tool_results.iter().map(tool_result_to_provider_message).collect::<Result<Vec<_>, _>>()?
    };
    let has_pending_tool_results = !tool_result_messages.is_empty();
    let reply_text = if provider_output.full_text.trim().is_empty() {
        summary_tokens.concat()
    } else {
        provider_output.full_text.clone()
    };

    if provider_response.completion_tokens > 0 {
        runtime_state
            .add_orchestrator_usage(OrchestratorUsageDelta {
                run_id: run_id.to_owned(),
                prompt_tokens_delta: 0,
                completion_tokens_delta: provider_response.completion_tokens,
            })
            .await?;
    }

    if let Some(message) = terminal_tool_failure {
        return Ok(RunStreamProviderResponseOutcome::Failed {
            message,
            provider_trace_ref: provider_output.raw_provider_refs.provider_trace_ref.clone(),
            reason: AgentLoopTerminationReason::ApprovalDenied,
        });
    }

    if !has_pending_tool_results {
        if let Some(message) = truncated_final_answer_without_tools(&provider_output) {
            return Ok(RunStreamProviderResponseOutcome::Failed {
                message,
                provider_trace_ref: provider_output.raw_provider_refs.provider_trace_ref.clone(),
                reason: AgentLoopTerminationReason::IncompleteFinalAnswer,
            });
        }
    }

    if !has_pending_tool_results && contains_raw_provider_tool_call_markup(reply_text.as_str()) {
        return Ok(RunStreamProviderResponseOutcome::Failed {
            message:
                "model provider returned raw tool-call markup instead of a structured tool proposal"
                    .to_owned(),
            provider_trace_ref: provider_output.raw_provider_refs.provider_trace_ref.clone(),
            reason: AgentLoopTerminationReason::ProviderError,
        });
    }

    if !has_pending_tool_results {
        persist_run_stream_reply_text(
            runtime_state,
            request_context,
            session_id_for_message,
            run_id,
            tape_seq,
            reply_text.as_str(),
        )
        .await?;
    }

    if !has_pending_tool_results && !summary_tokens.is_empty() {
        ingest_memory_best_effort(
            runtime_state,
            request_context.principal.as_str(),
            request_context.channel.as_deref(),
            Some(session_id_for_message),
            MemorySource::Summary,
            reply_text.as_str(),
            vec!["summary:model_output".to_owned()],
            Some(0.75),
            "run_stream_model_summary",
        )
        .await;
    }

    if let Ok(Some(run_snapshot)) =
        runtime_state.orchestrator_run_status_snapshot(run_id.to_owned()).await
    {
        if run_snapshot.state == RunLifecycleState::Done.as_str() {
            if let Err(error) =
                schedule_post_run_reflection(runtime_state, request_context, session_id, run_id)
                    .await
            {
                warn!(
                    run_id,
                    session_id,
                    status_code = ?error.code(),
                    status_message = %error.message(),
                    "failed to schedule post-run reflection"
                );
            }
        }
    }

    Ok(RunStreamProviderResponseOutcome::Completed {
        tool_result_messages,
        provider_trace_ref: provider_output.raw_provider_refs.provider_trace_ref.clone(),
        final_reply_text: (!has_pending_tool_results).then_some(reply_text),
    })
}

fn tool_result_to_provider_message(
    result: &RunStreamToolResultForModel,
) -> Result<ProviderMessage, Status> {
    let output = serde_json::from_slice::<Value>(result.outcome.output_json.as_slice())
        .unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&result.outcome.output_json) }));
    let content = if result.outcome.error.trim().is_empty() {
        output
    } else {
        let mut content = json!({
            "success": result.outcome.success,
            "tool_name": result.tool_name.as_str(),
            "error": result.outcome.error.as_str(),
            "output": output,
        });
        if let Some(claim_boundary) = failed_tool_claim_boundary(result.tool_name.as_str()) {
            content["diagnostic_status"] = json!("unknown");
            content["claim_boundary"] = json!(claim_boundary);
        }
        content
    };
    let serialized = serde_json::to_string(&content).map_err(|error| {
        Status::internal(format!("failed to serialize model-visible tool result: {error}"))
    })?;
    let redacted = crate::journal::redact_payload_json(serialized.as_bytes()).unwrap_or(serialized);
    Ok(ProviderMessage::tool_result(result.proposal_id.clone(), redacted))
}

fn failed_tool_claim_boundary(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        crate::gateway::BROWSER_CONSOLE_LOG_TOOL_NAME => Some(
            "browser console diagnostics failed; console status is unknown, so do not claim the page has no console errors or that the console is clean unless a later successful console diagnostic verifies it",
        ),
        _ => None,
    }
}

fn terminal_tool_authorization_failure(result: &RunStreamToolResultForModel) -> Option<String> {
    let error = result.outcome.error.trim();
    if result.outcome.success || error.is_empty() || !is_terminal_tool_authorization_error(error) {
        return None;
    }

    if is_noninteractive_cli_approval_denial(error) {
        return Some(format!(
            "tool execution requires approval, but the noninteractive CLI cannot prompt for it: tool={} proposal_id={} error={}. Rerun in an interactive terminal, use --approval-mode allow-once for per-request approval, or use --allow-sensitive-tools only after reviewing the requested tool risk.",
            result.tool_name,
            result.proposal_id,
            truncate_with_ellipsis(error.to_owned(), 512)
        ));
    }
    if is_cli_approval_mode_deny(error) {
        return Some(format!(
            "tool execution was blocked by --approval-mode deny: tool={} proposal_id={} error={}. No approval prompt is pending and the denied tool action was not executed. Rerun in an interactive terminal, use --approval-mode allow-once for per-request approval, or use --allow-sensitive-tools only after reviewing the requested tool risk.",
            result.tool_name,
            result.proposal_id,
            truncate_with_ellipsis(error.to_owned(), 512)
        ));
    }

    Some(format!(
        "tool execution blocked by approval or policy: tool={} proposal_id={} error={}",
        result.tool_name,
        result.proposal_id,
        truncate_with_ellipsis(error.to_owned(), 512)
    ))
}

fn is_terminal_tool_authorization_error(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    TERMINAL_TOOL_AUTHORIZATION_ERROR_MARKERS.iter().any(|marker| normalized.contains(marker))
}

fn is_noninteractive_cli_approval_denial(error: &str) -> bool {
    error.to_ascii_lowercase().contains("approval_required_non_interactive_cli")
}

fn is_cli_approval_mode_deny(error: &str) -> bool {
    error.to_ascii_lowercase().contains("denied_by_cli_approval_mode_deny")
}

fn contains_raw_provider_tool_call_markup(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    normalized.contains("<minimax:tool_call")
        || (normalized.contains("<tool_call") && normalized.contains("<invoke name="))
}

fn truncated_final_answer_without_tools(output: &ProviderTurnOutput) -> Option<String> {
    matches!(output.finish_reason, ProviderFinishReason::Length).then(|| {
        "model provider stopped because of an output token limit before returning a complete final answer or structured tool call (finish_reason=length)"
            .to_owned()
    })
}

fn incomplete_final_answer_without_tools(text: Option<&str>) -> Option<String> {
    let text = text.unwrap_or_default().trim();
    if text.is_empty() {
        return Some(
            "model returned an empty final answer without executing any requested tools".to_owned(),
        );
    }
    if final_answer_is_minimal_ack(text) {
        return Some("model returned a bare acknowledgement as the final answer".to_owned());
    }
    if final_answer_claims_tool_work_without_evidence(text) {
        return Some(
            "model claimed file, process, browser, or verification work without any successful tool results"
                .to_owned(),
        );
    }
    None
}

fn incomplete_terminal_final_answer(
    text: Option<&str>,
    loop_state: &AgentRunLoopState,
) -> Option<String> {
    if loop_state.completed_tool_calls() == 0 {
        return incomplete_final_answer_without_tools(text);
    }

    let text = text.unwrap_or_default().trim();
    if text.is_empty() {
        return Some("model returned an empty final answer after tool execution".to_owned());
    }

    let messages = loop_state.messages();
    if final_answer_is_minimal_ack(text) {
        return Some(
            "model returned a bare acknowledgement instead of a final answer with tool evidence"
                .to_owned(),
        );
    }
    if final_answer_claims_tool_work_without_evidence(text)
        && !final_answer_has_matching_tool_evidence(text, messages.as_slice())
    {
        return Some(
            "model claimed file, process, browser, or verification work without matching tool evidence"
                .to_owned(),
        );
    }
    None
}

fn final_answer_claims_tool_work_without_evidence(text: &str) -> bool {
    let normalized = normalize_final_answer_text(text);
    UNSUPPORTED_TOOL_WORK_CLAIMS.iter().any(|marker| normalized.contains(marker))
}

fn normalize_final_answer_text(text: &str) -> String {
    text.to_ascii_lowercase()
        .replace(['\u{2018}', '\u{2019}'], "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn final_answer_is_minimal_ack(text: &str) -> bool {
    matches!(
        normalize_final_answer_text(text).as_str(),
        "ack" | "ok" | "okay" | "done" | "complete" | "completed"
    )
}

fn has_action_tool_evidence(messages: &[ProviderMessage]) -> bool {
    messages.iter().flat_map(|message| message.tool_calls.iter()).any(|call| {
        !matches!(
            call.tool_name.as_str(),
            "palyra.fs.list_dir"
                | "palyra.fs.read_file"
                | "palyra.memory.search"
                | "palyra.memory.recall"
        )
    })
}

fn final_answer_has_matching_tool_evidence(text: &str, messages: &[ProviderMessage]) -> bool {
    let normalized = normalize_final_answer_text(text);
    if normalized.contains("i read the file") {
        return has_tool_name(messages, "palyra.fs.read_file");
    }
    if normalized.contains("i navigated") || normalized.contains("i opened the browser") {
        return has_tool_name_prefix(messages, "palyra.browser.");
    }
    if normalized.contains("i ran the test")
        || normalized.contains("i ran tests")
        || normalized.contains("test passed")
        || normalized.contains("tests passed")
    {
        return has_tool_name(messages, "palyra.process.run");
    }
    if normalized.contains("i applied the patch")
        || normalized.contains("i created")
        || normalized.contains("i edited")
        || normalized.contains("i fixed")
        || normalized.contains("i implemented")
        || normalized.contains("i modified")
        || normalized.contains("i updated")
        || normalized.contains("i wrote")
    {
        return has_tool_name(messages, "palyra.fs.apply_patch");
    }
    has_action_tool_evidence(messages)
}

fn has_tool_name(messages: &[ProviderMessage], tool_name: &str) -> bool {
    messages
        .iter()
        .flat_map(|message| message.tool_calls.iter())
        .any(|call| call.tool_name == tool_name)
}

fn has_tool_name_prefix(messages: &[ProviderMessage], prefix: &str) -> bool {
    messages
        .iter()
        .flat_map(|message| message.tool_calls.iter())
        .any(|call| call.tool_name.starts_with(prefix))
}

const UNSUPPORTED_TOOL_WORK_CLAIMS: &[&str] = &[
    "i applied the patch",
    "i created the file",
    "i created files",
    "i edited the file",
    "i fixed the file",
    "i implemented",
    "i modified the file",
    "i navigated",
    "i opened the browser",
    "i ran the test",
    "i ran tests",
    "i read the file",
    "i updated the file",
    "i verified",
    "i wrote the file",
    "test passed",
    "tests passed",
];

const TERMINAL_TOOL_AUTHORIZATION_ERROR_MARKERS: &[&str] = &[
    "approval_response_error",
    "approval_response_timeout",
    "approval_required_non_interactive_cli",
    "denied_by_cli_approval_mode_deny",
];

#[allow(clippy::result_large_err)]
async fn persist_run_stream_reply_text(
    runtime_state: &Arc<GatewayRuntimeState>,
    request_context: &RequestContext,
    session_id: &str,
    run_id: &str,
    tape_seq: &mut i64,
    reply_text: &str,
) -> Result<(), Status> {
    if reply_text.trim().is_empty() {
        return Ok(());
    }

    runtime_state
        .append_orchestrator_tape_event(OrchestratorTapeAppendRequest {
            run_id: run_id.to_owned(),
            seq: *tape_seq,
            event_type: "message.replied".to_owned(),
            payload_json: json!({
                "reply_text": reply_text,
            })
            .to_string(),
        })
        .await?;
    *tape_seq += 1;

    let _ = record_message_router_journal_event(
        runtime_state,
        request_context,
        session_id,
        run_id,
        "message.replied",
        common_v1::journal_event::EventActor::System as i32,
        json!({
            "reply_preview": truncate_with_ellipsis(reply_text.to_owned(), 256),
        }),
    )
    .await;

    Ok(())
}

#[allow(clippy::result_large_err)]
async fn persist_run_stream_provider_turn_output(
    runtime_state: &Arc<GatewayRuntimeState>,
    run_id: &str,
    tape_seq: &mut i64,
    output: &ProviderTurnOutput,
) -> Result<(), Status> {
    let payload_json = serde_json::to_string(output).map_err(|error| {
        Status::internal(format!("failed to serialize provider turn output: {error}"))
    })?;
    let payload_json =
        crate::journal::redact_payload_json(payload_json.as_bytes()).unwrap_or(payload_json);
    runtime_state
        .append_orchestrator_tape_event(OrchestratorTapeAppendRequest {
            run_id: run_id.to_owned(),
            seq: *tape_seq,
            event_type: "provider_turn_output".to_owned(),
            payload_json,
        })
        .await?;
    *tape_seq += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::AgentRunLoopState;
    use super::{
        contains_raw_provider_tool_call_markup, incomplete_final_answer_without_tools,
        incomplete_terminal_final_answer, terminal_tool_authorization_failure,
        tool_result_to_provider_message, truncated_final_answer_without_tools,
        RunStreamToolResultForModel,
    };
    use crate::model_provider::{
        ProviderFinishReason, ProviderMessage, ProviderMessageContentPart,
        ProviderOutputContentPart, ProviderRawProviderRefs, ProviderTurnOutput, ProviderUsage,
    };
    use serde_json::{json, Value};

    fn loop_state_after_tool(prompt: &str, tool_name: &str) -> AgentRunLoopState {
        let mut state =
            AgentRunLoopState::new(vec![ProviderMessage::user_text(prompt)], 4, 8, 10_000);
        state.append_assistant_turn(&ProviderTurnOutput {
            full_text: String::new(),
            content_parts: vec![ProviderOutputContentPart::ToolCall {
                proposal_id: "toolu_test_01".to_owned(),
                tool_name: tool_name.to_owned(),
                input_json: json!({}),
            }],
            finish_reason: ProviderFinishReason::ToolCalls,
            usage: ProviderUsage::new(0, 0, "test"),
            raw_provider_refs: ProviderRawProviderRefs::default(),
            redaction_state: Default::default(),
        });
        state.append_tool_result_messages(vec![ProviderMessage::tool_result(
            "toolu_test_01",
            r#"{"success":true}"#,
        )]);
        state
    }

    #[test]
    fn terminal_tool_authorization_failure_detects_approval_errors() {
        let result = RunStreamToolResultForModel {
            proposal_id: "toolu_approval_01".to_owned(),
            tool_name: "palyra.process.run".to_owned(),
            outcome: crate::tool_protocol::denied_execution_outcome(
                "toolu_approval_01",
                "palyra.process.run",
                br#"{"command":"cmd","args":["/C","whoami"]}"#,
                "approval_response_error: tool_approval_response.proposal_id is required",
            ),
        };

        let message = terminal_tool_authorization_failure(&result)
            .expect("approval protocol failures must terminate the run");
        assert!(message.contains("palyra.process.run"));
        assert!(message.contains("toolu_approval_01"));
        assert!(message.contains("approval_response_error"));
    }

    #[test]
    fn terminal_tool_authorization_failure_refeeds_explicit_approval_denials() {
        let result = RunStreamToolResultForModel {
            proposal_id: "toolu_denied_01".to_owned(),
            tool_name: "palyra.process.run".to_owned(),
            outcome: crate::tool_protocol::denied_execution_outcome(
                "toolu_denied_01",
                "palyra.process.run",
                br#"{"command":"cmd","args":["/C","whoami"]}"#,
                "approval.denied: operator denied tool execution",
            ),
        };

        assert!(
            terminal_tool_authorization_failure(&result).is_none(),
            "explicit approval denials are tool observations the model can recover from"
        );
    }

    #[test]
    fn terminal_tool_authorization_failure_stops_noninteractive_cli_denials() {
        let result = RunStreamToolResultForModel {
            proposal_id: "toolu_noninteractive_01".to_owned(),
            tool_name: "palyra.process.run".to_owned(),
            outcome: crate::tool_protocol::denied_execution_outcome(
                "toolu_noninteractive_01",
                "palyra.process.run",
                br#"{"command":"node","args":["-e","console.log(1)"]}"#,
                "approval.denied: approval_required_non_interactive_cli",
            ),
        };

        let message = terminal_tool_authorization_failure(&result)
            .expect("noninteractive CLI approval denials should terminate the run");
        assert!(message.contains("noninteractive CLI"));
        assert!(message.contains("--approval-mode allow-once"));
        assert!(message.contains("--allow-sensitive-tools"));
        assert!(message.contains("toolu_noninteractive_01"));
    }

    #[test]
    fn terminal_tool_authorization_failure_stops_cli_deny_mode() {
        let result = RunStreamToolResultForModel {
            proposal_id: "toolu_deny_mode_01".to_owned(),
            tool_name: "palyra.fs.read_file".to_owned(),
            outcome: crate::tool_protocol::denied_execution_outcome(
                "toolu_deny_mode_01",
                "palyra.fs.read_file",
                br#"{"path":"generated/temp.txt"}"#,
                "tool execution denied by explicit client approval response; tool=palyra.fs.read_file; approval_reason=denied_by_cli_approval_mode_deny; original_reason=requires approval",
            ),
        };

        let message = terminal_tool_authorization_failure(&result)
            .expect("CLI deny mode approval denials should terminate the run");
        assert!(message.contains("--approval-mode deny"));
        assert!(message.contains("No approval prompt is pending"));
        assert!(message.contains("was not executed"));
        assert!(message.contains("--approval-mode allow-once"));
        assert!(message.contains("toolu_deny_mode_01"));
    }

    #[test]
    fn terminal_tool_authorization_failure_ignores_regular_tool_errors() {
        let result = RunStreamToolResultForModel {
            proposal_id: "toolu_regular_error_01".to_owned(),
            tool_name: "palyra.process.run".to_owned(),
            outcome: crate::tool_protocol::build_tool_execution_outcome(
                "toolu_regular_error_01",
                "palyra.process.run",
                br#"{"command":"cmd","args":["/C","exit","1"]}"#,
                false,
                b"{}".to_vec(),
                "process exited with status 1".to_owned(),
                false,
                "builtin".to_owned(),
                "none".to_owned(),
            ),
        };

        assert!(
            terminal_tool_authorization_failure(&result).is_none(),
            "ordinary runtime errors can still be re-fed to the model"
        );
    }

    #[test]
    fn failed_browser_console_result_marks_console_status_unknown_for_model() {
        let result = RunStreamToolResultForModel {
            proposal_id: "toolu_console_01".to_owned(),
            tool_name: crate::gateway::BROWSER_CONSOLE_LOG_TOOL_NAME.to_owned(),
            outcome: crate::tool_protocol::build_tool_execution_outcome(
                "toolu_console_01",
                crate::gateway::BROWSER_CONSOLE_LOG_TOOL_NAME,
                br#"{"session_id":"browser-session-1"}"#,
                false,
                b"{}".to_vec(),
                "missing caller principal".to_owned(),
                false,
                "builtin".to_owned(),
                "none".to_owned(),
            ),
        };

        let message = tool_result_to_provider_message(&result)
            .expect("failed console tool result should serialize for model");
        let content = match message.content.first() {
            Some(ProviderMessageContentPart::Text { text }) => text,
            _ => panic!("tool result should be serialized as text content"),
        };
        let value: Value =
            serde_json::from_str(content).expect("tool result content should be JSON");

        assert_eq!(value.get("success").and_then(Value::as_bool), Some(false));
        assert_eq!(value.get("diagnostic_status").and_then(Value::as_str), Some("unknown"));
        assert!(
            value.get("claim_boundary").and_then(Value::as_str).is_some_and(
                |boundary| boundary.contains("do not claim the page has no console errors")
            ),
            "{value}"
        );
    }

    #[test]
    fn raw_provider_tool_call_markup_is_not_a_final_answer() {
        let raw_tool_call = r#"<minimax:tool_call>
<invoke name="palyra.fs.read_file">
{"path":"C:\\Users\\palo\\workspace\\calc.js"}
</invoke>
</minimax:tool_call>"#;

        assert!(contains_raw_provider_tool_call_markup(raw_tool_call));
        assert!(!contains_raw_provider_tool_call_markup(
            "The page had no tool calls and the final answer is complete."
        ));
    }

    #[test]
    fn incomplete_final_answer_without_tools_detects_bare_ack() {
        let message = incomplete_final_answer_without_tools(Some("done"))
            .expect("bare acknowledgement must not be accepted as a final answer");

        assert!(message.contains("bare acknowledgement"));
    }

    #[test]
    fn truncated_provider_output_is_not_a_final_answer_without_tools() {
        let output = ProviderTurnOutput::text(
            "Created fixtures/app and ran".to_owned(),
            ProviderFinishReason::Length,
            ProviderUsage::new(10, 20, "test"),
            ProviderRawProviderRefs::default(),
        );

        let message = truncated_final_answer_without_tools(&output)
            .expect("length-finished output must not be accepted as final");

        assert!(message.contains("finish_reason=length"));
    }

    #[test]
    fn stop_finished_provider_output_can_be_final_without_tools() {
        let output = ProviderTurnOutput::text(
            "Use cargo test to run the daemon tests.".to_owned(),
            ProviderFinishReason::Stop,
            ProviderUsage::new(10, 20, "test"),
            ProviderRawProviderRefs::default(),
        );

        assert!(truncated_final_answer_without_tools(&output).is_none());
    }

    #[test]
    fn incomplete_final_answer_without_tools_detects_unsupported_work_claims() {
        let message =
            incomplete_final_answer_without_tools(Some("I created the file and tests passed."))
                .expect("tool-work claims need tool evidence");

        assert!(message.contains("without any successful tool results"));
    }

    #[test]
    fn incomplete_final_answer_without_tools_allows_plain_answers() {
        assert!(incomplete_final_answer_without_tools(Some(
            "Use `cargo test -p palyra-daemon` to run the daemon tests."
        ))
        .is_none());
    }

    #[test]
    fn incomplete_terminal_final_answer_rejects_ack_for_requested_tool_work() {
        let state = loop_state_after_tool(
            "Create fixtures/landing-page and verify it.",
            "palyra.fs.list_dir",
        );
        let message = incomplete_terminal_final_answer(Some("ack"), &state)
            .expect("bare ack must not complete a requested tool workflow");

        assert!(message.contains("bare acknowledgement"));
    }

    #[test]
    fn incomplete_terminal_final_answer_allows_concrete_summary_after_action_tool() {
        let state = loop_state_after_tool(
            "Create fixtures/notes-api and run tests.",
            "palyra.fs.apply_patch",
        );

        assert!(incomplete_terminal_final_answer(
            Some("Created fixtures/notes-api and summarized the changed files."),
            &state,
        )
        .is_none());
    }

    #[test]
    fn incomplete_terminal_final_answer_allows_read_claim_after_read_tool() {
        let state =
            loop_state_after_tool("Read README.md and summarize it.", "palyra.fs.read_file");

        assert!(incomplete_terminal_final_answer(
            Some("I read the file. It describes the local development workflow."),
            &state,
        )
        .is_none());
    }
}
