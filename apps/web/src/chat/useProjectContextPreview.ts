import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";

import type { ConsoleApiClient, ProjectContextPreviewEnvelope } from "../consoleApi";

import { toErrorMessage } from "./chatShared";

type UseProjectContextPreviewArgs = {
  api: ConsoleApiClient;
  activeSessionId: string;
  composerText: string;
  setError: (next: string | null) => void;
};

type UseProjectContextPreviewResult = {
  projectContextPreview: ProjectContextPreviewEnvelope | null;
  projectContextPreviewBusy: boolean;
  projectContextPreviewStale: boolean;
  projectContextPromptPreview: string | null;
  loadProjectContextPreview: (
    text: string,
    options?: { reportError?: boolean },
  ) => Promise<ProjectContextPreviewEnvelope | null>;
  ensureProjectContextPreviewForCurrentDraft: () => Promise<ProjectContextPreviewEnvelope | null>;
  resetProjectContextPreview: () => void;
};

const PROJECT_CONTEXT_DISCOVERY_PATTERN = /@(?:file|folder|diff|staged)\b/i;

function buildProjectContextDraftKey(text: string): string {
  const trimmed = text.trim();
  return PROJECT_CONTEXT_DISCOVERY_PATTERN.test(trimmed) ? trimmed : "";
}

export function useProjectContextPreview({
  api,
  activeSessionId,
  composerText,
  setError,
}: UseProjectContextPreviewArgs): UseProjectContextPreviewResult {
  const projectContextPreviewRequestSeqRef = useRef(0);
  const activeSessionKeyRef = useRef("");
  const [projectContextPreviewBusy, setProjectContextPreviewBusy] = useState(false);
  const [projectContextPreview, setProjectContextPreview] =
    useState<ProjectContextPreviewEnvelope | null>(null);
  const [projectContextPromptPreview, setProjectContextPromptPreview] = useState<string | null>(
    null,
  );
  const [projectContextPreviewQuery, setProjectContextPreviewQuery] = useState("");
  const [projectContextPreviewSessionId, setProjectContextPreviewSessionId] = useState("");
  const deferredComposerText = useDeferredValue(composerText);
  const activeSessionKey = activeSessionId.trim();
  const projectContextPreviewSessionMatches =
    activeSessionKey.length > 0 && projectContextPreviewSessionId === activeSessionKey;
  const activeProjectContextPreview = projectContextPreviewSessionMatches
    ? projectContextPreview
    : null;
  const activeProjectContextPromptPreview = projectContextPreviewSessionMatches
    ? projectContextPromptPreview
    : null;

  const projectContextPreviewStale = useMemo(() => {
    if (activeSessionKey.length === 0 || activeProjectContextPreview === null) {
      return false;
    }
    return projectContextPreviewQuery !== buildProjectContextDraftKey(composerText);
  }, [activeProjectContextPreview, activeSessionKey, composerText, projectContextPreviewQuery]);

  const resetProjectContextPreview = useCallback(() => {
    projectContextPreviewRequestSeqRef.current += 1;
    setProjectContextPreviewBusy(false);
    setProjectContextPreview(null);
    setProjectContextPromptPreview(null);
    setProjectContextPreviewQuery("");
    setProjectContextPreviewSessionId("");
  }, []);

  const loadProjectContextPreview = useCallback(
    async (
      text: string,
      options: { reportError?: boolean } = {},
    ): Promise<ProjectContextPreviewEnvelope | null> => {
      const sessionId = activeSessionId.trim();
      if (sessionId.length === 0) {
        resetProjectContextPreview();
        return null;
      }
      const query = buildProjectContextDraftKey(text);
      projectContextPreviewRequestSeqRef.current += 1;
      const requestSeq = projectContextPreviewRequestSeqRef.current;
      setProjectContextPreviewBusy(true);
      try {
        const response = await api.previewChatProjectContext(
          sessionId,
          query.length > 0 ? { text: query } : {},
        );
        if (requestSeq !== projectContextPreviewRequestSeqRef.current) {
          return null;
        }
        setProjectContextPreview(response.preview);
        setProjectContextPromptPreview(response.prompt_preview ?? null);
        setProjectContextPreviewQuery(query);
        setProjectContextPreviewSessionId(sessionId);
        return response.preview;
      } catch (error) {
        if (requestSeq === projectContextPreviewRequestSeqRef.current) {
          setProjectContextPreview(null);
          setProjectContextPromptPreview(null);
          setProjectContextPreviewQuery("");
          setProjectContextPreviewSessionId("");
          if (options.reportError !== false) {
            setError(toErrorMessage(error));
          }
        }
        return null;
      } finally {
        if (requestSeq === projectContextPreviewRequestSeqRef.current) {
          setProjectContextPreviewBusy(false);
        }
      }
    },
    [activeSessionId, api, resetProjectContextPreview, setError],
  );

  const ensureProjectContextPreviewForCurrentDraft = useCallback(async () => {
    const query = buildProjectContextDraftKey(composerText);
    if (
      activeProjectContextPreview !== null &&
      projectContextPreviewQuery === query &&
      activeSessionKey.length > 0
    ) {
      return activeProjectContextPreview;
    }
    return loadProjectContextPreview(composerText, { reportError: true });
  }, [
    activeProjectContextPreview,
    activeSessionKey,
    composerText,
    loadProjectContextPreview,
    projectContextPreviewQuery,
  ]);

  useEffect(() => {
    if (activeSessionKey.length === 0) {
      activeSessionKeyRef.current = "";
      resetProjectContextPreview();
      return;
    }
    if (activeSessionKeyRef.current !== activeSessionKey) {
      activeSessionKeyRef.current = activeSessionKey;
      resetProjectContextPreview();
    }
    const timeoutHandle = window.setTimeout(() => {
      void loadProjectContextPreview(deferredComposerText, { reportError: false });
    }, 250);
    return () => {
      window.clearTimeout(timeoutHandle);
    };
  }, [
    activeSessionKey,
    deferredComposerText,
    loadProjectContextPreview,
    resetProjectContextPreview,
  ]);

  return {
    projectContextPreview: activeProjectContextPreview,
    projectContextPreviewBusy,
    projectContextPreviewStale,
    projectContextPromptPreview: activeProjectContextPromptPreview,
    loadProjectContextPreview,
    ensureProjectContextPreviewForCurrentDraft,
    resetProjectContextPreview,
  };
}
