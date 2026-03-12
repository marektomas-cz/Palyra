use anyhow::Result;
use serde_json::json;

use crate::{
    args::ChannelsRouterCommand, client::channels as channels_client, normalize_optional_text_arg,
    normalize_required_text_arg, output::channels as channels_output,
};

pub(crate) fn run(command: ChannelsRouterCommand) -> Result<()> {
    match command {
        ChannelsRouterCommand::Rules { url, token, principal, device_id, channel, json } => {
            let base_url = channels_client::resolve_base_url(url);
            let token = channels_client::resolve_token(token);
            let endpoint =
                format!("{}/admin/v1/channels/router/rules", base_url.trim_end_matches('/'));
            let client = channels_client::build_client()?;
            let response = channels_client::send_request(
                client.get(endpoint),
                token,
                principal,
                device_id,
                channel,
                "failed to call channel router rules endpoint",
            )?;
            channels_output::emit_router_rules(response, json)?;
        }
        ChannelsRouterCommand::Warnings { url, token, principal, device_id, channel, json } => {
            let base_url = channels_client::resolve_base_url(url);
            let token = channels_client::resolve_token(token);
            let endpoint =
                format!("{}/admin/v1/channels/router/warnings", base_url.trim_end_matches('/'));
            let client = channels_client::build_client()?;
            let response = channels_client::send_request(
                client.get(endpoint),
                token,
                principal,
                device_id,
                channel,
                "failed to call channel router warnings endpoint",
            )?;
            channels_output::emit_router_warnings(response, json)?;
        }
        ChannelsRouterCommand::Preview {
            route_channel,
            text,
            conversation_id,
            sender_identity,
            sender_display,
            sender_verified,
            is_direct_message,
            requested_broadcast,
            adapter_message_id,
            adapter_thread_id,
            max_payload_bytes,
            url,
            token,
            principal,
            device_id,
            channel,
            json,
        } => {
            let route_channel = normalize_route_channel_arg(route_channel)?;
            let text = normalize_required_text_arg(text, "text")?;
            let base_url = channels_client::resolve_base_url(url);
            let token = channels_client::resolve_token(token);
            let endpoint =
                format!("{}/admin/v1/channels/router/preview", base_url.trim_end_matches('/'));
            let client = channels_client::build_client()?;
            let payload = json!({
                "channel": route_channel,
                "text": text,
                "conversation_id": conversation_id.and_then(normalize_optional_text_arg),
                "sender_identity": sender_identity.and_then(normalize_optional_text_arg),
                "sender_display": sender_display.and_then(normalize_optional_text_arg),
                "sender_verified": sender_verified,
                "is_direct_message": is_direct_message,
                "requested_broadcast": requested_broadcast,
                "adapter_message_id": adapter_message_id.and_then(normalize_optional_text_arg),
                "adapter_thread_id": adapter_thread_id.and_then(normalize_optional_text_arg),
                "max_payload_bytes": max_payload_bytes,
            });
            let response = channels_client::send_request(
                client.post(endpoint).json(&payload),
                token,
                principal,
                device_id,
                channel,
                "failed to call channel router preview endpoint",
            )?;
            channels_output::emit_router_preview(response, json)?;
        }
        ChannelsRouterCommand::Pairings {
            route_channel,
            url,
            token,
            principal,
            device_id,
            channel,
            json,
        } => {
            let base_url = channels_client::resolve_base_url(url);
            let token = channels_client::resolve_token(token);
            let endpoint =
                format!("{}/admin/v1/channels/router/pairings", base_url.trim_end_matches('/'));
            let client = channels_client::build_client()?;
            let mut request = client.get(endpoint);
            if let Some(route_channel) = route_channel {
                let route_channel = normalize_route_channel_arg(route_channel)?;
                request = request.query(&[("channel", route_channel.as_str())]);
            }
            let response = channels_client::send_request(
                request,
                token,
                principal,
                device_id,
                channel,
                "failed to call channel router pairings endpoint",
            )?;
            channels_output::emit_router_pairings(response, json)?;
        }
        ChannelsRouterCommand::MintPairingCode {
            route_channel,
            issued_by,
            ttl_ms,
            url,
            token,
            principal,
            device_id,
            channel,
            json,
        } => {
            let route_channel = normalize_route_channel_arg(route_channel)?;
            let base_url = channels_client::resolve_base_url(url);
            let token = channels_client::resolve_token(token);
            let endpoint = format!(
                "{}/admin/v1/channels/router/pairing-codes",
                base_url.trim_end_matches('/')
            );
            let client = channels_client::build_client()?;
            let payload = json!({
                "channel": route_channel,
                "issued_by": issued_by.and_then(normalize_optional_text_arg),
                "ttl_ms": ttl_ms,
            });
            let response = channels_client::send_request(
                client.post(endpoint).json(&payload),
                token,
                principal,
                device_id,
                channel,
                "failed to call channel router pairing-code mint endpoint",
            )?;
            channels_output::emit_router_pairing_code(response, json)?;
        }
    }
    Ok(())
}

fn normalize_route_channel_arg(raw: String) -> Result<String> {
    normalize_required_text_arg(raw, "route_channel")
}
