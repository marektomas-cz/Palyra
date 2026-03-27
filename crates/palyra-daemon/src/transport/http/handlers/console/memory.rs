use crate::gateway::current_unix_ms;
use crate::journal::MemoryRetentionPolicy;
use crate::*;

pub(crate) async fn console_memory_status_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Response> {
    let _session = authorize_console_session(&state, &headers, false)?;
    let maintenance_status =
        state.runtime.memory_maintenance_status().await.map_err(runtime_status_response)?;
    let embeddings_status =
        state.runtime.memory_embeddings_status().await.map_err(runtime_status_response)?;
    let memory_config = state.runtime.memory_config_snapshot();
    let maintenance_interval_ms =
        i64::try_from(MEMORY_MAINTENANCE_INTERVAL.as_millis()).unwrap_or(i64::MAX);
    Ok(Json(json!({
        "usage": maintenance_status.usage,
        "embeddings": embeddings_status,
        "retention": {
            "max_entries": memory_config.retention_max_entries,
            "max_bytes": memory_config.retention_max_bytes,
            "ttl_days": memory_config.retention_ttl_days,
            "vacuum_schedule": memory_config.retention_vacuum_schedule,
        },
        "maintenance": {
            "interval_ms": maintenance_interval_ms,
            "last_run": maintenance_status.last_run,
            "last_vacuum_at_unix_ms": maintenance_status.last_vacuum_at_unix_ms,
            "next_vacuum_due_at_unix_ms": maintenance_status.next_vacuum_due_at_unix_ms,
            "next_run_at_unix_ms": maintenance_status.next_maintenance_run_at_unix_ms,
        }
    })))
}

pub(crate) async fn console_memory_index_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConsoleMemoryIndexRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, true)?;
    let batch_size = payload.batch_size.unwrap_or(64).clamp(1, 256);
    let until_complete = payload.until_complete.unwrap_or(false);
    let run_maintenance = payload.run_maintenance.unwrap_or(false);

    let maintenance =
        if run_maintenance { Some(run_memory_maintenance_now(&state).await?) } else { None };

    let mut outcome = state
        .runtime
        .run_memory_embeddings_backfill(batch_size)
        .await
        .map_err(runtime_status_response)?;
    let mut batches_executed = 1_u64;
    let mut scanned_count = outcome.scanned_count;
    let mut updated_count = outcome.updated_count;
    while until_complete && !outcome.is_complete() {
        outcome = state
            .runtime
            .run_memory_embeddings_backfill(batch_size)
            .await
            .map_err(runtime_status_response)?;
        scanned_count = scanned_count.saturating_add(outcome.scanned_count);
        updated_count = updated_count.saturating_add(outcome.updated_count);
        batches_executed = batches_executed.saturating_add(1);
    }
    let embeddings_status =
        state.runtime.memory_embeddings_status().await.map_err(runtime_status_response)?;
    let maintenance_payload = maintenance.as_ref().map(|outcome| {
        json!({
            "ran_at_unix_ms": outcome.ran_at_unix_ms,
            "deleted_expired_count": outcome.deleted_expired_count,
            "deleted_capacity_count": outcome.deleted_capacity_count,
            "deleted_total_count": outcome.deleted_total_count,
            "entries_before": outcome.entries_before,
            "entries_after": outcome.entries_after,
            "approx_bytes_before": outcome.approx_bytes_before,
            "approx_bytes_after": outcome.approx_bytes_after,
            "vacuum_performed": outcome.vacuum_performed,
            "last_vacuum_at_unix_ms": outcome.last_vacuum_at_unix_ms,
            "next_vacuum_due_at_unix_ms": outcome.next_vacuum_due_at_unix_ms,
            "next_maintenance_run_at_unix_ms": outcome.next_maintenance_run_at_unix_ms,
        })
    });
    let index_payload = json!({
        "ran_at_unix_ms": outcome.ran_at_unix_ms,
        "batch_size": outcome.batch_size,
        "batches_executed": batches_executed,
        "scanned_count": scanned_count,
        "updated_count": updated_count,
        "pending_count": outcome.pending_count,
        "complete": outcome.is_complete(),
        "target_model_id": outcome.target_model_id,
        "target_dims": outcome.target_dims,
        "target_version": outcome.target_version,
        "until_complete": until_complete,
    });
    let event_details = json!({
        "batch_size": batch_size,
        "until_complete": until_complete,
        "run_maintenance": run_maintenance,
        "index": index_payload.clone(),
        "maintenance": maintenance_payload.clone(),
    });
    if let Err(error) = state
        .runtime
        .record_console_event(&session.context, "memory.index.run", event_details)
        .await
    {
        warn!(error = %error, "failed to record memory index console event");
    }

    Ok(Json(json!({
        "maintenance": maintenance_payload,
        "index": index_payload,
        "embeddings": embeddings_status,
    })))
}

pub(crate) async fn console_memory_search_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ConsoleMemorySearchQuery>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, false)?;
    let search_query = query.query.trim();
    if search_query.is_empty() {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "query cannot be empty",
        )));
    }
    let min_score = query.min_score.unwrap_or(0.0);
    if !min_score.is_finite() || !(0.0..=1.0).contains(&min_score) {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "min_score must be in range 0.0..=1.0",
        )));
    }
    let session_scope = query.session_id.clone().and_then(trim_to_option);
    if let Some(session_scope) = session_scope.as_deref() {
        validate_canonical_id(session_scope).map_err(|_| {
            runtime_status_response(tonic::Status::invalid_argument(
                "session_id must be a canonical ULID",
            ))
        })?;
    }

    let sources = parse_memory_sources_csv(query.sources_csv.as_deref())?;
    let hits = state
        .runtime
        .search_memory(journal::MemorySearchRequest {
            principal: session.context.principal,
            channel: query.channel.or(session.context.channel),
            session_id: session_scope,
            query: search_query.to_owned(),
            top_k: query.top_k.unwrap_or(8).clamp(1, 50),
            min_score,
            tags: parse_csv_values(query.tags_csv.as_deref()),
            sources,
        })
        .await
        .map_err(runtime_status_response)?;
    Ok(Json(json!({ "hits": hits })))
}

pub(crate) async fn console_memory_purge_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ConsoleMemoryPurgeRequest>,
) -> Result<Json<Value>, Response> {
    let session = authorize_console_session(&state, &headers, true)?;
    let session_scope = payload.session_id.clone().and_then(trim_to_option);
    if let Some(session_scope) = session_scope.as_deref() {
        validate_canonical_id(session_scope).map_err(|_| {
            runtime_status_response(tonic::Status::invalid_argument(
                "session_id must be a canonical ULID",
            ))
        })?;
    }
    let purge_all_principal = payload.purge_all_principal.unwrap_or(false);
    if !purge_all_principal
        && payload.channel.as_deref().is_none_or(|value| value.trim().is_empty())
        && session_scope.is_none()
    {
        return Err(runtime_status_response(tonic::Status::invalid_argument(
            "purge request requires purge_all_principal=true or channel/session scope",
        )));
    }

    let deleted_count = state
        .runtime
        .purge_memory(MemoryPurgeRequest {
            principal: session.context.principal,
            channel: payload.channel,
            session_id: session_scope,
            purge_all_principal,
        })
        .await
        .map_err(runtime_status_response)?;
    Ok(Json(json!({ "deleted_count": deleted_count })))
}

#[allow(clippy::result_large_err)]
async fn run_memory_maintenance_now(
    state: &AppState,
) -> Result<crate::journal::MemoryMaintenanceOutcome, Response> {
    let now_unix_ms = current_unix_ms();
    let maintenance_status =
        state.runtime.memory_maintenance_status().await.map_err(runtime_status_response)?;
    let memory_config = state.runtime.memory_config_snapshot();
    state
        .runtime
        .run_memory_maintenance(
            now_unix_ms,
            MemoryRetentionPolicy {
                max_entries: memory_config.retention_max_entries,
                max_bytes: memory_config.retention_max_bytes,
                ttl_days: memory_config.retention_ttl_days,
            },
            maintenance_status.next_vacuum_due_at_unix_ms,
            Some(now_unix_ms.saturating_add(
                i64::try_from(MEMORY_MAINTENANCE_INTERVAL.as_millis()).unwrap_or(i64::MAX),
            )),
        )
        .await
        .map_err(runtime_status_response)
}
