import { useState } from "react";

import type { JsonValue } from "../../../consoleApi";
import { isJsonObject, type JsonObject } from "../../../console/shared";

const DEFAULT_CHANNEL_LOGS_LIMIT = "25";
const DEFAULT_CHANNEL_TEST_TEXT = "hello from web console";
const DEFAULT_CHANNEL_TEST_CONVERSATION_ID = "test:conversation";
const DEFAULT_CHANNEL_TEST_SENDER_ID = "test-user";
const DEFAULT_CHANNEL_ROUTER_PREVIEW_TEXT = "pair 000000";
const DEFAULT_CHANNEL_ROUTER_PREVIEW_MAX_PAYLOAD_BYTES = "2048";
const DEFAULT_CHANNEL_ROUTER_MINT_TTL_MS = "600000";

export function useChannelCoreState() {
  const [channelsBusy, setChannelsBusy] = useState(false);
  const [channelsConnectors, setChannelsConnectors] = useState<JsonObject[]>([]);
  const [channelsSelectedConnectorId, setChannelsSelectedConnectorId] = useState("");
  const [channelsSelectedStatus, setChannelsSelectedStatus] = useState<JsonObject | null>(null);
  const [channelsEvents, setChannelsEvents] = useState<JsonObject[]>([]);
  const [channelsDeadLetters, setChannelsDeadLetters] = useState<JsonObject[]>([]);
  const [channelsLogsLimit, setChannelsLogsLimit] = useState(DEFAULT_CHANNEL_LOGS_LIMIT);
  const [channelsTestText, setChannelsTestText] = useState(DEFAULT_CHANNEL_TEST_TEXT);
  const [channelsTestConversationId, setChannelsTestConversationId] = useState(
    DEFAULT_CHANNEL_TEST_CONVERSATION_ID,
  );
  const [channelsTestSenderId, setChannelsTestSenderId] = useState(DEFAULT_CHANNEL_TEST_SENDER_ID);
  const [channelsTestSenderDisplay, setChannelsTestSenderDisplay] = useState("");
  const [channelsTestCrashOnce, setChannelsTestCrashOnce] = useState(false);
  const [channelsTestDirectMessage, setChannelsTestDirectMessage] = useState(true);
  const [channelsTestBroadcast, setChannelsTestBroadcast] = useState(false);
  const [channelRouterRules, setChannelRouterRules] = useState<JsonObject | null>(null);
  const [channelRouterConfigHash, setChannelRouterConfigHash] = useState("");
  const [channelRouterWarnings, setChannelRouterWarnings] = useState<string[]>([]);
  const [channelRouterPreviewChannel, setChannelRouterPreviewChannel] = useState("");
  const [channelRouterPreviewText, setChannelRouterPreviewText] = useState(
    DEFAULT_CHANNEL_ROUTER_PREVIEW_TEXT,
  );
  const [channelRouterPreviewConversationId, setChannelRouterPreviewConversationId] = useState("");
  const [channelRouterPreviewSenderIdentity, setChannelRouterPreviewSenderIdentity] = useState("");
  const [channelRouterPreviewSenderDisplay, setChannelRouterPreviewSenderDisplay] = useState("");
  const [channelRouterPreviewSenderVerified, setChannelRouterPreviewSenderVerified] =
    useState(true);
  const [channelRouterPreviewIsDirectMessage, setChannelRouterPreviewIsDirectMessage] =
    useState(true);
  const [channelRouterPreviewRequestedBroadcast, setChannelRouterPreviewRequestedBroadcast] =
    useState(false);
  const [channelRouterPreviewMaxPayloadBytes, setChannelRouterPreviewMaxPayloadBytes] = useState(
    DEFAULT_CHANNEL_ROUTER_PREVIEW_MAX_PAYLOAD_BYTES,
  );
  const [channelRouterPreviewResult, setChannelRouterPreviewResult] = useState<JsonObject | null>(
    null,
  );
  const [channelRouterPairingsFilterChannel, setChannelRouterPairingsFilterChannel] = useState("");
  const [channelRouterPairings, setChannelRouterPairings] = useState<JsonObject[]>([]);
  const [channelRouterMintChannel, setChannelRouterMintChannel] = useState("");
  const [channelRouterMintIssuedBy, setChannelRouterMintIssuedBy] = useState("");
  const [channelRouterMintTtlMs, setChannelRouterMintTtlMs] = useState(
    DEFAULT_CHANNEL_ROUTER_MINT_TTL_MS,
  );
  const [channelRouterMintResult, setChannelRouterMintResult] = useState<JsonObject | null>(null);

  function setSelectedChannelStatusPayload(payload: JsonValue): void {
    setChannelsSelectedStatus(isJsonObject(payload) ? payload : null);
  }

  function resetChannelCoreState(): void {
    setChannelsBusy(false);
    setChannelsConnectors([]);
    setChannelsSelectedConnectorId("");
    setChannelsSelectedStatus(null);
    setChannelsEvents([]);
    setChannelsDeadLetters([]);
    setChannelsLogsLimit(DEFAULT_CHANNEL_LOGS_LIMIT);
    setChannelsTestText(DEFAULT_CHANNEL_TEST_TEXT);
    setChannelsTestConversationId(DEFAULT_CHANNEL_TEST_CONVERSATION_ID);
    setChannelsTestSenderId(DEFAULT_CHANNEL_TEST_SENDER_ID);
    setChannelsTestSenderDisplay("");
    setChannelsTestCrashOnce(false);
    setChannelsTestDirectMessage(true);
    setChannelsTestBroadcast(false);
    setChannelRouterRules(null);
    setChannelRouterConfigHash("");
    setChannelRouterWarnings([]);
    setChannelRouterPreviewChannel("");
    setChannelRouterPreviewText(DEFAULT_CHANNEL_ROUTER_PREVIEW_TEXT);
    setChannelRouterPreviewConversationId("");
    setChannelRouterPreviewSenderIdentity("");
    setChannelRouterPreviewSenderDisplay("");
    setChannelRouterPreviewSenderVerified(true);
    setChannelRouterPreviewIsDirectMessage(true);
    setChannelRouterPreviewRequestedBroadcast(false);
    setChannelRouterPreviewMaxPayloadBytes(DEFAULT_CHANNEL_ROUTER_PREVIEW_MAX_PAYLOAD_BYTES);
    setChannelRouterPreviewResult(null);
    setChannelRouterPairingsFilterChannel("");
    setChannelRouterPairings([]);
    setChannelRouterMintChannel("");
    setChannelRouterMintIssuedBy("");
    setChannelRouterMintTtlMs(DEFAULT_CHANNEL_ROUTER_MINT_TTL_MS);
    setChannelRouterMintResult(null);
  }

  return {
    channelsBusy,
    setChannelsBusy,
    channelsConnectors,
    setChannelsConnectors,
    channelsSelectedConnectorId,
    setChannelsSelectedConnectorId,
    channelsSelectedStatus,
    channelsEvents,
    setChannelsEvents,
    channelsDeadLetters,
    setChannelsDeadLetters,
    channelsLogsLimit,
    setChannelsLogsLimit,
    channelsTestText,
    setChannelsTestText,
    channelsTestConversationId,
    setChannelsTestConversationId,
    channelsTestSenderId,
    setChannelsTestSenderId,
    channelsTestSenderDisplay,
    setChannelsTestSenderDisplay,
    channelsTestCrashOnce,
    setChannelsTestCrashOnce,
    channelsTestDirectMessage,
    setChannelsTestDirectMessage,
    channelsTestBroadcast,
    setChannelsTestBroadcast,
    channelRouterRules,
    setChannelRouterRules,
    channelRouterConfigHash,
    setChannelRouterConfigHash,
    channelRouterWarnings,
    setChannelRouterWarnings,
    channelRouterPreviewChannel,
    setChannelRouterPreviewChannel,
    channelRouterPreviewText,
    setChannelRouterPreviewText,
    channelRouterPreviewConversationId,
    setChannelRouterPreviewConversationId,
    channelRouterPreviewSenderIdentity,
    setChannelRouterPreviewSenderIdentity,
    channelRouterPreviewSenderDisplay,
    setChannelRouterPreviewSenderDisplay,
    channelRouterPreviewSenderVerified,
    setChannelRouterPreviewSenderVerified,
    channelRouterPreviewIsDirectMessage,
    setChannelRouterPreviewIsDirectMessage,
    channelRouterPreviewRequestedBroadcast,
    setChannelRouterPreviewRequestedBroadcast,
    channelRouterPreviewMaxPayloadBytes,
    setChannelRouterPreviewMaxPayloadBytes,
    channelRouterPreviewResult,
    setChannelRouterPreviewResult,
    channelRouterPairingsFilterChannel,
    setChannelRouterPairingsFilterChannel,
    channelRouterPairings,
    setChannelRouterPairings,
    channelRouterMintChannel,
    setChannelRouterMintChannel,
    channelRouterMintIssuedBy,
    setChannelRouterMintIssuedBy,
    channelRouterMintTtlMs,
    setChannelRouterMintTtlMs,
    channelRouterMintResult,
    setChannelRouterMintResult,
    setSelectedChannelStatusPayload,
    resetChannelCoreState,
  };
}
