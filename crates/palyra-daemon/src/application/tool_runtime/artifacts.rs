use std::sync::Arc;

use palyra_common::runtime_contracts::{ArtifactReadRequest, ToolTurnBudget};
use serde_json::json;

use crate::{
    gateway::{GatewayRuntimeState, ToolRuntimeExecutionContext, ARTIFACT_READ_TOOL_NAME},
    journal::ToolResultArtifactReadRequest,
    tool_protocol::{build_tool_execution_outcome, ToolExecutionOutcome},
};

const TOOL_RESULT_ARTIFACT_TRUNCATION_RESERVE_BYTES: usize = 2 * 1024;
const TOOL_RESULT_ARTIFACT_TRUNCATION_MESSAGE: &str =
    "Original tool output exceeded the journal artifact payload limit; this artifact stores a bounded UTF-8 prefix.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundedToolResultArtifactContent {
    pub content: Vec<u8>,
    pub original_output_bytes: usize,
    pub stored_output_bytes: usize,
    pub truncated: bool,
}

pub(crate) fn bounded_tool_result_artifact_content(
    output_json: &[u8],
    max_payload_bytes: usize,
) -> Result<BoundedToolResultArtifactContent, String> {
    if output_json.len() <= max_payload_bytes {
        return Ok(BoundedToolResultArtifactContent {
            content: output_json.to_vec(),
            original_output_bytes: output_json.len(),
            stored_output_bytes: output_json.len(),
            truncated: false,
        });
    }

    let mut prefix_limit = output_json
        .len()
        .min(max_payload_bytes.saturating_sub(TOOL_RESULT_ARTIFACT_TRUNCATION_RESERVE_BYTES));
    loop {
        let stored_prefix = truncate_utf8_lossy(output_json, prefix_limit);
        let stored_output_bytes = stored_prefix.len();
        let content = serde_json::to_vec(&json!({
            "schema_version": 1,
            "artifact_content_truncated": true,
            "original_output_bytes": output_json.len(),
            "stored_output_bytes": stored_output_bytes,
            "stored_output_utf8_prefix": stored_prefix,
            "message": TOOL_RESULT_ARTIFACT_TRUNCATION_MESSAGE,
        }))
        .map_err(|error| format!("failed to serialize truncated tool result artifact: {error}"))?;
        if content.len() <= max_payload_bytes {
            return Ok(BoundedToolResultArtifactContent {
                content,
                original_output_bytes: output_json.len(),
                stored_output_bytes,
                truncated: true,
            });
        }
        if prefix_limit == 0 {
            break;
        }
        prefix_limit /= 2;
    }

    let content = serde_json::to_vec(&json!({
        "schema_version": 1,
        "artifact_content_truncated": true,
        "original_output_bytes": output_json.len(),
        "stored_output_bytes": 0,
        "message": TOOL_RESULT_ARTIFACT_TRUNCATION_MESSAGE,
    }))
    .map_err(|error| format!("failed to serialize truncated tool result artifact: {error}"))?;
    if content.len() <= max_payload_bytes {
        Ok(BoundedToolResultArtifactContent {
            content,
            original_output_bytes: output_json.len(),
            stored_output_bytes: 0,
            truncated: true,
        })
    } else {
        Err(format!(
            "tool result artifact payload limit ({max_payload_bytes} bytes) is too small to store truncation metadata"
        ))
    }
}

fn truncate_utf8_lossy(output_json: &[u8], max_bytes: usize) -> String {
    let value = String::from_utf8_lossy(output_json);
    if value.len() <= max_bytes {
        return value.into_owned();
    }
    value
        .char_indices()
        .take_while(|(index, ch)| index.saturating_add(ch.len_utf8()) <= max_bytes)
        .map(|(_, ch)| ch)
        .collect()
}

pub(crate) async fn execute_artifact_read_tool(
    runtime_state: &Arc<GatewayRuntimeState>,
    context: ToolRuntimeExecutionContext<'_>,
    proposal_id: &str,
    input_json: &[u8],
) -> ToolExecutionOutcome {
    let request = match serde_json::from_slice::<ArtifactReadRequest>(input_json) {
        Ok(request) if !request.artifact_id.trim().is_empty() => request,
        Ok(_) => {
            return artifact_read_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.artifact.read requires non-empty artifact_id".to_owned(),
            );
        }
        Err(error) => {
            return artifact_read_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!("palyra.artifact.read input must match artifact read schema: {error}"),
            );
        }
    };

    let budget = ToolTurnBudget::default();
    let requested_max = usize::try_from(request.max_bytes)
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or(budget.max_artifact_read_bytes)
        .min(budget.max_artifact_read_bytes);

    let read = ToolResultArtifactReadRequest {
        artifact_id: request.artifact_id,
        session_id: context.session_id.to_owned(),
        run_id: context.run_id.to_owned(),
        principal: context.principal.to_owned(),
        device_id: context.device_id.to_owned(),
        channel: context.channel.map(ToOwned::to_owned),
        expected_digest_sha256: request.expected_digest_sha256,
        offset_bytes: request.offset_bytes,
        max_bytes: requested_max,
        text_preview: request.text_preview,
    };

    match runtime_state.read_tool_result_artifact(read).await {
        Ok(response) => match serde_json::to_vec(&response) {
            Ok(output_json) => {
                artifact_read_outcome(proposal_id, input_json, true, output_json, String::new())
            }
            Err(error) => artifact_read_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!("failed to serialize artifact read response: {error}"),
            ),
        },
        Err(status) => {
            let output_json = serde_json::to_vec(&json!({
                "error": {
                    "code": format!("{:?}", status.code()).to_ascii_lowercase(),
                    "message": status.message(),
                }
            }))
            .unwrap_or_else(|_| b"{}".to_vec());
            artifact_read_outcome(
                proposal_id,
                input_json,
                false,
                output_json,
                format!("artifact read failed: {}", status.message()),
            )
        }
    }
}

fn artifact_read_outcome(
    proposal_id: &str,
    input_json: &[u8],
    success: bool,
    output_json: Vec<u8>,
    error: String,
) -> ToolExecutionOutcome {
    build_tool_execution_outcome(
        proposal_id,
        ARTIFACT_READ_TOOL_NAME,
        input_json,
        success,
        output_json,
        error,
        false,
        "gateway_artifacts".to_owned(),
        "artifact_scope".to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::bounded_tool_result_artifact_content;
    use serde_json::{json, Value};

    #[test]
    fn bounded_tool_result_artifact_content_keeps_small_payload_raw() {
        let output_json = serde_json::to_vec(&json!({"ok": true, "value": "small"}))
            .expect("test json should serialize");

        let content = bounded_tool_result_artifact_content(output_json.as_slice(), 256 * 1024)
            .expect("small payload should fit");

        assert!(!content.truncated);
        assert_eq!(content.original_output_bytes, output_json.len());
        assert_eq!(content.stored_output_bytes, output_json.len());
        assert_eq!(content.content, output_json);
    }

    #[test]
    fn bounded_tool_result_artifact_content_caps_large_payload_as_valid_json() {
        let output_json = serde_json::to_vec(&json!({
            "items": ["x".repeat(385_000)],
        }))
        .expect("test json should serialize");
        let max_payload_bytes = 256 * 1024;

        let content =
            bounded_tool_result_artifact_content(output_json.as_slice(), max_payload_bytes)
                .expect("large payload should be bounded");

        assert!(content.truncated);
        assert!(content.content.len() <= max_payload_bytes);
        assert_eq!(content.original_output_bytes, output_json.len());
        assert!(content.stored_output_bytes < content.original_output_bytes);

        let projected = serde_json::from_slice::<Value>(content.content.as_slice())
            .expect("bounded artifact content should stay valid JSON");
        assert_eq!(
            projected.pointer("/artifact_content_truncated").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            projected.pointer("/original_output_bytes").and_then(Value::as_u64),
            Some(output_json.len() as u64)
        );
        assert!(
            projected.pointer("/stored_output_utf8_prefix").and_then(Value::as_str).is_some(),
            "truncated artifact should retain a bounded useful prefix"
        );
    }
}
