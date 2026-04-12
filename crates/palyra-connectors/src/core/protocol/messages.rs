use serde::{Deserialize, Serialize};

use super::{
    attachments::{validate_attachments, AttachmentRef},
    capabilities::ConnectorOperationPreflight,
    validation::{
        validate_message_body, validate_non_empty_identifier, validate_optional_field,
        ProtocolError, MAX_CONVERSATION_ID_BYTES, MAX_CURSOR_ID_BYTES, MAX_EMOJI_BYTES,
        MAX_ENVELOPE_ID_BYTES, MAX_IDENTITY_BYTES, MAX_MESSAGE_BYTES, MAX_MESSAGE_LINK_BYTES,
        MAX_OPERATION_REASON_BYTES, MAX_SEARCH_QUERY_BYTES,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorConversationTarget {
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

impl ConnectorConversationTarget {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        validate_non_empty_identifier(
            self.conversation_id.as_str(),
            "target.conversation_id",
            MAX_CONVERSATION_ID_BYTES,
        )?;
        validate_optional_field(
            self.thread_id.as_deref(),
            "target.thread_id",
            MAX_CONVERSATION_ID_BYTES,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorMessageLocator {
    #[serde(flatten)]
    pub target: ConnectorConversationTarget,
    pub message_id: String,
}

impl ConnectorMessageLocator {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.target.validate()?;
        validate_non_empty_identifier(
            self.message_id.as_str(),
            "message_id",
            MAX_ENVELOPE_ID_BYTES,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorMessageReadRequest {
    #[serde(flatten)]
    pub target: ConnectorConversationTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub around_message_id: Option<String>,
    pub limit: usize,
}

impl ConnectorMessageReadRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.target.validate()?;
        validate_optional_field(self.message_id.as_deref(), "message_id", MAX_CURSOR_ID_BYTES)?;
        validate_optional_field(
            self.before_message_id.as_deref(),
            "before_message_id",
            MAX_CURSOR_ID_BYTES,
        )?;
        validate_optional_field(
            self.after_message_id.as_deref(),
            "after_message_id",
            MAX_CURSOR_ID_BYTES,
        )?;
        validate_optional_field(
            self.around_message_id.as_deref(),
            "around_message_id",
            MAX_CURSOR_ID_BYTES,
        )?;
        if self.limit == 0 {
            return Err(ProtocolError::InvalidField {
                field: "limit",
                reason: "must be greater than zero",
            });
        }
        if self.message_id.is_some()
            && (self.before_message_id.is_some()
                || self.after_message_id.is_some()
                || self.around_message_id.is_some())
        {
            return Err(ProtocolError::InvalidField {
                field: "message_id",
                reason: "exact message fetch cannot be combined with pagination cursors",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorMessageSearchRequest {
    #[serde(flatten)]
    pub target: ConnectorConversationTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_attachments: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_message_id: Option<String>,
    pub limit: usize,
}

impl ConnectorMessageSearchRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.target.validate()?;
        validate_optional_field(self.query.as_deref(), "query", MAX_SEARCH_QUERY_BYTES)?;
        validate_optional_field(self.author_id.as_deref(), "author_id", MAX_IDENTITY_BYTES)?;
        validate_optional_field(
            self.before_message_id.as_deref(),
            "before_message_id",
            MAX_CURSOR_ID_BYTES,
        )?;
        if self.limit == 0 {
            return Err(ProtocolError::InvalidField {
                field: "limit",
                reason: "must be greater than zero",
            });
        }
        if self.query.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_none()
            && self.author_id.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_none()
            && self.has_attachments.is_none()
        {
            return Err(ProtocolError::InvalidField {
                field: "query",
                reason: "search requires at least one filter",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorMessageEditRequest {
    pub locator: ConnectorMessageLocator,
    pub body: String,
}

impl ConnectorMessageEditRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.locator.validate()?;
        validate_message_body(self.body.as_str(), MAX_MESSAGE_BYTES, "body")?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorMessageDeleteRequest {
    pub locator: ConnectorMessageLocator,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl ConnectorMessageDeleteRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.locator.validate()?;
        validate_optional_field(self.reason.as_deref(), "reason", MAX_OPERATION_REASON_BYTES)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorMessageReactionRequest {
    pub locator: ConnectorMessageLocator,
    pub emoji: String,
}

impl ConnectorMessageReactionRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.locator.validate()?;
        validate_non_empty_identifier(self.emoji.as_str(), "emoji", MAX_EMOJI_BYTES)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConnectorMessageReactionRecord {
    pub emoji: String,
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub reacted_by_connector: bool,
}

impl ConnectorMessageReactionRecord {
    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
        validate_non_empty_identifier(self.emoji.as_str(), "reactions.emoji", MAX_EMOJI_BYTES)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorMessageRecord {
    pub locator: ConnectorMessageLocator,
    pub sender_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender_display: Option<String>,
    pub body: String,
    pub created_at_unix_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at_unix_ms: Option<i64>,
    #[serde(default)]
    pub is_direct_message: bool,
    #[serde(default)]
    pub is_connector_authored: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default)]
    pub attachments: Vec<AttachmentRef>,
    #[serde(default)]
    pub reactions: Vec<ConnectorMessageReactionRecord>,
}

impl ConnectorMessageRecord {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.locator.validate()?;
        validate_non_empty_identifier(self.sender_id.as_str(), "sender_id", MAX_IDENTITY_BYTES)?;
        validate_optional_field(
            self.sender_display.as_deref(),
            "sender_display",
            MAX_IDENTITY_BYTES,
        )?;
        if self.body.len() > MAX_MESSAGE_BYTES {
            return Err(ProtocolError::InvalidField {
                field: "body",
                reason: "message body exceeds size limit",
            });
        }
        validate_optional_field(self.link.as_deref(), "link", MAX_MESSAGE_LINK_BYTES)?;
        validate_attachments(self.attachments.as_slice())?;
        for reaction in &self.reactions {
            reaction.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorMessageReadResult {
    pub preflight: ConnectorOperationPreflight,
    #[serde(flatten)]
    pub target: ConnectorConversationTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_message_id: Option<String>,
    #[serde(default)]
    pub messages: Vec<ConnectorMessageRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_before_message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_after_message_id: Option<String>,
}

impl ConnectorMessageReadResult {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.preflight.validate()?;
        self.target.validate()?;
        validate_optional_field(
            self.exact_message_id.as_deref(),
            "exact_message_id",
            MAX_CURSOR_ID_BYTES,
        )?;
        validate_optional_field(
            self.next_before_message_id.as_deref(),
            "next_before_message_id",
            MAX_CURSOR_ID_BYTES,
        )?;
        validate_optional_field(
            self.next_after_message_id.as_deref(),
            "next_after_message_id",
            MAX_CURSOR_ID_BYTES,
        )?;
        for message in &self.messages {
            message.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorMessageSearchResult {
    pub preflight: ConnectorOperationPreflight,
    #[serde(flatten)]
    pub target: ConnectorConversationTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_attachments: Option<bool>,
    #[serde(default)]
    pub matches: Vec<ConnectorMessageRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_before_message_id: Option<String>,
}

impl ConnectorMessageSearchResult {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.preflight.validate()?;
        self.target.validate()?;
        validate_optional_field(self.query.as_deref(), "query", MAX_SEARCH_QUERY_BYTES)?;
        validate_optional_field(self.author_id.as_deref(), "author_id", MAX_IDENTITY_BYTES)?;
        validate_optional_field(
            self.next_before_message_id.as_deref(),
            "next_before_message_id",
            MAX_CURSOR_ID_BYTES,
        )?;
        for message in &self.matches {
            message.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorMessageMutationStatus {
    Updated,
    Deleted,
    ReactionAdded,
    ReactionRemoved,
    Denied,
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConnectorMessageMutationDiff {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_body: Option<String>,
}

impl ConnectorMessageMutationDiff {
    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
        validate_optional_field(
            self.before_body.as_deref(),
            "diff.before_body",
            MAX_MESSAGE_BYTES,
        )?;
        validate_optional_field(self.after_body.as_deref(), "diff.after_body", MAX_MESSAGE_BYTES)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorMessageMutationResult {
    pub preflight: ConnectorOperationPreflight,
    pub locator: ConnectorMessageLocator,
    pub status: ConnectorMessageMutationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<ConnectorMessageRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<ConnectorMessageMutationDiff>,
}

impl ConnectorMessageMutationResult {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        self.preflight.validate()?;
        self.locator.validate()?;
        validate_optional_field(self.reason.as_deref(), "reason", MAX_OPERATION_REASON_BYTES)?;
        if let Some(message) = self.message.as_ref() {
            message.validate()?;
        }
        if let Some(diff) = self.diff.as_ref() {
            diff.validate()?;
        }
        Ok(())
    }
}
