use crate::*;

pub(crate) fn run_completion(shell: CompletionShell) -> Result<()> {
    let mut command = Cli::command();
    clap_complete::generate(to_clap_shell(shell), &mut command, "palyra", &mut std::io::stdout());
    std::io::stdout().flush().context("stdout flush failed")
}
