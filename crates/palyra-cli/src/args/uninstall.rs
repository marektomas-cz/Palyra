use clap::Args;

#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct UninstallCommand {
    #[arg(long)]
    pub install_root: Option<String>,
    #[arg(long, default_value_t = false)]
    pub remove_state: bool,
    #[arg(long, default_value_t = false, conflicts_with = "dry_run")]
    pub yes: bool,
    #[arg(long, default_value_t = false, conflicts_with = "yes")]
    pub dry_run: bool,
}
