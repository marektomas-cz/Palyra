use serde::{Deserialize, Serialize};
use serde_json::Value;

const PROVIDER_STREAM_EVENT_TOKEN_CHUNK_SIZE: usize =
    crate::orchestrator::MAX_MODEL_TOKENS_PER_EVENT;
pub(super) const MAX_PROVIDER_TURN_TEXT_BYTES: usize = 64 * 1024;
const PROVIDER_OUTPUT_TRUNCATED_MARKER: &str = "\n\n[provider output truncated]";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderImageInput {
    pub mime_type: String,
    pub bytes_base64: String,
    pub file_name: Option<String>,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
    pub artifact_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMessageRole {
    System,
    Developer,
    User,
    Assistant,
    Tool,
}

impl ProviderMessageRole {
    #[must_use]
    pub const fn as_openai_role(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Developer => "developer",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }

    #[must_use]
    pub const fn as_anthropic_role(self) -> &'static str {
        match self {
            Self::Assistant => "assistant",
            Self::System | Self::Developer | Self::User | Self::Tool => "user",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderMessageContentPart {
    Text { text: String },
    Image { image: ProviderImageInput },
}

impl ProviderMessageContentPart {
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderMessage {
    pub role: ProviderMessageRole,
    pub content: Vec<ProviderMessageContentPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ProviderMessageToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderMessageToolCall {
    pub proposal_id: String,
    pub tool_name: String,
    pub input_json: Value,
}

impl ProviderMessage {
    #[must_use]
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::User,
            content: vec![ProviderMessageContentPart::text(text)],
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    #[must_use]
    pub fn assistant_from_output(output: &ProviderTurnOutput) -> Self {
        let mut content = Vec::new();
        let mut tool_calls = Vec::new();
        for part in &output.content_parts {
            match part {
                ProviderOutputContentPart::Text { text } => {
                    if !text.is_empty() {
                        content.push(ProviderMessageContentPart::text(text.clone()));
                    }
                }
                ProviderOutputContentPart::ToolCall { proposal_id, tool_name, input_json } => {
                    tool_calls.push(ProviderMessageToolCall {
                        proposal_id: proposal_id.clone(),
                        tool_name: tool_name.clone(),
                        input_json: input_json.clone(),
                    });
                }
            }
        }
        if content.is_empty() && tool_calls.is_empty() && !output.full_text.is_empty() {
            content.push(ProviderMessageContentPart::text(output.full_text.clone()));
        }
        Self {
            role: ProviderMessageRole::Assistant,
            content,
            name: None,
            tool_call_id: None,
            tool_calls,
        }
    }

    #[must_use]
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::Tool,
            content: vec![ProviderMessageContentPart::text(content.into())],
            name: None,
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: Vec::new(),
        }
    }

    #[must_use]
    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|part| match part {
                ProviderMessageContentPart::Text { text } => Some(text.as_str()),
                ProviderMessageContentPart::Image { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderRequest {
    pub input_text: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub user_visible_input_text: Option<String>,
    pub messages: Vec<ProviderMessage>,
    pub json_mode: bool,
    pub vision_inputs: Vec<ProviderImageInput>,
    pub model_override: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_catalog_snapshot: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
}

impl ProviderRequest {
    #[must_use]
    pub fn from_input_text(
        input_text: String,
        json_mode: bool,
        vision_inputs: Vec<ProviderImageInput>,
        model_override: Option<String>,
    ) -> Self {
        Self {
            messages: vec![ProviderMessage::user_text(input_text.clone())],
            input_text,
            user_visible_input_text: None,
            json_mode,
            vision_inputs,
            model_override,
            tool_catalog_snapshot: None,
            instruction_hash: None,
            context_trace_id: None,
            budget_profile: None,
            max_output_tokens: None,
        }
    }

    #[must_use]
    pub fn effective_messages(&self) -> Vec<ProviderMessage> {
        if self.messages.is_empty() {
            vec![ProviderMessage::user_text(self.input_text.clone())]
        } else {
            self.messages.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderEvent {
    ModelToken { token: String, is_final: bool },
    ToolProposal { proposal_id: String, tool_name: String, input_json: Vec<u8> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Cancelled,
    Error,
    Unknown,
}

impl ProviderFinishReason {
    #[must_use]
    pub fn from_openai(value: Option<&str>) -> Self {
        match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
            "stop" => Self::Stop,
            "length" => Self::Length,
            "tool_calls" | "function_call" => Self::ToolCalls,
            "content_filter" => Self::ContentFilter,
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub fn from_anthropic(value: Option<&str>) -> Self {
        match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
            "end_turn" | "stop_sequence" => Self::Stop,
            "max_tokens" => Self::Length,
            "tool_use" => Self::ToolCalls,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub source: String,
}

impl ProviderUsage {
    #[must_use]
    pub fn new(prompt_tokens: u64, completion_tokens: u64, source: impl Into<String>) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens.saturating_add(completion_tokens),
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderOutputContentPart {
    Text { text: String },
    ToolCall { proposal_id: String, tool_name: String, input_json: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProviderRawProviderRefs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_trace_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_spill_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderRedactionState {
    pub output_redacted: bool,
    pub user_visible_projected: bool,
    pub diagnostics_redacted: bool,
}

impl Default for ProviderRedactionState {
    fn default() -> Self {
        Self { output_redacted: false, user_visible_projected: true, diagnostics_redacted: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderTurnOutput {
    pub full_text: String,
    pub content_parts: Vec<ProviderOutputContentPart>,
    pub finish_reason: ProviderFinishReason,
    pub usage: ProviderUsage,
    pub raw_provider_refs: ProviderRawProviderRefs,
    pub redaction_state: ProviderRedactionState,
}

impl ProviderTurnOutput {
    #[must_use]
    pub fn text(
        full_text: String,
        finish_reason: ProviderFinishReason,
        usage: ProviderUsage,
        raw_provider_refs: ProviderRawProviderRefs,
    ) -> Self {
        let (full_text, output_redacted) =
            project_provider_output_text(full_text, MAX_PROVIDER_TURN_TEXT_BYTES);
        let mut raw_provider_refs = raw_provider_refs;
        if output_redacted && raw_provider_refs.stream_spill_ref.is_none() {
            raw_provider_refs.stream_spill_ref = Some(provider_output_truncation_ref());
        }
        let content_parts = if full_text.is_empty() {
            Vec::new()
        } else {
            vec![ProviderOutputContentPart::Text { text: full_text.clone() }]
        };
        Self {
            full_text,
            content_parts,
            finish_reason,
            usage,
            raw_provider_refs,
            redaction_state: ProviderRedactionState {
                output_redacted,
                ..ProviderRedactionState::default()
            },
        }
    }
}

pub(crate) fn bounded_provider_turn_output_for_persistence(
    output: &ProviderTurnOutput,
) -> ProviderTurnOutput {
    let mut bounded = output.clone();
    let (full_text, full_text_redacted) =
        project_provider_output_text(bounded.full_text, MAX_PROVIDER_TURN_TEXT_BYTES);
    bounded.full_text = full_text;
    let mut output_redacted = full_text_redacted;
    for part in &mut bounded.content_parts {
        if let ProviderOutputContentPart::Text { text } = part {
            let (bounded_text, text_redacted) =
                project_provider_output_text(std::mem::take(text), MAX_PROVIDER_TURN_TEXT_BYTES);
            *text = bounded_text;
            output_redacted |= text_redacted;
        }
    }
    if output_redacted {
        bounded.redaction_state.output_redacted = true;
        if bounded.raw_provider_refs.stream_spill_ref.is_none() {
            bounded.raw_provider_refs.stream_spill_ref = Some(provider_output_truncation_ref());
        }
    }
    bounded
}

pub(super) fn append_provider_text_with_hard_limit(
    target: &mut String,
    incoming: &str,
    max_bytes: usize,
) -> bool {
    if incoming.is_empty() {
        return false;
    }
    let limit = provider_output_text_limit(max_bytes);
    if target.ends_with(PROVIDER_OUTPUT_TRUNCATED_MARKER) {
        return true;
    }
    if target.len().saturating_add(incoming.len()) <= limit {
        target.push_str(incoming);
        return false;
    }

    let prefix_budget = limit.saturating_sub(PROVIDER_OUTPUT_TRUNCATED_MARKER.len());
    if target.len() > prefix_budget {
        truncate_string_to_utf8_boundary(target, prefix_budget);
    } else if target.len() < prefix_budget {
        let remaining = prefix_budget.saturating_sub(target.len());
        target.push_str(utf8_prefix(incoming, remaining));
    }
    target.push_str(PROVIDER_OUTPUT_TRUNCATED_MARKER);
    true
}

fn project_provider_output_text(full_text: String, max_bytes: usize) -> (String, bool) {
    let limit = provider_output_text_limit(max_bytes);
    if full_text.len() <= limit {
        return (full_text, false);
    }
    let mut bounded = String::with_capacity(limit);
    append_provider_text_with_hard_limit(&mut bounded, full_text.as_str(), limit);
    (bounded, true)
}

fn provider_output_text_limit(max_bytes: usize) -> usize {
    max_bytes.max(PROVIDER_OUTPUT_TRUNCATED_MARKER.len())
}

fn provider_output_truncation_ref() -> String {
    "provider-output-inline-truncated".to_owned()
}

fn truncate_string_to_utf8_boundary(value: &mut String, max_bytes: usize) {
    if value.len() <= max_bytes {
        return;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
}

fn utf8_prefix(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &value[..boundary]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResponse {
    pub output: ProviderTurnOutput,
    pub events: Vec<ProviderEvent>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub retry_count: u32,
    pub provider_id: String,
    pub model_id: String,
    pub served_from_cache: bool,
    pub failover_count: u32,
    pub attempts: Vec<super::ProviderAttemptSummary>,
}

pub(super) fn provider_request_has_vision(request: &ProviderRequest) -> bool {
    !request.vision_inputs.is_empty()
        || request.effective_messages().iter().any(|message| {
            message
                .content
                .iter()
                .any(|part| matches!(part, ProviderMessageContentPart::Image { .. }))
        })
}

fn split_provider_stream_text(input: &str, max_words_per_chunk: usize) -> Vec<String> {
    if max_words_per_chunk == 0 || input.trim().is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_words = 0_usize;
    let mut pending_whitespace = String::new();
    let mut current_word = String::new();

    for ch in input.chars() {
        if ch.is_whitespace() {
            if current_word.is_empty() {
                pending_whitespace.push(ch);
            } else {
                if current_words == max_words_per_chunk {
                    chunks.push(std::mem::take(&mut current));
                    current_words = 0;
                }
                current.push_str(pending_whitespace.as_str());
                pending_whitespace.clear();
                current.push_str(current_word.as_str());
                current_word.clear();
                current_words = current_words.saturating_add(1);
                pending_whitespace.push(ch);
            }
            continue;
        }
        current_word.push(ch);
    }

    if !current_word.is_empty() {
        if current_words == max_words_per_chunk {
            chunks.push(std::mem::take(&mut current));
            current_words = 0;
        }
        current.push_str(pending_whitespace.as_str());
        current.push_str(current_word.as_str());
        current_words = current_words.saturating_add(1);
        pending_whitespace.clear();
    } else if !pending_whitespace.is_empty() && !current.is_empty() {
        current.push_str(pending_whitespace.as_str());
    }

    if current_words > 0 || !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

pub(crate) fn provider_events_from_output(output: &ProviderTurnOutput) -> Vec<ProviderEvent> {
    let mut events = Vec::new();
    let should_mark_final_model_token =
        !matches!(output.finish_reason, ProviderFinishReason::ToolCalls)
            && !output
                .content_parts
                .iter()
                .any(|part| matches!(part, ProviderOutputContentPart::ToolCall { .. }));
    let mut last_model_token_index = None;
    for part in &output.content_parts {
        match part {
            ProviderOutputContentPart::Text { text } => {
                let chunks = split_provider_stream_text(
                    text.as_str(),
                    PROVIDER_STREAM_EVENT_TOKEN_CHUNK_SIZE,
                );
                for token in chunks {
                    last_model_token_index = Some(events.len());
                    events.push(ProviderEvent::ModelToken { token, is_final: false });
                }
            }
            ProviderOutputContentPart::ToolCall { proposal_id, tool_name, input_json } => {
                events.push(ProviderEvent::ToolProposal {
                    proposal_id: proposal_id.clone(),
                    tool_name: tool_name.clone(),
                    input_json: serde_json::to_vec(input_json).unwrap_or_else(|_| b"{}".to_vec()),
                });
            }
        }
    }
    if should_mark_final_model_token {
        if let Some(index) = last_model_token_index {
            if let Some(ProviderEvent::ModelToken { is_final, .. }) = events.get_mut(index) {
                *is_final = true;
            }
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        bounded_provider_turn_output_for_persistence, provider_events_from_output, ProviderEvent,
        ProviderFinishReason, ProviderOutputContentPart, ProviderRawProviderRefs,
        ProviderRedactionState, ProviderTurnOutput, ProviderUsage, MAX_PROVIDER_TURN_TEXT_BYTES,
    };

    fn provider_output(
        content_parts: Vec<ProviderOutputContentPart>,
        finish_reason: ProviderFinishReason,
    ) -> ProviderTurnOutput {
        let full_text = content_parts
            .iter()
            .filter_map(|part| match part {
                ProviderOutputContentPart::Text { text } => Some(text.as_str()),
                ProviderOutputContentPart::ToolCall { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("");
        ProviderTurnOutput {
            full_text,
            content_parts,
            finish_reason,
            usage: ProviderUsage::new(1, 1, "test"),
            raw_provider_refs: ProviderRawProviderRefs::default(),
            redaction_state: ProviderRedactionState::default(),
        }
    }

    #[test]
    fn provider_turn_output_truncates_large_text_before_persistence() {
        let output = ProviderTurnOutput::text(
            "a".repeat(MAX_PROVIDER_TURN_TEXT_BYTES + 1024),
            ProviderFinishReason::Stop,
            ProviderUsage::new(1, 1, "test"),
            ProviderRawProviderRefs::default(),
        );

        assert!(output.full_text.len() <= MAX_PROVIDER_TURN_TEXT_BYTES);
        assert!(output.full_text.ends_with("[provider output truncated]"));
        assert!(output.redaction_state.output_redacted);
        assert_eq!(
            output.raw_provider_refs.stream_spill_ref.as_deref(),
            Some("provider-output-inline-truncated")
        );
        assert!(
            matches!(
                output.content_parts.first(),
                Some(ProviderOutputContentPart::Text { text }) if text == &output.full_text
            ),
            "{:?}",
            output.content_parts
        );
        let serialized = serde_json::to_vec(&output).expect("bounded output should serialize");
        assert!(
            serialized.len() < 256 * 1024,
            "bounded provider turn output should fit the default journal payload limit"
        );
    }

    #[test]
    fn bounded_provider_turn_output_for_persistence_bounds_manual_output() {
        let output = provider_output(
            vec![ProviderOutputContentPart::Text {
                text: "b".repeat(MAX_PROVIDER_TURN_TEXT_BYTES + 1024),
            }],
            ProviderFinishReason::Stop,
        );

        let bounded = bounded_provider_turn_output_for_persistence(&output);

        assert!(bounded.full_text.len() <= MAX_PROVIDER_TURN_TEXT_BYTES);
        assert!(bounded.redaction_state.output_redacted);
        assert_eq!(
            bounded.raw_provider_refs.stream_spill_ref.as_deref(),
            Some("provider-output-inline-truncated")
        );
        assert!(
            matches!(
                bounded.content_parts.first(),
                Some(ProviderOutputContentPart::Text { text })
                    if text.len() <= MAX_PROVIDER_TURN_TEXT_BYTES
                        && text.ends_with("[provider output truncated]")
            ),
            "{:?}",
            bounded.content_parts
        );
    }

    #[test]
    fn provider_events_from_output_defers_final_when_tool_call_follows_text() {
        let output = provider_output(
            vec![
                ProviderOutputContentPart::Text {
                    text: "I will inspect the workspace.".to_owned(),
                },
                ProviderOutputContentPart::ToolCall {
                    proposal_id: "proposal-1".to_owned(),
                    tool_name: "palyra.process.run".to_owned(),
                    input_json: json!({"command": "ls", "args": []}),
                },
            ],
            ProviderFinishReason::ToolCalls,
        );

        let events = provider_events_from_output(&output);

        assert!(
            matches!(events.first(), Some(ProviderEvent::ModelToken { is_final: false, .. })),
            "{events:?}"
        );
        assert!(
            matches!(events.last(), Some(ProviderEvent::ToolProposal { proposal_id, .. }) if proposal_id == "proposal-1"),
            "{events:?}"
        );
    }

    #[test]
    fn provider_events_from_output_marks_only_last_text_token_final_without_tool_calls() {
        let output = provider_output(
            vec![
                ProviderOutputContentPart::Text { text: "First part.".to_owned() },
                ProviderOutputContentPart::Text { text: "Final answer.".to_owned() },
            ],
            ProviderFinishReason::Stop,
        );

        let events = provider_events_from_output(&output);
        let final_flags = events
            .iter()
            .filter_map(|event| match event {
                ProviderEvent::ModelToken { is_final, .. } => Some(*is_final),
                ProviderEvent::ToolProposal { .. } => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(final_flags, vec![false, true]);
    }
}
