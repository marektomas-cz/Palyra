import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";

import type { ConsoleApiClient, RecallPreviewEnvelope } from "../consoleApi";

import { emptyToUndefined, toErrorMessage } from "./chatShared";

type UseRecallPreviewArgs = {
  api: ConsoleApiClient;
  activeSessionId: string;
  composerText: string;
  selectedChannel?: string;
  setError: (next: string | null) => void;
};

type UseRecallPreviewResult = {
  recallPreview: RecallPreviewEnvelope | null;
  recallPreviewBusy: boolean;
  recallPreviewStale: boolean;
  loadRecallPreview: (
    query: string,
    options?: { reportError?: boolean },
  ) => Promise<RecallPreviewEnvelope | null>;
  ensureRecallPreviewForCurrentDraft: () => Promise<RecallPreviewEnvelope | null>;
  resetRecallPreview: () => void;
};

export function useRecallPreview({
  api,
  activeSessionId,
  composerText,
  selectedChannel,
  setError,
}: UseRecallPreviewArgs): UseRecallPreviewResult {
  const recallPreviewRequestSeqRef = useRef(0);
  const [recallPreviewBusy, setRecallPreviewBusy] = useState(false);
  const [recallPreview, setRecallPreview] = useState<RecallPreviewEnvelope | null>(null);
  const [recallPreviewQuery, setRecallPreviewQuery] = useState("");
  const deferredComposerText = useDeferredValue(composerText);

  const recallPreviewStale = useMemo(() => {
    const trimmed = composerText.trim();
    if (trimmed.length === 0 || trimmed.startsWith("/")) {
      return false;
    }
    return recallPreview !== null && recallPreviewQuery !== trimmed;
  }, [composerText, recallPreview, recallPreviewQuery]);

  const resetRecallPreview = useCallback(() => {
    recallPreviewRequestSeqRef.current += 1;
    setRecallPreviewBusy(false);
    setRecallPreview(null);
    setRecallPreviewQuery("");
  }, []);

  const loadRecallPreview = useCallback(
    async (
      query: string,
      options: { reportError?: boolean } = {},
    ): Promise<RecallPreviewEnvelope | null> => {
      const trimmed = query.trim();
      const sessionId = activeSessionId.trim();
      if (trimmed.length === 0 || trimmed.startsWith("/") || sessionId.length === 0) {
        resetRecallPreview();
        return null;
      }

      recallPreviewRequestSeqRef.current += 1;
      const requestSeq = recallPreviewRequestSeqRef.current;
      setRecallPreviewBusy(true);
      try {
        const response = await api.previewRecall({
          query: trimmed,
          channel: emptyToUndefined(selectedChannel ?? ""),
          session_id: sessionId,
          memory_top_k: 4,
          workspace_top_k: 4,
        });
        if (requestSeq !== recallPreviewRequestSeqRef.current) {
          return null;
        }
        setRecallPreview(response);
        setRecallPreviewQuery(trimmed);
        return response;
      } catch (error) {
        if (requestSeq === recallPreviewRequestSeqRef.current && options.reportError !== false) {
          setError(toErrorMessage(error));
        }
        return null;
      } finally {
        if (requestSeq === recallPreviewRequestSeqRef.current) {
          setRecallPreviewBusy(false);
        }
      }
    },
    [activeSessionId, api, resetRecallPreview, selectedChannel, setError],
  );

  const ensureRecallPreviewForCurrentDraft = useCallback(async (): Promise<RecallPreviewEnvelope | null> => {
    const trimmed = composerText.trim();
    if (trimmed.length === 0 || trimmed.startsWith("/")) {
      return null;
    }
    if (recallPreview !== null && recallPreviewQuery === trimmed) {
      return recallPreview;
    }
    return loadRecallPreview(trimmed, { reportError: true });
  }, [composerText, loadRecallPreview, recallPreview, recallPreviewQuery]);

  useEffect(() => {
    const sessionId = activeSessionId.trim();
    const trimmed = deferredComposerText.trim();
    if (sessionId.length === 0 || trimmed.length === 0 || trimmed.startsWith("/")) {
      resetRecallPreview();
      return;
    }

    const timeoutHandle = window.setTimeout(() => {
      void loadRecallPreview(trimmed, { reportError: false });
    }, 350);

    return () => {
      window.clearTimeout(timeoutHandle);
    };
  }, [activeSessionId, deferredComposerText, loadRecallPreview, resetRecallPreview]);

  return {
    recallPreview,
    recallPreviewBusy,
    recallPreviewStale,
    loadRecallPreview,
    ensureRecallPreviewForCurrentDraft,
    resetRecallPreview,
  };
}
