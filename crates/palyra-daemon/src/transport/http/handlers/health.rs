use axum::{extract::State, response::IntoResponse, Json};
use palyra_common::{health_response, HealthResponse};

use crate::app::state::AppState;

pub(crate) async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json::<HealthResponse>(health_response("palyrad", state.started_at))
}
