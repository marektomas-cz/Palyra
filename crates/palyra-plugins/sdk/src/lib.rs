//! Bootstrap SDK placeholder for Palyra plugin authors.
//!
//! The initial WIT contract is defined in `wit/palyra-sdk.wit`.

use serde::{Deserialize, Serialize};

/// WIT package identifier for the bootstrap plugin SDK contract.
pub const WIT_PACKAGE_ID: &str = "palyra:plugins/sdk@0.1.0";
/// WIT world exported by plugin modules.
pub const WIT_WORLD_NAME: &str = "palyra-plugin";
/// Core Wasm import module that exposes Tier A capability handles.
pub const HOST_CAPABILITIES_IMPORT_MODULE: &str = "palyra:plugins/host-capabilities@0.1.0";

/// Host function names defined by the Tier A capability contract.
pub const HOST_CAPABILITY_HTTP_COUNT_FN: &str = "http-count";
pub const HOST_CAPABILITY_HTTP_HANDLE_FN: &str = "http-handle";
pub const HOST_CAPABILITY_SECRET_COUNT_FN: &str = "secret-count";
pub const HOST_CAPABILITY_SECRET_HANDLE_FN: &str = "secret-handle";
pub const HOST_CAPABILITY_STORAGE_COUNT_FN: &str = "storage-count";
pub const HOST_CAPABILITY_STORAGE_HANDLE_FN: &str = "storage-handle";
pub const HOST_CAPABILITY_CHANNEL_COUNT_FN: &str = "channel-count";
pub const HOST_CAPABILITY_CHANNEL_HANDLE_FN: &str = "channel-handle";

/// Default plugin entrypoint exported by the runtime interface.
pub const DEFAULT_RUNTIME_ENTRYPOINT: &str = "run";
/// Source of truth WIT document embedded for tooling/tests.
pub const WIT_SOURCE: &str = include_str!("../wit/palyra-sdk.wit");
/// Initial contract version for typed plugin extension points.
pub const DEFAULT_TYPED_PLUGIN_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TypedPluginContractKind {
    MemoryProvider,
    ContextEngine,
    RoutingStrategy,
}

impl TypedPluginContractKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemoryProvider => "memory_provider",
            Self::ContextEngine => "context_engine",
            Self::RoutingStrategy => "routing_strategy",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TypedPluginCapabilityClass {
    HttpHosts,
    Secrets,
    StoragePrefixes,
    Channels,
}

impl TypedPluginCapabilityClass {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HttpHosts => "http_hosts",
            Self::Secrets => "secrets",
            Self::StoragePrefixes => "storage_prefixes",
            Self::Channels => "channels",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TypedPluginContractLifecyclePhase {
    Discover,
    Negotiate,
    Bind,
    Invoke,
    Audit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TypedPluginContractDeclaration {
    pub kind: TypedPluginContractKind,
    #[serde(default = "default_typed_plugin_contract_version")]
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TypedPluginContractDescriptor {
    pub kind: TypedPluginContractKind,
    pub version: u32,
    #[serde(default)]
    pub lifecycle: Vec<TypedPluginContractLifecyclePhase>,
    #[serde(default)]
    pub allowed_capability_classes: Vec<TypedPluginCapabilityClass>,
    #[serde(default)]
    pub error_codes: Vec<String>,
    #[serde(default)]
    pub redacted_fields: Vec<String>,
    #[serde(default)]
    pub audit_hooks: Vec<String>,
    #[serde(default)]
    pub observability_hooks: Vec<String>,
}

/// Returns the WIT package identifier.
#[must_use]
pub fn wit_package_id() -> &'static str {
    WIT_PACKAGE_ID
}

/// Returns embedded WIT source text.
#[must_use]
pub fn wit_source() -> &'static str {
    WIT_SOURCE
}

/// Returns the default version for typed plugin contracts.
#[must_use]
pub fn default_typed_plugin_contract_version() -> u32 {
    DEFAULT_TYPED_PLUGIN_CONTRACT_VERSION
}

/// Returns the built-in descriptor for a typed plugin contract version supported by the host.
#[must_use]
pub fn typed_plugin_contract_descriptor(
    kind: TypedPluginContractKind,
    version: u32,
) -> Option<TypedPluginContractDescriptor> {
    if version != DEFAULT_TYPED_PLUGIN_CONTRACT_VERSION {
        return None;
    }

    let lifecycle = vec![
        TypedPluginContractLifecyclePhase::Discover,
        TypedPluginContractLifecyclePhase::Negotiate,
        TypedPluginContractLifecyclePhase::Bind,
        TypedPluginContractLifecyclePhase::Invoke,
        TypedPluginContractLifecyclePhase::Audit,
    ];
    let allowed_capability_classes = vec![
        TypedPluginCapabilityClass::HttpHosts,
        TypedPluginCapabilityClass::Secrets,
        TypedPluginCapabilityClass::StoragePrefixes,
    ];

    let descriptor = match kind {
        TypedPluginContractKind::MemoryProvider => TypedPluginContractDescriptor {
            kind,
            version,
            lifecycle,
            allowed_capability_classes,
            error_codes: vec![
                "contract_negotiation_failed".to_owned(),
                "embed_request_failed".to_owned(),
                "invalid_embedding_dimensions".to_owned(),
                "degraded_fallback_active".to_owned(),
            ],
            redacted_fields: vec![
                "config.credentials".to_owned(),
                "request.input_text".to_owned(),
                "response.embedding_preview".to_owned(),
            ],
            audit_hooks: vec![
                "binding.negotiated".to_owned(),
                "memory.embedding_runtime".to_owned(),
                "memory.embedding_failure".to_owned(),
            ],
            observability_hooks: vec![
                "memory.embedding_runtime".to_owned(),
                "memory.embedding_backfill".to_owned(),
                "memory.embedding_degradation".to_owned(),
            ],
        },
        TypedPluginContractKind::ContextEngine => TypedPluginContractDescriptor {
            kind,
            version,
            lifecycle,
            allowed_capability_classes,
            error_codes: vec![
                "contract_negotiation_failed".to_owned(),
                "context_plan_failed".to_owned(),
                "segment_budget_exceeded".to_owned(),
                "segment_redaction_required".to_owned(),
            ],
            redacted_fields: vec![
                "context.segment.preview".to_owned(),
                "context.compaction.summary".to_owned(),
                "context.references.inline".to_owned(),
            ],
            audit_hooks: vec![
                "binding.negotiated".to_owned(),
                "context.plan".to_owned(),
                "context.segment_drop".to_owned(),
            ],
            observability_hooks: vec![
                "context.plan".to_owned(),
                "context.compaction".to_owned(),
                "context.recall_mix".to_owned(),
            ],
        },
        TypedPluginContractKind::RoutingStrategy => TypedPluginContractDescriptor {
            kind,
            version,
            lifecycle,
            allowed_capability_classes,
            error_codes: vec![
                "contract_negotiation_failed".to_owned(),
                "routing_decision_failed".to_owned(),
                "budget_gate_blocked".to_owned(),
                "provider_selection_rejected".to_owned(),
            ],
            redacted_fields: vec![
                "routing.prompt_preview".to_owned(),
                "routing.provider_credentials".to_owned(),
                "routing.override_payload".to_owned(),
            ],
            audit_hooks: vec![
                "binding.negotiated".to_owned(),
                "routing.decision".to_owned(),
                "routing.budget_gate".to_owned(),
            ],
            observability_hooks: vec![
                "routing.decision".to_owned(),
                "routing.model_mix".to_owned(),
                "routing.provider_health".to_owned(),
            ],
        },
    };
    Some(descriptor)
}

/// Returns the full set of built-in typed plugin contracts supported by the host.
#[must_use]
pub fn supported_typed_plugin_contracts() -> Vec<TypedPluginContractDescriptor> {
    [
        TypedPluginContractKind::MemoryProvider,
        TypedPluginContractKind::ContextEngine,
        TypedPluginContractKind::RoutingStrategy,
    ]
    .into_iter()
    .filter_map(|kind| {
        typed_plugin_contract_descriptor(kind, DEFAULT_TYPED_PLUGIN_CONTRACT_VERSION)
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        default_typed_plugin_contract_version, supported_typed_plugin_contracts,
        typed_plugin_contract_descriptor, wit_package_id, wit_source, TypedPluginCapabilityClass,
        TypedPluginContractKind, HOST_CAPABILITIES_IMPORT_MODULE, HOST_CAPABILITY_CHANNEL_COUNT_FN,
        HOST_CAPABILITY_CHANNEL_HANDLE_FN, HOST_CAPABILITY_HTTP_COUNT_FN,
        HOST_CAPABILITY_HTTP_HANDLE_FN, HOST_CAPABILITY_SECRET_COUNT_FN,
        HOST_CAPABILITY_SECRET_HANDLE_FN, HOST_CAPABILITY_STORAGE_COUNT_FN,
        HOST_CAPABILITY_STORAGE_HANDLE_FN, WIT_WORLD_NAME,
    };

    #[test]
    fn wit_package_id_is_stable() {
        assert_eq!(wit_package_id(), "palyra:plugins/sdk@0.1.0");
    }

    #[test]
    fn wit_source_contains_expected_world_and_imports() {
        let source = wit_source();
        assert!(source.contains("world palyra-plugin"));
        assert!(source.contains("import host-capabilities"));
        assert!(source.contains("run: func() -> s32"));
        assert!(source.contains("plugin-hello: func() -> string"));
        assert!(source.contains(HOST_CAPABILITY_HTTP_COUNT_FN));
        assert!(source.contains(HOST_CAPABILITY_HTTP_HANDLE_FN));
        assert!(source.contains(HOST_CAPABILITY_SECRET_COUNT_FN));
        assert!(source.contains(HOST_CAPABILITY_SECRET_HANDLE_FN));
        assert!(source.contains(HOST_CAPABILITY_STORAGE_COUNT_FN));
        assert!(source.contains(HOST_CAPABILITY_STORAGE_HANDLE_FN));
        assert!(source.contains(HOST_CAPABILITY_CHANNEL_COUNT_FN));
        assert!(source.contains(HOST_CAPABILITY_CHANNEL_HANDLE_FN));
    }

    #[test]
    fn exported_wit_symbol_names_are_stable() {
        assert_eq!(WIT_WORLD_NAME, "palyra-plugin");
        assert_eq!(HOST_CAPABILITIES_IMPORT_MODULE, "palyra:plugins/host-capabilities@0.1.0");
    }

    #[test]
    fn typed_contract_descriptors_cover_supported_extension_points() {
        let supported = supported_typed_plugin_contracts();
        assert_eq!(supported.len(), 3);
        assert!(supported.iter().all(|descriptor| descriptor.version == 1));
        assert!(supported.iter().all(|descriptor| !descriptor.lifecycle.is_empty()));
        assert!(supported.iter().all(|descriptor| {
            descriptor
                .allowed_capability_classes
                .iter()
                .all(|class| *class != TypedPluginCapabilityClass::Channels)
        }));
    }

    #[test]
    fn typed_contract_descriptor_rejects_unknown_versions() {
        assert!(
            typed_plugin_contract_descriptor(TypedPluginContractKind::MemoryProvider, 99).is_none()
        );
        assert_eq!(default_typed_plugin_contract_version(), 1);
    }
}
