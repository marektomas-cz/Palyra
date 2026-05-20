// @vitest-environment jsdom

import type { ComponentProps } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import type {
  SessionCanvasDetailEnvelope,
  SessionCanvasSummary,
  SessionCatalogRecord,
} from "../consoleApi";
import { ChatCanvasWorkspaceView } from "./ChatCanvasWorkspaceView";

afterEach(() => {
  cleanup();
});

describe("ChatCanvasWorkspaceView", () => {
  it("honors the sensitive reveal setting for persisted canvas state", () => {
    const session = sampleSession();
    const canvas = sampleCanvasSummary(session.session_id);
    const selectedCanvas = sampleCanvasDetail(session, canvas);
    const props = baseCanvasWorkspaceProps(selectedCanvas);

    const { rerender } = render(
      <ChatCanvasWorkspaceView {...props} revealSensitiveValues={false} />,
    );

    const redactedState = getCanvasStateBlock();
    expect(redactedState).toHaveTextContent('"apiToken": "[redacted]"');
    expect(redactedState).toHaveTextContent('"publicNote": "[redacted]"');
    expect(redactedState).toHaveTextContent('"safeLabel": "visible"');
    expect(redactedState).not.toHaveTextContent("sk-testsecret123");
    expect(redactedState).not.toHaveTextContent("Bearer canvas-runtime-token");

    rerender(<ChatCanvasWorkspaceView {...props} revealSensitiveValues />);

    const revealedState = getCanvasStateBlock();
    expect(revealedState).toHaveTextContent("sk-testsecret123");
    expect(revealedState).toHaveTextContent("Bearer canvas-runtime-token");
  });
});

function getCanvasStateBlock(): HTMLElement {
  return screen.getByText((content, element) => {
    return element?.tagName.toLowerCase() === "pre" && content.includes('"safeLabel": "visible"');
  });
}

function baseCanvasWorkspaceProps(
  selectedCanvas: SessionCanvasDetailEnvelope,
): ComponentProps<typeof ChatCanvasWorkspaceView> {
  return {
    canvases: [selectedCanvas.canvas],
    canvasesBusy: false,
    canvasDetailBusy: false,
    pinnedCanvasId: null,
    revealSensitiveValues: false,
    restoringStateVersion: null,
    runtimeFrameUrl: null,
    selectedCanvas,
    selectedCanvasId: selectedCanvas.canvas.canvas_id,
    selectedSessionBranchState: "root",
    selectedSessionContextFileCount: 0,
    selectedSessionFamilyLabel: null,
    selectedSessionLineage: "Root session",
    selectedSessionTitle: selectedCanvas.session.title,
    selectedSessionTitleState: "ready",
    sessionsBusy: false,
    sessionsSidebarProps: {
      sessionsBusy: false,
      newSessionLabel: "",
      setNewSessionLabel: vi.fn(),
      searchQuery: "",
      setSearchQuery: vi.fn(),
      includeArchived: false,
      setIncludeArchived: vi.fn(),
      sessionLabelDraft: "",
      setSessionLabelDraft: vi.fn(),
      selectedSession: selectedCanvas.session,
      renameSession: vi.fn(),
      resetSession: vi.fn(),
      archiveSession: vi.fn(),
      sortedSessions: [selectedCanvas.session],
      activeSessionId: selectedCanvas.session.session_id,
      setActiveSessionId: vi.fn(),
      createSession: vi.fn(),
    },
    onOpenConversation: vi.fn(),
    onOpenSourceRun: vi.fn(),
    onRefresh: vi.fn(),
    onRestoreCanvas: vi.fn(),
    onSelectCanvas: vi.fn(),
    onTogglePinnedCanvas: vi.fn(),
  };
}

function sampleCanvasDetail(
  session: SessionCatalogRecord,
  canvas: SessionCanvasSummary,
): SessionCanvasDetailEnvelope {
  return {
    contract: { contract_version: "console.v1" },
    session,
    canvas,
    runtime: null,
    runtime_error: null,
    state: {
      apiToken: "sk-testsecret123",
      publicNote: "Bearer canvas-runtime-token",
      safeLabel: "visible",
    },
    revisions: [],
  };
}

function sampleCanvasSummary(sessionId: string): SessionCanvasSummary {
  return {
    canvas_id: "canvas_01ARZ3NDEKTSV4RRFFQ69G5FAV",
    session_id: sessionId,
    state_version: 3,
    state_schema_version: 1,
    created_at_unix_ms: 1,
    updated_at_unix_ms: 2,
    expires_at_unix_ms: 3,
    closed: false,
    runtime_status: "ready",
    reference: {
      source_run_id: "run_01ARZ3NDEKTSV4RRFFQ69G5FAV",
      last_referenced_at_unix_ms: 2,
    },
  };
}

function sampleSession(): SessionCatalogRecord {
  return {
    session_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
    session_key: "session-local",
    session_label: "Local session",
    principal: "admin:local",
    device_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
    channel: "web",
    created_at_unix_ms: 1,
    updated_at_unix_ms: 2,
    title: "Local session",
    title_source: "session_label",
    title_generation_state: "ready",
    manual_title_locked: true,
    auto_title_updated_at_unix_ms: 1,
    manual_title_updated_at_unix_ms: 1,
    preview: "Preview",
    preview_state: "ready",
    last_intent_state: "missing",
    last_summary_state: "missing",
    branch_state: "root",
    prompt_tokens: 0,
    completion_tokens: 0,
    total_tokens: 0,
    archived: false,
    pending_approvals: 0,
    has_context_files: false,
    agent_id: "default",
    model_profile: "gpt-5.4",
    artifact_count: 0,
    family: {
      root_title: "Local session",
      sequence: 1,
      family_size: 1,
      relatives: [],
    },
    recap: {
      touched_files: [],
      active_context_files: [],
      recent_artifacts: [],
      ctas: [],
    },
    quick_controls: {
      agent: {
        value: "default",
        display_value: "Default agent",
        source: "default",
        inherited_value: "default",
        override_active: false,
      },
      model: {
        value: "gpt-5.4",
        display_value: "gpt-5.4",
        source: "default",
        inherited_value: "gpt-5.4",
        override_active: false,
      },
      thinking: {
        value: true,
        source: "surface_default",
        inherited_value: true,
        override_active: false,
      },
      trace: {
        value: false,
        source: "surface_default",
        inherited_value: false,
        override_active: false,
      },
      verbose: {
        value: false,
        source: "surface_default",
        inherited_value: false,
        override_active: false,
      },
      reset_to_default_available: false,
    },
  };
}
