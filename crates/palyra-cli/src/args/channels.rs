use clap::{Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ChannelProviderArg {
    Discord,
    Slack,
    Telegram,
    Webhook,
    Echo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ChannelResolveEntityArg {
    Channel,
    Conversation,
    Thread,
    User,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum ChannelsDiscordCommand {
    Setup {
        #[arg(long, default_value = "default")]
        account_id: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        verify_channel_id: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Status {
        #[arg(long, default_value = "default")]
        account_id: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    HealthRefresh {
        #[arg(long, default_value = "default")]
        account_id: String,
        #[arg(long)]
        verify_channel_id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(visible_alias = "test-send")]
    Verify {
        #[arg(long, default_value = "default")]
        account_id: String,
        #[arg(long)]
        to: String,
        #[arg(long, default_value = "palyra discord test message")]
        text: String,
        #[arg(long, default_value_t = false)]
        confirm: bool,
        #[arg(long)]
        auto_reaction: Option<String>,
        #[arg(long)]
        thread_id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum ChannelsRouterCommand {
    Rules {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Warnings {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Preview {
        #[arg(long = "route-channel")]
        route_channel: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        conversation_id: Option<String>,
        #[arg(long)]
        sender_identity: Option<String>,
        #[arg(long)]
        sender_display: Option<String>,
        #[arg(long, default_value_t = true)]
        sender_verified: bool,
        #[arg(long, default_value_t = true)]
        is_direct_message: bool,
        #[arg(long, default_value_t = false)]
        requested_broadcast: bool,
        #[arg(long)]
        adapter_message_id: Option<String>,
        #[arg(long)]
        adapter_thread_id: Option<String>,
        #[arg(long)]
        max_payload_bytes: Option<u64>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Pairings {
        #[arg(long = "route-channel")]
        route_channel: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    MintPairingCode {
        #[arg(long = "route-channel")]
        route_channel: String,
        #[arg(long)]
        issued_by: Option<String>,
        #[arg(long)]
        ttl_ms: Option<u64>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum ChannelsCommand {
    Add {
        #[arg(long, value_enum)]
        provider: ChannelProviderArg,
        #[arg(long, default_value = "default")]
        account_id: String,
        #[arg(long, default_value_t = false)]
        interactive: bool,
        #[arg(long)]
        credential: Option<String>,
        #[arg(long, default_value_t = false)]
        credential_stdin: bool,
        #[arg(long, default_value_t = false)]
        credential_prompt: bool,
        #[arg(long, default_value = "local")]
        mode: String,
        #[arg(long, default_value = "dm_only")]
        inbound_scope: String,
        #[arg(long = "allow-from")]
        allow_from: Vec<String>,
        #[arg(long = "deny-from")]
        deny_from: Vec<String>,
        #[arg(long)]
        require_mention: Option<bool>,
        #[arg(long = "mention-pattern")]
        mention_patterns: Vec<String>,
        #[arg(long)]
        concurrency_limit: Option<u64>,
        #[arg(long)]
        direct_message_policy: Option<String>,
        #[arg(long)]
        broadcast_strategy: Option<String>,
        #[arg(long, default_value_t = false)]
        confirm_open_guild_channels: bool,
        #[arg(long)]
        verify_channel_id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Login {
        #[arg(long, value_enum)]
        provider: ChannelProviderArg,
        #[arg(long, default_value = "default")]
        account_id: String,
        #[arg(long)]
        credential: Option<String>,
        #[arg(long, default_value_t = false)]
        credential_stdin: bool,
        #[arg(long, default_value_t = false)]
        credential_prompt: bool,
        #[arg(long, default_value = "local")]
        mode: String,
        #[arg(long, default_value = "dm_only")]
        inbound_scope: String,
        #[arg(long = "allow-from")]
        allow_from: Vec<String>,
        #[arg(long = "deny-from")]
        deny_from: Vec<String>,
        #[arg(long)]
        require_mention: Option<bool>,
        #[arg(long = "mention-pattern")]
        mention_patterns: Vec<String>,
        #[arg(long)]
        concurrency_limit: Option<u64>,
        #[arg(long)]
        direct_message_policy: Option<String>,
        #[arg(long)]
        broadcast_strategy: Option<String>,
        #[arg(long, default_value_t = false)]
        confirm_open_guild_channels: bool,
        #[arg(long)]
        verify_channel_id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Logout {
        #[arg(long, value_enum)]
        provider: ChannelProviderArg,
        #[arg(long, default_value = "default")]
        account_id: String,
        #[arg(long, default_value_t = false)]
        keep_credential: bool,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Remove {
        #[arg(long, value_enum)]
        provider: ChannelProviderArg,
        #[arg(long, default_value = "default")]
        account_id: String,
        #[arg(long, default_value_t = false)]
        keep_credential: bool,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Capabilities {
        connector_id: Option<String>,
        #[arg(long, value_enum)]
        provider: Option<ChannelProviderArg>,
        #[arg(long)]
        account_id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Resolve {
        #[arg(long, value_enum)]
        provider: ChannelProviderArg,
        #[arg(long, default_value = "default")]
        account_id: String,
        #[arg(long, value_enum)]
        entity: ChannelResolveEntityArg,
        #[arg(long)]
        value: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Pairings {
        connector_id: Option<String>,
        #[arg(long, value_enum)]
        provider: Option<ChannelProviderArg>,
        #[arg(long)]
        account_id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    PairingCode {
        connector_id: Option<String>,
        #[arg(long, value_enum)]
        provider: Option<ChannelProviderArg>,
        #[arg(long)]
        account_id: Option<String>,
        #[arg(long)]
        issued_by: Option<String>,
        #[arg(long)]
        ttl_ms: Option<u64>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Qr {
        connector_id: Option<String>,
        #[arg(long, value_enum)]
        provider: Option<ChannelProviderArg>,
        #[arg(long)]
        account_id: Option<String>,
        #[arg(long)]
        issued_by: Option<String>,
        #[arg(long)]
        ttl_ms: Option<u64>,
        #[arg(long)]
        artifact: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Discord {
        #[command(subcommand)]
        command: ChannelsDiscordCommand,
    },
    Router {
        #[command(subcommand)]
        command: ChannelsRouterCommand,
    },
    List {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Status {
        connector_id: Option<String>,
        #[arg(long, value_enum)]
        provider: Option<ChannelProviderArg>,
        #[arg(long)]
        account_id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    HealthRefresh {
        connector_id: Option<String>,
        #[arg(long, value_enum)]
        provider: Option<ChannelProviderArg>,
        #[arg(long)]
        account_id: Option<String>,
        #[arg(long)]
        verify_channel_id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Enable {
        connector_id: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    QueuePause {
        connector_id: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    QueueResume {
        connector_id: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    QueueDrain {
        connector_id: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    DeadLetterReplay {
        connector_id: String,
        dead_letter_id: i64,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    DeadLetterDiscard {
        connector_id: String,
        dead_letter_id: i64,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Disable {
        connector_id: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Logs {
        connector_id: Option<String>,
        #[arg(long, value_enum)]
        provider: Option<ChannelProviderArg>,
        #[arg(long)]
        account_id: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Test {
        connector_id: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        conversation_id: Option<String>,
        #[arg(long)]
        sender_id: Option<String>,
        #[arg(long)]
        sender_display: Option<String>,
        #[arg(long, default_value_t = false)]
        simulate_crash_once: bool,
        #[arg(long, default_value_t = true)]
        is_direct_message: bool,
        #[arg(long, default_value_t = false)]
        requested_broadcast: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}
