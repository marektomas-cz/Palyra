use palyra_common::{runtime_contracts::PruningPolicyClass, runtime_preview::RuntimePreviewMode};
use serde::Serialize;
use serde_json::{json, Value};

use crate::config::PruningPolicyMatrixConfig;

pub(crate) const SESSION_PRUNING_POLICY_ID: &str = "session_pruning.v1";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub(crate) enum PruningTaskClass {
    InteractiveChat,
    DelegatedChild,
    BackgroundRoutine,
    RecallSummary,
    WorkspaceMutationReview,
}

impl PruningTaskClass {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::InteractiveChat => "interactive_chat",
            Self::DelegatedChild => "delegated_child",
            Self::BackgroundRoutine => "background_routine",
            Self::RecallSummary => "recall_summary",
            Self::WorkspaceMutationReview => "workspace_mutation_review",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub(crate) enum PruningRiskLevel {
    Normal,
    Elevated,
}

impl PruningRiskLevel {
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Elevated => "elevated",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct SessionPruningDecision {
    pub(crate) policy_id: String,
    pub(crate) mode: RuntimePreviewMode,
    pub(crate) task_class: PruningTaskClass,
    pub(crate) risk_level: PruningRiskLevel,
    pub(crate) policy_class: PruningPolicyClass,
    pub(crate) apply_enabled: bool,
    pub(crate) manual_apply_enabled: bool,
    pub(crate) min_token_savings: u64,
    pub(crate) protected_tail_turns: usize,
    pub(crate) target_prompt_tokens: u64,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct SessionPruningOutcome {
    pub(crate) provider_input_text: String,
    pub(crate) source_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) tokens_saved: u64,
    pub(crate) applied: bool,
    pub(crate) eligible: bool,
    pub(crate) reason: String,
    pub(crate) explain_json: Value,
}

#[derive(Debug, Clone)]
struct PromptBlock {
    index: usize,
    label: String,
    text: String,
    estimated_tokens: u64,
    protected: bool,
    priority: u8,
}

#[must_use]
pub(crate) fn classify_pruning_task(
    memory_ingest_reason: &str,
    parameter_delta_json: Option<&str>,
) -> PruningTaskClass {
    let reason = memory_ingest_reason.to_ascii_lowercase();
    let parameter_delta = parameter_delta_json.unwrap_or_default().to_ascii_lowercase();
    if parameter_delta.contains("\"project_context\"") || parameter_delta.contains("workspace") {
        return PruningTaskClass::WorkspaceMutationReview;
    }
    if parameter_delta.contains("\"attachment_recall\"")
        || parameter_delta.contains("\"context_references\"")
        || parameter_delta.contains("\"explicit_recall\"")
    {
        return PruningTaskClass::RecallSummary;
    }
    if reason.contains("background") || reason.contains("routine") {
        return PruningTaskClass::BackgroundRoutine;
    }
    if reason.contains("delegat") || reason.contains("child") {
        return PruningTaskClass::DelegatedChild;
    }
    PruningTaskClass::InteractiveChat
}

#[must_use]
pub(crate) fn detect_pruning_risk(provider_input_text: &str) -> PruningRiskLevel {
    let lowered = provider_input_text.to_ascii_lowercase();
    let elevated = [
        "tool_call",
        "tool_result",
        "approval",
        "vault",
        "secret",
        "private key",
        "access token",
        "allow_sensitive_tools",
    ]
    .iter()
    .any(|needle| lowered.contains(needle));
    if elevated {
        PruningRiskLevel::Elevated
    } else {
        PruningRiskLevel::Normal
    }
}

#[must_use]
pub(crate) fn pruning_decision_from_config(
    config: &PruningPolicyMatrixConfig,
    task_class: PruningTaskClass,
    risk_level: PruningRiskLevel,
) -> SessionPruningDecision {
    let mut policy_class = match task_class {
        PruningTaskClass::WorkspaceMutationReview => PruningPolicyClass::Conservative,
        PruningTaskClass::InteractiveChat | PruningTaskClass::DelegatedChild => {
            PruningPolicyClass::Balanced
        }
        PruningTaskClass::BackgroundRoutine | PruningTaskClass::RecallSummary => {
            PruningPolicyClass::Aggressive
        }
    };
    if risk_level == PruningRiskLevel::Elevated {
        policy_class = PruningPolicyClass::Conservative;
    }
    let (protected_tail_turns, target_prompt_tokens) = match policy_class {
        PruningPolicyClass::Disabled => (0, u64::MAX),
        PruningPolicyClass::Conservative => (3, 8_192),
        PruningPolicyClass::Balanced => (3, 6_144),
        PruningPolicyClass::Aggressive => (2, 4_096),
    };
    SessionPruningDecision {
        policy_id: SESSION_PRUNING_POLICY_ID.to_owned(),
        mode: config.mode,
        task_class,
        risk_level,
        policy_class,
        apply_enabled: config.mode == RuntimePreviewMode::Enabled,
        manual_apply_enabled: config.manual_apply_enabled,
        min_token_savings: config.min_token_savings,
        protected_tail_turns,
        target_prompt_tokens,
        reason: format!(
            "{}:{}:{}",
            task_class.as_str(),
            risk_level.as_str(),
            policy_class.as_str()
        ),
    }
}

#[must_use]
pub(crate) fn apply_ephemeral_prompt_pruning(
    provider_input_text: &str,
    decision: &SessionPruningDecision,
) -> SessionPruningOutcome {
    let source_tokens = estimate_prompt_tokens(provider_input_text);
    if decision.mode == RuntimePreviewMode::Disabled {
        return no_pruning_outcome(provider_input_text, decision, source_tokens, "policy_disabled");
    }
    if source_tokens <= decision.target_prompt_tokens.saturating_add(decision.min_token_savings) {
        return no_pruning_outcome(provider_input_text, decision, source_tokens, "under_budget");
    }

    let mut blocks = split_prompt_blocks(provider_input_text);
    let mut removed = Vec::new();
    let mut selected_tokens = blocks.iter().map(|block| block.estimated_tokens).sum::<u64>();
    while selected_tokens > decision.target_prompt_tokens {
        let Some(remove_index) = blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| !block.protected)
            .min_by(|(_, left), (_, right)| {
                left.priority
                    .cmp(&right.priority)
                    .then_with(|| right.estimated_tokens.cmp(&left.estimated_tokens))
                    .then_with(|| left.index.cmp(&right.index))
            })
            .map(|(index, _)| index)
        else {
            break;
        };
        let removed_block = blocks.remove(remove_index);
        selected_tokens = selected_tokens.saturating_sub(removed_block.estimated_tokens);
        removed.push(removed_block);
    }

    let tokens_saved = source_tokens.saturating_sub(selected_tokens);
    let eligible = tokens_saved >= decision.min_token_savings && !removed.is_empty();
    if !eligible {
        return no_pruning_outcome(
            provider_input_text,
            decision,
            source_tokens,
            "min_token_savings_not_met",
        );
    }

    let pruned_text = if decision.apply_enabled {
        render_selected_blocks(blocks.as_slice(), decision, tokens_saved)
    } else {
        provider_input_text.to_owned()
    };
    let output_tokens = if decision.apply_enabled {
        estimate_prompt_tokens(pruned_text.as_str())
    } else {
        source_tokens
    };
    SessionPruningOutcome {
        provider_input_text: pruned_text,
        source_tokens,
        output_tokens,
        tokens_saved: source_tokens.saturating_sub(output_tokens),
        applied: decision.apply_enabled,
        eligible,
        reason: if decision.apply_enabled {
            "ephemeral_provider_input_pruned".to_owned()
        } else {
            "ephemeral_provider_input_pruning_preview".to_owned()
        },
        explain_json: json!({
            "policy": decision_snapshot_json(decision),
            "source_tokens": source_tokens,
            "output_tokens": output_tokens,
            "tokens_saved": source_tokens.saturating_sub(output_tokens),
            "applied": decision.apply_enabled,
            "eligible": eligible,
            "dropped_blocks": removed.iter().map(|block| {
                json!({
                    "index": block.index,
                    "label": block.label,
                    "estimated_tokens": block.estimated_tokens,
                    "reason": "dropped_by_ephemeral_pruning",
                })
            }).collect::<Vec<_>>(),
            "protected_tail_turns": decision.protected_tail_turns,
            "transcript_mutated": false,
        }),
    }
}

#[must_use]
pub(crate) fn context_engine_pruning_outcome(
    decision: &SessionPruningDecision,
    selected_tokens: u64,
    dropped_tokens: u64,
    dropped_segments: Value,
) -> Option<SessionPruningOutcome> {
    if decision.mode == RuntimePreviewMode::Disabled || dropped_tokens < decision.min_token_savings
    {
        return None;
    }
    let source_tokens = selected_tokens.saturating_add(dropped_tokens);
    Some(SessionPruningOutcome {
        provider_input_text: String::new(),
        source_tokens,
        output_tokens: selected_tokens,
        tokens_saved: dropped_tokens,
        applied: true,
        eligible: true,
        reason: "context_engine_budget_pruned_segments".to_owned(),
        explain_json: json!({
            "policy": decision_snapshot_json(decision),
            "source_tokens": source_tokens,
            "output_tokens": selected_tokens,
            "tokens_saved": dropped_tokens,
            "applied": true,
            "eligible": true,
            "dropped_segments": dropped_segments,
            "transcript_mutated": false,
        }),
    })
}

#[must_use]
pub(crate) fn estimate_prompt_tokens(text: &str) -> u64 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        0
    } else {
        trimmed.chars().count().div_ceil(4) as u64
    }
}

#[must_use]
pub(crate) fn decision_snapshot_json(decision: &SessionPruningDecision) -> Value {
    json!({
        "policy_id": decision.policy_id,
        "mode": decision.mode.as_str(),
        "task_class": decision.task_class.as_str(),
        "risk_level": decision.risk_level.as_str(),
        "policy_class": decision.policy_class.as_str(),
        "apply_enabled": decision.apply_enabled,
        "manual_apply_enabled": decision.manual_apply_enabled,
        "min_token_savings": decision.min_token_savings,
        "protected_tail_turns": decision.protected_tail_turns,
        "target_prompt_tokens": decision.target_prompt_tokens,
        "reason": decision.reason,
    })
}

fn no_pruning_outcome(
    provider_input_text: &str,
    decision: &SessionPruningDecision,
    source_tokens: u64,
    reason: &str,
) -> SessionPruningOutcome {
    SessionPruningOutcome {
        provider_input_text: provider_input_text.to_owned(),
        source_tokens,
        output_tokens: source_tokens,
        tokens_saved: 0,
        applied: false,
        eligible: false,
        reason: reason.to_owned(),
        explain_json: json!({
            "policy": decision_snapshot_json(decision),
            "source_tokens": source_tokens,
            "output_tokens": source_tokens,
            "tokens_saved": 0,
            "applied": false,
            "eligible": false,
            "reason": reason,
            "transcript_mutated": false,
        }),
    }
}

fn split_prompt_blocks(provider_input_text: &str) -> Vec<PromptBlock> {
    provider_input_text
        .split("\n\n")
        .enumerate()
        .filter_map(|(index, raw)| {
            let text = raw.trim();
            if text.is_empty() {
                return None;
            }
            let label = block_label(text);
            let protected = block_is_protected(text, index);
            Some(PromptBlock {
                index,
                label: label.to_owned(),
                text: text.to_owned(),
                estimated_tokens: estimate_prompt_tokens(text),
                protected,
                priority: block_priority(label),
            })
        })
        .collect()
}

fn render_selected_blocks(
    blocks: &[PromptBlock],
    decision: &SessionPruningDecision,
    tokens_saved: u64,
) -> String {
    let mut rendered = String::new();
    rendered.push_str("<pruning_note>\n");
    rendered.push_str(
        format!(
            "policy={} task_class={} policy_class={} tokens_saved={} transcript_mutated=false\n",
            decision.policy_id,
            decision.task_class.as_str(),
            decision.policy_class.as_str(),
            tokens_saved,
        )
        .as_str(),
    );
    rendered.push_str("</pruning_note>\n\n");
    rendered.push_str(
        blocks.iter().map(|block| block.text.as_str()).collect::<Vec<_>>().join("\n\n").as_str(),
    );
    rendered
}

fn block_label(text: &str) -> &'static str {
    let lowered = text.to_ascii_lowercase();
    if lowered.contains("<memory_context>") {
        "memory_context"
    } else if lowered.contains("<attachment_context>") {
        "attachment_context"
    } else if lowered.contains("<project_context>") {
        "project_context"
    } else if lowered.contains("<context_references>") {
        "context_references"
    } else if lowered.contains("<recent_conversation>") {
        "recent_conversation"
    } else if lowered.contains("<session_summary") {
        "session_compaction"
    } else {
        "user_input"
    }
}

fn block_priority(label: &str) -> u8 {
    match label {
        "memory_context" => 10,
        "attachment_context" => 20,
        "project_context" => 30,
        "session_compaction" => 40,
        "context_references" => 70,
        "recent_conversation" => 90,
        _ => 100,
    }
}

fn block_is_protected(text: &str, _index: usize) -> bool {
    let lowered = text.to_ascii_lowercase();
    block_label(text) == "user_input"
        || lowered.contains("tool_call")
        || lowered.contains("tool_result")
        || lowered.contains("approval")
        || lowered.contains("<recent_conversation>")
        || lowered.contains("<context_references>")
}

#[cfg(test)]
mod tests {
    use palyra_common::runtime_preview::RuntimePreviewMode;

    use crate::config::PruningPolicyMatrixConfig;

    use super::{
        apply_ephemeral_prompt_pruning, classify_pruning_task, detect_pruning_risk,
        pruning_decision_from_config, PruningRiskLevel, PruningTaskClass,
    };

    #[test]
    fn classifies_workspace_mutation_from_parameter_delta() {
        assert_eq!(
            classify_pruning_task("run_stream_user_input", Some(r#"{"project_context":{}}"#)),
            PruningTaskClass::WorkspaceMutationReview
        );
    }

    #[test]
    fn detects_elevated_tool_and_secret_risk() {
        assert_eq!(detect_pruning_risk("tool_result: ok"), PruningRiskLevel::Elevated);
        assert_eq!(detect_pruning_risk("ordinary prompt"), PruningRiskLevel::Normal);
    }

    #[test]
    fn enabled_policy_drops_low_priority_memory_without_mutating_tail() {
        let config = PruningPolicyMatrixConfig {
            mode: RuntimePreviewMode::Enabled,
            min_token_savings: 10,
            ..PruningPolicyMatrixConfig::default()
        };
        let decision = pruning_decision_from_config(
            &config,
            PruningTaskClass::BackgroundRoutine,
            PruningRiskLevel::Normal,
        );
        let large_memory = "memory ".repeat(18_000);
        let prompt = format!(
            "<memory_context>\n{large_memory}\n</memory_context>\n\n<recent_conversation>\n1. user: keep me\n</recent_conversation>\n\nfinal user request"
        );

        let outcome = apply_ephemeral_prompt_pruning(prompt.as_str(), &decision);

        assert!(outcome.applied);
        assert!(outcome.tokens_saved >= 10);
        assert!(!outcome.provider_input_text.contains("<memory_context>"));
        assert!(outcome.provider_input_text.contains("<recent_conversation>"));
        assert!(outcome.provider_input_text.contains("final user request"));
        assert_eq!(outcome.explain_json["transcript_mutated"], false);
    }
}
