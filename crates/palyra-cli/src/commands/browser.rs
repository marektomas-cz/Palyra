use crate::*;

pub(crate) fn run_browser(command: BrowserCommand) -> Result<()> {
    match command {
        BrowserCommand::Status { url } => {
            let base_url = url.unwrap_or_else(|| DEFAULT_BROWSER_URL.to_owned());
            let status_url = format!("{}/healthz", base_url.trim_end_matches('/'));
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .context("failed to build HTTP client")?;
            let response = fetch_health_with_retry(&client, &status_url)?;
            println!(
                "browser.status={} service={} version={} git_hash={} uptime_seconds={}",
                response.status,
                response.service,
                response.version,
                response.git_hash,
                response.uptime_seconds
            );
            std::io::stdout().flush().context("stdout flush failed")
        }
        BrowserCommand::Open { url } => {
            println!(
                "browser.open status=stub target_url={} message=\"browser action APIs ship in M24-M26\"",
                url
            );
            std::io::stdout().flush().context("stdout flush failed")
        }
    }
}
