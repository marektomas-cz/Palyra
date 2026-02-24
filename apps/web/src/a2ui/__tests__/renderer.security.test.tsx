import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { normalizeA2uiDocument } from "../normalize";
import { A2uiRenderer } from "../renderer";

describe("A2uiRenderer XSS regressions", () => {
  it("escapes hostile payloads and blocks javascript links", () => {
    const document = normalizeA2uiDocument({
      v: 1,
      surface: "security-regression",
      components: [
        {
          id: "text",
          type: "text",
          props: {
            value: "<img src=x onerror=alert('xss')>"
          }
        },
        {
          id: "markdown",
          type: "markdown",
          props: {
            value:
              "<script>alert('xss')</script>\n\n[click](javascript:alert('xss')) and [safe](https://example.com)"
          }
        },
        {
          id: "table",
          type: "table",
          props: {
            columns: ["Unsafe", "Safe"],
            rows: [["<svg onload=alert('xss')>", "ok"]]
          }
        },
        {
          id: "form",
          type: "form",
          props: {
            title: "Hostile",
            submitLabel: "Go",
            fields: [
              {
                id: "username\" autofocus onfocus=\"alert('xss')",
                label: "User",
                type: "text",
                default: ""
              }
            ]
          }
        }
      ]
    });

    const { container } = render(<A2uiRenderer document={document} />);
    const html = container.innerHTML.toLowerCase();
    const hasInlineEventHandler = Array.from(container.querySelectorAll("*")).some((element) =>
      element
        .getAttributeNames()
        .some((attributeName) => attributeName.toLowerCase().startsWith("on"))
    );

    expect(container.querySelector("script")).toBeNull();
    expect(html.includes("javascript:alert")).toBe(false);
    expect(hasInlineEventHandler).toBe(false);
    expect(container.textContent ?? "").toContain("<img src=x onerror=alert('xss')>");

    const links = Array.from(container.querySelectorAll("a"));
    expect(links).toHaveLength(1);
    expect(links[0].getAttribute("href")).toBe("https://example.com/");
  });
});
