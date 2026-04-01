use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use ulid::Ulid;

use crate::{
    application::{
        memory::redact_memory_text_for_output, service_authorization::authorize_memory_action,
    },
    gateway::{
        current_unix_ms, GatewayRuntimeState, MAX_MEMORY_SEARCH_TOP_K, MAX_MEMORY_TOOL_QUERY_BYTES,
        MAX_MEMORY_TOOL_TAGS,
    },
    journal::{MemorySearchHit, MemorySearchRequest, MemorySource},
    tool_protocol::{ToolAttestation, ToolExecutionOutcome},
};

pub(crate) fn memory_search_tool_output_payload(search_hits: &[MemorySearchHit]) -> Value {
    json!({
        "hits": search_hits.iter().map(|hit| {
            json!({
                "memory_id": hit.item.memory_id,
                "source": hit.item.source.as_str(),
                "snippet": redact_memory_text_for_output(hit.snippet.as_str()),
                "score": hit.score,
                "created_at_unix_ms": hit.item.created_at_unix_ms,
                "content_text": redact_memory_text_for_output(hit.item.content_text.as_str()),
                "content_hash": hit.item.content_hash,
                "tags": hit.item.tags,
                "confidence": hit.item.confidence,
                "breakdown": {
                    "lexical_score": hit.breakdown.lexical_score,
                    "vector_score": hit.breakdown.vector_score,
                    "recency_score": hit.breakdown.recency_score,
                    "final_score": hit.breakdown.final_score,
                }
            })
        }).collect::<Vec<_>>()
    })
}

pub(crate) async fn execute_memory_search_tool(
    runtime_state: &Arc<GatewayRuntimeState>,
    principal: &str,
    channel: Option<&str>,
    session_id: &str,
    proposal_id: &str,
    input_json: &[u8],
) -> ToolExecutionOutcome {
    let parsed = match serde_json::from_slice::<Value>(input_json) {
        Ok(Value::Object(map)) => map,
        Ok(_) => {
            return memory_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.memory.search requires JSON object input".to_owned(),
            );
        }
        Err(error) => {
            return memory_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!("palyra.memory.search invalid JSON input: {error}"),
            );
        }
    };

    let query = match parsed.get("query").and_then(Value::as_str).map(str::trim) {
        Some(value) if !value.is_empty() => value.to_owned(),
        _ => {
            return memory_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.memory.search requires non-empty string field 'query'".to_owned(),
            );
        }
    };
    if query.len() > MAX_MEMORY_TOOL_QUERY_BYTES {
        return memory_tool_execution_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!("palyra.memory.search query exceeds {MAX_MEMORY_TOOL_QUERY_BYTES} bytes"),
        );
    }

    let scope = parsed.get("scope").and_then(Value::as_str).unwrap_or("session");
    let (channel_scope, session_scope, resource) = match scope {
        "principal" => {
            let channel_scope = channel.map(str::to_owned);
            let resource = channel_scope
                .as_deref()
                .map(|value| format!("memory:channel:{value}"))
                .unwrap_or_else(|| "memory:principal".to_owned());
            (channel_scope, None, resource)
        }
        "channel" => {
            let Some(channel) = channel.map(str::to_owned) else {
                return memory_tool_execution_outcome(
                    proposal_id,
                    input_json,
                    false,
                    b"{}".to_vec(),
                    "palyra.memory.search scope=channel requires authenticated channel context"
                        .to_owned(),
                );
            };
            let resource = format!("memory:channel:{channel}");
            (Some(channel), None, resource)
        }
        "session" => {
            let channel = channel.map(str::to_owned);
            let session = Some(session_id.to_owned());
            (channel, session, format!("memory:session:{session_id}"))
        }
        _ => {
            return memory_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.memory.search scope must be one of: session|channel|principal".to_owned(),
            );
        }
    };

    if let Err(error) = authorize_memory_action(principal, "memory.search", resource.as_str()) {
        return memory_tool_execution_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!("memory policy denied tool search request: {}", error.message()),
        );
    }

    let min_score = parsed.get("min_score").and_then(Value::as_f64).unwrap_or(0.0);
    if !min_score.is_finite() || !(0.0..=1.0).contains(&min_score) {
        return memory_tool_execution_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            "palyra.memory.search min_score must be in range 0.0..=1.0".to_owned(),
        );
    }
    let top_k = parsed
        .get("top_k")
        .and_then(Value::as_u64)
        .map(|value| (value as usize).clamp(1, MAX_MEMORY_SEARCH_TOP_K))
        .unwrap_or(8);
    let tags = match parsed.get("tags") {
        Some(Value::Array(values)) => {
            if values.len() > MAX_MEMORY_TOOL_TAGS {
                return memory_tool_execution_outcome(
                    proposal_id,
                    input_json,
                    false,
                    b"{}".to_vec(),
                    format!("palyra.memory.search tags exceeds limit ({})", MAX_MEMORY_TOOL_TAGS),
                );
            }
            let mut parsed_tags = Vec::new();
            for value in values {
                let Some(tag) = value.as_str() else {
                    return memory_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        "palyra.memory.search tags must be strings".to_owned(),
                    );
                };
                if !tag.trim().is_empty() {
                    parsed_tags.push(tag.trim().to_owned());
                }
            }
            parsed_tags
        }
        Some(_) => {
            return memory_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.memory.search tags must be an array of strings".to_owned(),
            );
        }
        None => Vec::new(),
    };
    let sources = match parsed.get("sources") {
        Some(Value::Array(values)) => {
            let mut parsed_sources = Vec::new();
            for value in values {
                let Some(source) = value.as_str() else {
                    return memory_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        "palyra.memory.search sources must be an array of strings".to_owned(),
                    );
                };
                let Some(memory_source) = parse_memory_source_literal(source) else {
                    return memory_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        format!("palyra.memory.search unknown source value: {source}"),
                    );
                };
                parsed_sources.push(memory_source);
            }
            parsed_sources
        }
        Some(_) => {
            return memory_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.memory.search sources must be an array of strings".to_owned(),
            );
        }
        None => Vec::new(),
    };

    let search_hits = match runtime_state
        .search_memory(MemorySearchRequest {
            principal: principal.to_owned(),
            channel: channel_scope,
            session_id: session_scope,
            query,
            top_k,
            min_score,
            tags,
            sources,
        })
        .await
    {
        Ok(hits) => hits,
        Err(error) => {
            return memory_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!("palyra.memory.search failed: {}", error.message()),
            );
        }
    };

    let payload = memory_search_tool_output_payload(search_hits.as_slice());
    match serde_json::to_vec(&payload) {
        Ok(output_json) => {
            memory_tool_execution_outcome(proposal_id, input_json, true, output_json, String::new())
        }
        Err(error) => memory_tool_execution_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!("palyra.memory.search failed to serialize output: {error}"),
        ),
    }
}

fn parse_memory_source_literal(raw: &str) -> Option<MemorySource> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "tape:user_message" | "tape_user_message" | "user_message" => {
            Some(MemorySource::TapeUserMessage)
        }
        "tape:tool_result" | "tape_tool_result" | "tool_result" => {
            Some(MemorySource::TapeToolResult)
        }
        "summary" => Some(MemorySource::Summary),
        "manual" => Some(MemorySource::Manual),
        "import" => Some(MemorySource::Import),
        _ => None,
    }
}

fn memory_tool_execution_outcome(
    proposal_id: &str,
    input_json: &[u8],
    success: bool,
    output_json: Vec<u8>,
    error: String,
) -> ToolExecutionOutcome {
    let executed_at_unix_ms = current_unix_ms();
    let mut hasher = Sha256::new();
    hasher.update(b"palyra.memory.search.attestation.v1");
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
            executor: "memory_runtime".to_owned(),
            sandbox_enforcement: "none".to_owned(),
        },
    }
}
