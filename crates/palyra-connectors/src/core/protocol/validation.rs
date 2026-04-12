use serde_json::Value;
use thiserror::Error;

pub(super) const MAX_CONNECTOR_ID_BYTES: usize = 128;
pub(super) const MAX_CONNECTOR_PRINCIPAL_BYTES: usize = 128;
pub(super) const MAX_ENVELOPE_ID_BYTES: usize = 128;
pub(super) const MAX_CONVERSATION_ID_BYTES: usize = 256;
pub(super) const MAX_IDENTITY_BYTES: usize = 256;
pub(super) const MAX_MESSAGE_BYTES: usize = 128 * 1024;
pub(super) const MAX_ATTACHMENTS_PER_MESSAGE: usize = 32;
pub(super) const MAX_ATTACHMENT_REF_BYTES: usize = 1_024;
pub(super) const MAX_ATTACHMENT_FILENAME_BYTES: usize = 512;
pub(super) const MAX_ATTACHMENT_CONTENT_TYPE_BYTES: usize = 256;
pub(super) const MAX_ATTACHMENT_ID_BYTES: usize = 128;
pub(super) const MAX_ATTACHMENT_HASH_BYTES: usize = 128;
pub(super) const MAX_ATTACHMENT_ORIGIN_BYTES: usize = 128;
pub(super) const MAX_ATTACHMENT_POLICY_CONTEXT_BYTES: usize = 512;
pub(super) const MAX_ATTACHMENT_INLINE_BASE64_BYTES: usize = 2 * 1024 * 1024;
pub(super) const MAX_STRUCTURED_OUTPUT_BYTES: usize = 128 * 1024;
pub(super) const MAX_A2UI_SURFACE_BYTES: usize = 128;
pub(super) const MAX_A2UI_PATCH_BYTES: usize = 128 * 1024;
pub(super) const MAX_POLICY_ACTION_BYTES: usize = 128;
pub(super) const MAX_AUDIT_EVENT_TYPE_BYTES: usize = 128;
pub(super) const MAX_PERMISSION_LABEL_BYTES: usize = 128;
pub(super) const MAX_CURSOR_ID_BYTES: usize = 128;
pub(super) const MAX_SEARCH_QUERY_BYTES: usize = 1_024;
pub(super) const MAX_MESSAGE_LINK_BYTES: usize = 2_048;
pub(super) const MAX_EMOJI_BYTES: usize = 128;
pub(super) const MAX_OPERATION_REASON_BYTES: usize = 512;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("invalid field '{field}': {reason}")]
    InvalidField { field: &'static str, reason: &'static str },
}

pub(super) fn validate_non_empty_identifier(
    raw: &str,
    field: &'static str,
    max_bytes: usize,
) -> Result<(), ProtocolError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ProtocolError::InvalidField { field, reason: "cannot be empty" });
    }
    if trimmed.len() > max_bytes {
        return Err(ProtocolError::InvalidField { field, reason: "value exceeds size limit" });
    }
    Ok(())
}

pub(super) fn validate_message_body(
    raw: &str,
    max_bytes: usize,
    field: &'static str,
) -> Result<(), ProtocolError> {
    if raw.trim().is_empty() {
        return Err(ProtocolError::InvalidField { field, reason: "cannot be empty" });
    }
    let max_bytes = max_bytes.clamp(1, MAX_MESSAGE_BYTES);
    if raw.len() > max_bytes {
        return Err(ProtocolError::InvalidField {
            field,
            reason: "message body exceeds size limit",
        });
    }
    Ok(())
}

pub(super) fn validate_host_pattern(raw: &str) -> Result<(), ProtocolError> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Err(ProtocolError::InvalidField {
            field: "egress_allowlist",
            reason: "host pattern cannot be empty",
        });
    }
    let stripped = trimmed.strip_prefix("*.").unwrap_or(trimmed.as_str());
    if stripped.is_empty()
        || !stripped.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
    {
        return Err(ProtocolError::InvalidField {
            field: "egress_allowlist",
            reason: "host pattern contains unsupported characters",
        });
    }
    if stripped.starts_with('.') || stripped.ends_with('.') || stripped.contains("..") {
        return Err(ProtocolError::InvalidField {
            field: "egress_allowlist",
            reason: "host pattern is malformed",
        });
    }
    Ok(())
}

pub(super) fn validate_optional_field(
    raw: Option<&str>,
    field: &'static str,
    max_bytes: usize,
) -> Result<(), ProtocolError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    if value.len() > max_bytes {
        return Err(ProtocolError::InvalidField { field, reason: "value exceeds size limit" });
    }
    Ok(())
}

pub(super) fn validate_json_bytes(
    raw: &[u8],
    field: &'static str,
    max_bytes: usize,
) -> Result<(), ProtocolError> {
    if raw.is_empty() {
        return Err(ProtocolError::InvalidField { field, reason: "cannot be empty" });
    }
    if raw.len() > max_bytes {
        return Err(ProtocolError::InvalidField { field, reason: "value exceeds size limit" });
    }
    serde_json::from_slice::<Value>(raw)
        .map_err(|_| ProtocolError::InvalidField { field, reason: "value is not valid JSON" })?;
    Ok(())
}

pub(super) fn validate_permission_labels(
    values: &[String],
    field: &'static str,
) -> Result<(), ProtocolError> {
    for value in values {
        validate_non_empty_identifier(value.as_str(), field, MAX_PERMISSION_LABEL_BYTES)?;
    }
    Ok(())
}
