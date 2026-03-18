import { useEffect, useRef, useState } from "react";

import { DEFAULT_PATCH_BUDGET } from "./constants";
import { A2uiError, asA2uiError } from "./errors";
import { documentToJsonValue, normalizeA2uiDocument } from "./normalize";
import { applyPatchDocument, parsePatchDocument } from "./patch";
import type {
  A2uiDocument,
  PatchDocument,
  PatchProcessingBudget,
  PatchProcessingResult,
} from "./types";

interface RuntimeOptions {
  readonly budget?: Partial<PatchProcessingBudget>;
  readonly onBudgetYield?: (result: PatchProcessingResult) => void;
  readonly onPatchError?: (error: A2uiError) => void;
}

export interface A2uiRuntimeHandle {
  readonly document: A2uiDocument;
  readonly pendingPatchCount: number;
  enqueuePatch: (patch: PatchDocument) => void;
  enqueuePatchValue: (patchValue: unknown) => void;
  replaceDocument: (nextDocument: unknown) => void;
}

type FrameHandle =
  | { kind: "raf"; value: number }
  | { kind: "timeout"; value: ReturnType<typeof setTimeout> };

export function mergePatchBudget(
  budgetOverrides: Partial<PatchProcessingBudget> | undefined,
): PatchProcessingBudget {
  return {
    maxOpsPerPatch: budgetOverrides?.maxOpsPerPatch ?? DEFAULT_PATCH_BUDGET.maxOpsPerPatch,
    maxOpsPerTick: budgetOverrides?.maxOpsPerTick ?? DEFAULT_PATCH_BUDGET.maxOpsPerTick,
    maxQueueDepth: budgetOverrides?.maxQueueDepth ?? DEFAULT_PATCH_BUDGET.maxQueueDepth,
    maxPathLength: budgetOverrides?.maxPathLength ?? DEFAULT_PATCH_BUDGET.maxPathLength,
    maxApplyMsPerTick: budgetOverrides?.maxApplyMsPerTick ?? DEFAULT_PATCH_BUDGET.maxApplyMsPerTick,
  };
}

export function processPatchQueue(
  document: A2uiDocument,
  queuedPatches: readonly PatchDocument[],
  budget: PatchProcessingBudget = DEFAULT_PATCH_BUDGET,
  now: () => number = currentTimeMs,
): PatchProcessingResult {
  if (queuedPatches.length === 0) {
    return {
      nextDocument: document,
      appliedPatches: 0,
      remainingPatches: [],
      exhaustedBudget: false,
      elapsedMs: 0,
    };
  }

  let nextState = documentToJsonValue(document);
  let appliedPatches = 0;
  let consumedOps = 0;
  const startedAt = now();

  for (let index = 0; index < queuedPatches.length; index += 1) {
    const patch = queuedPatches[index];
    const patchOps = patch.ops.length;
    if (appliedPatches > 0 && consumedOps + patchOps > budget.maxOpsPerTick) {
      break;
    }
    if (appliedPatches > 0 && now() - startedAt >= budget.maxApplyMsPerTick) {
      break;
    }
    nextState = applyPatchDocument(nextState, patch, budget, now);
    appliedPatches += 1;
    consumedOps += patchOps;
  }

  const elapsedMs = Math.max(0, now() - startedAt);
  const remainingPatches = queuedPatches.slice(appliedPatches);
  const nextDocument = appliedPatches > 0 ? normalizeA2uiDocument(nextState) : document;

  return {
    nextDocument,
    appliedPatches,
    remainingPatches,
    exhaustedBudget: remainingPatches.length > 0,
    elapsedMs,
  };
}

export function useA2uiRuntime(
  initialDocument: unknown,
  options: RuntimeOptions = {},
): A2uiRuntimeHandle {
  const [document, setDocument] = useState(() => normalizeA2uiDocument(initialDocument));
  const [pendingPatchCount, setPendingPatchCount] = useState(0);

  const documentRef = useRef(document);
  const queueRef = useRef<PatchDocument[]>([]);
  const frameHandleRef = useRef<FrameHandle | null>(null);
  const budgetRef = useRef(mergePatchBudget(options.budget));
  const onBudgetYieldRef = useRef(options.onBudgetYield);
  const onPatchErrorRef = useRef(options.onPatchError);

  useEffect(() => {
    documentRef.current = document;
  }, [document]);

  useEffect(() => {
    budgetRef.current = mergePatchBudget(options.budget);
  }, [
    options.budget?.maxApplyMsPerTick,
    options.budget?.maxOpsPerPatch,
    options.budget?.maxOpsPerTick,
    options.budget?.maxPathLength,
    options.budget?.maxQueueDepth,
  ]);

  useEffect(() => {
    onBudgetYieldRef.current = options.onBudgetYield;
  }, [options.onBudgetYield]);

  useEffect(() => {
    onPatchErrorRef.current = options.onPatchError;
  }, [options.onPatchError]);

  useEffect(() => {
    return () => {
      if (frameHandleRef.current !== null) {
        cancelFrame(frameHandleRef.current);
      }
    };
  }, []);

  function scheduleFlush(): void {
    if (frameHandleRef.current !== null) {
      return;
    }
    frameHandleRef.current = scheduleFrame(() => {
      frameHandleRef.current = null;
      flushQueue();
    });
  }

  function flushQueue(): void {
    if (queueRef.current.length === 0) {
      setPendingPatchCount(0);
      return;
    }
    try {
      const result = processPatchQueue(documentRef.current, queueRef.current, budgetRef.current);
      queueRef.current = [...result.remainingPatches];
      setPendingPatchCount(queueRef.current.length);

      if (result.appliedPatches > 0) {
        documentRef.current = result.nextDocument;
        setDocument(result.nextDocument);
      }

      if (result.exhaustedBudget) {
        onBudgetYieldRef.current?.(result);
      }

      if (queueRef.current.length > 0) {
        scheduleFlush();
      }
    } catch (error) {
      queueRef.current = [];
      setPendingPatchCount(0);
      onPatchErrorRef.current?.(asA2uiError(error, "invalid_patch"));
    }
  }

  function enqueuePatch(patch: PatchDocument): void {
    const budget = budgetRef.current;
    if (queueRef.current.length >= budget.maxQueueDepth) {
      throw new A2uiError(
        "budget_exceeded",
        `Patch queue depth exceeds maxQueueDepth=${budget.maxQueueDepth}.`,
      );
    }
    queueRef.current.push(patch);
    setPendingPatchCount(queueRef.current.length);
    scheduleFlush();
  }

  function enqueuePatchValue(patchValue: unknown): void {
    enqueuePatch(parsePatchDocument(patchValue, budgetRef.current));
  }

  function replaceDocument(nextDocument: unknown): void {
    if (frameHandleRef.current !== null) {
      cancelFrame(frameHandleRef.current);
      frameHandleRef.current = null;
    }
    queueRef.current = [];
    setPendingPatchCount(0);
    const normalized = normalizeA2uiDocument(nextDocument);
    documentRef.current = normalized;
    setDocument(normalized);
  }

  return {
    document,
    pendingPatchCount,
    enqueuePatch,
    enqueuePatchValue,
    replaceDocument,
  };
}

function scheduleFrame(callback: () => void): FrameHandle {
  if (typeof window !== "undefined" && typeof window.requestAnimationFrame === "function") {
    return {
      kind: "raf",
      value: window.requestAnimationFrame(() => callback()),
    };
  }
  return {
    kind: "timeout",
    value: setTimeout(callback, 16),
  };
}

function cancelFrame(handle: FrameHandle): void {
  if (
    handle.kind === "raf" &&
    typeof window !== "undefined" &&
    typeof window.cancelAnimationFrame === "function"
  ) {
    window.cancelAnimationFrame(handle.value);
    return;
  }
  clearTimeout(handle.value);
}

function currentTimeMs(): number {
  if (typeof performance !== "undefined" && typeof performance.now === "function") {
    return performance.now();
  }
  return Date.now();
}
