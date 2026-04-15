import { useDeferredValue, useMemo } from "react";

import type {
  ChatRunStatusRecord,
  ChatTranscriptRecord,
  ConsoleApiClient,
  SessionCatalogRecord,
} from "../consoleApi";

import { buildSessionLineageHint, type TranscriptEntry } from "./chatShared";
import { useChatSessionQuickControls } from "./useChatSessionQuickControls";

type UseChatPanelViewStateArgs = {
  readonly api: ConsoleApiClient;
  readonly selectedSession: SessionCatalogRecord | null;
  readonly upsertSession: (session: SessionCatalogRecord, options?: { select?: boolean }) => void;
  readonly visibleTranscript: TranscriptEntry[];
  readonly hiddenTranscriptItems: number;
  readonly a2uiDocuments: Record<string, unknown>;
  readonly runIds: string[];
  readonly sessionRuns: ChatRunStatusRecord[];
  readonly runDrawerOpen: boolean;
  readonly activeRunId: string | null;
  readonly runDrawerId: string;
  readonly transcriptSearchQuery: string;
  readonly transcriptRecords: ChatTranscriptRecord[];
  readonly setError: (next: string | null) => void;
  readonly setNotice: (next: string | null) => void;
};

export function useChatPanelViewState({
  api,
  selectedSession,
  upsertSession,
  visibleTranscript,
  hiddenTranscriptItems,
  a2uiDocuments,
  runIds,
  sessionRuns,
  runDrawerOpen,
  activeRunId,
  runDrawerId,
  transcriptSearchQuery,
  transcriptRecords,
  setError,
  setNotice,
}: UseChatPanelViewStateArgs) {
  const {
    filteredTranscript,
    filteredHiddenTranscriptItems,
    sessionQuickControlHeaderProps,
    sessionQuickControlPanelProps,
  } = useChatSessionQuickControls({
    api,
    selectedSession,
    visibleTranscript,
    hiddenTranscriptItems,
    setError,
    setNotice,
    upsertSession,
  });

  const pendingApprovalCount = useMemo(
    () =>
      filteredTranscript.filter(
        (entry) => entry.kind === "approval_request" && typeof entry.approval_id === "string",
      ).length,
    [filteredTranscript],
  );
  const a2uiSurfaces = useMemo(() => Object.keys(a2uiDocuments), [a2uiDocuments]);
  const knownRunIds = useMemo(() => {
    const ordered = new Set<string>();
    for (const runId of runIds) {
      ordered.add(runId);
    }
    for (const run of [...sessionRuns].reverse()) {
      ordered.add(run.run_id);
    }
    return Array.from(ordered);
  }, [runIds, sessionRuns]);
  const inspectorVisible = runDrawerOpen || knownRunIds.length > 0;
  const actionableRunId =
    activeRunId ??
    (runDrawerId.trim().length > 0 ? runDrawerId.trim() : null) ??
    knownRunIds[0] ??
    null;
  const toolPayloadCount = useMemo(
    () => filteredTranscript.filter((entry) => entry.payload !== undefined).length,
    [filteredTranscript],
  );
  const recentTranscriptRecords = useMemo(
    () => [...transcriptRecords].slice(-8).reverse(),
    [transcriptRecords],
  );
  const deferredSearchQuery = useDeferredValue(transcriptSearchQuery);
  const selectedSessionLineage = useMemo(
    () => buildSessionLineageHint(selectedSession),
    [selectedSession],
  );

  return {
    filteredTranscript,
    filteredHiddenTranscriptItems,
    sessionQuickControlHeaderProps,
    sessionQuickControlPanelProps,
    pendingApprovalCount,
    a2uiSurfaces,
    knownRunIds,
    inspectorVisible,
    actionableRunId,
    toolPayloadCount,
    recentTranscriptRecords,
    deferredSearchQuery,
    selectedSessionLineage,
  };
}
