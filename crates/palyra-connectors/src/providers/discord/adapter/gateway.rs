use std::{
    net::{IpAddr, ToSocketAddrs},
    sync::{Arc, Mutex},
    time::Duration,
};

use flate2::{Decompress, FlushDecompress, Status};
use futures::{SinkExt, StreamExt};
use palyra_common::redaction::redact_auth_error;
use reqwest::Url;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio::time::{interval, MissedTickBehavior};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use ulid::Ulid;

use crate::{
    discord_default_egress_allowlist, net::ConnectorNetGuard, protocol::InboundMessageEvent,
    storage::ConnectorInstanceRecord, supervisor::ConnectorAdapterError,
};

use super::{
    records::normalize_discord_message_create,
    runtime::{DiscordGatewayInflater, DiscordGatewayResumeState, DiscordRuntimeState},
    transport::{parse_discord_error_summary, DiscordCredentialResolver, DiscordTransport},
    unix_ms_now, DiscordAdapterConfig, DISCORD_GATEWAY_COMPRESSION,
    DISCORD_GATEWAY_DECOMPRESS_CHUNK_BYTES, DISCORD_GATEWAY_ENCODING,
    DISCORD_GATEWAY_HEARTBEAT_FALLBACK_MS, DISCORD_GATEWAY_INTENTS,
    DISCORD_GATEWAY_MAX_COMPRESSED_FRAME_BYTES, DISCORD_GATEWAY_MAX_DECOMPRESSED_FRAME_BYTES,
    DISCORD_GATEWAY_MONITOR_JITTER_MAX_MS, DISCORD_GATEWAY_MONITOR_MAX_BACKOFF_MS,
    DISCORD_GATEWAY_MONITOR_MIN_BACKOFF_MS, DISCORD_GATEWAY_VERSION,
    DISCORD_GATEWAY_ZLIB_SYNC_FLUSH_SUFFIX,
};

#[derive(Clone)]
pub(super) struct DiscordGatewayMonitorContext {
    pub(super) connector_id: String,
    pub(super) instance: ConnectorInstanceRecord,
    pub(super) config: DiscordAdapterConfig,
    pub(super) transport: Arc<dyn DiscordTransport>,
    pub(super) credential_resolver: Arc<dyn DiscordCredentialResolver>,
    pub(super) runtime_state: Arc<Mutex<DiscordRuntimeState>>,
    pub(super) sender: mpsc::Sender<InboundMessageEvent>,
}

pub(super) async fn run_discord_gateway_monitor(context: DiscordGatewayMonitorContext) {
    let mut resume_state = DiscordGatewayResumeState::default();
    let mut attempts = 0_u32;

    loop {
        let result = run_discord_gateway_session(&context, &mut resume_state).await;
        let now_unix_ms = unix_ms_now();
        if let Ok(mut state) = context.runtime_state.lock() {
            state.gateway_connected = false;
            state.gateway_last_disconnect_unix_ms = Some(now_unix_ms);
        }

        match result {
            Ok(()) => {
                attempts = 0;
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(error) => {
                attempts = attempts.saturating_add(1);
                let message = redact_auth_error(error.to_string().as_str());
                if let Ok(mut state) = context.runtime_state.lock() {
                    state.last_error = Some(message);
                }
                let delay_ms = monitor_backoff_ms(attempts);
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }
}

async fn run_discord_gateway_session(
    context: &DiscordGatewayMonitorContext,
    resume_state: &mut DiscordGatewayResumeState,
) -> Result<(), ConnectorAdapterError> {
    let credential = context.credential_resolver.resolve_credential(&context.instance).await?;
    let guard = build_discord_net_guard(&context.instance)?;

    let gateway_probe_url = build_gateway_bot_url(&context.config.api_base_url)?;
    validate_discord_url_target(&guard, &gateway_probe_url)?;
    let gateway_probe = context
        .transport
        .get(&gateway_probe_url, credential.token.as_str(), context.config.request_timeout_ms)
        .await?;
    if gateway_probe.status == 401 || gateway_probe.status == 403 {
        return Err(ConnectorAdapterError::Backend(redact_auth_error(
            format!(
                "discord gateway lookup unauthorized (status={}): {}",
                gateway_probe.status,
                parse_discord_error_summary(gateway_probe.body.as_str())
                    .unwrap_or_else(|| "unauthorized".to_owned())
            )
            .as_str(),
        )));
    }
    if !(200..300).contains(&gateway_probe.status) {
        return Err(ConnectorAdapterError::Backend(format!(
            "discord gateway lookup failed (status={}): {}",
            gateway_probe.status,
            parse_discord_error_summary(gateway_probe.body.as_str())
                .unwrap_or_else(|| "unexpected response".to_owned())
        )));
    }
    let gateway_url = parse_gateway_ws_url(gateway_probe.body.as_str())?;
    let gateway_url = normalize_gateway_ws_url(gateway_url)?;
    validate_discord_url_target(&guard, &gateway_url)?;

    let connect_result = tokio::time::timeout(
        Duration::from_millis(context.config.request_timeout_ms.max(1)),
        connect_async(gateway_url.as_str()),
    )
    .await
    .map_err(|_| ConnectorAdapterError::Backend("discord gateway connection timed out".to_owned()))?
    .map_err(|error| {
        ConnectorAdapterError::Backend(format!("discord gateway connection failed: {error}"))
    })?;
    let (socket, _response) = connect_result;
    let (mut sink, mut stream) = socket.split();

    let now_unix_ms = unix_ms_now();
    if let Ok(mut state) = context.runtime_state.lock() {
        state.gateway_connected = true;
        state.gateway_last_connect_unix_ms = Some(now_unix_ms);
        state.last_error = None;
    }

    run_discord_gateway_transport_loop(
        &mut sink,
        &mut stream,
        context,
        credential.token.as_str(),
        resume_state,
    )
    .await
}

pub(super) async fn run_discord_gateway_transport_loop<S, St>(
    sink: &mut S,
    stream: &mut St,
    context: &DiscordGatewayMonitorContext,
    bot_token: &str,
    resume_state: &mut DiscordGatewayResumeState,
) -> Result<(), ConnectorAdapterError>
where
    S: futures::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
    St: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let mut heartbeat = interval(Duration::from_millis(DISCORD_GATEWAY_HEARTBEAT_FALLBACK_MS));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut hello_received = false;
    let mut gateway_inflater = DiscordGatewayInflater::default();

    loop {
        tokio::select! {
            _ = heartbeat.tick(), if hello_received => {
                let heartbeat_payload = resume_state.seq.map(Value::from).unwrap_or(Value::Null);
                send_gateway_op(sink, 1, heartbeat_payload).await?;
            }
            maybe_message = stream.next() => {
                let Some(message) = maybe_message else {
                    return Err(ConnectorAdapterError::Backend(
                        "discord gateway closed the websocket stream".to_owned(),
                    ));
                };
                let message = message.map_err(|error| {
                    ConnectorAdapterError::Backend(format!("discord gateway read failed: {error}"))
                })?;
                let raw_text = match message {
                    Message::Text(payload) => Some(payload.to_string()),
                    Message::Binary(payload) => {
                        decode_gateway_binary_payload(payload.as_ref(), &mut gateway_inflater)?
                    }
                    Message::Ping(payload) => {
                        sink.send(Message::Pong(payload)).await.map_err(|error| {
                            ConnectorAdapterError::Backend(format!("discord gateway pong failed: {error}"))
                        })?;
                        continue;
                    }
                    Message::Close(frame) => {
                        let reason = frame
                            .map(|value| value.reason.to_string())
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or_else(|| "no close reason".to_owned());
                        return Err(ConnectorAdapterError::Backend(format!(
                            "discord gateway closed websocket: {reason}"
                        )));
                    }
                    Message::Pong(_) | Message::Frame(_) => continue,
                };
                let Some(raw_text) = raw_text else {
                    continue;
                };

                let envelope = parse_gateway_envelope(raw_text.as_str())?;
                handle_gateway_envelope(
                    sink,
                    envelope,
                    context,
                    bot_token,
                    resume_state,
                    &mut heartbeat,
                    &mut hello_received,
                )
                .await?;
            }
        }
    }
}

pub(super) async fn handle_gateway_envelope<S>(
    sink: &mut S,
    envelope: DiscordGatewayEnvelope,
    context: &DiscordGatewayMonitorContext,
    bot_token: &str,
    resume_state: &mut DiscordGatewayResumeState,
    heartbeat: &mut tokio::time::Interval,
    hello_received: &mut bool,
) -> Result<(), ConnectorAdapterError>
where
    S: futures::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    if let Some(seq) = envelope.seq {
        resume_state.seq = Some(seq);
    }
    if let Some(event_type) = envelope.event_type.as_deref() {
        if let Ok(mut state) = context.runtime_state.lock() {
            state.gateway_last_event_type = Some(event_type.to_owned());
        }
    }

    match envelope.op {
        10 => {
            let heartbeat_interval_ms = envelope
                .data
                .get("heartbeat_interval")
                .and_then(Value::as_u64)
                .unwrap_or(DISCORD_GATEWAY_HEARTBEAT_FALLBACK_MS)
                .max(250);
            *heartbeat = interval(Duration::from_millis(heartbeat_interval_ms));
            heartbeat.set_missed_tick_behavior(MissedTickBehavior::Skip);
            *hello_received = true;

            if let (Some(session_id), Some(seq)) =
                (resume_state.session_id.clone(), resume_state.seq)
            {
                send_gateway_op(
                    sink,
                    6,
                    json!({
                        "token": bot_token,
                        "session_id": session_id,
                        "seq": seq,
                    }),
                )
                .await?;
            } else {
                send_gateway_op(
                    sink,
                    2,
                    json!({
                        "token": bot_token,
                        "intents": DISCORD_GATEWAY_INTENTS,
                        "properties": {
                            "os": std::env::consts::OS,
                            "browser": "palyra",
                            "device": "palyra",
                        },
                    }),
                )
                .await?;
            }
        }
        11 => {}
        1 => {
            let heartbeat_payload = resume_state.seq.map(Value::from).unwrap_or(Value::Null);
            send_gateway_op(sink, 1, heartbeat_payload).await?;
        }
        7 => {
            return Err(ConnectorAdapterError::Backend(
                "discord gateway requested reconnect".to_owned(),
            ));
        }
        9 => {
            let resumable = envelope.data.as_bool().unwrap_or(false);
            if !resumable {
                resume_state.session_id = None;
                resume_state.seq = None;
            }
            return Err(ConnectorAdapterError::Backend(
                "discord gateway invalidated session".to_owned(),
            ));
        }
        0 => {
            let event_type = envelope.event_type.unwrap_or_default();
            match event_type.as_str() {
                "READY" => {
                    resume_state.session_id = envelope
                        .data
                        .get("session_id")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned);
                    resume_state.bot_user_id = envelope
                        .data
                        .get("user")
                        .and_then(|value| value.get("id"))
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned);
                }
                "MESSAGE_CREATE" => {
                    let inbound = normalize_discord_message_create(
                        context.connector_id.as_str(),
                        &envelope.data,
                        resume_state.bot_user_id.as_deref(),
                    );
                    if let Some(event) = inbound {
                        if let Ok(mut state) = context.runtime_state.lock() {
                            state.last_inbound_unix_ms = Some(event.received_at_unix_ms);
                        }
                        match context.sender.try_send(event) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                if let Ok(mut state) = context.runtime_state.lock() {
                                    state.last_error = Some(
                                        "discord inbound queue full; dropping event".to_owned(),
                                    );
                                }
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                return Err(ConnectorAdapterError::Backend(
                                    "discord inbound queue closed".to_owned(),
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

async fn send_gateway_op<S>(sink: &mut S, op: i64, data: Value) -> Result<(), ConnectorAdapterError>
where
    S: futures::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let payload = json!({
        "op": op,
        "d": data,
    });
    sink.send(Message::Text(payload.to_string().into())).await.map_err(|error| {
        ConnectorAdapterError::Backend(format!("discord gateway write failed: {error}"))
    })
}

pub(super) fn decode_gateway_binary_payload(
    payload: &[u8],
    inflater: &mut DiscordGatewayInflater,
) -> Result<Option<String>, ConnectorAdapterError> {
    if payload.is_empty() {
        return Ok(None);
    }
    if let Ok(raw_text) = std::str::from_utf8(payload) {
        return Ok(Some(raw_text.to_owned()));
    }
    let projected_size = inflater.compressed_buffer.len().saturating_add(payload.len());
    if projected_size > DISCORD_GATEWAY_MAX_COMPRESSED_FRAME_BYTES {
        reset_gateway_inflater(inflater);
        return Err(ConnectorAdapterError::Backend(format!(
            "discord gateway compressed payload exceeded {} bytes",
            DISCORD_GATEWAY_MAX_COMPRESSED_FRAME_BYTES
        )));
    }
    inflater.compressed_buffer.extend_from_slice(payload);
    if !inflater.compressed_buffer.ends_with(&DISCORD_GATEWAY_ZLIB_SYNC_FLUSH_SUFFIX) {
        return Ok(None);
    }

    let estimated_capacity = inflater
        .compressed_buffer
        .len()
        .saturating_mul(2)
        .clamp(1024, DISCORD_GATEWAY_MAX_DECOMPRESSED_FRAME_BYTES);
    let mut decoded = Vec::with_capacity(estimated_capacity);
    let mut output_chunk = [0_u8; DISCORD_GATEWAY_DECOMPRESS_CHUNK_BYTES];
    let mut input_offset = 0usize;
    while input_offset < inflater.compressed_buffer.len() {
        let total_in_before = inflater.decompressor.total_in();
        let total_out_before = inflater.decompressor.total_out();
        let status = inflater
            .decompressor
            .decompress(
                &inflater.compressed_buffer[input_offset..],
                &mut output_chunk,
                FlushDecompress::Sync,
            )
            .map_err(|error| {
                reset_gateway_inflater(inflater);
                ConnectorAdapterError::Backend(format!(
                    "discord gateway zlib payload decompression failed: {error}"
                ))
            })?;

        let consumed =
            usize::try_from(inflater.decompressor.total_in().saturating_sub(total_in_before))
                .unwrap_or(usize::MAX);
        let produced =
            usize::try_from(inflater.decompressor.total_out().saturating_sub(total_out_before))
                .unwrap_or(usize::MAX);
        input_offset = input_offset.saturating_add(consumed);

        if produced > 0 {
            if decoded.len().saturating_add(produced) > DISCORD_GATEWAY_MAX_DECOMPRESSED_FRAME_BYTES
            {
                reset_gateway_inflater(inflater);
                return Err(ConnectorAdapterError::Backend(format!(
                    "discord gateway decompressed payload exceeded {} bytes",
                    DISCORD_GATEWAY_MAX_DECOMPRESSED_FRAME_BYTES
                )));
            }
            decoded.extend_from_slice(&output_chunk[..produced]);
        }

        if consumed == 0 && produced == 0 {
            reset_gateway_inflater(inflater);
            return Err(ConnectorAdapterError::Backend(
                "discord gateway zlib payload decompression stalled".to_owned(),
            ));
        }

        if matches!(status, Status::StreamEnd) {
            break;
        }
    }
    inflater.compressed_buffer.clear();
    String::from_utf8(decoded).map(Some).map_err(|error| {
        reset_gateway_inflater(inflater);
        ConnectorAdapterError::Backend(format!(
            "discord gateway payload is not valid UTF-8 after decompression: {error}"
        ))
    })
}

fn reset_gateway_inflater(inflater: &mut DiscordGatewayInflater) {
    inflater.compressed_buffer.clear();
    inflater.decompressor = Decompress::new(true);
}

pub(super) struct DiscordGatewayEnvelope {
    pub(super) op: i64,
    pub(super) data: Value,
    pub(super) seq: Option<i64>,
    pub(super) event_type: Option<String>,
}

fn parse_gateway_envelope(raw: &str) -> Result<DiscordGatewayEnvelope, ConnectorAdapterError> {
    let payload = serde_json::from_str::<Value>(raw).map_err(|error| {
        ConnectorAdapterError::Backend(format!(
            "discord gateway payload is not valid JSON: {error}"
        ))
    })?;
    let op = payload.get("op").and_then(Value::as_i64).ok_or_else(|| {
        ConnectorAdapterError::Backend("discord gateway payload missing op code".to_owned())
    })?;
    Ok(DiscordGatewayEnvelope {
        op,
        data: payload.get("d").cloned().unwrap_or(Value::Null),
        seq: payload.get("s").and_then(Value::as_i64),
        event_type: payload
            .get("t")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    })
}

pub(super) fn build_discord_net_guard(
    instance: &ConnectorInstanceRecord,
) -> Result<ConnectorNetGuard, ConnectorAdapterError> {
    let allowlist = if instance.egress_allowlist.is_empty() {
        discord_default_egress_allowlist()
    } else {
        instance.egress_allowlist.clone()
    };
    ConnectorNetGuard::new(allowlist.as_slice()).map_err(|error| {
        ConnectorAdapterError::Backend(format!("discord egress allowlist rejected: {error}"))
    })
}

pub(super) fn validate_discord_url_target(
    guard: &ConnectorNetGuard,
    url: &Url,
) -> Result<(), ConnectorAdapterError> {
    validate_discord_url_target_with_resolver(guard, url, resolve_discord_target_addresses)
}

pub(super) fn validate_discord_url_target_with_resolver<F>(
    guard: &ConnectorNetGuard,
    url: &Url,
    resolver: F,
) -> Result<(), ConnectorAdapterError>
where
    F: Fn(&str, u16) -> Result<Vec<IpAddr>, ConnectorAdapterError>,
{
    let Some(host) = url.host_str() else {
        return Err(ConnectorAdapterError::Backend(
            "discord request URL is missing host".to_owned(),
        ));
    };
    let default_port =
        if url.scheme().eq_ignore_ascii_case("ws") || url.scheme().eq_ignore_ascii_case("http") {
            80_u16
        } else {
            443_u16
        };
    let port = url.port().unwrap_or(default_port);
    let mut resolved = resolver(host, port)?;
    resolved.sort_unstable();
    resolved.dedup();
    if resolved.is_empty() {
        return Err(ConnectorAdapterError::Backend(format!(
            "discord egress denied: DNS resolution returned no addresses for '{host}:{port}'"
        )));
    }
    guard
        .validate_target(host, resolved.as_slice())
        .map_err(|error| ConnectorAdapterError::Backend(format!("discord egress denied: {error}")))
}

fn resolve_discord_target_addresses(
    host: &str,
    port: u16,
) -> Result<Vec<IpAddr>, ConnectorAdapterError> {
    let addrs = (host, port).to_socket_addrs().map_err(|error| {
        ConnectorAdapterError::Backend(format!(
            "discord egress denied: DNS resolution failed for '{host}:{port}': {error}"
        ))
    })?;
    Ok(addrs.map(|entry| entry.ip()).collect())
}

fn build_gateway_bot_url(api_base: &Url) -> Result<Url, ConnectorAdapterError> {
    let candidate = format!("{}/gateway/bot", api_base.as_str().trim_end_matches('/'));
    Url::parse(candidate.as_str()).map_err(|error| {
        ConnectorAdapterError::Backend(format!("discord API URL invalid: {error}"))
    })
}

fn parse_gateway_ws_url(raw_body: &str) -> Result<Url, ConnectorAdapterError> {
    let payload = serde_json::from_str::<Value>(raw_body).map_err(|error| {
        ConnectorAdapterError::Backend(format!(
            "discord gateway metadata payload is not valid JSON: {error}"
        ))
    })?;
    let raw_url = payload
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ConnectorAdapterError::Backend(
                "discord gateway metadata missing 'url' field".to_owned(),
            )
        })?;
    Url::parse(raw_url).map_err(|error| {
        ConnectorAdapterError::Backend(format!("discord gateway URL is invalid: {error}"))
    })
}

pub(super) fn normalize_gateway_ws_url(mut url: Url) -> Result<Url, ConnectorAdapterError> {
    let normalized_scheme = match url.scheme() {
        "https" => "wss".to_owned(),
        "http" => "ws".to_owned(),
        "wss" | "ws" => url.scheme().to_owned(),
        _ => {
            return Err(ConnectorAdapterError::Backend(format!(
                "discord gateway URL uses unsupported scheme '{}'",
                url.scheme()
            )));
        }
    };
    if url.scheme() != normalized_scheme.as_str() {
        url.set_scheme(normalized_scheme.as_str()).map_err(|_| {
            ConnectorAdapterError::Backend(
                "failed to normalize discord gateway URL scheme".to_owned(),
            )
        })?;
    }
    {
        let mut pairs = url.query_pairs_mut();
        pairs.clear();
        pairs.append_pair("v", DISCORD_GATEWAY_VERSION);
        pairs.append_pair("encoding", DISCORD_GATEWAY_ENCODING);
        pairs.append_pair("compress", DISCORD_GATEWAY_COMPRESSION);
    }
    Ok(url)
}

fn monitor_backoff_ms(attempts: u32) -> u64 {
    let exponent = attempts.min(6);
    let base = DISCORD_GATEWAY_MONITOR_MIN_BACKOFF_MS
        .saturating_mul(1_u64 << exponent)
        .min(DISCORD_GATEWAY_MONITOR_MAX_BACKOFF_MS);
    base.saturating_add(monitor_jitter_ms()).min(DISCORD_GATEWAY_MONITOR_MAX_BACKOFF_MS)
}

fn monitor_jitter_ms() -> u64 {
    if DISCORD_GATEWAY_MONITOR_JITTER_MAX_MS <= 1 {
        return 0;
    }
    unix_ms_now().unsigned_abs() % DISCORD_GATEWAY_MONITOR_JITTER_MAX_MS
}

pub(super) fn deterministic_inbound_envelope_id(connector_id: &str, message_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(connector_id.as_bytes());
    hasher.update(b":");
    hasher.update(message_id.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Ulid::from_bytes(bytes).to_string()
}
