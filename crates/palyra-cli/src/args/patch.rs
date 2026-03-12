use clap::Subcommand;

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum PatchCommand {
    Apply {
        #[arg(long, default_value_t = false)]
        stdin: bool,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}
