import type { CapabilityCatalog } from "../../consoleApi";
import { formatUnixMs, readNumber, readString, type JsonObject } from "../shared";

export function readCapabilityCatalog(value: JsonObject | null): CapabilityCatalog | null {
  return value !== null && Array.isArray(value.capabilities)
    ? (value as unknown as CapabilityCatalog)
    : null;
}

export function readJsonObjectArray(value: unknown): JsonObject[] {
  return Array.isArray(value)
    ? value.filter(
        (entry): entry is JsonObject =>
          entry !== null && typeof entry === "object" && !Array.isArray(entry),
      )
    : [];
}

export function formatAuditTime(event: JsonObject): string {
  return (
    formatUnixMs(
      readNumber(event, "timestamp_unix_ms") ??
        readNumber(event, "observed_at_unix_ms") ??
        readNumber(event, "created_at_unix_ms"),
    ) ??
    readString(event, "occurred_at") ??
    readString(event, "created_at") ??
    "n/a"
  );
}

export function formatAuditEventName(event: JsonObject): string {
  return (
    readString(event, "event_type") ??
    readString(event, "event") ??
    mapAuditKind(readNumber(event, "kind")) ??
    "unknown"
  );
}

export function shortDiagnosticId(value: string | null): string {
  if (value === null || value.length === 0) {
    return "n/a";
  }
  return value.length > 12 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}

export function formatAuditSummary(event: JsonObject): string {
  const summary =
    readString(event, "message") ?? readString(event, "summary") ?? readString(event, "reason");
  if (summary !== null) {
    return summary;
  }

  if (event.payload !== undefined && event.payload !== null) {
    if (
      typeof event.payload === "string" ||
      typeof event.payload === "number" ||
      typeof event.payload === "boolean"
    ) {
      return String(event.payload);
    }
    if (typeof event.payload === "object" && !Array.isArray(event.payload)) {
      const entries = Object.entries(event.payload as Record<string, unknown>);
      if (entries.length > 0) {
        return entries.map(([key, value]) => `${key}: ${String(value)}`).join(", ");
      }
    }
  }

  return readString(event, "payload_json") ?? "No summary";
}

function mapAuditKind(kind: number | null): string | null {
  switch (kind) {
    case 1:
      return "message.received";
    case 2:
      return "model.token";
    case 3:
      return "tool.proposed";
    case 4:
      return "tool.executed";
    case 5:
      return "a2ui.updated";
    case 6:
      return "run.completed";
    case 7:
      return "run.failed";
    default:
      return null;
  }
}
