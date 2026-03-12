use serde::Serialize;

pub const DISCORD_APP_FLAG_GATEWAY_PRESENCE: u64 = 1 << 12;
pub const DISCORD_APP_FLAG_GATEWAY_PRESENCE_LIMITED: u64 = 1 << 13;
pub const DISCORD_APP_FLAG_GATEWAY_GUILD_MEMBERS: u64 = 1 << 14;
pub const DISCORD_APP_FLAG_GATEWAY_GUILD_MEMBERS_LIMITED: u64 = 1 << 15;
pub const DISCORD_APP_FLAG_GATEWAY_MESSAGE_CONTENT: u64 = 1 << 18;
pub const DISCORD_APP_FLAG_GATEWAY_MESSAGE_CONTENT_LIMITED: u64 = 1 << 19;

pub const DISCORD_PERMISSION_VIEW_CHANNEL: u64 = 1 << 10;
pub const DISCORD_PERMISSION_SEND_MESSAGES: u64 = 1 << 11;
pub const DISCORD_PERMISSION_EMBED_LINKS: u64 = 1 << 14;
pub const DISCORD_PERMISSION_ATTACH_FILES: u64 = 1 << 15;
pub const DISCORD_PERMISSION_READ_MESSAGE_HISTORY: u64 = 1 << 16;
pub const DISCORD_PERMISSION_SEND_MESSAGES_IN_THREADS: u64 = 1 << 38;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscordPrivilegedIntentStatus {
    Enabled,
    Limited,
    Disabled,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DiscordPrivilegedIntentsSummary {
    pub message_content: DiscordPrivilegedIntentStatus,
    pub guild_members: DiscordPrivilegedIntentStatus,
    pub presence: DiscordPrivilegedIntentStatus,
}

#[must_use]
pub fn resolve_discord_intents_from_flags(flags: u64) -> DiscordPrivilegedIntentsSummary {
    let resolve = |enabled_bit: u64, limited_bit: u64| {
        if (flags & enabled_bit) != 0 {
            DiscordPrivilegedIntentStatus::Enabled
        } else if (flags & limited_bit) != 0 {
            DiscordPrivilegedIntentStatus::Limited
        } else {
            DiscordPrivilegedIntentStatus::Disabled
        }
    };
    DiscordPrivilegedIntentsSummary {
        presence: resolve(
            DISCORD_APP_FLAG_GATEWAY_PRESENCE,
            DISCORD_APP_FLAG_GATEWAY_PRESENCE_LIMITED,
        ),
        guild_members: resolve(
            DISCORD_APP_FLAG_GATEWAY_GUILD_MEMBERS,
            DISCORD_APP_FLAG_GATEWAY_GUILD_MEMBERS_LIMITED,
        ),
        message_content: resolve(
            DISCORD_APP_FLAG_GATEWAY_MESSAGE_CONTENT,
            DISCORD_APP_FLAG_GATEWAY_MESSAGE_CONTENT_LIMITED,
        ),
    }
}

#[must_use]
pub fn discord_required_permissions() -> [(&'static str, u64); 6] {
    [
        ("View Channels", DISCORD_PERMISSION_VIEW_CHANNEL),
        ("Send Messages", DISCORD_PERMISSION_SEND_MESSAGES),
        ("Read Message History", DISCORD_PERMISSION_READ_MESSAGE_HISTORY),
        ("Embed Links", DISCORD_PERMISSION_EMBED_LINKS),
        ("Attach Files", DISCORD_PERMISSION_ATTACH_FILES),
        ("Send Messages in Threads", DISCORD_PERMISSION_SEND_MESSAGES_IN_THREADS),
    ]
}

#[must_use]
pub fn discord_required_permission_labels() -> Vec<String> {
    discord_required_permissions().iter().map(|(name, _)| (*name).to_owned()).collect()
}

#[must_use]
pub fn discord_min_invite_permissions() -> u64 {
    discord_required_permissions().iter().fold(0_u64, |mask, (_, bit)| mask | *bit)
}
