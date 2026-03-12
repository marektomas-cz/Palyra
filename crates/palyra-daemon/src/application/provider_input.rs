use std::sync::Arc;

use base64::Engine as _;
use serde_json::{json, Value};
use tonic::Status;
use tracing::warn;

use crate::{
    application::service_authorization::authorize_memory_action,
    gateway::{
        ingest_memory_best_effort, non_empty, truncate_with_ellipsis, GatewayRuntimeState,
        MAX_PREVIOUS_RUN_CONTEXT_ENTRY_CHARS, MAX_PREVIOUS_RUN_CONTEXT_TAPE_EVENTS,
        MAX_PREVIOUS_RUN_CONTEXT_TURNS, MEMORY_AUTO_INJECT_MIN_SCORE,
    },
    journal::{
        MemorySearchHit, MemorySearchRequest, MemorySource, OrchestratorTapeAppendRequest,
        OrchestratorTapeRecord,
    },
    media::MediaRuntimeConfig,
    model_provider::ProviderImageInput,
    transport::grpc::{auth::RequestContext, proto::palyra::common::v1 as common_v1},
};

#[derive(Debug, Clone)]
pub(crate) struct PreparedModelProviderInput {
    pub(crate) provider_input_text: String,
    pub(crate) vision_inputs: Vec<ProviderImageInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemoryPromptFailureMode {
    Fail,
    FallbackToRawInput { warn_message: &'static str },
}

pub(crate) struct PrepareModelProviderInputRequest<'a> {
    pub(crate) run_id: &'a str,
    pub(crate) tape_seq: &'a mut i64,
    pub(crate) session_id: &'a str,
    pub(crate) previous_run_id: Option<&'a str>,
    pub(crate) input_text: &'a str,
    pub(crate) attachments: &'a [common_v1::MessageAttachment],
    pub(crate) memory_ingest_reason: &'a str,
    pub(crate) memory_prompt_failure_mode: MemoryPromptFailureMode,
    pub(crate) channel_for_log: &'a str,
}

fn build_provider_image_inputs(
    attachments: &[common_v1::MessageAttachment],
    media_config: &MediaRuntimeConfig,
) -> Vec<ProviderImageInput> {
    let mut inputs = Vec::new();
    let mut total_bytes = 0usize;
    for attachment in attachments {
        if attachment.kind != common_v1::message_attachment::AttachmentKind::Image as i32 {
            continue;
        }
        if inputs.len() >= media_config.vision_max_image_count {
            break;
        }
        let Some(mime_type) = non_empty(attachment.declared_content_type.clone()) else {
            continue;
        };
        if !media_config.vision_allowed_content_types.iter().any(|allowed| allowed == &mime_type) {
            continue;
        }
        if attachment.inline_bytes.is_empty() {
            continue;
        }
        let image_bytes = attachment.inline_bytes.len();
        if image_bytes > media_config.vision_max_image_bytes {
            continue;
        }
        if total_bytes.saturating_add(image_bytes) > media_config.vision_max_total_bytes {
            break;
        }
        let width_px = (attachment.width_px > 0).then_some(attachment.width_px);
        let height_px = (attachment.height_px > 0).then_some(attachment.height_px);
        if width_px.is_some_and(|value| value > media_config.vision_max_dimension_px)
            || height_px.is_some_and(|value| value > media_config.vision_max_dimension_px)
        {
            continue;
        }
        total_bytes = total_bytes.saturating_add(image_bytes);
        inputs.push(ProviderImageInput {
            mime_type,
            bytes_base64: base64::engine::general_purpose::STANDARD
                .encode(attachment.inline_bytes.as_slice()),
            file_name: non_empty(attachment.filename.clone()),
            width_px,
            height_px,
            artifact_id: attachment.artifact_id.as_ref().map(|value| value.ulid.clone()),
        });
    }
    inputs
}

#[allow(clippy::result_large_err)]
async fn build_memory_augmented_prompt(
    runtime_state: &Arc<GatewayRuntimeState>,
    context: &RequestContext,
    run_id: &str,
    tape_seq: &mut i64,
    session_id: &str,
    memory_query_text: &str,
    prompt_input_text: &str,
) -> Result<String, Status> {
    let trimmed_input = memory_query_text.trim();
    if trimmed_input.is_empty() {
        return Ok(prompt_input_text.to_owned());
    }
    let memory_config = runtime_state.memory_config_snapshot();
    if !memory_config.auto_inject_enabled || memory_config.auto_inject_max_items == 0 {
        return Ok(prompt_input_text.to_owned());
    }
    let resource = format!("memory:session:{session_id}");
    if let Err(error) =
        authorize_memory_action(context.principal.as_str(), "memory.search", resource.as_str())
    {
        warn!(
            run_id,
            principal = %context.principal,
            session_id,
            status_message = %error.message(),
            "memory auto-inject skipped because policy denied access"
        );
        return Ok(prompt_input_text.to_owned());
    }

    let search_hits = match runtime_state
        .search_memory(MemorySearchRequest {
            principal: context.principal.clone(),
            channel: context.channel.clone(),
            session_id: Some(session_id.to_owned()),
            query: memory_query_text.to_owned(),
            top_k: memory_config.auto_inject_max_items,
            min_score: MEMORY_AUTO_INJECT_MIN_SCORE,
            tags: Vec::new(),
            sources: Vec::new(),
        })
        .await
    {
        Ok(hits) => hits,
        Err(error) => {
            warn!(
                run_id,
                principal = %context.principal,
                session_id,
                status_code = ?error.code(),
                status_message = %error.message(),
                "memory auto-inject search failed"
            );
            return Ok(prompt_input_text.to_owned());
        }
    };
    if search_hits.is_empty() {
        return Ok(prompt_input_text.to_owned());
    }

    let selected_hits =
        search_hits.into_iter().take(memory_config.auto_inject_max_items).collect::<Vec<_>>();

    runtime_state
        .append_orchestrator_tape_event(OrchestratorTapeAppendRequest {
            run_id: run_id.to_owned(),
            seq: *tape_seq,
            event_type: "memory_auto_inject".to_owned(),
            payload_json: memory_auto_inject_tape_payload(
                memory_query_text,
                selected_hits.as_slice(),
            ),
        })
        .await?;
    *tape_seq = tape_seq.saturating_add(1);
    runtime_state.record_memory_auto_inject_event();

    Ok(render_memory_augmented_prompt(selected_hits.as_slice(), prompt_input_text))
}

fn extract_previous_run_turn_from_tape_event(
    event: &OrchestratorTapeRecord,
) -> Option<(&'static str, String)> {
    let payload = serde_json::from_str::<Value>(event.payload_json.as_str()).ok()?;
    let (speaker, raw_text) = match event.event_type.as_str() {
        "message.received" => ("user", payload.get("text").and_then(Value::as_str)?),
        "message.replied" => ("assistant", payload.get("reply_text").and_then(Value::as_str)?),
        _ => return None,
    };
    let normalized = raw_text.replace(['\r', '\n'], " ");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some((
        speaker,
        truncate_with_ellipsis(trimmed.to_owned(), MAX_PREVIOUS_RUN_CONTEXT_ENTRY_CHARS),
    ))
}

#[allow(clippy::result_large_err)]
pub(crate) async fn build_previous_run_context_prompt(
    runtime_state: &Arc<GatewayRuntimeState>,
    previous_run_id: Option<&str>,
    input_text: &str,
) -> Result<String, Status> {
    let Some(previous_run_id) = previous_run_id else {
        return Ok(input_text.to_owned());
    };
    let tape_snapshot = match runtime_state
        .orchestrator_tape_snapshot(
            previous_run_id.to_owned(),
            None,
            Some(MAX_PREVIOUS_RUN_CONTEXT_TAPE_EVENTS),
        )
        .await
    {
        Ok(snapshot) => snapshot,
        Err(error) if error.code() == tonic::Code::NotFound => return Ok(input_text.to_owned()),
        Err(error) => return Err(error),
    };

    let mut turns = tape_snapshot
        .events
        .iter()
        .filter_map(extract_previous_run_turn_from_tape_event)
        .collect::<Vec<_>>();
    if turns.is_empty() {
        return Ok(input_text.to_owned());
    }
    if turns.len() > MAX_PREVIOUS_RUN_CONTEXT_TURNS {
        let keep_from = turns.len() - MAX_PREVIOUS_RUN_CONTEXT_TURNS;
        turns.drain(0..keep_from);
    }

    let mut block = String::from("<recent_conversation>\n");
    for (index, (speaker, text)) in turns.iter().enumerate() {
        block.push_str(format!("{}. {}: {text}\n", index + 1, speaker).as_str());
    }
    block.push_str("</recent_conversation>");
    Ok(format!("{block}\n\n{input_text}"))
}

#[allow(clippy::result_large_err)]
pub(crate) async fn prepare_model_provider_input(
    runtime_state: &Arc<GatewayRuntimeState>,
    context: &RequestContext,
    request: PrepareModelProviderInputRequest<'_>,
) -> Result<PreparedModelProviderInput, Status> {
    let PrepareModelProviderInputRequest {
        run_id,
        tape_seq,
        session_id,
        previous_run_id,
        input_text,
        attachments,
        memory_ingest_reason,
        memory_prompt_failure_mode,
        channel_for_log,
    } = request;
    ingest_memory_best_effort(
        runtime_state,
        context.principal.as_str(),
        context.channel.as_deref(),
        Some(session_id),
        MemorySource::TapeUserMessage,
        input_text,
        Vec::new(),
        Some(0.9),
        memory_ingest_reason,
    )
    .await;
    let input_with_recent_context =
        match build_previous_run_context_prompt(runtime_state, previous_run_id, input_text).await {
            Ok(value) => value,
            Err(error) => {
                warn!(
                    run_id,
                    principal = %context.principal,
                    session_id,
                    previous_run_id = %previous_run_id.unwrap_or("n/a"),
                    channel = channel_for_log,
                    status_code = ?error.code(),
                    status_message = %error.message(),
                    "failed to enrich prompt with previous-run context; continuing with raw input"
                );
                input_text.to_owned()
            }
        };
    let provider_input_text = match build_memory_augmented_prompt(
        runtime_state,
        context,
        run_id,
        tape_seq,
        session_id,
        input_text,
        input_with_recent_context.as_str(),
    )
    .await
    {
        Ok(value) => value,
        Err(error) => match memory_prompt_failure_mode {
            MemoryPromptFailureMode::Fail => return Err(error),
            MemoryPromptFailureMode::FallbackToRawInput { warn_message } => {
                warn!(
                    run_id,
                    principal = %context.principal,
                    session_id,
                    channel = channel_for_log,
                    status_code = ?error.code(),
                    status_message = %error.message(),
                    "{warn_message}"
                );
                input_text.to_owned()
            }
        },
    };
    Ok(PreparedModelProviderInput {
        provider_input_text,
        vision_inputs: build_provider_image_inputs(attachments, &runtime_state.config.media),
    })
}

pub(crate) fn render_memory_augmented_prompt(hits: &[MemorySearchHit], input_text: &str) -> String {
    let mut context_lines = Vec::with_capacity(hits.len());
    for (index, hit) in hits.iter().enumerate() {
        let snippet = hit.snippet.replace(['\r', '\n'], " ").trim().to_owned();
        context_lines.push(format!(
            "{}. id={} source={} score={:.4} created_at_unix_ms={} snippet={}",
            index + 1,
            hit.item.memory_id,
            hit.item.source.as_str(),
            hit.score,
            hit.item.created_at_unix_ms,
            truncate_with_ellipsis(snippet, 256),
        ));
    }
    let mut block = String::from("<memory_context>\n");
    block.push_str(context_lines.join("\n").as_str());
    block.push_str("\n</memory_context>");
    format!("{block}\n\n{input_text}")
}

pub(crate) fn memory_auto_inject_tape_payload(query: &str, hits: &[MemorySearchHit]) -> String {
    let payload = json!({
        "query": truncate_with_ellipsis(query.to_owned(), 512),
        "injected_count": hits.len(),
        "hits": hits.iter().map(|hit| {
            json!({
                "memory_id": hit.item.memory_id,
                "source": hit.item.source.as_str(),
                "score": hit.score,
                "created_at_unix_ms": hit.item.created_at_unix_ms,
                "snippet": truncate_with_ellipsis(hit.snippet.clone(), 256),
            })
        }).collect::<Vec<_>>(),
    })
    .to_string();
    crate::journal::redact_payload_json(payload.as_bytes()).unwrap_or(payload)
}
