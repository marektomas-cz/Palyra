use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use palyra_connectors::{
    providers::default_instance_specs, ConnectorInstanceSpec, ConnectorSupervisorConfig,
};

pub(super) fn media_db_path_from_connector_db_path(connector_db_path: &std::path::Path) -> PathBuf {
    let parent = connector_db_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    parent.join("media.sqlite3")
}

pub(super) fn media_content_root_from_connector_db_path(
    connector_db_path: &std::path::Path,
) -> PathBuf {
    let parent = connector_db_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    parent.join("media")
}

pub(super) fn default_connector_specs() -> Vec<ConnectorInstanceSpec> {
    default_instance_specs()
}

pub(super) fn route_message_max_payload_bytes(config: &ConnectorSupervisorConfig) -> u64 {
    u64::try_from(config.max_outbound_body_bytes).unwrap_or(u64::MAX)
}

pub(super) fn unix_ms_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(i64::MAX))
        .unwrap_or_default()
}
