import type { Dispatch, FormEvent, SetStateAction } from "react";

import { type ConsoleApiClient, type JsonValue } from "../../../consoleApi";
import {
  discardChannelDeadLetter as discardChannelDeadLetterRequest,
  drainChannelQueue as drainChannelQueueRequest,
  getChannelRouterRules,
  getChannelRouterWarnings,
  getChannelStatus,
  listChannelLogs,
  listChannelRouterPairings,
  listChannels,
  mintChannelRouterPairingCode as mintChannelRouterPairingCodeRequest,
  pauseChannelQueue as pauseChannelQueueRequest,
  previewChannelRoute,
  replayChannelDeadLetter as replayChannelDeadLetterRequest,
  resumeChannelQueue as resumeChannelQueueRequest,
  sendChannelTestMessage,
  setChannelEnabled as setChannelEnabledRequest,
} from "./api";
import {
  emptyToUndefined,
  isJsonObject,
  isVisibleChannelConnector,
  parseInteger,
  readString,
  toErrorMessage,
  toJsonObjectArray,
  toStringArray,
  type JsonObject,
} from "../../../console/shared";

type Setter<T> = Dispatch<SetStateAction<T>>;

export type ChannelCoreDomainDeps = {
  api: ConsoleApiClient;
  channelsLogsLimit: string;
  channelsSelectedConnectorId: string;
  channelRouterPairingsFilterChannel: string;
  channelsTestText: string;
  channelsTestConversationId: string;
  channelsTestSenderId: string;
  channelsTestSenderDisplay: string;
  channelsTestCrashOnce: boolean;
  channelsTestDirectMessage: boolean;
  channelsTestBroadcast: boolean;
  channelRouterPreviewChannel: string;
  channelRouterPreviewText: string;
  channelRouterPreviewConversationId: string;
  channelRouterPreviewSenderIdentity: string;
  channelRouterPreviewSenderDisplay: string;
  channelRouterPreviewSenderVerified: boolean;
  channelRouterPreviewIsDirectMessage: boolean;
  channelRouterPreviewRequestedBroadcast: boolean;
  channelRouterPreviewMaxPayloadBytes: string;
  channelRouterMintChannel: string;
  channelRouterMintIssuedBy: string;
  channelRouterMintTtlMs: string;
  setChannelsBusy: Setter<boolean>;
  setError: Setter<string | null>;
  setNotice: Setter<string | null>;
  setChannelsConnectors: Setter<JsonObject[]>;
  setChannelsSelectedConnectorId: Setter<string>;
  setChannelsEvents: Setter<JsonObject[]>;
  setChannelsDeadLetters: Setter<JsonObject[]>;
  setChannelsTestCrashOnce: Setter<boolean>;
  setChannelRouterRules: Setter<JsonObject | null>;
  setChannelRouterConfigHash: Setter<string>;
  setChannelRouterWarnings: Setter<string[]>;
  setChannelRouterPairings: Setter<JsonObject[]>;
  setChannelRouterPreviewChannel: Setter<string>;
  setChannelRouterPreviewResult: Setter<JsonObject | null>;
  setChannelRouterMintChannel: Setter<string>;
  setChannelRouterMintResult: Setter<JsonObject | null>;
  setChannelRouterPairingsFilterChannel: Setter<string>;
  setSelectedChannelStatusPayload: (payload: JsonValue) => void;
};

export function createChannelCoreDomain(deps: ChannelCoreDomainDeps) {
  const {
    api,
    channelsLogsLimit,
    channelsSelectedConnectorId,
    channelRouterPairingsFilterChannel,
    channelsTestText,
    channelsTestConversationId,
    channelsTestSenderId,
    channelsTestSenderDisplay,
    channelsTestCrashOnce,
    channelsTestDirectMessage,
    channelsTestBroadcast,
    channelRouterPreviewChannel,
    channelRouterPreviewText,
    channelRouterPreviewConversationId,
    channelRouterPreviewSenderIdentity,
    channelRouterPreviewSenderDisplay,
    channelRouterPreviewSenderVerified,
    channelRouterPreviewIsDirectMessage,
    channelRouterPreviewRequestedBroadcast,
    channelRouterPreviewMaxPayloadBytes,
    channelRouterMintChannel,
    channelRouterMintIssuedBy,
    channelRouterMintTtlMs,
    setChannelsBusy,
    setError,
    setNotice,
    setChannelsConnectors,
    setChannelsSelectedConnectorId,
    setChannelsEvents,
    setChannelsDeadLetters,
    setChannelsTestCrashOnce,
    setChannelRouterRules,
    setChannelRouterConfigHash,
    setChannelRouterWarnings,
    setChannelRouterPairings,
    setChannelRouterPreviewChannel,
    setChannelRouterPreviewResult,
    setChannelRouterMintChannel,
    setChannelRouterMintResult,
    setChannelRouterPairingsFilterChannel,
    setSelectedChannelStatusPayload,
  } = deps;

  async function refreshChannelLogs(connectorId: string): Promise<void> {
    const params = new URLSearchParams();
    const parsedLimit = parseInteger(channelsLogsLimit);
    if (parsedLimit !== null && parsedLimit > 0) {
      params.set("limit", String(parsedLimit));
    }
    const response = await listChannelLogs(api, connectorId, params.size > 0 ? params : undefined);
    setChannelsEvents(toJsonObjectArray(response.events));
    setChannelsDeadLetters(toJsonObjectArray(response.dead_letters));
  }

  async function refreshChannelRouter(pairingsFilterOverride?: string): Promise<void> {
    const pairingsChannel = (pairingsFilterOverride ?? channelRouterPairingsFilterChannel).trim();
    const pairingsParams = new URLSearchParams();
    if (pairingsChannel.length > 0) {
      pairingsParams.set("channel", pairingsChannel);
    }

    const [rulesResponse, warningsResponse, pairingsResponse] = await Promise.all([
      getChannelRouterRules(api),
      getChannelRouterWarnings(api),
      listChannelRouterPairings(api, pairingsParams.size > 0 ? pairingsParams : undefined),
    ]);

    const rulesPayload = (rulesResponse as { rules?: unknown }).rules;
    const rules =
      typeof rulesPayload === "object" && rulesPayload !== null && !Array.isArray(rulesPayload)
        ? (rulesPayload as JsonObject)
        : null;
    setChannelRouterRules(rules);
    setChannelRouterConfigHash(readString(rulesResponse, "config_hash") ?? "");
    setChannelRouterWarnings(toStringArray(warningsResponse.warnings));
    setChannelRouterPairings(toJsonObjectArray(pairingsResponse.pairings));
  }

  async function refreshChannels(preferredConnectorId?: string): Promise<void> {
    setChannelsBusy(true);
    setError(null);
    try {
      const response = await listChannels(api);
      const connectors = toJsonObjectArray(response.connectors).filter(isVisibleChannelConnector);
      setChannelsConnectors(connectors);

      const requested = preferredConnectorId ?? channelsSelectedConnectorId;
      const requestedTrimmed = requested.trim();
      const connectorIds = connectors
        .map((entry) => readString(entry, "connector_id"))
        .filter((value): value is string => value !== null);
      const nextConnectorId =
        requestedTrimmed.length > 0 && connectorIds.includes(requestedTrimmed)
          ? requestedTrimmed
          : (connectorIds[0] ?? "");

      setChannelsSelectedConnectorId(nextConnectorId);
      if (nextConnectorId.length === 0) {
        setSelectedChannelStatusPayload(null);
        setChannelsEvents([]);
        setChannelsDeadLetters([]);
        setChannelRouterRules(null);
        setChannelRouterConfigHash("");
        setChannelRouterWarnings([]);
        setChannelRouterPairings([]);
        return;
      }

      const statusResponse = await getChannelStatus(api, nextConnectorId);
      setSelectedChannelStatusPayload(statusResponse as JsonValue);
      setChannelRouterPreviewChannel((previous) =>
        previous.trim().length > 0 ? previous : nextConnectorId,
      );
      setChannelRouterMintChannel((previous) =>
        previous.trim().length > 0 ? previous : nextConnectorId,
      );
      const pairingsFilter =
        channelRouterPairingsFilterChannel.trim().length > 0
          ? channelRouterPairingsFilterChannel.trim()
          : nextConnectorId;
      if (channelRouterPairingsFilterChannel.trim().length === 0) {
        setChannelRouterPairingsFilterChannel(nextConnectorId);
      }
      await refreshChannelLogs(nextConnectorId);
      await refreshChannelRouter(pairingsFilter);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function loadChannel(connectorId: string): Promise<void> {
    if (connectorId.trim().length === 0) {
      setError("Select a connector first.");
      return;
    }
    setChannelsBusy(true);
    setError(null);
    try {
      const normalizedConnectorId = connectorId.trim();
      setChannelsSelectedConnectorId(normalizedConnectorId);
      const statusResponse = await getChannelStatus(api, normalizedConnectorId);
      setSelectedChannelStatusPayload(statusResponse as JsonValue);
      setChannelRouterPreviewChannel(normalizedConnectorId);
      setChannelRouterMintChannel(normalizedConnectorId);
      const pairingsFilter =
        channelRouterPairingsFilterChannel.trim().length > 0
          ? channelRouterPairingsFilterChannel.trim()
          : normalizedConnectorId;
      if (channelRouterPairingsFilterChannel.trim().length === 0) {
        setChannelRouterPairingsFilterChannel(normalizedConnectorId);
      }
      await refreshChannelLogs(normalizedConnectorId);
      await refreshChannelRouter(pairingsFilter);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function setChannelEnabled(entry: JsonObject, enabled: boolean): Promise<void> {
    const connectorId = readString(entry, "connector_id");
    if (connectorId === null) {
      setError("Connector payload missing connector_id.");
      return;
    }
    setChannelsBusy(true);
    setError(null);
    try {
      await setChannelEnabledRequest(api, connectorId, enabled);
      setNotice(`Connector ${enabled ? "enabled" : "disabled"}.`);
      await refreshChannels(connectorId);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function submitChannelTestMessage(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (channelsSelectedConnectorId.trim().length === 0) {
      setError("Select a connector before sending a test message.");
      return;
    }
    if (channelsTestText.trim().length === 0) {
      setError("Test message text cannot be empty.");
      return;
    }
    setChannelsBusy(true);
    setError(null);
    try {
      const connectorId = channelsSelectedConnectorId.trim();
      const response = await sendChannelTestMessage(api, connectorId, {
        text: channelsTestText.trim(),
        conversation_id: emptyToUndefined(channelsTestConversationId),
        sender_id: emptyToUndefined(channelsTestSenderId),
        sender_display: emptyToUndefined(channelsTestSenderDisplay),
        simulate_crash_once: channelsTestCrashOnce,
        is_direct_message: channelsTestDirectMessage,
        requested_broadcast: channelsTestBroadcast,
      });
      if (isJsonObject(response.ingest)) {
        const accepted = response.ingest.accepted === true ? "true" : "false";
        const immediateDeliveryValue = response.ingest.immediate_delivery;
        const immediateDelivery =
          typeof immediateDeliveryValue === "number" || typeof immediateDeliveryValue === "string"
            ? String(immediateDeliveryValue)
            : "0";
        setNotice(
          `Channel test dispatched (accepted=${accepted}, immediate_delivery=${immediateDelivery}).`,
        );
      } else {
        setNotice("Channel test dispatched.");
      }
      setChannelsTestCrashOnce(false);
      await refreshChannelLogs(connectorId);
      await refreshChannels(connectorId);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function submitChannelRouterPreview(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const routeChannel = channelRouterPreviewChannel.trim();
    const text = channelRouterPreviewText.trim();
    if (routeChannel.length === 0) {
      setError("Router preview channel cannot be empty.");
      return;
    }
    if (text.length === 0) {
      setError("Router preview text cannot be empty.");
      return;
    }

    setChannelsBusy(true);
    setError(null);
    try {
      const maxPayloadBytes = parseInteger(channelRouterPreviewMaxPayloadBytes);
      const response = await previewChannelRoute(api, {
        channel: routeChannel,
        text,
        conversation_id: emptyToUndefined(channelRouterPreviewConversationId),
        sender_identity: emptyToUndefined(channelRouterPreviewSenderIdentity),
        sender_display: emptyToUndefined(channelRouterPreviewSenderDisplay),
        sender_verified: channelRouterPreviewSenderVerified,
        is_direct_message: channelRouterPreviewIsDirectMessage,
        requested_broadcast: channelRouterPreviewRequestedBroadcast,
        max_payload_bytes:
          maxPayloadBytes !== null && maxPayloadBytes > 0 ? maxPayloadBytes : undefined,
      });
      setChannelRouterPreviewResult(isJsonObject(response.preview) ? response.preview : null);
      if (isJsonObject(response.preview)) {
        const accepted = response.preview.accepted === true ? "accepted" : "rejected";
        const reason = readString(response.preview, "reason") ?? "unknown";
        setNotice(`Route preview ${accepted}: ${reason}.`);
      } else {
        setNotice("Route preview completed.");
      }
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function refreshChannelRouterPairings(): Promise<void> {
    setChannelsBusy(true);
    setError(null);
    try {
      const filterChannel = channelRouterPairingsFilterChannel.trim();
      await refreshChannelRouter(filterChannel.length > 0 ? filterChannel : undefined);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function mintChannelRouterPairingCode(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const routeChannel = channelRouterMintChannel.trim();
    if (routeChannel.length === 0) {
      setError("Pairing code channel cannot be empty.");
      return;
    }

    const parsedTtl = parseInteger(channelRouterMintTtlMs);
    if (parsedTtl !== null && parsedTtl <= 0) {
      setError("Pairing code TTL must be a positive integer.");
      return;
    }

    setChannelsBusy(true);
    setError(null);
    try {
      const response = await mintChannelRouterPairingCodeRequest(api, {
        channel: routeChannel,
        issued_by: emptyToUndefined(channelRouterMintIssuedBy),
        ttl_ms: parsedTtl !== null ? parsedTtl : undefined,
      });
      setChannelRouterMintResult(isJsonObject(response.code) ? response.code : null);
      await refreshChannelRouter(
        channelRouterPairingsFilterChannel.trim().length > 0
          ? channelRouterPairingsFilterChannel.trim()
          : routeChannel,
      );
      if (isJsonObject(response.code)) {
        const code = readString(response.code, "code") ?? "(missing)";
        setNotice(`Pairing code minted: ${code}.`);
      } else {
        setNotice("Pairing code minted.");
      }
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function pauseChannelQueue(): Promise<void> {
    if (channelsSelectedConnectorId.trim().length === 0) {
      setError("Select a connector before pausing its queue.");
      return;
    }
    setChannelsBusy(true);
    setError(null);
    try {
      const connectorId = channelsSelectedConnectorId.trim();
      const response = await pauseChannelQueueRequest(api, connectorId);
      setSelectedChannelStatusPayload(response as JsonValue);
      setNotice("Channel queue paused.");
      await refreshChannelLogs(connectorId);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function resumeChannelQueue(): Promise<void> {
    if (channelsSelectedConnectorId.trim().length === 0) {
      setError("Select a connector before resuming its queue.");
      return;
    }
    setChannelsBusy(true);
    setError(null);
    try {
      const connectorId = channelsSelectedConnectorId.trim();
      const response = await resumeChannelQueueRequest(api, connectorId);
      setSelectedChannelStatusPayload(response as JsonValue);
      setNotice("Channel queue resumed.");
      await refreshChannelLogs(connectorId);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function drainChannelQueue(): Promise<void> {
    if (channelsSelectedConnectorId.trim().length === 0) {
      setError("Select a connector before draining its queue.");
      return;
    }
    setChannelsBusy(true);
    setError(null);
    try {
      const connectorId = channelsSelectedConnectorId.trim();
      const response = await drainChannelQueueRequest(api, connectorId);
      setSelectedChannelStatusPayload(response as JsonValue);
      setNotice("Channel queue drain completed.");
      await refreshChannelLogs(connectorId);
      await refreshChannels(connectorId);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function replayChannelDeadLetter(deadLetterId: number): Promise<void> {
    if (channelsSelectedConnectorId.trim().length === 0) {
      setError("Select a connector before replaying dead letters.");
      return;
    }
    setChannelsBusy(true);
    setError(null);
    try {
      const connectorId = channelsSelectedConnectorId.trim();
      const response = await replayChannelDeadLetterRequest(api, connectorId, deadLetterId);
      setSelectedChannelStatusPayload(response as JsonValue);
      setNotice(`Dead letter ${deadLetterId} replayed.`);
      await refreshChannelLogs(connectorId);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function discardChannelDeadLetter(deadLetterId: number): Promise<void> {
    if (channelsSelectedConnectorId.trim().length === 0) {
      setError("Select a connector before discarding dead letters.");
      return;
    }
    setChannelsBusy(true);
    setError(null);
    try {
      const connectorId = channelsSelectedConnectorId.trim();
      const response = await discardChannelDeadLetterRequest(api, connectorId, deadLetterId);
      setSelectedChannelStatusPayload(response as JsonValue);
      setNotice(`Dead letter ${deadLetterId} discarded.`);
      await refreshChannelLogs(connectorId);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  return {
    refreshChannelLogs,
    refreshChannelRouter,
    refreshChannels,
    loadChannel,
    setChannelEnabled,
    submitChannelTestMessage,
    submitChannelRouterPreview,
    refreshChannelRouterPairings,
    mintChannelRouterPairingCode,
    pauseChannelQueue,
    resumeChannelQueue,
    drainChannelQueue,
    replayChannelDeadLetter,
    discardChannelDeadLetter,
  };
}
