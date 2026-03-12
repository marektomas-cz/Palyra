use clap::{Parser, Subcommand};

mod agent;
mod agents;
mod approvals;
mod auth;
mod browser;
mod channels;
mod completion;
mod config;
mod cron;
mod daemon;
mod init;
mod memory;
mod onboarding;
#[cfg(not(windows))]
mod pairing;
mod patch;
mod policy;
mod protocol;
mod secrets;
mod skills;
mod support_bundle;

pub use agent::AgentCommand;
pub use agents::AgentsCommand;
pub use approvals::{ApprovalDecisionArg, ApprovalExportFormatArg, ApprovalsCommand};
pub use auth::{
    AuthCommand, AuthCredentialArg, AuthProfilesCommand, AuthProviderArg, AuthScopeArg,
};
pub use browser::BrowserCommand;
pub use channels::{ChannelsCommand, ChannelsDiscordCommand, ChannelsRouterCommand};
pub use completion::CompletionShell;
pub use config::ConfigCommand;
pub use cron::{CronCommand, CronConcurrencyPolicyArg, CronMisfirePolicyArg, CronScheduleTypeArg};
pub use daemon::{DaemonCommand, JournalCheckpointModeArg};
pub use init::{InitModeArg, InitTlsScaffoldArg};
pub use memory::{MemoryCommand, MemoryScopeArg, MemorySourceArg};
pub use onboarding::OnboardingCommand;
#[cfg(not(windows))]
pub use pairing::{PairingClientKindArg, PairingCommand, PairingMethodArg};
pub use patch::PatchCommand;
pub use policy::PolicyCommand;
pub use protocol::ProtocolCommand;
pub use secrets::SecretsCommand;
pub use skills::{SkillsCommand, SkillsPackageCommand};
pub use support_bundle::SupportBundleCommand;

#[derive(Debug, Parser)]
#[command(name = "palyra", about = "Palyra CLI bootstrap stub")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum Command {
    Version,
    Init {
        #[arg(long, value_enum, default_value_t = InitModeArg::Local)]
        mode: InitModeArg,
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = false)]
        force: bool,
        #[arg(long, value_enum, default_value_t = InitTlsScaffoldArg::BringYourOwn)]
        tls_scaffold: InitTlsScaffoldArg,
    },
    Doctor {
        #[arg(long, default_value_t = false)]
        strict: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Status {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        grpc_url: Option<String>,
        #[arg(long, default_value_t = false)]
        admin: bool,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "user:local")]
        principal: String,
        #[arg(long, default_value = "01ARZ3NDEKTSV4RRFFQ69G5FAV")]
        device_id: String,
        #[arg(long)]
        channel: Option<String>,
    },
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    Agents {
        #[command(subcommand)]
        command: AgentsCommand,
    },
    Cron {
        #[command(subcommand)]
        command: CronCommand,
    },
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
    Approvals {
        #[command(subcommand)]
        command: ApprovalsCommand,
    },
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    Channels {
        #[command(subcommand)]
        command: ChannelsCommand,
    },
    Browser {
        #[command(subcommand)]
        command: BrowserCommand,
    },
    Completion {
        #[arg(long, value_enum)]
        shell: CompletionShell,
    },
    Onboarding {
        #[command(subcommand)]
        command: OnboardingCommand,
    },
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
    SupportBundle {
        #[command(subcommand)]
        command: SupportBundleCommand,
    },
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
    Protocol {
        #[command(subcommand)]
        command: ProtocolCommand,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Patch {
        #[command(subcommand)]
        command: PatchCommand,
    },
    #[command(visible_alias = "skill")]
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
    Secrets {
        #[command(subcommand)]
        command: SecretsCommand,
    },
    Tunnel {
        #[arg(long)]
        ssh: String,
        #[arg(long, default_value_t = 7142)]
        remote_port: u16,
        #[arg(long, default_value_t = 7142)]
        local_port: u16,
        #[arg(long, default_value_t = false)]
        open: bool,
        #[arg(long)]
        identity_file: Option<String>,
    },
    #[cfg(not(windows))]
    Pairing {
        #[command(subcommand)]
        command: PairingCommand,
    },
}

#[cfg(test)]
mod tests;
