use crate::*;
use serde_json::Value;

pub(crate) fn emit_memory_index_drift(payload: &Value, json_output: bool) -> Result<()> {
    if json_output {
        return output::print_json_pretty(
            payload,
            "failed to encode memory index drift payload as JSON",
        );
    }

    if let Some(drift) = payload.get("drift").filter(|value| !value.is_null()) {
        print_external_drift_line("memory.external_index.drift", drift);
    }
    if let Some(external_index) = memory_external_index_payload(payload) {
        print_external_index_line("memory.external_index", external_index);
    }
    std::io::stdout().flush().context("stdout flush failed")
}

pub(crate) fn emit_memory_index_reconcile(payload: &Value, json_output: bool) -> Result<()> {
    if json_output {
        return output::print_json_pretty(
            payload,
            "failed to encode memory index reconciliation payload as JSON",
        );
    }

    let reconciliation = payload.get("reconciliation").unwrap_or(&Value::Null);
    let success = reconciliation.get("success").and_then(Value::as_bool).unwrap_or(false);
    let checked_at_unix_ms = reconciliation
        .get("checked_at_unix_ms")
        .and_then(Value::as_i64)
        .map_or("none".to_owned(), |value| value.to_string());
    let drift_before =
        reconciliation.pointer("/drift_before/drift_count").and_then(Value::as_u64).unwrap_or(0);
    let drift_after =
        reconciliation.pointer("/drift_after/drift_count").and_then(Value::as_u64).unwrap_or(0);
    println!(
        "memory.external_index.reconcile success={} checked_at_unix_ms={} drift_before={} drift_after={}",
        success, checked_at_unix_ms, drift_before, drift_after
    );
    if let Some(external_index) = memory_external_index_payload(payload) {
        print_external_index_line("memory.external_index", external_index);
    }
    std::io::stdout().flush().context("stdout flush failed")
}

pub(crate) fn memory_external_index_payload(payload: &Value) -> Option<&Value> {
    payload
        .pointer("/external_index")
        .filter(|value| !value.is_null())
        .or_else(|| payload.pointer("/retrieval/external_index").filter(|value| !value.is_null()))
        .or_else(|| {
            payload.pointer("/retrieval/backend/external_index").filter(|value| !value.is_null())
        })
}

pub(crate) fn print_external_index_line(label: &str, external_index: &Value) {
    let state = external_index.get("state").and_then(Value::as_str).unwrap_or("unknown");
    let reason = external_index.get("reason").and_then(Value::as_str).unwrap_or("unknown");
    let indexed_memory_items =
        external_index.get("indexed_memory_items").and_then(Value::as_u64).unwrap_or(0);
    let indexed_workspace_chunks =
        external_index.get("indexed_workspace_chunks").and_then(Value::as_u64).unwrap_or(0);
    let drift_count = external_index.get("drift_count").and_then(Value::as_u64).unwrap_or(0);
    let freshness_lag_ms = external_index
        .pointer("/scale_slos/freshness_lag_ms")
        .and_then(Value::as_u64)
        .map_or("unknown".to_owned(), |value| value.to_string());
    let gate = external_index
        .pointer("/scale_slos/preview_gate_state")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    println!(
        "{label} state={} indexed_memory_items={} indexed_workspace_chunks={} drift_count={} freshness_lag_ms={} gate={} reason={}",
        state,
        indexed_memory_items,
        indexed_workspace_chunks,
        drift_count,
        freshness_lag_ms,
        gate,
        reason
    );
}

pub(crate) fn print_external_drift_line(label: &str, drift: &Value) {
    let drift_count = drift.get("drift_count").and_then(Value::as_u64).unwrap_or(0);
    let memory_drift = drift.get("memory_drift").and_then(Value::as_i64).unwrap_or(0);
    let workspace_chunk_drift =
        drift.get("workspace_chunk_drift").and_then(Value::as_i64).unwrap_or(0);
    let freshness_lag_ms = drift
        .get("freshness_lag_ms")
        .and_then(Value::as_u64)
        .map_or("unknown".to_owned(), |value| value.to_string());
    let reconciliation_required =
        drift.get("reconciliation_required").and_then(Value::as_bool).unwrap_or(false);
    println!(
        "{label} drift_count={} memory_drift={} workspace_chunk_drift={} freshness_lag_ms={} reconciliation_required={}",
        drift_count,
        memory_drift,
        workspace_chunk_drift,
        freshness_lag_ms,
        reconciliation_required
    );
}
