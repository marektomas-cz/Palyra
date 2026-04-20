use palyra_common::runtime_contracts::{QueueDecision, QueueMode};
use serde::Serialize;
use serde_json::{json, Value};

use crate::config::SessionQueuePolicyConfig;

pub(crate) const SESSION_QUEUE_POLICY_ID: &str = "session_queue.v1";
const DEFAULT_PRIORITY_LANE: &str = "normal";
const DEFAULT_DROP_POLICY: &str = "summarize_oldest";
const DEFAULT_OVERFLOW_BEHAVIOR: &str = "deterministic_backlog_summary";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct SessionQueuePolicy {
    pub(crate) policy_id: String,
    pub(crate) mode: QueueMode,
    pub(crate) priority_lane: String,
    pub(crate) debounce_ms: u64,
    pub(crate) cap: usize,
    pub(crate) drop_policy: String,
    pub(crate) overflow_behavior: String,
    pub(crate) coalescing_group: String,
    pub(crate) source: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct SessionQueueSafeBoundary {
    pub(crate) active_run_stream: bool,
    pub(crate) pending_approval: bool,
    pub(crate) sensitive_tool_execution: bool,
    pub(crate) before_model_round: bool,
    pub(crate) after_model_round: bool,
    pub(crate) after_tool_result: bool,
    pub(crate) after_approval_wait: bool,
    pub(crate) after_child_merge: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct SessionQueueDecision {
    pub(crate) decision: QueueDecision,
    pub(crate) mode: QueueMode,
    pub(crate) accepted: bool,
    pub(crate) reason: String,
    pub(crate) safe_boundary: SessionQueueSafeBoundary,
    pub(crate) policy: SessionQueuePolicy,
}

impl SessionQueuePolicy {
    #[must_use]
    pub(crate) fn from_config(
        config: &SessionQueuePolicyConfig,
        session_id: &str,
        channel: Option<&str>,
        agent_id: Option<&str>,
    ) -> Self {
        let scope = agent_id
            .map(|agent_id| format!("agent:{agent_id}"))
            .or_else(|| channel.map(|channel| format!("channel:{channel}")))
            .unwrap_or_else(|| "session".to_owned());
        Self {
            policy_id: SESSION_QUEUE_POLICY_ID.to_owned(),
            mode: QueueMode::Followup,
            priority_lane: DEFAULT_PRIORITY_LANE.to_owned(),
            debounce_ms: config.merge_window_ms,
            cap: config.max_depth,
            drop_policy: DEFAULT_DROP_POLICY.to_owned(),
            overflow_behavior: DEFAULT_OVERFLOW_BEHAVIOR.to_owned(),
            coalescing_group: format!("{scope}:{session_id}"),
            source: "config.session_queue_policy".to_owned(),
        }
    }

    #[must_use]
    pub(crate) fn snapshot_json(&self) -> Value {
        json!({
            "policy_id": self.policy_id,
            "mode": self.mode.as_str(),
            "priority_lane": self.priority_lane,
            "debounce_ms": self.debounce_ms,
            "cap": self.cap,
            "drop_policy": self.drop_policy,
            "overflow_behavior": self.overflow_behavior,
            "coalescing_group": self.coalescing_group,
            "source": self.source,
        })
    }
}

impl SessionQueueSafeBoundary {
    #[must_use]
    pub(crate) fn active(active_run_stream: bool, pending_approval: bool) -> Self {
        Self {
            active_run_stream,
            pending_approval,
            sensitive_tool_execution: false,
            before_model_round: active_run_stream && !pending_approval,
            after_model_round: false,
            after_tool_result: false,
            after_approval_wait: pending_approval,
            after_child_merge: false,
        }
    }

    #[must_use]
    pub(crate) const fn can_steer(&self) -> bool {
        self.active_run_stream
            && !self.pending_approval
            && !self.sensitive_tool_execution
            && (self.before_model_round
                || self.after_model_round
                || self.after_tool_result
                || self.after_child_merge)
    }

    #[must_use]
    pub(crate) const fn can_interrupt(&self) -> bool {
        self.active_run_stream && !self.pending_approval && !self.sensitive_tool_execution
    }
}

impl SessionQueueDecision {
    #[must_use]
    pub(crate) fn explain_json(&self) -> Value {
        json!({
            "decision": self.decision.as_str(),
            "mode": self.mode.as_str(),
            "accepted": self.accepted,
            "reason": self.reason,
            "safe_boundary": self.safe_boundary,
            "policy": self.policy.snapshot_json(),
        })
    }
}

#[must_use]
pub(crate) fn decide_session_queue_mode(
    policy: SessionQueuePolicy,
    requested_mode: Option<QueueMode>,
    safe_boundary: SessionQueueSafeBoundary,
    current_depth: usize,
) -> SessionQueueDecision {
    let requested_mode = requested_mode.unwrap_or(policy.mode);
    if current_depth >= policy.cap {
        return SessionQueueDecision {
            decision: QueueDecision::Overflow,
            mode: QueueMode::Collect,
            accepted: true,
            reason: "queue_cap_reached_overflow_summary_required".to_owned(),
            safe_boundary,
            policy,
        };
    }
    let (decision, mode, reason) = match requested_mode {
        QueueMode::Interrupt if safe_boundary.can_interrupt() => {
            (QueueDecision::Interrupt, QueueMode::Interrupt, "safe_boundary_allows_interrupt")
        }
        QueueMode::Interrupt => {
            (QueueDecision::Defer, QueueMode::Collect, "interrupt_deferred_until_safe_boundary")
        }
        QueueMode::Steer if safe_boundary.can_steer() => {
            (QueueDecision::Steer, QueueMode::Steer, "safe_boundary_allows_steer")
        }
        QueueMode::Steer => {
            (QueueDecision::Defer, QueueMode::Collect, "steer_deferred_until_safe_boundary")
        }
        QueueMode::SteerBacklog => {
            (QueueDecision::SteerBacklog, QueueMode::SteerBacklog, "backlog_steering_requested")
        }
        QueueMode::Collect => (QueueDecision::Enqueue, QueueMode::Collect, "collect_requested"),
        QueueMode::Followup => (QueueDecision::Enqueue, QueueMode::Followup, "followup_requested"),
    };
    SessionQueueDecision {
        decision,
        mode,
        accepted: true,
        reason: reason.to_owned(),
        safe_boundary,
        policy,
    }
}

#[cfg(test)]
mod tests {
    use palyra_common::runtime_contracts::{QueueDecision, QueueMode};

    use crate::config::SessionQueuePolicyConfig;

    use super::{decide_session_queue_mode, SessionQueuePolicy, SessionQueueSafeBoundary};

    #[test]
    fn policy_maps_legacy_depth_and_merge_window_to_cap_and_debounce() {
        let mut config = SessionQueuePolicyConfig::default();
        config.max_depth = 12;
        config.merge_window_ms = 2_500;

        let policy =
            SessionQueuePolicy::from_config(&config, "session-1", Some("discord"), Some("agent-1"));

        assert_eq!(policy.cap, 12);
        assert_eq!(policy.debounce_ms, 2_500);
        assert_eq!(policy.priority_lane, "normal");
        assert_eq!(policy.drop_policy, "summarize_oldest");
        assert_eq!(policy.overflow_behavior, "deterministic_backlog_summary");
        assert_eq!(policy.coalescing_group, "agent:agent-1:session-1");
    }

    #[test]
    fn pending_approval_defers_steer_into_collect() {
        let policy = SessionQueuePolicy::from_config(
            &SessionQueuePolicyConfig::default(),
            "session-1",
            None,
            None,
        );
        let decision = decide_session_queue_mode(
            policy,
            Some(QueueMode::Steer),
            SessionQueueSafeBoundary::active(true, true),
            0,
        );

        assert_eq!(decision.decision, QueueDecision::Defer);
        assert_eq!(decision.mode, QueueMode::Collect);
        assert_eq!(decision.reason, "steer_deferred_until_safe_boundary");
        assert!(decision.safe_boundary.pending_approval);
    }

    #[test]
    fn queue_cap_switches_to_overflow_summary() {
        let mut config = SessionQueuePolicyConfig::default();
        config.max_depth = 2;
        let policy = SessionQueuePolicy::from_config(&config, "session-1", None, None);
        let decision = decide_session_queue_mode(
            policy,
            Some(QueueMode::Followup),
            SessionQueueSafeBoundary::active(true, false),
            2,
        );

        assert_eq!(decision.decision, QueueDecision::Overflow);
        assert_eq!(decision.mode, QueueMode::Collect);
        assert_eq!(decision.reason, "queue_cap_reached_overflow_summary_required");
    }
}
