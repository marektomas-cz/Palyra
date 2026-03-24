use clap::Subcommand;

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum ModelsCommand {
    Status {
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    List {
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Set {
        model: String,
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = 5)]
        backups: usize,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    SetEmbeddings {
        model: String,
        #[arg(long)]
        dims: Option<u32>,
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = 5)]
        backups: usize,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}
