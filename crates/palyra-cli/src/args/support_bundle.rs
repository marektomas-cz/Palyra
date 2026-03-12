use clap::Subcommand;

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum SupportBundleCommand {
    Export {
        #[arg(long)]
        output: Option<String>,
        #[arg(long, default_value_t = 512 * 1024)]
        max_bytes: usize,
        #[arg(long, default_value_t = 64)]
        journal_hash_limit: usize,
        #[arg(long, default_value_t = 64)]
        error_limit: usize,
    },
}
