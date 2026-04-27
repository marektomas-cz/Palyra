use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tool_protocol::{ToolCallConfig, ToolRequestContext};

pub(super) const TOOL_CATALOG_SCHEMA_VERSION: u32 = 1;
pub(super) const TOOL_REGISTRY_ENTRY_VERSION: u32 = 1;
pub(super) const TOOL_REJECTION_SCHEMA_VERSION: u32 = 1;
pub(super) const MAX_SCHEMA_DEPTH: usize = 8;
pub(super) const MAX_SCHEMA_PROPERTIES: usize = 128;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolSchemaDialect {
    OpenAiCompatible,
    Anthropic,
    Deterministic,
}

impl ToolSchemaDialect {
    pub(crate) fn from_provider_kind(provider_kind: &str) -> Self {
        match provider_kind.trim().to_ascii_lowercase().as_str() {
            "anthropic" => Self::Anthropic,
            "deterministic" => Self::Deterministic,
            _ => Self::OpenAiCompatible,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai_compatible",
            Self::Anthropic => "anthropic",
            Self::Deterministic => "deterministic",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolExposureSurface {
    RunStream,
    RouteMessage,
}

impl ToolExposureSurface {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::RunStream => "run_stream",
            Self::RouteMessage => "route_message",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolApprovalPosture {
    Safe,
    ApprovalRequired,
}

impl ToolApprovalPosture {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::ApprovalRequired => "approval_required",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolParallelismPolicy {
    ReadOnly,
    Idempotent,
    Exclusive,
}

impl ToolParallelismPolicy {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Idempotent => "idempotent",
            Self::Exclusive => "exclusive",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolResultProjectionPolicy {
    InlineUnlessLarge,
    SummarizeAndArtifact,
    RedactedPreviewAndArtifact,
}

impl ToolResultProjectionPolicy {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::InlineUnlessLarge => "inline_unless_large",
            Self::SummarizeAndArtifact => "summarize_and_artifact",
            Self::RedactedPreviewAndArtifact => "redacted_preview_and_artifact",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolCatalogFilterReasonCode {
    NotAllowlisted,
    UnknownTool,
    RuntimeUnavailable,
    ProviderSchemaIncompatible,
    SurfaceUnsupported,
    BudgetExhausted,
    PolicyInvisible,
}

impl ToolCatalogFilterReasonCode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NotAllowlisted => "not_allowlisted",
            Self::UnknownTool => "unknown_tool",
            Self::RuntimeUnavailable => "runtime_unavailable",
            Self::ProviderSchemaIncompatible => "provider_schema_incompatible",
            Self::SurfaceUnsupported => "surface_unsupported",
            Self::BudgetExhausted => "budget_exhausted",
            Self::PolicyInvisible => "policy_invisible",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ToolRegistryEntry {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) version: u32,
    pub(crate) provenance: String,
    pub(crate) input_schema: Value,
    pub(crate) schema_hash: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) approval_posture: ToolApprovalPosture,
    pub(crate) projection_policy: ToolResultProjectionPolicy,
    pub(crate) parallelism_policy: ToolParallelismPolicy,
    pub(crate) target_surfaces: Vec<ToolExposureSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ModelVisibleTool {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) version: u32,
    pub(crate) provenance: String,
    pub(crate) schema: Value,
    pub(crate) provider_schema: Value,
    pub(crate) internal_schema_hash: String,
    pub(crate) provider_schema_hash: String,
    pub(crate) description_hash: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) approval_posture: ToolApprovalPosture,
    pub(crate) projection_policy: ToolResultProjectionPolicy,
    pub(crate) parallelism_policy: ToolParallelismPolicy,
    pub(crate) exposure_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct FilteredToolCatalogEntry {
    pub(crate) name: String,
    pub(crate) reason_code: ToolCatalogFilterReasonCode,
    pub(crate) repair_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ModelVisibleToolCatalogSnapshot {
    pub(crate) schema_version: u32,
    pub(crate) snapshot_id: String,
    pub(crate) catalog_hash: String,
    pub(crate) provider_dialect: ToolSchemaDialect,
    pub(crate) provider_kind: String,
    pub(crate) provider_model_id: Option<String>,
    pub(crate) surface: ToolExposureSurface,
    pub(crate) principal_hash: String,
    pub(crate) channel_hash: Option<String>,
    pub(crate) remaining_tool_budget: u32,
    pub(crate) created_at_unix_ms: i64,
    pub(crate) tools: Vec<ModelVisibleTool>,
    pub(crate) filtered_tools: Vec<FilteredToolCatalogEntry>,
}

pub(crate) struct ToolCatalogBuildRequest<'a> {
    pub(crate) config: &'a ToolCallConfig,
    pub(crate) browser_service_enabled: bool,
    pub(crate) request_context: &'a ToolRequestContext,
    pub(crate) provider_kind: &'a str,
    pub(crate) provider_model_id: Option<&'a str>,
    pub(crate) surface: ToolExposureSurface,
    pub(crate) remaining_tool_budget: u32,
    pub(crate) created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ToolArgumentNormalizationStep {
    pub(crate) json_pointer: String,
    pub(crate) from_type: String,
    pub(crate) to_type: String,
    pub(crate) reason_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ToolArgumentNormalizationAudit {
    pub(crate) raw_json_hash: String,
    pub(crate) normalized_json_hash: String,
    pub(crate) steps: Vec<ToolArgumentNormalizationStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct NormalizedToolCall {
    pub(crate) input_json: Vec<u8>,
    pub(crate) audit: ToolArgumentNormalizationAudit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolCallRejectionKind {
    UnknownTool,
    UnavailableTool,
    MalformedArguments,
    SchemaInvalid,
    PolicyInvisible,
    UnsupportedParallelism,
}

impl ToolCallRejectionKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownTool => "unknown_tool",
            Self::UnavailableTool => "unavailable_tool",
            Self::MalformedArguments => "malformed_arguments",
            Self::SchemaInvalid => "schema_invalid",
            Self::PolicyInvisible => "policy_invisible",
            Self::UnsupportedParallelism => "unsupported_parallelism",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ToolCallRejection {
    pub(crate) schema_version: u32,
    pub(crate) kind: ToolCallRejectionKind,
    pub(crate) tool_name: String,
    pub(crate) reason_code: String,
    pub(crate) message: String,
    pub(crate) raw_json_hash: String,
    pub(crate) snapshot_id: Option<String>,
    pub(crate) catalog_hash: Option<String>,
}
