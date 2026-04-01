// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import { ChatComposer } from "./ChatComposer";
import { ChatTranscript } from "./ChatTranscript";
import {
  buildContextBudgetSummary,
  parseSlashCommand,
  type ComposerAttachment,
  type TranscriptEntry,
} from "./chatShared";

afterEach(() => {
  cleanup();
});

describe("Chat web UX primitives", () => {
  it("shows slash command help, attachment preview, budget warning, and drag-drop upload in the composer", () => {
    const attachFiles = vi.fn();
    const removeAttachment = vi.fn();
    const queueFollowUp = vi.fn();
    const attachment = sampleAttachment();

    render(
      <ChatComposer
        composerText="/queue Inspect backlog after deploy"
        setComposerText={vi.fn()}
        streaming={false}
        activeSessionId="01ARZ3NDEKTSV4RRFFQ69G5FAV"
        attachments={[attachment]}
        attachmentBusy={false}
        canQueueFollowUp
        submitMessage={vi.fn()}
        retryLast={vi.fn()}
        branchSession={vi.fn()}
        queueFollowUp={queueFollowUp}
        cancelStreaming={vi.fn()}
        clearTranscript={vi.fn()}
        openAttachmentPicker={vi.fn()}
        removeAttachment={removeAttachment}
        attachFiles={attachFiles}
        showSlashPalette
        parsedSlashCommand={parseSlashCommand("/queue Inspect backlog after deploy")}
        slashCommandMatches={[
          {
            name: "queue",
            synopsis: "/queue <text>",
            description: "Queue a follow-up message for the active run.",
            example: "/queue Inspect backlog after deploy",
          },
        ]}
        useSlashCommand={vi.fn()}
        contextBudget={buildContextBudgetSummary({
          baseline_tokens: 14_500,
          draft_text: "Inspect backlog after deploy and summarize the risky items.",
          attachments: [attachment],
        })}
      />,
    );

    expect(screen.getByText("Operator shortcuts")).toBeInTheDocument();
    expect(screen.getByText("/queue <text>")).toBeInTheDocument();
    expect(screen.getByText("screen.png")).toBeInTheDocument();
    expect(screen.getByText(/approaching the budget/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Queue follow-up" }));
    expect(queueFollowUp).toHaveBeenCalledTimes(1);

    fireEvent.drop(screen.getByLabelText("Message"), {
      dataTransfer: {
        files: [new File(["payload"], "drop.txt", { type: "text/plain" })],
      },
    });
    expect(attachFiles).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(removeAttachment).toHaveBeenCalledWith(attachment.local_id);
  });

  it("moves payload details out of the main transcript and into the inspect callback", () => {
    const inspectPayload = vi.fn();
    const payloadEntry = sampleToolEntry();

    render(
      <ChatTranscript
        visibleTranscript={[payloadEntry]}
        hiddenTranscriptItems={0}
        transcriptBoxRef={{ current: null }}
        approvalDrafts={{}}
        a2uiDocuments={{}}
        selectedDetailId={null}
        updateApprovalDraft={vi.fn()}
        decideInlineApproval={vi.fn()}
        openRunDetails={vi.fn()}
        inspectPayload={inspectPayload}
      />,
    );

    expect(screen.getByText("Payload moved to side panel")).toBeInTheDocument();
    expect(screen.queryByText(/token":"secret/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Inspect payload" }));
    expect(inspectPayload).toHaveBeenCalledWith(payloadEntry);
  });
});

function sampleAttachment(): ComposerAttachment {
  return {
    local_id: "local-1",
    artifact_id: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
    attachment_id: "att-1",
    filename: "screen.png",
    declared_content_type: "image/png",
    content_hash: "sha256:abc",
    size_bytes: 2_048,
    kind: "image",
    budget_tokens: 640,
    preview_url: "data:image/png;base64,AA==",
  };
}

function sampleToolEntry(): TranscriptEntry {
  return {
    id: "tool-1",
    kind: "tool",
    created_at_unix_ms: 100,
    run_id: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
    title: "Tool result",
    text: "The tool completed successfully.",
    payload: {
      token: "secret",
      status: "ok",
    },
  };
}
