use clap::{Subcommand, ValueEnum};

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum MemoryCommand {
    Search {
        query: String,
        #[arg(long, value_enum, default_value_t = MemoryScopeArg::Principal)]
        scope: MemoryScopeArg,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        top_k: Option<u32>,
        #[arg(long)]
        min_score: Option<String>,
        #[arg(long)]
        tag: Vec<String>,
        #[arg(long, value_enum)]
        source: Vec<MemorySourceArg>,
        #[arg(long, default_value_t = false)]
        include_score_breakdown: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Purge {
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long, default_value_t = false)]
        principal: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Ingest {
        content: String,
        #[arg(long, value_enum, default_value_t = MemorySourceArg::Manual)]
        source: MemorySourceArg,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        tag: Vec<String>,
        #[arg(long)]
        confidence: Option<String>,
        #[arg(long)]
        ttl_unix_ms: Option<i64>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MemoryScopeArg {
    Session,
    Channel,
    Principal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MemorySourceArg {
    TapeUserMessage,
    TapeToolResult,
    Summary,
    Manual,
    Import,
}
