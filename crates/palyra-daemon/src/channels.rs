use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use palyra_common::{validate_canonical_id, CANONICAL_PROTOCOL_MAJOR};
use palyra_connectors::{
    connectors::default_adapters, ConnectorInstanceSpec, ConnectorKind, ConnectorRouter,
    ConnectorRouterError, ConnectorStatusSnapshot, ConnectorSupervisor, ConnectorSupervisorConfig,
    ConnectorSupervisorError, InboundIngestOutcome, InboundMessageEvent, OutboundMessageRequest,
    RouteInboundResult, RoutedOutboundMessage,
};
use serde_json::Value;
use thiserror::Error;
use tokio::time::{interval, MissedTickBehavior};
use tonic::metadata::MetadataValue;
use tracing::warn;
use ulid::Ulid;

use crate::gateway::{
    proto::palyra::{common::v1 as common_v1, gateway::v1 as gateway_v1},
    GatewayAuthConfig, HEADER_CHANNEL, HEADER_DEVICE_ID, HEADER_PRINCIPAL,
};

mod discord;

pub use discord::{
    discord_connector_id, discord_principal, discord_token_vault_ref, normalize_discord_account_id,
};

const CHANNEL_WORKER_DEVICE_ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_CHANNEL_WORKER_INTERVAL_MS: u64 = 1_000;
const DEFAULT_LOG_PAGE_LIMIT: usize = 100;

#[derive(Debug, Error)]
pub enum ChannelPlatformError {
    #[error(transparent)]
    Supervisor(#[from] ConnectorSupervisorError),
    #[error(transparent)]
    Store(#[from] palyra_connectors::ConnectorStoreError),
    #[error("invalid test message input: {0}")]
    InvalidInput(String),
}

#[derive(Debug, Clone)]
pub struct ChannelTestMessageRequest {
    pub text: String,
    pub conversation_id: String,
    pub sender_id: String,
    pub sender_display: Option<String>,
    pub simulate_crash_once: bool,
    pub is_direct_message: bool,
    pub requested_broadcast: bool,
}

#[derive(Debug, Clone)]
pub struct ChannelDiscordTestSendRequest {
    pub target: String,
    pub text: String,
    pub confirm: bool,
    pub auto_reaction: Option<String>,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChannelDiscordTestSendOutcome {
    pub envelope_id: String,
    pub connector_id: String,
    pub target: String,
    pub enqueued: bool,
    pub delivered: usize,
    pub retried: usize,
    pub dead_lettered: usize,
}

pub struct ChannelPlatform {
    supervisor: Arc<ConnectorSupervisor>,
    worker_interval: Duration,
}

impl ChannelPlatform {
    pub fn initialize(
        grpc_url: String,
        auth: GatewayAuthConfig,
        db_path: PathBuf,
    ) -> Result<Self, ChannelPlatformError> {
        let store = Arc::new(palyra_connectors::ConnectorStore::open(db_path)?);
        let router = Arc::new(GrpcChannelRouter { grpc_url, auth });
        let supervisor = Arc::new(ConnectorSupervisor::new(
            Arc::clone(&store),
            router,
            default_adapters(),
            ConnectorSupervisorConfig::default(),
        ));
        let platform = Self {
            supervisor,
            worker_interval: Duration::from_millis(DEFAULT_CHANNEL_WORKER_INTERVAL_MS),
        };
        platform.ensure_default_connector_inventory()?;
        Ok(platform)
    }

    pub fn list(&self) -> Result<Vec<ConnectorStatusSnapshot>, ChannelPlatformError> {
        self.supervisor.list_status().map_err(ChannelPlatformError::from)
    }

    pub fn status(
        &self,
        connector_id: &str,
    ) -> Result<ConnectorStatusSnapshot, ChannelPlatformError> {
        self.supervisor.status(connector_id).map_err(ChannelPlatformError::from)
    }

    pub fn ensure_discord_connector(
        &self,
        account_id: &str,
    ) -> Result<ConnectorStatusSnapshot, ChannelPlatformError> {
        let normalized_account_id = normalize_discord_account_id(account_id)?;
        let connector_id = discord_connector_id(normalized_account_id.as_str());
        if let Ok(status) = self.supervisor.status(connector_id.as_str()) {
            if status.kind != ConnectorKind::Discord {
                return Err(ChannelPlatformError::InvalidInput(format!(
                    "connector '{}' is not a Discord connector (kind={})",
                    connector_id,
                    status.kind.as_str()
                )));
            }
            return Ok(status);
        }
        let spec = discord::discord_connector_spec(normalized_account_id.as_str(), false);
        self.supervisor.register_connector(&spec)?;
        self.supervisor.status(spec.connector_id.as_str()).map_err(ChannelPlatformError::from)
    }

    pub fn runtime_snapshot(
        &self,
        connector_id: &str,
    ) -> Result<Option<Value>, ChannelPlatformError> {
        self.supervisor.runtime_snapshot(connector_id).map_err(ChannelPlatformError::from)
    }

    pub fn set_enabled(
        &self,
        connector_id: &str,
        enabled: bool,
    ) -> Result<ConnectorStatusSnapshot, ChannelPlatformError> {
        self.supervisor.set_enabled(connector_id, enabled).map_err(ChannelPlatformError::from)
    }

    pub fn logs(
        &self,
        connector_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<palyra_connectors::ConnectorEventRecord>, ChannelPlatformError> {
        self.supervisor
            .list_logs(connector_id, limit.unwrap_or(DEFAULT_LOG_PAGE_LIMIT))
            .map_err(ChannelPlatformError::from)
    }

    pub fn dead_letters(
        &self,
        connector_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<palyra_connectors::DeadLetterRecord>, ChannelPlatformError> {
        self.supervisor
            .list_dead_letters(connector_id, limit.unwrap_or(DEFAULT_LOG_PAGE_LIMIT))
            .map_err(ChannelPlatformError::from)
    }

    pub async fn submit_test_message(
        &self,
        connector_id: &str,
        request: ChannelTestMessageRequest,
    ) -> Result<InboundIngestOutcome, ChannelPlatformError> {
        if request.text.trim().is_empty() {
            return Err(ChannelPlatformError::InvalidInput("text cannot be empty".to_owned()));
        }
        if request.conversation_id.trim().is_empty() {
            return Err(ChannelPlatformError::InvalidInput(
                "conversation_id cannot be empty".to_owned(),
            ));
        }
        if request.sender_id.trim().is_empty() {
            return Err(ChannelPlatformError::InvalidInput("sender_id cannot be empty".to_owned()));
        }

        let mut body = request.text;
        if request.simulate_crash_once {
            body.push_str(" [connector-crash-once]");
        }
        let event = InboundMessageEvent {
            envelope_id: Ulid::new().to_string(),
            connector_id: connector_id.trim().to_owned(),
            conversation_id: request.conversation_id.trim().to_owned(),
            thread_id: None,
            sender_id: request.sender_id.trim().to_owned(),
            sender_display: request.sender_display,
            body,
            adapter_message_id: Some(Ulid::new().to_string()),
            adapter_thread_id: None,
            received_at_unix_ms: unix_ms_now(),
            is_direct_message: request.is_direct_message,
            requested_broadcast: request.requested_broadcast,
        };
        self.supervisor.ingest_inbound(event).await.map_err(ChannelPlatformError::from)
    }

    pub async fn submit_discord_test_send(
        &self,
        connector_id: &str,
        request: ChannelDiscordTestSendRequest,
    ) -> Result<ChannelDiscordTestSendOutcome, ChannelPlatformError> {
        let connector_id = connector_id.trim();
        if connector_id.is_empty() {
            return Err(ChannelPlatformError::InvalidInput(
                "connector_id cannot be empty".to_owned(),
            ));
        }
        if !request.confirm {
            return Err(ChannelPlatformError::InvalidInput(
                "discord test send requires explicit confirmation".to_owned(),
            ));
        }
        let status = self.status(connector_id)?;
        if status.kind != ConnectorKind::Discord {
            return Err(ChannelPlatformError::InvalidInput(format!(
                "discord test send is only supported for discord connectors (received kind={})",
                status.kind.as_str()
            )));
        }

        let text = request.text.trim();
        if text.is_empty() {
            return Err(ChannelPlatformError::InvalidInput(
                "test-send text cannot be empty".to_owned(),
            ));
        }
        let target = discord::normalize_discord_target(request.target.as_str())?;
        let thread_id = request
            .thread_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let auto_reaction = request
            .auto_reaction
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);

        let outbound = OutboundMessageRequest {
            envelope_id: Ulid::new().to_string(),
            connector_id: connector_id.to_owned(),
            conversation_id: target.clone(),
            reply_thread_id: thread_id,
            in_reply_to_message_id: None,
            text: text.to_owned(),
            broadcast: false,
            auto_ack_text: None,
            auto_reaction,
            timeout_ms: 30_000,
            max_payload_bytes: self.supervisor_config().max_outbound_body_bytes,
        };
        let enqueue = self.supervisor.enqueue_outbound(&outbound)?;
        let drain = self
            .supervisor
            .drain_due_outbox_for_connector(
                connector_id,
                self.supervisor_config().immediate_drain_batch_size,
            )
            .await?;
        Ok(ChannelDiscordTestSendOutcome {
            envelope_id: outbound.envelope_id,
            connector_id: connector_id.to_owned(),
            target,
            enqueued: enqueue.created,
            delivered: drain.delivered,
            retried: drain.retried,
            dead_lettered: drain.dead_lettered,
        })
    }

    pub async fn drain_due(&self) -> Result<palyra_connectors::DrainOutcome, ChannelPlatformError> {
        self.supervisor
            .drain_due_outbox(self.supervisor_config().background_drain_batch_size)
            .await
            .map_err(ChannelPlatformError::from)
    }

    #[must_use]
    pub fn worker_interval(&self) -> Duration {
        self.worker_interval
    }

    pub fn spawn_worker(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = interval(self.worker_interval());
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(error) = self.drain_due().await {
                    warn!(error = %error, "channel connector worker drain failed");
                }
            }
        })
    }

    fn supervisor_config(&self) -> ConnectorSupervisorConfig {
        ConnectorSupervisorConfig::default()
    }

    fn ensure_default_connector_inventory(&self) -> Result<(), ChannelPlatformError> {
        for spec in default_connector_specs() {
            let exists =
                self.supervisor.store().get_instance(spec.connector_id.as_str())?.is_some();
            if !exists {
                self.supervisor.register_connector(&spec)?;
            }
        }
        Ok(())
    }
}

fn default_connector_specs() -> Vec<ConnectorInstanceSpec> {
    vec![
        ConnectorInstanceSpec {
            connector_id: "echo:default".to_owned(),
            kind: ConnectorKind::Echo,
            principal: "channel:echo:default".to_owned(),
            auth_profile_ref: None,
            token_vault_ref: None,
            egress_allowlist: Vec::new(),
            enabled: true,
        },
        discord::discord_connector_spec("default", false),
        ConnectorInstanceSpec {
            connector_id: "slack:default".to_owned(),
            kind: ConnectorKind::Slack,
            principal: "channel:slack:default".to_owned(),
            auth_profile_ref: Some("slack.default".to_owned()),
            token_vault_ref: None,
            egress_allowlist: vec!["slack.com".to_owned(), "*.slack.com".to_owned()],
            enabled: false,
        },
        ConnectorInstanceSpec {
            connector_id: "telegram:default".to_owned(),
            kind: ConnectorKind::Telegram,
            principal: "channel:telegram:default".to_owned(),
            auth_profile_ref: Some("telegram.default".to_owned()),
            token_vault_ref: None,
            egress_allowlist: vec!["telegram.org".to_owned(), "*.telegram.org".to_owned()],
            enabled: false,
        },
    ]
}

struct GrpcChannelRouter {
    grpc_url: String,
    auth: GatewayAuthConfig,
}

#[async_trait::async_trait]
impl ConnectorRouter for GrpcChannelRouter {
    async fn route_inbound(
        &self,
        principal: &str,
        event: &InboundMessageEvent,
    ) -> Result<RouteInboundResult, ConnectorRouterError> {
        validate_canonical_id(event.envelope_id.as_str()).map_err(|_| {
            ConnectorRouterError::Message("inbound envelope_id must be a canonical ULID".to_owned())
        })?;
        let discord_connector = discord::is_discord_connector(event.connector_id.as_str());
        let conversation_id = if discord_connector {
            discord::canonical_discord_channel_identity(event.conversation_id.as_str())
        } else {
            event.conversation_id.clone()
        };
        let sender_handle = if discord_connector {
            discord::canonical_discord_sender_identity(event.sender_id.as_str())
        } else {
            event.sender_id.clone()
        };
        let mut client = gateway_v1::gateway_service_client::GatewayServiceClient::connect(
            self.grpc_url.clone(),
        )
        .await
        .map_err(|error| ConnectorRouterError::Message(error.to_string()))?;

        let mut request = tonic::Request::new(gateway_v1::RouteMessageRequest {
            v: CANONICAL_PROTOCOL_MAJOR,
            envelope: Some(common_v1::MessageEnvelope {
                v: CANONICAL_PROTOCOL_MAJOR,
                envelope_id: Some(common_v1::CanonicalId { ulid: event.envelope_id.clone() }),
                timestamp_unix_ms: event.received_at_unix_ms,
                origin: Some(common_v1::EnvelopeOrigin {
                    r#type: common_v1::envelope_origin::OriginType::Channel as i32,
                    channel: event.connector_id.clone(),
                    conversation_id,
                    sender_display: event.sender_display.clone().unwrap_or_default(),
                    sender_handle,
                    sender_verified: discord_connector,
                }),
                content: Some(common_v1::MessageContent {
                    text: event.body.clone(),
                    attachments: Vec::new(),
                }),
                security: None,
                max_payload_bytes: u64::try_from(event.body.len()).unwrap_or(u64::MAX),
            }),
            is_direct_message: event.is_direct_message,
            request_broadcast: event.requested_broadcast,
            adapter_message_id: event.adapter_message_id.clone().unwrap_or_default(),
            adapter_thread_id: event.adapter_thread_id.clone().unwrap_or_default(),
            retry_attempt: 0,
            session_label: String::new(),
        });
        let effective_principal = if self.auth.require_auth {
            self.auth.bound_principal.as_deref().unwrap_or(principal)
        } else {
            principal
        };
        let metadata = request.metadata_mut();
        metadata.insert(
            HEADER_PRINCIPAL,
            MetadataValue::try_from(effective_principal)
                .map_err(|error| ConnectorRouterError::Message(error.to_string()))?,
        );
        metadata.insert(
            HEADER_DEVICE_ID,
            MetadataValue::try_from(CHANNEL_WORKER_DEVICE_ID)
                .map_err(|error| ConnectorRouterError::Message(error.to_string()))?,
        );
        metadata.insert(
            HEADER_CHANNEL,
            MetadataValue::try_from(event.connector_id.as_str())
                .map_err(|error| ConnectorRouterError::Message(error.to_string()))?,
        );
        if self.auth.require_auth {
            let Some(token) = self.auth.admin_token.as_deref() else {
                return Err(ConnectorRouterError::Message(
                    "admin auth is required but no admin token is configured".to_owned(),
                ));
            };
            let bearer = format!("Bearer {token}");
            metadata.insert(
                "authorization",
                MetadataValue::try_from(bearer.as_str())
                    .map_err(|error| ConnectorRouterError::Message(error.to_string()))?,
            );
        }

        let response = client
            .route_message(request)
            .await
            .map_err(|error| ConnectorRouterError::Message(error.to_string()))?
            .into_inner();
        let outputs = response
            .outputs
            .into_iter()
            .map(|output| RoutedOutboundMessage {
                text: output.text,
                thread_id: non_empty(output.thread_id),
                in_reply_to_message_id: non_empty(output.in_reply_to_message_id),
                broadcast: output.broadcast,
                auto_ack_text: non_empty(output.auto_ack_text),
                auto_reaction: non_empty(output.auto_reaction),
            })
            .collect();
        Ok(RouteInboundResult {
            accepted: response.accepted,
            queued_for_retry: response.queued_for_retry,
            decision_reason: response.decision_reason,
            outputs,
            route_key: non_empty(response.route_key),
            retry_attempt: response.retry_attempt,
        })
    }
}

fn non_empty(raw: String) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn unix_ms_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(i64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        discord, discord_connector_id, discord_token_vault_ref, normalize_discord_account_id,
        ChannelPlatformError,
    };

    #[test]
    fn discord_account_id_normalization_enforces_supported_charset() {
        let normalized =
            normalize_discord_account_id(" Ops.Team_1 ").expect("account id should normalize");
        assert_eq!(normalized, "ops.team_1");
        let invalid = normalize_discord_account_id("bad/account")
            .expect_err("unsupported account_id characters should be rejected");
        assert!(
            matches!(invalid, ChannelPlatformError::InvalidInput(_)),
            "invalid account id should return an InvalidInput error"
        );
    }

    #[test]
    fn discord_connector_and_vault_ref_helpers_match_default_conventions() {
        assert_eq!(discord_connector_id("default"), "discord:default");
        assert_eq!(discord_token_vault_ref("default"), "global/discord_bot_token");
        assert_eq!(
            discord_token_vault_ref("ops"),
            "global/discord_bot_token.ops",
            "non-default account should use scoped vault key suffix"
        );
    }

    #[test]
    fn normalize_discord_target_rejects_empty_and_unsupported_values() {
        let normalized = discord::normalize_discord_target(" channel:123456 ")
            .expect("channel prefix should normalize to a target id");
        assert_eq!(normalized, "123456");
        let empty =
            discord::normalize_discord_target("  ").expect_err("empty target should be rejected");
        assert!(
            matches!(empty, ChannelPlatformError::InvalidInput(_)),
            "empty target should return InvalidInput"
        );
        let unsupported = discord::normalize_discord_target("channel:12 34")
            .expect_err("targets with spaces should be rejected");
        assert!(
            matches!(unsupported, ChannelPlatformError::InvalidInput(_)),
            "unsupported target should return InvalidInput"
        );
    }

    #[test]
    fn canonical_discord_identities_apply_expected_prefixes() {
        assert_eq!(
            discord::canonical_discord_sender_identity("12345"),
            "discord:user:12345",
            "plain sender ids should receive discord:user prefix"
        );
        assert_eq!(
            discord::canonical_discord_sender_identity("<@!67890>"),
            "discord:user:67890",
            "mention syntax should normalize to canonical sender identity"
        );
        assert_eq!(
            discord::canonical_discord_channel_identity("thread:abc"),
            "discord:channel:abc",
            "thread/channel aliases should normalize to canonical channel identity"
        );
        assert_eq!(
            discord::canonical_discord_channel_identity("<#C123>"),
            "discord:channel:c123",
            "channel mention syntax should normalize to canonical channel identity"
        );
    }
}
