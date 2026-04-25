use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::providers::{provider_availability, provider_capabilities};

use super::super::storage::ConnectorStoreError;
use super::super::{
    protocol::{
        ConnectorAvailability, ConnectorCapabilitySet, ConnectorKind,
        ConnectorMessageDeleteRequest, ConnectorMessageEditRequest, ConnectorMessageMutationResult,
        ConnectorMessageReactionRequest, ConnectorMessageReadRequest, ConnectorMessageReadResult,
        ConnectorMessageSearchRequest, ConnectorMessageSearchResult, ConnectorReadiness,
        DeliveryOutcome, InboundMessageEvent, OutboundMessageRequest, RetryClass,
        RouteInboundResult,
    },
    storage::ConnectorInstanceRecord,
};

#[derive(Debug, Clone)]
pub struct ConnectorSupervisorConfig {
    pub inbound_dedupe_window_ms: i64,
    pub max_inbound_body_bytes: usize,
    pub max_outbound_body_bytes: usize,
    pub max_retry_attempts: u32,
    pub min_retry_delay_ms: u64,
    pub base_retry_delay_ms: u64,
    pub max_retry_delay_ms: u64,
    pub disabled_poll_delay_ms: u64,
    pub immediate_drain_batch_size: usize,
    pub background_drain_batch_size: usize,
}

impl Default for ConnectorSupervisorConfig {
    fn default() -> Self {
        Self {
            inbound_dedupe_window_ms: 7 * 24 * 60 * 60 * 1_000,
            max_inbound_body_bytes: 64 * 1024,
            max_outbound_body_bytes: 64 * 1024,
            max_retry_attempts: 5,
            min_retry_delay_ms: 250,
            base_retry_delay_ms: 1_000,
            max_retry_delay_ms: 60_000,
            disabled_poll_delay_ms: 30_000,
            immediate_drain_batch_size: 64,
            background_drain_batch_size: 128,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DispatchResult {
    Delivered,
    Retried,
    DeadLettered,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DrainOutcome {
    pub processed: usize,
    pub delivered: usize,
    pub retried: usize,
    pub dead_lettered: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct InboundIngestOutcome {
    pub accepted: bool,
    pub duplicate: bool,
    pub queued_for_retry: bool,
    pub decision_reason: String,
    pub route_key: Option<String>,
    pub enqueued_outbound: usize,
    pub immediate_delivery: usize,
}

#[derive(Debug, Error)]
pub enum ConnectorRouterError {
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Error)]
pub enum ConnectorAdapterError {
    #[error("{0}")]
    Backend(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorAdapterSdkOperation {
    Inbound,
    Outbound,
    Binding,
    RateLimit,
    ErrorMapping,
}

impl ConnectorAdapterSdkOperation {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
            Self::Binding => "binding",
            Self::RateLimit => "rate_limit",
            Self::ErrorMapping => "error_mapping",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorAdapterSdkDescriptor {
    pub schema_version: u32,
    pub kind: ConnectorKind,
    pub operations: Vec<ConnectorAdapterSdkOperation>,
    pub binding_contract: String,
    pub delivery_receipt_contract: String,
    pub error_contract: String,
}

impl ConnectorAdapterSdkDescriptor {
    #[must_use]
    pub fn for_kind(kind: ConnectorKind) -> Self {
        Self {
            schema_version: 1,
            kind,
            operations: vec![
                ConnectorAdapterSdkOperation::Inbound,
                ConnectorAdapterSdkOperation::Outbound,
                ConnectorAdapterSdkOperation::Binding,
                ConnectorAdapterSdkOperation::RateLimit,
                ConnectorAdapterSdkOperation::ErrorMapping,
            ],
            binding_contract: "conversation_binding_record".to_owned(),
            delivery_receipt_contract: "ack_nack_unknown_v1".to_owned(),
            error_contract: "connector_adapter_error_v1".to_owned(),
        }
    }
}

#[async_trait]
pub trait ConnectorRouter: Send + Sync {
    async fn route_inbound(
        &self,
        principal: &str,
        event: &InboundMessageEvent,
    ) -> Result<RouteInboundResult, ConnectorRouterError>;
}

#[async_trait]
pub trait ConnectorAdapter: Send + Sync {
    fn kind(&self) -> ConnectorKind;

    fn sdk_descriptor(&self) -> ConnectorAdapterSdkDescriptor {
        ConnectorAdapterSdkDescriptor::for_kind(self.kind())
    }

    fn availability(&self) -> ConnectorAvailability {
        provider_availability(self.kind())
    }

    fn capabilities(&self) -> ConnectorCapabilitySet {
        provider_capabilities(self.kind())
    }

    fn split_outbound(
        &self,
        _instance: &ConnectorInstanceRecord,
        request: &OutboundMessageRequest,
    ) -> Result<Vec<OutboundMessageRequest>, ConnectorAdapterError> {
        Ok(vec![request.clone()])
    }

    fn runtime_snapshot(&self, _instance: &ConnectorInstanceRecord) -> Option<Value> {
        None
    }

    async fn poll_inbound(
        &self,
        _instance: &ConnectorInstanceRecord,
        _limit: usize,
    ) -> Result<Vec<InboundMessageEvent>, ConnectorAdapterError> {
        Ok(Vec::new())
    }

    async fn send_outbound(
        &self,
        instance: &ConnectorInstanceRecord,
        request: &OutboundMessageRequest,
    ) -> Result<DeliveryOutcome, ConnectorAdapterError>;

    async fn read_messages(
        &self,
        _instance: &ConnectorInstanceRecord,
        _request: &ConnectorMessageReadRequest,
    ) -> Result<ConnectorMessageReadResult, ConnectorAdapterError> {
        Err(ConnectorAdapterError::Backend(format!(
            "{} connector does not support message read",
            self.kind().as_str()
        )))
    }

    async fn search_messages(
        &self,
        _instance: &ConnectorInstanceRecord,
        _request: &ConnectorMessageSearchRequest,
    ) -> Result<ConnectorMessageSearchResult, ConnectorAdapterError> {
        Err(ConnectorAdapterError::Backend(format!(
            "{} connector does not support message search",
            self.kind().as_str()
        )))
    }

    async fn edit_message(
        &self,
        _instance: &ConnectorInstanceRecord,
        _request: &ConnectorMessageEditRequest,
    ) -> Result<ConnectorMessageMutationResult, ConnectorAdapterError> {
        Err(ConnectorAdapterError::Backend(format!(
            "{} connector does not support message edit",
            self.kind().as_str()
        )))
    }

    async fn delete_message(
        &self,
        _instance: &ConnectorInstanceRecord,
        _request: &ConnectorMessageDeleteRequest,
    ) -> Result<ConnectorMessageMutationResult, ConnectorAdapterError> {
        Err(ConnectorAdapterError::Backend(format!(
            "{} connector does not support message delete",
            self.kind().as_str()
        )))
    }

    async fn add_reaction(
        &self,
        _instance: &ConnectorInstanceRecord,
        _request: &ConnectorMessageReactionRequest,
    ) -> Result<ConnectorMessageMutationResult, ConnectorAdapterError> {
        Err(ConnectorAdapterError::Backend(format!(
            "{} connector does not support reaction add",
            self.kind().as_str()
        )))
    }

    async fn remove_reaction(
        &self,
        _instance: &ConnectorInstanceRecord,
        _request: &ConnectorMessageReactionRequest,
    ) -> Result<ConnectorMessageMutationResult, ConnectorAdapterError> {
        Err(ConnectorAdapterError::Backend(format!(
            "{} connector does not support reaction removal",
            self.kind().as_str()
        )))
    }
}

#[derive(Debug, Error)]
pub enum ConnectorSupervisorError {
    #[error(transparent)]
    Store(#[from] ConnectorStoreError),
    #[error("connector protocol validation failed: {0}")]
    Validation(String),
    #[error("connector instance not found: {0}")]
    NotFound(String),
    #[error("connector adapter missing for kind '{0}'")]
    MissingAdapter(ConnectorKind),
    #[error("router failed: {0}")]
    Router(String),
    #[error("adapter failed: {0}")]
    Adapter(String),
    #[error("failed to read system clock: {0}")]
    Clock(String),
}

pub(super) fn classify_permanent_failure(reason: &str) -> ConnectorReadiness {
    let normalized = reason.trim().to_ascii_lowercase();
    if normalized.contains("credential missing") || normalized.contains("missing credential") {
        return ConnectorReadiness::MissingCredential;
    }
    if normalized.contains("auth")
        || normalized.contains("token")
        || normalized.contains("unauthorized")
        || normalized.contains("forbidden")
    {
        return ConnectorReadiness::AuthFailed;
    }
    ConnectorReadiness::Misconfigured
}

pub(super) fn retry_class_label(class: RetryClass) -> String {
    format!("{class:?}")
}
