import { describe, expect, it } from "vitest";

import { createDemoDocument } from "../sample";
import { processPatchQueue } from "../runtime";
import type { PatchDocument, PatchProcessingBudget } from "../types";

describe("A2UI runtime patch queue", () => {
  it("yields when patch processing budgets are exhausted", () => {
    const patches: PatchDocument[] = [
      {
        v: 1,
        ops: [
          {
            op: "replace",
            path: "/components/0/props/value",
            value: "budget-step-1"
          },
          {
            op: "replace",
            path: "/components/5/props/series/0/value",
            value: 7
          }
        ]
      },
      {
        v: 1,
        ops: [
          {
            op: "replace",
            path: "/components/0/props/value",
            value: "budget-step-2"
          },
          {
            op: "replace",
            path: "/components/5/props/series/1/value",
            value: 9
          }
        ]
      }
    ];

    const budget: PatchProcessingBudget = {
      maxOpsPerPatch: 256,
      maxOpsPerTick: 2,
      maxQueueDepth: 128,
      maxPathLength: 512,
      maxApplyMsPerTick: 32
    };

    const result = processPatchQueue(createDemoDocument(), patches, budget);
    expect(result.appliedPatches).toBe(1);
    expect(result.remainingPatches).toHaveLength(1);
    expect(result.exhaustedBudget).toBe(true);
    expect(result.nextDocument.components[0].type).toBe("text");
  });
});
