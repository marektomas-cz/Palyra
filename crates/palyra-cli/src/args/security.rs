use clap::Subcommand;

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum SecurityCommand {
    Audit {
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = false)]
        offline: bool,
        #[arg(long, default_value_t = false)]
        strict: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}
