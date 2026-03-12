use palyra_connector_core::{ConnectorInstanceSpec, ConnectorKind};

use crate::{
    discord_auth_profile_ref, discord_connector_id, discord_principal, discord_token_vault_ref,
    normalize_discord_account_id, DiscordSemanticsError,
};

const DISCORD_DEFAULT_EGRESS_ALLOWLIST: [&str; 8] = [
    "discord.com",
    "*.discord.com",
    "discordapp.com",
    "*.discordapp.com",
    "discord.gg",
    "*.discord.gg",
    "discordapp.net",
    "*.discordapp.net",
];

#[must_use]
pub fn discord_default_egress_allowlist() -> Vec<String> {
    DISCORD_DEFAULT_EGRESS_ALLOWLIST.iter().map(|entry| (*entry).to_owned()).collect()
}

pub fn discord_connector_spec(
    account_id: &str,
    enabled: bool,
) -> Result<ConnectorInstanceSpec, DiscordSemanticsError> {
    let normalized = normalize_discord_account_id(account_id)?;
    Ok(ConnectorInstanceSpec {
        connector_id: discord_connector_id(normalized.as_str()),
        kind: ConnectorKind::Discord,
        principal: discord_principal(normalized.as_str()),
        auth_profile_ref: Some(discord_auth_profile_ref(normalized.as_str())),
        token_vault_ref: Some(discord_token_vault_ref(normalized.as_str())),
        egress_allowlist: discord_default_egress_allowlist(),
        enabled,
    })
}
