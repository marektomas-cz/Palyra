import type { PatchProcessingBudget, RenderInputLimits } from "./types";

export const DEFAULT_RENDER_INPUT_LIMITS: Readonly<RenderInputLimits> = Object.freeze({
  maxSurfaceLength: 128,
  maxComponents: 128,
  maxComponentIdLength: 64,
  maxStringLength: 512,
  maxMarkdownLength: 12_000,
  maxListItems: 256,
  maxTableColumns: 16,
  maxTableRows: 256,
  maxFormFields: 32,
  maxSelectOptions: 64,
  maxChartPoints: 64,
});

export const DEFAULT_PATCH_BUDGET: Readonly<PatchProcessingBudget> = Object.freeze({
  maxOpsPerPatch: 256,
  maxOpsPerTick: 512,
  maxQueueDepth: 1_024,
  maxPathLength: 512,
  maxApplyMsPerTick: 12,
});
