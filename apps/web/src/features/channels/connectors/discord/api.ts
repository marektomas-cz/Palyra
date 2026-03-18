import type { ConsoleApiClient } from "../../../../consoleApi";

export type DiscordTestSendPayload = {
  target: string;
  text?: string;
  confirm: boolean;
  auto_reaction?: string;
  thread_id?: string;
};

export type DiscordOnboardingPayload = {
  account_id?: string;
  token: string;
  mode?: "local" | "remote_vps";
  inbound_scope?: "dm_only" | "allowlisted_guild_channels" | "open_guild_channels";
  allow_from?: string[];
  deny_from?: string[];
  require_mention?: boolean;
  mention_patterns?: string[];
  concurrency_limit?: number;
  broadcast_strategy?: "deny" | "mention_only" | "allow";
  confirm_open_guild_channels?: boolean;
  verify_channel_id?: string;
};

export type DiscordHealthRefreshPayload = {
  verify_channel_id?: string;
};

export function sendDiscordTest(
  api: ConsoleApiClient,
  connectorId: string,
  payload: DiscordTestSendPayload,
) {
  return api.sendChannelDiscordTestSend(connectorId, payload);
}

export function refreshDiscordChannelHealth(
  api: ConsoleApiClient,
  connectorId: string,
  payload: DiscordHealthRefreshPayload,
) {
  return api.refreshChannelHealth(connectorId, payload);
}

export function probeDiscordOnboarding(api: ConsoleApiClient, payload: DiscordOnboardingPayload) {
  return api.probeDiscordOnboarding(payload);
}

export function applyDiscordOnboarding(api: ConsoleApiClient, payload: DiscordOnboardingPayload) {
  return api.applyDiscordOnboarding(payload);
}
