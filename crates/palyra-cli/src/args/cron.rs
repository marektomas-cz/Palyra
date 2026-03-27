use clap::{Subcommand, ValueEnum};

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum CronCommand {
    Status {
        #[arg(long)]
        after: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        enabled: Option<bool>,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    List {
        #[arg(long)]
        after: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        enabled: Option<bool>,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Show {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        prompt: String,
        #[arg(long, value_enum)]
        schedule_type: CronScheduleTypeArg,
        #[arg(long)]
        schedule: String,
        #[arg(long, default_value_t = true)]
        enabled: bool,
        #[arg(long, value_enum, default_value_t = CronConcurrencyPolicyArg::Forbid)]
        concurrency: CronConcurrencyPolicyArg,
        #[arg(long, default_value_t = 1)]
        retry_max_attempts: u32,
        #[arg(long, default_value_t = 1000)]
        retry_backoff_ms: u64,
        #[arg(long, value_enum, default_value_t = CronMisfirePolicyArg::Skip)]
        misfire: CronMisfirePolicyArg,
        #[arg(long, default_value_t = 0)]
        jitter_ms: u64,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        session_key: Option<String>,
        #[arg(long)]
        session_label: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(visible_alias = "edit")]
    Update {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        prompt: Option<String>,
        #[arg(long, value_enum, requires = "schedule")]
        schedule_type: Option<CronScheduleTypeArg>,
        #[arg(long, requires = "schedule_type")]
        schedule: Option<String>,
        #[arg(long)]
        enabled: Option<bool>,
        #[arg(long, value_enum)]
        concurrency: Option<CronConcurrencyPolicyArg>,
        #[arg(long, requires = "retry_backoff_ms")]
        retry_max_attempts: Option<u32>,
        #[arg(long, requires = "retry_max_attempts")]
        retry_backoff_ms: Option<u64>,
        #[arg(long, value_enum)]
        misfire: Option<CronMisfirePolicyArg>,
        #[arg(long)]
        jitter_ms: Option<u64>,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        session_key: Option<String>,
        #[arg(long)]
        session_label: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Enable {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Disable {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    RunNow {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(visible_alias = "rm")]
    Delete {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(visible_alias = "runs")]
    Logs {
        #[arg(long)]
        id: String,
        #[arg(long)]
        after: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CronScheduleTypeArg {
    Cron,
    Every,
    At,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CronConcurrencyPolicyArg {
    Forbid,
    Replace,
    QueueOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CronMisfirePolicyArg {
    Skip,
    CatchUp,
}
