use std::net::SocketAddr;

use palyra_common::{netguard, secret_refs::SecretRef};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialBindingPlan {
    pub header_name: String,
    pub secret_ref: SecretRef,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressProxyRequest<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub allow_private_targets: bool,
    pub allowed_hosts: &'a [String],
    pub allowed_dns_suffixes: &'a [String],
    pub max_response_bytes: usize,
    pub credential_bindings: &'a [CredentialBindingPlan],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EgressPolicyVerdict {
    pub allowed: bool,
    pub reason_code: String,
    pub message: String,
    pub request_fingerprint_sha256: String,
    pub host: String,
    #[serde(skip_serializing, skip_deserializing, default)]
    pub resolved_addresses: Vec<SocketAddr>,
    pub resolved_socket_addrs: Vec<String>,
    pub injected_credential_headers: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EgressPolicyError {
    #[error("unsupported URL scheme '{0}'")]
    UnsupportedScheme(String),
    #[error("URL credentials are not allowed")]
    CredentialsForbidden,
    #[error("URL host is required")]
    MissingHost,
    #[error("URL port could not be resolved")]
    MissingPort,
    #[error("DNS resolution returned no addresses for host '{host}'")]
    EmptyResolution { host: String },
    #[error("DNS resolution failed for host '{host}': {message}")]
    DnsResolution { host: String, message: String },
    #[error("host '{host}' is not present in the egress allowlist")]
    HostNotAllowlisted { host: String },
    #[error("target resolves to private/local address and is blocked by policy")]
    PrivateTargetBlocked,
    #[error("response budget must be greater than zero")]
    InvalidResponseBudget,
    #[error("credential binding '{header_name}' uses a disallowed header name")]
    InvalidCredentialHeader { header_name: String },
}

#[derive(Debug, Default)]
pub struct EgressProxyPolicyService;

impl EgressProxyPolicyService {
    pub fn evaluate_request(
        &self,
        request: &EgressProxyRequest<'_>,
    ) -> Result<EgressPolicyVerdict, EgressPolicyError> {
        if request.max_response_bytes == 0 {
            return Err(EgressPolicyError::InvalidResponseBudget);
        }

        let url = Url::parse(request.url)
            .map_err(|error| EgressPolicyError::DnsResolution {
                host: request.url.to_owned(),
                message: error.to_string(),
            })?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(EgressPolicyError::UnsupportedScheme(url.scheme().to_owned()));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(EgressPolicyError::CredentialsForbidden);
        }

        let host = url.host_str().ok_or(EgressPolicyError::MissingHost)?.to_ascii_lowercase();
        let port = url
            .port_or_known_default()
            .ok_or(EgressPolicyError::MissingPort)?;
        validate_host_allowlist(
            host.as_str(),
            request.allowed_hosts,
            request.allowed_dns_suffixes,
        )?;
        let resolved = resolve_socket_addrs(host.as_str(), port)?;
        validate_resolved_addrs(resolved.as_slice(), request.allow_private_targets)?;
        validate_credential_bindings(request.credential_bindings)?;

        Ok(EgressPolicyVerdict {
            allowed: true,
            reason_code: "egress.allowed".to_owned(),
            message: format!(
                "egress allowed for host '{host}' with {} resolved address(es)",
                resolved.len()
            ),
            request_fingerprint_sha256: request_fingerprint(request),
            host,
            resolved_addresses: resolved.clone(),
            resolved_socket_addrs: resolved.iter().map(ToString::to_string).collect(),
            injected_credential_headers: request
                .credential_bindings
                .iter()
                .map(|binding| binding.header_name.to_ascii_lowercase())
                .collect(),
        })
    }
}

pub fn validate_resolved_addrs(
    addrs: &[SocketAddr],
    allow_private_targets: bool,
) -> Result<(), EgressPolicyError> {
    let ips = addrs.iter().map(|address| address.ip()).collect::<Vec<_>>();
    netguard::validate_resolved_ip_addrs(ips.as_slice(), allow_private_targets)
        .map_err(|_| EgressPolicyError::PrivateTargetBlocked)
}

fn validate_host_allowlist(
    host: &str,
    allowed_hosts: &[String],
    allowed_dns_suffixes: &[String],
) -> Result<(), EgressPolicyError> {
    if allowed_hosts.is_empty() && allowed_dns_suffixes.is_empty() {
        return Ok(());
    }
    let host_allowed = allowed_hosts.iter().any(|candidate| candidate.eq_ignore_ascii_case(host));
    let suffix_allowed = allowed_dns_suffixes.iter().any(|suffix| {
        let normalized = suffix.trim().trim_start_matches('.').to_ascii_lowercase();
        !normalized.is_empty()
            && (host == normalized
                || host
                    .strip_suffix(normalized.as_str())
                    .is_some_and(|prefix| prefix.ends_with('.')))
    });
    if host_allowed || suffix_allowed {
        return Ok(());
    }
    Err(EgressPolicyError::HostNotAllowlisted { host: host.to_owned() })
}

fn resolve_socket_addrs(host: &str, port: u16) -> Result<Vec<SocketAddr>, EgressPolicyError> {
    let addrs = if let Some(ip) = netguard::parse_host_ip_literal(host).map_err(|error| {
        EgressPolicyError::DnsResolution { host: host.to_owned(), message: error }
    })? {
        vec![SocketAddr::new(ip, port)]
    } else {
        std::net::ToSocketAddrs::to_socket_addrs(&(host, port))
            .map_err(|error| EgressPolicyError::DnsResolution {
                host: host.to_owned(),
                message: error.to_string(),
            })?
            .collect::<Vec<_>>()
    };
    if addrs.is_empty() {
        return Err(EgressPolicyError::EmptyResolution { host: host.to_owned() });
    }
    Ok(addrs)
}

fn validate_credential_bindings(
    bindings: &[CredentialBindingPlan],
) -> Result<(), EgressPolicyError> {
    for binding in bindings {
        let normalized = binding.header_name.trim().to_ascii_lowercase();
        if normalized.is_empty()
            || !(normalized.starts_with("authorization")
                || normalized.starts_with("x-")
                || normalized.ends_with("-token")
                || normalized.ends_with("-api-key")
                || normalized == "cookie")
        {
            return Err(EgressPolicyError::InvalidCredentialHeader {
                header_name: binding.header_name.clone(),
            });
        }
    }
    Ok(())
}

fn request_fingerprint(request: &EgressProxyRequest<'_>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"palyra.egress.proxy.v1");
    hasher.update(request.method.as_bytes());
    hasher.update(request.url.as_bytes());
    hasher.update([u8::from(request.allow_private_targets)]);
    hasher.update(request.max_response_bytes.to_be_bytes());
    for host in request.allowed_hosts {
        hasher.update(host.as_bytes());
        hasher.update([0]);
    }
    for suffix in request.allowed_dns_suffixes {
        hasher.update(suffix.as_bytes());
        hasher.update([1]);
    }
    for binding in request.credential_bindings {
        hasher.update(binding.header_name.as_bytes());
        hasher.update(binding.secret_ref.fingerprint().as_bytes());
        hasher.update([u8::from(binding.required)]);
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use palyra_common::secret_refs::SecretRef;

    use super::{
        validate_resolved_addrs, CredentialBindingPlan, EgressPolicyError, EgressProxyPolicyService,
        EgressProxyRequest,
    };

    fn binding(header_name: &str) -> CredentialBindingPlan {
        CredentialBindingPlan {
            header_name: header_name.to_owned(),
            secret_ref: SecretRef::from_legacy_vault_ref("global/example"),
            required: true,
        }
    }

    #[test]
    fn egress_proxy_allows_explicit_allowlisted_host() {
        let service = EgressProxyPolicyService;
        let request = EgressProxyRequest {
            method: "GET",
            url: "https://93.184.216.34/path",
            allow_private_targets: false,
            allowed_hosts: &["93.184.216.34".to_owned()],
            allowed_dns_suffixes: &[],
            max_response_bytes: 1024,
            credential_bindings: &[binding("authorization")],
        };
        let verdict = service.evaluate_request(&request).expect("request should pass");
        assert!(verdict.allowed);
        assert_eq!(verdict.reason_code, "egress.allowed");
        assert_eq!(verdict.host, "93.184.216.34");
    }

    #[test]
    fn resolved_private_targets_are_blocked_fail_closed() {
        let addrs = vec![
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                443,
            ),
        ];
        let error = validate_resolved_addrs(addrs.as_slice(), false)
            .expect_err("loopback target should be rejected");
        assert_eq!(error, EgressPolicyError::PrivateTargetBlocked);
    }

    #[test]
    fn mixed_public_and_private_resolution_is_treated_like_dns_rebinding_and_rejected() {
        let addrs = vec![
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(93, 184, 216, 34)),
                443,
            ),
            std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 7)),
                443,
            ),
        ];
        let error = validate_resolved_addrs(addrs.as_slice(), false)
            .expect_err("mixed private/public answers should fail closed");
        assert_eq!(error, EgressPolicyError::PrivateTargetBlocked);
    }
}
