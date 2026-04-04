import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";

import type { ContextReferencePreviewEnvelope, ConsoleApiClient } from "../consoleApi";

import { toErrorMessage } from "./chatShared";

type UseContextReferencePreviewArgs = {
  api: ConsoleApiClient;
  activeSessionId: string;
  composerText: string;
  setComposerText: (value: string) => void;
  setError: (next: string | null) => void;
};

type UseContextReferencePreviewResult = {
  contextReferencePreview: ContextReferencePreviewEnvelope | null;
  contextReferencePreviewBusy: boolean;
  contextReferencePreviewStale: boolean;
  loadContextReferencePreview: (
    text: string,
    options?: { reportError?: boolean },
  ) => Promise<ContextReferencePreviewEnvelope | null>;
  ensureContextReferencePreviewForCurrentDraft: () => Promise<ContextReferencePreviewEnvelope | null>;
  resetContextReferencePreview: () => void;
  removeContextReference: (referenceId: string) => void;
};

export function useContextReferencePreview({
  api,
  activeSessionId,
  composerText,
  setComposerText,
  setError,
}: UseContextReferencePreviewArgs): UseContextReferencePreviewResult {
  const contextReferencePreviewRequestSeqRef = useRef(0);
  const [contextReferencePreviewBusy, setContextReferencePreviewBusy] = useState(false);
  const [contextReferencePreview, setContextReferencePreview] =
    useState<ContextReferencePreviewEnvelope | null>(null);
  const [contextReferencePreviewQuery, setContextReferencePreviewQuery] = useState("");
  const deferredComposerText = useDeferredValue(composerText);

  const contextReferencePreviewStale = useMemo(() => {
    const trimmed = composerText.trim();
    if (trimmed.length === 0 || trimmed.startsWith("/") || !trimmed.includes("@")) {
      return false;
    }
    return contextReferencePreview !== null && contextReferencePreviewQuery !== composerText.trim();
  }, [composerText, contextReferencePreview, contextReferencePreviewQuery]);

  const resetContextReferencePreview = useCallback(() => {
    contextReferencePreviewRequestSeqRef.current += 1;
    setContextReferencePreviewBusy(false);
    setContextReferencePreview(null);
    setContextReferencePreviewQuery("");
  }, []);

  const loadContextReferencePreview = useCallback(
    async (
      text: string,
      options: { reportError?: boolean } = {},
    ): Promise<ContextReferencePreviewEnvelope | null> => {
      const trimmed = text.trim();
      const sessionId = activeSessionId.trim();
      if (
        trimmed.length === 0 ||
        trimmed.startsWith("/") ||
        !trimmed.includes("@") ||
        sessionId.length === 0
      ) {
        resetContextReferencePreview();
        return null;
      }

      contextReferencePreviewRequestSeqRef.current += 1;
      const requestSeq = contextReferencePreviewRequestSeqRef.current;
      setContextReferencePreviewBusy(true);
      try {
        const response = await api.previewChatContextReferences(sessionId, { text: trimmed });
        if (requestSeq !== contextReferencePreviewRequestSeqRef.current) {
          return null;
        }
        setContextReferencePreview(response);
        setContextReferencePreviewQuery(trimmed);
        return response;
      } catch (error) {
        if (
          requestSeq === contextReferencePreviewRequestSeqRef.current &&
          options.reportError !== false
        ) {
          setError(toErrorMessage(error));
        }
        return null;
      } finally {
        if (requestSeq === contextReferencePreviewRequestSeqRef.current) {
          setContextReferencePreviewBusy(false);
        }
      }
    },
    [activeSessionId, api, resetContextReferencePreview, setError],
  );

  const ensureContextReferencePreviewForCurrentDraft = useCallback(async () => {
    const trimmed = composerText.trim();
    if (trimmed.length === 0 || trimmed.startsWith("/") || !trimmed.includes("@")) {
      return null;
    }
    if (contextReferencePreview !== null && contextReferencePreviewQuery === trimmed) {
      return contextReferencePreview;
    }
    return loadContextReferencePreview(trimmed, { reportError: true });
  }, [
    composerText,
    contextReferencePreview,
    contextReferencePreviewQuery,
    loadContextReferencePreview,
  ]);

  useEffect(() => {
    const sessionId = activeSessionId.trim();
    const trimmed = deferredComposerText.trim();
    if (
      sessionId.length === 0 ||
      trimmed.length === 0 ||
      trimmed.startsWith("/") ||
      !trimmed.includes("@")
    ) {
      resetContextReferencePreview();
      return;
    }

    const timeoutHandle = window.setTimeout(() => {
      void loadContextReferencePreview(trimmed, { reportError: false });
    }, 250);

    return () => {
      window.clearTimeout(timeoutHandle);
    };
  }, [
    activeSessionId,
    deferredComposerText,
    loadContextReferencePreview,
    resetContextReferencePreview,
  ]);

  const removeContextReference = useCallback(
    (referenceId: string) => {
      const trimmed = composerText.trim();
      if (
        contextReferencePreview === null ||
        contextReferencePreviewQuery !== trimmed ||
        referenceId.trim().length === 0
      ) {
        return;
      }
      const target = contextReferencePreview.references.find(
        (reference) => reference.reference_id === referenceId,
      );
      if (target === undefined) {
        return;
      }
      const next =
        `${composerText.slice(0, target.start_offset)} ${composerText.slice(target.end_offset)}`
          .replace(/[ \t]{2,}/g, " ")
          .replace(/\s+\n/g, "\n")
          .replace(/\n\s+/g, "\n")
          .trim();
      setComposerText(next);
    },
    [composerText, contextReferencePreview, contextReferencePreviewQuery, setComposerText],
  );

  return {
    contextReferencePreview,
    contextReferencePreviewBusy,
    contextReferencePreviewStale,
    loadContextReferencePreview,
    ensureContextReferencePreviewForCurrentDraft,
    resetContextReferencePreview,
    removeContextReference,
  };
}
