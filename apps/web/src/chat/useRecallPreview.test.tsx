// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";
import type { ConsoleApiClient, RecallPreviewEnvelope } from "../consoleApi";

import { useRecallPreview } from "./useRecallPreview";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("useRecallPreview", () => {
  it("keeps unsent drafts local until recall preview is explicitly requested", async () => {
    vi.useFakeTimers();
    const previewRecall = vi.fn().mockResolvedValue(recallPreviewEnvelope("rotate prod secret"));
    const api = { previewRecall } as unknown as ConsoleApiClient;

    render(
      <RecallPreviewHarness
        activeSessionId="01ARZ3NDEKTSV4RRFFQ69G5FAV"
        api={api}
        composerText="rotate prod secret"
        selectedChannel="web"
      />,
    );

    vi.advanceTimersByTime(1_000);
    expect(previewRecall).not.toHaveBeenCalled();

    vi.useRealTimers();
    fireEvent.click(screen.getByRole("button", { name: "Refresh recall" }));

    await waitFor(() => {
      expect(previewRecall).toHaveBeenCalledWith({
        query: "rotate prod secret",
        channel: "web",
        session_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        memory_top_k: 4,
        workspace_top_k: 4,
      });
    });
  });
});

function RecallPreviewHarness({
  activeSessionId,
  api,
  composerText,
  selectedChannel,
}: {
  activeSessionId: string;
  api: ConsoleApiClient;
  composerText: string;
  selectedChannel?: string;
}) {
  const { loadRecallPreview } = useRecallPreview({
    activeSessionId,
    api,
    composerText,
    selectedChannel,
    setError: vi.fn(),
  });

  return (
    <button
      type="button"
      onClick={() => void loadRecallPreview(composerText, { reportError: true })}
    >
      Refresh recall
    </button>
  );
}

function recallPreviewEnvelope(query: string): RecallPreviewEnvelope {
  return {
    query,
    memory_hits: [],
    workspace_hits: [],
    parameter_delta: null,
    prompt_preview: "",
    contract: {
      contract_version: "test",
    },
  };
}
