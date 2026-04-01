use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use palyra_common::netguard;
use reqwest::{redirect::Policy, Url};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use ulid::Ulid;

use crate::{
    gateway::{
        current_unix_ms, CachedHttpFetchEntry, GatewayRuntimeState, MAX_HTTP_FETCH_BODY_BYTES,
        MAX_HTTP_FETCH_CACHE_KEY_BYTES, MAX_HTTP_FETCH_REDIRECTS, MAX_HTTP_FETCH_TOOL_INPUT_BYTES,
    },
    tool_protocol::{ToolAttestation, ToolExecutionOutcome},
};

pub(crate) async fn execute_http_fetch_tool(
    runtime_state: &Arc<GatewayRuntimeState>,
    proposal_id: &str,
    input_json: &[u8],
) -> ToolExecutionOutcome {
    if input_json.len() > MAX_HTTP_FETCH_TOOL_INPUT_BYTES {
        return http_fetch_tool_execution_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!("palyra.http.fetch input exceeds {MAX_HTTP_FETCH_TOOL_INPUT_BYTES} bytes"),
        );
    }

    let payload = match serde_json::from_slice::<Value>(input_json) {
        Ok(Value::Object(map)) => map,
        Ok(_) => {
            return http_fetch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.http.fetch requires JSON object input".to_owned(),
            );
        }
        Err(error) => {
            return http_fetch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!("palyra.http.fetch invalid JSON input: {error}"),
            );
        }
    };

    let url_raw = match payload.get("url").and_then(Value::as_str).map(str::trim) {
        Some(value) if !value.is_empty() => value.to_owned(),
        _ => {
            return http_fetch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.http.fetch requires non-empty string field 'url'".to_owned(),
            );
        }
    };
    let method = payload
        .get("method")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_uppercase())
        .unwrap_or_else(|| "GET".to_owned());
    if !matches!(method.as_str(), "GET" | "HEAD" | "POST") {
        return http_fetch_tool_execution_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            "palyra.http.fetch method must be one of: GET|HEAD|POST".to_owned(),
        );
    }

    let body = match payload.get("body") {
        Some(Value::String(value)) => value.clone(),
        Some(_) => {
            return http_fetch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.http.fetch body must be a string".to_owned(),
            );
        }
        None => String::new(),
    };

    let request_headers = match payload.get("headers") {
        Some(Value::Object(values)) => {
            let mut headers = Vec::new();
            for (name, value) in values {
                let Value::String(raw_value) = value else {
                    return http_fetch_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        format!("palyra.http.fetch header '{name}' must be a string"),
                    );
                };
                let normalized_name = name.trim().to_ascii_lowercase();
                if normalized_name.is_empty() {
                    return http_fetch_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        "palyra.http.fetch header names cannot be empty".to_owned(),
                    );
                }
                if !runtime_state
                    .config
                    .http_fetch
                    .allowed_request_headers
                    .iter()
                    .any(|allowed| allowed == &normalized_name)
                {
                    return http_fetch_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        format!(
                            "palyra.http.fetch header '{normalized_name}' is not allowed by policy"
                        ),
                    );
                }
                headers.push((normalized_name, raw_value.clone()));
            }
            headers
        }
        Some(_) => {
            return http_fetch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.http.fetch headers must be an object map".to_owned(),
            );
        }
        None => Vec::new(),
    };

    let allow_redirects = payload
        .get("allow_redirects")
        .and_then(Value::as_bool)
        .unwrap_or(runtime_state.config.http_fetch.allow_redirects);
    let max_redirects = payload
        .get("max_redirects")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(runtime_state.config.http_fetch.max_redirects)
        .clamp(1, MAX_HTTP_FETCH_REDIRECTS);
    let allow_private_targets = runtime_state.config.http_fetch.allow_private_targets
        && payload.get("allow_private_targets").and_then(Value::as_bool).unwrap_or(true);
    let max_response_bytes = payload
        .get("max_response_bytes")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(runtime_state.config.http_fetch.max_response_bytes)
        .clamp(1, MAX_HTTP_FETCH_BODY_BYTES);
    let cache_enabled = payload
        .get("cache")
        .and_then(Value::as_bool)
        .unwrap_or(runtime_state.config.http_fetch.cache_enabled)
        && matches!(method.as_str(), "GET" | "HEAD");
    let cache_ttl_ms = payload
        .get("cache_ttl_ms")
        .and_then(Value::as_u64)
        .unwrap_or(runtime_state.config.http_fetch.cache_ttl_ms)
        .max(1);
    let allowed_content_types = match payload.get("allowed_content_types") {
        Some(Value::Array(values)) => {
            let mut parsed = Vec::new();
            for value in values {
                let Some(content_type) = value.as_str() else {
                    return http_fetch_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        "palyra.http.fetch allowed_content_types must be strings".to_owned(),
                    );
                };
                let normalized =
                    content_type.split(';').next().unwrap_or_default().trim().to_ascii_lowercase();
                if normalized.is_empty() {
                    continue;
                }
                if !runtime_state
                    .config
                    .http_fetch
                    .allowed_content_types
                    .iter()
                    .any(|allowed| allowed == &normalized)
                {
                    return http_fetch_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        format!(
                            "palyra.http.fetch content type '{normalized}' is not allowed by policy"
                        ),
                    );
                }
                if !parsed.iter().any(|existing| existing == &normalized) {
                    parsed.push(normalized);
                }
            }
            if parsed.is_empty() {
                runtime_state.config.http_fetch.allowed_content_types.clone()
            } else {
                parsed
            }
        }
        Some(_) => {
            return http_fetch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                "palyra.http.fetch allowed_content_types must be an array of strings".to_owned(),
            );
        }
        None => runtime_state.config.http_fetch.allowed_content_types.clone(),
    };

    let url = match Url::parse(url_raw.as_str()) {
        Ok(value) => value,
        Err(error) => {
            return http_fetch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!("palyra.http.fetch URL is invalid: {error}"),
            );
        }
    };
    if !matches!(url.scheme(), "http" | "https") {
        return http_fetch_tool_execution_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            format!("palyra.http.fetch blocked URL scheme '{}'", url.scheme()),
        );
    }
    if !url.username().is_empty() || url.password().is_some() {
        return http_fetch_tool_execution_outcome(
            proposal_id,
            input_json,
            false,
            b"{}".to_vec(),
            "palyra.http.fetch URL credentials are not allowed".to_owned(),
        );
    }

    let initial_resolved_addrs =
        match resolve_fetch_target_addresses(&url, allow_private_targets).await {
            Ok(value) => value,
            Err(error) => {
                return http_fetch_tool_execution_outcome(
                    proposal_id,
                    input_json,
                    false,
                    b"{}".to_vec(),
                    format!("palyra.http.fetch target blocked: {error}"),
                );
            }
        };

    let cache_policy = HttpFetchCachePolicy {
        allow_private_targets,
        allow_redirects,
        max_redirects,
        max_response_bytes,
        allowed_content_types: allowed_content_types.as_slice(),
    };
    let cache_key = http_fetch_cache_key(
        method.as_str(),
        url.as_str(),
        request_headers.as_slice(),
        body.as_str(),
        &cache_policy,
    );
    if cache_enabled {
        let now = current_unix_ms();
        if let Ok(mut cache) = runtime_state.http_fetch_cache.lock() {
            cache.retain(|_, entry| entry.expires_at_unix_ms > now);
            if let Some(cached) = cache.get(cache_key.as_str()) {
                return http_fetch_tool_execution_outcome(
                    proposal_id,
                    input_json,
                    true,
                    cached.output_json.clone(),
                    String::new(),
                );
            }
        }
    }

    let started_at = Instant::now();
    let mut current_url = url;
    let mut redirects_followed = 0_usize;
    let mut next_resolved_addrs = Some(initial_resolved_addrs);
    loop {
        let resolved_addrs = if let Some(resolved) = next_resolved_addrs.take() {
            resolved
        } else {
            match resolve_fetch_target_addresses(&current_url, allow_private_targets).await {
                Ok(value) => value,
                Err(error) => {
                    return http_fetch_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        format!("palyra.http.fetch target blocked: {error}"),
                    );
                }
            }
        };

        let host = current_url.host_str().unwrap_or_default().to_owned();
        let mut client_builder = reqwest::Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_millis(
                runtime_state.config.http_fetch.connect_timeout_ms,
            ))
            .timeout(Duration::from_millis(runtime_state.config.http_fetch.request_timeout_ms));
        if !host.is_empty() && host.parse::<IpAddr>().is_err() {
            for address in resolved_addrs {
                client_builder = client_builder.resolve(host.as_str(), address);
            }
        }
        let client = match client_builder.build() {
            Ok(value) => value,
            Err(error) => {
                return http_fetch_tool_execution_outcome(
                    proposal_id,
                    input_json,
                    false,
                    b"{}".to_vec(),
                    format!("palyra.http.fetch failed to build HTTP client: {error}"),
                );
            }
        };

        let method_value = match method.parse::<reqwest::Method>() {
            Ok(value) => value,
            Err(error) => {
                return http_fetch_tool_execution_outcome(
                    proposal_id,
                    input_json,
                    false,
                    b"{}".to_vec(),
                    format!("palyra.http.fetch invalid method: {error}"),
                );
            }
        };
        let mut request = client.request(method_value, current_url.clone());
        for (name, value) in request_headers.as_slice() {
            request = request.header(name, value);
        }
        if method == "POST" && !body.is_empty() {
            request = request.body(body.clone());
        }
        let mut response = match request.send().await {
            Ok(value) => value,
            Err(error) => {
                return http_fetch_tool_execution_outcome(
                    proposal_id,
                    input_json,
                    false,
                    b"{}".to_vec(),
                    format!("palyra.http.fetch request failed: {error}"),
                );
            }
        };

        if response.status().is_redirection() {
            if !allow_redirects {
                return http_fetch_tool_execution_outcome(
                    proposal_id,
                    input_json,
                    false,
                    b"{}".to_vec(),
                    "palyra.http.fetch redirect blocked by policy".to_owned(),
                );
            }
            if redirects_followed >= max_redirects {
                return http_fetch_tool_execution_outcome(
                    proposal_id,
                    input_json,
                    false,
                    b"{}".to_vec(),
                    format!("palyra.http.fetch redirect limit exceeded ({max_redirects})"),
                );
            }
            let Some(location) = response.headers().get(reqwest::header::LOCATION) else {
                return http_fetch_tool_execution_outcome(
                    proposal_id,
                    input_json,
                    false,
                    b"{}".to_vec(),
                    "palyra.http.fetch redirect response missing Location header".to_owned(),
                );
            };
            let location_str = match location.to_str() {
                Ok(value) => value,
                Err(_) => {
                    return http_fetch_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        "palyra.http.fetch redirect Location header is invalid UTF-8".to_owned(),
                    );
                }
            };
            current_url = match current_url.join(location_str) {
                Ok(value) => value,
                Err(error) => {
                    return http_fetch_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        format!("palyra.http.fetch redirect URL is invalid: {error}"),
                    );
                }
            };
            redirects_followed = redirects_followed.saturating_add(1);
            continue;
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or_default().trim().to_ascii_lowercase())
            .unwrap_or_default();
        if !content_type.is_empty()
            && !allowed_content_types.iter().any(|allowed| allowed == &content_type)
        {
            return http_fetch_tool_execution_outcome(
                proposal_id,
                input_json,
                false,
                b"{}".to_vec(),
                format!("palyra.http.fetch content type '{content_type}' is blocked by policy"),
            );
        }

        let mut body_bytes = Vec::new();
        if method != "HEAD" {
            loop {
                let chunk = match response.chunk().await {
                    Ok(value) => value,
                    Err(error) => {
                        return http_fetch_tool_execution_outcome(
                            proposal_id,
                            input_json,
                            false,
                            b"{}".to_vec(),
                            format!("palyra.http.fetch failed to stream response body: {error}"),
                        );
                    }
                };
                let Some(chunk) = chunk else {
                    break;
                };
                if body_bytes.len().saturating_add(chunk.len()) > max_response_bytes {
                    return http_fetch_tool_execution_outcome(
                        proposal_id,
                        input_json,
                        false,
                        b"{}".to_vec(),
                        format!(
                            "palyra.http.fetch response exceeds max_response_bytes ({max_response_bytes})"
                        ),
                    );
                }
                body_bytes.extend_from_slice(chunk.as_ref());
            }
        }

        let status_code = response.status().as_u16();
        let success = response.status().is_success();
        let output_json = json!({
            "url": current_url.as_str(),
            "method": method,
            "status_code": status_code,
            "redirects_followed": redirects_followed,
            "content_type": content_type,
            "body_bytes": body_bytes.len(),
            "body_text": String::from_utf8_lossy(body_bytes.as_slice()).to_string(),
            "latency_ms": started_at.elapsed().as_millis() as u64,
            "request_headers": redacted_http_headers(request_headers.as_slice()),
        });
        let serialized = serde_json::to_vec(&output_json).unwrap_or_else(|_| b"{}".to_vec());
        if cache_enabled && success {
            if let Ok(mut cache) = runtime_state.http_fetch_cache.lock() {
                let now = current_unix_ms();
                cache.retain(|_, entry| entry.expires_at_unix_ms > now);
                while cache.len() >= runtime_state.config.http_fetch.max_cache_entries {
                    let Some(first_key) = cache.keys().next().cloned() else {
                        break;
                    };
                    cache.remove(first_key.as_str());
                }
                cache.insert(
                    cache_key.clone(),
                    CachedHttpFetchEntry {
                        expires_at_unix_ms: now.saturating_add(cache_ttl_ms as i64),
                        output_json: serialized.clone(),
                    },
                );
            }
        }
        return http_fetch_tool_execution_outcome(
            proposal_id,
            input_json,
            success,
            serialized,
            if success {
                String::new()
            } else {
                format!("palyra.http.fetch returned HTTP {status_code}")
            },
        );
    }
}

pub(crate) struct HttpFetchCachePolicy<'a> {
    pub(crate) allow_private_targets: bool,
    pub(crate) allow_redirects: bool,
    pub(crate) max_redirects: usize,
    pub(crate) max_response_bytes: usize,
    pub(crate) allowed_content_types: &'a [String],
}

pub(crate) fn http_fetch_cache_key(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &str,
    policy: &HttpFetchCachePolicy<'_>,
) -> String {
    let mut normalized_headers =
        headers.iter().map(|(name, value)| format!("{name}:{value}")).collect::<Vec<_>>();
    normalized_headers.sort();
    let mut normalized_content_types = policy
        .allowed_content_types
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized_content_types.sort();
    normalized_content_types.dedup();
    let policy_fingerprint = format!(
        "allow_private_targets={};allow_redirects={};max_redirects={};max_response_bytes={};allowed_content_types={}",
        policy.allow_private_targets,
        policy.allow_redirects,
        policy.max_redirects,
        policy.max_response_bytes,
        normalized_content_types.join(",")
    );
    let mut key = format!(
        "{method}|{url}|{}|{}|{}",
        normalized_headers.join("&"),
        sha256_hex(body.as_bytes()),
        sha256_hex(policy_fingerprint.as_bytes())
    );
    if key.len() > MAX_HTTP_FETCH_CACHE_KEY_BYTES {
        key = format!("sha256:{}", sha256_hex(key.as_bytes()));
    }
    key
}

pub(crate) async fn resolve_fetch_target_addresses(
    url: &Url,
    allow_private_targets: bool,
) -> Result<Vec<SocketAddr>, String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!("blocked URL scheme '{}'", url.scheme()));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("URL credentials are not allowed".to_owned());
    }
    let host = url.host_str().ok_or_else(|| "URL host is required".to_owned())?;
    let port =
        url.port_or_known_default().ok_or_else(|| "URL port could not be resolved".to_owned())?;

    let addrs = if let Some(ip) = netguard::parse_host_ip_literal(host)? {
        vec![SocketAddr::new(ip, port)]
    } else {
        let resolved = tokio::net::lookup_host((host, port))
            .await
            .map_err(|error| format!("DNS resolution failed for host '{host}': {error}"))?;
        resolved.collect::<Vec<_>>()
    };
    if addrs.is_empty() {
        return Err(format!("DNS resolution returned no addresses for host '{host}'"));
    }
    validate_resolved_fetch_addresses(addrs.as_slice(), allow_private_targets)?;
    Ok(addrs)
}

pub(crate) fn validate_resolved_fetch_addresses(
    addrs: &[SocketAddr],
    allow_private_targets: bool,
) -> Result<(), String> {
    let ips = addrs.iter().map(|address| address.ip()).collect::<Vec<_>>();
    netguard::validate_resolved_ip_addrs(ips.as_slice(), allow_private_targets)
}

fn redacted_http_headers(headers: &[(String, String)]) -> Vec<serde_json::Value> {
    headers
        .iter()
        .map(|(name, value)| {
            let sensitive = name.contains("authorization")
                || name.contains("cookie")
                || name.contains("token")
                || name.contains("api-key")
                || name.contains("apikey");
            json!({
                "name": name,
                "value": if sensitive { "<redacted>" } else { value.as_str() }
            })
        })
        .collect()
}

fn http_fetch_tool_execution_outcome(
    proposal_id: &str,
    input_json: &[u8],
    success: bool,
    output_json: Vec<u8>,
    error: String,
) -> ToolExecutionOutcome {
    let executed_at_unix_ms = current_unix_ms();
    let mut hasher = Sha256::new();
    hasher.update(b"palyra.http.fetch.attestation.v1");
    hasher.update((proposal_id.len() as u64).to_be_bytes());
    hasher.update(proposal_id.as_bytes());
    hasher.update((input_json.len() as u64).to_be_bytes());
    hasher.update(input_json);
    hasher.update([u8::from(success)]);
    hasher.update((output_json.len() as u64).to_be_bytes());
    hasher.update(output_json.as_slice());
    hasher.update((error.len() as u64).to_be_bytes());
    hasher.update(error.as_bytes());
    hasher.update(executed_at_unix_ms.to_be_bytes());
    let execution_sha256 = hex::encode(hasher.finalize());

    ToolExecutionOutcome {
        success,
        output_json,
        error,
        attestation: ToolAttestation {
            attestation_id: Ulid::new().to_string(),
            execution_sha256,
            executed_at_unix_ms,
            timed_out: false,
            executor: "gateway_http_fetch".to_owned(),
            sandbox_enforcement: "ssrf_guard".to_owned(),
        },
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
