use std::{
    fs,
    process::{Command, Output},
};

use anyhow::{Context, Result};
use tempfile::TempDir;

fn run_cli(workdir: &TempDir, args: &[&str]) -> Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_palyra"))
        .current_dir(workdir.path())
        .args(args)
        .output()
        .with_context(|| format!("failed to execute palyra {}", args.join(" ")))
}

#[test]
fn models_set_updates_text_and_embeddings_defaults() -> Result<()> {
    let workdir = TempDir::new().context("failed to create temporary workdir")?;
    let config_path = workdir.path().join("palyra.toml");
    fs::write(&config_path, "version = 1\n")
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    let config_path_string = config_path.to_string_lossy().into_owned();

    let text_output = run_cli(
        &workdir,
        &["models", "set", "gpt-4.1-mini", "--path", &config_path_string, "--json"],
    )?;
    assert!(
        text_output.status.success(),
        "models set should succeed: {}",
        String::from_utf8_lossy(&text_output.stderr)
    );

    let embeddings_output = run_cli(
        &workdir,
        &[
            "models",
            "set-embeddings",
            "text-embedding-3-large",
            "--path",
            &config_path_string,
            "--dims",
            "3072",
            "--json",
        ],
    )?;
    assert!(
        embeddings_output.status.success(),
        "models set-embeddings should succeed: {}",
        String::from_utf8_lossy(&embeddings_output.stderr)
    );

    let status_output =
        run_cli(&workdir, &["models", "status", "--path", &config_path_string, "--json"])?;
    assert!(
        status_output.status.success(),
        "models status should succeed: {}",
        String::from_utf8_lossy(&status_output.stderr)
    );
    let status_stdout =
        String::from_utf8(status_output.stdout).context("stdout was not valid UTF-8")?;
    assert!(
        status_stdout.contains("\"provider_kind\": \"openai_compatible\""),
        "models status should report openai_compatible provider kind: {status_stdout}"
    );
    assert!(
        status_stdout.contains("\"text_model\": \"gpt-4.1-mini\""),
        "models status should report the configured text model: {status_stdout}"
    );
    assert!(
        status_stdout.contains("\"embeddings_model\": \"text-embedding-3-large\""),
        "models status should report the configured embeddings model: {status_stdout}"
    );
    assert!(
        status_stdout.contains("\"embeddings_dims\": 3072"),
        "models status should report embeddings dims: {status_stdout}"
    );

    let config_body = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    assert!(
        config_body.contains("kind = \"openai_compatible\""),
        "models set should persist provider kind: {config_body}"
    );
    assert!(
        config_body.contains("openai_base_url = \"https://api.openai.com/v1\""),
        "models set should persist the default OpenAI base URL: {config_body}"
    );
    assert!(
        config_body.contains("openai_model = \"gpt-4.1-mini\""),
        "models set should persist the text model: {config_body}"
    );
    assert!(
        config_body.contains("openai_embeddings_model = \"text-embedding-3-large\""),
        "models set-embeddings should persist the embeddings model: {config_body}"
    );
    assert!(
        config_body.contains("openai_embeddings_dims = 3072"),
        "models set-embeddings should persist embeddings dims: {config_body}"
    );
    Ok(())
}

#[test]
fn bare_config_command_falls_back_to_status_using_global_config_path() -> Result<()> {
    let workdir = TempDir::new().context("failed to create temporary workdir")?;
    let config_path = workdir.path().join("palyra.toml");
    fs::write(
        &config_path,
        "version = 1\n[model_provider]\nkind = \"openai_compatible\"\nopenai_model = \"gpt-4o-mini\"\n",
    )
    .with_context(|| format!("failed to write {}", config_path.display()))?;
    let config_path_string = config_path.to_string_lossy().into_owned();

    let output =
        run_cli(&workdir, &["--config", &config_path_string, "--output-format", "json", "config"])?;
    assert!(
        output.status.success(),
        "bare config command should fall back to status: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).context("stdout was not valid UTF-8")?;
    assert!(
        stdout.contains("\"path\":"),
        "config status output should include the resolved path: {stdout}"
    );
    assert!(
        stdout.contains("\"parsed\": true"),
        "config status should confirm the config parsed successfully: {stdout}"
    );
    assert!(
        stdout.contains("\"provider_kind\": \"openai_compatible\""),
        "config status should surface the effective provider kind: {stdout}"
    );
    Ok(())
}
