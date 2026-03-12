use crate::*;

pub(crate) async fn console_pairing_summary_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<control_plane::PairingSummaryEnvelope>, Response> {
    let _session = authorize_console_session(&state, &headers, false)?;
    Ok(Json(control_plane::PairingSummaryEnvelope {
        contract: contract_descriptor(),
        channels: state
            .runtime
            .channel_router_pairing_snapshot(None)
            .iter()
            .map(control_plane_pairing_snapshot_from_runtime)
            .collect(),
    }))
}

pub(crate) async fn console_pairing_code_mint_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<control_plane::PairingCodeMintRequest>,
) -> Result<Json<control_plane::PairingSummaryEnvelope>, Response> {
    let session = authorize_console_session(&state, &headers, true)?;
    state
        .runtime
        .channel_router_mint_pairing_code(
            payload.channel.as_str(),
            payload.issued_by.as_deref().unwrap_or(session.context.principal.as_str()),
            payload.ttl_ms,
        )
        .map_err(runtime_status_response)?;
    Ok(Json(control_plane::PairingSummaryEnvelope {
        contract: contract_descriptor(),
        channels: state
            .runtime
            .channel_router_pairing_snapshot(Some(payload.channel.as_str()))
            .iter()
            .map(control_plane_pairing_snapshot_from_runtime)
            .collect(),
    }))
}
