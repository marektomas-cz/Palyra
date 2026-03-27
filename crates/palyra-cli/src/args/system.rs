use clap::Subcommand;

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum SystemCommand {
    Heartbeat {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Presence {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    #[command(visible_alias = "events")]
    Event {
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}
