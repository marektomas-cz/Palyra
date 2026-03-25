use crate::*;

pub(crate) fn run_doctor(strict: bool, json: bool) -> Result<()> {
    let checks = build_doctor_checks();
    let report = build_doctor_report(checks.as_slice())?;
    let blocking_checks = checks.iter().filter(|check| check.required && !check.ok).collect::<Vec<_>>();
    let warning_checks = checks.iter().filter(|check| !check.required && !check.ok).collect::<Vec<_>>();

    if json {
        let encoded = serde_json::to_string_pretty(&report)
            .context("failed to serialize doctor JSON report")?;
        println!("{encoded}");
    } else {
        for check in &checks {
            println!("doctor.{}={} required={}", check.key, check.ok, check.required);
        }
        println!(
            "doctor.config path={} exists={} parsed={}",
            report.config.path.as_deref().unwrap_or("none"),
            report.config.exists,
            report.config.parsed
        );
        println!(
            "doctor.identity root={} exists={} writable={}",
            report.identity.store_root.as_deref().unwrap_or("unavailable"),
            report.identity.exists,
            report.identity.writable
        );
        println!(
            "doctor.connectivity daemon_url={} http_ok={} grpc_ok={} admin_ok={}",
            report.connectivity.daemon_url,
            report.connectivity.http.ok,
            report.connectivity.grpc.ok,
            report.provider_auth.fetched
        );
        println!(
            "doctor.sandbox tier_b_preflight_only={} tier_c_strict_offline={} tier_c_windows_backend_supported={}",
            report.sandbox.tier_b_egress_allowlists_preflight_only,
            report.sandbox.tier_c_strict_offline_only,
            report.sandbox.tier_c_windows_backend_supported
        );
        println!(
            "doctor.deployment mode={} bind_profile={} remote_bind_detected={} gateway_tls_enabled={} admin_auth_required={} admin_token_configured={}",
            report.deployment.mode,
            report.deployment.bind_profile,
            report.deployment.remote_bind_detected,
            report.deployment.gateway_tls_enabled,
            report.deployment.admin_auth_required,
            report.deployment.admin_token_configured,
        );
        println!(
            "doctor.summary blocking={} warnings={} required_checks_failed={}",
            blocking_checks.len(),
            warning_checks.len(),
            report.summary.required_checks_failed
        );
        for check in blocking_checks.as_slice() {
            println!("doctor.finding severity=blocking key={}", check.key);
        }
        for check in warning_checks.as_slice() {
            println!("doctor.finding severity=warning key={}", check.key);
        }
        for warning in report.deployment.warnings.as_slice() {
            println!("doctor.warning={warning}");
        }
        if checks.iter().any(|check| check.key == "memory_embeddings_model_configured" && !check.ok)
        {
            println!(
                "doctor.hint.memory_embeddings_model=configure model_provider.openai_embeddings_model (or PALYRA_MODEL_PROVIDER_OPENAI_EMBEDDINGS_MODEL) for openai-compatible semantic memory embeddings"
            );
        }
        if !report.connectivity.http.ok || !report.connectivity.grpc.ok || !blocking_checks.is_empty() {
            println!("doctor.next_step=palyra health");
            println!("doctor.next_step=palyra logs --lines 50");
        }
        if !report.deployment.warnings.is_empty() {
            println!("doctor.next_step=palyra security audit --offline");
        }
        if !blocking_checks.is_empty() || !warning_checks.is_empty() || !report.deployment.warnings.is_empty()
        {
            println!("doctor.next_step=palyra support-bundle export --output ./support-bundle.json");
        }
    }

    if strict {
        let failing_required = checks.iter().find(|check| check.required && !check.ok);
        if let Some(check) = failing_required {
            anyhow::bail!("strict doctor failed: {}", check.key);
        }
    }

    std::io::stdout().flush().context("stdout flush failed")
}
