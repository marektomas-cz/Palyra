use clap::{Args, Subcommand};

#[derive(Debug, Clone, PartialEq, Eq, Args, Default)]
pub struct AcpConnectionArgs {
    #[arg(long)]
    pub grpc_url: Option<String>,
    #[arg(long)]
    pub token: Option<String>,
    #[arg(long)]
    pub principal: Option<String>,
    #[arg(long)]
    pub device_id: Option<String>,
    #[arg(long)]
    pub channel: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args, Default)]
pub struct AcpSessionDefaultsArgs {
    #[arg(long)]
    pub session_key: Option<String>,
    #[arg(long)]
    pub session_label: Option<String>,
    #[arg(long, default_value_t = false)]
    pub require_existing: bool,
    #[arg(long, default_value_t = false)]
    pub reset_session: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Args, Default)]
pub struct AcpBridgeArgs {
    #[command(flatten)]
    pub connection: AcpConnectionArgs,
    #[command(flatten)]
    pub session_defaults: AcpSessionDefaultsArgs,
    #[arg(long, default_value_t = false)]
    pub allow_sensitive_tools: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Args, Default)]
pub struct AcpShimArgs {
    #[command(flatten)]
    pub connection: AcpConnectionArgs,
    #[arg(long)]
    pub session_id: Option<String>,
    #[command(flatten)]
    pub session_defaults: AcpSessionDefaultsArgs,
    #[arg(long)]
    pub run_id: Option<String>,
    #[arg(long)]
    pub prompt: Option<String>,
    #[arg(long, default_value_t = false)]
    pub prompt_stdin: bool,
    #[arg(long, default_value_t = false)]
    pub allow_sensitive_tools: bool,
    #[arg(
        long,
        default_value_t = false,
        conflicts_with_all = ["session_id", "run_id", "prompt", "prompt_stdin"]
    )]
    pub ndjson_stdin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum AcpSubcommand {
    #[command(about = "Run the legacy NDJSON ACP compatibility shim")]
    Shim {
        #[command(flatten)]
        command: AcpShimArgs,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Args, Default)]
#[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
pub struct AcpCommand {
    #[command(flatten)]
    pub bridge: AcpBridgeArgs,
    #[command(subcommand)]
    pub subcommand: Option<AcpSubcommand>,
}
