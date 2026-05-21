import { describe, expect, it } from "vitest";

import { editablePluginConfig } from "./SkillsSection";
import type { JsonObject } from "../shared";

function pluginDetail(configured: JsonObject, redactedFields: string[] = []): JsonObject {
  return {
    check: {
      config: {
        validation: {
          redacted_fields: redactedFields,
        },
        configured,
      },
    },
  };
}

describe("editablePluginConfig", () => {
  it("does not populate secret-like configured values into the editor draft", () => {
    const draft = editablePluginConfig(
      pluginDetail({
        api_base_url: "https://api.example.com",
        api_token: "super-secret-token-123",
      }),
    );

    expect(draft).toBe("");
  });

  it("does not populate manifest-redacted configured values into the editor draft", () => {
    const draft = editablePluginConfig(
      pluginDetail(
        {
          api_base_url: "https://api.example.com",
          api_token: "[redacted]",
        },
        ["api_token"],
      ),
    );

    expect(draft).toBe("");
  });

  it("keeps clean configured values editable", () => {
    const draft = editablePluginConfig(
      pluginDetail({
        api_base_url: "https://api.example.com",
        retries: 2,
      }),
    );

    expect(draft).toContain('"api_base_url": "https://api.example.com"');
    expect(draft).toContain('"retries": 2');
  });
});
