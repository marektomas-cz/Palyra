use clap::Args;

#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct TuiCommand {
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
    #[arg(long)]
    pub session_id: Option<String>,
    #[arg(long)]
    pub session_key: Option<String>,
    #[arg(long)]
    pub session_label: Option<String>,
    #[arg(long, default_value_t = false)]
    pub require_existing: bool,
    #[arg(long, default_value_t = false)]
    pub allow_sensitive_tools: bool,
    #[arg(long, default_value_t = false)]
    pub include_archived_sessions: bool,
}
