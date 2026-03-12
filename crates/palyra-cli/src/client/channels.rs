use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde_json::Value;

use crate::{env, DEFAULT_DAEMON_URL};

pub(crate) fn resolve_base_url(url: Option<String>) -> String {
    url.or_else(|| env::var("PALYRA_DAEMON_URL").ok())
        .unwrap_or_else(|| DEFAULT_DAEMON_URL.to_owned())
}

pub(crate) fn resolve_token(token: Option<String>) -> Option<String> {
    token.or_else(|| env::var("PALYRA_ADMIN_TOKEN").ok())
}

pub(crate) fn build_client() -> Result<Client> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .context("failed to build channels HTTP client")
}

pub(crate) fn send_request(
    request: reqwest::blocking::RequestBuilder,
    token: Option<String>,
    principal: String,
    device_id: String,
    channel: Option<String>,
    error_context: &'static str,
) -> Result<Value> {
    let mut request =
        request.header("x-palyra-principal", principal).header("x-palyra-device-id", device_id);
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    if let Some(channel) = channel {
        request = request.header("x-palyra-channel", channel);
    }
    let response = request.send().context(error_context)?;
    let response =
        response.error_for_status().context("channels endpoint returned non-success status")?;
    response.json().context("failed to parse channels endpoint JSON payload")
}
