use clap::Subcommand;

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum BrowserCommand {
    Status {
        #[arg(long)]
        url: Option<String>,
    },
    Open {
        #[arg(long)]
        url: String,
    },
}
