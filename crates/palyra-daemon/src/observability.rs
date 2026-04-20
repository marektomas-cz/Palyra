use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use palyra_common::runtime_preview::{
    RuntimeDecisionEventType, RuntimeDecisionPayload, ALL_RUNTIME_DECISION_EVENT_TYPES,
};
use serde::Serialize;

const RECENT_FAILURE_LIMIT: usize = 24;
const RECENT_RUNTIME_DECISION_LIMIT: usize = 24;

#[derive(Debug, Clone, Default, Serialize)]
pub struct CorrelationSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onboarding_flow_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    #[serde(rename = "product_failure")]
    Product,
    #[serde(rename = "config_failure")]
    Config,
    #[serde(rename = "upstream_provider_failure")]
    UpstreamProvider,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailureSnapshot {
    pub operation: String,
    pub failure_class: FailureClass,
    pub message: String,
    pub observed_at_unix_ms: i64,
    pub correlation: CorrelationSnapshot,
}

#[derive(Debug, Default)]
pub struct ObservabilityState {
    provider_auth_attempts: AtomicU64,
    provider_auth_failures: AtomicU64,
    provider_refresh_failures: AtomicU64,
    dashboard_mutation_attempts: AtomicU64,
    dashboard_mutation_successes: AtomicU64,
    dashboard_mutation_failures: AtomicU64,
    support_bundle_exports_started: AtomicU64,
    support_bundle_exports_succeeded: AtomicU64,
    support_bundle_exports_failed: AtomicU64,
    recent_failures: Mutex<VecDeque<FailureSnapshot>>,
    runtime_decisions: Mutex<RuntimeDecisionTelemetryState>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CounterSnapshot {
    pub attempts: u64,
    pub successes: u64,
    pub failures: u64,
    pub failure_rate_bps: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDecisionCatalogEntry {
    pub event_type: String,
    pub journal_event: String,
    pub label: String,
    pub summary: String,
    pub emitted: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_at_unix_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDecisionEventSample {
    pub event_type: String,
    pub journal_event: String,
    pub actor_kind: String,
    pub principal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    pub reason: String,
    pub policy_id: String,
    pub observed_at_unix_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_state: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDecisionMetricsSnapshot {
    pub queue_depth: u64,
    pub queue_peak_depth: u64,
    pub pruning_apply_events: u64,
    pub pruning_tokens_saved: u64,
    pub retrieval_searches: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval_branch_latency_avg_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval_branch_latency_max_ms: Option<u64>,
    pub auxiliary_task_events: u64,
    pub auxiliary_budget_tokens: u64,
    pub flow_retries: u64,
    pub arbitration_suppressions: u64,
    pub worker_events: u64,
    pub worker_orphaned_events: u64,
    pub worker_orphan_rate_bps: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeDecisionObservabilitySnapshot {
    pub state: String,
    pub catalog: Vec<RuntimeDecisionCatalogEntry>,
    pub metrics: RuntimeDecisionMetricsSnapshot,
    pub recent_events: Vec<RuntimeDecisionEventSample>,
}

#[derive(Debug, Default)]
struct RuntimeDecisionTelemetryState {
    event_counts: BTreeMap<String, u64>,
    last_seen_at_unix_ms: BTreeMap<String, i64>,
    recent_events: VecDeque<RuntimeDecisionEventSample>,
    queue_depth: u64,
    queue_peak_depth: u64,
    pruning_apply_events: u64,
    pruning_tokens_saved: u64,
    retrieval_searches: u64,
    retrieval_branch_latency_total_ms: u64,
    retrieval_branch_latency_max_ms: u64,
    auxiliary_task_events: u64,
    auxiliary_budget_tokens: u64,
    flow_retries: u64,
    arbitration_suppressions: u64,
    worker_events: u64,
    worker_orphaned_events: u64,
}

impl ObservabilityState {
    pub fn record_provider_auth_attempt(&self) {
        self.provider_auth_attempts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_provider_auth_failure(
        &self,
        operation: impl Into<String>,
        failure_class: FailureClass,
        message: impl Into<String>,
        observed_at_unix_ms: i64,
        correlation: CorrelationSnapshot,
    ) {
        self.provider_auth_failures.fetch_add(1, Ordering::Relaxed);
        self.push_failure(FailureSnapshot {
            operation: operation.into(),
            failure_class,
            message: message.into(),
            observed_at_unix_ms,
            correlation,
        });
    }

    pub fn record_provider_refresh_failure(
        &self,
        operation: impl Into<String>,
        failure_class: FailureClass,
        message: impl Into<String>,
        observed_at_unix_ms: i64,
        correlation: CorrelationSnapshot,
    ) {
        self.provider_refresh_failures.fetch_add(1, Ordering::Relaxed);
        self.push_failure(FailureSnapshot {
            operation: operation.into(),
            failure_class,
            message: message.into(),
            observed_at_unix_ms,
            correlation,
        });
    }

    pub fn record_dashboard_mutation_result(
        &self,
        success: bool,
        operation: impl Into<String>,
        failure_class: FailureClass,
        message: impl Into<String>,
        observed_at_unix_ms: i64,
        correlation: CorrelationSnapshot,
    ) {
        self.dashboard_mutation_attempts.fetch_add(1, Ordering::Relaxed);
        if success {
            self.dashboard_mutation_successes.fetch_add(1, Ordering::Relaxed);
        } else {
            self.dashboard_mutation_failures.fetch_add(1, Ordering::Relaxed);
            self.push_failure(FailureSnapshot {
                operation: operation.into(),
                failure_class,
                message: message.into(),
                observed_at_unix_ms,
                correlation,
            });
        }
    }

    pub fn record_support_bundle_export_started(&self) {
        self.support_bundle_exports_started.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_support_bundle_export_result(
        &self,
        success: bool,
        operation: impl Into<String>,
        message: impl Into<String>,
        observed_at_unix_ms: i64,
        correlation: CorrelationSnapshot,
    ) {
        if success {
            self.support_bundle_exports_succeeded.fetch_add(1, Ordering::Relaxed);
        } else {
            self.support_bundle_exports_failed.fetch_add(1, Ordering::Relaxed);
            self.push_failure(FailureSnapshot {
                operation: operation.into(),
                failure_class: FailureClass::Product,
                message: message.into(),
                observed_at_unix_ms,
                correlation,
            });
        }
    }

    pub fn provider_auth_snapshot(&self) -> CounterSnapshot {
        let attempts = self.provider_auth_attempts.load(Ordering::Relaxed);
        let failures = self.provider_auth_failures.load(Ordering::Relaxed);
        CounterSnapshot {
            attempts,
            successes: attempts.saturating_sub(failures),
            failures,
            failure_rate_bps: ratio_bps(failures, attempts),
        }
    }

    pub fn dashboard_mutation_snapshot(&self) -> CounterSnapshot {
        let attempts = self.dashboard_mutation_attempts.load(Ordering::Relaxed);
        let successes = self.dashboard_mutation_successes.load(Ordering::Relaxed);
        let failures = self.dashboard_mutation_failures.load(Ordering::Relaxed);
        CounterSnapshot {
            attempts,
            successes,
            failures,
            failure_rate_bps: ratio_bps(failures, attempts),
        }
    }

    pub fn support_bundle_snapshot(&self) -> CounterSnapshot {
        let attempts = self.support_bundle_exports_started.load(Ordering::Relaxed);
        let successes = self.support_bundle_exports_succeeded.load(Ordering::Relaxed);
        let failures = self.support_bundle_exports_failed.load(Ordering::Relaxed);
        CounterSnapshot {
            attempts,
            successes,
            failures,
            failure_rate_bps: ratio_bps(failures, attempts),
        }
    }

    pub fn provider_refresh_failures(&self) -> u64 {
        self.provider_refresh_failures.load(Ordering::Relaxed)
    }

    pub fn recent_failures(&self) -> Vec<FailureSnapshot> {
        self.recent_failures
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .cloned()
            .collect()
    }

    pub fn record_runtime_decision_event(&self, payload: &RuntimeDecisionPayload) {
        let mut guard = self.runtime_decisions.lock().unwrap_or_else(|error| error.into_inner());
        let event_key = payload.event_type.as_str().to_owned();
        *guard.event_counts.entry(event_key.clone()).or_default() += 1;
        guard.last_seen_at_unix_ms.insert(event_key, payload.timing.observed_at_unix_ms);
        guard.push_recent_event(RuntimeDecisionEventSample {
            event_type: payload.event_type.as_str().to_owned(),
            journal_event: payload.event_type.journal_event().to_owned(),
            actor_kind: payload.actor.kind.as_str().to_owned(),
            principal: payload.actor.principal.clone(),
            channel: payload.actor.channel.clone(),
            reason: payload.reason.clone(),
            policy_id: payload.policy_id.clone(),
            observed_at_unix_ms: payload.timing.observed_at_unix_ms,
            input_kind: payload.input.as_ref().map(|value| value.kind.clone()),
            input_id: payload.input.as_ref().map(|value| value.id.clone()),
            output_kind: payload.output.as_ref().map(|value| value.kind.clone()),
            output_id: payload.output.as_ref().map(|value| value.id.clone()),
            output_state: payload.output.as_ref().and_then(|value| value.state.clone()),
        });
        match payload.event_type {
            RuntimeDecisionEventType::PruningApply => {
                guard.pruning_apply_events = guard.pruning_apply_events.saturating_add(1);
                guard.pruning_tokens_saved = guard.pruning_tokens_saved.saturating_add(
                    payload.resource_budget.pruning_token_delta.unwrap_or_default(),
                );
            }
            RuntimeDecisionEventType::RecallSessionSearch => {
                guard.retrieval_searches = guard.retrieval_searches.saturating_add(1);
                if let Some(latency_ms) = payload.resource_budget.retrieval_branch_latency_ms {
                    guard.retrieval_branch_latency_total_ms =
                        guard.retrieval_branch_latency_total_ms.saturating_add(latency_ms);
                    guard.retrieval_branch_latency_max_ms =
                        guard.retrieval_branch_latency_max_ms.max(latency_ms);
                }
            }
            RuntimeDecisionEventType::AuxiliaryTaskLifecycle => {
                guard.auxiliary_task_events = guard.auxiliary_task_events.saturating_add(1);
                guard.auxiliary_budget_tokens = guard
                    .auxiliary_budget_tokens
                    .saturating_add(payload.resource_budget.token_budget.unwrap_or_default());
            }
            RuntimeDecisionEventType::FlowLifecycle => {
                guard.flow_retries = guard.flow_retries.saturating_add(u64::from(
                    payload.resource_budget.retry_count.unwrap_or_default(),
                ));
            }
            RuntimeDecisionEventType::DeliveryArbitration => {
                guard.arbitration_suppressions = guard
                    .arbitration_suppressions
                    .saturating_add(payload.resource_budget.suppression_count.unwrap_or_default());
            }
            RuntimeDecisionEventType::WorkerLeaseLifecycle => {
                guard.worker_events = guard.worker_events.saturating_add(1);
                if payload.output.as_ref().and_then(|value| value.state.as_deref())
                    == Some("orphaned")
                {
                    guard.worker_orphaned_events = guard.worker_orphaned_events.saturating_add(1);
                }
            }
            RuntimeDecisionEventType::QueueEnqueue
            | RuntimeDecisionEventType::QueueMerge
            | RuntimeDecisionEventType::QueueSteer
            | RuntimeDecisionEventType::QueueInterrupt
            | RuntimeDecisionEventType::QueueOverflow => {}
        }
    }

    pub fn observe_runtime_queue_depth(&self, queue_depth: u64) {
        let mut guard = self.runtime_decisions.lock().unwrap_or_else(|error| error.into_inner());
        guard.queue_depth = queue_depth;
        guard.queue_peak_depth = guard.queue_peak_depth.max(queue_depth);
    }

    pub fn runtime_decision_snapshot(&self) -> RuntimeDecisionObservabilitySnapshot {
        let guard = self.runtime_decisions.lock().unwrap_or_else(|error| error.into_inner());
        let catalog = ALL_RUNTIME_DECISION_EVENT_TYPES
            .into_iter()
            .map(|event_type| RuntimeDecisionCatalogEntry {
                event_type: event_type.as_str().to_owned(),
                journal_event: event_type.journal_event().to_owned(),
                label: event_type.label().to_owned(),
                summary: event_type.summary().to_owned(),
                emitted: guard.event_counts.get(event_type.as_str()).copied().unwrap_or_default(),
                last_seen_at_unix_ms: guard.last_seen_at_unix_ms.get(event_type.as_str()).copied(),
            })
            .collect::<Vec<_>>();
        let retrieval_branch_latency_avg_ms = if guard.retrieval_searches == 0 {
            None
        } else {
            Some(guard.retrieval_branch_latency_total_ms / guard.retrieval_searches)
        };
        RuntimeDecisionObservabilitySnapshot {
            state: if guard.event_counts.is_empty() {
                "contract_ready".to_owned()
            } else {
                "active".to_owned()
            },
            catalog,
            metrics: RuntimeDecisionMetricsSnapshot {
                queue_depth: guard.queue_depth,
                queue_peak_depth: guard.queue_peak_depth,
                pruning_apply_events: guard.pruning_apply_events,
                pruning_tokens_saved: guard.pruning_tokens_saved,
                retrieval_searches: guard.retrieval_searches,
                retrieval_branch_latency_avg_ms,
                retrieval_branch_latency_max_ms: (guard.retrieval_branch_latency_max_ms > 0)
                    .then_some(guard.retrieval_branch_latency_max_ms),
                auxiliary_task_events: guard.auxiliary_task_events,
                auxiliary_budget_tokens: guard.auxiliary_budget_tokens,
                flow_retries: guard.flow_retries,
                arbitration_suppressions: guard.arbitration_suppressions,
                worker_events: guard.worker_events,
                worker_orphaned_events: guard.worker_orphaned_events,
                worker_orphan_rate_bps: ratio_bps(
                    guard.worker_orphaned_events,
                    guard.worker_events,
                ),
            },
            recent_events: guard.recent_events.iter().cloned().collect(),
        }
    }

    fn push_failure(&self, failure: FailureSnapshot) {
        let mut guard = self.recent_failures.lock().unwrap_or_else(|error| error.into_inner());
        guard.push_front(failure);
        while guard.len() > RECENT_FAILURE_LIMIT {
            guard.pop_back();
        }
    }
}

impl RuntimeDecisionTelemetryState {
    fn push_recent_event(&mut self, event: RuntimeDecisionEventSample) {
        self.recent_events.push_front(event);
        while self.recent_events.len() > RECENT_RUNTIME_DECISION_LIMIT {
            self.recent_events.pop_back();
        }
    }
}

fn ratio_bps(numerator: u64, denominator: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    let scaled = numerator.saturating_mul(10_000) / denominator;
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::ObservabilityState;
    use palyra_common::runtime_preview::{
        RuntimeDecisionActor, RuntimeDecisionActorKind, RuntimeDecisionEventType,
        RuntimeDecisionPayload, RuntimeDecisionTiming, RuntimeEntityRef, RuntimeResourceBudget,
    };

    #[test]
    fn runtime_decision_snapshot_tracks_catalog_metrics_and_recent_events() {
        let state = ObservabilityState::default();
        state.observe_runtime_queue_depth(2);
        state.record_runtime_decision_event(
            &RuntimeDecisionPayload::new(
                RuntimeDecisionEventType::QueueEnqueue,
                RuntimeDecisionActor::new(
                    RuntimeDecisionActorKind::Operator,
                    "admin:web-console",
                    "device-1",
                    Some("web".to_owned()),
                ),
                "queued_followup_submitted",
                "session_queue.preview.followup",
                RuntimeDecisionTiming::observed(1_730_000_000_000),
            )
            .with_input(RuntimeEntityRef::new("queued_input", "queued_input", "Q1"))
            .with_output(RuntimeEntityRef::new("run", "run", "R1").with_state("active")),
        );
        state.record_runtime_decision_event(
            &RuntimeDecisionPayload::new(
                RuntimeDecisionEventType::PruningApply,
                RuntimeDecisionActor::new(
                    RuntimeDecisionActorKind::System,
                    "system:compaction",
                    "daemon",
                    Some("system".to_owned()),
                ),
                "token_tape_cap_reached",
                "pruning.preview.session_compaction",
                RuntimeDecisionTiming::observed_with_duration(1_730_000_000_100, 42),
            )
            .with_output(RuntimeEntityRef::new("checkpoint", "checkpoint", "CHK1"))
            .with_resource_budget(RuntimeResourceBudget {
                queue_depth: None,
                token_budget: None,
                pruning_token_delta: Some(128),
                retrieval_branch_latency_ms: None,
                retry_count: None,
                suppression_count: None,
            }),
        );
        state.record_runtime_decision_event(
            &RuntimeDecisionPayload::new(
                RuntimeDecisionEventType::WorkerLeaseLifecycle,
                RuntimeDecisionActor::new(
                    RuntimeDecisionActorKind::Worker,
                    "system:networked-worker",
                    "networked-worker",
                    Some("system".to_owned()),
                ),
                "worker.orphaned",
                "networked_workers.lease.preview",
                RuntimeDecisionTiming::observed(1_730_000_000_200),
            )
            .with_output(
                RuntimeEntityRef::new("worker", "worker", "worker-01").with_state("orphaned"),
            ),
        );

        let snapshot = state.runtime_decision_snapshot();
        assert_eq!(snapshot.state, "active");
        assert_eq!(snapshot.metrics.queue_depth, 2);
        assert_eq!(snapshot.metrics.queue_peak_depth, 2);
        assert_eq!(snapshot.metrics.pruning_apply_events, 1);
        assert_eq!(snapshot.metrics.pruning_tokens_saved, 128);
        assert_eq!(snapshot.metrics.worker_events, 1);
        assert_eq!(snapshot.metrics.worker_orphaned_events, 1);
        assert_eq!(snapshot.metrics.worker_orphan_rate_bps, 10_000);
        assert!(
            snapshot
                .catalog
                .iter()
                .any(|entry| entry.event_type == "queue_enqueue" && entry.emitted == 1),
            "queue enqueue should appear in the runtime preview catalog"
        );
        assert_eq!(snapshot.recent_events.len(), 3);
    }
}
