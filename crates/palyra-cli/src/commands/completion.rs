use crate::*;
use std::io::{self, ErrorKind, Write};

pub(crate) fn run_completion(shell: CompletionShell) -> Result<()> {
    let mut command = Cli::command();
    let mut completion = Vec::new();
    clap_complete::generate(to_clap_shell(shell), &mut command, "palyra", &mut completion);

    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    write_stdout_allow_broken_pipe(&mut stdout, completion.as_slice())?;
    flush_stdout_allow_broken_pipe(&mut stdout)
}

fn write_stdout_allow_broken_pipe(writer: &mut impl Write, bytes: &[u8]) -> Result<()> {
    match writer.write_all(bytes) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error).context("stdout write failed"),
    }
}

fn flush_stdout_allow_broken_pipe(writer: &mut impl Write) -> Result<()> {
    match writer.flush() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error).context("stdout flush failed"),
    }
}
