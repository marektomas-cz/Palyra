use clap::{Subcommand, ValueEnum};

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum AuthCommand {
    Profiles {
        #[command(subcommand)]
        command: AuthProfilesCommand,
    },
    Openai {
        #[command(subcommand)]
        command: AuthOpenAiCommand,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum AuthProfilesCommand {
    List {
        #[arg(long)]
        after: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long, value_enum)]
        provider: Option<AuthProviderArg>,
        #[arg(long)]
        provider_name: Option<String>,
        #[arg(long, value_enum)]
        scope: Option<AuthScopeArg>,
        #[arg(long)]
        agent_id: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Show {
        profile_id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Set {
        profile_id: String,
        #[arg(long, value_enum)]
        provider: AuthProviderArg,
        #[arg(long)]
        provider_name: Option<String>,
        #[arg(long)]
        profile_name: String,
        #[arg(long, value_enum, default_value_t = AuthScopeArg::Global)]
        scope: AuthScopeArg,
        #[arg(long)]
        agent_id: Option<String>,
        #[arg(long, value_enum)]
        credential: AuthCredentialArg,
        #[arg(long)]
        api_key_ref: Option<String>,
        #[arg(long)]
        access_token_ref: Option<String>,
        #[arg(long)]
        refresh_token_ref: Option<String>,
        #[arg(long)]
        token_endpoint: Option<String>,
        #[arg(long)]
        client_id: Option<String>,
        #[arg(long)]
        client_secret_ref: Option<String>,
        #[arg(long = "scope-value")]
        scope_value: Vec<String>,
        #[arg(long)]
        expires_at_unix_ms: Option<i64>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Delete {
        profile_id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Health {
        #[arg(long)]
        agent_id: Option<String>,
        #[arg(long, default_value_t = false)]
        include_profiles: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum AuthOpenAiCommand {
    Status {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    ApiKey {
        #[arg(long)]
        profile_id: Option<String>,
        #[arg(long, default_value = "OpenAI")]
        profile_name: String,
        #[arg(long, value_enum, default_value_t = AuthScopeArg::Global)]
        scope: AuthScopeArg,
        #[arg(long)]
        agent_id: Option<String>,
        #[arg(long)]
        api_key_env: Option<String>,
        #[arg(long, default_value_t = false)]
        api_key_stdin: bool,
        #[arg(long, default_value_t = false)]
        api_key_prompt: bool,
        #[arg(long, default_value_t = false)]
        set_default: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    OauthStart {
        #[arg(long)]
        profile_id: Option<String>,
        #[arg(long)]
        profile_name: Option<String>,
        #[arg(long, value_enum, default_value_t = AuthScopeArg::Global)]
        scope: AuthScopeArg,
        #[arg(long)]
        agent_id: Option<String>,
        #[arg(long)]
        client_id: String,
        #[arg(long)]
        client_secret_env: Option<String>,
        #[arg(long, default_value_t = false)]
        client_secret_stdin: bool,
        #[arg(long, default_value_t = false)]
        client_secret_prompt: bool,
        #[arg(long = "scope-value")]
        scope_value: Vec<String>,
        #[arg(long, default_value_t = false)]
        set_default: bool,
        #[arg(long, default_value_t = false)]
        open: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    OauthState {
        attempt_id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Refresh {
        profile_id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Reconnect {
        profile_id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Revoke {
        profile_id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    UseProfile {
        profile_id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AuthProviderArg {
    Openai,
    Anthropic,
    Telegram,
    Slack,
    Discord,
    Webhook,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AuthScopeArg {
    Global,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AuthCredentialArg {
    ApiKey,
    Oauth,
}
