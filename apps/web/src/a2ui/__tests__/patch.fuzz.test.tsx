import fc from "fast-check";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vite-plus/test";

import { A2uiError } from "../errors";
import { normalizeA2uiDocument } from "../normalize";
import { applyPatchDocument, parsePatchDocument } from "../patch";
import { createDemoDocument } from "../sample";
import { SanitizedMarkdown } from "../markdown";
import type { JsonValue } from "../types";

describe("A2uiRenderer fuzz and malicious payload resilience", () => {
  it("sanitized markdown rendering never emits executable script tags", () => {
    fc.assert(
      fc.property(fc.string({ minLength: 0, maxLength: 256 }), (input) => {
        const markup = renderToStaticMarkup(<SanitizedMarkdown value={input} />);
        const normalizedMarkup = markup.toLowerCase();
        expect(normalizedMarkup.includes("<script")).toBe(false);
        expect(normalizedMarkup.includes("javascript:")).toBe(false);
      }),
      { numRuns: 220 },
    );
  });

  it("patch parser/apply pipeline fails with typed A2UI errors for malformed operations", () => {
    const operationArbitrary = fc.record({
      op: fc.constantFrom("add", "replace", "remove"),
      path: fc.string({ minLength: 0, maxLength: 48 }),
      value: fc.option(fc.jsonValue(), { nil: undefined }),
    });
    const patchArbitrary = fc.record({
      v: fc.constant(1),
      ops: fc.array(operationArbitrary, { minLength: 1, maxLength: 24 }),
    });

    fc.assert(
      fc.property(patchArbitrary, (input) => {
        try {
          const parsed = parsePatchDocument(input);
          const patched = applyPatchDocument(createDemoDocument() as unknown as JsonValue, parsed);
          normalizeA2uiDocument(patched);
        } catch (error) {
          expect(error).toBeInstanceOf(A2uiError);
        }
      }),
      { numRuns: 180 },
    );
  });
});
