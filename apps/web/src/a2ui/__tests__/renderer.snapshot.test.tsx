import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vite-plus/test";

import { applyPatchDocument } from "../patch";
import { A2uiRenderer } from "../renderer";
import { createDemoDocument } from "../sample";
import type { JsonValue, PatchDocument } from "../types";
import { normalizeA2uiDocument } from "../normalize";

describe("A2uiRenderer coverage", () => {
  it("renders deterministic markup for baseline document", () => {
    const document = createDemoDocument();
    render(<A2uiRenderer document={document} />);

    expect(screen.getByText("Experimental surface")).toBeInTheDocument();
    expect(screen.getByText("native-canvas-preview")).toBeInTheDocument();
    expect(screen.getByText("canvas_host.enabled")).toBeInTheDocument();
    expect(
      screen.getByText("Renderer online. Waiting for incremental patches."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Apply" })).toBeInTheDocument();
    expect(screen.getByRole("grid", { name: "metrics" })).toBeInTheDocument();
    expect(screen.getByText("Patch Loop Latency (ms)")).toBeInTheDocument();
  });

  it("renders deterministic markup after patch updates", () => {
    const patch: PatchDocument = {
      v: 1,
      ops: [
        {
          op: "replace",
          path: "/components/0/props/value",
          value: "Renderer online. Snapshot patch applied.",
        },
        {
          op: "add",
          path: "/components/2/props/items/-",
          value: "Snapshot list extension",
        },
        {
          op: "replace",
          path: "/components/5/props/series/1/value",
          value: 14,
        },
      ],
    };
    const patchedState = applyPatchDocument(createDemoDocument() as unknown as JsonValue, patch);
    const patchedDocument = normalizeA2uiDocument(patchedState);
    render(<A2uiRenderer document={patchedDocument} />);

    expect(screen.getByText("Renderer online. Snapshot patch applied.")).toBeInTheDocument();
    expect(screen.getByText("Snapshot list extension")).toBeInTheDocument();
    expect(screen.getByText("14")).toBeInTheDocument();
  });

  it("rejects ambient experiments without explicit consent", () => {
    expect(() =>
      normalizeA2uiDocument({
        v: 1,
        surface: "ambient-preview",
        experimental: {
          track_id: "ambient-companion",
          feature_flag: "desktop_companion.voice_capture_enabled",
          rollout_stage: "operator_preview",
          ambient_mode: "push_to_talk",
          support_summary: "Manual capture pilot.",
          security_review: ["Keep uploads in the existing media pipeline."],
          exit_criteria: ["Disable on unexpected audio capture."],
        },
        components: [
          {
            id: "status",
            type: "text",
            props: { tone: "normal", value: "Ambient draft" },
          },
        ],
      }),
    ).toThrowError(/consent_required=true/i);
  });
});
