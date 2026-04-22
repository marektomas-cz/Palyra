use crate::*;
use serde_json::{json, Value};

pub(crate) async fn console_memory_index_drift_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Response> {
    let _session = authorize_console_session(&state, &headers, false)?;
    let drift =
        state.runtime.external_retrieval_drift_report().await.map_err(runtime_status_response)?;
    let retrieval_backend =
        state.runtime.retrieval_backend_snapshot().map_err(runtime_status_response)?;
    Ok(Json(json!({
        "drift": drift,
        "external_index": retrieval_backend.external_index.clone(),
        "retrieval": {
            "backend": retrieval_backend,
        },
        "contract": contract_descriptor(),
    })))
}

pub(crate) async fn console_memory_index_reconcile_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConsoleMemoryIndexRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, true)?;
    let batch_size = payload.batch_size.unwrap_or(256).clamp(1, 10_000);
    let reconciliation = state
        .runtime
        .reconcile_external_retrieval_index(batch_size)
        .await
        .map_err(runtime_status_response)?;
    let retrieval_backend =
        state.runtime.retrieval_backend_snapshot().map_err(runtime_status_response)?;
    let event_details = json!({
        "batch_size": batch_size,
        "reconciliation": reconciliation.clone(),
    });
    if let Err(error) = state
        .runtime
        .record_console_event(&session.context, "memory.index.reconcile", event_details)
        .await
    {
        tracing::warn!(error = %error, "failed to record memory index reconciliation console event");
    }
    Ok(Json(json!({
        "reconciliation": reconciliation,
        "external_index": retrieval_backend.external_index.clone(),
        "retrieval": {
            "backend": retrieval_backend,
        },
        "contract": contract_descriptor(),
    })))
}
