import type { SessionCatalogRecord } from "../consoleApi";
import { shortId } from "./chatShared";

type ChatSessionsSlice = {
  sessionsBusy: boolean;
  newSessionLabel: string;
  setNewSessionLabel: (value: string) => void;
  searchQuery: string;
  setSearchQuery: (value: string) => void;
  includeArchived: boolean;
  setIncludeArchived: (value: boolean) => void;
  sessionLabelDraft: string;
  setSessionLabelDraft: (value: string) => void;
  selectedSession: SessionCatalogRecord | null;
  sortedSessions: SessionCatalogRecord[];
  activeSessionId: string;
  setActiveSessionId: (value: string) => void;
};

type BuildSessionsSidebarPropsParams = {
  sessions: ChatSessionsSlice;
  createSession: () => void;
  renameSession: () => void;
  resetSession: () => void;
  archiveSession: () => void;
};

export function describeSelectedSessionTitle(session: SessionCatalogRecord | null): string {
  return session?.title ?? (session ? shortId(session.session_id) : "Operator workspace");
}

export function buildSessionsSidebarProps({
  sessions,
  createSession,
  renameSession,
  resetSession,
  archiveSession,
}: BuildSessionsSidebarPropsParams) {
  return {
    sessionsBusy: sessions.sessionsBusy,
    newSessionLabel: sessions.newSessionLabel,
    setNewSessionLabel: sessions.setNewSessionLabel,
    searchQuery: sessions.searchQuery,
    setSearchQuery: sessions.setSearchQuery,
    includeArchived: sessions.includeArchived,
    setIncludeArchived: sessions.setIncludeArchived,
    sessionLabelDraft: sessions.sessionLabelDraft,
    setSessionLabelDraft: sessions.setSessionLabelDraft,
    selectedSession: sessions.selectedSession,
    sortedSessions: sessions.sortedSessions,
    activeSessionId: sessions.activeSessionId,
    setActiveSessionId: sessions.setActiveSessionId,
    createSession,
    renameSession,
    resetSession,
    archiveSession,
  };
}
