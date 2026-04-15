import type { SessionCatalogRecord } from "../consoleApi";

export function shortId(value: string): string {
  if (value.length <= 12) {
    return value;
  }
  return `${value.slice(0, 6)}…${value.slice(-4)}`;
}

export function describeBranchState(branchState: string | null | undefined): string {
  const normalized = branchState?.trim().toLowerCase() ?? "";
  if (normalized.length === 0) {
    return "Unknown lineage";
  }
  if (normalized === "root") {
    return "Root session";
  }
  if (normalized === "branched" || normalized === "active_branch") {
    return "Active branch";
  }
  if (normalized === "branch_source") {
    return "Branch source";
  }
  if (normalized === "missing") {
    return "No lineage";
  }
  return branchState ?? "Unknown lineage";
}

export function describeTitleGenerationState(
  titleGenerationState: string | null | undefined,
  manualTitleLocked: boolean,
): string {
  if (manualTitleLocked) {
    return "Manual title";
  }
  const normalized = titleGenerationState?.trim().toLowerCase() ?? "";
  if (normalized.length === 0) {
    return "Auto title unavailable";
  }
  if (normalized === "ready") {
    return "Auto title ready";
  }
  if (normalized === "pending") {
    return "Auto title pending";
  }
  if (normalized === "failed") {
    return "Auto title failed";
  }
  if (normalized === "idle") {
    return "Auto title idle";
  }
  return titleGenerationState ?? "Auto title unavailable";
}

export function buildSessionLineageHint(session: SessionCatalogRecord | null): string {
  if (session === null) {
    return "Select a session to inspect lineage.";
  }
  const normalized = session.branch_state?.trim().toLowerCase() ?? "";
  if (normalized.length === 0) {
    return "Lineage metadata is unavailable for this session.";
  }
  const parent = session.parent_session_id?.trim();
  const originRunId = session.branch_origin_run_id?.trim();
  if (normalized === "root") {
    return originRunId ? `Root session anchored at run ${shortId(originRunId)}.` : "Root session.";
  }
  if (normalized === "branched" || normalized === "active_branch") {
    if (parent !== undefined && parent.length > 0) {
      return originRunId !== undefined && originRunId.length > 0
        ? `Active branch from ${shortId(parent)} at run ${shortId(originRunId)}.`
        : `Active branch from ${shortId(parent)}.`;
    }
    return originRunId !== undefined && originRunId.length > 0
      ? `Active branch anchored at run ${shortId(originRunId)}.`
      : "Active branch.";
  }
  if (normalized === "branch_source") {
    if (parent !== undefined && parent.length > 0) {
      return originRunId !== undefined && originRunId.length > 0
        ? `Branch source with upstream ${shortId(parent)} at run ${shortId(originRunId)}.`
        : `Branch source with upstream ${shortId(parent)}.`;
    }
    return originRunId !== undefined && originRunId.length > 0
      ? `Branch source anchored at run ${shortId(originRunId)}.`
      : "Branch source.";
  }
  const branchLabel = describeBranchState(session.branch_state);
  return parent !== undefined && parent.length > 0
    ? `${branchLabel} from ${shortId(parent)}.`
    : `${branchLabel}.`;
}
