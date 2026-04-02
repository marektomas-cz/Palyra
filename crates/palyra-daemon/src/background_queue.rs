#![allow(clippy::result_large_err)]

use std::{sync::Arc, time::Duration};

use serde_json::{json, Value};
use tokio_stream::StreamExt;
use tonic::{Request, Status};
use tracing::warn;
use ulid::Ulid;

use crate::{
    gateway::{
        proto::palyra::{common::v1 as common_v1, gateway::v1 as gateway_v1},
        GatewayAuthConfig, GatewayRuntimeState, HEADER_CHANNEL, HEADER_DEVICE_ID, HEADER_PRINCIPAL,
    },
    journal::{OrchestratorBackgroundTaskRecord, OrchestratorBackgroundTaskUpdateRequest},
};

const BACKGROUND_QUEUE_IDLE_SLEEP: Duration = Duration::from_secs(3);
const DEFAULT_BACKGROUND_CHANNEL: &str = "console:background";

pub(crate) fn spawn_background_queue_loop(
    runtime: Arc<GatewayRuntimeState>,
    auth: GatewayAuthConfig,
    grpc_url: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Err(error) = poll_background_queue(&runtime, &auth, grpc_url.as_str()).await {
                warn!(status_code = ?error.code(), status_message = %error.message(), "background queue poll failed");
            }
            tokio::time::sleep(BACKGROUND_QUEUE_IDLE_SLEEP).await;
        }
    })
}

async fn poll_background_queue(
    runtime: &Arc<GatewayRuntimeState>,
    auth: &GatewayAuthConfig,
    grpc_url: &str,
) -> Result<(), Status> {
    let tasks = runtime
        .list_orchestrator_background_tasks(crate::journal::OrchestratorBackgroundTaskListFilter {
            owner_principal: None,
            device_id: None,
            channel: None,
            session_id: None,
            include_completed: false,
            limit: 32,
        })
        .await?;
    for task in tasks {
        if let Err(error) = process_background_task(runtime, auth, grpc_url, &task).await {
            warn!(
                task_id = %task.task_id,
                status_code = ?error.code(),
                status_message = %error.message(),
                "background task processing failed"
            );
            let _ = runtime
                .update_orchestrator_background_task(OrchestratorBackgroundTaskUpdateRequest {
                    task_id: task.task_id.clone(),
                    state: Some("failed".to_owned()),
                    target_run_id: None,
                    increment_attempt_count: false,
                    last_error: Some(Some(error.message().to_owned())),
                    result_json: Some(Some(
                        json!({
                            "status": "failed",
                            "task_id": task.task_id,
                            "error": error.message(),
                        })
                        .to_string(),
                    )),
                    started_at_unix_ms: None,
                    completed_at_unix_ms: Some(Some(crate::gateway::current_unix_ms())),
                })
                .await;
        }
    }
    Ok(())
}

async fn process_background_task(
    runtime: &Arc<GatewayRuntimeState>,
    auth: &GatewayAuthConfig,
    grpc_url: &str,
    task: &OrchestratorBackgroundTaskRecord,
) -> Result<(), Status> {
    if is_terminal_task_state(task.state.as_str()) || task.state == "paused" {
        return Ok(());
    }

    let now = crate::gateway::current_unix_ms();
    if let Some(expires_at_unix_ms) = task.expires_at_unix_ms {
        if expires_at_unix_ms <= now {
            runtime
                .update_orchestrator_background_task(OrchestratorBackgroundTaskUpdateRequest {
                    task_id: task.task_id.clone(),
                    state: Some("expired".to_owned()),
                    target_run_id: None,
                    increment_attempt_count: false,
                    last_error: Some(Some("background task expired before dispatch".to_owned())),
                    result_json: Some(Some(
                        json!({
                            "status": "expired",
                            "task_id": task.task_id,
                            "expired_at_unix_ms": expires_at_unix_ms,
                        })
                        .to_string(),
                    )),
                    started_at_unix_ms: None,
                    completed_at_unix_ms: Some(Some(now)),
                })
                .await?;
            return Ok(());
        }
    }
    if task.not_before_unix_ms.is_some_and(|not_before| not_before > now) {
        return Ok(());
    }
    if task.state == "cancel_requested" {
        if let Some(target_run_id) = task.target_run_id.as_deref() {
            let snapshot =
                runtime.orchestrator_run_status_snapshot(target_run_id.to_owned()).await?;
            if snapshot
                .as_ref()
                .map(|run| is_terminal_run_state(run.state.as_str()))
                .unwrap_or(true)
            {
                finalize_task_from_run(runtime, task, snapshot.as_ref(), "cancelled").await?;
            }
        } else {
            runtime
                .update_orchestrator_background_task(OrchestratorBackgroundTaskUpdateRequest {
                    task_id: task.task_id.clone(),
                    state: Some("cancelled".to_owned()),
                    target_run_id: None,
                    increment_attempt_count: false,
                    last_error: Some(Some("cancelled before dispatch".to_owned())),
                    result_json: Some(Some(
                        json!({
                            "status": "cancelled",
                            "task_id": task.task_id,
                        })
                        .to_string(),
                    )),
                    started_at_unix_ms: None,
                    completed_at_unix_ms: Some(Some(now)),
                })
                .await?;
        }
        return Ok(());
    }
    if task.state == "running" {
        if let Some(target_run_id) = task.target_run_id.as_deref() {
            let snapshot =
                runtime.orchestrator_run_status_snapshot(target_run_id.to_owned()).await?;
            if let Some(run) = snapshot.as_ref() {
                if is_terminal_run_state(run.state.as_str()) {
                    finalize_task_from_run(runtime, task, Some(run), run.state.as_str()).await?;
                }
                return Ok(());
            }
        }
    }
    if task.max_attempts > 0 && task.attempt_count >= task.max_attempts {
        runtime
            .update_orchestrator_background_task(OrchestratorBackgroundTaskUpdateRequest {
                task_id: task.task_id.clone(),
                state: Some("failed".to_owned()),
                target_run_id: None,
                increment_attempt_count: false,
                last_error: Some(Some("background task exhausted retry budget".to_owned())),
                result_json: Some(Some(
                    json!({
                        "status": "failed",
                        "task_id": task.task_id,
                        "attempt_count": task.attempt_count,
                        "max_attempts": task.max_attempts,
                    })
                    .to_string(),
                )),
                started_at_unix_ms: None,
                completed_at_unix_ms: Some(Some(now)),
            })
            .await?;
        return Ok(());
    }

    dispatch_background_task(runtime, auth, grpc_url, task).await
}

async fn dispatch_background_task(
    runtime: &Arc<GatewayRuntimeState>,
    auth: &GatewayAuthConfig,
    grpc_url: &str,
    task: &OrchestratorBackgroundTaskRecord,
) -> Result<(), Status> {
    let prompt = task
        .input_text
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| Status::failed_precondition("background task is missing input_text"))?;
    let run_id = Ulid::new().to_string();
    let started_at_unix_ms = crate::gateway::current_unix_ms();
    runtime
        .update_orchestrator_background_task(OrchestratorBackgroundTaskUpdateRequest {
            task_id: task.task_id.clone(),
            state: Some("running".to_owned()),
            target_run_id: Some(Some(run_id.clone())),
            increment_attempt_count: true,
            last_error: Some(None),
            result_json: Some(None),
            started_at_unix_ms: Some(Some(started_at_unix_ms)),
            completed_at_unix_ms: Some(None),
        })
        .await?;

    let mut client =
        gateway_v1::gateway_service_client::GatewayServiceClient::connect(grpc_url.to_owned())
            .await
            .map_err(|error| Status::unavailable(format!("failed to connect gateway: {error}")))?;

    let mut resolve_request = Request::new(gateway_v1::ResolveSessionRequest {
        v: 1,
        session_id: Some(common_v1::CanonicalId { ulid: task.session_id.clone() }),
        session_key: String::new(),
        session_label: String::new(),
        require_existing: true,
        reset_session: false,
    });
    inject_background_metadata(
        resolve_request.metadata_mut(),
        auth,
        task.owner_principal.as_str(),
        task.device_id.as_str(),
        task.channel.as_deref(),
    )?;
    client
        .resolve_session(resolve_request)
        .await
        .map_err(|error| Status::internal(format!("ResolveSession failed: {error}")))?;

    let parameter_delta_json = extract_parameter_delta_bytes(task.payload_json.as_deref())?;
    let mut stream_request = Request::new(tokio_stream::iter(vec![common_v1::RunStreamRequest {
        v: 1,
        session_id: Some(common_v1::CanonicalId { ulid: task.session_id.clone() }),
        run_id: Some(common_v1::CanonicalId { ulid: run_id.clone() }),
        input: Some(common_v1::MessageEnvelope {
            v: 1,
            envelope_id: Some(common_v1::CanonicalId { ulid: Ulid::new().to_string() }),
            timestamp_unix_ms: crate::gateway::current_unix_ms(),
            origin: Some(common_v1::EnvelopeOrigin {
                r#type: common_v1::envelope_origin::OriginType::System as i32,
                channel: task
                    .channel
                    .clone()
                    .unwrap_or_else(|| DEFAULT_BACKGROUND_CHANNEL.to_owned()),
                conversation_id: task.task_id.clone(),
                sender_display: "palyra-background".to_owned(),
                sender_handle: "background".to_owned(),
                sender_verified: true,
            }),
            content: Some(common_v1::MessageContent { text: prompt, attachments: Vec::new() }),
            security: None,
            max_payload_bytes: 0,
        }),
        allow_sensitive_tools: false,
        session_key: String::new(),
        session_label: String::new(),
        reset_session: false,
        require_existing: true,
        tool_approval_response: None,
        origin_kind: "background".to_owned(),
        origin_run_id: task.parent_run_id.clone().map(|ulid| common_v1::CanonicalId { ulid }),
        parameter_delta_json,
        queued_input_id: task.queued_input_id.clone().map(|ulid| common_v1::CanonicalId { ulid }),
    }]));
    inject_background_metadata(
        stream_request.metadata_mut(),
        auth,
        task.owner_principal.as_str(),
        task.device_id.as_str(),
        task.channel.as_deref(),
    )?;

    let mut stream = client
        .run_stream(stream_request)
        .await
        .map_err(|error| Status::internal(format!("RunStream failed: {error}")))?
        .into_inner();

    let mut final_state = "failed".to_owned();
    while let Some(event) = stream.next().await {
        let event =
            event.map_err(|error| Status::internal(format!("run stream read failed: {error}")))?;
        if let Some(common_v1::run_stream_event::Body::Status(status)) = event.body {
            final_state = match status.kind {
                kind if kind == common_v1::stream_status::StatusKind::Done as i32 => {
                    "done".to_owned()
                }
                kind if kind == common_v1::stream_status::StatusKind::Failed as i32 => {
                    "failed".to_owned()
                }
                _ => final_state,
            };
        }
    }

    let snapshot = runtime.orchestrator_run_status_snapshot(run_id.clone()).await?;
    let final_label =
        snapshot.as_ref().map(|run| run.state.as_str()).unwrap_or(final_state.as_str()).to_owned();
    finalize_task_from_run(runtime, task, snapshot.as_ref(), final_label.as_str()).await
}

async fn finalize_task_from_run(
    runtime: &Arc<GatewayRuntimeState>,
    task: &OrchestratorBackgroundTaskRecord,
    run: Option<&crate::journal::OrchestratorRunStatusSnapshot>,
    fallback_state: &str,
) -> Result<(), Status> {
    let normalized_state = match run.map(|value| value.state.as_str()).unwrap_or(fallback_state) {
        "done" => "succeeded",
        "cancelled" => "cancelled",
        "failed" => "failed",
        "running" | "accepted" | "in_progress" => "running",
        other => other,
    };
    if normalized_state == "running" {
        return Ok(());
    }
    let completed_at_unix_ms = run
        .and_then(|value| value.completed_at_unix_ms)
        .unwrap_or_else(crate::gateway::current_unix_ms);
    runtime
        .update_orchestrator_background_task(OrchestratorBackgroundTaskUpdateRequest {
            task_id: task.task_id.clone(),
            state: Some(normalized_state.to_owned()),
            target_run_id: None,
            increment_attempt_count: false,
            last_error: Some(run.and_then(|value| value.last_error.clone())),
            result_json: Some(Some(
                json!({
                    "status": normalized_state,
                    "task_id": task.task_id,
                    "run": run.map(run_status_to_json).unwrap_or_else(|| json!({
                        "state": fallback_state,
                    })),
                })
                .to_string(),
            )),
            started_at_unix_ms: None,
            completed_at_unix_ms: Some(Some(completed_at_unix_ms)),
        })
        .await
}

fn extract_parameter_delta_bytes(payload_json: Option<&str>) -> Result<Vec<u8>, Status> {
    let Some(payload_json) = payload_json else {
        return Ok(Vec::new());
    };
    if payload_json.trim().is_empty() {
        return Ok(Vec::new());
    }
    let payload = serde_json::from_str::<Value>(payload_json).map_err(|error| {
        Status::invalid_argument(format!("invalid background payload_json: {error}"))
    })?;
    let Some(parameter_delta) = payload.get("parameter_delta") else {
        return Ok(Vec::new());
    };
    serde_json::to_vec(parameter_delta).map_err(|error| {
        Status::internal(format!("failed to encode background parameter_delta: {error}"))
    })
}

fn inject_background_metadata(
    metadata: &mut tonic::metadata::MetadataMap,
    auth: &GatewayAuthConfig,
    principal: &str,
    device_id: &str,
    channel: Option<&str>,
) -> Result<(), Status> {
    if auth.require_auth {
        let token = auth.admin_token.as_ref().ok_or_else(|| {
            Status::permission_denied("admin token is required for background queue auth")
        })?;
        metadata.insert(
            "authorization",
            format!("Bearer {token}").parse().map_err(|_| {
                Status::internal("failed to encode background queue authorization metadata")
            })?,
        );
    }
    metadata.insert(
        HEADER_PRINCIPAL,
        principal
            .parse()
            .map_err(|_| Status::invalid_argument("background principal metadata is invalid"))?,
    );
    metadata.insert(
        HEADER_DEVICE_ID,
        device_id
            .parse()
            .map_err(|_| Status::invalid_argument("background device_id metadata is invalid"))?,
    );
    let header_channel =
        channel.filter(|value| !value.trim().is_empty()).unwrap_or(DEFAULT_BACKGROUND_CHANNEL);
    metadata.insert(
        HEADER_CHANNEL,
        header_channel
            .parse()
            .map_err(|_| Status::invalid_argument("background channel metadata is invalid"))?,
    );
    Ok(())
}

fn run_status_to_json(run: &crate::journal::OrchestratorRunStatusSnapshot) -> Value {
    json!({
        "run_id": run.run_id,
        "session_id": run.session_id,
        "state": run.state,
        "cancel_requested": run.cancel_requested,
        "cancel_reason": run.cancel_reason,
        "prompt_tokens": run.prompt_tokens,
        "completion_tokens": run.completion_tokens,
        "total_tokens": run.total_tokens,
        "origin_kind": run.origin_kind,
        "origin_run_id": run.origin_run_id,
        "updated_at_unix_ms": run.updated_at_unix_ms,
        "completed_at_unix_ms": run.completed_at_unix_ms,
        "last_error": run.last_error,
    })
}

fn is_terminal_task_state(state: &str) -> bool {
    matches!(state, "succeeded" | "failed" | "cancelled" | "expired")
}

fn is_terminal_run_state(state: &str) -> bool {
    matches!(state, "done" | "failed" | "cancelled")
}
