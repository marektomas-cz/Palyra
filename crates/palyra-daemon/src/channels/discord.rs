use palyra_connector_discord as shared;
use palyra_connectors::ConnectorInstanceSpec;

use super::ChannelPlatformError;

pub use shared::{
    canonical_discord_channel_identity, canonical_discord_sender_identity, discord_connector_id,
    discord_default_egress_allowlist, discord_principal, discord_token_vault_ref,
    is_discord_connector,
};

#[allow(clippy::result_large_err)]
pub fn normalize_discord_account_id(raw: &str) -> Result<String, ChannelPlatformError> {
    shared::normalize_discord_account_id(raw)
        .map_err(|error| ChannelPlatformError::InvalidInput(error.to_string()))
}

pub(super) fn discord_connector_spec(account_id: &str, enabled: bool) -> ConnectorInstanceSpec {
    shared::discord_connector_spec(account_id, enabled)
        .expect("daemon should only construct Discord connector specs from validated account ids")
}

#[allow(clippy::result_large_err)]
pub(super) fn normalize_discord_target(raw: &str) -> Result<String, ChannelPlatformError> {
    shared::normalize_discord_target(raw)
        .map_err(|error| ChannelPlatformError::InvalidInput(error.to_string()))
}
