use super::*;

pub(crate) fn current_unix_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}

pub(crate) fn redact_session_id(session_id: &str) -> String {
    if session_id.len() <= 8 {
        return "***".to_owned();
    }
    let prefix = &session_id[..4];
    let suffix = &session_id[session_id.len().saturating_sub(4)..];
    format!("{prefix}***{suffix}")
}

pub(crate) const fn status_kind_name(kind: common_v1::stream_status::StatusKind) -> &'static str {
    match kind {
        common_v1::stream_status::StatusKind::Unspecified => "unspecified",
        common_v1::stream_status::StatusKind::Accepted => "accepted",
        common_v1::stream_status::StatusKind::InProgress => "in_progress",
        common_v1::stream_status::StatusKind::Done => "done",
        common_v1::stream_status::StatusKind::Failed => "failed",
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn unix_ms_now_for_status() -> Result<i64, Status> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Status::internal(format!("failed to read system clock: {error}")))?;
    Ok(i64::try_from(now.as_millis()).unwrap_or(i64::MAX))
}

#[allow(clippy::result_large_err)]
pub(crate) fn canonical_id(
    value: Option<common_v1::CanonicalId>,
    field_name: &'static str,
) -> Result<String, Status> {
    let id = value
        .and_then(|id| non_empty(id.ulid))
        .ok_or_else(|| Status::invalid_argument(format!("{field_name} is required")))?;
    validate_canonical_id(id.as_str())
        .map_err(|_| Status::invalid_argument(format!("{field_name} must be a canonical ULID")))?;
    Ok(id)
}

#[allow(clippy::result_large_err)]
pub(crate) fn optional_canonical_id(
    value: Option<common_v1::CanonicalId>,
    field_name: &'static str,
) -> Result<Option<String>, Status> {
    let Some(value) = value else {
        return Ok(None);
    };
    let id = non_empty(value.ulid)
        .ok_or_else(|| Status::invalid_argument(format!("{field_name} must be non-empty")))?;
    validate_canonical_id(id.as_str())
        .map_err(|_| Status::invalid_argument(format!("{field_name} must be a canonical ULID")))?;
    Ok(Some(id))
}

#[allow(clippy::result_large_err)]
pub(crate) fn normalize_agent_identifier(
    raw: &str,
    field_name: &'static str,
) -> Result<String, Status> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(Status::invalid_argument(format!("{field_name} cannot be empty")));
    }
    if value.len() > 64 {
        return Err(Status::invalid_argument(format!("{field_name} cannot exceed 64 bytes")));
    }
    for character in value.chars() {
        if !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')) {
            return Err(Status::invalid_argument(format!(
                "{field_name} contains unsupported character '{character}'"
            )));
        }
    }
    Ok(value.to_ascii_lowercase())
}

pub(crate) fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut diff = left.len() ^ right.len();
    for index in 0..max_len {
        let lhs = left.get(index).copied().unwrap_or_default();
        let rhs = right.get(index).copied().unwrap_or_default();
        diff |= usize::from(lhs ^ rhs);
    }
    diff == 0
}

pub(crate) fn extract_pairing_code_command(raw: &str) -> Option<String> {
    let mut parts = raw.split_whitespace();
    let command = parts.next()?.trim().to_ascii_lowercase();
    if command != "pair" {
        return None;
    }
    let code = parts.next()?.trim();
    if code.is_empty() {
        return None;
    }
    Some(code.to_owned())
}

pub(crate) fn non_empty(input: String) -> Option<String> {
    if input.trim().is_empty() {
        None
    } else {
        Some(input)
    }
}
