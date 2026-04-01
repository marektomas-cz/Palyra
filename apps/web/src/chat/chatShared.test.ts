import { describe, expect, it } from "vite-plus/test";

import {
  buildContextBudgetSummary,
  buildSessionLineageHint,
  parseSlashCommand,
} from "./chatShared";

describe("chatShared helpers", () => {
  it("parses slash commands with arguments", () => {
    expect(parseSlashCommand("/branch Incident follow-up")).toEqual({
      name: "branch",
      args: "Incident follow-up",
    });
    expect(parseSlashCommand("/help")).toEqual({
      name: "help",
      args: "",
    });
  });

  it("builds context budget warnings from draft and attachment estimates", () => {
    const summary = buildContextBudgetSummary({
      baseline_tokens: 15_200,
      draft_text: "Inspect the last failed run and summarize the root cause.",
      attachments: [
        {
          local_id: "local-1",
          artifact_id: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
          attachment_id: "att-1",
          filename: "screen.png",
          declared_content_type: "image/png",
          content_hash: "sha256:abc",
          size_bytes: 2_048,
          kind: "image",
          budget_tokens: 900,
        },
      ],
    });

    expect(summary.tone).toBe("danger");
    expect(summary.warning).toMatch(/above the safe working budget/i);
  });

  it("renders session lineage hints for child branches", () => {
    expect(
      buildSessionLineageHint({
        session_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        session_key: "web",
        title: "Incident follow-up",
        title_source: "manual",
        preview_state: "computed",
        last_intent_state: "computed",
        last_summary_state: "computed",
        branch_state: "branched",
        parent_session_id: "01ARZ3NDEKTSV4RRFFQ69G5FA0",
        principal: "admin:web-console",
        device_id: "device-1",
        created_at_unix_ms: 100,
        updated_at_unix_ms: 100,
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
        archived: false,
        pending_approvals: 0,
      }),
    ).toMatch(/Child branch/i);
  });
});
