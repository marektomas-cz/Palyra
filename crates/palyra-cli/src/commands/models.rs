use crate::*;
use palyra_common::daemon_config_schema::FileModelProviderConfig;

const OPENAI_COMPATIBLE_PROVIDER_KIND: &str = "openai_compatible";
const ANTHROPIC_PROVIDER_KIND: &str = "anthropic";
const DETERMINISTIC_PROVIDER_KIND: &str = "deterministic";
const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const CURATED_TEXT_MODELS: &[&str] = &["gpt-4o-mini", "gpt-4.1-mini"];
const CURATED_EMBEDDING_MODELS: &[&str] = &["text-embedding-3-small", "text-embedding-3-large"];

#[derive(Debug, Serialize)]
pub(crate) struct ModelsStatusPayload {
    pub(crate) path: String,
    pub(crate) provider_kind: String,
    pub(crate) openai_base_url: Option<String>,
    pub(crate) text_model: Option<String>,
    pub(crate) embeddings_model: Option<String>,
    pub(crate) embeddings_dims: Option<u32>,
    pub(crate) auth_profile_id: Option<String>,
    pub(crate) api_key_configured: bool,
    pub(crate) default_chat_model_id: Option<String>,
    pub(crate) default_embeddings_model_id: Option<String>,
    pub(crate) failover_enabled: bool,
    pub(crate) response_cache_enabled: bool,
    pub(crate) registry_provider_count: usize,
    pub(crate) registry_model_count: usize,
    pub(crate) registry_valid: bool,
    pub(crate) validation_issues: Vec<String>,
    pub(crate) migrated: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelCatalogEntry<'a> {
    pub(crate) target: &'a str,
    pub(crate) id: String,
    pub(crate) configured: bool,
    pub(crate) preferred: bool,
    pub(crate) source: &'a str,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryProviderEntry {
    pub(crate) provider_id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) kind: String,
    pub(crate) base_url: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) auth_profile_id: Option<String>,
    pub(crate) auth_provider_kind: Option<String>,
    pub(crate) api_key_configured: bool,
    pub(crate) source: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistryModelEntry {
    pub(crate) model_id: String,
    pub(crate) provider_id: String,
    pub(crate) role: String,
    pub(crate) enabled: bool,
    pub(crate) metadata_source: String,
    pub(crate) operator_override: bool,
    pub(crate) tool_calls: bool,
    pub(crate) json_mode: bool,
    pub(crate) vision: bool,
    pub(crate) audio_transcribe: bool,
    pub(crate) embeddings: bool,
    pub(crate) max_context_tokens: Option<u32>,
    pub(crate) cost_tier: String,
    pub(crate) latency_tier: String,
    pub(crate) recommended_use_cases: Vec<String>,
    pub(crate) known_limitations: Vec<String>,
    pub(crate) source: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelsListPayload {
    pub(crate) status: ModelsStatusPayload,
    pub(crate) models: Vec<ModelCatalogEntry<'static>>,
    pub(crate) providers: Vec<RegistryProviderEntry>,
    pub(crate) registry_models: Vec<RegistryModelEntry>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelsMutationPayload {
    pub(crate) path: String,
    pub(crate) provider_kind: String,
    pub(crate) target: &'static str,
    pub(crate) model: String,
    pub(crate) embeddings_dims: Option<u32>,
    pub(crate) backups: usize,
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
                    "models.list provider_kind={} text_model={} embeddings_model={} auth_profile_id={} registry_providers={} registry_models={} registry_valid={}",
                    payload.status.provider_kind,
                    payload.status.text_model.as_deref().unwrap_or("none"),
                    payload.status.embeddings_model.as_deref().unwrap_or("none"),
                    payload.status.auth_profile_id.as_deref().unwrap_or("none"),
                    payload.providers.len(),
                    payload.registry_models.len(),
                    payload.status.registry_valid
                );
                for entry in payload.providers {
                    println!(
                        "models.provider id={} kind={} enabled={} auth_profile_id={} api_key_configured={} source={}",
                        entry.provider_id,
                        entry.kind,
                        entry.enabled,
                        entry.auth_profile_id.as_deref().unwrap_or("none"),
                        entry.api_key_configured,
                        entry.source
                    );
                }
                for entry in payload.registry_models {
                    println!(
                        "models.registry_model id={} provider_id={} role={} enabled={} json_mode={} vision={} embeddings={} source={}",
                        entry.model_id,
                        entry.provider_id,
                        entry.role,
                        entry.enabled,
                        entry.json_mode,
                        entry.vision,
                        entry.embeddings,
                        entry.source
                    );
                }
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
            "models.status.provider base_url={} embeddings_dims={} default_chat_model={} default_embeddings_model={} registry_providers={} registry_models={} failover_enabled={} response_cache_enabled={} registry_valid={}",
            payload.openai_base_url.as_deref().unwrap_or("none"),
            payload
                .embeddings_dims
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            payload.default_chat_model_id.as_deref().unwrap_or("none"),
            payload.default_embeddings_model_id.as_deref().unwrap_or("none"),
            payload.registry_provider_count,
            payload.registry_model_count,
            payload.failover_enabled,
            payload.response_cache_enabled,
            payload.registry_valid
        );
        for issue in &payload.validation_issues {
            println!("models.status.validation issue={issue}");
        }
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

pub(crate) fn build_models_list(path: Option<String>) -> Result<ModelsListPayload> {
    let overview = load_models_overview(path)?;
    let status = overview.status;
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
    Ok(ModelsListPayload {
        status,
        models,
        providers: overview.providers,
        registry_models: overview.models,
    })
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

pub(crate) fn mutate_model_defaults(
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
    let has_registry = registry_configured(&document)?;
    if !has_registry {
        set_value_at_path(
            &mut document,
            "model_provider.kind",
            toml::Value::String(OPENAI_COMPATIBLE_PROVIDER_KIND.to_owned()),
        )
        .context("invalid config key path: model_provider.kind")?;
        let existing_base_url =
            get_string_value_at_path(&document, "model_provider.openai_base_url")?;
        if existing_base_url.is_none() {
            set_value_at_path(
                &mut document,
                "model_provider.openai_base_url",
                toml::Value::String(OPENAI_DEFAULT_BASE_URL.to_owned()),
            )
            .context("invalid config key path: model_provider.openai_base_url")?;
        }
    }

    match target {
        "text" => {
            let key = if has_registry {
                "model_provider.default_chat_model_id"
            } else {
                "model_provider.openai_model"
            };
            set_value_at_path(&mut document, key, toml::Value::String(model.clone()))
                .with_context(|| format!("invalid config key path: {key}"))?;
        }
        "embeddings" => {
            let key = if has_registry {
                "model_provider.default_embeddings_model_id"
            } else {
                "model_provider.openai_embeddings_model"
            };
            set_value_at_path(&mut document, key, toml::Value::String(model.clone()))
                .with_context(|| format!("invalid config key path: {key}"))?;
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
        provider_kind: get_string_value_at_path(&document, "model_provider.kind")?
            .unwrap_or_else(|| OPENAI_COMPATIBLE_PROVIDER_KIND.to_owned()),
        target,
        model,
        embeddings_dims: dims,
        backups,
    })
}

pub(crate) fn load_models_status(path: Option<String>) -> Result<ModelsStatusPayload> {
    Ok(load_models_overview(path)?.status)
}

struct ModelsOverview {
    status: ModelsStatusPayload,
    providers: Vec<RegistryProviderEntry>,
    models: Vec<RegistryModelEntry>,
}

fn load_models_overview(path: Option<String>) -> Result<ModelsOverview> {
    let path = resolve_config_path(path, true)?;
    let (document, migration) = load_document_from_existing_path(Path::new(&path))
        .with_context(|| format!("failed to parse {path}"))?;
    let root_config = parse_root_file_config(&document)?;
    let model_provider = root_config.model_provider.unwrap_or_default();
    let provider_kind =
        model_provider.kind.clone().unwrap_or_else(|| DETERMINISTIC_PROVIDER_KIND.to_owned());
    let (providers, models) = registry_views_from_config(&model_provider);
    let validation_issues = validate_registry_views(
        providers.as_slice(),
        models.as_slice(),
        model_provider.default_chat_model_id.as_deref(),
        model_provider.default_embeddings_model_id.as_deref(),
    );
    let openai_base_url = model_provider.openai_base_url.clone();
    let text_model = model_provider
        .default_chat_model_id
        .clone()
        .or_else(|| model_provider.openai_model.clone())
        .or_else(|| model_provider.anthropic_model.clone());
    let embeddings_model = model_provider
        .default_embeddings_model_id
        .clone()
        .or_else(|| model_provider.openai_embeddings_model.clone());
    let embeddings_dims = model_provider.openai_embeddings_dims;
    let auth_profile_id = model_provider.auth_profile_id.clone().or_else(|| {
        providers
            .iter()
            .find(|entry| {
                Some(entry.provider_id.as_str())
                    == default_provider_id(models.as_slice(), &model_provider)
            })
            .and_then(|entry| entry.auth_profile_id.clone())
    });
    let api_key_configured =
        model_provider.openai_api_key.as_deref().filter(|value| !value.trim().is_empty()).is_some()
            || model_provider
                .openai_api_key_vault_ref
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .is_some()
            || model_provider
                .anthropic_api_key
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .is_some()
            || model_provider
                .anthropic_api_key_vault_ref
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .is_some()
            || providers.iter().any(|entry| entry.api_key_configured);
    Ok(ModelsOverview {
        status: ModelsStatusPayload {
            path,
            provider_kind,
            openai_base_url,
            text_model,
            embeddings_model,
            embeddings_dims,
            auth_profile_id,
            api_key_configured,
            default_chat_model_id: model_provider.default_chat_model_id,
            default_embeddings_model_id: model_provider.default_embeddings_model_id,
            failover_enabled: model_provider.failover_enabled.unwrap_or(true),
            response_cache_enabled: model_provider.response_cache_enabled.unwrap_or(true),
            registry_provider_count: providers.len(),
            registry_model_count: models.len(),
            registry_valid: validation_issues.is_empty(),
            validation_issues,
            migrated: migration.migrated,
        },
        providers,
        models,
    })
}

fn parse_root_file_config(document: &toml::Value) -> Result<RootFileConfig> {
    let serialized = toml::to_string(document)
        .context("failed to serialize config document for model parsing")?;
    toml::from_str(&serialized).context("failed to parse model provider config snapshot")
}

fn registry_configured(document: &toml::Value) -> Result<bool> {
    Ok(get_value_at_path(document, "model_provider.providers")
        .with_context(|| "invalid config key path: model_provider.providers")?
        .is_some()
        || get_value_at_path(document, "model_provider.models")
            .with_context(|| "invalid config key path: model_provider.models")?
            .is_some())
}

fn registry_views_from_config(
    config: &FileModelProviderConfig,
) -> (Vec<RegistryProviderEntry>, Vec<RegistryModelEntry>) {
    let providers = config
        .providers
        .as_ref()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| RegistryProviderEntry {
                    provider_id: entry.provider_id.clone().unwrap_or_default(),
                    display_name: entry.display_name.clone(),
                    kind: entry.kind.clone().unwrap_or_else(|| {
                        config
                            .kind
                            .clone()
                            .unwrap_or_else(|| DETERMINISTIC_PROVIDER_KIND.to_owned())
                    }),
                    base_url: entry.base_url.clone(),
                    enabled: entry.enabled.unwrap_or(true),
                    auth_profile_id: entry.auth_profile_id.clone(),
                    auth_provider_kind: entry.auth_provider_kind.clone(),
                    api_key_configured: entry
                        .api_key
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                        .is_some()
                        || entry
                            .api_key_vault_ref
                            .as_deref()
                            .filter(|value| !value.trim().is_empty())
                            .is_some(),
                    source: "registry",
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| legacy_provider_entries(config));
    let models = config
        .models
        .as_ref()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| RegistryModelEntry {
                    model_id: entry.model_id.clone().unwrap_or_default(),
                    provider_id: entry.provider_id.clone().unwrap_or_default(),
                    role: entry.role.clone().unwrap_or_else(|| "chat".to_owned()),
                    enabled: entry.enabled.unwrap_or(true),
                    metadata_source: entry
                        .metadata_source
                        .clone()
                        .unwrap_or_else(|| "static".to_owned()),
                    operator_override: entry.operator_override.unwrap_or(false),
                    tool_calls: entry.tool_calls.unwrap_or(false),
                    json_mode: entry.json_mode.unwrap_or(false),
                    vision: entry.vision.unwrap_or(false),
                    audio_transcribe: entry.audio_transcribe.unwrap_or(false),
                    embeddings: entry.embeddings.unwrap_or(false),
                    max_context_tokens: entry.max_context_tokens,
                    cost_tier: entry.cost_tier.clone().unwrap_or_else(|| "standard".to_owned()),
                    latency_tier: entry
                        .latency_tier
                        .clone()
                        .unwrap_or_else(|| "standard".to_owned()),
                    recommended_use_cases: entry.recommended_use_cases.clone().unwrap_or_default(),
                    known_limitations: entry.known_limitations.clone().unwrap_or_default(),
                    source: "registry",
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| legacy_model_entries(config));
    (providers, models)
}

fn legacy_provider_entries(config: &FileModelProviderConfig) -> Vec<RegistryProviderEntry> {
    let kind = config.kind.clone().unwrap_or_else(|| DETERMINISTIC_PROVIDER_KIND.to_owned());
    let provider_id = legacy_provider_id(kind.as_str()).to_owned();
    vec![RegistryProviderEntry {
        provider_id,
        display_name: Some(kind.replace('_', " ")),
        kind,
        base_url: config.openai_base_url.clone().or_else(|| config.anthropic_base_url.clone()),
        enabled: true,
        auth_profile_id: config.auth_profile_id.clone(),
        auth_provider_kind: config.auth_provider_kind.clone(),
        api_key_configured: config
            .openai_api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .is_some()
            || config
                .openai_api_key_vault_ref
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .is_some()
            || config
                .anthropic_api_key
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .is_some()
            || config
                .anthropic_api_key_vault_ref
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .is_some(),
        source: "legacy",
    }]
}

fn legacy_model_entries(config: &FileModelProviderConfig) -> Vec<RegistryModelEntry> {
    let kind = config.kind.clone().unwrap_or_else(|| DETERMINISTIC_PROVIDER_KIND.to_owned());
    let provider_id = legacy_provider_id(kind.as_str()).to_owned();
    let mut models = Vec::new();
    if let Some(model_id) = config
        .openai_model
        .clone()
        .or_else(|| config.anthropic_model.clone())
        .or_else(|| Some("deterministic".to_owned()))
    {
        models.push(legacy_registry_model(model_id, provider_id.clone(), "chat", kind.as_str()));
    }
    if let Some(model_id) = config.openai_embeddings_model.clone() {
        models.push(legacy_registry_model(
            model_id,
            provider_id,
            "embeddings",
            OPENAI_COMPATIBLE_PROVIDER_KIND,
        ));
    }
    models
}

fn legacy_registry_model(
    model_id: String,
    provider_id: String,
    role: &str,
    provider_kind: &str,
) -> RegistryModelEntry {
    let is_chat = role == "chat";
    RegistryModelEntry {
        model_id,
        provider_id,
        role: role.to_owned(),
        enabled: true,
        metadata_source: "legacy_migration".to_owned(),
        operator_override: false,
        tool_calls: is_chat,
        json_mode: is_chat && provider_kind != DETERMINISTIC_PROVIDER_KIND,
        vision: is_chat && provider_kind != DETERMINISTIC_PROVIDER_KIND,
        audio_transcribe: is_chat && provider_kind == OPENAI_COMPATIBLE_PROVIDER_KIND,
        embeddings: role == "embeddings",
        max_context_tokens: if provider_kind == DETERMINISTIC_PROVIDER_KIND {
            None
        } else {
            Some(128_000)
        },
        cost_tier: if provider_kind == ANTHROPIC_PROVIDER_KIND {
            "premium".to_owned()
        } else if provider_kind == OPENAI_COMPATIBLE_PROVIDER_KIND && role == "embeddings" {
            "low".to_owned()
        } else {
            "standard".to_owned()
        },
        latency_tier: if provider_kind == ANTHROPIC_PROVIDER_KIND {
            "high".to_owned()
        } else {
            "standard".to_owned()
        },
        recommended_use_cases: if role == "embeddings" {
            vec!["memory retrieval".to_owned()]
        } else {
            vec!["general chat".to_owned()]
        },
        known_limitations: Vec::new(),
        source: "legacy",
    }
}

fn legacy_provider_id(provider_kind: &str) -> &'static str {
    match provider_kind {
        OPENAI_COMPATIBLE_PROVIDER_KIND => "openai-primary",
        ANTHROPIC_PROVIDER_KIND => "anthropic-primary",
        _ => "deterministic-primary",
    }
}

fn validate_registry_views(
    providers: &[RegistryProviderEntry],
    models: &[RegistryModelEntry],
    default_chat_model_id: Option<&str>,
    default_embeddings_model_id: Option<&str>,
) -> Vec<String> {
    let mut issues = Vec::new();
    if providers.is_empty() {
        issues.push("provider registry does not define any providers".to_owned());
    }
    if models.is_empty() {
        issues.push("provider registry does not define any models".to_owned());
    }

    let mut provider_ids = std::collections::HashSet::new();
    for provider in providers {
        if provider.provider_id.trim().is_empty() {
            issues.push("provider registry entry is missing provider_id".to_owned());
        } else if !provider_ids.insert(provider.provider_id.clone()) {
            issues.push(format!("duplicate provider id '{}'", provider.provider_id));
        }
    }

    let mut model_ids = std::collections::HashSet::new();
    for model in models {
        if model.model_id.trim().is_empty() {
            issues.push("provider registry model is missing model_id".to_owned());
        } else if !model_ids.insert(model.model_id.clone()) {
            issues.push(format!("duplicate model id '{}'", model.model_id));
        }
        if !provider_ids.contains(model.provider_id.as_str()) {
            issues.push(format!(
                "model '{}' references unknown provider '{}'",
                model.model_id, model.provider_id
            ));
        }
    }

    if let Some(model_id) = default_chat_model_id {
        if !models.iter().any(|entry| entry.model_id == model_id && entry.role == "chat") {
            issues.push(format!(
                "default chat model '{}' was not found among configured chat models",
                model_id
            ));
        }
    }
    if let Some(model_id) = default_embeddings_model_id {
        if !models.iter().any(|entry| entry.model_id == model_id && entry.role == "embeddings") {
            issues.push(format!(
                "default embeddings model '{}' was not found among configured embeddings models",
                model_id
            ));
        }
    }

    issues
}

fn default_provider_id<'a>(
    models: &'a [RegistryModelEntry],
    config: &FileModelProviderConfig,
) -> Option<&'a str> {
    let default_chat_model_id = config
        .default_chat_model_id
        .as_deref()
        .or(config.openai_model.as_deref())
        .or(config.anthropic_model.as_deref());
    default_chat_model_id.and_then(|model_id| {
        models
            .iter()
            .find(|entry| entry.model_id == model_id && entry.role == "chat")
            .map(|entry| entry.provider_id.as_str())
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
