use clap::Subcommand;

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum ExtensionCommand {
    Doctor {
        #[arg(long)]
        artifact: String,
        #[arg(long)]
        trust_store: Option<String>,
        #[arg(long = "trusted-publisher")]
        trusted_publishers: Vec<String>,
        #[arg(long, default_value_t = false)]
        allow_tofu: bool,
        #[arg(long = "grant")]
        grants: Vec<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}
