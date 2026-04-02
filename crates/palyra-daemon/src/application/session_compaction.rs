use serde_json::{json, Value};

use crate::{
    journal::{
        OrchestratorSessionPinRecord, OrchestratorSessionRecord,
        OrchestratorSessionTranscriptRecord,
    },
    orchestrator::estimate_token_count,
};

pub(crate) const SESSION_COMPACTION_STRATEGY: &str = "session_window_v1";
pub(crate) const SESSION_COMPACTION_VERSION: &str = "palyra-session-compaction-v1";
const SESSION_COMPACTION_KEEP_RECENT_TEXT_EVENTS: usize = 6;
const SESSION_COMPACTION_MIN_CONDENSED_EVENTS: usize = 4;
const SESSION_COMPACTION_MAX_SUMMARY_LINES: usize = 8;
const SESSION_COMPACTION_PREVIEW_LEN: usize = 220;

#[derive(Debug, Clone)]
pub(crate) struct SessionCompactionPlan {
    pub(crate) eligible: bool,
    pub(crate) trigger_reason: String,
    pub(crate) trigger_policy: Option<String>,
    pub(crate) trigger_inputs_json: String,
    pub(crate) summary_text: String,
    pub(crate) summary_preview: String,
    pub(crate) source_event_count: u64,
    pub(crate) protected_event_count: u64,
    pub(crate) condensed_event_count: u64,
    pub(crate) omitted_event_count: u64,
    pub(crate) estimated_input_tokens: u64,
    pub(crate) estimated_output_tokens: u64,
    pub(crate) source_records_json: String,
    pub(crate) summary_json: String,
}

impl SessionCompactionPlan {
    pub(crate) fn to_response_json(&self) -> Value {
        json!({
            "eligible": self.eligible,
            "strategy": SESSION_COMPACTION_STRATEGY,
            "compressor_version": SESSION_COMPACTION_VERSION,
            "trigger_reason": self.trigger_reason,
            "trigger_policy": self.trigger_policy,
            "estimated_input_tokens": self.estimated_input_tokens,
            "estimated_output_tokens": self.estimated_output_tokens,
            "token_delta": self.estimated_input_tokens.saturating_sub(self.estimated_output_tokens),
            "source_event_count": self.source_event_count,
            "protected_event_count": self.protected_event_count,
            "condensed_event_count": self.condensed_event_count,
            "omitted_event_count": self.omitted_event_count,
            "summary_text": self.summary_text,
            "summary_preview": self.summary_preview,
            "source_records": serde_json::from_str::<Value>(self.source_records_json.as_str())
                .unwrap_or_else(|_| json!({ "records": [] })),
            "summary": serde_json::from_str::<Value>(self.summary_json.as_str())
                .unwrap_or_else(|_| json!({ "summary_text": self.summary_text })),
        })
    }
}

#[derive(Debug, Clone)]
struct SessionCompactionRecordSnapshot {
    run_id: String,
    seq: i64,
    event_type: String,
    created_at_unix_ms: i64,
    text: String,
    bucket: &'static str,
    reason: Option<&'static str>,
}

pub(crate) fn build_session_compaction_plan(
    session: &OrchestratorSessionRecord,
    transcript: &[OrchestratorSessionTranscriptRecord],
    pins: &[OrchestratorSessionPinRecord],
    trigger_reason: Option<&str>,
    trigger_policy: Option<&str>,
) -> SessionCompactionPlan {
    let pin_keys = pins
        .iter()
        .map(|pin| (pin.run_id.clone(), pin.tape_seq))
        .collect::<std::collections::HashSet<_>>();
    let extracted = transcript
        .iter()
        .filter_map(|record| {
            let text = extract_transcript_search_text(record)?;
            Some(SessionCompactionRecordSnapshot {
                run_id: record.run_id.clone(),
                seq: record.seq,
                event_type: record.event_type.clone(),
                created_at_unix_ms: record.created_at_unix_ms,
                text,
                bucket: "condensed",
                reason: None,
            })
        })
        .collect::<Vec<_>>();
    let source_event_count = extracted.len() as u64;
    let estimated_input_tokens =
        extracted.iter().map(|record| estimate_token_count(record.text.as_str())).sum::<u64>();

    let mut protected_start =
        extracted.len().saturating_sub(SESSION_COMPACTION_KEEP_RECENT_TEXT_EVENTS);
    for (index, record) in extracted.iter().enumerate() {
        if pin_keys.contains(&(record.run_id.clone(), record.seq))
            || record.event_type == "rollback.marker"
            || record.event_type == "checkpoint.restore"
        {
            protected_start = protected_start.min(index);
        }
    }

    let mut protected_records = Vec::new();
    let mut condensed_records = Vec::new();
    for (index, record) in extracted.iter().enumerate() {
        if pin_keys.contains(&(record.run_id.clone(), record.seq)) {
            let mut protected = record.clone();
            protected.bucket = "protected";
            protected.reason = Some("pinned");
            protected_records.push(protected);
            continue;
        }
        if record.event_type == "rollback.marker" || record.event_type == "checkpoint.restore" {
            let mut protected = record.clone();
            protected.bucket = "protected";
            protected.reason = Some("lineage_marker");
            protected_records.push(protected);
            continue;
        }
        if index >= protected_start {
            let mut protected = record.clone();
            protected.bucket = "protected";
            protected.reason = Some("recent_context");
            protected_records.push(protected);
            continue;
        }
        condensed_records.push(record.clone());
    }

    let eligible = condensed_records.len() >= SESSION_COMPACTION_MIN_CONDENSED_EVENTS;
    let summary_lines = condensed_records
        .iter()
        .take(SESSION_COMPACTION_MAX_SUMMARY_LINES)
        .enumerate()
        .map(|(index, record)| {
            format!(
                "{}. {}: {}",
                index + 1,
                compaction_event_label(record.event_type.as_str()),
                truncate_console_text(record.text.as_str(), 180),
            )
        })
        .collect::<Vec<_>>();
    let omitted_event_count =
        condensed_records.len().saturating_sub(SESSION_COMPACTION_MAX_SUMMARY_LINES) as u64;
    let summary_text = if summary_lines.is_empty() {
        format!("No eligible older transcript range was found for session {}.", session.session_id)
    } else {
        let mut text = String::from("Condensed earlier transcript context:\n");
        text.push_str(summary_lines.join("\n").as_str());
        if omitted_event_count > 0 {
            text.push('\n');
            text.push_str(
                format!("{omitted_event_count} older records were omitted from this compact view.")
                    .as_str(),
            );
        }
        text
    };
    let summary_preview =
        truncate_console_text(summary_text.as_str(), SESSION_COMPACTION_PREVIEW_LEN);
    let protected_event_count = protected_records.len() as u64;
    let condensed_event_count = condensed_records.len() as u64;
    let protected_tokens = protected_records
        .iter()
        .map(|record| estimate_token_count(record.text.as_str()))
        .sum::<u64>();
    let estimated_output_tokens =
        estimate_token_count(summary_text.as_str()).saturating_add(protected_tokens);
    let summary_json = json!({
        "session_id": session.session_id,
        "branch_state": session.branch_state,
        "eligible": eligible,
        "summary_lines": summary_lines,
        "omitted_event_count": omitted_event_count,
        "protected_records": protected_records.iter().map(compaction_record_json).collect::<Vec<_>>(),
    })
    .to_string();
    let source_records_json = json!({
        "records": condensed_records.iter().map(compaction_record_json).collect::<Vec<_>>(),
        "protected": protected_records.iter().map(compaction_record_json).collect::<Vec<_>>(),
    })
    .to_string();
    let trigger_reason = trigger_reason
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("operator_requested_compaction")
        .to_owned();
    let trigger_policy =
        trigger_policy.map(str::trim).filter(|value| !value.is_empty()).map(ToOwned::to_owned);
    let trigger_inputs_json = json!({
        "source_event_count": source_event_count,
        "protected_event_count": protected_event_count,
        "condensed_event_count": condensed_event_count,
        "estimated_input_tokens": estimated_input_tokens,
        "estimated_output_tokens": estimated_output_tokens,
    })
    .to_string();
    SessionCompactionPlan {
        eligible,
        trigger_reason,
        trigger_policy,
        trigger_inputs_json,
        summary_text,
        summary_preview,
        source_event_count,
        protected_event_count,
        condensed_event_count,
        omitted_event_count,
        estimated_input_tokens,
        estimated_output_tokens,
        source_records_json,
        summary_json,
    }
}

pub(crate) fn render_compaction_prompt_block(
    artifact_id: &str,
    mode: &str,
    trigger_reason: &str,
    summary_text: &str,
) -> String {
    format!(
        "<session_compaction_summary artifact_id=\"{artifact_id}\" mode=\"{mode}\" trigger_reason=\"{trigger_reason}\">\n{summary_text}\n</session_compaction_summary>"
    )
}

fn compaction_record_json(record: &SessionCompactionRecordSnapshot) -> Value {
    json!({
        "run_id": record.run_id,
        "seq": record.seq,
        "event_type": record.event_type,
        "created_at_unix_ms": record.created_at_unix_ms,
        "text": record.text,
        "bucket": record.bucket,
        "reason": record.reason,
    })
}

fn compaction_event_label(event_type: &str) -> &'static str {
    match event_type {
        "message.received" | "queued.input" => "User",
        "message.replied" => "Assistant",
        "rollback.marker" => "Lineage",
        "checkpoint.restore" => "Checkpoint restore",
        _ => "Event",
    }
}

fn extract_transcript_search_text(record: &OrchestratorSessionTranscriptRecord) -> Option<String> {
    match record.event_type.as_str() {
        "message.received" | "queued.input" => extract_transcript_text(record, "text"),
        "message.replied" => extract_transcript_text(record, "reply_text"),
        "rollback.marker" => {
            serde_json::from_str::<Value>(record.payload_json.as_str()).ok().and_then(|payload| {
                payload.get("event").and_then(Value::as_str).map(ToOwned::to_owned)
            })
        }
        _ => None,
    }
}

fn extract_transcript_text(
    record: &OrchestratorSessionTranscriptRecord,
    key: &str,
) -> Option<String> {
    serde_json::from_str::<Value>(record.payload_json.as_str())
        .ok()?
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn truncate_console_text(raw: &str, max_chars: usize) -> String {
    let normalized = raw.replace(['\r', '\n'], " ");
    let trimmed = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.chars().count() <= max_chars {
        return trimmed;
    }
    let mut shortened = trimmed.chars().take(max_chars).collect::<String>();
    shortened.push_str("...");
    shortened
}

#[cfg(test)]
mod tests {
    use super::{build_session_compaction_plan, render_compaction_prompt_block};
    use crate::journal::{
        OrchestratorSessionPinRecord, OrchestratorSessionRecord,
        OrchestratorSessionTranscriptRecord,
    };

    fn session_record() -> OrchestratorSessionRecord {
        OrchestratorSessionRecord {
            session_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
            session_key: "ops:phase4".to_owned(),
            session_label: Some("Ops Phase 4".to_owned()),
            principal: "user:ops".to_owned(),
            device_id: "device-1".to_owned(),
            channel: Some("console".to_owned()),
            created_at_unix_ms: 1,
            updated_at_unix_ms: 2,
            last_run_id: Some("01ARZ3NDEKTSV4RRFFQ69G5FAW".to_owned()),
            archived_at_unix_ms: None,
            auto_title: None,
            auto_title_source: None,
            auto_title_generator_version: None,
            title: "Ops triage".to_owned(),
            title_source: "manual".to_owned(),
            title_generator_version: None,
            preview: None,
            last_intent: None,
            last_summary: None,
            match_snippet: None,
            branch_state: "active_branch".to_owned(),
            parent_session_id: Some("01ARZ3NDEKTSV4RRFFQ69G5FAX".to_owned()),
            branch_origin_run_id: None,
            last_run_state: Some("done".to_owned()),
        }
    }

    fn transcript_record(
        seq: i64,
        event_type: &str,
        payload_json: &str,
    ) -> OrchestratorSessionTranscriptRecord {
        OrchestratorSessionTranscriptRecord {
            session_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
            run_id: "01ARZ3NDEKTSV4RRFFQ69G5FAW".to_owned(),
            seq,
            event_type: event_type.to_owned(),
            payload_json: payload_json.to_owned(),
            created_at_unix_ms: 10 + seq,
            origin_kind: "manual".to_owned(),
            origin_run_id: None,
        }
    }

    #[test]
    fn compaction_plan_keeps_pins_and_recent_context_protected() {
        let transcript = vec![
            transcript_record(
                0,
                "message.received",
                r#"{"text":"First user request with enough detail for compaction."}"#,
            ),
            transcript_record(
                1,
                "message.replied",
                r#"{"reply_text":"First assistant response with a fairly long explanation."}"#,
            ),
            transcript_record(
                2,
                "message.received",
                r#"{"text":"Second user request worth preserving because it gets pinned."}"#,
            ),
            transcript_record(
                3,
                "message.replied",
                r#"{"reply_text":"Second assistant response that will be pinned."}"#,
            ),
            transcript_record(
                4,
                "message.received",
                r#"{"text":"Third user request still belongs to the older condensed range."}"#,
            ),
            transcript_record(
                5,
                "message.replied",
                r#"{"reply_text":"Third assistant response still belongs to the older condensed range."}"#,
            ),
            transcript_record(
                6,
                "message.received",
                r#"{"text":"Fourth user request still belongs to the older condensed range."}"#,
            ),
            transcript_record(
                7,
                "message.replied",
                r#"{"reply_text":"Fourth assistant response still belongs to the older condensed range."}"#,
            ),
            transcript_record(
                8,
                "message.received",
                r#"{"text":"Fifth user turn remains recent context."}"#,
            ),
            transcript_record(
                9,
                "message.replied",
                r#"{"reply_text":"Fifth assistant turn remains recent context."}"#,
            ),
            transcript_record(
                10,
                "message.received",
                r#"{"text":"Sixth user turn remains recent context."}"#,
            ),
            transcript_record(
                11,
                "message.replied",
                r#"{"reply_text":"Sixth assistant turn remains recent context."}"#,
            ),
            transcript_record(
                12,
                "message.received",
                r#"{"text":"Seventh user turn remains recent context."}"#,
            ),
            transcript_record(
                13,
                "message.replied",
                r#"{"reply_text":"Seventh assistant turn remains recent context."}"#,
            ),
        ];
        let pins = vec![OrchestratorSessionPinRecord {
            pin_id: "01ARZ3NDEKTSV4RRFFQ69G5FAY".to_owned(),
            session_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
            run_id: "01ARZ3NDEKTSV4RRFFQ69G5FAW".to_owned(),
            tape_seq: 9,
            title: "Keep this".to_owned(),
            note: None,
            created_at_unix_ms: 42,
        }];

        let plan = build_session_compaction_plan(
            &session_record(),
            transcript.as_slice(),
            pins.as_slice(),
            Some("manual"),
            Some("operator"),
        );

        assert!(plan.eligible, "older transcript range should be eligible for compaction");
        assert_eq!(plan.source_event_count, 14);
        assert!(plan.protected_event_count >= 6, "recent context should stay protected");
        assert!(plan.condensed_event_count >= 3, "older events should remain in condensed range");
        assert!(
            plan.summary_text.contains("Condensed earlier transcript context"),
            "summary should explain the compacted range"
        );
        assert!(
            plan.source_records_json.contains("\"bucket\":\"protected\""),
            "protected records should be labeled in the source snapshot"
        );
        assert!(
            plan.source_records_json.contains("\"reason\":\"pinned\""),
            "pinned records should be preserved explicitly in the source snapshot"
        );
    }

    #[test]
    fn compaction_prompt_block_wraps_summary_in_explicit_tag() {
        let block = render_compaction_prompt_block(
            "artifact-1",
            "automatic",
            "budget_guard_v1",
            "Condensed earlier transcript context:\n1. User: hello",
        );

        assert!(block.starts_with("<session_compaction_summary"), "block should be tagged");
        assert!(block.contains("artifact_id=\"artifact-1\""), "artifact id should be present");
        assert!(block.contains("budget_guard_v1"), "trigger reason should be preserved");
        assert!(
            block.contains("Condensed earlier transcript context"),
            "summary should be embedded"
        );
    }
}
