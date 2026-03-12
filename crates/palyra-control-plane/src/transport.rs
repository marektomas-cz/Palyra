pub(crate) fn fallback_error_message(status: u16, body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return format!("request failed with HTTP {status}");
    }
    if trimmed.len() > 256 {
        return format!("request failed with HTTP {status}");
    }
    trimmed.to_owned()
}

pub(crate) fn urlencoding(raw: &str) -> String {
    raw.bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![char::from(byte)]
            }
            _ => format!("%{byte:02X}").chars().collect::<Vec<_>>(),
        })
        .collect()
}
