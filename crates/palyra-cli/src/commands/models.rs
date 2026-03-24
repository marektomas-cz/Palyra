use crate::*;

const OPENAI_COMPATIBLE_PROVIDER_KIND: &str = "openai_compatible";
const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const CURATED_TEXT_MODELS: &[&str] = &["gpt-4o-mini", "gpt-4.1-mini"];
const CURATED_EMBEDDING_MODELS: &[&str] = &["text-embedding-3-small", "text-embedding-3-large"];

#[derive(Debug, Serialize)]
struct ModelsStatusPayload {
    path: String,
    provider_kind: String,
    openai_base_url: Option<String>,
    text_model: Option<String>,
    embeddings_model: Option<String>,
    embeddings_dims: Option<u32>,
    auth_profile_id: Option<String>,
    api_key_configured: bool,
    migrated: bool,
}

#[derive(Debug, Serialize)]
struct ModelCatalogEntry<'a> {
    target: &'a str,
    id: String,
    configured: bool,
    preferred: bool,
    source: &'a str,
}

#[derive(Debug, Serialize)]
struct ModelsListPayload {
    status: ModelsStatusPayload,
    models: Vec<ModelCatalogEntry<'static>>,
}

#[derive(Debug, Serialize)]
struct ModelsMutationPayload {
    path: String,
    provider_kind: String,
    target: &'static str,
    model: String,
    embeddings_dims: Option<u32>,
    backups: usize,
}

pub(crate) fn run_models(command: ModelsCommand) -> Result<()> {
    match command {
        ModelsCommand::Status { path, json } => {
            let payload = load_models_status(path)?;
            emit_models_status(&payload, output::preferred_json(json))
        }
        ModelsCommand::List { path, json } => {
            let payload = build_models_list(path)?;
            if output::preferred_json(json) {
                output::print_json_pretty(&payload, "failed to encode models list as JSON")?;
            } else {
                println!(
                    "models.list provider_kind={} text_model={} embeddings_model={} auth_profile_id={}",
                    payload.status.provider_kind,
                    payload.status.text_model.as_deref().unwrap_or("none"),
                    payload.status.embeddings_model.as_deref().unwrap_or("none"),
                    payload.status.auth_profile_id.as_deref().unwrap_or("none")
                );
                for entry in payload.models {
                    println!(
                        "models.entry target={} id={} configured={} preferred={} source={}",
                        entry.target, entry.id, entry.configured, entry.preferred, entry.source
                    );
                }
            }
            std::io::stdout().flush().context("stdout flush failed")
        }
        ModelsCommand::Set { model, path, backups, json } => {
            let payload = mutate_model_defaults(path, backups, "text", model, None)?;
            emit_models_mutation(&payload, output::preferred_json(json))
        }
        ModelsCommand::SetEmbeddings { model, dims, path, backups, json } => {
            let payload = mutate_model_defaults(path, backups, "embeddings", model, dims)?;
            emit_models_mutation(&payload, output::preferred_json(json))
        }
    }
}

fn emit_models_status(payload: &ModelsStatusPayload, json_output: bool) -> Result<()> {
    if json_output {
        output::print_json_pretty(payload, "failed to encode models status as JSON")?;
    } else {
        println!(
            "models.status path={} provider_kind={} text_model={} embeddings_model={} auth_profile_id={} api_key_configured={} migrated={}",
            payload.path,
            payload.provider_kind,
            payload.text_model.as_deref().unwrap_or("none"),
            payload.embeddings_model.as_deref().unwrap_or("none"),
            payload.auth_profile_id.as_deref().unwrap_or("none"),
            payload.api_key_configured,
            payload.migrated
        );
        println!(
            "models.status.provider base_url={} embeddings_dims={}",
            payload.openai_base_url.as_deref().unwrap_or("none"),
            payload
                .embeddings_dims
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned())
        );
    }
    std::io::stdout().flush().context("stdout flush failed")
}

fn emit_models_mutation(payload: &ModelsMutationPayload, json_output: bool) -> Result<()> {
    if json_output {
        output::print_json_pretty(payload, "failed to encode models mutation as JSON")?;
    } else {
        println!(
            "models.set path={} provider_kind={} target={} model={} embeddings_dims={} backups={}",
            payload.path,
            payload.provider_kind,
            payload.target,
            payload.model,
            payload
                .embeddings_dims
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            payload.backups
        );
    }
    std::io::stdout().flush().context("stdout flush failed")
}

fn build_models_list(path: Option<String>) -> Result<ModelsListPayload> {
    let status = load_models_status(path)?;
    let mut models = Vec::new();
    append_catalog_entries(
        &mut models,
        "text",
        CURATED_TEXT_MODELS,
        status.text_model.as_deref(),
        Some("gpt-4o-mini"),
    );
    append_catalog_entries(
        &mut models,
        "embeddings",
        CURATED_EMBEDDING_MODELS,
        status.embeddings_model.as_deref(),
        Some("text-embedding-3-small"),
    );
    if let Some(configured) = status.text_model.as_deref() {
        append_ad_hoc_entry(&mut models, "text", configured);
    }
    if let Some(configured) = status.embeddings_model.as_deref() {
        append_ad_hoc_entry(&mut models, "embeddings", configured);
    }
    Ok(ModelsListPayload { status, models })
}

fn append_catalog_entries(
    target_entries: &mut Vec<ModelCatalogEntry<'static>>,
    target: &'static str,
    catalog: &[&str],
    configured: Option<&str>,
    preferred: Option<&str>,
) {
    for model in catalog {
        target_entries.push(ModelCatalogEntry {
            target,
            id: (*model).to_owned(),
            configured: configured.is_some_and(|value| value == *model),
            preferred: preferred.is_some_and(|value| value == *model),
            source: "curated",
        });
    }
}

fn append_ad_hoc_entry(
    target_entries: &mut Vec<ModelCatalogEntry<'static>>,
    target: &'static str,
    configured: &str,
) {
    if target_entries.iter().any(|entry| entry.target == target && entry.id == configured) {
        return;
    }
    target_entries.push(ModelCatalogEntry {
        target,
        id: configured.to_owned(),
        configured: true,
        preferred: false,
        source: "configured",
    });
}

fn mutate_model_defaults(
    path: Option<String>,
    backups: usize,
    target: &'static str,
    model: String,
    dims: Option<u32>,
) -> Result<ModelsMutationPayload> {
    let path = resolve_config_path(path, false)?;
    let path_ref = Path::new(&path);
    let (mut document, _) = load_document_for_mutation(path_ref)
        .with_context(|| format!("failed to parse {}", path_ref.display()))?;
    set_value_at_path(
        &mut document,
        "model_provider.kind",
        toml::Value::String(OPENAI_COMPATIBLE_PROVIDER_KIND.to_owned()),
    )
    .context("invalid config key path: model_provider.kind")?;
    let existing_base_url = get_string_value_at_path(&document, "model_provider.openai_base_url")?;
    if existing_base_url.is_none() {
        set_value_at_path(
            &mut document,
            "model_provider.openai_base_url",
            toml::Value::String(OPENAI_DEFAULT_BASE_URL.to_owned()),
        )
        .context("invalid config key path: model_provider.openai_base_url")?;
    }

    match target {
        "text" => {
            set_value_at_path(
                &mut document,
                "model_provider.openai_model",
                toml::Value::String(model.clone()),
            )
            .context("invalid config key path: model_provider.openai_model")?;
        }
        "embeddings" => {
            set_value_at_path(
                &mut document,
                "model_provider.openai_embeddings_model",
                toml::Value::String(model.clone()),
            )
            .context("invalid config key path: model_provider.openai_embeddings_model")?;
            if let Some(value) = dims {
                set_value_at_path(
                    &mut document,
                    "model_provider.openai_embeddings_dims",
                    toml::Value::Integer(i64::from(value)),
                )
                .context("invalid config key path: model_provider.openai_embeddings_dims")?;
            }
        }
        _ => anyhow::bail!("unsupported model target: {target}"),
    }

    validate_daemon_compatible_document(&document).with_context(|| {
        format!("mutated config {} does not match daemon schema", path_ref.display())
    })?;
    write_document_with_backups(path_ref, &document, backups)
        .with_context(|| format!("failed to persist config {}", path_ref.display()))?;
    Ok(ModelsMutationPayload {
        path,
        provider_kind: OPENAI_COMPATIBLE_PROVIDER_KIND.to_owned(),
        target,
        model,
        embeddings_dims: dims,
        backups,
    })
}

fn load_models_status(path: Option<String>) -> Result<ModelsStatusPayload> {
    let path = resolve_config_path(path, true)?;
    let (document, migration) = load_document_from_existing_path(Path::new(&path))
        .with_context(|| format!("failed to parse {path}"))?;
    let provider_kind = get_string_value_at_path(&document, "model_provider.kind")?
        .unwrap_or_else(|| "deterministic".to_owned());
    let openai_base_url = get_string_value_at_path(&document, "model_provider.openai_base_url")?;
    let text_model = get_string_value_at_path(&document, "model_provider.openai_model")?;
    let embeddings_model =
        get_string_value_at_path(&document, "model_provider.openai_embeddings_model")?;
    let embeddings_dims =
        get_integer_value_at_path(&document, "model_provider.openai_embeddings_dims")?
            .and_then(|value| u32::try_from(value).ok());
    let auth_profile_id = get_string_value_at_path(&document, "model_provider.auth_profile_id")?;
    let inline_api_key = get_string_value_at_path(&document, "model_provider.openai_api_key")?;
    let vault_api_key =
        get_string_value_at_path(&document, "model_provider.openai_api_key_vault_ref")?;
    Ok(ModelsStatusPayload {
        path,
        provider_kind,
        openai_base_url,
        text_model,
        embeddings_model,
        embeddings_dims,
        auth_profile_id,
        api_key_configured: inline_api_key.is_some() || vault_api_key.is_some(),
        migrated: migration.migrated,
    })
}

fn get_string_value_at_path(document: &toml::Value, key: &str) -> Result<Option<String>> {
    Ok(get_value_at_path(document, key)
        .with_context(|| format!("invalid config key path: {key}"))?
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned))
}

fn get_integer_value_at_path(document: &toml::Value, key: &str) -> Result<Option<i64>> {
    Ok(get_value_at_path(document, key)
        .with_context(|| format!("invalid config key path: {key}"))?
        .and_then(toml::Value::as_integer))
}
