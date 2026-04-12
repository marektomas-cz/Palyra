use serde::{Deserialize, Serialize};

use super::validation::{
    validate_json_bytes, validate_optional_field, ProtocolError, MAX_A2UI_PATCH_BYTES,
    MAX_A2UI_SURFACE_BYTES, MAX_ATTACHMENTS_PER_MESSAGE, MAX_ATTACHMENT_CONTENT_TYPE_BYTES,
    MAX_ATTACHMENT_FILENAME_BYTES, MAX_ATTACHMENT_HASH_BYTES, MAX_ATTACHMENT_ID_BYTES,
    MAX_ATTACHMENT_INLINE_BASE64_BYTES, MAX_ATTACHMENT_ORIGIN_BYTES,
    MAX_ATTACHMENT_POLICY_CONTEXT_BYTES, MAX_ATTACHMENT_REF_BYTES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Image,
    #[default]
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AttachmentRef {
    #[serde(default)]
    pub kind: AttachmentKind,
    #[serde(default)]
    pub attachment_id: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub artifact_ref: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub content_hash: Option<String>,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub policy_context: Option<String>,
    #[serde(default)]
    pub upload_requested: bool,
    #[serde(default)]
    pub inline_base64: Option<String>,
    #[serde(default)]
    pub width_px: Option<u32>,
    #[serde(default)]
    pub height_px: Option<u32>,
}

impl AttachmentRef {
    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
        validate_optional_field(
            self.attachment_id.as_deref(),
            "attachments.attachment_id",
            MAX_ATTACHMENT_ID_BYTES,
        )?;
        validate_optional_field(self.url.as_deref(), "attachments.url", MAX_ATTACHMENT_REF_BYTES)?;
        validate_optional_field(
            self.artifact_ref.as_deref(),
            "attachments.artifact_ref",
            MAX_ATTACHMENT_REF_BYTES,
        )?;
        validate_optional_field(
            self.filename.as_deref(),
            "attachments.filename",
            MAX_ATTACHMENT_FILENAME_BYTES,
        )?;
        validate_optional_field(
            self.content_type.as_deref(),
            "attachments.content_type",
            MAX_ATTACHMENT_CONTENT_TYPE_BYTES,
        )?;
        validate_optional_field(
            self.content_hash.as_deref(),
            "attachments.content_hash",
            MAX_ATTACHMENT_HASH_BYTES,
        )?;
        validate_optional_field(
            self.origin.as_deref(),
            "attachments.origin",
            MAX_ATTACHMENT_ORIGIN_BYTES,
        )?;
        validate_optional_field(
            self.policy_context.as_deref(),
            "attachments.policy_context",
            MAX_ATTACHMENT_POLICY_CONTEXT_BYTES,
        )?;
        validate_optional_field(
            self.inline_base64.as_deref(),
            "attachments.inline_base64",
            MAX_ATTACHMENT_INLINE_BASE64_BYTES,
        )?;
        if self.upload_requested
            && self
                .inline_base64
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
        {
            return Err(ProtocolError::InvalidField {
                field: "attachments.inline_base64",
                reason: "upload_requested attachments must include inline_base64",
            });
        }
        Ok(())
    }
}

pub(super) fn validate_attachments(attachments: &[AttachmentRef]) -> Result<(), ProtocolError> {
    if attachments.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(ProtocolError::InvalidField {
            field: "attachments",
            reason: "message exceeds attachment count limit",
        });
    }
    for attachment in attachments {
        attachment.validate()?;
    }
    Ok(())
}

pub type OutboundAttachment = AttachmentRef;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundA2uiUpdate {
    pub surface: String,
    #[serde(default)]
    pub patch_json: Vec<u8>,
}

impl OutboundA2uiUpdate {
    pub(crate) fn validate(&self, max_patch_bytes: usize) -> Result<(), ProtocolError> {
        super::validation::validate_non_empty_identifier(
            self.surface.as_str(),
            "a2ui_update.surface",
            MAX_A2UI_SURFACE_BYTES,
        )?;
        validate_json_bytes(
            self.patch_json.as_slice(),
            "a2ui_update.patch_json",
            max_patch_bytes.min(MAX_A2UI_PATCH_BYTES),
        )
    }
}
