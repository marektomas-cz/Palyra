use std::fs;
use std::process::{Command, Output};

use anyhow::{Context, Result};
use serde_json::Value;
use tempfile::TempDir;

fn configure_cli_env(command: &mut Command, workdir: &TempDir) {
    command
        .env("XDG_CONFIG_HOME", workdir.path().join("xdg-config"))
        .env("HOME", workdir.path().join("home"))
        .env("LOCALAPPDATA", workdir.path().join("localappdata"))
        .env("APPDATA", workdir.path().join("appdata"))
        .env("PROGRAMDATA", workdir.path().join("programdata"));
}

fn run_cli(workdir: &TempDir, args: &[&str]) -> Result<Output> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_palyra"));
    command.current_dir(workdir.path()).args(args);
    configure_cli_env(&mut command, workdir);
    command.output().with_context(|| format!("failed to execute palyra {}", args.join(" ")))
}

#[test]
fn doctor_json_uses_global_config_path() -> Result<()> {
    let workdir = TempDir::new().context("failed to create temporary workdir")?;
    let config_path = workdir.path().join("custom-palyra.toml");
    fs::write(
        config_path.as_path(),
        r#"
version = 1

[admin]
require_auth = false
"#,
    )
    .with_context(|| format!("failed to write {}", config_path.display()))?;

    let config_path_string = config_path.to_string_lossy().into_owned();
    let output = run_cli(&workdir, &["--config", &config_path_string, "doctor", "--json"])?;

    assert!(
        output.status.success(),
        "doctor --json should succeed without --strict: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).context("stdout was not UTF-8")?;
    let payload: Value = serde_json::from_str(stdout.as_str()).context("stdout was not JSON")?;

    assert_eq!(
        payload.pointer("/diagnostics/config/path").and_then(Value::as_str),
        Some(config_path_string.as_str())
    );
    assert_eq!(payload.pointer("/diagnostics/config/exists").and_then(Value::as_bool), Some(true));
    assert_eq!(payload.pointer("/diagnostics/config/parsed").and_then(Value::as_bool), Some(true));
    Ok(())
}
