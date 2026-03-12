use crate::*;

pub(crate) fn connector_id(account_id: &str) -> Result<String> {
    let normalized = palyra_connector_discord::normalize_discord_account_id(account_id)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(palyra_connector_discord::discord_connector_id(normalized.as_str()))
}

pub(crate) fn probe_payload(
    account_id: &str,
    token: &str,
    setup_mode: &str,
    verify_channel_id: Option<&str>,
) -> Value {
    json!({
        "account_id": account_id,
        "token": token,
        "mode": setup_mode,
        "verify_channel_id": verify_channel_id,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_payload(
    account_id: &str,
    token: &str,
    setup_mode: &str,
    inbound_scope: &str,
    allow_from: &[String],
    deny_from: &[String],
    require_mention: bool,
    concurrency_limit: u64,
    broadcast_strategy: &str,
    confirm_open_guild_channels: bool,
    verify_channel_id: Option<&str>,
) -> Value {
    json!({
        "account_id": account_id,
        "token": token,
        "mode": setup_mode,
        "inbound_scope": inbound_scope,
        "allow_from": allow_from,
        "deny_from": deny_from,
        "require_mention": require_mention,
        "concurrency_limit": concurrency_limit,
        "broadcast_strategy": broadcast_strategy,
        "confirm_open_guild_channels": confirm_open_guild_channels,
        "verify_channel_id": verify_channel_id,
    })
}

pub(crate) fn verify_payload(
    target: &str,
    text: &str,
    auto_reaction: Option<&str>,
    thread_id: Option<&str>,
) -> Value {
    json!({
        "target": target,
        "text": text,
        "confirm": true,
        "auto_reaction": auto_reaction,
        "thread_id": thread_id,
    })
}
