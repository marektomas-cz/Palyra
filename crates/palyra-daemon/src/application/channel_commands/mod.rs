use std::collections::BTreeMap;

use palyra_common::redaction::{redact_auth_error, redact_url_segments_in_text};
use palyra_connectors::{
    ChannelCommandArgumentKind, ChannelNativeCommandArgument,
    ChannelNativeCommandInvocationPayload, ChannelNativeCommandSpec,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

const CHANNEL_COMMAND_SCHEMA_VERSION: u32 = 1;
const COMMAND_PREFIXES: &[&str] = &["/palyra", "!palyra"];
const MAX_FREEFORM_ARG_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChannelCommandName {
    Status,
    Stop,
    Reset,
    Compact,
    Approve,
    Queue,
    Whoami,
}

impl ChannelCommandName {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Stop => "stop",
            Self::Reset => "reset",
            Self::Compact => "compact",
            Self::Approve => "approve",
            Self::Queue => "queue",
            Self::Whoami => "whoami",
        }
    }

    #[must_use]
    pub(crate) const fn policy_action(self) -> &'static str {
        match self {
            Self::Status => "channel.command.status",
            Self::Stop => "channel.command.stop",
            Self::Reset => "channel.command.reset",
            Self::Compact => "channel.command.compact",
            Self::Approve => "channel.command.approve",
            Self::Queue => "channel.command.queue",
            Self::Whoami => "channel.command.whoami",
        }
    }

    #[must_use]
    pub(crate) const fn side_effecting(self) -> bool {
        !matches!(self, Self::Status | Self::Queue | Self::Whoami)
    }

    #[must_use]
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "status" => Some(Self::Status),
            "stop" | "abort" | "cancel" => Some(Self::Stop),
            "reset" => Some(Self::Reset),
            "compact" | "summarize" => Some(Self::Compact),
            "approve" | "approval" => Some(Self::Approve),
            "queue" => Some(Self::Queue),
            "whoami" | "who" => Some(Self::Whoami),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChannelCommandSourceKind {
    Text,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelCommandArgumentSpec {
    pub(crate) name: String,
    pub(crate) kind: ChannelCommandArgumentKind,
    pub(crate) required: bool,
    pub(crate) enum_values: Vec<String>,
    pub(crate) description: String,
}

impl ChannelCommandArgumentSpec {
    #[must_use]
    fn native(&self) -> ChannelNativeCommandArgument {
        ChannelNativeCommandArgument {
            name: self.name.clone(),
            kind: self.kind,
            required: self.required,
            enum_values: self.enum_values.clone(),
            description: Some(self.description.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelCommandSpec {
    pub(crate) name: ChannelCommandName,
    pub(crate) description: String,
    pub(crate) policy_action: String,
    pub(crate) side_effecting: bool,
    pub(crate) arguments: Vec<ChannelCommandArgumentSpec>,
}

impl ChannelCommandSpec {
    #[must_use]
    pub(crate) fn native(&self) -> ChannelNativeCommandSpec {
        ChannelNativeCommandSpec {
            name: self.name.as_str().to_owned(),
            description: self.description.clone(),
            policy_action: self.policy_action.clone(),
            side_effecting: self.side_effecting,
            arguments: self.arguments.iter().map(ChannelCommandArgumentSpec::native).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelCommandInvocation {
    pub(crate) command: ChannelCommandName,
    pub(crate) source: ChannelCommandSourceKind,
    pub(crate) arguments: BTreeMap<String, ChannelCommandValue>,
    pub(crate) raw_text: Option<String>,
    pub(crate) native_interaction_id: Option<String>,
}

impl ChannelCommandInvocation {
    #[must_use]
    pub(crate) fn idempotency_key(&self, scope: &ChannelCommandScope) -> String {
        let payload = serde_json::to_vec(&json!({
            "schema_version": CHANNEL_COMMAND_SCHEMA_VERSION,
            "command": self.command.as_str(),
            "arguments": self.arguments,
            "scope": scope,
        }))
        .unwrap_or_default();
        format!("channel_command:{}:{}", self.command.as_str(), sha256_hex(payload.as_slice()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub(crate) enum ChannelCommandValue {
    String(String),
    Enum(String),
    Bool(bool),
    Int(i64),
    DurationMs(u64),
    IdRef(String),
    FreeformTail(String),
}

impl ChannelCommandValue {
    #[must_use]
    fn user_visible(&self) -> String {
        match self {
            Self::String(value)
            | Self::Enum(value)
            | Self::IdRef(value)
            | Self::FreeformTail(value) => redact_url_segments_in_text(&redact_auth_error(value)),
            Self::Bool(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::DurationMs(value) => format!("{value}ms"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelCommandScope {
    pub(crate) channel: String,
    pub(crate) conversation_id: Option<String>,
    pub(crate) thread_id: Option<String>,
    pub(crate) sender_identity: Option<String>,
    pub(crate) principal: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelCommandRuntimeView {
    pub(crate) queue_depth: usize,
    pub(crate) route_config_hash: String,
    pub(crate) command_catalog_hash: String,
    pub(crate) binding_id: Option<String>,
    pub(crate) binding_kind: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) pending_approval_count: usize,
    pub(crate) provider_wait_ms: Option<u64>,
    pub(crate) last_error: Option<String>,
    pub(crate) observed_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelCommandResponse {
    pub(crate) ok: bool,
    pub(crate) code: String,
    pub(crate) text: String,
    pub(crate) audit_json: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelCommandErrorEnvelope {
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) recovery_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChannelCommandParseOutcome {
    NotCommand,
    Parsed(ChannelCommandInvocation),
    Malformed(ChannelCommandErrorEnvelope),
}

#[derive(Debug, Clone)]
pub(crate) struct ChannelCommandRegistry {
    specs: BTreeMap<ChannelCommandName, ChannelCommandSpec>,
}

impl Default for ChannelCommandRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

impl ChannelCommandRegistry {
    #[must_use]
    pub(crate) fn builtin() -> Self {
        let specs = [
            command_spec(
                ChannelCommandName::Status,
                "Show scoped channel runtime status",
                &[
                    arg(
                        "session_id",
                        ChannelCommandArgumentKind::IdRef,
                        false,
                        &[],
                        "Session id to inspect",
                    ),
                    arg(
                        "run_id",
                        ChannelCommandArgumentKind::IdRef,
                        false,
                        &[],
                        "Run id to inspect",
                    ),
                ],
            ),
            command_spec(
                ChannelCommandName::Stop,
                "Stop the bound or specified run",
                &[
                    arg("run_id", ChannelCommandArgumentKind::IdRef, false, &[], "Run id to stop"),
                    arg(
                        "reason",
                        ChannelCommandArgumentKind::FreeformTail,
                        false,
                        &[],
                        "Stop reason",
                    ),
                ],
            ),
            command_spec(
                ChannelCommandName::Reset,
                "Detach/reset the scoped conversation binding",
                &[
                    arg(
                        "session_id",
                        ChannelCommandArgumentKind::IdRef,
                        false,
                        &[],
                        "Session id to reset",
                    ),
                    arg(
                        "confirm",
                        ChannelCommandArgumentKind::Bool,
                        false,
                        &[],
                        "Require explicit confirmation",
                    ),
                ],
            ),
            command_spec(
                ChannelCommandName::Compact,
                "Compact the scoped session context",
                &[
                    arg(
                        "session_id",
                        ChannelCommandArgumentKind::IdRef,
                        false,
                        &[],
                        "Session id to compact",
                    ),
                    arg(
                        "mode",
                        ChannelCommandArgumentKind::Enum,
                        false,
                        &["hybrid", "deterministic"],
                        "Compaction mode",
                    ),
                    arg(
                        "reason",
                        ChannelCommandArgumentKind::FreeformTail,
                        false,
                        &[],
                        "Compaction reason",
                    ),
                ],
            ),
            command_spec(
                ChannelCommandName::Approve,
                "Resolve a pending approval in scope",
                &[
                    arg("approval_id", ChannelCommandArgumentKind::IdRef, true, &[], "Approval id"),
                    arg(
                        "decision",
                        ChannelCommandArgumentKind::Enum,
                        true,
                        &["allow", "deny"],
                        "Approval decision",
                    ),
                    arg(
                        "reason",
                        ChannelCommandArgumentKind::FreeformTail,
                        false,
                        &[],
                        "Decision reason",
                    ),
                ],
            ),
            command_spec(
                ChannelCommandName::Queue,
                "Inspect or explain the scoped queue",
                &[
                    arg("session_id", ChannelCommandArgumentKind::IdRef, false, &[], "Session id"),
                    arg(
                        "action",
                        ChannelCommandArgumentKind::Enum,
                        false,
                        &["list", "explain"],
                        "Queue action",
                    ),
                ],
            ),
            command_spec(
                ChannelCommandName::Whoami,
                "Show the channel principal and binding identity",
                &[],
            ),
        ]
        .into_iter()
        .map(|spec| (spec.name, spec))
        .collect::<BTreeMap<_, _>>();
        Self { specs }
    }

    #[must_use]
    pub(crate) fn native_specs(&self) -> Vec<ChannelNativeCommandSpec> {
        self.specs.values().map(ChannelCommandSpec::native).collect()
    }

    #[must_use]
    pub(crate) fn catalog_hash(&self) -> String {
        let payload = serde_json::to_vec(&self.native_specs()).unwrap_or_default();
        sha256_hex(payload.as_slice())
    }

    #[must_use]
    pub(crate) fn parse_text(&self, text: &str) -> ChannelCommandParseOutcome {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return ChannelCommandParseOutcome::NotCommand;
        }
        let Some((_, body)) = COMMAND_PREFIXES.iter().find_map(|prefix| {
            trimmed
                .strip_prefix(prefix)
                .and_then(|rest| rest.strip_prefix(char::is_whitespace))
                .map(|rest| (*prefix, rest.trim()))
        }) else {
            return ChannelCommandParseOutcome::NotCommand;
        };
        let tokens = match split_command_tokens(body) {
            Ok(tokens) => tokens,
            Err(message) => return malformed("channel_command/malformed_text", message),
        };
        self.parse_tokens(tokens, Some(trimmed.to_owned()))
    }

    #[allow(dead_code)]
    #[must_use]
    pub(crate) fn parse_native(
        &self,
        payload: &ChannelNativeCommandInvocationPayload,
    ) -> ChannelCommandParseOutcome {
        if let Err(error) = payload.validate() {
            return malformed(
                "channel_command/malformed_native",
                format!("native command payload failed validation: {error}"),
            );
        }
        let command = match ChannelCommandName::parse(payload.command.as_str()) {
            Some(command) => command,
            None => {
                return malformed(
                    "channel_command/unknown",
                    format!("unknown channel command `{}`", payload.command),
                )
            }
        };
        let spec = match self.specs.get(&command) {
            Some(spec) => spec,
            None => {
                return malformed(
                    "channel_command/unregistered",
                    format!("channel command `{}` is not registered", command.as_str()),
                )
            }
        };
        let args = if payload.args_json.is_empty() {
            Value::Object(Map::new())
        } else {
            match serde_json::from_slice::<Value>(payload.args_json.as_slice()) {
                Ok(value) => value,
                Err(error) => {
                    return malformed(
                        "channel_command/malformed_native",
                        format!("native command args_json is not valid JSON: {error}"),
                    )
                }
            }
        };
        let Some(object) = args.as_object() else {
            return malformed(
                "channel_command/malformed_native",
                "native command args_json must be a JSON object",
            );
        };
        match coerce_named_arguments(spec, object) {
            Ok(arguments) => ChannelCommandParseOutcome::Parsed(ChannelCommandInvocation {
                command,
                source: ChannelCommandSourceKind::Native,
                arguments,
                raw_text: None,
                native_interaction_id: payload.native_interaction_id.clone(),
            }),
            Err(error) => malformed("channel_command/invalid_arguments", error),
        }
    }

    fn parse_tokens(
        &self,
        tokens: Vec<String>,
        raw_text: Option<String>,
    ) -> ChannelCommandParseOutcome {
        let Some(command_token) = tokens.first() else {
            return malformed("channel_command/missing_command", "missing command name");
        };
        let command = match ChannelCommandName::parse(command_token) {
            Some(command) => command,
            None => {
                return malformed(
                    "channel_command/unknown",
                    format!("unknown channel command `{command_token}`"),
                )
            }
        };
        let Some(spec) = self.specs.get(&command) else {
            return malformed(
                "channel_command/unregistered",
                format!("channel command `{}` is not registered", command.as_str()),
            );
        };
        match coerce_text_arguments(spec, &tokens[1..]) {
            Ok(arguments) => ChannelCommandParseOutcome::Parsed(ChannelCommandInvocation {
                command,
                source: ChannelCommandSourceKind::Text,
                arguments,
                raw_text,
                native_interaction_id: None,
            }),
            Err(error) => malformed("channel_command/invalid_arguments", error),
        }
    }
}

#[must_use]
pub(crate) fn build_channel_command_response(
    invocation: &ChannelCommandInvocation,
    scope: &ChannelCommandScope,
    runtime: &ChannelCommandRuntimeView,
) -> ChannelCommandResponse {
    let idempotency_key = invocation.idempotency_key(scope);
    let rendered_args = invocation
        .arguments
        .iter()
        .map(|(key, value)| format!("{key}={}", value.user_visible()))
        .collect::<Vec<_>>();
    let command = invocation.command.as_str();
    let code = match invocation.command {
        ChannelCommandName::Status => "channel_command/status",
        ChannelCommandName::Queue => "channel_command/queue",
        ChannelCommandName::Whoami => "channel_command/whoami",
        ChannelCommandName::Stop
        | ChannelCommandName::Reset
        | ChannelCommandName::Compact
        | ChannelCommandName::Approve => {
            if runtime.binding_id.is_none() && !has_explicit_target(invocation) {
                "channel_command/requires_binding"
            } else {
                "channel_command/accepted"
            }
        }
    };
    let ok = !code.ends_with("requires_binding");
    let text = match invocation.command {
        ChannelCommandName::Status => format!(
            "status: channel={} queue_depth={} pending_approvals={} binding={} session={} run={} last_error={}",
            scope.channel,
            runtime.queue_depth,
            runtime.pending_approval_count,
            runtime.binding_id.as_deref().unwrap_or("none"),
            runtime.session_id.as_deref().unwrap_or("none"),
            runtime.run_id.as_deref().unwrap_or("none"),
            runtime
                .last_error
                .as_deref()
                .map(redact_auth_error)
                .unwrap_or_else(|| "none".to_owned())
        ),
        ChannelCommandName::Queue => format!(
            "queue: channel={} route_queue_depth={} action={}",
            scope.channel,
            runtime.queue_depth,
            invocation
                .arguments
                .get("action")
                .map(ChannelCommandValue::user_visible)
                .unwrap_or_else(|| "list".to_owned())
        ),
        ChannelCommandName::Whoami => format!(
            "whoami: principal={} device_scope=channel sender={} conversation={} thread={} binding={}",
            redact_auth_error(scope.principal.as_str()),
            scope.sender_identity.as_deref().unwrap_or("unknown"),
            scope.conversation_id.as_deref().unwrap_or("default"),
            scope.thread_id.as_deref().unwrap_or("none"),
            runtime.binding_id.as_deref().unwrap_or("none")
        ),
        ChannelCommandName::Stop
        | ChannelCommandName::Reset
        | ChannelCommandName::Compact
        | ChannelCommandName::Approve
            if !ok =>
        {
            format!(
                "{command}: no active conversation binding or explicit target was found; send a scoped target argument or wait for this channel thread to bind to a session"
            )
        }
        _ => format!(
            "{command}: accepted in scoped command runtime; args={}",
            if rendered_args.is_empty() {
                "none".to_owned()
            } else {
                rendered_args.join(" ")
            }
        ),
    };
    ChannelCommandResponse {
        ok,
        code: code.to_owned(),
        text,
        audit_json: json!({
            "schema_version": CHANNEL_COMMAND_SCHEMA_VERSION,
            "event": "channel.command.evaluated",
            "command": command,
            "source": invocation.source,
            "policy_action": invocation.command.policy_action(),
            "side_effecting": invocation.command.side_effecting(),
            "idempotency_key": idempotency_key,
            "scope": scope,
            "runtime": runtime,
            "arguments": invocation.arguments,
            "outcome": {
                "ok": ok,
                "code": code,
            },
        }),
    }
}

#[must_use]
pub(crate) fn build_malformed_command_response(
    error: &ChannelCommandErrorEnvelope,
    scope: &ChannelCommandScope,
    runtime: &ChannelCommandRuntimeView,
) -> ChannelCommandResponse {
    let message = redact_auth_error(error.message.as_str());
    ChannelCommandResponse {
        ok: false,
        code: error.code.clone(),
        text: format!("command error: {message}; hint: {}", error.recovery_hint),
        audit_json: json!({
            "schema_version": CHANNEL_COMMAND_SCHEMA_VERSION,
            "event": "channel.command.rejected",
            "code": error.code,
            "message": message,
            "recovery_hint": error.recovery_hint,
            "scope": scope,
            "runtime": runtime,
        }),
    }
}

#[must_use]
pub(crate) fn build_policy_denied_command_response(
    invocation: &ChannelCommandInvocation,
    scope: &ChannelCommandScope,
    runtime: &ChannelCommandRuntimeView,
    reason: &str,
) -> ChannelCommandResponse {
    let message = redact_auth_error(reason);
    ChannelCommandResponse {
        ok: false,
        code: "channel_command/policy_denied".to_owned(),
        text: format!("{}: denied by policy; {}", invocation.command.as_str(), message),
        audit_json: json!({
            "schema_version": CHANNEL_COMMAND_SCHEMA_VERSION,
            "event": "channel.command.denied",
            "command": invocation.command.as_str(),
            "source": invocation.source,
            "policy_action": invocation.command.policy_action(),
            "reason": message,
            "scope": scope,
            "runtime": runtime,
        }),
    }
}

fn command_spec(
    name: ChannelCommandName,
    description: &str,
    arguments: &[ChannelCommandArgumentSpec],
) -> ChannelCommandSpec {
    ChannelCommandSpec {
        name,
        description: description.to_owned(),
        policy_action: name.policy_action().to_owned(),
        side_effecting: name.side_effecting(),
        arguments: arguments.to_vec(),
    }
}

fn arg(
    name: &str,
    kind: ChannelCommandArgumentKind,
    required: bool,
    enum_values: &[&str],
    description: &str,
) -> ChannelCommandArgumentSpec {
    ChannelCommandArgumentSpec {
        name: name.to_owned(),
        kind,
        required,
        enum_values: enum_values.iter().map(|value| (*value).to_owned()).collect(),
        description: description.to_owned(),
    }
}

fn coerce_text_arguments(
    spec: &ChannelCommandSpec,
    tokens: &[String],
) -> Result<BTreeMap<String, ChannelCommandValue>, String> {
    let mut named = Map::new();
    let mut positional = Vec::new();
    for token in tokens {
        if let Some((name, value)) = token.split_once('=') {
            named.insert(name.trim().to_owned(), Value::String(value.trim().to_owned()));
        } else {
            positional.push(token.clone());
        }
    }

    let mut output = BTreeMap::new();
    let mut position = 0usize;
    for argument in &spec.arguments {
        if argument.kind == ChannelCommandArgumentKind::FreeformTail {
            let tail = positional[position..].join(" ");
            if !tail.is_empty() {
                output.insert(argument.name.clone(), coerce_text_value(argument, tail.as_str())?);
            }
            continue;
        }
        if let Some(value) = named.get(argument.name.as_str()) {
            output.insert(argument.name.clone(), coerce_json_value(argument, value)?);
            continue;
        }
        if let Some(value) = positional.get(position) {
            output.insert(argument.name.clone(), coerce_text_value(argument, value)?);
            position = position.saturating_add(1);
        } else if argument.required {
            return Err(format!("missing required argument `{}`", argument.name));
        }
    }
    Ok(output)
}

fn coerce_named_arguments(
    spec: &ChannelCommandSpec,
    object: &Map<String, Value>,
) -> Result<BTreeMap<String, ChannelCommandValue>, String> {
    let mut output = BTreeMap::new();
    for argument in &spec.arguments {
        match object.get(argument.name.as_str()) {
            Some(value) => {
                output.insert(argument.name.clone(), coerce_json_value(argument, value)?);
            }
            None if argument.required => {
                return Err(format!("missing required argument `{}`", argument.name));
            }
            None => {}
        }
    }
    Ok(output)
}

fn coerce_json_value(
    spec: &ChannelCommandArgumentSpec,
    value: &Value,
) -> Result<ChannelCommandValue, String> {
    match spec.kind {
        ChannelCommandArgumentKind::Bool => value
            .as_bool()
            .map(ChannelCommandValue::Bool)
            .ok_or_else(|| format!("argument `{}` must be boolean", spec.name)),
        ChannelCommandArgumentKind::Int => value
            .as_i64()
            .map(ChannelCommandValue::Int)
            .ok_or_else(|| format!("argument `{}` must be integer", spec.name)),
        _ => {
            let Some(text) = value.as_str() else {
                return Err(format!("argument `{}` must be string-compatible", spec.name));
            };
            coerce_text_value(spec, text)
        }
    }
}

fn coerce_text_value(
    spec: &ChannelCommandArgumentSpec,
    value: &str,
) -> Result<ChannelCommandValue, String> {
    let trimmed = value.trim();
    match spec.kind {
        ChannelCommandArgumentKind::String => Ok(ChannelCommandValue::String(trimmed.to_owned())),
        ChannelCommandArgumentKind::Enum => {
            let normalized = trimmed.to_ascii_lowercase();
            if spec.enum_values.iter().any(|candidate| candidate == &normalized) {
                Ok(ChannelCommandValue::Enum(normalized))
            } else {
                Err(format!(
                    "argument `{}` must be one of {}",
                    spec.name,
                    spec.enum_values.join("|")
                ))
            }
        }
        ChannelCommandArgumentKind::Bool => parse_bool(trimmed)
            .map(ChannelCommandValue::Bool)
            .ok_or_else(|| format!("argument `{}` must be true or false", spec.name)),
        ChannelCommandArgumentKind::Int => trimmed
            .parse::<i64>()
            .map(ChannelCommandValue::Int)
            .map_err(|_| format!("argument `{}` must be integer", spec.name)),
        ChannelCommandArgumentKind::Duration => parse_duration_ms(trimmed)
            .map(ChannelCommandValue::DurationMs)
            .ok_or_else(|| format!("argument `{}` must be a duration like 30s or 5m", spec.name)),
        ChannelCommandArgumentKind::IdRef => {
            if trimmed.is_empty() {
                Err(format!("argument `{}` must not be empty", spec.name))
            } else {
                Ok(ChannelCommandValue::IdRef(trimmed.to_owned()))
            }
        }
        ChannelCommandArgumentKind::FreeformTail => {
            if trimmed.len() > MAX_FREEFORM_ARG_BYTES {
                Err(format!("argument `{}` exceeds freeform size limit", spec.name))
            } else {
                Ok(ChannelCommandValue::FreeformTail(trimmed.to_owned()))
            }
        }
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "y" | "1" | "on" => Some(true),
        "false" | "no" | "n" | "0" | "off" => Some(false),
        _ => None,
    }
}

fn parse_duration_ms(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    let suffix_start = trimmed.find(|ch: char| !ch.is_ascii_digit()).unwrap_or(trimmed.len());
    let (digits, suffix) = trimmed.split_at(suffix_start);
    let amount = digits.parse::<u64>().ok()?;
    match suffix {
        "" | "ms" => Some(amount),
        "s" => amount.checked_mul(1_000),
        "m" => amount.checked_mul(60_000),
        "h" => amount.checked_mul(60 * 60_000),
        _ => None,
    }
}

fn split_command_tokens(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in input.chars() {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            ch if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if quote.is_some() {
        return Err("unterminated quoted argument".to_owned());
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

fn has_explicit_target(invocation: &ChannelCommandInvocation) -> bool {
    invocation.arguments.contains_key("run_id")
        || invocation.arguments.contains_key("session_id")
        || invocation.arguments.contains_key("approval_id")
}

fn malformed(code: impl Into<String>, message: impl Into<String>) -> ChannelCommandParseOutcome {
    ChannelCommandParseOutcome::Malformed(ChannelCommandErrorEnvelope {
        code: code.into(),
        message: message.into(),
        recovery_hint: "refresh the command schema and retry with supported arguments".to_owned(),
    })
}

fn sha256_hex(payload: &[u8]) -> String {
    let digest = Sha256::digest(payload);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        build_channel_command_response, ChannelCommandName, ChannelCommandParseOutcome,
        ChannelCommandRegistry, ChannelCommandRuntimeView, ChannelCommandScope,
    };

    #[test]
    fn text_parser_validates_typed_arguments() {
        let registry = ChannelCommandRegistry::builtin();
        let parsed =
            registry.parse_text("/palyra approve approval_id=01ARZ3 decision=allow reason ok");
        let ChannelCommandParseOutcome::Parsed(invocation) = parsed else {
            panic!("approve command should parse");
        };

        assert_eq!(invocation.command, ChannelCommandName::Approve);
        assert!(invocation.arguments.contains_key("approval_id"));
        assert!(invocation.arguments.contains_key("decision"));
        assert!(invocation.arguments.contains_key("reason"));
    }

    #[test]
    fn command_catalog_hash_is_deterministic() {
        let first = ChannelCommandRegistry::builtin().catalog_hash();
        let second = ChannelCommandRegistry::builtin().catalog_hash();

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn malformed_command_reports_stable_error() {
        let registry = ChannelCommandRegistry::builtin();
        let parsed = registry.parse_text("/palyra missing");

        let ChannelCommandParseOutcome::Malformed(error) = parsed else {
            panic!("unknown command should be malformed");
        };
        assert_eq!(error.code, "channel_command/unknown");
    }

    #[test]
    fn side_effecting_command_without_binding_fails_closed() {
        let registry = ChannelCommandRegistry::builtin();
        let ChannelCommandParseOutcome::Parsed(invocation) = registry.parse_text("/palyra stop")
        else {
            panic!("stop command should parse");
        };
        let response = build_channel_command_response(
            &invocation,
            &ChannelCommandScope {
                channel: "discord:default".to_owned(),
                conversation_id: Some("c1".to_owned()),
                thread_id: None,
                sender_identity: Some("u1".to_owned()),
                principal: "channel:discord:default".to_owned(),
            },
            &ChannelCommandRuntimeView {
                queue_depth: 0,
                route_config_hash: "0".repeat(64),
                command_catalog_hash: "0".repeat(64),
                binding_id: None,
                binding_kind: None,
                session_id: None,
                run_id: None,
                pending_approval_count: 0,
                provider_wait_ms: None,
                last_error: None,
                observed_at_unix_ms: 1,
            },
        );

        assert!(!response.ok);
        assert_eq!(response.code, "channel_command/requires_binding");
    }
}
