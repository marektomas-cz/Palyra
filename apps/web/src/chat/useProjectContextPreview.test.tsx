// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";
import type { ConsoleApiClient, ProjectContextPreviewEnvelope } from "../consoleApi";

import { useProjectContextPreview } from "./useProjectContextPreview";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("useProjectContextPreview", () => {
  it("does not reuse cached preview state across active sessions", async () => {
    const previewChatProjectContext = vi
      .fn()
      .mockImplementation((sessionId: string, payload: { text?: string }) => {
        if (sessionId === "session-alpha") {
          return Promise.resolve({
            preview: projectContextPreview("alpha-entry", "SECRET PROJECT RULES FROM alpha"),
            prompt_preview: "PROMPT SECRET FROM alpha",
            contract: { contract_version: "test" },
          });
        }
        expect(payload).toEqual({ text: "@file PALYRA.md" });
        return Promise.reject(new Error("beta preview unavailable"));
      });
    const api = { previewChatProjectContext } as unknown as ConsoleApiClient;
    const setError = vi.fn();
    const { rerender } = render(
      <ProjectContextPreviewHarness
        activeSessionId="session-alpha"
        api={api}
        composerText="@file PALYRA.md"
        setError={setError}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Load project context" }));
    await waitFor(() => {
      expect(screen.getByTestId("preview-text")).toHaveTextContent(
        "SECRET PROJECT RULES FROM alpha",
      );
      expect(screen.getByTestId("prompt-preview")).toHaveTextContent("PROMPT SECRET FROM alpha");
    });

    rerender(
      <ProjectContextPreviewHarness
        activeSessionId="session-beta"
        api={api}
        composerText="@file PALYRA.md"
        setError={setError}
      />,
    );

    expect(screen.getByTestId("preview-text")).toHaveTextContent("none");
    expect(screen.getByTestId("prompt-preview")).toHaveTextContent("none");
    expect(screen.getByTestId("stale")).toHaveTextContent("false");

    fireEvent.click(screen.getByRole("button", { name: "Ensure project context" }));
    await waitFor(() => {
      expect(previewChatProjectContext).toHaveBeenCalledWith("session-beta", {
        text: "@file PALYRA.md",
      });
      expect(screen.getByTestId("ensured-preview")).toHaveTextContent("none");
      expect(setError).toHaveBeenCalledWith("beta preview unavailable");
    });
    expect(screen.getByTestId("preview-text")).toHaveTextContent("none");
    expect(screen.getByTestId("prompt-preview")).toHaveTextContent("none");
  });
});

function ProjectContextPreviewHarness({
  activeSessionId,
  api,
  composerText,
  setError,
}: {
  activeSessionId: string;
  api: ConsoleApiClient;
  composerText: string;
  setError: (next: string | null) => void;
}) {
  const [ensuredPreview, setEnsuredPreview] = useState("none");
  const {
    ensureProjectContextPreviewForCurrentDraft,
    loadProjectContextPreview,
    projectContextPreview,
    projectContextPromptPreview,
    projectContextPreviewStale,
  } = useProjectContextPreview({
    activeSessionId,
    api,
    composerText,
    setError,
  });
  const previewText = projectContextPreview?.entries[0]?.preview_text ?? "none";

  return (
    <div>
      <div data-testid="preview-text">{previewText}</div>
      <div data-testid="prompt-preview">{projectContextPromptPreview ?? "none"}</div>
      <div data-testid="stale">{String(projectContextPreviewStale)}</div>
      <div data-testid="ensured-preview">{ensuredPreview}</div>
      <button
        type="button"
        onClick={() => {
          void loadProjectContextPreview(composerText, { reportError: true });
        }}
      >
        Load project context
      </button>
      <button
        type="button"
        onClick={() => {
          void ensureProjectContextPreviewForCurrentDraft().then((preview) => {
            setEnsuredPreview(preview?.entries[0]?.preview_text ?? "none");
          });
        }}
      >
        Ensure project context
      </button>
    </div>
  );
}

function projectContextPreview(
  entryId: string,
  previewText: string,
): ProjectContextPreviewEnvelope {
  return {
    generated_at_unix_ms: 1,
    active_estimated_tokens: 12,
    active_entries: 1,
    blocked_entries: 0,
    approval_required_entries: 0,
    disabled_entries: 0,
    warnings: [],
    focus_paths: [],
    entries: [
      {
        entry_id: entryId,
        order: 1,
        path: "PALYRA.md",
        directory: ".",
        source_kind: "file",
        source_label: "PALYRA.md",
        precedence_label: "project",
        depth: 0,
        root: true,
        active: true,
        disabled: false,
        approved: true,
        status: "active",
        estimated_tokens: 12,
        content_hash: "abc123",
        loaded_at_unix_ms: 1,
        byte_size: 42,
        line_count: 2,
        discovery_reasons: [],
        warnings: [],
        risk: {
          recommended_action: "allow",
          score: 0,
          findings: [],
        },
        preview_text: previewText,
        resolved_text: previewText,
      },
    ],
  };
}
