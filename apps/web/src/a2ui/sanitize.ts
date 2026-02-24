import type { JsonValue } from "./types";

const SAFE_LINK_PROTOCOLS = new Set(["http:", "https:", "mailto:"]);

export function clampText(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value;
  }
  return `${value.slice(0, Math.max(0, maxLength - 1))}\u2026`;
}

export function coerceString(value: unknown, fallback: string, maxLength: number): string {
  if (typeof value !== "string") {
    return fallback;
  }
  return clampText(value.trim(), maxLength);
}

export function coerceBoolean(value: unknown, fallback: boolean): boolean {
  if (typeof value !== "boolean") {
    return fallback;
  }
  return value;
}

export function coerceFiniteNumber(
  value: unknown,
  fallback: number,
  min: number,
  max: number
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback;
  }
  return Math.max(min, Math.min(max, value));
}

export function sanitizeIdentifier(value: unknown, fallback: string, maxLength: number): string {
  const text = coerceString(value, fallback, maxLength);
  const sanitized = text.replace(/[^a-zA-Z0-9:_-]/g, "-").replace(/-+/g, "-");
  if (sanitized.length === 0) {
    return fallback;
  }
  return clampText(sanitized, maxLength);
}

export function stringifyJsonValue(value: unknown, maxLength: number): string {
  if (typeof value === "string") {
    return clampText(value, maxLength);
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (value === null) {
    return "null";
  }
  return clampText(JSON.stringify(value), maxLength);
}

export function cloneJsonValue<TValue extends JsonValue>(value: TValue): TValue {
  if (typeof globalThis.structuredClone === "function") {
    return globalThis.structuredClone(value);
  }
  return JSON.parse(JSON.stringify(value)) as TValue;
}

export function sanitizeExternalUrl(rawValue: string): string | null {
  const trimmed = rawValue.trim();
  if (trimmed.length === 0) {
    return null;
  }
  if (trimmed.startsWith("/")) {
    return trimmed;
  }
  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return null;
  }
  if (!SAFE_LINK_PROTOCOLS.has(parsed.protocol)) {
    return null;
  }
  return parsed.toString();
}
