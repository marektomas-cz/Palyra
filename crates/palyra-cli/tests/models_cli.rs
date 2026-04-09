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

#[test]
fn models_list_reports_registry_providers_and_models() -> Result<()> {
    let workdir = TempDir::new().context("failed to create temporary workdir")?;
    let config_path = workdir.path().join("palyra.toml");
    fs::write(
        &config_path,
        r#"
version = 1
[model_provider]
kind = "openai_compatible"
default_chat_model_id = "claude-3-5-sonnet-latest"
default_embeddings_model_id = "text-embedding-3-small"
failover_enabled = true
response_cache_enabled = true

[[model_provider.providers]]
provider_id = "openai-primary"
display_name = "OpenAI"
kind = "openai_compatible"
base_url = "https://api.openai.com/v1"
auth_profile_id = "openai-default"

[[model_provider.providers]]
provider_id = "anthropic-primary"
display_name = "Anthropic"
kind = "anthropic"
base_url = "https://api.anthropic.com"
auth_profile_id = "anthropic-default"

[[model_provider.models]]
model_id = "gpt-4o-mini"
provider_id = "openai-primary"
role = "chat"
enabled = true
metadata_source = "static"
tool_calls = true
json_mode = true
vision = true
cost_tier = "standard"
latency_tier = "standard"

[[model_provider.models]]
model_id = "claude-3-5-sonnet-latest"
provider_id = "anthropic-primary"
role = "chat"
enabled = true
metadata_source = "discovery"
tool_calls = true
json_mode = true
vision = true
cost_tier = "premium"
latency_tier = "high"

[[model_provider.models]]
model_id = "text-embedding-3-small"
provider_id = "openai-primary"
role = "embeddings"
enabled = true
metadata_source = "static"
embeddings = true
cost_tier = "low"
latency_tier = "standard"
"#,
    )
    .with_context(|| format!("failed to write {}", config_path.display()))?;
    let config_path_string = config_path.to_string_lossy().into_owned();

    let output = run_cli(&workdir, &["models", "list", "--path", &config_path_string, "--json"])?;
    assert!(
        output.status.success(),
        "models list should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).context("stdout was not valid UTF-8")?;
    assert!(
        stdout.contains("\"registry_provider_count\": 2"),
        "models list should report provider registry count: {stdout}"
    );
    assert!(
        stdout.contains("\"provider_id\": \"anthropic-primary\""),
        "models list should include anthropic provider entry: {stdout}"
    );
    assert!(
        stdout.contains("\"model_id\": \"claude-3-5-sonnet-latest\""),
        "models list should include registry model entries: {stdout}"
    );
    assert!(
        stdout.contains("\"registry_valid\": true"),
        "models list should report registry validation status: {stdout}"
    );
    Ok(())
}

#[test]
fn models_set_updates_registry_default_chat_model_when_registry_exists() -> Result<()> {
    let workdir = TempDir::new().context("failed to create temporary workdir")?;
    let config_path = workdir.path().join("palyra.toml");
    fs::write(
        &config_path,
        r#"
version = 1
[model_provider]
kind = "openai_compatible"
default_chat_model_id = "gpt-4o-mini"

[[model_provider.providers]]
provider_id = "openai-primary"
kind = "openai_compatible"

[[model_provider.providers]]
provider_id = "anthropic-primary"
kind = "anthropic"

[[model_provider.models]]
model_id = "gpt-4o-mini"
provider_id = "openai-primary"
role = "chat"
enabled = true

[[model_provider.models]]
model_id = "claude-3-5-sonnet-latest"
provider_id = "anthropic-primary"
role = "chat"
enabled = true
"#,
    )
    .with_context(|| format!("failed to write {}", config_path.display()))?;
    let config_path_string = config_path.to_string_lossy().into_owned();

    let output = run_cli(
        &workdir,
        &["models", "set", "claude-3-5-sonnet-latest", "--path", &config_path_string, "--json"],
    )?;
    assert!(
        output.status.success(),
        "models set should succeed for registry config: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config_body = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    assert!(
        config_body.contains("default_chat_model_id = \"claude-3-5-sonnet-latest\""),
        "models set should update the registry default chat model: {config_body}"
    );
    assert!(
        !config_body.contains("openai_base_url = \"https://api.openai.com/v1\""),
        "registry-aware models set should not inject legacy base_url defaults into registry configs: {config_body}"
    );
    Ok(())
}
