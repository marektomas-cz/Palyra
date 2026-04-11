use serde_json::{json, Value};

use crate::{
    app::state::AppState,
    application::channels::providers::{
        build_channel_provider_health_refresh_payload, build_channel_provider_operations_payload,
        find_matching_message,
    },
    *,
};

#[allow(clippy::result_large_err)]
pub(crate) fn build_channel_status_payload(
    state: &AppState,
    connector_id: &str,
) -> Result<Value, Response> {
    let connector = state.channels.status(connector_id).map_err(channel_platform_error_response)?;
    let runtime =
        state.channels.runtime_snapshot(connector_id).map_err(channel_platform_error_response)?;
    let queue =
        state.channels.queue_snapshot(connector_id).map_err(channel_platform_error_response)?;
    let recent_dead_letters = state
        .channels
        .dead_letters(connector_id, Some(5))
        .map_err(channel_platform_error_response)?;
    Ok(json!({
        "connector": connector,
        "runtime": runtime,
        "operations": build_channel_operations_snapshot(
            connector_id,
            &connector,
            runtime.as_ref(),
            &queue,
            recent_dead_letters.as_slice(),
        ),
    }))
}

#[allow(clippy::result_large_err)]
pub(crate) async fn build_channel_health_refresh_payload(
    state: &AppState,
    connector_id: &str,
    verify_channel_id: Option<String>,
) -> Result<Value, Response> {
    let mut payload = build_channel_status_payload(state, connector_id)?;
    payload["health_refresh"] =
        build_channel_provider_health_refresh_payload(state, connector_id, verify_channel_id)
            .await?;
    Ok(payload)
}

fn build_channel_operations_snapshot(
    connector_id: &str,
    connector: &palyra_connectors::ConnectorStatusSnapshot,
    runtime: Option<&Value>,
    queue: &palyra_connectors::ConnectorQueueSnapshot,
    recent_dead_letters: &[palyra_connectors::DeadLetterRecord],
) -> Value {
    let last_runtime_error = runtime
        .and_then(|payload| payload.get("last_error"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let runtime_global_retry_after_ms = runtime
        .and_then(|payload| payload.get("global_retry_after_ms"))
        .and_then(Value::as_i64)
        .filter(|value| *value > 0);
    let active_route_limits = runtime
        .and_then(|payload| payload.get("route_rate_limits"))
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    entry
                        .get("retry_after_ms")
                        .and_then(Value::as_i64)
                        .is_some_and(|value| value > 0)
                })
                .count()
        })
        .unwrap_or(0);
    let last_auth_failure = find_matching_message(
        [
            connector.last_error.as_deref(),
            last_runtime_error.as_deref(),
            recent_dead_letters.first().map(|entry| entry.reason.as_str()),
        ],
        &["auth", "token", "unauthorized", "credential missing", "missing credential"],
    );
    let mut saturation_reasons = Vec::new();
    let saturation_state = if !connector.enabled {
        saturation_reasons.push("connector_disabled".to_owned());
        "paused"
    } else if queue.paused {
        saturation_reasons.push("queue_paused".to_owned());
        if let Some(reason) = queue.pause_reason.as_deref() {
            saturation_reasons.push(format!("pause_reason={reason}"));
        }
        "paused"
    } else if queue.dead_letters > 0 {
        saturation_reasons.push(format!("dead_letters={}", queue.dead_letters));
        "dead_lettered"
    } else if runtime_global_retry_after_ms.is_some() || active_route_limits > 0 {
        if let Some(wait_ms) = runtime_global_retry_after_ms {
            saturation_reasons.push(format!("global_retry_after_ms={wait_ms}"));
        }
        if active_route_limits > 0 {
            saturation_reasons.push(format!("active_route_limits={active_route_limits}"));
        }
        "rate_limited"
    } else if queue.claimed_outbox > 0 || queue.due_outbox > 0 {
        if queue.claimed_outbox > 0 {
            saturation_reasons.push(format!("claimed_outbox={}", queue.claimed_outbox));
        }
        if queue.due_outbox > 0 {
            saturation_reasons.push(format!("due_outbox={}", queue.due_outbox));
        }
        "backpressure"
    } else if queue.pending_outbox > 0 {
        saturation_reasons.push(format!("pending_outbox={}", queue.pending_outbox));
        "retrying"
    } else {
        "healthy"
    };
    if let Some(error) = &connector.last_error {
        saturation_reasons.push(format!("last_error={error}"));
    } else if let Some(error) = &last_runtime_error {
        saturation_reasons.push(format!("runtime_error={error}"));
    }
    let provider = build_channel_provider_operations_payload(
        connector_id,
        connector,
        runtime,
        recent_dead_letters,
    );
    json!({
        "queue": {
            "pending_outbox": queue.pending_outbox,
            "due_outbox": queue.due_outbox,
            "claimed_outbox": queue.claimed_outbox,
            "dead_letters": queue.dead_letters,
            "paused": queue.paused,
            "pause_reason": queue.pause_reason,
            "pause_updated_at_unix_ms": queue.pause_updated_at_unix_ms,
            "next_attempt_unix_ms": queue.next_attempt_unix_ms,
            "oldest_pending_created_at_unix_ms": queue.oldest_pending_created_at_unix_ms,
            "latest_dead_letter_unix_ms": queue.latest_dead_letter_unix_ms,
        },
        "saturation": {
            "state": saturation_state,
            "reasons": saturation_reasons,
        },
        "last_auth_failure": last_auth_failure,
        "rate_limits": {
            "global_retry_after_ms": runtime_global_retry_after_ms,
            "active_route_limits": active_route_limits,
            "routes": runtime.and_then(|payload| payload.get("route_rate_limits")).cloned(),
        },
        "discord": provider,
    })
}
