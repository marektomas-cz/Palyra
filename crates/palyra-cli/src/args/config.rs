use clap::Subcommand;

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum ConfigCommand {
    Validate {
        #[arg(long)]
        path: Option<String>,
    },
    List {
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = false)]
        show_secrets: bool,
    },
    Get {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        key: String,
        #[arg(long, default_value_t = false)]
        show_secrets: bool,
    },
    Set {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
        #[arg(long, default_value_t = 5)]
        backups: usize,
    },
    Unset {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        key: String,
        #[arg(long, default_value_t = 5)]
        backups: usize,
    },
    Migrate {
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = 5)]
        backups: usize,
    },
    Recover {
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = 1)]
        backup: usize,
        #[arg(long, default_value_t = 5)]
        backups: usize,
    },
}
