use crate::{transport, *};

pub async fn run() -> Result<()> {
    init_tracing();
    let args = Args::parse();
    let runtime = Arc::new(BrowserRuntimeState::new(&args)?);
    spawn_cleanup_loop(Arc::clone(&runtime));

    let admin_address =
        parse_daemon_bind_socket(&args.bind, args.port).context("invalid bind address or port")?;
    let grpc_address = parse_daemon_bind_socket(&args.grpc_bind, args.grpc_port)
        .context("invalid gRPC bind address or port")?;
    enforce_non_loopback_bind_auth(admin_address, grpc_address, runtime.auth_token.is_some())?;

    let build = build_metadata();
    info!(
        service = "palyra-browserd",
        version = build.version,
        git_hash = build.git_hash,
        build_profile = build.build_profile,
        bind_addr = %args.bind,
        port = args.port,
        grpc_bind_addr = %args.grpc_bind,
        grpc_port = args.grpc_port,
        auth_enabled = runtime.auth_token.is_some(),
        state_persistence_enabled = runtime.state_store.is_some(),
        "browser service startup"
    );

    let http_router = transport::http::build_router(Arc::clone(&runtime));
    let grpc_service = transport::grpc::BrowserServiceImpl { runtime: Arc::clone(&runtime) };

    let admin_listener = tokio::net::TcpListener::bind(admin_address)
        .await
        .context("failed to bind browserd health listener")?;
    let grpc_listener = tokio::net::TcpListener::bind(grpc_address)
        .await
        .context("failed to bind browserd gRPC listener")?;

    info!(
        listen_addr = %admin_listener.local_addr().context("health local_addr")?,
        "browserd health endpoint ready"
    );
    info!(
        grpc_listen_addr = %grpc_listener.local_addr().context("grpc local_addr")?,
        "browserd gRPC endpoint ready"
    );

    let http_server =
        axum::serve(admin_listener, http_router).with_graceful_shutdown(shutdown_signal());
    let grpc_server = Server::builder()
        .add_service(browser_v1::browser_service_server::BrowserServiceServer::new(grpc_service))
        .serve_with_incoming_shutdown(TcpListenerStream::new(grpc_listener), shutdown_signal());

    let (http_result, grpc_result) = tokio::join!(http_server, grpc_server);
    http_result.context("browserd health server failed")?;
    grpc_result.context("browserd gRPC server failed")?;
    Ok(())
}

pub(crate) fn enforce_non_loopback_bind_auth(
    admin_address: SocketAddr,
    grpc_address: SocketAddr,
    auth_enabled: bool,
) -> Result<()> {
    if auth_enabled {
        return Ok(());
    }

    let admin_non_loopback = !admin_address.ip().is_loopback();
    let grpc_non_loopback = !grpc_address.ip().is_loopback();
    if admin_non_loopback || grpc_non_loopback {
        anyhow::bail!(
            "browser service auth token is required for non-loopback bindings (admin: {admin_address}, grpc: {grpc_address}); set --auth-token or PALYRA_BROWSERD_AUTH_TOKEN"
        );
    }

    Ok(())
}

pub(crate) fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().json().with_env_filter(filter).init();
}

pub(crate) fn spawn_cleanup_loop(runtime: Arc<BrowserRuntimeState>) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(CLEANUP_INTERVAL_MS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let now = Instant::now();
            let expired_ids = {
                let sessions = runtime.sessions.lock().await;
                sessions
                    .iter()
                    .filter_map(|(session_id, session)| {
                        let idle_alive =
                            now.saturating_duration_since(session.last_active) <= session.idle_ttl;
                        let lifetime_alive = now.saturating_duration_since(session.created_at)
                            <= Duration::from_millis(session.budget.max_session_lifetime_ms);
                        if idle_alive && lifetime_alive {
                            None
                        } else {
                            Some(session_id.clone())
                        }
                    })
                    .collect::<Vec<_>>()
            };
            if expired_ids.is_empty() {
                continue;
            }
            let removed_sessions = {
                let mut sessions = runtime.sessions.lock().await;
                expired_ids
                    .iter()
                    .filter_map(|session_id| sessions.remove(session_id.as_str()))
                    .collect::<Vec<_>>()
            };
            {
                let mut chromium_sessions = runtime.chromium_sessions.lock().await;
                for session_id in &expired_ids {
                    chromium_sessions.remove(session_id.as_str());
                }
            }
            {
                let mut download_sessions = runtime.download_sessions.lock().await;
                for session_id in &expired_ids {
                    download_sessions.remove(session_id.as_str());
                }
            }
            if let Some(store) = runtime.state_store.as_ref() {
                for session in removed_sessions {
                    if session.persistence.enabled {
                        if let Err(error) = persist_session_snapshot(store, &session) {
                            warn!(
                                principal = session.principal,
                                channel = ?session.channel,
                                error = %error,
                                "failed to persist state while expiring session"
                            );
                        }
                    }
                }
            }
        }
    });
}

pub(crate) async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(error = %error, "failed to register Ctrl+C handler");
        std::future::pending::<()>().await;
    }
}
