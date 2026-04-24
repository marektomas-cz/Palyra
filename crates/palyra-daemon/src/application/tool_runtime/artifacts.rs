use std::sync::Arc;

use palyra_common::runtime_contracts::{ArtifactReadRequest, ToolTurnBudget};
use serde_json::json;

use crate::{
    gateway::{GatewayRuntimeState, ToolRuntimeExecutionContext, ARTIFACT_READ_TOOL_NAME},
    journal::ToolResultArtifactReadRequest,
    tool_protocol::{build_tool_execution_outcome, ToolExecutionOutcome},
};

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
