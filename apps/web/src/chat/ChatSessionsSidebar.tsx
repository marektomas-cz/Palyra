import type { ChatSessionRecord } from "../consoleApi";

import { shortId } from "./chatShared";

type ChatSessionsSidebarProps = {
  sessionsBusy: boolean;
  newSessionLabel: string;
  setNewSessionLabel: (value: string) => void;
  createSession: () => void;
  sessionLabelDraft: string;
  setSessionLabelDraft: (value: string) => void;
  selectedSession: ChatSessionRecord | null;
  renameSession: () => void;
  resetSession: () => void;
  sortedSessions: ChatSessionRecord[];
  activeSessionId: string;
  setActiveSessionId: (sessionId: string) => void;
};

export function ChatSessionsSidebar({
  sessionsBusy,
  newSessionLabel,
  setNewSessionLabel,
  createSession,
  sessionLabelDraft,
  setSessionLabelDraft,
  selectedSession,
  renameSession,
  resetSession,
  sortedSessions,
  activeSessionId,
  setActiveSessionId
}: ChatSessionsSidebarProps) {
  return (
    <aside className="chat-sessions" aria-label="Chat sessions">
      <h3>Sessions</h3>
      <div className="chat-session-create">
        <label>
          New label
          <input
            value={newSessionLabel}
            onChange={(event) => setNewSessionLabel(event.target.value)}
            placeholder="optional"
          />
        </label>
        <button type="button" onClick={createSession} disabled={sessionsBusy}>
          Create
        </button>
      </div>

      <div className="chat-session-edit">
        <label>
          Active label
          <input
            value={sessionLabelDraft}
            onChange={(event) => setSessionLabelDraft(event.target.value)}
            disabled={selectedSession === null || sessionsBusy}
          />
        </label>
        <div className="console-inline-actions">
          <button type="button" onClick={renameSession} disabled={selectedSession === null || sessionsBusy}>
            Rename
          </button>
          <button type="button" className="button--warn" onClick={resetSession} disabled={selectedSession === null || sessionsBusy}>
            Reset
          </button>
        </div>
      </div>

      <div className="chat-session-list" role="listbox" aria-label="Conversation sessions">
        {sortedSessions.length === 0 ? (
          <p className="chat-muted">No sessions yet.</p>
        ) : (
          sortedSessions.map((session) => {
            const active = session.session_id === activeSessionId;
            const label = session.session_label?.trim().length
              ? session.session_label
              : shortId(session.session_id);
            return (
              <button
                key={session.session_id}
                type="button"
                className={`chat-session-item${active ? " is-active" : ""}`}
                onClick={() => setActiveSessionId(session.session_id)}
                aria-selected={active}
              >
                <span>{label}</span>
                <small>
                  Updated {new Date(session.updated_at_unix_ms).toLocaleTimeString()} · {shortId(session.session_id)}
                </small>
              </button>
            );
          })
        )}
      </div>
    </aside>
  );
}
