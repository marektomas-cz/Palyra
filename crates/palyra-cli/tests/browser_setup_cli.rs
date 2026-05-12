use std::fs;
use std::process::{Command, Output};

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde_json::Value;
use tempfile::TempDir;

fn configure_cli_env(command: &mut Command, workdir: &TempDir) {
    command
        .env("PALYRA_VAULT_DIR", workdir.path().join("vault"))
        .env("PALYRA_VAULT_BACKEND", "encrypted_file")
        .env("XDG_STATE_HOME", workdir.path().join("xdg-state"))
        .env("HOME", workdir.path().join("home"))
        .env("LOCALAPPDATA", workdir.path().join("localappdata"))
        .env("APPDATA", workdir.path().join("appdata"));
}

fn run_cli(workdir: &TempDir, args: &[&str]) -> Result<Output> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_palyra"));
    command.current_dir(workdir.path()).args(args);
    configure_cli_env(&mut command, workdir);
    command.output().with_context(|| format!("failed to execute palyra {}", args.join(" ")))
}

#[test]
fn browser_setup_configures_gateway_and_browserd_prerequisites() -> Result<()> {
    let workdir = TempDir::new().context("failed to create temporary workdir")?;
    let config_path = workdir.path().join("palyra.toml");
    let config_arg = config_path.to_string_lossy().into_owned();

    let setup_config =
        run_cli(&workdir, &["setup", "--mode", "local", "--path", config_arg.as_str(), "--force"])?;
    assert!(
        setup_config.status.success(),
        "setup should succeed before browser setup: {}",
        String::from_utf8_lossy(&setup_config.stderr)
    );

    let browser_setup = run_cli(
        &workdir,
        &[
            "browser",
            "setup",
            "--path",
            config_arg.as_str(),
            "--token",
            "browser-token-for-test",
            "--json",
        ],
    )?;
    assert!(
        browser_setup.status.success(),
        "browser setup should succeed: {}",
        String::from_utf8_lossy(&browser_setup.stderr)
    );
    let payload: Value = serde_json::from_slice(&browser_setup.stdout)
        .context("browser setup stdout was not JSON")?;
    assert_eq!(payload.get("browser_service_enabled").and_then(Value::as_bool), Some(true));
    assert_eq!(payload.get("auth_token_configured").and_then(Value::as_bool), Some(true));
    assert_eq!(
        payload.get("state_key_vault_ref").and_then(Value::as_str),
        Some("global/browser_state_key")
    );
    assert_eq!(payload.get("gateway_reload_required").and_then(Value::as_bool), Some(true));
    assert_eq!(
        payload.get("gateway_restart_command").and_then(Value::as_str),
        Some("palyra gateway run")
    );
    assert_eq!(
        payload.get("gateway_verify_command").and_then(Value::as_str),
        Some("palyra browser status --json")
    );
    assert!(
        payload.get("gateway_next_step").and_then(Value::as_str).is_some_and(|value| {
            value.contains("palyra gateway run")
                && value.contains("palyra browser status --json")
                && value.contains("palyra browser open")
        }),
        "browser setup should explain gateway reload workflow: {payload}"
    );

    let config_toml = fs::read_to_string(&config_path).context("failed to read config")?;
    assert!(
        config_toml.contains("enabled = true")
            && config_toml.contains("auth_token = \"browser-token-for-test\"")
            && config_toml.contains("state_key_vault_ref = \"global/browser_state_key\"")
            && config_toml.contains("\"palyra.browser.navigate\"")
            && config_toml.contains("\"palyra.browser.screenshot\""),
        "browser setup should write gateway and browserd prerequisites: {config_toml}"
    );

    let state_key =
        run_cli(&workdir, &["secrets", "get", "global", "browser_state_key", "--reveal"])?;
    assert!(
        state_key.status.success(),
        "browser setup should store the generated state key: {}",
        String::from_utf8_lossy(&state_key.stderr)
    );
    let state_key_stdout =
        String::from_utf8(state_key.stdout).context("state key stdout was not UTF-8")?;
    let encoded = state_key_stdout.trim();
    let decoded =
        BASE64_STANDARD.decode(encoded).context("generated browser state key should be base64")?;
    assert_eq!(decoded.len(), 32, "generated browser state key should be 32 bytes");
    Ok(())
}

#[test]
fn browser_stop_supports_json_without_lifecycle_metadata() -> Result<()> {
    let workdir = TempDir::new().context("failed to create temporary workdir")?;

    let stop = run_cli(&workdir, &["browser", "stop", "--json"])?;

    assert!(
        stop.status.success(),
        "browser stop --json should succeed without metadata: {}",
        String::from_utf8_lossy(&stop.stderr)
    );
    let payload: Value =
        serde_json::from_slice(&stop.stdout).context("browser stop stdout was not JSON")?;
    assert_eq!(payload.get("action").and_then(Value::as_str), Some("stop"));
    assert_eq!(payload.get("running").and_then(Value::as_bool), Some(false));
    assert_eq!(
        payload.get("detail").and_then(Value::as_str),
        Some("no CLI-managed browser service metadata found")
    );
    Ok(())
}
