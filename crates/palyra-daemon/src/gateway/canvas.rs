use super::*;
use reqwest::Url;

pub(crate) const MAX_CANVAS_SQLITE_VERSION: u64 = i64::MAX as u64;
pub(crate) const CANVAS_EXPERIMENT_STRUCTURED_CONTRACT: &str = "a2ui.v1";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct CanvasExperimentLimitsSnapshot {
    pub(crate) max_state_bytes: usize,
    pub(crate) max_bundle_bytes: usize,
    pub(crate) max_assets_per_bundle: usize,
    pub(crate) max_updates_per_minute: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct CanvasExperimentTrackSnapshot {
    pub(crate) track_id: String,
    pub(crate) enabled: bool,
    pub(crate) feature_flag: String,
    pub(crate) rollout_stage: String,
    pub(crate) ambient_mode: String,
    pub(crate) consent_required: bool,
    pub(crate) support_summary: String,
    pub(crate) security_review: Vec<String>,
    pub(crate) exit_criteria: Vec<String>,
    pub(crate) limits: CanvasExperimentLimitsSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct CanvasExperimentGovernanceSnapshot {
    pub(crate) structured_contract: String,
    pub(crate) fail_closed: bool,
    pub(crate) requires_console_diagnostics: bool,
    pub(crate) native_canvas: CanvasExperimentTrackSnapshot,
}

pub(crate) fn ensure_canvas_version_fits_sqlite(field: &str, value: u64) -> Result<(), Status> {
    if value > MAX_CANVAS_SQLITE_VERSION {
        return Err(Status::invalid_argument(format!(
            "{field} exceeds maximum supported value {MAX_CANVAS_SQLITE_VERSION}"
        )));
    }
    Ok(())
}

#[must_use]
pub(crate) fn build_canvas_experiment_governance_snapshot(
    config: &CanvasHostRuntimeConfig,
) -> CanvasExperimentGovernanceSnapshot {
    CanvasExperimentGovernanceSnapshot {
        structured_contract: CANVAS_EXPERIMENT_STRUCTURED_CONTRACT.to_owned(),
        fail_closed: true,
        requires_console_diagnostics: true,
        native_canvas: CanvasExperimentTrackSnapshot {
            track_id: "native-canvas-preview".to_owned(),
            enabled: config.enabled,
            feature_flag: "canvas_host.enabled".to_owned(),
            rollout_stage: if config.enabled {
                "operator_preview".to_owned()
            } else {
                "disabled".to_owned()
            },
            ambient_mode: "disabled".to_owned(),
            consent_required: false,
            support_summary: "Native canvas stays behind the bounded canvas host and keeps A2UI as the only structured render contract.".to_owned(),
            security_review: vec![
                "Preserve CSP, frame-ancestor allowlists, and token-scoped access.".to_owned(),
                "Keep state, bundle, and update budgets fail-closed in diagnostics and support flows."
                    .to_owned(),
            ],
            exit_criteria: vec![
                "Disable immediately if diagnostics, support bundle export, or replay fidelity regress."
                    .to_owned(),
                "Retire the experiment if it cannot justify operator value beyond the browser surface."
                    .to_owned(),
            ],
            limits: CanvasExperimentLimitsSnapshot {
                max_state_bytes: config.max_state_bytes,
                max_bundle_bytes: config.max_bundle_bytes,
                max_assets_per_bundle: config.max_assets_per_bundle,
                max_updates_per_minute: config.max_updates_per_minute,
            },
        },
    }
}

fn canvas_bundle_message(bundle: &CanvasBundleRecord) -> gateway_v1::CanvasBundle {
    let mut assets = bundle.assets.iter().collect::<Vec<_>>();
    assets.sort_by(|left, right| left.0.cmp(right.0));
    gateway_v1::CanvasBundle {
        bundle_id: bundle.bundle_id.clone(),
        entrypoint_path: bundle.entrypoint_path.clone(),
        assets: assets
            .into_iter()
            .map(|(path, asset)| gateway_v1::CanvasAsset {
                path: path.clone(),
                content_type: asset.content_type.clone(),
                body: asset.body.clone(),
            })
            .collect(),
        sha256: bundle.sha256.clone(),
        signature: bundle.signature.clone(),
    }
}

pub(crate) fn canvas_message(record: &CanvasRecord) -> gateway_v1::Canvas {
    gateway_v1::Canvas {
        v: CANONICAL_PROTOCOL_MAJOR,
        canvas_id: Some(common_v1::CanonicalId { ulid: record.canvas_id.clone() }),
        session_id: Some(common_v1::CanonicalId { ulid: record.session_id.clone() }),
        principal: record.principal.clone(),
        state_version: record.state_version,
        state_json: record.state_json.clone(),
        bundle: Some(canvas_bundle_message(&record.bundle)),
        allowed_parent_origins: record.allowed_parent_origins.clone(),
        created_at_unix_ms: record.created_at_unix_ms,
        updated_at_unix_ms: record.updated_at_unix_ms,
        expires_at_unix_ms: record.expires_at_unix_ms,
        closed: record.closed,
        close_reason: record.close_reason.clone().unwrap_or_default(),
        state_schema_version: record.state_schema_version,
    }
}

pub(crate) fn canvas_patch_update_message(
    patch: &CanvasStatePatchRecord,
    include_snapshot_state: bool,
) -> gateway_v1::SubscribeCanvasUpdatesResponse {
    gateway_v1::SubscribeCanvasUpdatesResponse {
        v: CANONICAL_PROTOCOL_MAJOR,
        canvas_id: Some(common_v1::CanonicalId { ulid: patch.canvas_id.clone() }),
        state_version: patch.state_version,
        base_state_version: patch.base_state_version,
        state_schema_version: patch.state_schema_version,
        patch_json: patch.patch_json.as_bytes().to_vec(),
        state_json: if include_snapshot_state {
            patch.resulting_state_json.as_bytes().to_vec()
        } else {
            Vec::new()
        },
        closed: patch.closed,
        close_reason: patch.close_reason.clone().unwrap_or_default(),
        applied_at_unix_ms: patch.applied_at_unix_ms,
    }
}

pub(crate) fn resolve_canvas_state_schema_version(
    requested_state_schema_version: Option<u64>,
    state: &Value,
    fallback_state_schema_version: Option<u64>,
) -> Result<u64, Status> {
    if let Some(value) = requested_state_schema_version {
        if value == 0 {
            return Err(Status::invalid_argument("state_schema_version must be greater than 0"));
        }
    }
    let embedded_state_schema_version =
        state.as_object().and_then(|value| value.get("schema_version")).and_then(Value::as_u64);
    if let Some(value) = embedded_state_schema_version {
        if value == 0 {
            return Err(Status::invalid_argument("embedded schema_version must be greater than 0"));
        }
    }
    if let (Some(requested), Some(embedded)) =
        (requested_state_schema_version, embedded_state_schema_version)
    {
        if requested != embedded {
            return Err(Status::invalid_argument(format!(
                "state_schema_version mismatch between request ({requested}) and state payload ({embedded})"
            )));
        }
    }
    let resolved = requested_state_schema_version
        .or(embedded_state_schema_version)
        .or(fallback_state_schema_version)
        .unwrap_or(1);
    ensure_canvas_version_fits_sqlite("state_schema_version", resolved)?;
    Ok(resolved)
}

pub(crate) fn load_canvas_records_from_snapshots(
    snapshots: &[CanvasStateSnapshotRecord],
) -> Result<HashMap<String, CanvasRecord>, JournalError> {
    let mut records = HashMap::with_capacity(snapshots.len());
    for snapshot in snapshots {
        serde_json::from_str::<Value>(snapshot.state_json.as_str()).map_err(|error| {
            JournalError::InvalidCanvasReplay {
                canvas_id: snapshot.canvas_id.clone(),
                reason: format!("snapshot state_json is invalid: {error}"),
            }
        })?;
        let bundle: CanvasBundleRecord = serde_json::from_str(snapshot.bundle_json.as_str())
            .map_err(|error| JournalError::InvalidCanvasReplay {
                canvas_id: snapshot.canvas_id.clone(),
                reason: format!("snapshot bundle_json is invalid: {error}"),
            })?;
        let allowed_parent_origins: Vec<String> = serde_json::from_str(
            snapshot.allowed_parent_origins_json.as_str(),
        )
        .map_err(|error| JournalError::InvalidCanvasReplay {
            canvas_id: snapshot.canvas_id.clone(),
            reason: format!("snapshot allowed_parent_origins_json is invalid: {error}"),
        })?;
        if snapshot.state_version == 0 {
            return Err(JournalError::InvalidCanvasReplay {
                canvas_id: snapshot.canvas_id.clone(),
                reason: "snapshot state_version must be greater than 0".to_owned(),
            });
        }
        if snapshot.state_schema_version == 0 {
            return Err(JournalError::InvalidCanvasReplay {
                canvas_id: snapshot.canvas_id.clone(),
                reason: "snapshot state_schema_version must be greater than 0".to_owned(),
            });
        }
        records.insert(
            snapshot.canvas_id.clone(),
            CanvasRecord {
                canvas_id: snapshot.canvas_id.clone(),
                session_id: snapshot.session_id.clone(),
                principal: snapshot.principal.clone(),
                state_version: snapshot.state_version,
                state_schema_version: snapshot.state_schema_version,
                state_json: snapshot.state_json.as_bytes().to_vec(),
                bundle,
                allowed_parent_origins,
                created_at_unix_ms: snapshot.created_at_unix_ms,
                updated_at_unix_ms: snapshot.updated_at_unix_ms,
                expires_at_unix_ms: snapshot.expires_at_unix_ms,
                closed: snapshot.closed,
                close_reason: snapshot.close_reason.clone(),
                update_timestamps_unix_ms: VecDeque::new(),
            },
        );
    }
    Ok(records)
}

pub(crate) fn generate_canvas_signing_secret() -> [u8; 32] {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let mut hasher = Sha256::new();
    hasher.update(Ulid::new().to_string().as_bytes());
    hasher.update(now.to_string().as_bytes());
    let digest = hasher.finalize();
    let mut secret = [0_u8; 32];
    secret.copy_from_slice(&digest[..32]);
    secret
}

#[allow(clippy::result_large_err)]
pub(crate) fn normalize_canvas_identifier(
    raw: &str,
    field_name: &'static str,
) -> Result<String, Status> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Status::invalid_argument(format!("{field_name} cannot be empty")));
    }
    if trimmed.len() > MAX_CANVAS_ID_BYTES {
        return Err(Status::invalid_argument(format!(
            "{field_name} exceeds maximum bytes ({} > {MAX_CANVAS_ID_BYTES})",
            trimmed.len()
        )));
    }
    validate_canonical_id(trimmed).map_err(|_| {
        Status::invalid_argument(format!("{field_name} must be a canonical ULID identifier"))
    })?;
    Ok(trimmed.to_owned())
}

#[allow(clippy::result_large_err)]
pub(crate) fn normalize_canvas_bundle_identifier(raw: &str) -> Result<String, Status> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(format!("bundle-{}", Ulid::new().to_string().to_ascii_lowercase()));
    }
    if trimmed.len() > MAX_CANVAS_BUNDLE_ID_BYTES {
        return Err(Status::invalid_argument(format!(
            "bundle.bundle_id exceeds maximum bytes ({} > {MAX_CANVAS_BUNDLE_ID_BYTES})",
            trimmed.len()
        )));
    }
    if !trimmed.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')) {
        return Err(Status::invalid_argument("bundle.bundle_id contains unsupported characters"));
    }
    Ok(trimmed.to_ascii_lowercase())
}

#[allow(clippy::result_large_err)]
pub(crate) fn normalize_canvas_asset_path(raw: &str, field_name: &str) -> Result<String, Status> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Status::invalid_argument(format!("{field_name} cannot be empty")));
    }
    if trimmed.len() > MAX_CANVAS_ASSET_PATH_BYTES {
        return Err(Status::invalid_argument(format!(
            "{field_name} exceeds maximum bytes ({} > {MAX_CANVAS_ASSET_PATH_BYTES})",
            trimmed.len()
        )));
    }
    if trimmed.starts_with('/') || trimmed.starts_with('\\') || trimmed.contains('\\') {
        return Err(Status::invalid_argument(format!(
            "{field_name} must be a relative forward-slash path"
        )));
    }
    if trimmed.contains("..") {
        return Err(Status::invalid_argument(format!(
            "{field_name} cannot contain parent traversal"
        )));
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '_' | '.' | '~'))
    {
        return Err(Status::invalid_argument(format!(
            "{field_name} contains unsupported characters"
        )));
    }
    if trimmed.split('/').any(|segment| segment.is_empty() || segment == "." || segment == "..") {
        return Err(Status::invalid_argument(format!(
            "{field_name} contains invalid path segment"
        )));
    }
    Ok(trimmed.to_owned())
}

#[allow(clippy::result_large_err)]
pub(crate) fn normalize_canvas_asset_content_type(
    raw: &str,
    field_name: &str,
) -> Result<String, Status> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Err(Status::invalid_argument(format!("{field_name}.content_type cannot be empty")));
    }
    if trimmed.len() > MAX_CANVAS_ASSET_CONTENT_TYPE_BYTES {
        return Err(Status::invalid_argument(format!(
            "{field_name}.content_type exceeds maximum bytes ({} > {MAX_CANVAS_ASSET_CONTENT_TYPE_BYTES})",
            trimmed.len()
        )));
    }
    if trimmed.contains(';') || trimmed.contains(char::is_whitespace) {
        return Err(Status::invalid_argument(format!(
            "{field_name}.content_type must not include parameters or whitespace"
        )));
    }
    if !matches!(
        trimmed.as_str(),
        "application/javascript"
            | "text/javascript"
            | "text/css"
            | "application/json"
            | "text/plain"
            | "image/svg+xml"
    ) {
        return Err(Status::failed_precondition(format!(
            "{field_name}.content_type '{trimmed}' is not allowed by canvas host policy"
        )));
    }
    Ok(trimmed)
}

pub(crate) fn is_canvas_javascript_content_type(content_type: &str) -> bool {
    matches!(content_type, "application/javascript" | "text/javascript")
}

pub(crate) fn compute_canvas_bundle_sha256(assets: &HashMap<String, CanvasAssetRecord>) -> String {
    let mut ordered = BTreeMap::new();
    for (path, asset) in assets.iter() {
        ordered.insert(path, asset);
    }
    let mut hasher = Sha256::new();
    for (path, asset) in ordered {
        hasher.update(path.as_bytes());
        hasher.update(b"\n");
        hasher.update(asset.content_type.as_bytes());
        hasher.update(b"\n");
        hasher.update(asset.body.as_slice());
        hasher.update(b"\n--\n");
    }
    hex::encode(hasher.finalize())
}

#[allow(clippy::result_large_err)]
pub(crate) fn parse_canvas_allowed_parent_origins(
    origins: &[String],
) -> Result<Vec<String>, Status> {
    if origins.is_empty() {
        return Err(Status::invalid_argument(
            "allowed_parent_origins must include at least one origin",
        ));
    }
    if origins.len() > MAX_CANVAS_ALLOWED_PARENT_ORIGINS {
        return Err(Status::invalid_argument(format!(
            "allowed_parent_origins exceeds limit ({} > {MAX_CANVAS_ALLOWED_PARENT_ORIGINS})",
            origins.len()
        )));
    }
    let mut normalized = Vec::new();
    for (index, origin) in origins.iter().enumerate() {
        let source = format!("allowed_parent_origins[{index}]");
        let parsed = normalize_canvas_origin(origin.as_str(), source.as_str())?;
        if !normalized.iter().any(|existing| existing == &parsed) {
            normalized.push(parsed);
        }
    }
    Ok(normalized)
}

#[allow(clippy::result_large_err)]
pub(crate) fn normalize_canvas_origin(raw: &str, field_name: &str) -> Result<String, Status> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Status::invalid_argument(format!("{field_name} cannot be empty")));
    }
    if trimmed.len() > MAX_CANVAS_ORIGIN_BYTES {
        return Err(Status::invalid_argument(format!(
            "{field_name} exceeds maximum bytes ({} > {MAX_CANVAS_ORIGIN_BYTES})",
            trimmed.len()
        )));
    }
    let parsed = Url::parse(trimmed).map_err(|error| {
        Status::invalid_argument(format!("{field_name} must be a valid URL: {error}"))
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(Status::invalid_argument(format!(
            "{field_name} must use http or https scheme"
        )));
    }
    if parsed.host_str().is_none() {
        return Err(Status::invalid_argument(format!("{field_name} must include host")));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(Status::invalid_argument(format!("{field_name} must not include credentials")));
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(Status::invalid_argument(format!(
            "{field_name} must not include query or fragment"
        )));
    }
    if parsed.path() != "/" && !parsed.path().is_empty() {
        return Err(Status::invalid_argument(format!(
            "{field_name} must not include path segments"
        )));
    }
    Ok(parsed.origin().ascii_serialization())
}

pub(crate) fn build_canvas_csp_header(allowed_parent_origins: &[String]) -> String {
    let frame_ancestors = if allowed_parent_origins.is_empty() {
        "'none'".to_owned()
    } else {
        allowed_parent_origins.join(" ")
    };
    format!(
        "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors {frame_ancestors}; sandbox allow-scripts"
    )
}

pub(crate) fn url_encode_component(raw: &str) -> String {
    percent_encode_canvas(raw, false)
}

pub(crate) fn url_encode_path_component(raw: &str) -> String {
    percent_encode_canvas(raw, true)
}

fn percent_encode_canvas(raw: &str, allow_slash: bool) -> String {
    let mut encoded = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        let is_unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');
        if is_unreserved || (allow_slash && byte == b'/') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(format!("{byte:02X}").as_str());
        }
    }
    encoded
}

pub(crate) fn escape_html_attribute(raw: &str) -> String {
    raw.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::{
        build_canvas_experiment_governance_snapshot, CanvasHostRuntimeConfig,
        CANVAS_EXPERIMENT_STRUCTURED_CONTRACT,
    };

    fn canvas_host_config(enabled: bool) -> CanvasHostRuntimeConfig {
        CanvasHostRuntimeConfig {
            enabled,
            public_base_url: "http://127.0.0.1:7142".to_owned(),
            token_ttl_ms: 60_000,
            max_state_bytes: 8_192,
            max_bundle_bytes: 65_536,
            max_assets_per_bundle: 8,
            max_updates_per_minute: 30,
        }
    }

    #[test]
    fn canvas_experiment_snapshot_defaults_to_disabled_rollout() {
        let snapshot = build_canvas_experiment_governance_snapshot(&canvas_host_config(false));

        assert_eq!(snapshot.structured_contract, CANVAS_EXPERIMENT_STRUCTURED_CONTRACT);
        assert!(snapshot.fail_closed);
        assert!(snapshot.requires_console_diagnostics);
        assert!(!snapshot.native_canvas.enabled);
        assert_eq!(snapshot.native_canvas.rollout_stage, "disabled");
        assert_eq!(snapshot.native_canvas.feature_flag, "canvas_host.enabled");
        assert_eq!(snapshot.native_canvas.ambient_mode, "disabled");
    }

    #[test]
    fn canvas_experiment_snapshot_carries_bounded_limits_when_enabled() {
        let snapshot = build_canvas_experiment_governance_snapshot(&canvas_host_config(true));

        assert!(snapshot.native_canvas.enabled);
        assert_eq!(snapshot.native_canvas.rollout_stage, "operator_preview");
        assert_eq!(snapshot.native_canvas.limits.max_state_bytes, 8_192);
        assert_eq!(snapshot.native_canvas.limits.max_bundle_bytes, 65_536);
        assert_eq!(snapshot.native_canvas.limits.max_assets_per_bundle, 8);
        assert_eq!(snapshot.native_canvas.limits.max_updates_per_minute, 30);
        assert_eq!(snapshot.native_canvas.exit_criteria.len(), 2);
    }
}
