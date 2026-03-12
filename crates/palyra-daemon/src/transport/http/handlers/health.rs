use axum::{
    extract::State,
    response::{Html, IntoResponse},
    Json,
};
use palyra_common::{health_response, HealthResponse};

use crate::app::state::AppState;

pub(crate) async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json::<HealthResponse>(health_response("palyrad", state.started_at))
}

pub(crate) async fn dashboard_handoff_handler(State(state): State<AppState>) -> impl IntoResponse {
    let health = health_response("palyrad", state.started_at);
    Html(format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Palyra Dashboard Handoff</title>
    <style>
      :root {{
        color-scheme: light;
        font-family: "Segoe UI", "Helvetica Neue", sans-serif;
        background: #f4efe4;
        color: #1b1d20;
      }}
      body {{
        margin: 0;
        min-height: 100vh;
        background:
          radial-gradient(circle at top right, rgba(205, 122, 41, 0.18), transparent 32rem),
          linear-gradient(180deg, #f8f2e7 0%, #efe4d3 100%);
      }}
      main {{
        max-width: 48rem;
        margin: 0 auto;
        padding: 3rem 1.5rem 4rem;
      }}
      .panel {{
        background: rgba(255, 251, 245, 0.92);
        border: 1px solid rgba(27, 29, 32, 0.08);
        border-radius: 1.25rem;
        box-shadow: 0 1.25rem 3rem rgba(49, 41, 25, 0.12);
        padding: 1.5rem;
      }}
      h1 {{
        margin: 0 0 0.75rem;
        font-size: clamp(2rem, 6vw, 3.25rem);
        line-height: 1;
      }}
      p {{
        margin: 0 0 1rem;
        line-height: 1.55;
      }}
      ul {{
        margin: 1.25rem 0 0;
        padding-left: 1.2rem;
      }}
      li + li {{
        margin-top: 0.6rem;
      }}
      .badge {{
        display: inline-flex;
        align-items: center;
        gap: 0.45rem;
        border-radius: 999px;
        padding: 0.35rem 0.75rem;
        background: rgba(23, 124, 73, 0.12);
        color: #16553a;
        font-size: 0.92rem;
        font-weight: 600;
      }}
      a {{
        color: #0f4f8a;
      }}
      code {{
        font-family: "Cascadia Code", "Fira Code", monospace;
        font-size: 0.95em;
      }}
    </style>
  </head>
  <body>
    <main>
      <div class="panel">
        <div class="badge">Runtime {status}</div>
        <h1>Palyra Local Runtime</h1>
        <p>
          The local control plane is responding. Desktop Control Center owns startup, onboarding,
          recovery, and auth bootstrap on this machine.
        </p>
        <p>
          Use the desktop app for first-run workflows. The authenticated operator APIs remain
          available under <code>/console/v1/*</code> once a console session is established.
        </p>
        <ul>
          <li><a href="/healthz">Health endpoint</a></li>
          <li><a href="/console/v1/control-plane/capabilities">Capability catalog</a></li>
          <li><a href="/console/v1/diagnostics">Diagnostics snapshot</a></li>
        </ul>
      </div>
    </main>
  </body>
</html>"#,
        status = health.status
    ))
}
