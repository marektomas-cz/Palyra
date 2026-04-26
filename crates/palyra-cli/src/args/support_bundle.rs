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
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    ReplayExport {
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        output: String,
        #[arg(long)]
        journal_db: Option<String>,
        #[arg(long, default_value_t = 128)]
        max_events: usize,
    },
    ReplayImport {
        #[arg(long)]
        input: String,
        #[arg(long)]
        output_dir: String,
    },
    ReplayRun {
        #[arg(long)]
        input: String,
        #[arg(long)]
        diff_output: Option<String>,
    },
    ReplayBaseline {
        #[arg(long)]
        input: String,
        #[arg(long)]
        output: String,
    },
}
