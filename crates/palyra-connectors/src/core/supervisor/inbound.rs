use serde_json::json;

use super::super::{
    protocol::{InboundMessageEvent, OutboundMessageRequest},
    storage::OutboxEnqueueOutcome,
};
use super::{unix_ms_now, ConnectorSupervisor, ConnectorSupervisorError};

impl ConnectorSupervisor {
    pub fn enqueue_outbound(
        &self,
        request: &OutboundMessageRequest,
    ) -> Result<OutboxEnqueueOutcome, ConnectorSupervisorError> {
        request
            .validate(self.config.max_outbound_body_bytes)
            .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
        let now = unix_ms_now()?;
        let Some(instance) = self.store.get_instance(request.connector_id.as_str())? else {
            return Err(ConnectorSupervisorError::NotFound(request.connector_id.clone()));
        };
        let outcome =
            self.store.enqueue_outbox_if_absent(request, self.config.max_retry_attempts, now)?;
        if outcome.created {
            self.store.record_event(
                instance.connector_id.as_str(),
                "outbox.enqueued",
                "info",
                "outbound message queued by direct enqueue operation",
                Some(&json!({
                    "envelope_id": request.envelope_id,
                    "conversation_id": request.conversation_id,
                    "text_bytes": request.text.len(),
                })),
                now,
            )?;
        }
        Ok(outcome)
    }

    pub async fn ingest_inbound(
        &self,
        event: InboundMessageEvent,
    ) -> Result<super::InboundIngestOutcome, ConnectorSupervisorError> {
        event
            .validate(self.config.max_inbound_body_bytes)
            .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
        let now = unix_ms_now()?;
        let Some(instance) = self.store.get_instance(event.connector_id.as_str())? else {
            return Err(ConnectorSupervisorError::NotFound(event.connector_id));
        };
        if !instance.enabled {
            self.store.record_event(
                instance.connector_id.as_str(),
                "inbound.rejected",
                "warn",
                "inbound message dropped because connector is disabled",
                Some(&json!({
                    "envelope_id": event.envelope_id,
                })),
                now,
            )?;
            return Ok(super::InboundIngestOutcome {
                accepted: false,
                duplicate: false,
                queued_for_retry: false,
                decision_reason: "connector_disabled".to_owned(),
                route_key: None,
                enqueued_outbound: 0,
                immediate_delivery: 0,
            });
        }

        let is_new = self.store.record_inbound_dedupe_if_new(
            instance.connector_id.as_str(),
            event.envelope_id.as_str(),
            now,
            self.config.inbound_dedupe_window_ms,
        )?;
        if !is_new {
            self.store.record_event(
                instance.connector_id.as_str(),
                "inbound.duplicate",
                "info",
                "inbound duplicate ignored by dedupe window",
                Some(&json!({
                    "envelope_id": event.envelope_id,
                })),
                now,
            )?;
            return Ok(super::InboundIngestOutcome {
                accepted: true,
                duplicate: true,
                queued_for_retry: false,
                decision_reason: "duplicate_envelope".to_owned(),
                route_key: None,
                enqueued_outbound: 0,
                immediate_delivery: 0,
            });
        }
        self.store.record_last_inbound(instance.connector_id.as_str(), now)?;
        self.store.record_event(
            instance.connector_id.as_str(),
            "inbound.received",
            "info",
            "inbound event accepted by supervisor",
            Some(&json!({
                "envelope_id": event.envelope_id,
                "conversation_id": event.conversation_id,
                "is_direct_message": event.is_direct_message,
                "requested_broadcast": event.requested_broadcast,
            })),
            now,
        )?;

        let routed = self
            .router
            .route_inbound(instance.principal.as_str(), &event)
            .await
            .map_err(|error| ConnectorSupervisorError::Router(error.to_string()))?;
        if !routed.accepted {
            self.store.record_event(
                instance.connector_id.as_str(),
                "inbound.not_routed",
                if routed.queued_for_retry { "warn" } else { "info" },
                routed.decision_reason.as_str(),
                Some(&json!({
                    "envelope_id": event.envelope_id,
                    "queued_for_retry": routed.queued_for_retry,
                    "retry_attempt": routed.retry_attempt,
                    "route_message_latency_ms": routed.route_message_latency_ms,
                })),
                now,
            )?;
            return Ok(super::InboundIngestOutcome {
                accepted: false,
                duplicate: false,
                queued_for_retry: routed.queued_for_retry,
                decision_reason: routed.decision_reason,
                route_key: routed.route_key,
                enqueued_outbound: 0,
                immediate_delivery: 0,
            });
        }
        self.store.record_event(
            instance.connector_id.as_str(),
            "inbound.routed",
            "info",
            "inbound event routed to gateway",
            Some(&json!({
                "envelope_id": event.envelope_id,
                "route_key": routed.route_key.clone(),
                "outputs": routed.outputs.len(),
                "retry_attempt": routed.retry_attempt,
                "route_message_latency_ms": routed.route_message_latency_ms,
            })),
            now,
        )?;

        let mut enqueued_outbound = 0usize;
        for (index, output) in routed.outputs.iter().enumerate() {
            let base_request = OutboundMessageRequest {
                envelope_id: format!("{}:{index}", event.envelope_id),
                connector_id: instance.connector_id.clone(),
                conversation_id: event.conversation_id.clone(),
                reply_thread_id: output.thread_id.clone(),
                in_reply_to_message_id: output.in_reply_to_message_id.clone(),
                text: output.text.clone(),
                broadcast: output.broadcast,
                auto_ack_text: output.auto_ack_text.clone(),
                auto_reaction: output.auto_reaction.clone(),
                attachments: output.attachments.clone(),
                structured_json: output.structured_json.clone(),
                a2ui_update: output.a2ui_update.clone(),
                timeout_ms: 30_000,
                max_payload_bytes: self.config.max_outbound_body_bytes,
            };
            base_request
                .validate(self.config.max_outbound_body_bytes)
                .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;

            let split_requests = if let Some(adapter) = self.adapters.get(&instance.kind) {
                adapter
                    .split_outbound(&instance, &base_request)
                    .map_err(|error| ConnectorSupervisorError::Adapter(error.to_string()))?
            } else {
                vec![base_request]
            };
            if split_requests.is_empty() {
                continue;
            }
            for request in split_requests {
                request
                    .validate(self.config.max_outbound_body_bytes)
                    .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
                let enqueue = self.store.enqueue_outbox_if_absent(
                    &request,
                    self.config.max_retry_attempts,
                    now,
                )?;
                if enqueue.created {
                    enqueued_outbound = enqueued_outbound.saturating_add(1);
                    self.store.record_event(
                        instance.connector_id.as_str(),
                        "outbox.enqueued",
                        "info",
                        "outbound response queued for connector delivery",
                        Some(&json!({
                            "envelope_id": request.envelope_id,
                            "text_bytes": request.text.len(),
                        })),
                        now,
                    )?;
                }
            }
        }

        let drain = self
            .drain_due_outbox_for_connector(
                instance.connector_id.as_str(),
                self.config.immediate_drain_batch_size,
            )
            .await?;
        Ok(super::InboundIngestOutcome {
            accepted: true,
            duplicate: false,
            queued_for_retry: false,
            decision_reason: "routed".to_owned(),
            route_key: routed.route_key,
            enqueued_outbound,
            immediate_delivery: drain.delivered,
        })
    }

    pub async fn poll_inbound(
        &self,
        per_connector_limit: usize,
    ) -> Result<usize, ConnectorSupervisorError> {
        let limit = per_connector_limit.max(1);
        let instances = self.store.list_instances()?;
        let mut processed = 0_usize;

        for instance in instances {
            if !instance.enabled {
                continue;
            }
            let Some(adapter) = self.adapters.get(&instance.kind) else {
                continue;
            };
            let inbound = match adapter.poll_inbound(&instance, limit).await {
                Ok(inbound) => inbound,
                Err(error) => {
                    let now = unix_ms_now()?;
                    self.store.record_event(
                        instance.connector_id.as_str(),
                        "inbound.poll_error",
                        "warn",
                        "adapter inbound poll failed; continuing with remaining connectors",
                        Some(&json!({
                            "error": error.to_string(),
                        })),
                        now,
                    )?;
                    continue;
                }
            };
            for event in inbound {
                self.ingest_inbound(event).await?;
                processed = processed.saturating_add(1);
            }
        }

        Ok(processed)
    }
}
