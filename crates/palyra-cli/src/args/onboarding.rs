use clap::Subcommand;

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum OnboardingCommand {
    Wizard {
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = false)]
        force: bool,
        #[arg(long, default_value = "http://127.0.0.1:7142")]
        daemon_url: String,
        #[arg(long, default_value = "PALYRA_ADMIN_TOKEN")]
        admin_token_env: String,
    },
}
