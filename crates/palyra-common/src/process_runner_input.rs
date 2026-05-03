use serde::Deserialize;
use thiserror::Error;

/// Canonical input payload for `palyra.process.run`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessRunnerToolInput {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub requested_egress_hosts: Vec<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub background: bool,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProcessRunnerToolInputParseError {
    #[error("{0}")]
    InvalidJson(String),
}

/// Parse the raw JSON payload for `palyra.process.run`.
pub fn parse_process_runner_tool_input(
    input_json: &[u8],
) -> Result<ProcessRunnerToolInput, ProcessRunnerToolInputParseError> {
    let mut input = serde_json::from_slice::<ProcessRunnerToolInput>(input_json)
        .map_err(|error| ProcessRunnerToolInputParseError::InvalidJson(error.to_string()))?;
    normalize_repeated_command_argument(&mut input);
    Ok(input)
}

fn normalize_repeated_command_argument(input: &mut ProcessRunnerToolInput) {
    if input.args.len() != 1 {
        return;
    }
    let command = input.command.trim();
    if command.is_empty() {
        return;
    }
    let argument = input.args[0].trim_start();
    let Some((first_token, rest)) = argument.split_once(char::is_whitespace) else {
        if argument.eq_ignore_ascii_case(command) {
            input.args.clear();
        }
        return;
    };
    if !first_token.eq_ignore_ascii_case(command) {
        return;
    }
    input.args =
        rest.split_whitespace().filter(|arg| !arg.is_empty()).map(ToOwned::to_owned).collect();
}

#[cfg(test)]
mod tests {
    use super::{parse_process_runner_tool_input, ProcessRunnerToolInputParseError};

    #[test]
    fn parse_process_runner_tool_input_accepts_valid_payload() {
        let input =
            br#"{"command":"uname","args":["-a"],"cwd":"workspace","requested_egress_hosts":["api.example.com"]}"#;
        let parsed = parse_process_runner_tool_input(input)
            .expect("valid process-runner payload should parse");
        assert_eq!(parsed.command, "uname");
        assert_eq!(parsed.args, vec!["-a"]);
        assert_eq!(parsed.cwd.as_deref(), Some("workspace"));
        assert_eq!(parsed.requested_egress_hosts, vec!["api.example.com"]);
        assert_eq!(parsed.timeout_ms, None);
        assert!(!parsed.background);
    }

    #[test]
    fn parse_process_runner_tool_input_accepts_background_flag() {
        let input =
            br#"{"command":"python3","args":["-m","http.server","8765"],"background":true}"#;
        let parsed = parse_process_runner_tool_input(input)
            .expect("valid background process-runner payload should parse");

        assert_eq!(parsed.command, "python3");
        assert!(parsed.background);
    }

    #[test]
    fn parse_process_runner_tool_input_normalizes_repeated_command_in_single_arg() {
        let input = br#"{"command":"echo","args":["echo PALYRA_TERMINAL_OK"]}"#;
        let parsed = parse_process_runner_tool_input(input)
            .expect("valid process-runner payload should parse");

        assert_eq!(parsed.command, "echo");
        assert_eq!(parsed.args, vec!["PALYRA_TERMINAL_OK"]);
    }

    #[test]
    fn parse_process_runner_tool_input_keeps_literal_command_arg_when_split_already() {
        let input = br#"{"command":"echo","args":["echo","PALYRA_TERMINAL_OK"]}"#;
        let parsed = parse_process_runner_tool_input(input)
            .expect("valid process-runner payload should parse");

        assert_eq!(parsed.args, vec!["echo", "PALYRA_TERMINAL_OK"]);
    }

    #[test]
    fn parse_process_runner_tool_input_rejects_unknown_fields() {
        let input = br#"{"command":"uname","unknown":true}"#;
        let error =
            parse_process_runner_tool_input(input).expect_err("unknown fields must fail parsing");
        assert!(
            matches!(error, ProcessRunnerToolInputParseError::InvalidJson(_)),
            "unknown fields should fail as JSON schema violation"
        );
    }

    #[test]
    fn parse_process_runner_tool_input_rejects_invalid_json() {
        let input = br#"{"command":"uname","#;
        let error = parse_process_runner_tool_input(input)
            .expect_err("invalid JSON payload must fail parsing");
        assert!(
            matches!(error, ProcessRunnerToolInputParseError::InvalidJson(_)),
            "invalid JSON should map to parser error"
        );
    }
}
