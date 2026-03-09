import { describe, expect, it } from "vitest";

import { A2uiError } from "../errors";
import { applyPatchDocument, parsePatchDocument } from "../patch";
import { createDemoDocument } from "../sample";
import type { JsonValue, PatchDocument } from "../types";

describe("A2UI patch security", () => {
  it("rejects forbidden prototype-pollution pointer tokens", () => {
    const baseDocument = createDemoDocument() as unknown as JsonValue;
    const badPatches: PatchDocument[] = [
      {
        v: 1,
        ops: [{ op: "add", path: "/__proto__/polluted", value: "yes" }]
      },
      {
        v: 1,
        ops: [{ op: "add", path: "/components/0/props/__proto__/polluted", value: "yes" }]
      },
      {
        v: 1,
        ops: [{ op: "replace", path: "/constructor/prototype/polluted", value: "yes" }]
      }
    ];

    for (const patch of badPatches) {
      expect(() => parsePatchDocument(patch)).not.toThrow();
      expect(() => applyPatchDocument(baseDocument, patch)).toThrowError(A2uiError);
    }
  });
});
