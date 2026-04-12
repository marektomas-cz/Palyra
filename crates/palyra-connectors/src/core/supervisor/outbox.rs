use serde_json::json;

use super::super::{
    protocol::{ConnectorLiveness, ConnectorReadiness, DeliveryOutcome, RetryClass},
    storage::OutboxEntryRecord,
};
use super::types::{classify_permanent_failure, retry_class_label, DispatchResult};
use super::{unix_ms_now, ConnectorSupervisor, ConnectorSupervisorError, DrainOutcome};

impl ConnectorSupervisor {
    pub async fn drain_due_outbox(
        &self,
        limit: usize,
    ) -> Result<DrainOutcome, ConnectorSupervisorError> {
        let now = unix_ms_now()?;
        let entries = self.store.load_due_outbox(now, limit, None, false)?;
        self.process_due_entries(entries).await
    }

    pub async fn drain_due_outbox_for_connector(
        &self,
        connector_id: &str,
        limit: usize,
    ) -> Result<DrainOutcome, ConnectorSupervisorError> {
        let now = unix_ms_now()?;
        let entries = self.store.load_due_outbox(now, limit, Some(connector_id), false)?;
        self.process_due_entries(entries).await
    }

    pub async fn drain_due_outbox_for_connector_force(
        &self,
        connector_id: &str,
        limit: usize,
    ) -> Result<DrainOutcome, ConnectorSupervisorError> {
        let now = unix_ms_now()?;
        let entries = self.store.load_due_outbox(now, limit, Some(connector_id), true)?;
        self.process_due_entries(entries).await
    }

    async fn process_due_entries(
        &self,
        entries: Vec<OutboxEntryRecord>,
    ) -> Result<DrainOutcome, ConnectorSupervisorError> {
        let mut outcome = DrainOutcome::default();
        for entry in entries {
            outcome.processed = outcome.processed.saturating_add(1);
            match self.dispatch_outbox_entry(entry).await? {
                DispatchResult::Delivered => {
                    outcome.delivered = outcome.delivered.saturating_add(1);
                }
                DispatchResult::Retried => {
                    outcome.retried = outcome.retried.saturating_add(1);
                }
                DispatchResult::DeadLettered => {
                    outcome.dead_lettered = outcome.dead_lettered.saturating_add(1);
                }
            }
        }
        Ok(outcome)
    }

    async fn dispatch_outbox_entry(
        &self,
        entry: OutboxEntryRecord,
    ) -> Result<DispatchResult, ConnectorSupervisorError> {
        let now = unix_ms_now()?;
        let Some(instance) = self.store.get_instance(entry.connector_id.as_str())? else {
            self.store.move_outbox_to_dead_letter(
                entry.outbox_id,
                entry.claim_token.as_str(),
                "connector instance not found",
                now,
            )?;
            return Ok(DispatchResult::DeadLettered);
        };
        if !instance.enabled {
            let retry_at = now.saturating_add(
                i64::try_from(self.config.disabled_poll_delay_ms).unwrap_or(i64::MAX),
            );
            self.store.schedule_outbox_retry(
                entry.outbox_id,
                entry.claim_token.as_str(),
                entry.attempts,
                "connector disabled",
                retry_at,
            )?;
            return Ok(DispatchResult::Retried);
        }

        let Some(adapter) = self.adapters.get(&instance.kind).cloned() else {
            self.store.move_outbox_to_dead_letter(
                entry.outbox_id,
                entry.claim_token.as_str(),
                "connector adapter implementation missing",
                now,
            )?;
            self.store.record_event(
                instance.connector_id.as_str(),
                "outbox.dead_letter",
                "error",
                "connector adapter implementation missing",
                Some(&json!({
                    "kind": instance.kind.as_str(),
                    "envelope_id": entry.envelope_id,
                })),
                now,
            )?;
            return Ok(DispatchResult::DeadLettered);
        };

        let delivery = match adapter.send_outbound(&instance, &entry.payload).await {
            Ok(outcome) => outcome,
            Err(error) => {
                self.store.record_event(
                    instance.connector_id.as_str(),
                    "outbox.adapter_error",
                    "warn",
                    "adapter delivery call failed; scheduling retry",
                    Some(&json!({
                        "envelope_id": entry.envelope_id,
                        "error": error.to_string(),
                    })),
                    now,
                )?;
                DeliveryOutcome::Retry {
                    class: RetryClass::TransientNetwork,
                    reason: error.to_string(),
                    retry_after_ms: None,
                }
            }
        };
        self.apply_delivery_outcome(&instance, &entry, delivery, now).await
    }

    async fn apply_delivery_outcome(
        &self,
        instance: &super::super::storage::ConnectorInstanceRecord,
        entry: &OutboxEntryRecord,
        delivery: DeliveryOutcome,
        now_unix_ms: i64,
    ) -> Result<DispatchResult, ConnectorSupervisorError> {
        match delivery {
            DeliveryOutcome::Delivered { native_message_id } => {
                self.store.mark_outbox_delivered(
                    entry.outbox_id,
                    entry.claim_token.as_str(),
                    native_message_id.as_str(),
                    now_unix_ms,
                )?;
                self.store.record_last_outbound(instance.connector_id.as_str(), now_unix_ms)?;
                self.store.record_event(
                    instance.connector_id.as_str(),
                    "outbox.delivered",
                    "info",
                    "outbound message delivered",
                    Some(&json!({
                        "envelope_id": entry.envelope_id,
                        "native_message_id": native_message_id,
                    })),
                    now_unix_ms,
                )?;
                Ok(DispatchResult::Delivered)
            }
            DeliveryOutcome::Retry { class, reason, retry_after_ms } => {
                let attempts = entry.attempts.saturating_add(1);
                let max_attempts = entry.max_attempts.min(self.config.max_retry_attempts).max(1);
                if attempts >= max_attempts {
                    self.store.move_outbox_to_dead_letter(
                        entry.outbox_id,
                        entry.claim_token.as_str(),
                        reason.as_str(),
                        now_unix_ms,
                    )?;
                    self.store.record_event(
                        instance.connector_id.as_str(),
                        "outbox.dead_letter",
                        "warn",
                        "retry budget exhausted; moved to dead letter",
                        Some(&json!({
                            "envelope_id": entry.envelope_id,
                            "attempts": attempts,
                            "reason": reason,
                            "retry_class": retry_class_label(class),
                        })),
                        now_unix_ms,
                    )?;
                    return Ok(DispatchResult::DeadLettered);
                }

                let delay_ms = self.retry_delay_ms(attempts, retry_after_ms);
                let next_attempt_unix_ms =
                    now_unix_ms.saturating_add(i64::try_from(delay_ms).unwrap_or(i64::MAX));
                self.store.schedule_outbox_retry(
                    entry.outbox_id,
                    entry.claim_token.as_str(),
                    attempts,
                    reason.as_str(),
                    next_attempt_unix_ms,
                )?;
                if matches!(class, RetryClass::ConnectorRestarting) {
                    self.store.increment_restart_count(
                        instance.connector_id.as_str(),
                        now_unix_ms,
                        reason.as_str(),
                    )?;
                } else {
                    self.store.set_instance_runtime_state(
                        instance.connector_id.as_str(),
                        ConnectorReadiness::Ready,
                        ConnectorLiveness::Running,
                        Some(reason.as_str()),
                        now_unix_ms,
                    )?;
                }
                self.store.record_event(
                    instance.connector_id.as_str(),
                    "outbox.retry",
                    "warn",
                    "connector delivery requested retry",
                    Some(&json!({
                        "envelope_id": entry.envelope_id,
                        "attempts": attempts,
                        "next_attempt_unix_ms": next_attempt_unix_ms,
                        "reason": reason,
                        "retry_class": retry_class_label(class),
                    })),
                    now_unix_ms,
                )?;
                Ok(DispatchResult::Retried)
            }
            DeliveryOutcome::PermanentFailure { reason } => {
                self.store.move_outbox_to_dead_letter(
                    entry.outbox_id,
                    entry.claim_token.as_str(),
                    reason.as_str(),
                    now_unix_ms,
                )?;
                let readiness = classify_permanent_failure(reason.as_str());
                self.store.set_instance_runtime_state(
                    instance.connector_id.as_str(),
                    readiness,
                    ConnectorLiveness::Running,
                    Some(reason.as_str()),
                    now_unix_ms,
                )?;
                self.store.record_event(
                    instance.connector_id.as_str(),
                    "outbox.dead_letter",
                    "warn",
                    "connector delivery returned permanent failure",
                    Some(&json!({
                        "envelope_id": entry.envelope_id,
                        "reason": reason,
                    })),
                    now_unix_ms,
                )?;
                Ok(DispatchResult::DeadLettered)
            }
        }
    }

    fn retry_delay_ms(&self, attempts: u32, requested_retry_after_ms: Option<u64>) -> u64 {
        let exponent = attempts.saturating_sub(1).min(10);
        let exponential = self
            .config
            .base_retry_delay_ms
            .saturating_mul(1_u64 << exponent)
            .min(self.config.max_retry_delay_ms);
        requested_retry_after_ms
            .unwrap_or(exponential)
            .max(self.config.min_retry_delay_ms)
            .min(self.config.max_retry_delay_ms)
    }
}
