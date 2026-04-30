use std::io::Write;
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result};
use serde_json::Value;
use tempfile::TempDir;

fn run_cli_with_stdin(workdir: &TempDir, args: &[&str], stdin_payload: &[u8]) -> Result<Output> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_palyra"));
    command
        .current_dir(workdir.path())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("XDG_CONFIG_HOME", workdir.path().join("xdg-config"))
        .env("XDG_STATE_HOME", workdir.path().join("xdg-state"))
        .env("HOME", workdir.path().join("home"))
        .env("LOCALAPPDATA", workdir.path().join("localappdata"))
        .env("APPDATA", workdir.path().join("appdata"));
    let mut child =
        command.spawn().with_context(|| format!("failed to spawn palyra {}", args.join(" ")))?;
    let stdin = child.stdin.as_mut().context("palyra command stdin was not available")?;
    stdin.write_all(stdin_payload).context("failed to write stdin payload")?;
    child
        .wait_with_output()
        .with_context(|| format!("failed to wait for palyra {}", args.join(" ")))
}

#[test]
fn patch_apply_json_parse_error_emits_single_validation_payload() -> Result<()> {
    let workdir = TempDir::new().context("failed to create temporary workdir")?;
    let output = run_cli_with_stdin(
        &workdir,
        &["patch", "apply", "--stdin", "--dry-run", "--json"],
        b"not a patch",
    )?;

    assert_eq!(
        output.status.code(),
        Some(2),
        "malformed patches should fail with validation exit code; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "JSON patch failures should not emit an additional text error: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value =
        serde_json::from_slice(&output.stdout).context("patch failure stdout was not JSON")?;
    assert_eq!(payload.get("success").and_then(Value::as_bool), Some(false));
    assert_eq!(payload.get("error_kind").and_then(Value::as_str), Some("validation_error"));
    assert_eq!(payload.get("exit_code").and_then(Value::as_u64), Some(2));
    assert_eq!(payload.pointer("/parse_error/line").and_then(Value::as_u64), Some(1));
    assert_eq!(payload.pointer("/parse_error/column").and_then(Value::as_u64), Some(1));
    assert!(
        payload
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|value| { value.contains("patch parse error") }),
        "payload should include the parser failure: {payload}"
    );
    Ok(())
}
