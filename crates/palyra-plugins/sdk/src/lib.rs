//! Bootstrap SDK placeholder for Palyra plugin authors.
//!
//! The initial WIT contract is defined in `wit/palyra-sdk.wit`.

use serde::{Deserialize, Serialize};

/// WIT package identifier for the bootstrap plugin SDK contract.
pub const WIT_PACKAGE_ID: &str = "palyra:plugins/sdk@0.1.0";
/// Host/plugin ABI marker independent from the package crate version.
pub const SDK_ABI_VERSION: &str = "palyra.plugins.sdk.abi.v1";
/// Current SDK ABI major version.
pub const SDK_ABI_MAJOR: u32 = 1;
/// Oldest SDK ABI major version accepted by this host.
pub const SDK_ABI_MIN_MAJOR: u32 = 1;
/// Newest SDK ABI major version accepted by this host.
pub const SDK_ABI_MAX_MAJOR: u32 = 1;
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
/// Default per-invocation timeout for typed plugin contracts.
pub const DEFAULT_TYPED_PLUGIN_CONTRACT_TIMEOUT_MS: u64 = 2_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TypedPluginContractKind {
    MemoryProvider,
    ContextEngine,
    RoutingStrategy,
    RunLifecycleHook,
    CompactionProvider,
    DiagnosticsProvider,
    PolicySignalProvider,
    ChannelBindingProvider,
    DeliveryAdapter,
    SchedulerTaskProvider,
    ModelAuthProvider,
    ConnectorAdapter,
}

impl TypedPluginContractKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemoryProvider => "memory_provider",
            Self::ContextEngine => "context_engine",
            Self::RoutingStrategy => "routing_strategy",
            Self::RunLifecycleHook => "run_lifecycle_hook",
            Self::CompactionProvider => "compaction_provider",
            Self::DiagnosticsProvider => "diagnostics_provider",
            Self::PolicySignalProvider => "policy_signal_provider",
            Self::ChannelBindingProvider => "channel_binding_provider",
            Self::DeliveryAdapter => "delivery_adapter",
            Self::SchedulerTaskProvider => "scheduler_task_provider",
            Self::ModelAuthProvider => "model_auth_provider",
            Self::ConnectorAdapter => "connector_adapter",
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum TypedPluginDataSensitivity {
    Public,
    #[default]
    Internal,
    Sensitive,
    Secret,
}

impl TypedPluginDataSensitivity {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
            Self::Sensitive => "sensitive",
            Self::Secret => "secret",
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TypedPluginContractOperation {
    ProvideMemory,
    PlanContext,
    RouteModel,
    DecideRunLifecycle,
    CompactContext,
    EmitDiagnostics,
    EmitPolicySignal,
    DiscoverBinding,
    ValidateBinding,
    RepairBindingHint,
    ExplainBinding,
    DeliverMessage,
    ScheduleTask,
    ResolveModelAuth,
    ConnectorInbound,
    ConnectorOutbound,
    ConnectorRateLimit,
}

impl TypedPluginContractOperation {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProvideMemory => "provide_memory",
            Self::PlanContext => "plan_context",
            Self::RouteModel => "route_model",
            Self::DecideRunLifecycle => "decide_run_lifecycle",
            Self::CompactContext => "compact_context",
            Self::EmitDiagnostics => "emit_diagnostics",
            Self::EmitPolicySignal => "emit_policy_signal",
            Self::DiscoverBinding => "discover_binding",
            Self::ValidateBinding => "validate_binding",
            Self::RepairBindingHint => "repair_binding_hint",
            Self::ExplainBinding => "explain_binding",
            Self::DeliverMessage => "deliver_message",
            Self::ScheduleTask => "schedule_task",
            Self::ResolveModelAuth => "resolve_model_auth",
            Self::ConnectorInbound => "connector_inbound",
            Self::ConnectorOutbound => "connector_outbound",
            Self::ConnectorRateLimit => "connector_rate_limit",
        }
    }
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
    #[serde(default = "default_sdk_abi_major")]
    pub sdk_abi_major: u32,
    #[serde(default)]
    pub input_schema: String,
    #[serde(default)]
    pub output_schema: String,
    #[serde(default = "default_typed_plugin_contract_timeout_ms")]
    pub default_timeout_ms: u64,
    #[serde(default)]
    pub sensitivity: TypedPluginDataSensitivity,
    #[serde(default)]
    pub lifecycle: Vec<TypedPluginContractLifecyclePhase>,
    #[serde(default)]
    pub operations: Vec<TypedPluginContractOperation>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SdkAbiCompatibility {
    pub abi_major: u32,
    pub min_abi_major: u32,
    pub max_abi_major: u32,
}

impl SdkAbiCompatibility {
    #[must_use]
    pub const fn accepts(self, abi_major: u32) -> bool {
        abi_major >= self.min_abi_major && abi_major <= self.max_abi_major
    }
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

/// Returns the SDK ABI marker independent from the package crate version.
#[must_use]
pub fn sdk_abi_version() -> &'static str {
    SDK_ABI_VERSION
}

/// Returns the SDK ABI compatibility range supported by this host.
#[must_use]
pub const fn sdk_abi_compatibility() -> SdkAbiCompatibility {
    SdkAbiCompatibility {
        abi_major: SDK_ABI_MAJOR,
        min_abi_major: SDK_ABI_MIN_MAJOR,
        max_abi_major: SDK_ABI_MAX_MAJOR,
    }
}

/// Returns the default version for typed plugin contracts.
#[must_use]
pub fn default_typed_plugin_contract_version() -> u32 {
    DEFAULT_TYPED_PLUGIN_CONTRACT_VERSION
}

/// Returns the default SDK ABI major version for persisted descriptor compatibility.
#[must_use]
pub fn default_sdk_abi_major() -> u32 {
    SDK_ABI_MAJOR
}

/// Returns the default timeout for typed plugin contract invocations.
#[must_use]
pub fn default_typed_plugin_contract_timeout_ms() -> u64 {
    DEFAULT_TYPED_PLUGIN_CONTRACT_TIMEOUT_MS
}

/// Returns all typed plugin contract kinds known by this SDK.
#[must_use]
pub fn all_typed_plugin_contract_kinds() -> Vec<TypedPluginContractKind> {
    vec![
        TypedPluginContractKind::MemoryProvider,
        TypedPluginContractKind::ContextEngine,
        TypedPluginContractKind::RoutingStrategy,
        TypedPluginContractKind::RunLifecycleHook,
        TypedPluginContractKind::CompactionProvider,
        TypedPluginContractKind::DiagnosticsProvider,
        TypedPluginContractKind::PolicySignalProvider,
        TypedPluginContractKind::ChannelBindingProvider,
        TypedPluginContractKind::DeliveryAdapter,
        TypedPluginContractKind::SchedulerTaskProvider,
        TypedPluginContractKind::ModelAuthProvider,
        TypedPluginContractKind::ConnectorAdapter,
    ]
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

    let descriptor = match kind {
        TypedPluginContractKind::MemoryProvider => build_descriptor(
            kind,
            version,
            "palyra.plugin.memory_provider.input.v1",
            "palyra.plugin.memory_provider.output.v1",
            TypedPluginDataSensitivity::Sensitive,
            vec![TypedPluginContractOperation::ProvideMemory],
            default_data_capabilities(),
            vec![
                "contract_negotiation_failed".to_owned(),
                "embed_request_failed".to_owned(),
                "invalid_embedding_dimensions".to_owned(),
                "degraded_fallback_active".to_owned(),
            ],
            vec![
                "config.credentials".to_owned(),
                "request.input_text".to_owned(),
                "response.embedding_preview".to_owned(),
            ],
            vec![
                "binding.negotiated".to_owned(),
                "memory.embedding_runtime".to_owned(),
                "memory.embedding_failure".to_owned(),
            ],
            vec![
                "memory.embedding_runtime".to_owned(),
                "memory.embedding_backfill".to_owned(),
                "memory.embedding_degradation".to_owned(),
            ],
        ),
        TypedPluginContractKind::ContextEngine => build_descriptor(
            kind,
            version,
            "palyra.plugin.context_engine.input.v1",
            "palyra.plugin.context_engine.output.v1",
            TypedPluginDataSensitivity::Sensitive,
            vec![TypedPluginContractOperation::PlanContext],
            default_data_capabilities(),
            vec![
                "contract_negotiation_failed".to_owned(),
                "context_plan_failed".to_owned(),
                "segment_budget_exceeded".to_owned(),
                "segment_redaction_required".to_owned(),
            ],
            vec![
                "context.segment.preview".to_owned(),
                "context.compaction.summary".to_owned(),
                "context.references.inline".to_owned(),
            ],
            vec![
                "binding.negotiated".to_owned(),
                "context.plan".to_owned(),
                "context.segment_drop".to_owned(),
            ],
            vec![
                "context.plan".to_owned(),
                "context.compaction".to_owned(),
                "context.recall_mix".to_owned(),
            ],
        ),
        TypedPluginContractKind::RoutingStrategy => build_descriptor(
            kind,
            version,
            "palyra.plugin.routing_strategy.input.v1",
            "palyra.plugin.routing_strategy.output.v1",
            TypedPluginDataSensitivity::Sensitive,
            vec![TypedPluginContractOperation::RouteModel],
            default_data_capabilities(),
            vec![
                "contract_negotiation_failed".to_owned(),
                "routing_decision_failed".to_owned(),
                "budget_gate_blocked".to_owned(),
                "provider_selection_rejected".to_owned(),
            ],
            vec![
                "routing.prompt_preview".to_owned(),
                "routing.provider_credentials".to_owned(),
                "routing.override_payload".to_owned(),
            ],
            vec![
                "binding.negotiated".to_owned(),
                "routing.decision".to_owned(),
                "routing.budget_gate".to_owned(),
            ],
            vec![
                "routing.decision".to_owned(),
                "routing.model_mix".to_owned(),
                "routing.provider_health".to_owned(),
            ],
        ),
        TypedPluginContractKind::RunLifecycleHook => build_descriptor(
            kind,
            version,
            "palyra.plugin.run_lifecycle_hook.input.v1",
            "palyra.plugin.run_lifecycle_hook.decision.v1",
            TypedPluginDataSensitivity::Sensitive,
            vec![TypedPluginContractOperation::DecideRunLifecycle],
            Vec::new(),
            vec![
                "contract_negotiation_failed".to_owned(),
                "hook_timeout".to_owned(),
                "hook_panic".to_owned(),
                "terminal_decision_conflict".to_owned(),
            ],
            vec!["hook.payload.preview".to_owned(), "hook.decision.reason".to_owned()],
            vec![
                "hook.dispatched".to_owned(),
                "hook.failed".to_owned(),
                "hook.decision".to_owned(),
            ],
            vec!["hook.latency".to_owned(), "hook.terminal_decision".to_owned()],
        ),
        TypedPluginContractKind::CompactionProvider => build_descriptor(
            kind,
            version,
            "palyra.plugin.compaction_provider.input.v1",
            "palyra.plugin.compaction_provider.output.v1",
            TypedPluginDataSensitivity::Sensitive,
            vec![TypedPluginContractOperation::CompactContext],
            default_data_capabilities(),
            vec![
                "contract_negotiation_failed".to_owned(),
                "compaction_budget_exceeded".to_owned(),
                "compaction_output_invalid".to_owned(),
            ],
            vec!["compaction.input.preview".to_owned(), "compaction.output.summary".to_owned()],
            vec!["compaction.provider_invoked".to_owned(), "compaction.provider_failed".to_owned()],
            vec!["compaction.provider_latency".to_owned()],
        ),
        TypedPluginContractKind::DiagnosticsProvider => build_descriptor(
            kind,
            version,
            "palyra.plugin.diagnostics_provider.input.v1",
            "palyra.plugin.diagnostics_provider.output.v1",
            TypedPluginDataSensitivity::Sensitive,
            vec![TypedPluginContractOperation::EmitDiagnostics],
            vec![TypedPluginCapabilityClass::StoragePrefixes],
            vec![
                "contract_negotiation_failed".to_owned(),
                "diagnostics_redaction_required".to_owned(),
                "diagnostics_output_invalid".to_owned(),
            ],
            vec!["diagnostics.raw_payload".to_owned(), "diagnostics.secret_like_values".to_owned()],
            vec!["diagnostics.provider_invoked".to_owned()],
            vec!["diagnostics.provider_latency".to_owned()],
        ),
        TypedPluginContractKind::PolicySignalProvider => build_descriptor(
            kind,
            version,
            "palyra.plugin.policy_signal_provider.input.v1",
            "palyra.plugin.policy_signal_provider.output.v1",
            TypedPluginDataSensitivity::Sensitive,
            vec![TypedPluginContractOperation::EmitPolicySignal],
            Vec::new(),
            vec!["contract_negotiation_failed".to_owned(), "policy_signal_invalid".to_owned()],
            vec!["policy.signal.context".to_owned()],
            vec!["policy.signal.provider_invoked".to_owned()],
            vec!["policy.signal.count".to_owned()],
        ),
        TypedPluginContractKind::ChannelBindingProvider => build_descriptor(
            kind,
            version,
            "palyra.plugin.channel_binding_provider.input.v1",
            "palyra.plugin.channel_binding_provider.output.v1",
            TypedPluginDataSensitivity::Internal,
            vec![
                TypedPluginContractOperation::DiscoverBinding,
                TypedPluginContractOperation::ValidateBinding,
                TypedPluginContractOperation::RepairBindingHint,
                TypedPluginContractOperation::ExplainBinding,
            ],
            vec![TypedPluginCapabilityClass::Channels],
            vec![
                "contract_negotiation_failed".to_owned(),
                "binding_collision".to_owned(),
                "binding_validation_failed".to_owned(),
            ],
            vec!["binding.external_identity".to_owned()],
            vec!["binding.provider_invoked".to_owned(), "binding.repair_hint".to_owned()],
            vec!["binding.provider_latency".to_owned()],
        ),
        TypedPluginContractKind::DeliveryAdapter => build_descriptor(
            kind,
            version,
            "palyra.plugin.delivery_adapter.input.v1",
            "palyra.plugin.delivery_adapter.receipt.v1",
            TypedPluginDataSensitivity::Sensitive,
            vec![TypedPluginContractOperation::DeliverMessage],
            vec![TypedPluginCapabilityClass::Channels],
            vec![
                "contract_negotiation_failed".to_owned(),
                "delivery_ack_unknown".to_owned(),
                "delivery_duplicate_ack".to_owned(),
                "delivery_nack".to_owned(),
            ],
            vec!["delivery.message_preview".to_owned()],
            vec!["delivery.adapter_invoked".to_owned(), "delivery.receipt".to_owned()],
            vec!["delivery.latency".to_owned(), "delivery.retry_class".to_owned()],
        ),
        TypedPluginContractKind::SchedulerTaskProvider => build_descriptor(
            kind,
            version,
            "palyra.plugin.scheduler_task_provider.input.v1",
            "palyra.plugin.scheduler_task_provider.output.v1",
            TypedPluginDataSensitivity::Internal,
            vec![TypedPluginContractOperation::ScheduleTask],
            vec![TypedPluginCapabilityClass::StoragePrefixes],
            vec![
                "contract_negotiation_failed".to_owned(),
                "scheduler_task_invalid".to_owned(),
                "wake_gate_denied".to_owned(),
            ],
            vec!["scheduler.task_payload".to_owned()],
            vec!["scheduler.provider_invoked".to_owned()],
            vec!["scheduler.task_count".to_owned()],
        ),
        TypedPluginContractKind::ModelAuthProvider => build_descriptor(
            kind,
            version,
            "palyra.plugin.model_auth_provider.input.v1",
            "palyra.plugin.model_auth_provider.output.v1",
            TypedPluginDataSensitivity::Secret,
            vec![TypedPluginContractOperation::ResolveModelAuth],
            vec![TypedPluginCapabilityClass::Secrets],
            vec![
                "contract_negotiation_failed".to_owned(),
                "credential_handle_missing".to_owned(),
                "rate_limit_metadata_invalid".to_owned(),
            ],
            vec!["model_auth.credential_handle".to_owned()],
            vec!["model_auth.provider_invoked".to_owned()],
            vec!["model_auth.rate_limit".to_owned()],
        ),
        TypedPluginContractKind::ConnectorAdapter => build_descriptor(
            kind,
            version,
            "palyra.plugin.connector_adapter.input.v1",
            "palyra.plugin.connector_adapter.output.v1",
            TypedPluginDataSensitivity::Sensitive,
            vec![
                TypedPluginContractOperation::ConnectorInbound,
                TypedPluginContractOperation::ConnectorOutbound,
                TypedPluginContractOperation::ConnectorRateLimit,
                TypedPluginContractOperation::DiscoverBinding,
            ],
            vec![
                TypedPluginCapabilityClass::HttpHosts,
                TypedPluginCapabilityClass::Secrets,
                TypedPluginCapabilityClass::StoragePrefixes,
                TypedPluginCapabilityClass::Channels,
            ],
            vec![
                "contract_negotiation_failed".to_owned(),
                "connector_reconnect_failed".to_owned(),
                "connector_duplicate_inbound".to_owned(),
                "connector_delivery_nack".to_owned(),
            ],
            vec![
                "connector.inbound.body".to_owned(),
                "connector.outbound.body".to_owned(),
                "connector.auth.handle".to_owned(),
            ],
            vec!["connector.adapter_invoked".to_owned(), "connector.binding_discovered".to_owned()],
            vec!["connector.rate_limit".to_owned(), "connector.delivery_latency".to_owned()],
        ),
    };
    Some(descriptor)
}

/// Returns the full set of built-in typed plugin contracts supported by the host.
#[must_use]
pub fn supported_typed_plugin_contracts() -> Vec<TypedPluginContractDescriptor> {
    all_typed_plugin_contract_kinds()
        .into_iter()
        .filter_map(|kind| {
            typed_plugin_contract_descriptor(kind, DEFAULT_TYPED_PLUGIN_CONTRACT_VERSION)
        })
        .collect()
}

fn default_lifecycle() -> Vec<TypedPluginContractLifecyclePhase> {
    vec![
        TypedPluginContractLifecyclePhase::Discover,
        TypedPluginContractLifecyclePhase::Negotiate,
        TypedPluginContractLifecyclePhase::Bind,
        TypedPluginContractLifecyclePhase::Invoke,
        TypedPluginContractLifecyclePhase::Audit,
    ]
}

fn default_data_capabilities() -> Vec<TypedPluginCapabilityClass> {
    vec![
        TypedPluginCapabilityClass::HttpHosts,
        TypedPluginCapabilityClass::Secrets,
        TypedPluginCapabilityClass::StoragePrefixes,
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_descriptor(
    kind: TypedPluginContractKind,
    version: u32,
    input_schema: &'static str,
    output_schema: &'static str,
    sensitivity: TypedPluginDataSensitivity,
    operations: Vec<TypedPluginContractOperation>,
    allowed_capability_classes: Vec<TypedPluginCapabilityClass>,
    error_codes: Vec<String>,
    redacted_fields: Vec<String>,
    audit_hooks: Vec<String>,
    observability_hooks: Vec<String>,
) -> TypedPluginContractDescriptor {
    TypedPluginContractDescriptor {
        kind,
        version,
        sdk_abi_major: SDK_ABI_MAJOR,
        input_schema: input_schema.to_owned(),
        output_schema: output_schema.to_owned(),
        default_timeout_ms: DEFAULT_TYPED_PLUGIN_CONTRACT_TIMEOUT_MS,
        sensitivity,
        lifecycle: default_lifecycle(),
        operations,
        allowed_capability_classes,
        error_codes,
        redacted_fields,
        audit_hooks,
        observability_hooks,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        all_typed_plugin_contract_kinds, default_typed_plugin_contract_version,
        sdk_abi_compatibility, sdk_abi_version, supported_typed_plugin_contracts,
        typed_plugin_contract_descriptor, wit_package_id, wit_source, TypedPluginCapabilityClass,
        TypedPluginContractKind, TypedPluginContractOperation, HOST_CAPABILITIES_IMPORT_MODULE,
        HOST_CAPABILITY_CHANNEL_COUNT_FN, HOST_CAPABILITY_CHANNEL_HANDLE_FN,
        HOST_CAPABILITY_HTTP_COUNT_FN, HOST_CAPABILITY_HTTP_HANDLE_FN,
        HOST_CAPABILITY_SECRET_COUNT_FN, HOST_CAPABILITY_SECRET_HANDLE_FN,
        HOST_CAPABILITY_STORAGE_COUNT_FN, HOST_CAPABILITY_STORAGE_HANDLE_FN, SDK_ABI_MAJOR,
        WIT_WORLD_NAME,
    };

    #[test]
    fn wit_package_id_is_stable() {
        assert_eq!(wit_package_id(), "palyra:plugins/sdk@0.1.0");
        assert_eq!(sdk_abi_version(), "palyra.plugins.sdk.abi.v1");
        assert!(sdk_abi_compatibility().accepts(SDK_ABI_MAJOR));
    }

    #[test]
    fn wit_source_contains_expected_world_and_imports() {
        let source = wit_source();
        assert!(source.contains("world palyra-plugin"));
        assert!(source.contains("import host-capabilities"));
        assert!(source.contains("run: func() -> s32"));
        assert!(source.contains("sdk-abi-version: func() -> string"));
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
        assert_eq!(supported.len(), all_typed_plugin_contract_kinds().len());
        assert!(supported.iter().all(|descriptor| descriptor.version == 1));
        assert!(supported.iter().all(|descriptor| descriptor.sdk_abi_major == 1));
        assert!(supported.iter().all(|descriptor| !descriptor.input_schema.is_empty()));
        assert!(supported.iter().all(|descriptor| !descriptor.output_schema.is_empty()));
        assert!(supported.iter().all(|descriptor| descriptor.default_timeout_ms > 0));
        assert!(supported.iter().all(|descriptor| !descriptor.lifecycle.is_empty()));
        assert!(supported.iter().any(|descriptor| {
            descriptor.kind == TypedPluginContractKind::RunLifecycleHook
                && descriptor.operations.contains(&TypedPluginContractOperation::DecideRunLifecycle)
                && descriptor.allowed_capability_classes.is_empty()
        }));
        assert!(supported.iter().any(|descriptor| {
            descriptor.kind == TypedPluginContractKind::ConnectorAdapter
                && descriptor
                    .allowed_capability_classes
                    .contains(&TypedPluginCapabilityClass::Channels)
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
