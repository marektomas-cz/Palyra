import type { Dispatch, SetStateAction } from "react";

import type {
  ChatCheckpointRecord,
  ConsoleApiClient,
  JsonValue,
  SessionCatalogRecord,
} from "../consoleApi";

import type { DetailPanelState, TranscriptSearchMatch } from "./ChatInspectorColumn";
import {
  exportChatTranscript,
  restoreChatCheckpoint,
  runChatCompactionFlow,
  searchChatTranscript,
} from "./chatConsoleOperations";
import { createUndoCheckpoint } from "./chatSlashActions";
import {
  archiveSessionAndTranscriptAction,
  branchCurrentSessionAction,
  createNewSessionAction,
  queueFollowUpTextAction,
  resetSessionAndTranscriptAction,
  resumeSessionAction,
  retryLatestTurnAction,
} from "./chatSessionActions";
import type { ComposerAttachment, TranscriptEntry } from "./chatShared";
import type { ChatUxMetricKey } from "./useChatSlashPalette";

type SetAttachments = Dispatch<SetStateAction<ComposerAttachment[]>>;
type SetTranscriptSearchResults = Dispatch<SetStateAction<TranscriptSearchMatch[]>>;
type AppendLocalEntry = (entry: Omit<TranscriptEntry, "id" | "created_at_unix_ms">) => void;
type RecordUxMetric = (key: ChatUxMetricKey) => void;
type SendMessage = (
  onComplete: () => Promise<void>,
  options?: {
    text?: string;
    origin_kind?: string;
    origin_run_id?: string;
    parameter_delta?: JsonValue;
  },
) => Promise<boolean>;

export function createChatConsoleSessionHandlers(args: {
  api: ConsoleApiClient;
  activeSessionId: string;
  selectedSession: SessionCatalogRecord | null;
  sortedSessions: SessionCatalogRecord[];
  createSessionWithLabel: (sessionLabel?: string) => Promise<string | null>;
  setActiveSessionId: (sessionId: string) => void;
  upsertSession: (session: SessionCatalogRecord, options?: { select?: boolean }) => void;
  refreshSessions: (ensureSession: boolean) => Promise<void>;
  resetSession: () => Promise<boolean>;
  archiveSession: () => Promise<boolean>;
  clearTranscriptState: () => void;
  setDetailPanel: (next: DetailPanelState | null) => void;
  setTranscriptSearchResults: SetTranscriptSearchResults;
  setTranscriptSearchBusy: (next: boolean) => void;
  setAttachments: SetAttachments;
  setSessionAttachments: () => void;
  setSessionDerivedArtifacts: () => void;
  setComposerText: (value: string) => void;
  setExportBusy: (next: "json" | "markdown" | null) => void;
  setCommandBusy: (next: string | null) => void;
  setSessionMaintenanceBusyKey: (next: string | null) => void;
  setError: (next: string | null) => void;
  setNotice: (next: string | null) => void;
  appendLocalEntry: AppendLocalEntry;
  sendMessage: SendMessage;
  actionableRunId: string | null;
  checkpoints: readonly ChatCheckpointRecord[];
  visibleTranscript: readonly Pick<TranscriptEntry, "run_id" | "kind">[];
  transcriptRecordsLength: number;
  sessionRunsLength: number;
  transcriptSearchQuery: string;
  nextTranscriptSearchRequestSeq: () => number;
  getCurrentTranscriptSearchSeq: () => number;
  refreshSessionTranscript: () => Promise<void>;
  recordUxMetric: RecordUxMetric;
}) {
  const {
    api,
    activeSessionId,
    selectedSession,
    sortedSessions,
    createSessionWithLabel,
    setActiveSessionId,
    upsertSession,
    refreshSessions,
    resetSession,
    archiveSession,
    clearTranscriptState,
    setDetailPanel,
    setTranscriptSearchResults,
    setTranscriptSearchBusy,
    setAttachments,
    setSessionAttachments,
    setSessionDerivedArtifacts,
    setComposerText,
    setExportBusy,
    setCommandBusy,
    setSessionMaintenanceBusyKey,
    setError,
    setNotice,
    appendLocalEntry,
    sendMessage,
    actionableRunId,
    checkpoints,
    visibleTranscript,
    transcriptRecordsLength,
    sessionRunsLength,
    transcriptSearchQuery,
    nextTranscriptSearchRequestSeq,
    getCurrentTranscriptSearchSeq,
    refreshSessionTranscript,
    recordUxMetric,
  } = args;

  return {
    resetSessionAndTranscript: async (): Promise<void> => {
      await resetSessionAndTranscriptAction({
        resetSession,
        clearTranscriptState,
        setDetailPanel,
        setTranscriptSearchResults,
        setAttachments,
        setSessionAttachments,
        setSessionDerivedArtifacts,
        setNotice,
      });
      void refreshSessionTranscript();
    },
    archiveSessionAndTranscript: async (): Promise<void> => {
      await archiveSessionAndTranscriptAction({
        archiveSession,
        clearTranscriptState,
        setDetailPanel,
        setTranscriptSearchResults,
        setAttachments,
        setSessionAttachments,
        setSessionDerivedArtifacts,
        setNotice,
      });
    },
    runCompactionFlow: async (mode: "preview" | "apply"): Promise<void> => {
      await runChatCompactionFlow({
        api,
        activeSessionId,
        mode,
        upsertSession,
        refreshSessionTranscript,
        setDetailPanel,
        appendLocalEntry,
        setCommandBusy,
        setError,
        setNotice,
      });
    },
    createNewSession: async (requestedLabel?: string): Promise<void> => {
      await createNewSessionAction({
        requestedLabel,
        createSessionWithLabel,
        clearTranscriptState,
        setDetailPanel,
        setTranscriptSearchResults,
        setAttachments,
        setComposerText,
        setError,
        setNotice,
      });
    },
    resumeSession: async (rawTarget: string): Promise<void> => {
      resumeSessionAction({
        rawTarget,
        sortedSessions,
        setActiveSessionId,
        setComposerText,
        setError,
        setNotice,
      });
    },
    retryLatestTurn: async (): Promise<void> => {
      await createUndoCheckpoint({
        api,
        activeSessionId,
        transcriptRecordCount: transcriptRecordsLength,
        sessionRunCount: sessionRunsLength,
        source: "retry",
        setNotice,
        recordUxMetric,
      });
      await retryLatestTurnAction({
        api,
        sessionId: activeSessionId.trim(),
        refreshSessions,
        refreshSessionTranscript,
        sendMessage,
        appendLocalEntry,
        setCommandBusy,
        setError,
        setNotice,
      });
    },
    branchCurrentSession: async (requestedLabel?: string): Promise<void> => {
      await branchCurrentSessionAction({
        api,
        sessionId: activeSessionId.trim(),
        requestedLabel,
        upsertSession,
        clearTranscriptState,
        setDetailPanel,
        setAttachments,
        setComposerText,
        refreshSessions,
        refreshSessionTranscript,
        setCommandBusy,
        setError,
        setNotice,
      });
    },
    queueFollowUpText: async (text: string): Promise<void> => {
      await queueFollowUpTextAction({
        api,
        targetRunId: actionableRunId,
        text,
        sessionId: activeSessionId,
        appendLocalEntry,
        refreshSessionTranscript,
        setComposerText,
        setCommandBusy,
        setError,
        setNotice,
      });
    },
    searchTranscript: async (query = transcriptSearchQuery): Promise<void> => {
      const requestSeq = nextTranscriptSearchRequestSeq();
      await searchChatTranscript({
        api,
        activeSessionId,
        query,
        transcriptSearchRequestSeq: requestSeq,
        getCurrentTranscriptSearchSeq,
        upsertSession,
        setTranscriptSearchResults,
        setTranscriptSearchBusy,
        setError,
      });
    },
    exportTranscript: async (format: "json" | "markdown"): Promise<void> => {
      await exportChatTranscript({
        api,
        activeSessionId,
        sessionLabel: selectedSession?.session_label,
        format,
        setExportBusy,
        setError,
        setNotice,
      });
    },
    restoreCheckpoint: async (
      checkpointId: string,
      options?: { source?: "undo" | "checkpoint" | "inspector" },
    ): Promise<void> => {
      await restoreChatCheckpoint({
        api,
        checkpointId,
        checkpoints,
        actionableRunId,
        visibleTranscript,
        selectedSession,
        clearTranscriptState,
        setAttachments,
        refreshSessions: async (preserveSelection = false) => {
          await refreshSessions(preserveSelection);
        },
        refreshSessionTranscript,
        setDetailPanel,
        setSessionMaintenanceBusyKey,
        setError,
        setNotice,
        upsertSession,
        source: options?.source,
      });
    },
  };
}
