use crate::*;

pub(crate) fn run_onboarding(command: OnboardingCommand) -> Result<()> {
    match command {
        OnboardingCommand::Wizard { path, force, daemon_url, admin_token_env } => {
            if admin_token_env.trim().is_empty() {
                anyhow::bail!("admin token env variable name cannot be empty");
            }

            let config_path = resolve_onboarding_path(path)?;
            if config_path.exists() && !force {
                anyhow::bail!(
                    "onboarding target already exists: {} (use --force to overwrite)",
                    config_path.display()
                );
            }
            if let Some(parent) = config_path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent).with_context(|| {
                        format!("failed to create config directory {}", parent.display())
                    })?;
                }
            }

            let template = onboarding_template();
            let (document, _) = parse_document_with_migration(template)
                .context("failed to validate generated onboarding config")?;
            validate_daemon_compatible_document(&document)
                .context("generated onboarding config does not match daemon schema")?;
            fs::write(&config_path, template).with_context(|| {
                format!("failed to write onboarding config {}", config_path.display())
            })?;

            println!(
                "onboarding.status=complete config_path={} daemon_url={} admin_token_env={}",
                config_path.display(),
                daemon_url,
                admin_token_env
            );
            std::io::stdout().flush().context("stdout flush failed")
        }
    }
}
