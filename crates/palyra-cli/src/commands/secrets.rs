use crate::*;

pub(crate) fn run_secrets(command: SecretsCommand) -> Result<()> {
    let vault = open_cli_vault().context("failed to initialize vault runtime")?;
    match command {
        SecretsCommand::Set { scope, key, value_stdin } => {
            if !value_stdin {
                anyhow::bail!(
                    "secrets set requires --value-stdin to avoid exposing raw values in process args"
                );
            }
            let scope = parse_vault_scope(scope.as_str())?;
            let mut value = Vec::new();
            std::io::stdin()
                .read_to_end(&mut value)
                .context("failed to read secret value from stdin")?;
            if value.is_empty() {
                anyhow::bail!("stdin did not contain any secret bytes");
            }
            let metadata = vault
                .put_secret(&scope, key.as_str(), value.as_slice())
                .with_context(|| format!("failed to store secret key={} scope={scope}", key))?;
            println!(
                "secrets.set scope={} key={} value_bytes={} backend={}",
                scope,
                metadata.key,
                metadata.value_bytes,
                vault.backend_kind().as_str(),
            );
            std::io::stdout().flush().context("stdout flush failed")
        }
        SecretsCommand::Get { scope, key, reveal } => {
            let scope = parse_vault_scope(scope.as_str())?;
            let value = vault
                .get_secret(&scope, key.as_str())
                .with_context(|| format!("failed to load secret key={} scope={scope}", key))?;
            if reveal {
                eprintln!(
                    "warning: printing secret bytes to stdout can leak via shell history or logs"
                );
                std::io::stdout()
                    .write_all(value.as_slice())
                    .context("failed to write secret value to stdout")?;
            } else {
                println!(
                    "secrets.get scope={} key={} value=<redacted> value_bytes={} reveal=false",
                    scope,
                    key,
                    value.len()
                );
            }
            std::io::stdout().flush().context("stdout flush failed")
        }
        SecretsCommand::List { scope } => {
            let scope = parse_vault_scope(scope.as_str())?;
            let secrets = vault
                .list_secrets(&scope)
                .with_context(|| format!("failed to list secrets for scope={scope}"))?;
            println!(
                "secrets.list scope={} count={} backend={}",
                scope,
                secrets.len(),
                vault.backend_kind().as_str()
            );
            for metadata in secrets {
                println!(
                    "secrets.entry key={} created_at_unix_ms={} updated_at_unix_ms={} value_bytes={}",
                    metadata.key,
                    metadata.created_at_unix_ms,
                    metadata.updated_at_unix_ms,
                    metadata.value_bytes
                );
            }
            std::io::stdout().flush().context("stdout flush failed")
        }
        SecretsCommand::Delete { scope, key } => {
            let scope = parse_vault_scope(scope.as_str())?;
            let deleted = vault
                .delete_secret(&scope, key.as_str())
                .with_context(|| format!("failed to delete secret key={} scope={scope}", key))?;
            println!("secrets.delete scope={} key={} deleted={}", scope, key, deleted);
            std::io::stdout().flush().context("stdout flush failed")
        }
    }
}
