use crate::*;

pub(crate) async fn admin_status_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    state.runtime.record_admin_status_request();
    let snapshot = state
        .runtime
        .status_snapshot_async(context.clone(), state.auth.clone())
        .await
        .map_err(runtime_status_response)?;
    let auth_snapshot = state
        .auth_runtime
        .admin_status_snapshot(Arc::clone(&state.runtime))
        .await
        .map_err(runtime_status_response)?;
    let mut payload = serde_json::to_value(&snapshot).map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to serialize admin status snapshot: {error}"
        )))
    })?;
    let auth_payload = serde_json::to_value(auth_snapshot).map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to serialize auth status snapshot: {error}"
        )))
    })?;
    let media_payload = state.channels.media_snapshot().map_err(channel_platform_error_response)?;
    let observability_payload = build_observability_payload(
        &state,
        &context,
        &snapshot.model_provider,
        &auth_payload,
        &media_payload,
    )
    .await?;
    let tool_jobs = state
        .runtime
        .list_tool_jobs(crate::journal::ToolJobsListFilter {
            owner_principal: Some(context.principal.clone()),
            session_id: None,
            run_id: None,
            include_terminal: true,
            limit: 256,
        })
        .await
        .map_err(runtime_status_response)?;
    let generated_at_unix_ms = unix_ms_now().map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to read system clock: {error}"
        )))
    })?;
    let memory_payload = json!({
        "usage": {
            "entries": snapshot.counters.memory_items_ingested,
            "bytes": 0,
        },
        "providers": [],
    });
    let skills_payload = collect_console_skills_diagnostics(&state).await;
    let plugins_payload = collect_console_plugins_diagnostics();
    let networked_workers_payload = collect_console_networked_worker_diagnostics(&state);
    let null_payload = Value::Null;
    let runtime_preview_payload =
        observability_payload.pointer("/runtime_preview").unwrap_or(&null_payload);
    let support_bundle_payload =
        observability_payload.pointer("/support_bundle").unwrap_or(&null_payload);
    let runtime_health = crate::runtime_diagnostics::build_runtime_health_snapshot(
        generated_at_unix_ms,
        &snapshot,
        &auth_payload,
        &memory_payload,
        &skills_payload,
        &plugins_payload,
        &networked_workers_payload,
        support_bundle_payload,
        runtime_preview_payload,
        &tool_jobs,
    );
    let runtime_metrics = crate::runtime_diagnostics::build_agent_runtime_metrics_snapshot(
        &snapshot,
        runtime_preview_payload,
        &memory_payload,
        &tool_jobs,
    );
    if let Value::Object(ref mut map) = payload {
        map.insert("auth".to_owned(), auth_payload);
        map.insert("media".to_owned(), media_payload);
        map.insert("observability".to_owned(), observability_payload);
        map.insert(
            "runtime_health".to_owned(),
            serde_json::to_value(runtime_health).map_err(|error| {
                runtime_status_response(tonic::Status::internal(format!(
                    "failed to serialize runtime health snapshot: {error}"
                )))
            })?,
        );
        map.insert("agent_runtime_metrics".to_owned(), runtime_metrics);
    }
    Ok(Json(payload))
}

pub(crate) async fn admin_metrics_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    state.runtime.record_admin_status_request();
    let snapshot = state
        .runtime
        .status_snapshot_async(context.clone(), state.auth.clone())
        .await
        .map_err(runtime_status_response)?;
    let tool_jobs = state
        .runtime
        .list_tool_jobs(crate::journal::ToolJobsListFilter {
            owner_principal: Some(context.principal),
            session_id: None,
            run_id: None,
            include_terminal: true,
            limit: 256,
        })
        .await
        .map_err(runtime_status_response)?;
    let body = crate::runtime_diagnostics::render_prometheus_metrics(&snapshot, &tool_jobs);
    Ok(([(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], body).into_response())
}

pub(crate) async fn admin_journal_recent_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<JournalRecentQuery>,
) -> Result<Json<gateway::JournalRecentSnapshot>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let _context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    state.runtime.record_admin_status_request();
    let limit = query.limit.unwrap_or(20);
    let snapshot =
        state.runtime.recent_journal_snapshot(limit).await.map_err(runtime_status_response)?;
    Ok(Json(snapshot))
}

pub(crate) async fn admin_policy_explain_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PolicyExplainQuery>,
) -> Result<Json<PolicyExplainResponse>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let _context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    state.runtime.record_admin_status_request();

    let request = PolicyRequest {
        principal: query.principal,
        action: query.action,
        resource: query.resource,
    };
    let evaluation =
        evaluate_with_config(&request, &PolicyEvaluationConfig::default()).map_err(|error| {
            runtime_status_response(tonic::Status::internal(format!(
                "failed to evaluate policy with Cedar engine: {error}"
            )))
        })?;
    let diagnostics = palyra_policy::policy_explain_diagnostics_value(&request, &evaluation);
    let (decision, approval_required, reason) = match &evaluation.decision {
        PolicyDecision::Allow => ("allow".to_owned(), false, evaluation.explanation.reason.clone()),
        PolicyDecision::DenyByDefault { reason } => {
            ("deny_by_default".to_owned(), true, reason.clone())
        }
    };

    Ok(Json(PolicyExplainResponse {
        principal: request.principal,
        action: request.action,
        resource: request.resource,
        decision,
        approval_required,
        reason,
        matched_policies: evaluation.explanation.matched_policy_ids,
        diagnostics,
    }))
}

pub(crate) async fn admin_run_status_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<OrchestratorRunStatusSnapshot>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let _context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    validate_canonical_id(run_id.as_str()).map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument("run_id must be a canonical ULID"))
    })?;
    state.runtime.record_admin_status_request();
    let snapshot = state
        .runtime
        .orchestrator_run_status_snapshot(run_id.clone())
        .await
        .map_err(runtime_status_response)?;
    let Some(snapshot) = snapshot else {
        return Err(runtime_status_response(tonic::Status::not_found(format!(
            "orchestrator run not found: {run_id}"
        ))));
    };
    Ok(Json(snapshot))
}

pub(crate) async fn admin_run_tape_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Query(query): Query<RunTapeQuery>,
) -> Result<Json<gateway::RunTapeSnapshot>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let _context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    validate_canonical_id(run_id.as_str()).map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument("run_id must be a canonical ULID"))
    })?;
    state.runtime.record_admin_status_request();
    let snapshot = state
        .runtime
        .orchestrator_tape_snapshot(run_id, query.after_seq, query.limit)
        .await
        .map_err(runtime_status_response)?;
    Ok(Json(snapshot))
}

pub(crate) async fn admin_run_cancel_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    payload: Option<Json<RunCancelRequest>>,
) -> Result<Json<gateway::RunCancelSnapshot>, Response> {
    authorize_headers(&headers, &state.auth).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    let _context = request_context_from_headers(&headers).map_err(|error| {
        state.runtime.record_denied();
        auth_error_response(error)
    })?;
    validate_canonical_id(run_id.as_str()).map_err(|_| {
        runtime_status_response(tonic::Status::invalid_argument("run_id must be a canonical ULID"))
    })?;
    state.runtime.record_admin_status_request();
    let reason = payload
        .and_then(|body| body.0.reason)
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        })
        .unwrap_or_else(|| "admin_cancel_requested".to_owned());
    let response = state
        .runtime
        .request_orchestrator_cancel(OrchestratorCancelRequest { run_id, reason })
        .await
        .map_err(runtime_status_response)?;
    Ok(Json(response))
}
