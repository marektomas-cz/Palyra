import { normalizeA2uiDocument } from "./normalize";
import type { A2uiDocument } from "./types";

export function createDemoDocument(): A2uiDocument {
  return normalizeA2uiDocument({
    v: 1,
    surface: "web-console-preview",
    components: [
      {
        id: "status-line",
        type: "text",
        props: {
          tone: "success",
          value: "Renderer online. Waiting for incremental patches.",
        },
      },
      {
        id: "summary",
        type: "markdown",
        props: {
          value:
            "### A2UI Renderer\\nThis renderer keeps **strict escaping** and supports [safe links](https://palyra.dev).",
        },
      },
      {
        id: "tasks",
        type: "list",
        props: {
          ordered: false,
          items: [
            "No `dangerouslySetInnerHTML`",
            "Deterministic patch application",
            "Budget-aware update queue",
          ],
        },
      },
      {
        id: "metrics",
        type: "table",
        props: {
          columns: ["Metric", "Value"],
          rows: [
            ["Rendered components", "6"],
            ["Queued patches", "0"],
          ],
        },
      },
      {
        id: "operator-form",
        type: "form",
        props: {
          title: "Operator Action",
          submitLabel: "Apply",
          fields: [
            {
              id: "operator-note",
              label: "Note",
              type: "text",
              default: "",
              placeholder: "Type an audit note",
              required: true,
            },
            {
              id: "target-priority",
              label: "Priority",
              type: "select",
              default: "normal",
              options: [
                { label: "Normal", value: "normal" },
                { label: "High", value: "high" },
              ],
            },
            {
              id: "dry-run",
              label: "Dry run",
              type: "checkbox",
              default: true,
            },
          ],
        },
      },
      {
        id: "latency-chart",
        type: "chart",
        props: {
          title: "Patch Loop Latency (ms)",
          series: [
            { label: "P50", value: 9 },
            { label: "P95", value: 16 },
            { label: "P99", value: 23 },
          ],
        },
      },
    ],
  });
}
