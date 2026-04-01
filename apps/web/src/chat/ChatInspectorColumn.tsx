import { Chip } from "@heroui/react";

import type {
  ChatPinRecord,
  ChatQueuedInputRecord,
  ChatRunStatusRecord,
  ChatRunTapeSnapshot,
  ChatTranscriptRecord,
  SessionCatalogRecord,
  JsonValue,
} from "../consoleApi";
import {
  ActionButton,
  EmptyState,
  KeyValueList,
  SectionCard,
} from "../console/components/ui";

import { ChatRunDrawer } from "./ChatRunDrawer";
import {
  PrettyJsonBlock,
  describeBranchState,
  formatApproxTokens,
  prettifyEventType,
  shortId,
} from "./chatShared";

export type TranscriptSearchMatch = {
  session_id: string;
  run_id: string;
  seq: number;
  event_type: string;
  created_at_unix_ms: number;
  origin_kind: string;
  origin_run_id?: string;
  snippet: string;
};

export type DetailPanelState = {
  id: string;
  title: string;
  subtitle: string;
  body?: string;
  payload?: JsonValue;
};

type ChatInspectorColumnProps = {
  pendingApprovalCount: number;
  a2uiSurfaces: string[];
  runIds: string[];
  selectedSession: SessionCatalogRecord | null;
  selectedSessionLineage: string;
  contextBudgetEstimatedTokens: number;
  transcriptBusy: boolean;
  transcriptSearchQuery: string;
  setTranscriptSearchQuery: (value: string) => void;
  transcriptSearchBusy: boolean;
  canSearchTranscript: boolean;
  pinnedRecordKeys: ReadonlySet<string>;
  searchResults: TranscriptSearchMatch[];
  searchTranscript: () => void;
  inspectSearchMatch: (match: TranscriptSearchMatch) => void;
  exportBusy: "json" | "markdown" | null;
  exportTranscript: (format: "json" | "markdown") => void;
  recentTranscriptRecords: ChatTranscriptRecord[];
  inspectTranscriptRecord: (record: ChatTranscriptRecord) => void;
  pinTranscriptRecord: (record: ChatTranscriptRecord) => void;
  sessionPins: ChatPinRecord[];
  deletePin: (pinId: string) => void;
  queuedInputs: ChatQueuedInputRecord[];
  detailPanel: DetailPanelState | null;
  revealSensitiveValues: boolean;
  inspectorVisible: boolean;
  runDrawerId: string;
  setRunDrawerId: (runId: string) => void;
  runDrawerBusy: boolean;
  runStatus: ChatRunStatusRecord | null;
  runTape: ChatRunTapeSnapshot | null;
  refreshRunDetails: () => void;
  closeRunDrawer: () => void;
};

export function ChatInspectorColumn({
  pendingApprovalCount,
  a2uiSurfaces,
  runIds,
  selectedSession,
  selectedSessionLineage,
  contextBudgetEstimatedTokens,
  transcriptBusy,
  transcriptSearchQuery,
  setTranscriptSearchQuery,
  transcriptSearchBusy,
  canSearchTranscript,
  pinnedRecordKeys,
  searchResults,
  searchTranscript,
  inspectSearchMatch,
  exportBusy,
  exportTranscript,
  recentTranscriptRecords,
  inspectTranscriptRecord,
  pinTranscriptRecord,
  sessionPins,
  deletePin,
  queuedInputs,
  detailPanel,
  revealSensitiveValues,
  inspectorVisible,
  runDrawerId,
  setRunDrawerId,
  runDrawerBusy,
  runStatus,
  runTape,
  refreshRunDetails,
  closeRunDrawer,
}: ChatInspectorColumnProps) {
  return (
    <div className="chat-inspector-column">
      <SectionCard
        className="chat-panel chat-panel--sticky"
        description="Branch lineage, queue backlog, and persisted transcript tools stay visible without turning the main conversation into a debug dump."
        title="Workspace signals"
      >
        <div className="workspace-tag-row">
          <Chip color={pendingApprovalCount > 0 ? "warning" : "success"} variant="soft">
            {pendingApprovalCount} approval{pendingApprovalCount === 1 ? "" : "s"}
          </Chip>
          <Chip variant="secondary">
            {a2uiSurfaces.length} A2UI surface{a2uiSurfaces.length === 1 ? "" : "s"}
          </Chip>
          <Chip variant="secondary">
            {runIds.length} known run{runIds.length === 1 ? "" : "s"}
          </Chip>
        </div>
        <KeyValueList
          items={[
            {
              label: "Session",
              value:
                selectedSession?.title ||
                (selectedSession ? shortId(selectedSession.session_id) : "none"),
            },
            {
              label: "Branch state",
              value: describeBranchState(selectedSession?.branch_state ?? "missing"),
            },
            {
              label: "Lineage",
              value: selectedSessionLineage,
            },
            {
              label: "Budget",
              value: `${formatApproxTokens(contextBudgetEstimatedTokens)} estimated`,
            },
          ]}
        />
        {a2uiSurfaces.length === 0 ? (
          <EmptyState
            compact
            description="No A2UI documents published for this session yet."
            title="No A2UI surfaces"
          />
        ) : (
          <ul className="workspace-bullet-list">
            {a2uiSurfaces.map((surface) => (
              <li key={surface}>{surface}</li>
            ))}
          </ul>
        )}
      </SectionCard>

      <SectionCard
        className="chat-panel"
        description="Search persisted events, pin important tape entries, export the session, and inspect queued follow-ups."
        title="Transcript tools"
      >
        <div className="workspace-field-grid workspace-field-grid--double">
          <label className="workspace-field">
            <span className="workspace-kicker">Transcript search</span>
            <input
              className="w-full"
              placeholder="approval, tool, or summary text"
              value={transcriptSearchQuery}
              onChange={(event) => setTranscriptSearchQuery(event.currentTarget.value)}
            />
          </label>
          <div className="chat-transcript-tools__actions">
            <ActionButton
              isDisabled={transcriptSearchBusy || !canSearchTranscript}
              type="button"
              variant="secondary"
              onPress={searchTranscript}
            >
              {transcriptSearchBusy ? "Searching..." : "Search"}
            </ActionButton>
            <ActionButton
              isDisabled={exportBusy !== null}
              type="button"
              variant="secondary"
              onPress={() => exportTranscript("json")}
            >
              {exportBusy === "json" ? "Exporting..." : "Export JSON"}
            </ActionButton>
            <ActionButton
              isDisabled={exportBusy !== null}
              type="button"
              variant="secondary"
              onPress={() => exportTranscript("markdown")}
            >
              {exportBusy === "markdown" ? "Exporting..." : "Export Markdown"}
            </ActionButton>
          </div>
        </div>

        {searchResults.length > 0 ? (
          <div className="chat-ops-list">
            <div className="workspace-panel__intro">
              <p className="workspace-kicker">Matches</p>
              <h3>{searchResults.length} results</h3>
            </div>
            {searchResults.map((match) => (
              <article key={`${match.run_id}-${match.seq}`} className="chat-ops-card">
                <div className="chat-ops-card__copy">
                  <strong>
                    {prettifyEventType(match.event_type)} #{match.seq}
                  </strong>
                  {pinnedRecordKeys.has(`${match.run_id}:${match.seq}`) ? (
                    <div className="workspace-chip-row">
                      <Chip size="sm" variant="secondary">
                        Pinned
                      </Chip>
                    </div>
                  ) : null}
                  <span>
                    {match.origin_kind}
                    {match.origin_run_id !== undefined
                      ? ` · from ${shortId(match.origin_run_id)}`
                      : ""}
                  </span>
                  <p>{match.snippet}</p>
                </div>
                <ActionButton
                  size="sm"
                  type="button"
                  variant="secondary"
                  onPress={() => inspectSearchMatch(match)}
                >
                  Inspect
                </ActionButton>
              </article>
            ))}
          </div>
        ) : null}

        <div className="chat-ops-list">
          <div className="workspace-panel__intro">
            <p className="workspace-kicker">Recent persisted events</p>
            <h3>{transcriptBusy ? "Loading..." : `${recentTranscriptRecords.length} records`}</h3>
          </div>
          {recentTranscriptRecords.length === 0 ? (
            <EmptyState
              compact
              description="Stream or retry a run to populate persisted transcript events."
              title="No persisted transcript yet"
            />
          ) : (
            recentTranscriptRecords.map((record) => (
              <article key={`${record.run_id}-${record.seq}`} className="chat-ops-card">
                <div className="chat-ops-card__copy">
                  <strong>
                    {prettifyEventType(record.event_type)} #{record.seq}
                  </strong>
                  {pinnedRecordKeys.has(`${record.run_id}:${record.seq}`) ? (
                    <div className="workspace-chip-row">
                      <Chip size="sm" variant="secondary">
                        Pinned
                      </Chip>
                    </div>
                  ) : null}
                  <span>
                    {record.origin_kind}
                    {record.origin_run_id !== undefined
                      ? ` · from ${shortId(record.origin_run_id)}`
                      : ""}
                  </span>
                </div>
                <div className="chat-ops-card__actions">
                  <ActionButton
                    size="sm"
                    type="button"
                    variant="secondary"
                    onPress={() => inspectTranscriptRecord(record)}
                  >
                    Inspect
                  </ActionButton>
                  <ActionButton
                    size="sm"
                    type="button"
                    variant="secondary"
                    onPress={() => pinTranscriptRecord(record)}
                  >
                    Pin
                  </ActionButton>
                </div>
              </article>
            ))
          )}
        </div>

        <div className="chat-ops-list">
          <div className="workspace-panel__intro">
            <p className="workspace-kicker">Pins</p>
            <h3>{sessionPins.length}</h3>
          </div>
          {sessionPins.length === 0 ? (
            <p className="chat-muted">Pin important transcript events to keep them visible.</p>
          ) : (
            sessionPins.map((pin) => (
              <article key={pin.pin_id} className="chat-ops-card">
                <div className="chat-ops-card__copy">
                  <strong>{pin.title}</strong>
                  <span>
                    Run {shortId(pin.run_id)} · tape #{pin.tape_seq}
                  </span>
                  {pin.note !== undefined ? <p>{pin.note}</p> : null}
                </div>
                <ActionButton
                  size="sm"
                  type="button"
                  variant="danger"
                  onPress={() => deletePin(pin.pin_id)}
                >
                  Delete
                </ActionButton>
              </article>
            ))
          )}
        </div>

        <div className="chat-ops-list">
          <div className="workspace-panel__intro">
            <p className="workspace-kicker">Queued follow-ups</p>
            <h3>{queuedInputs.length}</h3>
          </div>
          {queuedInputs.length === 0 ? (
            <p className="chat-muted">No queued follow-ups are stored for this session.</p>
          ) : (
            [...queuedInputs].reverse().map((queued) => (
              <article key={queued.queued_input_id} className="chat-ops-card">
                <div className="chat-ops-card__copy">
                  <strong>{queued.state}</strong>
                  <span>
                    {shortId(queued.queued_input_id)} · run {shortId(queued.run_id)}
                  </span>
                  <p>{queued.text}</p>
                </div>
              </article>
            ))
          )}
        </div>
      </SectionCard>

      <SectionCard
        className="chat-panel"
        description="Inspect raw tool payloads, persisted transcript events, and search matches without flooding the conversation timeline."
        title="Detail sidebar"
      >
        {detailPanel === null ? (
          <EmptyState
            compact
            description="Choose Inspect on a payload, transcript event, or search result."
            title="No detail selected"
          />
        ) : (
          <div className="chat-detail-panel">
            <div className="workspace-panel__intro">
              <p className="workspace-kicker">Selected detail</p>
              <h3>{detailPanel.title}</h3>
              <p className="chat-muted">{detailPanel.subtitle}</p>
            </div>
            {detailPanel.body !== undefined ? (
              <p className="chat-entry-text">{detailPanel.body}</p>
            ) : null}
            {detailPanel.payload !== undefined ? (
              <PrettyJsonBlock
                className="chat-detail-panel__payload"
                revealSensitiveValues={revealSensitiveValues}
                value={detailPanel.payload}
              />
            ) : null}
          </div>
        )}
      </SectionCard>

      {inspectorVisible ? (
        <SectionCard
          className="chat-panel"
          description="Status, tape, token usage, and lineage metadata stay secondary to the transcript but available on demand."
          title="Run inspector"
        >
          <ChatRunDrawer
            open
            runIds={runIds}
            runDrawerId={runDrawerId}
            setRunDrawerId={setRunDrawerId}
            runDrawerBusy={runDrawerBusy}
            runStatus={runStatus}
            runTape={runTape}
            revealSensitiveValues={revealSensitiveValues}
            refreshRun={refreshRunDetails}
            close={closeRunDrawer}
          />
        </SectionCard>
      ) : (
        <SectionCard
          className="chat-panel"
          description="Run details become available after the first streamed response."
          title="Run inspector"
        >
          <EmptyState
            compact
            description="Open a run after the first streamed response to inspect status, tape, and token usage."
            title="Run details will appear here"
          />
        </SectionCard>
      )}
    </div>
  );
}
