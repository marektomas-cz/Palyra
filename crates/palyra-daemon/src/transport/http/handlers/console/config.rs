use crate::*;

pub(crate) async fn console_config_inspect_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<control_plane::ConfigInspectRequest>,
) -> Result<Json<control_plane::ConfigDocumentSnapshot>, Response> {
    let _session = authorize_console_session(&state, &headers, true)?;
    let (mut document, migration, source_path) =
        load_console_config_snapshot(payload.path.as_deref(), true)?;
    if !payload.show_secrets {
        redact_secret_config_values(&mut document);
    }
    let rendered = serialize_document_pretty(&document).map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to serialize config document: {error}"
        )))
    })?;
    Ok(Json(control_plane::ConfigDocumentSnapshot {
        contract: contract_descriptor(),
        source_path: source_path.clone(),
        config_version: migration.target_version,
        migrated_from_version: migration.migrated.then_some(migration.source_version),
        redacted: !payload.show_secrets,
        document_toml: rendered,
        backups: config_backup_records(Some(source_path.as_str()), payload.backups.max(1), false)?,
    }))
}

pub(crate) async fn console_config_validate_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<control_plane::ConfigValidateRequest>,
) -> Result<Json<control_plane::ConfigValidationEnvelope>, Response> {
    let _session = authorize_console_session(&state, &headers, true)?;
    let (document, migration, source_path) =
        load_console_config_snapshot(payload.path.as_deref(), false)?;
    validate_daemon_compatible_document(&document)?;
    Ok(Json(control_plane::ConfigValidationEnvelope {
        contract: contract_descriptor(),
        source_path,
        valid: true,
        config_version: migration.target_version,
        migrated_from_version: migration.migrated.then_some(migration.source_version),
    }))
}

pub(crate) async fn console_config_mutate_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<control_plane::ConfigMutationRequest>,
) -> Result<Json<control_plane::ConfigMutationEnvelope>, Response> {
    let _session = authorize_console_session(&state, &headers, true)?;
    let path = resolve_console_config_path(payload.path.as_deref(), false)?.ok_or_else(|| {
        runtime_status_response(tonic::Status::failed_precondition(
            "config path could not be resolved",
        ))
    })?;
    let path_ref = FsPath::new(path.as_str());
    let (mut document, migration) = load_console_document_for_mutation(path_ref)?;
    let operation = if let Some(value) = payload.value.as_deref() {
        let literal = parse_toml_value_literal(value).map_err(|error| {
            runtime_status_response(tonic::Status::invalid_argument(format!(
                "config value must be a valid TOML literal: {error}"
            )))
        })?;
        set_value_at_path(&mut document, payload.key.as_str(), literal).map_err(|error| {
            runtime_status_response(tonic::Status::invalid_argument(format!(
                "invalid config key path: {error}"
            )))
        })?;
        "set"
    } else {
        let removed =
            unset_value_at_path(&mut document, payload.key.as_str()).map_err(|error| {
                runtime_status_response(tonic::Status::invalid_argument(format!(
                    "invalid config key path: {error}"
                )))
            })?;
        if !removed {
            return Err(runtime_status_response(tonic::Status::not_found(format!(
                "config key not found: {}",
                payload.key
            ))));
        }
        "unset"
    };
    validate_daemon_compatible_document(&document)?;
    write_document_with_backups(path_ref, &document, payload.backups).map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to persist config {}: {error}",
            path_ref.display()
        )))
    })?;
    Ok(Json(control_plane::ConfigMutationEnvelope {
        contract: contract_descriptor(),
        operation: operation.to_owned(),
        source_path: path,
        backups_retained: payload.backups,
        config_version: migration.target_version,
        migrated_from_version: migration.migrated.then_some(migration.source_version),
        changed_key: Some(payload.key),
    }))
}

pub(crate) async fn console_config_migrate_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<control_plane::ConfigInspectRequest>,
) -> Result<Json<control_plane::ConfigMutationEnvelope>, Response> {
    let _session = authorize_console_session(&state, &headers, true)?;
    let path = resolve_console_config_path(payload.path.as_deref(), true)?.ok_or_else(|| {
        runtime_status_response(tonic::Status::failed_precondition(
            "no daemon config file found to migrate",
        ))
    })?;
    let path_ref = FsPath::new(path.as_str());
    let (document, migration) = load_console_document_from_existing_path(path_ref)?;
    validate_daemon_compatible_document(&document)?;
    if migration.migrated {
        write_document_with_backups(path_ref, &document, payload.backups).map_err(|error| {
            runtime_status_response(tonic::Status::internal(format!(
                "failed to persist migrated config {}: {error}",
                path_ref.display()
            )))
        })?;
    }
    Ok(Json(control_plane::ConfigMutationEnvelope {
        contract: contract_descriptor(),
        operation: "migrate".to_owned(),
        source_path: path,
        backups_retained: payload.backups,
        config_version: migration.target_version,
        migrated_from_version: migration.migrated.then_some(migration.source_version),
        changed_key: None,
    }))
}

pub(crate) async fn console_config_recover_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<control_plane::ConfigRecoverRequest>,
) -> Result<Json<control_plane::ConfigMutationEnvelope>, Response> {
    let _session = authorize_console_session(&state, &headers, true)?;
    let path = resolve_console_config_path(payload.path.as_deref(), false)?.ok_or_else(|| {
        runtime_status_response(tonic::Status::failed_precondition(
            "config path could not be resolved",
        ))
    })?;
    let path_ref = FsPath::new(path.as_str());
    let candidate_backup = backup_path(path_ref, payload.backup);
    let (backup_document, _) =
        load_console_document_from_existing_path(candidate_backup.as_path())?;
    validate_daemon_compatible_document(&backup_document)?;
    recover_config_from_backup(path_ref, payload.backup, payload.backups).map_err(|error| {
        runtime_status_response(tonic::Status::internal(format!(
            "failed to recover config {} from backup {}: {error}",
            path_ref.display(),
            payload.backup
        )))
    })?;
    let (document, migration) = load_console_document_from_existing_path(path_ref)?;
    validate_daemon_compatible_document(&document)?;
    Ok(Json(control_plane::ConfigMutationEnvelope {
        contract: contract_descriptor(),
        operation: "recover".to_owned(),
        source_path: path,
        backups_retained: payload.backups,
        config_version: migration.target_version,
        migrated_from_version: migration.migrated.then_some(migration.source_version),
        changed_key: None,
    }))
}
