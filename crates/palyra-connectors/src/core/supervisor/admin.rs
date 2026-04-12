use std::sync::Arc;

use serde_json::{json, Map, Value};

use crate::providers::{provider_availability, provider_capabilities};

use super::super::{
    protocol::{
        ConnectorAvailability, ConnectorCapabilitySet, ConnectorInstanceSpec, ConnectorKind,
        ConnectorMessageDeleteRequest, ConnectorMessageEditRequest, ConnectorMessageMutationResult,
        ConnectorMessageReactionRequest, ConnectorMessageReadRequest, ConnectorMessageReadResult,
        ConnectorMessageSearchRequest, ConnectorMessageSearchResult, ConnectorStatusSnapshot,
    },
    storage::{
        ConnectorEventRecord, ConnectorInstanceRecord, ConnectorQueueSnapshot, DeadLetterRecord,
    },
};
use super::metrics::build_saturation_snapshot;
use super::{unix_ms_now, ConnectorAdapter, ConnectorSupervisor, ConnectorSupervisorError};

impl ConnectorSupervisor {
    pub fn register_connector(
        &self,
        spec: &ConnectorInstanceSpec,
    ) -> Result<(), ConnectorSupervisorError> {
        let now = unix_ms_now()?;
        self.store.upsert_instance(spec, now)?;
        self.store.record_event(
            spec.connector_id.as_str(),
            "connector.registered",
            "info",
            "connector instance registered",
            Some(&json!({
                "connector_id": spec.connector_id,
                "kind": spec.kind.as_str(),
                "availability": provider_availability(spec.kind).as_str(),
                "enabled": spec.enabled,
            })),
            now,
        )?;
        Ok(())
    }

    pub fn set_enabled(
        &self,
        connector_id: &str,
        enabled: bool,
    ) -> Result<ConnectorStatusSnapshot, ConnectorSupervisorError> {
        let now = unix_ms_now()?;
        self.store.set_instance_enabled(connector_id, enabled, now)?;
        self.store.record_event(
            connector_id,
            "connector.enabled_changed",
            "info",
            if enabled { "connector enabled" } else { "connector disabled" },
            Some(&json!({ "enabled": enabled })),
            now,
        )?;
        self.status(connector_id)
    }

    pub fn remove_connector(&self, connector_id: &str) -> Result<(), ConnectorSupervisorError> {
        let Some(_instance) = self.store.get_instance(connector_id)? else {
            return Err(ConnectorSupervisorError::NotFound(connector_id.to_owned()));
        };
        self.store.delete_instance(connector_id)?;
        Ok(())
    }

    pub fn status(
        &self,
        connector_id: &str,
    ) -> Result<ConnectorStatusSnapshot, ConnectorSupervisorError> {
        let Some(instance) = self.store.get_instance(connector_id)? else {
            return Err(ConnectorSupervisorError::NotFound(connector_id.to_owned()));
        };
        let queue_depth = self.store.queue_depth(connector_id)?;
        Ok(ConnectorStatusSnapshot {
            connector_id: instance.connector_id,
            kind: instance.kind,
            availability: self.connector_availability(instance.kind),
            capabilities: self.connector_capabilities(instance.kind),
            principal: instance.principal,
            enabled: instance.enabled,
            readiness: instance.readiness,
            liveness: instance.liveness,
            restart_count: instance.restart_count,
            queue_depth,
            last_error: instance.last_error,
            last_inbound_unix_ms: instance.last_inbound_unix_ms,
            last_outbound_unix_ms: instance.last_outbound_unix_ms,
            updated_at_unix_ms: instance.updated_at_unix_ms,
        })
    }

    pub fn list_status(&self) -> Result<Vec<ConnectorStatusSnapshot>, ConnectorSupervisorError> {
        let instances = self.store.list_instances()?;
        let mut snapshots = Vec::with_capacity(instances.len());
        for instance in instances {
            let queue_depth = self.store.queue_depth(instance.connector_id.as_str())?;
            snapshots.push(ConnectorStatusSnapshot {
                connector_id: instance.connector_id,
                kind: instance.kind,
                availability: self.connector_availability(instance.kind),
                capabilities: self.connector_capabilities(instance.kind),
                principal: instance.principal,
                enabled: instance.enabled,
                readiness: instance.readiness,
                liveness: instance.liveness,
                restart_count: instance.restart_count,
                queue_depth,
                last_error: instance.last_error,
                last_inbound_unix_ms: instance.last_inbound_unix_ms,
                last_outbound_unix_ms: instance.last_outbound_unix_ms,
                updated_at_unix_ms: instance.updated_at_unix_ms,
            });
        }
        Ok(snapshots)
    }

    pub fn runtime_snapshot(
        &self,
        connector_id: &str,
    ) -> Result<Option<Value>, ConnectorSupervisorError> {
        let Some(instance) = self.store.get_instance(connector_id)? else {
            return Err(ConnectorSupervisorError::NotFound(connector_id.to_owned()));
        };
        let queue = self.queue_snapshot(instance.connector_id.as_str())?;
        let adapter_runtime = self
            .adapters
            .get(&instance.kind)
            .and_then(|adapter| adapter.runtime_snapshot(&instance));
        let metrics = self.build_runtime_metrics(instance.connector_id.as_str())?;
        let mut runtime = match adapter_runtime {
            Some(Value::Object(object)) => Value::Object(object),
            Some(other) => {
                let mut object = Map::new();
                object.insert("adapter".to_owned(), other);
                Value::Object(object)
            }
            None => Value::Object(Map::new()),
        };
        if let Some(object) = runtime.as_object_mut() {
            object.insert("metrics".to_owned(), json!(metrics));
            object.insert("queue".to_owned(), json!(queue));
            object.insert("saturation".to_owned(), json!(build_saturation_snapshot(&queue)));
        }
        Ok(Some(runtime))
    }

    pub async fn read_messages(
        &self,
        connector_id: &str,
        request: &ConnectorMessageReadRequest,
    ) -> Result<ConnectorMessageReadResult, ConnectorSupervisorError> {
        request
            .validate()
            .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
        let (instance, adapter) = self.instance_and_adapter(connector_id)?;
        let result = adapter
            .read_messages(&instance, request)
            .await
            .map_err(|error| ConnectorSupervisorError::Adapter(error.to_string()))?;
        result
            .validate()
            .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
        self.record_message_admin_event(
            instance.connector_id.as_str(),
            result.preflight.allowed,
            result.preflight.audit_event_type.as_str(),
            "connector message read completed",
            Some(&json!({
                "policy_action": result.preflight.policy_action,
                "approval_mode": result.preflight.approval_mode,
                "risk_level": result.preflight.risk_level,
                "reason": result.preflight.reason,
                "conversation_id": result.target.conversation_id,
                "thread_id": result.target.thread_id,
                "exact_message_id": result.exact_message_id,
                "messages_returned": result.messages.len(),
                "next_before_message_id": result.next_before_message_id,
                "next_after_message_id": result.next_after_message_id,
            })),
        )?;
        Ok(result)
    }

    pub async fn search_messages(
        &self,
        connector_id: &str,
        request: &ConnectorMessageSearchRequest,
    ) -> Result<ConnectorMessageSearchResult, ConnectorSupervisorError> {
        request
            .validate()
            .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
        let (instance, adapter) = self.instance_and_adapter(connector_id)?;
        let result = adapter
            .search_messages(&instance, request)
            .await
            .map_err(|error| ConnectorSupervisorError::Adapter(error.to_string()))?;
        result
            .validate()
            .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
        self.record_message_admin_event(
            instance.connector_id.as_str(),
            result.preflight.allowed,
            result.preflight.audit_event_type.as_str(),
            "connector message search completed",
            Some(&json!({
                "policy_action": result.preflight.policy_action,
                "approval_mode": result.preflight.approval_mode,
                "risk_level": result.preflight.risk_level,
                "reason": result.preflight.reason,
                "conversation_id": result.target.conversation_id,
                "thread_id": result.target.thread_id,
                "query": result.query,
                "author_id": result.author_id,
                "has_attachments": result.has_attachments,
                "matches_returned": result.matches.len(),
                "next_before_message_id": result.next_before_message_id,
            })),
        )?;
        Ok(result)
    }

    pub async fn edit_message(
        &self,
        connector_id: &str,
        request: &ConnectorMessageEditRequest,
    ) -> Result<ConnectorMessageMutationResult, ConnectorSupervisorError> {
        request
            .validate()
            .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
        let (instance, adapter) = self.instance_and_adapter(connector_id)?;
        let result = adapter
            .edit_message(&instance, request)
            .await
            .map_err(|error| ConnectorSupervisorError::Adapter(error.to_string()))?;
        self.validate_and_record_mutation_result(
            instance.connector_id.as_str(),
            &result,
            "connector message edit completed",
        )?;
        Ok(result)
    }

    pub async fn delete_message(
        &self,
        connector_id: &str,
        request: &ConnectorMessageDeleteRequest,
    ) -> Result<ConnectorMessageMutationResult, ConnectorSupervisorError> {
        request
            .validate()
            .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
        let (instance, adapter) = self.instance_and_adapter(connector_id)?;
        let result = adapter
            .delete_message(&instance, request)
            .await
            .map_err(|error| ConnectorSupervisorError::Adapter(error.to_string()))?;
        self.validate_and_record_mutation_result(
            instance.connector_id.as_str(),
            &result,
            "connector message delete completed",
        )?;
        Ok(result)
    }

    pub async fn add_reaction(
        &self,
        connector_id: &str,
        request: &ConnectorMessageReactionRequest,
    ) -> Result<ConnectorMessageMutationResult, ConnectorSupervisorError> {
        request
            .validate()
            .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
        let (instance, adapter) = self.instance_and_adapter(connector_id)?;
        let result = adapter
            .add_reaction(&instance, request)
            .await
            .map_err(|error| ConnectorSupervisorError::Adapter(error.to_string()))?;
        self.validate_and_record_mutation_result(
            instance.connector_id.as_str(),
            &result,
            "connector reaction add completed",
        )?;
        Ok(result)
    }

    pub async fn remove_reaction(
        &self,
        connector_id: &str,
        request: &ConnectorMessageReactionRequest,
    ) -> Result<ConnectorMessageMutationResult, ConnectorSupervisorError> {
        request
            .validate()
            .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
        let (instance, adapter) = self.instance_and_adapter(connector_id)?;
        let result = adapter
            .remove_reaction(&instance, request)
            .await
            .map_err(|error| ConnectorSupervisorError::Adapter(error.to_string()))?;
        self.validate_and_record_mutation_result(
            instance.connector_id.as_str(),
            &result,
            "connector reaction removal completed",
        )?;
        Ok(result)
    }

    pub fn list_logs(
        &self,
        connector_id: &str,
        limit: usize,
    ) -> Result<Vec<ConnectorEventRecord>, ConnectorSupervisorError> {
        self.store.list_events(connector_id, limit).map_err(ConnectorSupervisorError::from)
    }

    pub fn list_dead_letters(
        &self,
        connector_id: &str,
        limit: usize,
    ) -> Result<Vec<DeadLetterRecord>, ConnectorSupervisorError> {
        self.store.list_dead_letters(connector_id, limit).map_err(ConnectorSupervisorError::from)
    }

    pub fn queue_snapshot(
        &self,
        connector_id: &str,
    ) -> Result<ConnectorQueueSnapshot, ConnectorSupervisorError> {
        let now = unix_ms_now()?;
        self.store.queue_snapshot(connector_id, now).map_err(ConnectorSupervisorError::from)
    }

    pub fn set_queue_paused(
        &self,
        connector_id: &str,
        paused: bool,
        reason: Option<&str>,
    ) -> Result<ConnectorQueueSnapshot, ConnectorSupervisorError> {
        let now = unix_ms_now()?;
        self.store.set_queue_paused(connector_id, paused, reason, now)?;
        self.store.record_event(
            connector_id,
            if paused { "queue.paused" } else { "queue.resumed" },
            "info",
            if paused { "connector outbox queue paused" } else { "connector outbox queue resumed" },
            Some(&json!({
                "paused": paused,
                "reason": reason,
            })),
            now,
        )?;
        self.queue_snapshot(connector_id)
    }

    pub fn replay_dead_letter(
        &self,
        connector_id: &str,
        dead_letter_id: i64,
    ) -> Result<DeadLetterRecord, ConnectorSupervisorError> {
        let now = unix_ms_now()?;
        let replayed = self.store.replay_dead_letter(
            connector_id,
            dead_letter_id,
            self.config.max_retry_attempts,
            now,
        )?;
        self.store.record_event(
            connector_id,
            "dead_letter.replayed",
            "info",
            "dead-letter entry replayed into outbox",
            Some(&json!({
                "dead_letter_id": dead_letter_id,
                "envelope_id": replayed.envelope_id,
            })),
            now,
        )?;
        Ok(replayed)
    }

    pub fn discard_dead_letter(
        &self,
        connector_id: &str,
        dead_letter_id: i64,
    ) -> Result<DeadLetterRecord, ConnectorSupervisorError> {
        let now = unix_ms_now()?;
        let discarded = self.store.discard_dead_letter(connector_id, dead_letter_id)?;
        self.store.record_event(
            connector_id,
            "dead_letter.discarded",
            "info",
            "dead-letter entry discarded by operator",
            Some(&json!({
                "dead_letter_id": dead_letter_id,
                "envelope_id": discarded.envelope_id,
            })),
            now,
        )?;
        Ok(discarded)
    }

    fn connector_availability(&self, kind: ConnectorKind) -> ConnectorAvailability {
        self.adapters
            .get(&kind)
            .map(|adapter| adapter.availability())
            .unwrap_or_else(|| provider_availability(kind))
    }

    fn connector_capabilities(&self, kind: ConnectorKind) -> ConnectorCapabilitySet {
        self.adapters
            .get(&kind)
            .map(|adapter| adapter.capabilities())
            .unwrap_or_else(|| provider_capabilities(kind))
    }

    fn instance_and_adapter(
        &self,
        connector_id: &str,
    ) -> Result<(ConnectorInstanceRecord, Arc<dyn ConnectorAdapter>), ConnectorSupervisorError>
    {
        let Some(instance) = self.store.get_instance(connector_id)? else {
            return Err(ConnectorSupervisorError::NotFound(connector_id.to_owned()));
        };
        let Some(adapter) = self.adapters.get(&instance.kind).cloned() else {
            return Err(ConnectorSupervisorError::MissingAdapter(instance.kind));
        };
        Ok((instance, adapter))
    }

    fn validate_and_record_mutation_result(
        &self,
        connector_id: &str,
        result: &ConnectorMessageMutationResult,
        message: &'static str,
    ) -> Result<(), ConnectorSupervisorError> {
        result
            .validate()
            .map_err(|error| ConnectorSupervisorError::Validation(error.to_string()))?;
        self.record_message_admin_event(
            connector_id,
            result.preflight.allowed,
            result.preflight.audit_event_type.as_str(),
            message,
            Some(&json!({
                "policy_action": result.preflight.policy_action,
                "approval_mode": result.preflight.approval_mode,
                "risk_level": result.preflight.risk_level,
                "preflight_reason": result.preflight.reason,
                "message_id": result.locator.message_id,
                "conversation_id": result.locator.target.conversation_id,
                "thread_id": result.locator.target.thread_id,
                "status": result.status,
                "result_reason": result.reason,
                "has_message": result.message.is_some(),
                "has_diff": result.diff.is_some(),
            })),
        )
    }

    fn record_message_admin_event(
        &self,
        connector_id: &str,
        allowed: bool,
        event_type: &str,
        message: &str,
        details: Option<&Value>,
    ) -> Result<(), ConnectorSupervisorError> {
        let now = unix_ms_now()?;
        self.store.record_event(
            connector_id,
            event_type,
            if allowed { "info" } else { "warn" },
            message,
            details,
            now,
        )?;
        Ok(())
    }
}
