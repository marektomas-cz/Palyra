use palyra_connectors::{ConnectorInstanceSpec, ConnectorKind};

use super::ChannelPlatformError;

#[must_use]
pub fn discord_connector_id(account_id: &str) -> String {
    format!("discord:{}", account_id.trim().to_ascii_lowercase())
}

#[must_use]
pub fn discord_principal(account_id: &str) -> String {
    format!("channel:{}", discord_connector_id(account_id))
}

#[must_use]
pub fn discord_token_vault_ref(account_id: &str) -> String {
    let normalized = account_id.trim().to_ascii_lowercase();
    if normalized == "default" {
        return "global/discord_bot_token".to_owned();
    }
    format!("global/discord_bot_token.{normalized}")
}

#[allow(clippy::result_large_err)]
pub fn normalize_discord_account_id(raw: &str) -> Result<String, ChannelPlatformError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ChannelPlatformError::InvalidInput(
            "discord account_id cannot be empty".to_owned(),
        ));
    }
    if !trimmed.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')) {
        return Err(ChannelPlatformError::InvalidInput(
            "discord account_id contains unsupported characters".to_owned(),
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

pub(super) fn discord_connector_spec(account_id: &str, enabled: bool) -> ConnectorInstanceSpec {
    let normalized = account_id.trim().to_ascii_lowercase();
    ConnectorInstanceSpec {
        connector_id: discord_connector_id(normalized.as_str()),
        kind: ConnectorKind::Discord,
        principal: discord_principal(normalized.as_str()),
        auth_profile_ref: Some(format!("discord.{normalized}")),
        token_vault_ref: Some(discord_token_vault_ref(normalized.as_str())),
        egress_allowlist: vec![
            "discord.com".to_owned(),
            "*.discord.com".to_owned(),
            "discordapp.com".to_owned(),
            "*.discordapp.com".to_owned(),
            "discord.gg".to_owned(),
            "*.discord.gg".to_owned(),
            "discordapp.net".to_owned(),
            "*.discordapp.net".to_owned(),
        ],
        enabled,
    }
}

#[must_use]
pub fn discord_default_egress_allowlist() -> Vec<String> {
    discord_connector_spec("default", false).egress_allowlist
}

#[must_use]
pub(super) fn is_discord_connector(connector_id: &str) -> bool {
    connector_id.trim().to_ascii_lowercase().starts_with("discord:")
}

#[must_use]
pub(super) fn canonical_discord_sender_identity(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "discord:user:unknown".to_owned();
    }
    let normalized = trimmed
        .strip_prefix("discord:user:")
        .or_else(|| trimmed.strip_prefix("user:"))
        .map(str::trim)
        .or_else(|| parse_discord_user_mention(trimmed))
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    format!("discord:user:{normalized}")
}

#[must_use]
pub(super) fn canonical_discord_channel_identity(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "discord:channel:unknown".to_owned();
    }
    let normalized = trimmed
        .strip_prefix("discord:channel:")
        .or_else(|| trimmed.strip_prefix("channel:"))
        .or_else(|| trimmed.strip_prefix("thread:"))
        .map(str::trim)
        .or_else(|| parse_discord_channel_mention(trimmed))
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    format!("discord:channel:{normalized}")
}

fn parse_discord_user_mention(raw: &str) -> Option<&str> {
    let body = raw.strip_prefix("<@")?.strip_suffix('>')?;
    body.strip_prefix('!').or(Some(body))
}

fn parse_discord_channel_mention(raw: &str) -> Option<&str> {
    raw.strip_prefix("<#")?.strip_suffix('>')
}

#[allow(clippy::result_large_err)]
pub(super) fn normalize_discord_target(raw: &str) -> Result<String, ChannelPlatformError> {
    let trimmed = raw.trim();
    let normalized = trimmed
        .strip_prefix("channel:")
        .or_else(|| trimmed.strip_prefix("thread:"))
        .map(str::trim)
        .unwrap_or(trimmed);
    if normalized.is_empty() {
        return Err(ChannelPlatformError::InvalidInput(
            "discord test target cannot be empty".to_owned(),
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.'))
    {
        return Err(ChannelPlatformError::InvalidInput(
            "discord test target contains unsupported characters".to_owned(),
        ));
    }
    Ok(normalized.to_owned())
}
