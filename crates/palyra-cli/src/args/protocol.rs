use clap::Subcommand;

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum ProtocolCommand {
    Version,
    ValidateId {
        #[arg(long)]
        id: String,
    },
}
