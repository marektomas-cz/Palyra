import type { ConsoleApiClient, JsonValue } from "../../../consoleApi";

export type ChannelTestMessagePayload = {
  text: string;
  conversation_id?: string;
  sender_id?: string;
  sender_display?: string;
  simulate_crash_once?: boolean;
  is_direct_message?: boolean;
  requested_broadcast?: boolean;
};

export type ChannelRoutePreviewPayload = {
  channel: string;
  text: string;
  conversation_id?: string;
  sender_identity?: string;
  sender_display?: string;
  sender_verified?: boolean;
  is_direct_message?: boolean;
  requested_broadcast?: boolean;
  adapter_message_id?: string;
  adapter_thread_id?: string;
  max_payload_bytes?: number;
};

export type ChannelRouterPairingCodePayload = {
  channel: string;
  issued_by?: string;
  ttl_ms?: number;
};

export function listChannels(api: ConsoleApiClient) {
  return api.listChannels();
}

export function getChannelStatus(api: ConsoleApiClient, connectorId: string) {
  return api.getChannelStatus(connectorId);
}

export function setChannelEnabled(api: ConsoleApiClient, connectorId: string, enabled: boolean) {
  return api.setChannelEnabled(connectorId, enabled);
}

export function listChannelLogs(
  api: ConsoleApiClient,
  connectorId: string,
  params?: URLSearchParams,
) {
  return api.listChannelLogs(connectorId, params);
}

export function sendChannelTestMessage(
  api: ConsoleApiClient,
  connectorId: string,
  payload: ChannelTestMessagePayload,
) {
  return api.sendChannelTestMessage(connectorId, payload);
}

export function getChannelRouterRules(api: ConsoleApiClient) {
  return api.getChannelRouterRules();
}

export function getChannelRouterWarnings(api: ConsoleApiClient) {
  return api.getChannelRouterWarnings();
}

export function previewChannelRoute(api: ConsoleApiClient, payload: ChannelRoutePreviewPayload) {
  return api.previewChannelRoute(payload);
}

export function listChannelRouterPairings(api: ConsoleApiClient, params?: URLSearchParams) {
  return api.listChannelRouterPairings(params);
}

export function mintChannelRouterPairingCode(
  api: ConsoleApiClient,
  payload: ChannelRouterPairingCodePayload,
) {
  return api.mintChannelRouterPairingCode(payload);
}

export function pauseChannelQueue(api: ConsoleApiClient, connectorId: string) {
  return api.pauseChannelQueue(connectorId);
}

export function resumeChannelQueue(api: ConsoleApiClient, connectorId: string) {
  return api.resumeChannelQueue(connectorId);
}

export function drainChannelQueue(api: ConsoleApiClient, connectorId: string) {
  return api.drainChannelQueue(connectorId);
}

export function replayChannelDeadLetter(
  api: ConsoleApiClient,
  connectorId: string,
  deadLetterId: number,
) {
  return api.replayChannelDeadLetter(connectorId, deadLetterId);
}

export function discardChannelDeadLetter(
  api: ConsoleApiClient,
  connectorId: string,
  deadLetterId: number,
) {
  return api.discardChannelDeadLetter(connectorId, deadLetterId);
}

export type ChannelPayload = JsonValue;
