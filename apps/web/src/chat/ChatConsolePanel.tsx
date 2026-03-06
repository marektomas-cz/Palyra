import { useEffect } from "react";

import type { ConsoleApiClient } from "../consoleApi";

import { ChatComposer } from "./ChatComposer";
import { ChatRunDrawer } from "./ChatRunDrawer";
import { ChatSessionsSidebar } from "./ChatSessionsSidebar";
import { ChatTranscript } from "./ChatTranscript";
import { shortId } from "./chatShared";
import { useChatRunStream } from "./useChatRunStream";
import { useChatSessions } from "./useChatSessions";

interface ChatConsolePanelProps {
  readonly api: ConsoleApiClient;
  readonly revealSensitiveValues: boolean;
  readonly setError: (next: string | null) => void;
  readonly setNotice: (next: string | null) => void;
}

export function ChatConsolePanel({
  api,
  revealSensitiveValues,
  setError,
  setNotice
}: ChatConsolePanelProps) {
  const sessions = useChatSessions({
    api,
    setError,
    setNotice
  });

  const {
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
  } = useChatRunStream({
    api,
    activeSessionId: sessions.activeSessionId,
    sessionLabelDraft: sessions.sessionLabelDraft,
    setError,
    setNotice
  });

  useEffect(() => {
    void sessions.refreshSessions(true);
    return () => {
      dispose();
    };
  }, []);

  async function resetSessionAndTranscript(): Promise<void> {
    const resetApplied = await sessions.resetSession();
    if (!resetApplied) {
      return;
    }
    clearTranscriptState();
    setNotice("Session reset applied. Local transcript cleared.");
  }

  return (
    <main className="console-card chat-console-panel">
      <header className="console-card__header">
        <div>
          <h2>Chat Workspace</h2>
          <p className="console-copy">
            Streaming runs, inline approvals, A2UI surfaces, and canvas embeds in one operator-safe view.
          </p>
        </div>
        <div className="console-inline-actions">
          <button
            type="button"
            onClick={() => void sessions.refreshSessions(false)}
            disabled={sessions.sessionsBusy}
          >
            {sessions.sessionsBusy ? "Refreshing..." : "Refresh sessions"}
          </button>
          <button
            type="button"
            onClick={() => {
              if (activeRunId === null) {
                setError("No active run selected.");
                return;
              }
              openRunDetails(activeRunId);
            }}
            disabled={activeRunId === null}
          >
            Run details
          </button>
        </div>
      </header>

      <div className="chat-layout">
        <ChatSessionsSidebar
          sessionsBusy={sessions.sessionsBusy}
          newSessionLabel={sessions.newSessionLabel}
          setNewSessionLabel={sessions.setNewSessionLabel}
          createSession={() => {
            void sessions.createSession();
          }}
          sessionLabelDraft={sessions.sessionLabelDraft}
          setSessionLabelDraft={sessions.setSessionLabelDraft}
          selectedSession={sessions.selectedSession}
          renameSession={() => {
            void sessions.renameSession();
          }}
          resetSession={() => {
            void resetSessionAndTranscript();
          }}
          sortedSessions={sessions.sortedSessions}
          activeSessionId={sessions.activeSessionId}
          setActiveSessionId={sessions.setActiveSessionId}
        />

        <section className="chat-main" aria-label="Conversation stream">
          <header className="chat-main-header">
            <div>
              <h3>
                {sessions.selectedSession === null
                  ? "No active session"
                  : sessions.selectedSession.session_label?.trim().length
                    ? sessions.selectedSession.session_label
                    : shortId(sessions.selectedSession.session_id)}
              </h3>
              <p className="chat-muted">
                {activeRunId === null ? "No active run" : `Active run: ${activeRunId}`}
              </p>
            </div>
            <label className="console-checkbox-inline">
              <input
                type="checkbox"
                checked={allowSensitiveTools}
                onChange={(event) => setAllowSensitiveTools(event.target.checked)}
              />
              Allow sensitive tools for next run
            </label>
          </header>

          <ChatTranscript
            visibleTranscript={visibleTranscript}
            hiddenTranscriptItems={hiddenTranscriptItems}
            transcriptBoxRef={transcriptBoxRef}
            approvalDrafts={approvalDrafts}
            a2uiDocuments={a2uiDocuments}
            revealSensitiveValues={revealSensitiveValues}
            updateApprovalDraft={updateApprovalDraftValue}
            decideInlineApproval={(approvalId, approved) => {
              void decideInlineApproval(approvalId, approved);
            }}
            openRunDetails={openRunDetails}
          />

          <ChatComposer
            composerText={composerText}
            setComposerText={setComposerText}
            streaming={streaming}
            activeSessionId={sessions.activeSessionId}
            submitMessage={() => {
              void sendMessage(() => sessions.refreshSessions(false));
            }}
            cancelStreaming={cancelStreaming}
            clearTranscript={() => {
              clearTranscriptState();
              setNotice("Local transcript cleared.");
            }}
          />
        </section>
      </div>

      <ChatRunDrawer
        open={runDrawerOpen}
        runIds={runIds}
        runDrawerId={runDrawerId}
        setRunDrawerId={setRunDrawerId}
        runDrawerBusy={runDrawerBusy}
        runStatus={runStatus}
        runTape={runTape}
        revealSensitiveValues={revealSensitiveValues}
        refreshRun={refreshRunDetails}
        close={closeRunDrawer}
      />
    </main>
  );
}
