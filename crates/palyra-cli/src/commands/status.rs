use crate::*;

pub(crate) fn run_status(
    url: Option<String>,
    grpc_url: Option<String>,
    admin: bool,
    token: Option<String>,
    principal: String,
    device_id: String,
    channel: Option<String>,
) -> Result<()> {
    let base_url = url
        .or_else(|| env::var("PALYRA_DAEMON_URL").ok())
        .unwrap_or_else(|| DEFAULT_DAEMON_URL.to_owned());
    let status_url = format!("{}/healthz", base_url.trim_end_matches('/'));
    let http_client = Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .context("failed to build HTTP client")?;
    let health = fetch_health_with_retry(&http_client, &status_url)?;
    println!(
        "status.http={} service={} version={} git_hash={} uptime_seconds={}",
        health.status, health.service, health.version, health.git_hash, health.uptime_seconds
    );

    let runtime = build_runtime()?;
    let grpc_health =
        runtime.block_on(fetch_grpc_health_with_retry(resolve_grpc_url(grpc_url)?))?;
    println!(
        "status.grpc={} service={} version={} git_hash={} uptime_seconds={}",
        grpc_health.status,
        grpc_health.service,
        grpc_health.version,
        grpc_health.git_hash,
        grpc_health.uptime_seconds
    );

    if admin {
        let admin_response = fetch_admin_status(
            &http_client,
            base_url.as_str(),
            token.or_else(|| env::var("PALYRA_ADMIN_TOKEN").ok()),
            principal,
            device_id,
            channel,
        )?;
        println!(
            "status.admin={} service={} grpc={}:{} quic_enabled={} denied_requests={} journal_events={}",
            admin_response.status,
            admin_response.service,
            admin_response.transport.grpc_bind_addr,
            admin_response.transport.grpc_port,
            admin_response.transport.quic_enabled,
            admin_response.counters.denied_requests,
            admin_response.counters.journal_events
        );
    }

    std::io::stdout().flush().context("stdout flush failed")
}
