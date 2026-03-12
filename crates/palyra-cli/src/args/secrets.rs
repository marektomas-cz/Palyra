use clap::Subcommand;

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum SecretsCommand {
    Set {
        scope: String,
        key: String,
        #[arg(long, default_value_t = false)]
        value_stdin: bool,
    },
    Get {
        scope: String,
        key: String,
        #[arg(long, default_value_t = false)]
        reveal: bool,
    },
    List {
        scope: String,
    },
    Delete {
        scope: String,
        key: String,
    },
}
