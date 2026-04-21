use clap::{Subcommand, ValueEnum};

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum FlowsCommand {
    List {
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long, value_enum)]
        state: Option<FlowStateArg>,
        #[arg(long, default_value_t = false)]
        active_only: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Show {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Pause {
        #[arg(long)]
        id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Resume {
        #[arg(long)]
        id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Cancel {
        #[arg(long)]
        id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    RetryStep {
        #[arg(long)]
        id: String,
        #[arg(long)]
        step_id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    SkipStep {
        #[arg(long)]
        id: String,
        #[arg(long)]
        step_id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    CompensateStep {
        #[arg(long)]
        id: String,
        #[arg(long)]
        step_id: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum FlowStateArg {
    Pending,
    Ready,
    Running,
    WaitingForApproval,
    Paused,
    Blocked,
    Retrying,
    Compensating,
    TimedOut,
    Succeeded,
    Failed,
    CancelRequested,
    Cancelled,
}

impl FlowStateArg {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::WaitingForApproval => "waiting_for_approval",
            Self::Paused => "paused",
            Self::Blocked => "blocked",
            Self::Retrying => "retrying",
            Self::Compensating => "compensating",
            Self::TimedOut => "timed_out",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::CancelRequested => "cancel_requested",
            Self::Cancelled => "cancelled",
        }
    }
}
