import { useState } from "react";

import type { ConsoleApiClient } from "../../consoleApi";
import {
  cancelFlow,
  compensateFlowStep,
  getFlow,
  listFlows,
  pauseFlow,
  resumeFlow,
  retryFlowStep,
  skipFlowStep,
  type ConsoleFlowBundleEnvelope,
} from "../../flowApi";
import { ActionButton } from "../components/ui";
import { WorkspaceSectionCard, WorkspaceStatusChip } from "../components/workspace/WorkspaceChrome";
import {
  WorkspaceEmptyState,
  WorkspaceTable,
  workspaceToneForState,
} from "../components/workspace/WorkspacePatterns";
import {
  formatUnixMs,
  readNumber,
  readObject,
  readString,
  toErrorMessage,
  type JsonObject,
} from "../shared";

type FlowOperationsPanelProps = {
  api: ConsoleApiClient;
  diagnostics: JsonObject | null;
  setError: (message: string | null) => void;
  setNotice: (message: string | null) => void;
};

export function FlowOperationsPanel({
  api,
  diagnostics,
  setError,
  setNotice,
}: FlowOperationsPanelProps) {
  const [flowsBusy, setFlowsBusy] = useState(false);
  const [flowActionBusy, setFlowActionBusy] = useState(false);
  const [flowRecords, setFlowRecords] = useState<JsonObject[]>([]);
  const [selectedFlowId, setSelectedFlowId] = useState("");
  const [selectedFlowBundle, setSelectedFlowBundle] = useState<ConsoleFlowBundleEnvelope | null>(
    null,
  );
  const flowsSnapshot = readObject(diagnostics ?? {}, "flows");
  const flowsSnapshotSummary = readObject(flowsSnapshot ?? {}, "by_state");
  const flowStepSnapshot = readObject(flowsSnapshot ?? {}, "steps");
  const flowSnapshotRecent = readJsonObjectArray(flowsSnapshot?.recent);
  const visibleFlows = flowRecords.length > 0 ? flowRecords : flowSnapshotRecent;
  const selectedFlowFromBundle =
    selectedFlowBundle === null ? null : (selectedFlowBundle.flow as unknown as JsonObject);
  const selectedFlow =
    selectedFlowFromBundle ??
    visibleFlows.find((flow) => readString(flow, "flow_id") === selectedFlowId.trim()) ??
    visibleFlows[0] ??
    null;
  const selectedFlowResolvedId = readString(selectedFlow ?? {}, "flow_id") ?? selectedFlowId.trim();
  const selectedFlowSteps = readJsonObjectArray(selectedFlowBundle?.steps);
  const selectedFlowEvents = readJsonObjectArray(selectedFlowBundle?.events);
  const selectedFlowRevisions = readJsonObjectArray(selectedFlowBundle?.revisions);
  const selectedFlowBlockers = readJsonObjectArray(selectedFlowBundle?.blockers);
  const activeFlowCount =
    readNumber(flowsSnapshot ?? {}, "active_flows") ??
    readNumber(flowsSnapshot ?? {}, "active_count") ??
    readNumber(flowsSnapshotSummary ?? {}, "running") ??
    visibleFlows.filter((flow) => !isTerminalFlowState(readString(flow, "state"))).length;
  const blockedFlowCount =
    readNumber(flowsSnapshot ?? {}, "blocked_flows") ??
    readNumber(flowsSnapshot ?? {}, "blocked_count") ??
    readNumber(flowsSnapshotSummary ?? {}, "blocked") ??
    0;
  const waitingFlowCount =
    readNumber(flowsSnapshot ?? {}, "waiting_for_approval_flows") ??
    readNumber(flowsSnapshot ?? {}, "waiting_for_approval_count") ??
    readNumber(flowsSnapshotSummary ?? {}, "waiting_for_approval") ??
    0;

  async function refreshFlows(): Promise<void> {
    setFlowsBusy(true);
    setError(null);
    try {
      const response = await listFlows(
        api,
        new URLSearchParams({ limit: "32", include_terminal: "true" }),
      );
      const records = response.flows as unknown as JsonObject[];
      setFlowRecords(records);
      setSelectedFlowId((current) => {
        if (current.trim().length > 0) {
          return current;
        }
        return readString(records[0] ?? {}, "flow_id") ?? "";
      });
      setNotice(`Loaded ${records.length} flow(s).`);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setFlowsBusy(false);
    }
  }

  async function loadFlow(flowId: string = selectedFlowResolvedId): Promise<void> {
    const targetFlowId = flowId.trim();
    if (targetFlowId.length === 0) {
      setError("Select a flow first.");
      return;
    }
    setFlowsBusy(true);
    setError(null);
    try {
      const response = await getFlow(api, targetFlowId);
      setSelectedFlowBundle(response);
      setSelectedFlowId(response.flow.flow_id);
      mergeFlowRecord(response.flow as unknown as JsonObject);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setFlowsBusy(false);
    }
  }

  async function runFlowAction(action: "pause" | "resume" | "cancel"): Promise<void> {
    const targetFlowId = selectedFlowResolvedId.trim();
    if (targetFlowId.length === 0) {
      setError("Select a flow first.");
      return;
    }
    setFlowActionBusy(true);
    setError(null);
    try {
      const payload = { reason: `operator requested ${action}` };
      const response =
        action === "pause"
          ? await pauseFlow(api, targetFlowId, payload)
          : action === "resume"
            ? await resumeFlow(api, targetFlowId, payload)
            : await cancelFlow(api, targetFlowId, payload);
      setSelectedFlowBundle(response);
      setSelectedFlowId(response.flow.flow_id);
      mergeFlowRecord(response.flow as unknown as JsonObject);
      setNotice(`Flow ${action} requested.`);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setFlowActionBusy(false);
    }
  }

  async function runStepAction(
    stepId: string,
    action: "retry" | "skip" | "compensate",
  ): Promise<void> {
    const targetFlowId = selectedFlowResolvedId.trim();
    const targetStepId = stepId.trim();
    if (targetFlowId.length === 0 || targetStepId.length === 0) {
      setError("Select a flow step first.");
      return;
    }
    setFlowActionBusy(true);
    setError(null);
    try {
      const payload = { reason: `operator requested ${action}` };
      const response =
        action === "retry"
          ? await retryFlowStep(api, targetFlowId, targetStepId, payload)
          : action === "skip"
            ? await skipFlowStep(api, targetFlowId, targetStepId, payload)
            : await compensateFlowStep(api, targetFlowId, targetStepId, payload);
      setSelectedFlowBundle(response);
      setSelectedFlowId(response.flow.flow_id);
      mergeFlowRecord(response.flow as unknown as JsonObject);
      setNotice(`Flow step ${action} requested.`);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setFlowActionBusy(false);
    }
  }

  function mergeFlowRecord(flow: JsonObject): void {
    const flowId = readString(flow, "flow_id");
    if (flowId === null) {
      return;
    }
    setFlowRecords((current) => {
      const next = current.filter((record) => readString(record, "flow_id") !== flowId);
      return [flow, ...next].slice(0, 32);
    });
  }

  return (
    <WorkspaceSectionCard
      title="Durable flows"
      description="Flow orchestration exposes the current run lineage, blocked steps, and operator controls in one timeline."
      actions={
        <div className="workspace-inline">
          <ActionButton
            type="button"
            size="sm"
            variant="secondary"
            onPress={() => void refreshFlows()}
            isDisabled={flowsBusy || flowActionBusy}
          >
            {flowsBusy ? "Refreshing..." : "Refresh flows"}
          </ActionButton>
          <ActionButton
            type="button"
            size="sm"
            variant="ghost"
            onPress={() => void loadFlow()}
            isDisabled={flowsBusy || flowActionBusy || selectedFlowResolvedId.length === 0}
          >
            Load selected
          </ActionButton>
        </div>
      }
    >
      <div className="workspace-stack">
        {flowsSnapshot !== null ? (
          <WorkspaceTable ariaLabel="Flow diagnostics" columns={["Metric", "Value", "Detail"]}>
            <tr>
              <td>Active</td>
              <td>{activeFlowCount}</td>
              <td>
                {blockedFlowCount} blocked · {waitingFlowCount} waiting for approval
              </td>
            </tr>
            <tr>
              <td>Steps</td>
              <td>
                {readNumber(flowStepSnapshot ?? {}, "blocked") ?? selectedFlowBlockers.length}
              </td>
              <td>
                {readNumber(flowStepSnapshot ?? {}, "retrying") ?? 0} retrying ·{" "}
                {readNumber(flowStepSnapshot ?? {}, "timed_out") ?? 0} timed out
              </td>
            </tr>
          </WorkspaceTable>
        ) : null}

        {visibleFlows.length === 0 ? (
          <WorkspaceEmptyState
            compact
            title="No flows loaded"
            description="Refresh diagnostics or flow list to load durable orchestration state."
          />
        ) : (
          <WorkspaceTable
            ariaLabel="Durable flows"
            columns={["Flow", "State", "Source", "Updated", "Open"]}
          >
            {visibleFlows.slice(0, 10).map((flow, index) => {
              const flowId = readString(flow, "flow_id") ?? "";
              return (
                <tr key={`${flowId || "flow"}-${index}`}>
                  <td>
                    {readString(flow, "title") ?? shortDiagnosticId(flowId)}
                    <br />
                    <span className="chat-muted">{shortDiagnosticId(flowId)}</span>
                  </td>
                  <td>
                    <WorkspaceStatusChip
                      tone={workspaceToneForState(readString(flow, "state") ?? "unknown")}
                    >
                      {readString(flow, "state") ?? "unknown"}
                    </WorkspaceStatusChip>
                  </td>
                  <td>{flowSourceLabel(flow)}</td>
                  <td>{formatUnixMs(readNumber(flow, "updated_at_unix_ms")) ?? "n/a"}</td>
                  <td>
                    <ActionButton
                      type="button"
                      size="sm"
                      variant={flowId === selectedFlowResolvedId ? "primary" : "ghost"}
                      onPress={() => {
                        setSelectedFlowId(flowId);
                        void loadFlow(flowId);
                      }}
                      isDisabled={flowId.length === 0 || flowsBusy || flowActionBusy}
                    >
                      Open
                    </ActionButton>
                  </td>
                </tr>
              );
            })}
          </WorkspaceTable>
        )}

        {selectedFlow === null ? null : (
          <>
            <WorkspaceTable ariaLabel="Selected flow" columns={["Field", "Value", "Detail"]}>
              <tr>
                <td>Selected</td>
                <td>{shortDiagnosticId(selectedFlowResolvedId)}</td>
                <td>
                  {readString(selectedFlow, "summary") ??
                    readString(selectedFlow, "title") ??
                    "n/a"}
                </td>
              </tr>
              <tr>
                <td>Revision</td>
                <td>{readNumber(selectedFlow, "revision") ?? 0}</td>
                <td>
                  current step {shortDiagnosticId(readString(selectedFlow, "current_step_id"))}
                </td>
              </tr>
              <tr>
                <td>Blockers</td>
                <td>{selectedFlowBlockers.length}</td>
                <td>
                  {selectedFlowEvents.length} events · {selectedFlowRevisions.length} revisions
                </td>
              </tr>
            </WorkspaceTable>

            <div className="workspace-inline">
              <ActionButton
                type="button"
                size="sm"
                variant="secondary"
                onPress={() => void runFlowAction("pause")}
                isDisabled={flowActionBusy || selectedFlowResolvedId.length === 0}
              >
                Pause
              </ActionButton>
              <ActionButton
                type="button"
                size="sm"
                variant="secondary"
                onPress={() => void runFlowAction("resume")}
                isDisabled={flowActionBusy || selectedFlowResolvedId.length === 0}
              >
                Resume
              </ActionButton>
              <ActionButton
                type="button"
                size="sm"
                variant="danger"
                onPress={() => void runFlowAction("cancel")}
                isDisabled={flowActionBusy || selectedFlowResolvedId.length === 0}
              >
                Cancel
              </ActionButton>
            </div>

            {selectedFlowSteps.length === 0 ? (
              <WorkspaceEmptyState
                compact
                title="No selected flow steps"
                description="Open a flow to load step-level retry, skip, and compensation controls."
              />
            ) : (
              <WorkspaceTable
                ariaLabel="Selected flow steps"
                columns={["Step", "State", "Attempts", "Action"]}
              >
                {selectedFlowSteps.map((step, index) => {
                  const stepId = readString(step, "step_id") ?? "";
                  return (
                    <tr key={`${stepId || "step"}-${index}`}>
                      <td>
                        {readString(step, "title") ?? shortDiagnosticId(stepId)}
                        <br />
                        <span className="chat-muted">
                          {readString(step, "adapter") ?? "adapter"} ·{" "}
                          {readString(step, "step_kind") ?? "step"}
                        </span>
                      </td>
                      <td>
                        <WorkspaceStatusChip
                          tone={workspaceToneForState(readString(step, "state") ?? "unknown")}
                        >
                          {readString(step, "state") ?? "unknown"}
                        </WorkspaceStatusChip>
                      </td>
                      <td>
                        {readNumber(step, "attempt_count") ?? 0}/
                        {readNumber(step, "max_attempts") ?? 0}
                        <br />
                        <span className="chat-muted">
                          {readString(step, "waiting_reason") ??
                            readString(step, "last_error") ??
                            "no blocker"}
                        </span>
                      </td>
                      <td>
                        <div className="workspace-inline">
                          <ActionButton
                            type="button"
                            size="sm"
                            variant="ghost"
                            onPress={() => void runStepAction(stepId, "retry")}
                            isDisabled={flowActionBusy || stepId.length === 0}
                          >
                            Retry
                          </ActionButton>
                          <ActionButton
                            type="button"
                            size="sm"
                            variant="ghost"
                            onPress={() => void runStepAction(stepId, "skip")}
                            isDisabled={flowActionBusy || stepId.length === 0}
                          >
                            Skip
                          </ActionButton>
                          <ActionButton
                            type="button"
                            size="sm"
                            variant="ghost"
                            onPress={() => void runStepAction(stepId, "compensate")}
                            isDisabled={flowActionBusy || stepId.length === 0}
                          >
                            Compensate
                          </ActionButton>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </WorkspaceTable>
            )}

            {selectedFlowEvents.length > 0 ? (
              <WorkspaceTable
                ariaLabel="Selected flow timeline"
                columns={["When", "Event", "Step", "Summary"]}
              >
                {selectedFlowEvents.slice(0, 8).map((event, index) => (
                  <tr key={`${readString(event, "event_id") ?? "event"}-${index}`}>
                    <td>{formatUnixMs(readNumber(event, "created_at_unix_ms")) ?? "n/a"}</td>
                    <td>{readString(event, "event_type") ?? "unknown"}</td>
                    <td>{shortDiagnosticId(readString(event, "step_id"))}</td>
                    <td>{readString(event, "summary") ?? "No event summary."}</td>
                  </tr>
                ))}
              </WorkspaceTable>
            ) : null}
          </>
        )}
      </div>
    </WorkspaceSectionCard>
  );
}

function isTerminalFlowState(state: string | null): boolean {
  return (
    state === "completed" || state === "failed" || state === "cancelled" || state === "compensated"
  );
}

function flowSourceLabel(flow: JsonObject): string {
  const objectiveId = readString(flow, "objective_id");
  if (objectiveId !== null) {
    return `objective ${shortDiagnosticId(objectiveId)}`;
  }
  const routineId = readString(flow, "routine_id");
  if (routineId !== null) {
    return `routine ${shortDiagnosticId(routineId)}`;
  }
  const webhookId = readString(flow, "webhook_id");
  if (webhookId !== null) {
    return `webhook ${shortDiagnosticId(webhookId)}`;
  }
  const runId = readString(flow, "origin_run_id");
  if (runId !== null) {
    return `run ${shortDiagnosticId(runId)}`;
  }
  return readString(flow, "mode") ?? "managed";
}

function shortDiagnosticId(value: string | null): string {
  if (value === null || value.length === 0) {
    return "n/a";
  }
  return value.length > 12 ? `${value.slice(0, 8)}...${value.slice(-4)}` : value;
}

function readJsonObjectArray(value: unknown): JsonObject[] {
  return Array.isArray(value)
    ? value.filter(
        (entry): entry is JsonObject =>
          entry !== null && typeof entry === "object" && !Array.isArray(entry),
      )
    : [];
}
