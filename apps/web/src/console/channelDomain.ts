import type { Dispatch, FormEvent, SetStateAction } from "react";

import { type ConsoleApiClient, type JsonValue } from "../consoleApi";
import {
  emptyToUndefined,
  isJsonObject,
  isVisibleChannelConnector,
  parseInteger,
  readString,
  toErrorMessage,
  toJsonObjectArray,
  toStringArray,
  type JsonObject
} from "./shared";

type Setter<T> = Dispatch<SetStateAction<T>>;

type ChannelDomainDeps = {
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
  channelsDiscordTarget: string;
  channelsDiscordText: string;
  channelsDiscordAutoReaction: string;
  channelsDiscordThreadId: string;
  channelsDiscordConfirm: boolean;
  discordWizardVerifyChannelId: string;
  setChannelsBusy: Setter<boolean>;
  setError: Setter<string | null>;
  setNotice: Setter<string | null>;
  setChannelsConnectors: Setter<JsonObject[]>;
  setChannelsSelectedConnectorId: Setter<string>;
  setChannelsEvents: Setter<JsonObject[]>;
  setChannelsDeadLetters: Setter<JsonObject[]>;
  setChannelsTestCrashOnce: Setter<boolean>;
  setChannelsDiscordConfirm: Setter<boolean>;
  setChannelRouterRules: Setter<JsonObject | null>;
  setChannelRouterConfigHash: Setter<string>;
  setChannelRouterWarnings: Setter<string[]>;
  setChannelRouterPairings: Setter<JsonObject[]>;
  setChannelRouterPreviewChannel: Setter<string>;
  setChannelRouterMintChannel: Setter<string>;
  setChannelRouterPairingsFilterChannel: Setter<string>;
  setSelectedChannelStatusPayload: (payload: JsonValue) => void;
};

export function createChannelDomain(deps: ChannelDomainDeps) {
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
    channelsDiscordTarget,
    channelsDiscordText,
    channelsDiscordAutoReaction,
    channelsDiscordThreadId,
    channelsDiscordConfirm,
    discordWizardVerifyChannelId,
    setChannelsBusy,
    setError,
    setNotice,
    setChannelsConnectors,
    setChannelsSelectedConnectorId,
    setChannelsEvents,
    setChannelsDeadLetters,
    setChannelsTestCrashOnce,
    setChannelsDiscordConfirm,
    setChannelRouterRules,
    setChannelRouterConfigHash,
    setChannelRouterWarnings,
    setChannelRouterPairings,
    setChannelRouterPreviewChannel,
    setChannelRouterMintChannel,
    setChannelRouterPairingsFilterChannel,
    setSelectedChannelStatusPayload
  } = deps;

  async function refreshChannelLogs(connectorId: string): Promise<void> {
    const params = new URLSearchParams();
    const parsedLimit = parseInteger(channelsLogsLimit);
    if (parsedLimit !== null && parsedLimit > 0) {
      params.set("limit", String(parsedLimit));
    }
    const response = await api.listChannelLogs(connectorId, params.size > 0 ? params : undefined);
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
      api.getChannelRouterRules(),
      api.getChannelRouterWarnings(),
      api.listChannelRouterPairings(pairingsParams.size > 0 ? pairingsParams : undefined)
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
      const response = await api.listChannels();
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

      const statusResponse = await api.getChannelStatus(nextConnectorId);
      setSelectedChannelStatusPayload(statusResponse as JsonValue);
      setChannelRouterPreviewChannel((previous) =>
        previous.trim().length > 0 ? previous : nextConnectorId
      );
      setChannelRouterMintChannel((previous) =>
        previous.trim().length > 0 ? previous : nextConnectorId
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
      const statusResponse = await api.getChannelStatus(normalizedConnectorId);
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
      await api.setChannelEnabled(connectorId, enabled);
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
      const response = await api.sendChannelTestMessage(connectorId, {
        text: channelsTestText.trim(),
        conversation_id: emptyToUndefined(channelsTestConversationId),
        sender_id: emptyToUndefined(channelsTestSenderId),
        sender_display: emptyToUndefined(channelsTestSenderDisplay),
        simulate_crash_once: channelsTestCrashOnce,
        is_direct_message: channelsTestDirectMessage,
        requested_broadcast: channelsTestBroadcast
      });
      if (isJsonObject(response.ingest)) {
        const accepted = response.ingest.accepted === true ? "true" : "false";
        const immediateDeliveryValue = response.ingest.immediate_delivery;
        const immediateDelivery =
          typeof immediateDeliveryValue === "number" || typeof immediateDeliveryValue === "string"
            ? String(immediateDeliveryValue)
            : "0";
        setNotice(
          `Channel test dispatched (accepted=${accepted}, immediate_delivery=${immediateDelivery}).`
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

  async function submitChannelDiscordTestSend(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (channelsSelectedConnectorId.trim().length === 0) {
      setError("Select a connector before dispatching Discord test send.");
      return;
    }
    if (!channelsSelectedConnectorId.trim().startsWith("discord:")) {
      setError("Discord test send is available only for Discord connectors.");
      return;
    }
    if (channelsDiscordTarget.trim().length === 0) {
      setError("Discord test target cannot be empty.");
      return;
    }
    if (!channelsDiscordConfirm) {
      setError("Discord test send requires explicit confirmation.");
      return;
    }

    setChannelsBusy(true);
    setError(null);
    try {
      const connectorId = channelsSelectedConnectorId.trim();
      await api.sendChannelDiscordTestSend(connectorId, {
        target: channelsDiscordTarget.trim(),
        text: emptyToUndefined(channelsDiscordText),
        confirm: true,
        auto_reaction: emptyToUndefined(channelsDiscordAutoReaction),
        thread_id: emptyToUndefined(channelsDiscordThreadId)
      });
      setNotice("Discord test send dispatched.");
      setChannelsDiscordConfirm(false);
      await refreshChannelLogs(connectorId);
      await refreshChannels(connectorId);
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

  async function refreshChannelHealth(): Promise<void> {
    if (channelsSelectedConnectorId.trim().length === 0) {
      setError("Select a connector before running health refresh.");
      return;
    }
    setChannelsBusy(true);
    setError(null);
    try {
      const response = await api.refreshChannelHealth(channelsSelectedConnectorId.trim(), {
        verify_channel_id: emptyToUndefined(discordWizardVerifyChannelId)
      });
      setSelectedChannelStatusPayload(response as JsonValue);
      setNotice("Channel health refresh completed.");
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
      const response = await api.pauseChannelQueue(connectorId);
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
      const response = await api.resumeChannelQueue(connectorId);
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
      const response = await api.drainChannelQueue(connectorId);
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
      const response = await api.replayChannelDeadLetter(connectorId, deadLetterId);
      setSelectedChannelStatusPayload(response as JsonValue);
      setNotice(`Dead letter ${deadLetterId} replayed.`);
      await refreshChannelLogs(connectorId);
      await refreshChannels(connectorId);
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
      const response = await api.discardChannelDeadLetter(connectorId, deadLetterId);
      setSelectedChannelStatusPayload(response as JsonValue);
      setNotice(`Dead letter ${deadLetterId} discarded.`);
      await refreshChannelLogs(connectorId);
      await refreshChannels(connectorId);
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
    submitChannelDiscordTestSend,
    refreshChannelRouterPairings,
    refreshChannelHealth,
    pauseChannelQueue,
    resumeChannelQueue,
    drainChannelQueue,
    replayChannelDeadLetter,
    discardChannelDeadLetter
  };
}
