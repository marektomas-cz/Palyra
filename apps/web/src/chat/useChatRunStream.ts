import { useEffect, useMemo, useRef, useState } from "react";

import {
  applyPatchDocument,
  documentToJsonValue,
  normalizeA2uiDocument,
  parsePatchDocument,
  type A2uiDocument
} from "../a2ui";
import type {
  ChatRunStatusRecord,
  ChatRunTapeSnapshot,
  ChatStreamEventEnvelope,
  ChatStreamLine,
  ConsoleApiClient,
  JsonValue
} from "../consoleApi";

import {
  DEFAULT_APPROVAL_SCOPE,
  DEFAULT_APPROVAL_TTL_MS,
  MAX_RENDERED_TRANSCRIPT,
  asBoolean,
  asObject,
  asString,
  collectCanvasFrameUrls,
  emptyToUndefined,
  isAbortError,
  normalizePatchValue,
  parseInteger,
  prettifyEventType,
  retainTranscriptWindow,
  toErrorMessage,
  type ApprovalDraft,
  type TranscriptEntry
} from "./chatShared";

type UseChatRunStreamArgs = {
  api: ConsoleApiClient;
  activeSessionId: string;
  sessionLabelDraft: string;
  setError: (next: string | null) => void;
  setNotice: (next: string | null) => void;
};

type UseChatRunStreamResult = {
  composerText: string;
  setComposerText: (value: string) => void;
  allowSensitiveTools: boolean;
  setAllowSensitiveTools: (value: boolean) => void;
  streaming: boolean;
  activeRunId: string | null;
  runDrawerOpen: boolean;
  runDrawerBusy: boolean;
  runDrawerId: string;
  runStatus: ChatRunStatusRecord | null;
  runTape: ChatRunTapeSnapshot | null;
  transcriptBoxRef: React.RefObject<HTMLDivElement | null>;
  approvalDrafts: Record<string, ApprovalDraft>;
  a2uiDocuments: Record<string, A2uiDocument>;
  runIds: string[];
  hiddenTranscriptItems: number;
  visibleTranscript: TranscriptEntry[];
  sendMessage: (onStreamComplete: () => Promise<void>) => Promise<void>;
  cancelStreaming: () => void;
  clearTranscriptState: () => void;
  openRunDetails: (runId: string) => void;
  closeRunDrawer: () => void;
  refreshRunDetails: () => void;
  setRunDrawerId: (runId: string) => void;
  updateApprovalDraftValue: (approvalId: string, next: ApprovalDraft) => void;
  decideInlineApproval: (approvalId: string, approved: boolean) => Promise<void>;
  dispose: () => void;
};

export function useChatRunStream({
  api,
  activeSessionId,
  sessionLabelDraft,
  setError,
  setNotice
}: UseChatRunStreamArgs): UseChatRunStreamResult {
  const [composerText, setComposerText] = useState("");
  const [allowSensitiveTools, setAllowSensitiveTools] = useState(false);
  const [streaming, setStreaming] = useState(false);
  const streamAbortRef = useRef<AbortController | null>(null);
  const runDetailsRequestSeqRef = useRef(0);

  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const [runDrawerOpen, setRunDrawerOpen] = useState(false);
  const [runDrawerBusy, setRunDrawerBusy] = useState(false);
  const [runDrawerId, setRunDrawerId] = useState("");
  const [runStatus, setRunStatus] = useState<ChatRunStatusRecord | null>(null);
  const [runTape, setRunTape] = useState<ChatRunTapeSnapshot | null>(null);

  const [transcript, setTranscript] = useState<TranscriptEntry[]>([]);
  const transcriptBoxRef = useRef<HTMLDivElement | null>(null);
  const assistantEntryByRunRef = useRef<Map<string, string>>(new Map());
  const canvasEntrySetRef = useRef<Set<string>>(new Set());

  const [approvalDrafts, setApprovalDrafts] = useState<Record<string, ApprovalDraft>>({});
  const [a2uiDocuments, setA2uiDocuments] = useState<Record<string, A2uiDocument>>({});
  const a2uiDocumentsRef = useRef<Record<string, A2uiDocument>>({});

  const runIds = useMemo(() => {
    const values = new Set<string>();
    for (const entry of transcript) {
      if (typeof entry.run_id === "string" && entry.run_id.length > 0) {
        values.add(entry.run_id);
      }
    }
    return Array.from(values).reverse();
  }, [transcript]);

  const hiddenTranscriptItems = Math.max(0, transcript.length - MAX_RENDERED_TRANSCRIPT);
  const visibleTranscript = useMemo(() => {
    if (transcript.length <= MAX_RENDERED_TRANSCRIPT) {
      return transcript;
    }
    return transcript.slice(-MAX_RENDERED_TRANSCRIPT);
  }, [transcript]);

  useEffect(() => {
    return () => {
      streamAbortRef.current?.abort();
      streamAbortRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (transcriptBoxRef.current === null) {
      return;
    }
    transcriptBoxRef.current.scrollTop = transcriptBoxRef.current.scrollHeight;
  }, [visibleTranscript.length]);

  useEffect(() => {
    if (!runDrawerOpen || runDrawerId.trim().length === 0) {
      return;
    }
    void loadRunDetails(runDrawerId.trim());
  }, [runDrawerOpen, runDrawerId]);

  function dispose(): void {
    streamAbortRef.current?.abort();
    streamAbortRef.current = null;
  }

  function clearTranscriptState(): void {
    runDetailsRequestSeqRef.current += 1;
    assistantEntryByRunRef.current.clear();
    canvasEntrySetRef.current.clear();
    setTranscript([]);
    setActiveRunId(null);
    setRunDrawerBusy(false);
    setRunDrawerId("");
    setRunStatus(null);
    setRunTape(null);
    a2uiDocumentsRef.current = {};
    setA2uiDocuments({});
    setApprovalDrafts({});
  }

  async function sendMessage(onStreamComplete: () => Promise<void>): Promise<void> {
    if (activeSessionId.trim().length === 0) {
      setError("Select or create a chat session before sending a message.");
      return;
    }
    const trimmed = composerText.trim();
    if (trimmed.length === 0) {
      setError("Message cannot be empty.");
      return;
    }
    if (streaming) {
      setError("A stream is already active. Cancel it first.");
      return;
    }

    setError(null);
    setNotice(null);
    setComposerText("");
    appendTranscriptEntry({
      id: `user-${Date.now()}`,
      kind: "user",
      created_at_unix_ms: Date.now(),
      session_id: activeSessionId,
      title: "You",
      text: trimmed
    });

    const controller = new AbortController();
    streamAbortRef.current = controller;
    setStreaming(true);
    try {
      await api.streamChatMessage(
        activeSessionId,
        {
          text: trimmed,
          allow_sensitive_tools: allowSensitiveTools,
          session_label: emptyToUndefined(sessionLabelDraft)
        },
        {
          signal: controller.signal,
          onLine: handleStreamLine
        }
      );
      await onStreamComplete();
    } catch (error) {
      if (isAbortError(error)) {
        setNotice("Streaming canceled.");
      } else {
        setError(toErrorMessage(error));
      }
    } finally {
      if (streamAbortRef.current === controller) {
        streamAbortRef.current = null;
      }
      setStreaming(false);
    }
  }

  function cancelStreaming(): void {
    if (streamAbortRef.current !== null) {
      streamAbortRef.current.abort();
      streamAbortRef.current = null;
    }
  }

  function handleStreamLine(line: ChatStreamLine): void {
    if (line.type === "meta") {
      setActiveRunId(line.run_id);
      if (runDrawerId.trim().length === 0) {
        setRunDrawerId(line.run_id);
      }
      appendTranscriptEntry({
        id: `meta-${line.run_id}`,
        kind: "meta",
        created_at_unix_ms: Date.now(),
        run_id: line.run_id,
        session_id: line.session_id,
        title: "Run accepted",
        text: `Run ${line.run_id} attached to session ${line.session_id}.`
      });
      return;
    }

    if (line.type === "error") {
      appendTranscriptEntry({
        id: `error-${Date.now()}`,
        kind: "error",
        created_at_unix_ms: Date.now(),
        run_id: line.run_id,
        title: "Stream error",
        text: line.error
      });
      setError(line.error);
      return;
    }

    if (line.type === "complete") {
      appendTranscriptEntry({
        id: `complete-${Date.now()}`,
        kind: "complete",
        created_at_unix_ms: Date.now(),
        run_id: line.run_id,
        title: "Run complete",
        text: `Run status: ${line.status}`,
        status: line.status
      });
      setActiveRunId(line.run_id);
      setRunDrawerId((previous) => (previous.trim().length === 0 ? line.run_id : previous));
      return;
    }

    handleRunEvent(line.event);
  }

  function handleRunEvent(event: ChatStreamEventEnvelope): void {
    const runId = event.run_id;
    if (typeof runId === "string" && runId.length > 0) {
      setActiveRunId(runId);
    }

    if (event.event_type === "model_token") {
      const modelToken = asObject(event.model_token);
      const token = asString(modelToken?.token) ?? "";
      const isFinal = asBoolean(modelToken?.is_final) ?? false;
      if (token.length > 0 || isFinal) {
        appendAssistantToken(runId, token, isFinal);
      }
      return;
    }

    if (event.event_type === "status") {
      const status = asObject(event.status);
      const statusKind = asString(status?.kind) ?? "unknown";
      const message = asString(status?.message) ?? "";
      appendTranscriptEntry({
        id: `status-${Date.now()}`,
        kind: "status",
        created_at_unix_ms: Date.now(),
        run_id: runId,
        title: `Status: ${statusKind}`,
        text: message,
        status: statusKind,
        payload: event
      });
      return;
    }

    if (event.event_type === "tool_approval_request") {
      const request = asObject(event.tool_approval_request);
      const approvalId = asString(request?.approval_id) ?? "";
      const proposalId = asString(request?.proposal_id) ?? "";
      const toolName = asString(request?.tool_name) ?? "tool";
      const summary = asString(request?.request_summary) ?? "Approval required.";
      if (approvalId.length > 0) {
        ensureApprovalDraft(approvalId);
      }
      appendTranscriptEntry({
        id: `approval-request-${approvalId}-${Date.now()}`,
        kind: "approval_request",
        created_at_unix_ms: Date.now(),
        run_id: runId,
        title: `Approval request: ${toolName}`,
        text: summary,
        approval_id: approvalId,
        proposal_id: proposalId,
        tool_name: toolName,
        payload: request ?? event
      });
      return;
    }

    if (event.event_type === "tool_approval_response") {
      appendTranscriptEntry({
        id: `approval-response-${Date.now()}`,
        kind: "approval_response",
        created_at_unix_ms: Date.now(),
        run_id: runId,
        title: "Approval response",
        payload: event.tool_approval_response ?? event
      });
      return;
    }

    if (event.event_type === "a2ui_update") {
      const update = asObject(event.a2ui_update);
      const surface = asString(update?.surface) ?? "chat";
      const patchValue = normalizePatchValue(update?.patch_json);
      if (patchValue !== null) {
        applyA2uiPatch(surface, patchValue);
      }
      appendTranscriptEntry({
        id: `a2ui-${Date.now()}`,
        kind: "a2ui",
        created_at_unix_ms: Date.now(),
        run_id: runId,
        title: `A2UI update: ${surface}`,
        surface,
        payload: update ?? event
      });
      return;
    }

    if (event.event_type === "journal_event") {
      appendTranscriptEntry({
        id: `journal-${Date.now()}`,
        kind: "journal",
        created_at_unix_ms: Date.now(),
        run_id: runId,
        title: "Journal event",
        payload: event.journal_event ?? event
      });
      return;
    }

    if (
      event.event_type === "tool_proposal" ||
      event.event_type === "tool_decision" ||
      event.event_type === "tool_result" ||
      event.event_type === "tool_attestation"
    ) {
      const payloadKey = event.event_type;
      const payloadValue = (event as Record<string, JsonValue>)[payloadKey];
      appendTranscriptEntry({
        id: `${event.event_type}-${Date.now()}`,
        kind: "tool",
        created_at_unix_ms: Date.now(),
        run_id: runId,
        title: prettifyEventType(event.event_type),
        payload: payloadValue ?? event
      });
      const canvasCandidates = collectCanvasFrameUrls(payloadValue ?? event);
      for (const canvasUrl of canvasCandidates) {
        appendCanvasEntry(runId, canvasUrl);
      }
      return;
    }

    appendTranscriptEntry({
      id: `event-${Date.now()}`,
      kind: "event",
      created_at_unix_ms: Date.now(),
      run_id: runId,
      title: prettifyEventType(event.event_type),
      payload: event
    });
  }

  function ensureApprovalDraft(approvalId: string): void {
    setApprovalDrafts((previous) => {
      if (previous[approvalId] !== undefined) {
        return previous;
      }
      return {
        ...previous,
        [approvalId]: {
          scope: DEFAULT_APPROVAL_SCOPE,
          reason: "",
          ttl_ms: DEFAULT_APPROVAL_TTL_MS,
          busy: false
        }
      };
    });
  }

  function applyA2uiPatch(surface: string, patchValue: unknown): void {
    const currentDocument =
      a2uiDocumentsRef.current[surface] ??
      normalizeA2uiDocument({
        v: 1,
        surface,
        components: []
      });
    let nextDocument: A2uiDocument;
    try {
      const patch = parsePatchDocument(patchValue);
      const patchedValue = applyPatchDocument(documentToJsonValue(currentDocument), patch);
      nextDocument = normalizeA2uiDocument(patchedValue);
    } catch (error) {
      setError(`A2UI patch rejected for surface '${surface}': ${toErrorMessage(error)}`);
      return;
    }
    const nextDocuments = {
      ...a2uiDocumentsRef.current,
      [surface]: nextDocument
    };
    a2uiDocumentsRef.current = nextDocuments;
    setA2uiDocuments(nextDocuments);
  }

  function appendCanvasEntry(runId: string, canvasUrl: string): void {
    const key = `${runId}:${canvasUrl}`;
    if (canvasEntrySetRef.current.has(key)) {
      return;
    }
    canvasEntrySetRef.current.add(key);
    appendTranscriptEntry({
      id: `canvas-${Date.now()}`,
      kind: "canvas",
      created_at_unix_ms: Date.now(),
      run_id: runId,
      title: "Canvas",
      canvas_url: canvasUrl,
      text: canvasUrl
    });
  }

  function appendAssistantToken(runId: string, token: string, isFinal: boolean): void {
    setTranscript((previous) => {
      const mappedEntryId = assistantEntryByRunRef.current.get(runId);
      if (mappedEntryId !== undefined) {
        const index = previous.findIndex((entry) => entry.id === mappedEntryId);
        if (index >= 0) {
          const existing = previous[index];
          const nextEntry: TranscriptEntry = {
            ...existing,
            text: `${existing.text ?? ""}${token}`,
            is_final: Boolean(existing.is_final) || isFinal
          };
          const next = [...previous];
          next[index] = nextEntry;
          return retainTranscriptWindow(next);
        }
      }

      const entryId = `assistant-${runId}-${Date.now()}`;
      assistantEntryByRunRef.current.set(runId, entryId);
      const appended: TranscriptEntry = {
        id: entryId,
        kind: "assistant",
        created_at_unix_ms: Date.now(),
        run_id: runId,
        title: "Assistant",
        text: token,
        is_final: isFinal
      };
      return retainTranscriptWindow([...previous, appended]);
    });
  }

  function appendTranscriptEntry(entry: TranscriptEntry): void {
    setTranscript((previous) => retainTranscriptWindow([...previous, entry]));
  }

  function updateApprovalDraft(
    approvalId: string,
    mutator: (draft: ApprovalDraft) => ApprovalDraft
  ): void {
    setApprovalDrafts((previous) => {
      const current =
        previous[approvalId] ??
        ({
          scope: DEFAULT_APPROVAL_SCOPE,
          reason: "",
          ttl_ms: DEFAULT_APPROVAL_TTL_MS,
          busy: false
        } satisfies ApprovalDraft);
      return {
        ...previous,
        [approvalId]: mutator(current)
      };
    });
  }

  function updateApprovalDraftValue(approvalId: string, next: ApprovalDraft): void {
    updateApprovalDraft(approvalId, () => next);
  }

  async function decideInlineApproval(approvalId: string, approved: boolean): Promise<void> {
    const draft = approvalDrafts[approvalId];
    if (draft === undefined) {
      setError("Approval draft state is missing.");
      return;
    }

    updateApprovalDraft(approvalId, (current) => ({ ...current, busy: true }));
    setError(null);
    setNotice(null);
    try {
      const ttl = parseInteger(draft.ttl_ms);
      await api.decideApproval(approvalId, {
        approved,
        reason: emptyToUndefined(draft.reason),
        decision_scope: draft.scope,
        decision_scope_ttl_ms:
          draft.scope === "timeboxed" && ttl !== null && ttl > 0 ? ttl : undefined
      });
      setNotice(approved ? `Approval ${approvalId} allowed.` : `Approval ${approvalId} denied.`);
    } catch (error) {
      setError(toErrorMessage(error));
    } finally {
      updateApprovalDraft(approvalId, (current) => ({ ...current, busy: false }));
    }
  }

  async function loadRunDetails(runId: string): Promise<void> {
    const requestSeq = runDetailsRequestSeqRef.current + 1;
    runDetailsRequestSeqRef.current = requestSeq;
    setRunDrawerBusy(true);
    try {
      const params = new URLSearchParams();
      params.set("limit", "256");
      const [statusResponse, eventsResponse] = await Promise.all([
        api.chatRunStatus(runId),
        api.chatRunEvents(runId, params)
      ]);
      if (requestSeq !== runDetailsRequestSeqRef.current) {
        return;
      }
      setRunStatus(statusResponse.run);
      setRunTape(eventsResponse.tape);
    } catch (error) {
      if (requestSeq !== runDetailsRequestSeqRef.current) {
        return;
      }
      setError(toErrorMessage(error));
    } finally {
      if (requestSeq === runDetailsRequestSeqRef.current) {
        setRunDrawerBusy(false);
      }
    }
  }

  function openRunDetails(runId: string): void {
    setRunDrawerId(runId);
    setRunDrawerOpen(true);
  }

  function closeRunDrawer(): void {
    setRunDrawerOpen(false);
  }

  function refreshRunDetails(): void {
    if (runDrawerId.trim().length > 0) {
      void loadRunDetails(runDrawerId.trim());
    }
  }

  return {
    composerText,
    setComposerText,
    allowSensitiveTools,
    setAllowSensitiveTools,
    streaming,
    activeRunId,
    runDrawerOpen,
    runDrawerBusy,
    runDrawerId,
    runStatus,
    runTape,
    transcriptBoxRef,
    approvalDrafts,
    a2uiDocuments,
    runIds,
    hiddenTranscriptItems,
    visibleTranscript,
    sendMessage,
    cancelStreaming,
    clearTranscriptState,
    openRunDetails,
    closeRunDrawer,
    refreshRunDetails,
    setRunDrawerId,
    updateApprovalDraftValue,
    decideInlineApproval,
    dispose
  };
}
