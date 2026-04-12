use serde::{Deserialize, Serialize};

use super::{
    attachments::{validate_attachments, OutboundA2uiUpdate, OutboundAttachment},
    capabilities::ConnectorCapabilitySet,
    kinds::{ConnectorAvailability, ConnectorKind, ConnectorLiveness, ConnectorReadiness},
    validation::{
        validate_host_pattern, validate_json_bytes, validate_message_body,
        validate_non_empty_identifier, ProtocolError, MAX_CONNECTOR_ID_BYTES,
        MAX_CONNECTOR_PRINCIPAL_BYTES, MAX_CONVERSATION_ID_BYTES, MAX_ENVELOPE_ID_BYTES,
        MAX_IDENTITY_BYTES, MAX_STRUCTURED_OUTPUT_BYTES,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorInstanceSpec {
    pub connector_id: String,
    pub kind: ConnectorKind,
    pub principal: String,
    pub auth_profile_ref: Option<String>,
    pub token_vault_ref: Option<String>,
    pub egress_allowlist: Vec<String>,
    pub enabled: bool,
}

impl ConnectorInstanceSpec {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_non_empty_identifier(
            self.connector_id.as_str(),
            "connector_id",
            MAX_CONNECTOR_ID_BYTES,
        )?;
        validate_non_empty_identifier(
            self.principal.as_str(),
            "principal",
            MAX_CONNECTOR_PRINCIPAL_BYTES,
        )?;
        for host in &self.egress_allowlist {
            validate_host_pattern(host)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboundMessageEvent {
    pub envelope_id: String,
    pub connector_id: String,
    pub conversation_id: String,
    pub thread_id: Option<String>,
    pub sender_id: String,
    pub sender_display: Option<String>,
    pub body: String,
    pub adapter_message_id: Option<String>,
    pub adapter_thread_id: Option<String>,
    pub received_at_unix_ms: i64,
    pub is_direct_message: bool,
    pub requested_broadcast: bool,
    #[serde(default)]
    pub attachments: Vec<super::AttachmentRef>,
}

impl InboundMessageEvent {
    pub fn validate(&self, max_body_bytes: usize) -> Result<(), ProtocolError> {
        validate_non_empty_identifier(
            self.envelope_id.as_str(),
            "envelope_id",
            MAX_ENVELOPE_ID_BYTES,
        )?;
        validate_non_empty_identifier(
            self.connector_id.as_str(),
            "connector_id",
            MAX_CONNECTOR_ID_BYTES,
        )?;
        validate_non_empty_identifier(
            self.conversation_id.as_str(),
            "conversation_id",
            MAX_CONVERSATION_ID_BYTES,
        )?;
        validate_non_empty_identifier(self.sender_id.as_str(), "sender_id", MAX_IDENTITY_BYTES)?;
        validate_message_body(self.body.as_str(), max_body_bytes, "body")?;
        validate_attachments(self.attachments.as_slice())?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedOutboundMessage {
    pub text: String,
    pub thread_id: Option<String>,
    pub in_reply_to_message_id: Option<String>,
    pub broadcast: bool,
    pub auto_ack_text: Option<String>,
    pub auto_reaction: Option<String>,
    #[serde(default)]
    pub attachments: Vec<OutboundAttachment>,
    #[serde(default)]
    pub structured_json: Option<Vec<u8>>,
    #[serde(default)]
    pub a2ui_update: Option<OutboundA2uiUpdate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteInboundResult {
    pub accepted: bool,
    pub queued_for_retry: bool,
    pub decision_reason: String,
    pub outputs: Vec<RoutedOutboundMessage>,
    pub route_key: Option<String>,
    pub retry_attempt: u32,
    #[serde(default)]
    pub route_message_latency_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundMessageRequest {
    pub envelope_id: String,
    pub connector_id: String,
    pub conversation_id: String,
    pub reply_thread_id: Option<String>,
    pub in_reply_to_message_id: Option<String>,
    pub text: String,
    pub broadcast: bool,
    pub auto_ack_text: Option<String>,
    pub auto_reaction: Option<String>,
    #[serde(default)]
    pub attachments: Vec<OutboundAttachment>,
    #[serde(default)]
    pub structured_json: Option<Vec<u8>>,
    #[serde(default)]
    pub a2ui_update: Option<OutboundA2uiUpdate>,
    pub timeout_ms: u64,
    pub max_payload_bytes: usize,
}

impl OutboundMessageRequest {
    pub fn validate(&self, max_text_bytes: usize) -> Result<(), ProtocolError> {
        validate_non_empty_identifier(
            self.envelope_id.as_str(),
            "envelope_id",
            MAX_ENVELOPE_ID_BYTES,
        )?;
        validate_non_empty_identifier(
            self.connector_id.as_str(),
            "connector_id",
            MAX_CONNECTOR_ID_BYTES,
        )?;
        validate_non_empty_identifier(
            self.conversation_id.as_str(),
            "conversation_id",
            MAX_CONVERSATION_ID_BYTES,
        )?;
        validate_message_body(self.text.as_str(), max_text_bytes, "text")?;
        validate_attachments(self.attachments.as_slice())?;
        if self.timeout_ms == 0 {
            return Err(ProtocolError::InvalidField {
                field: "timeout_ms",
                reason: "must be greater than zero",
            });
        }
        if self.max_payload_bytes == 0 {
            return Err(ProtocolError::InvalidField {
                field: "max_payload_bytes",
                reason: "must be greater than zero",
            });
        }
        let max_payload_bytes = self.max_payload_bytes.min(max_text_bytes);
        if let Some(structured_json) = self.structured_json.as_deref() {
            validate_json_bytes(
                structured_json,
                "structured_json",
                max_payload_bytes.min(MAX_STRUCTURED_OUTPUT_BYTES),
            )?;
        }
        if let Some(update) = self.a2ui_update.as_ref() {
            update.validate(max_payload_bytes)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    RateLimit,
    TransientNetwork,
    ConnectorRestarting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeliveryOutcome {
    Delivered { native_message_id: String },
    Retry { class: RetryClass, reason: String, retry_after_ms: Option<u64> },
    PermanentFailure { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorQueueDepth {
    pub pending_outbox: u64,
    pub dead_letters: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorStatusSnapshot {
    pub connector_id: String,
    pub kind: ConnectorKind,
    pub availability: ConnectorAvailability,
    pub capabilities: ConnectorCapabilitySet,
    pub principal: String,
    pub enabled: bool,
    pub readiness: ConnectorReadiness,
    pub liveness: ConnectorLiveness,
    pub restart_count: u32,
    pub queue_depth: ConnectorQueueDepth,
    pub last_error: Option<String>,
    pub last_inbound_unix_ms: Option<i64>,
    pub last_outbound_unix_ms: Option<i64>,
    pub updated_at_unix_ms: i64,
}
