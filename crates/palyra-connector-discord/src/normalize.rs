use crate::DiscordSemanticsError;

#[must_use]
pub fn is_discord_connector(connector_id: &str) -> bool {
    connector_id.trim().to_ascii_lowercase().starts_with("discord:")
}

#[must_use]
pub fn canonical_discord_sender_identity(raw: &str) -> String {
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
pub fn canonical_discord_channel_identity(raw: &str) -> String {
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

pub fn normalize_discord_target(raw: &str) -> Result<String, DiscordSemanticsError> {
    let trimmed = raw.trim();
    let normalized = trimmed
        .strip_prefix("channel:")
        .or_else(|| trimmed.strip_prefix("thread:"))
        .map(str::trim)
        .unwrap_or(trimmed);
    if normalized.is_empty() {
        return Err(DiscordSemanticsError::EmptyTarget);
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.'))
    {
        return Err(DiscordSemanticsError::InvalidTarget);
    }
    Ok(normalized.to_owned())
}

fn parse_discord_user_mention(raw: &str) -> Option<&str> {
    let body = raw.strip_prefix("<@")?.strip_suffix('>')?;
    body.strip_prefix('!').or(Some(body))
}

fn parse_discord_channel_mention(raw: &str) -> Option<&str> {
    raw.strip_prefix("<#")?.strip_suffix('>')
}
