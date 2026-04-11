import type { Dispatch, FormEvent, SetStateAction } from "react";

import { type ConsoleApiClient, type JsonValue } from "../../../consoleApi";
import {
  addChannelMessageReaction as addChannelMessageReactionRequest,
  deleteChannelMessage as deleteChannelMessageRequest,
  editChannelMessage as editChannelMessageRequest,
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
  readChannelMessages as readChannelMessagesRequest,
  replayChannelDeadLetter as replayChannelDeadLetterRequest,
  removeChannelMessageReaction as removeChannelMessageReactionRequest,
  resumeChannelQueue as resumeChannelQueueRequest,
  searchChannelMessages as searchChannelMessagesRequest,
  sendChannelTestMessage,
  setChannelEnabled as setChannelEnabledRequest,
} from "./api";
import {
  emptyToUndefined,
  isJsonObject,
  readObject,
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
  channelMessageConversationId: string;
  channelMessageThreadId: string;
  channelMessageReadMessageId: string;
  channelMessageReadBeforeMessageId: string;
  channelMessageReadAfterMessageId: string;
  channelMessageReadAroundMessageId: string;
  channelMessageReadLimit: string;
  channelMessageSearchQuery: string;
  channelMessageSearchAuthorId: string;
  channelMessageSearchHasAttachments: string;
  channelMessageSearchBeforeMessageId: string;
  channelMessageSearchLimit: string;
  channelMessageMutationMessageId: string;
  channelMessageMutationApprovalId: string;
  channelMessageMutationBody: string;
  channelMessageMutationDeleteReason: string;
  channelMessageMutationEmoji: string;
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
  setChannelMessageConversationId: Setter<string>;
  setChannelMessageThreadId: Setter<string>;
  setChannelMessageMutationMessageId: Setter<string>;
  setChannelMessageMutationApprovalId: Setter<string>;
  setChannelMessageReadResultPayload: (payload: JsonValue) => void;
  setChannelMessageSearchResultPayload: (payload: JsonValue) => void;
  setChannelMessageMutationResultPayload: (payload: JsonValue) => void;
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
    channelMessageConversationId,
    channelMessageThreadId,
    channelMessageReadMessageId,
    channelMessageReadBeforeMessageId,
    channelMessageReadAfterMessageId,
    channelMessageReadAroundMessageId,
    channelMessageReadLimit,
    channelMessageSearchQuery,
    channelMessageSearchAuthorId,
    channelMessageSearchHasAttachments,
    channelMessageSearchBeforeMessageId,
    channelMessageSearchLimit,
    channelMessageMutationMessageId,
    channelMessageMutationApprovalId,
    channelMessageMutationBody,
    channelMessageMutationDeleteReason,
    channelMessageMutationEmoji,
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
    setChannelMessageConversationId,
    setChannelMessageThreadId,
    setChannelMessageMutationMessageId,
    setChannelMessageMutationApprovalId,
    setChannelMessageReadResultPayload,
    setChannelMessageSearchResultPayload,
    setChannelMessageMutationResultPayload,
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

  function normalizedSelectedConnectorId(actionLabel: string): string | null {
    const connectorId = channelsSelectedConnectorId.trim();
    if (connectorId.length === 0) {
      setError(`Select a connector before ${actionLabel}.`);
      return null;
    }
    return connectorId;
  }

  function normalizedMessageLocator(actionLabel: string): {
    connectorId: string;
    conversationId: string;
    threadId?: string;
    messageId: string;
  } | null {
    const connectorId = normalizedSelectedConnectorId(actionLabel);
    if (connectorId === null) {
      return null;
    }
    const conversationId = channelMessageConversationId.trim();
    if (conversationId.length === 0) {
      setError(`Conversation ID is required before ${actionLabel}.`);
      return null;
    }
    const messageId = channelMessageMutationMessageId.trim();
    if (messageId.length === 0) {
      setError(`Message ID is required before ${actionLabel}.`);
      return null;
    }
    return {
      connectorId,
      conversationId,
      threadId: emptyToUndefined(channelMessageThreadId),
      messageId,
    };
  }

  function parsePositiveLimit(raw: string, label: string): number | null {
    const parsed = parseInteger(raw);
    if (parsed === null || parsed <= 0) {
      setError(`${label} must be a positive integer.`);
      return null;
    }
    return parsed;
  }

  function parseHasAttachmentsFilter(): boolean | undefined {
    if (channelMessageSearchHasAttachments === "with") {
      return true;
    }
    if (channelMessageSearchHasAttachments === "without") {
      return false;
    }
    return undefined;
  }

  function seedMutationLocatorFromResult(payload: JsonObject): void {
    const locator =
      readObject(payload, "locator") ??
      readObject(readObject(payload, "message") ?? {}, "locator") ??
      readObject(readObject(payload, "preview") ?? {}, "locator");
    if (locator === null) {
      return;
    }
    const conversationId = readString(locator, "conversation_id");
    const threadId = readString(locator, "thread_id");
    const messageId = readString(locator, "message_id");
    if (conversationId !== null) {
      setChannelMessageConversationId(conversationId);
    }
    setChannelMessageThreadId(threadId ?? "");
    if (messageId !== null) {
      setChannelMessageMutationMessageId(messageId);
    }
  }

  async function readChannelMessages(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const connectorId = normalizedSelectedConnectorId("reading messages");
    if (connectorId === null) {
      return;
    }
    const conversationId = channelMessageConversationId.trim();
    if (conversationId.length === 0) {
      setError("Conversation ID is required before reading messages.");
      return;
    }
    const limit = parsePositiveLimit(channelMessageReadLimit, "Read limit");
    if (limit === null) {
      return;
    }

    setChannelsBusy(true);
    setError(null);
    try {
      const response = await readChannelMessagesRequest(api, connectorId, {
        request: {
          conversation_id: conversationId,
          thread_id: emptyToUndefined(channelMessageThreadId),
          message_id: emptyToUndefined(channelMessageReadMessageId),
          before_message_id: emptyToUndefined(channelMessageReadBeforeMessageId),
          after_message_id: emptyToUndefined(channelMessageReadAfterMessageId),
          around_message_id: emptyToUndefined(channelMessageReadAroundMessageId),
          limit,
        },
      });
      setChannelMessageReadResultPayload(response as JsonValue);
      const result = isJsonObject(response.result) ? response.result : null;
      const messages = result?.messages;
      const count = Array.isArray(messages) ? messages.length : 0;
      setNotice(
        count === 0
          ? "Message read completed with no results."
          : `Loaded ${count} message${count === 1 ? "" : "s"}.`,
      );
      setChannelMessageSearchResultPayload(null);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function searchChannelMessages(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    const connectorId = normalizedSelectedConnectorId("searching messages");
    if (connectorId === null) {
      return;
    }
    const conversationId = channelMessageConversationId.trim();
    if (conversationId.length === 0) {
      setError("Conversation ID is required before searching messages.");
      return;
    }
    const limit = parsePositiveLimit(channelMessageSearchLimit, "Search limit");
    if (limit === null) {
      return;
    }

    setChannelsBusy(true);
    setError(null);
    try {
      const response = await searchChannelMessagesRequest(api, connectorId, {
        request: {
          conversation_id: conversationId,
          thread_id: emptyToUndefined(channelMessageThreadId),
          query: emptyToUndefined(channelMessageSearchQuery),
          author_id: emptyToUndefined(channelMessageSearchAuthorId),
          has_attachments: parseHasAttachmentsFilter(),
          before_message_id: emptyToUndefined(channelMessageSearchBeforeMessageId),
          limit,
        },
      });
      setChannelMessageSearchResultPayload(response as JsonValue);
      const result = isJsonObject(response.result) ? response.result : null;
      const matches = result?.matches;
      const count = Array.isArray(matches) ? matches.length : 0;
      setNotice(
        count === 0
          ? "Message search completed with no matches."
          : `Found ${count} matching message${count === 1 ? "" : "s"}.`,
      );
      setChannelMessageReadResultPayload(null);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setChannelsBusy(false);
    }
  }

  async function mutateChannelMessage(
    action: "edit" | "delete" | "react-add" | "react-remove",
  ): Promise<void> {
    const locator = normalizedMessageLocator(
      action === "edit"
        ? "editing a message"
        : action === "delete"
          ? "deleting a message"
          : "mutating reactions",
    );
    if (locator === null) {
      return;
    }

    if (action === "edit" && channelMessageMutationBody.trim().length === 0) {
      setError("Edit body cannot be empty.");
      return;
    }
    if (
      (action === "react-add" || action === "react-remove") &&
      channelMessageMutationEmoji.trim().length === 0
    ) {
      setError("Emoji is required for reaction mutations.");
      return;
    }

    setChannelsBusy(true);
    setError(null);
    try {
      const payloadLocator = {
        conversation_id: locator.conversationId,
        thread_id: locator.threadId,
        message_id: locator.messageId,
      };
      const approvalId = emptyToUndefined(channelMessageMutationApprovalId);
      const response =
        action === "edit"
          ? await editChannelMessageRequest(api, locator.connectorId, {
              request: {
                locator: payloadLocator,
                body: channelMessageMutationBody.trim(),
              },
              approval_id: approvalId,
            })
          : action === "delete"
            ? await deleteChannelMessageRequest(api, locator.connectorId, {
                request: {
                  locator: payloadLocator,
                  reason: emptyToUndefined(channelMessageMutationDeleteReason),
                },
                approval_id: approvalId,
              })
            : action === "react-add"
              ? await addChannelMessageReactionRequest(api, locator.connectorId, {
                  request: {
                    locator: payloadLocator,
                    emoji: channelMessageMutationEmoji.trim(),
                  },
                  approval_id: approvalId,
                })
              : await removeChannelMessageReactionRequest(api, locator.connectorId, {
                  request: {
                    locator: payloadLocator,
                    emoji: channelMessageMutationEmoji.trim(),
                  },
                  approval_id: approvalId,
                });

      setChannelMessageMutationResultPayload(response as JsonValue);
      if (isJsonObject(response)) {
        seedMutationLocatorFromResult(response);
      }

      if (response.approval_required === true) {
        const approvalPayload = response.approval ?? null;
        const approval = isJsonObject(approvalPayload) ? approvalPayload : null;
        const approvalIdValue = readString(approval ?? {}, "approval_id");
        if (approvalIdValue !== null) {
          setChannelMessageMutationApprovalId(approvalIdValue);
        }
        setNotice("Approval required. Preview prepared, no platform change applied yet.");
      } else {
        const resultPayload = response.result ?? null;
        const result = isJsonObject(resultPayload) ? resultPayload : null;
        const status = readString(result ?? {}, "status") ?? "completed";
        setNotice(`Message ${action} completed (${status}).`);
        await refreshChannelLogs(locator.connectorId);
        await refreshChannels(locator.connectorId);
      }
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
    readChannelMessages,
    searchChannelMessages,
    editChannelMessage: () => mutateChannelMessage("edit"),
    deleteChannelMessage: () => mutateChannelMessage("delete"),
    addChannelMessageReaction: () => mutateChannelMessage("react-add"),
    removeChannelMessageReaction: () => mutateChannelMessage("react-remove"),
  };
}
