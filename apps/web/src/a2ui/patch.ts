import { DEFAULT_PATCH_BUDGET } from "./constants";
import { A2uiError } from "./errors";
import { cloneJsonValue } from "./sanitize";
import type {
  JsonObject,
  JsonValue,
  PatchDocument,
  PatchOperation,
  PatchOperationKind,
  PatchProcessingBudget
} from "./types";
import { isJsonObject, isJsonValue } from "./types";

type MutableContainer = JsonObject | JsonValue[];

export function parsePatchDocument(
  input: unknown,
  budget: PatchProcessingBudget = DEFAULT_PATCH_BUDGET
): PatchDocument {
  if (!isJsonObject(input)) {
    throw new A2uiError("invalid_patch", "Patch payload must be a JSON object.");
  }
  if (input.v !== 1) {
    throw new A2uiError("invalid_patch", "Patch payload must use version v=1.");
  }
  if (!Array.isArray(input.ops) || input.ops.length === 0) {
    throw new A2uiError("invalid_patch", "Patch payload must include at least one operation.");
  }
  if (input.ops.length > budget.maxOpsPerPatch) {
    throw new A2uiError(
      "budget_exceeded",
      `Patch operation count ${input.ops.length} exceeds maxOpsPerPatch=${budget.maxOpsPerPatch}.`
    );
  }

  const operations: PatchOperation[] = input.ops.map((entry, index) =>
    parsePatchOperation(entry, index, budget)
  );
  return {
    v: 1,
    ops: operations
  };
}

export function applyPatchDocument(
  state: JsonValue,
  patch: PatchDocument,
  budget: PatchProcessingBudget = DEFAULT_PATCH_BUDGET,
  now: () => number = currentTimeMs
): JsonValue {
  if (patch.v !== 1) {
    throw new A2uiError("invalid_patch", "Patch payload must use version v=1.");
  }
  if (patch.ops.length === 0) {
    throw new A2uiError("invalid_patch", "Patch payload must include at least one operation.");
  }
  if (patch.ops.length > budget.maxOpsPerPatch) {
    throw new A2uiError(
      "budget_exceeded",
      `Patch operation count ${patch.ops.length} exceeds maxOpsPerPatch=${budget.maxOpsPerPatch}.`
    );
  }

  let nextState = cloneJsonValue(state);
  const startedAt = now();

  for (let index = 0; index < patch.ops.length; index += 1) {
    const operation = patch.ops[index];
    if (operation.path.length > budget.maxPathLength) {
      throw new A2uiError(
        "budget_exceeded",
        `Patch path exceeds maxPathLength=${budget.maxPathLength}.`,
        operation.path
      );
    }
    const elapsed = now() - startedAt;
    if (elapsed > budget.maxApplyMsPerTick) {
      throw new A2uiError(
        "budget_exceeded",
        `Patch application exceeded maxApplyMsPerTick=${budget.maxApplyMsPerTick}.`
      );
    }
    nextState = applySingleOperation(nextState, operation, index, budget.maxPathLength);
  }

  return nextState;
}

function parsePatchOperation(
  value: unknown,
  index: number,
  budget: PatchProcessingBudget
): PatchOperation {
  if (!isJsonObject(value)) {
    throw new A2uiError("invalid_patch", `Patch operation at index ${index} must be an object.`);
  }
  const op = parseOperationKind(value.op, index);
  const path = parseOperationPath(value.path, index, budget.maxPathLength);
  const hasValue = Object.prototype.hasOwnProperty.call(value, "value");

  if (op === "remove") {
    if (hasValue) {
      throw new A2uiError(
        "invalid_patch",
        `Patch remove operation at index ${index} must not include a value.`
      );
    }
    return { op, path };
  }

  if (!hasValue || !isJsonValue(value.value)) {
    throw new A2uiError(
      "invalid_patch",
      `Patch ${op} operation at index ${index} must include a JSON value.`
    );
  }

  return {
    op,
    path,
    value: cloneJsonValue(value.value)
  };
}

function parseOperationKind(value: unknown, index: number): PatchOperationKind {
  if (typeof value !== "string") {
    throw new A2uiError("invalid_patch", `Patch operation at index ${index} has invalid op type.`);
  }
  if (value === "add" || value === "replace" || value === "remove") {
    return value;
  }
  throw new A2uiError("invalid_patch", `Patch operation at index ${index} uses unsupported op '${value}'.`);
}

function parseOperationPath(value: unknown, index: number, maxPathLength: number): string {
  if (typeof value !== "string") {
    throw new A2uiError(
      "invalid_patch",
      `Patch operation at index ${index} has invalid path type.`
    );
  }
  if (value.length > maxPathLength) {
    throw new A2uiError(
      "budget_exceeded",
      `Patch path length ${value.length} exceeds maxPathLength=${maxPathLength}.`
    );
  }
  if (value === "") {
    return value;
  }
  if (!value.startsWith("/")) {
    throw new A2uiError(
      "invalid_patch",
      `Patch path '${value}' at index ${index} must start with '/'.`
    );
  }
  return value;
}

function applySingleOperation(
  root: JsonValue,
  operation: PatchOperation,
  opIndex: number,
  maxPathLength: number
): JsonValue {
  const tokens = parsePointerTokens(operation.path, opIndex, maxPathLength);
  if (tokens.length === 0) {
    if (operation.op === "remove") {
      throw new A2uiError("invalid_patch", "Removing the patch root is not supported.");
    }
    return cloneJsonValue(operation.value);
  }

  const { container, token } = resolveContainer(root, tokens, opIndex, operation.path);
  if (operation.op === "add") {
    applyAddOperation(container, token, operation.value, opIndex);
    return root;
  }
  if (operation.op === "replace") {
    applyReplaceOperation(container, token, operation.value, opIndex);
    return root;
  }
  applyRemoveOperation(container, token, opIndex);
  return root;
}

function applyAddOperation(
  container: MutableContainer,
  token: string,
  value: JsonValue,
  opIndex: number
): void {
  if (Array.isArray(container)) {
    if (token === "-") {
      container.push(cloneJsonValue(value));
      return;
    }
    const index = parseArrayIndex(token, opIndex, container.length, true);
    container.splice(index, 0, cloneJsonValue(value));
    return;
  }
  container[token] = cloneJsonValue(value);
}

function applyReplaceOperation(
  container: MutableContainer,
  token: string,
  value: JsonValue,
  opIndex: number
): void {
  if (Array.isArray(container)) {
    const index = parseArrayIndex(token, opIndex, container.length, false);
    container[index] = cloneJsonValue(value);
    return;
  }
  if (!Object.prototype.hasOwnProperty.call(container, token)) {
    throw new A2uiError(
      "conflict",
      `Patch operation at index ${opIndex} cannot replace missing object path '${token}'.`
    );
  }
  container[token] = cloneJsonValue(value);
}

function applyRemoveOperation(container: MutableContainer, token: string, opIndex: number): void {
  if (Array.isArray(container)) {
    const index = parseArrayIndex(token, opIndex, container.length, false);
    container.splice(index, 1);
    return;
  }
  if (!Object.prototype.hasOwnProperty.call(container, token)) {
    throw new A2uiError(
      "conflict",
      `Patch operation at index ${opIndex} cannot remove missing object path '${token}'.`
    );
  }
  delete container[token];
}

function resolveContainer(
  root: JsonValue,
  tokens: readonly string[],
  opIndex: number,
  originalPath: string
): { container: MutableContainer; token: string } {
  let current: JsonValue = root;

  for (let index = 0; index < tokens.length - 1; index += 1) {
    const token = tokens[index];
    if (Array.isArray(current)) {
      const arrayIndex = parseArrayIndex(token, opIndex, current.length, false);
      current = current[arrayIndex];
      continue;
    }
    if (isJsonObject(current)) {
      if (!Object.prototype.hasOwnProperty.call(current, token)) {
        throw new A2uiError(
          "conflict",
          `Patch operation at index ${opIndex} cannot resolve path '${originalPath}'.`
        );
      }
      current = current[token];
      continue;
    }
    throw new A2uiError(
      "conflict",
      `Patch operation at index ${opIndex} cannot traverse into primitive at '${originalPath}'.`
    );
  }

  if (Array.isArray(current) || isJsonObject(current)) {
    return {
      container: current,
      token: tokens[tokens.length - 1]
    };
  }

  throw new A2uiError(
    "conflict",
    `Patch operation at index ${opIndex} cannot modify primitive at '${originalPath}'.`
  );
}

function parsePointerTokens(path: string, opIndex: number, maxPathLength: number): string[] {
  if (path.length > maxPathLength) {
    throw new A2uiError(
      "budget_exceeded",
      `Patch path length ${path.length} exceeds maxPathLength=${maxPathLength}.`
    );
  }
  if (path === "") {
    return [];
  }
  if (!path.startsWith("/")) {
    throw new A2uiError("invalid_patch", `Patch path '${path}' at index ${opIndex} is invalid.`);
  }

  const segments = path.slice(1).split("/");
  return segments.map((segment) => decodePointerSegment(segment, opIndex, path));
}

function decodePointerSegment(segment: string, opIndex: number, originalPath: string): string {
  let decoded = "";
  for (let cursor = 0; cursor < segment.length; cursor += 1) {
    const character = segment[cursor];
    if (character !== "~") {
      decoded += character;
      continue;
    }
    if (cursor + 1 >= segment.length) {
      throw new A2uiError(
        "invalid_patch",
        `Patch path '${originalPath}' at index ${opIndex} has invalid escape sequence.`
      );
    }
    const escaped = segment[cursor + 1];
    if (escaped === "0") {
      decoded += "~";
      cursor += 1;
      continue;
    }
    if (escaped === "1") {
      decoded += "/";
      cursor += 1;
      continue;
    }
    throw new A2uiError(
      "invalid_patch",
      `Patch path '${originalPath}' at index ${opIndex} has invalid escape sequence.`
    );
  }
  return decoded;
}

function parseArrayIndex(
  raw: string,
  opIndex: number,
  arrayLength: number,
  allowEndExclusive: boolean
): number {
  if (!/^(0|[1-9][0-9]*)$/.test(raw)) {
    throw new A2uiError("conflict", `Patch operation at index ${opIndex} has invalid array index '${raw}'.`);
  }
  const value = Number.parseInt(raw, 10);
  const maxAllowed = allowEndExclusive ? arrayLength : arrayLength - 1;
  if (value > maxAllowed) {
    throw new A2uiError(
      "conflict",
      `Patch operation at index ${opIndex} index ${value} is out of bounds (len=${arrayLength}).`
    );
  }
  return value;
}

function currentTimeMs(): number {
  if (typeof performance !== "undefined" && typeof performance.now === "function") {
    return performance.now();
  }
  return Date.now();
}
