use crate::*;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) runtime: Arc<BrowserRuntimeState>,
}

pub(crate) fn build_router(runtime: Arc<BrowserRuntimeState>) -> Router {
    Router::new().route("/healthz", get(health_handler)).with_state(AppState { runtime })
}

async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json::<HealthResponse>(health_response("palyra-browserd", state.runtime.started_at))
}
