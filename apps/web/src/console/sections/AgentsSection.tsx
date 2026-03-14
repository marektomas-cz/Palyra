import { Button, Modal } from "@heroui/react";
import { useEffect, useMemo, useState } from "react";

import type {
  AgentCreateRequest,
  AgentEnvelope,
  AgentListEnvelope,
  AgentRecord
} from "../../consoleApi";
import {
  WorkspaceMetricCard,
  WorkspacePageHeader,
  WorkspaceSectionCard,
  WorkspaceStatusChip
} from "../components/workspace/WorkspaceChrome";
import type { ConsoleAppState } from "../useConsoleAppState";

type AgentsSectionProps = {
  app: Pick<ConsoleAppState, "api" | "setError" | "setNotice">;
};

type WizardStep = 0 | 1 | 2 | 3;

type AgentDraft = {
  agentId: string;
  displayName: string;
  agentDir: string;
  workspaceRoots: string;
  defaultModelProfile: string;
  defaultToolAllowlist: string;
  defaultSkillAllowlist: string;
  setDefault: boolean;
  allowAbsolutePaths: boolean;
};

const WIZARD_STEPS: ReadonlyArray<{
  id: WizardStep;
  label: string;
  description: string;
}> = [
  { id: 0, label: "Identity", description: "Choose a stable id and an operator-friendly display name." },
  { id: 1, label: "Storage", description: "Keep workspace paths local unless you explicitly opt into absolute paths." },
  { id: 2, label: "Defaults", description: "Set the default model and any explicit tool or skill allowlists." },
  { id: 3, label: "Review", description: "Confirm the new registry entry before submitting it." }
];

function createDefaultDraft(): AgentDraft {
  return {
    agentId: "",
    displayName: "",
    agentDir: "",
    workspaceRoots: "workspace",
    defaultModelProfile: "gpt-4o-mini",
    defaultToolAllowlist: "",
    defaultSkillAllowlist: "",
    setDefault: false,
    allowAbsolutePaths: false
  };
}

function parseTextList(value: string): string[] {
  const entries = value
    .split(/[\r\n,]+/)
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
  return Array.from(new Set(entries));
}

function resolveWorkspaceRoots(draft: AgentDraft): string[] {
  const roots = parseTextList(draft.workspaceRoots);
  return roots.length > 0 ? roots : ["workspace"];
}

function validationMessageForStep(step: WizardStep, draft: AgentDraft): string | null {
  if (step === 0) {
    if (!/^[a-z0-9][a-z0-9-]*$/.test(draft.agentId.trim())) {
      return "Agent ID must use lowercase letters, numbers, and hyphens only.";
    }
    if (draft.displayName.trim().length === 0) {
      return "Display name is required.";
    }
  }

  if (step === 1 && resolveWorkspaceRoots(draft).length === 0) {
    return "At least one workspace root is required.";
  }

  return null;
}

function buildCreatePayload(draft: AgentDraft): AgentCreateRequest {
  const agentDir = draft.agentDir.trim();
  const defaultModelProfile = draft.defaultModelProfile.trim();

  return {
    agent_id: draft.agentId.trim(),
    display_name: draft.displayName.trim(),
    agent_dir: agentDir.length > 0 ? agentDir : undefined,
    workspace_roots: resolveWorkspaceRoots(draft),
    default_model_profile: defaultModelProfile.length > 0 ? defaultModelProfile : undefined,
    default_tool_allowlist: parseTextList(draft.defaultToolAllowlist),
    default_skill_allowlist: parseTextList(draft.defaultSkillAllowlist),
    set_default: draft.setDefault,
    allow_absolute_paths: draft.allowAbsolutePaths
  };
}

function formatUnixMs(value: number): string {
  return new Intl.DateTimeFormat("sv-SE", {
    dateStyle: "short",
    timeStyle: "short",
    timeZone: "UTC"
  })
    .format(new Date(value))
    .replace(",", "");
}

export function AgentsSection({ app }: AgentsSectionProps) {
  const [agentsBusy, setAgentsBusy] = useState(false);
  const [detailBusy, setDetailBusy] = useState(false);
  const [filter, setFilter] = useState("");
  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [defaultAgentId, setDefaultAgentId] = useState<string | null>(null);
  const [selectedAgentId, setSelectedAgentId] = useState("");
  const [selectedAgent, setSelectedAgent] = useState<AgentEnvelope | null>(null);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [wizardStep, setWizardStep] = useState<WizardStep>(0);
  const [draft, setDraft] = useState<AgentDraft>(createDefaultDraft);

  async function loadAgent(agentId: string): Promise<void> {
    if (agentId.trim().length === 0) {
      setSelectedAgent(null);
      return;
    }

    setDetailBusy(true);
    app.setError(null);
    try {
      const envelope = await app.api.getAgent(agentId);
      setSelectedAgent(envelope);
    } catch (error) {
      app.setError(error instanceof Error ? error.message : "Failed to load agent detail.");
    } finally {
      setDetailBusy(false);
    }
  }

  async function refreshAgents(preferredAgentId?: string): Promise<void> {
    setAgentsBusy(true);
    app.setError(null);
    try {
      const envelope: AgentListEnvelope = await app.api.listAgents();
      setAgents(envelope.agents);
      setDefaultAgentId(envelope.default_agent_id ?? null);

      const nextSelectedId =
        preferredAgentId ??
        (selectedAgentId.length > 0 && envelope.agents.some((agent) => agent.agent_id === selectedAgentId)
          ? selectedAgentId
          : envelope.default_agent_id ?? envelope.agents[0]?.agent_id ?? "");

      setSelectedAgentId(nextSelectedId);
      if (nextSelectedId.length === 0) {
        setSelectedAgent(null);
      } else {
        await loadAgent(nextSelectedId);
      }
    } catch (error) {
      app.setError(error instanceof Error ? error.message : "Failed to load agents.");
    } finally {
      setAgentsBusy(false);
    }
  }

  useEffect(() => {
    void refreshAgents();
  }, []);

  useEffect(() => {
    if (selectedAgentId.length === 0 || selectedAgent?.agent.agent_id === selectedAgentId) {
      return;
    }
    void loadAgent(selectedAgentId);
  }, [selectedAgentId]);

  const filteredAgents = useMemo(() => {
    const query = filter.trim().toLowerCase();
    if (query.length === 0) {
      return agents;
    }
    return agents.filter((agent) =>
      `${agent.display_name} ${agent.agent_id} ${agent.default_model_profile}`.toLowerCase().includes(query)
    );
  }, [agents, filter]);

  const validationMessage = validationMessageForStep(wizardStep, draft);
  const detailRecord = selectedAgent?.agent ?? null;

  function closeWizard(): void {
    setWizardOpen(false);
    setWizardStep(0);
    setDraft(createDefaultDraft());
  }

  async function handleCreateAgent(): Promise<void> {
    setAgentsBusy(true);
    app.setError(null);
    try {
      const created = await app.api.createAgent(buildCreatePayload(draft));
      app.setNotice(`Agent '${created.agent.display_name}' created.`);
      closeWizard();
      await refreshAgents(created.agent.agent_id);
    } catch (error) {
      app.setError(error instanceof Error ? error.message : "Failed to create agent.");
    } finally {
      setAgentsBusy(false);
    }
  }

  async function handleSetDefault(agentId: string): Promise<void> {
    setAgentsBusy(true);
    app.setError(null);
    try {
      const result = await app.api.setDefaultAgent(agentId);
      app.setNotice(`Default agent set to '${result.default_agent_id}'.`);
      await refreshAgents(result.default_agent_id);
    } catch (error) {
      app.setError(error instanceof Error ? error.message : "Failed to set default agent.");
    } finally {
      setAgentsBusy(false);
    }
  }

  return (
    <main className="workspace-page">
      <WorkspacePageHeader
        eyebrow="Agent"
        title="Agents"
        description="Work against the real agent registry, create new agents with safe defaults, and promote a default agent without dropping to the CLI."
        status={
          <>
            <WorkspaceStatusChip tone={agents.length > 0 ? "success" : "default"}>
              {agents.length} registered
            </WorkspaceStatusChip>
            <WorkspaceStatusChip tone={defaultAgentId !== null ? "success" : "warning"}>
              {defaultAgentId !== null ? `Default ${defaultAgentId}` : "No default agent"}
            </WorkspaceStatusChip>
            <WorkspaceStatusChip tone={detailRecord !== null ? "accent" : "default"}>
              {detailRecord?.default_model_profile ?? "No model selected"}
            </WorkspaceStatusChip>
          </>
        }
        actions={
          <div className="console-inline-actions">
            <Button variant="secondary" onPress={() => void refreshAgents()} isDisabled={agentsBusy}>
              {agentsBusy ? "Refreshing..." : "Refresh agents"}
            </Button>
            <Button onPress={() => setWizardOpen(true)}>Create agent</Button>
          </div>
        }
      />

      <section className="workspace-metric-grid workspace-metric-grid--compact">
        <WorkspaceMetricCard
          label="Registry size"
          value={agents.length}
          detail={agents.length > 0 ? `${agents[0]?.display_name ?? "Agent"} is ready for review.` : "Create the first agent to establish a default registry."}
          tone={agents.length > 0 ? "success" : "warning"}
        />
        <WorkspaceMetricCard
          label="Workspace roots"
          value={detailRecord?.workspace_roots.length ?? 0}
          detail={detailRecord?.workspace_roots[0] ?? "Workspace defaults stay local and explicit."}
        />
        <WorkspaceMetricCard
          label="Default model"
          value={detailRecord?.default_model_profile ?? "n/a"}
          detail="Each agent keeps its own runtime defaults and allowlists."
          tone={detailRecord !== null ? "accent" : "default"}
        />
      </section>

      <section className="workspace-two-column">
        <WorkspaceSectionCard
          title="Registry"
          description="Search the registry, scan default state quickly, and select an agent for detail review."
        >
          <div className="workspace-stack">
            <label>
              Search agents
              <input
                value={filter}
                onChange={(event) => setFilter(event.target.value)}
                placeholder="main, review, gpt-4o-mini"
              />
            </label>

            <div className="workspace-list">
              {filteredAgents.length === 0 ? (
                <div className="workspace-empty">
                  {agents.length === 0 ? "No agents registered yet. Open the setup wizard to create the first one." : "No agents match the current filter."}
                </div>
              ) : (
                filteredAgents.map((agent) => {
                  const isActive = agent.agent_id === selectedAgentId;
                  const isDefault = agent.agent_id === defaultAgentId;
                  return (
                    <article key={agent.agent_id} className={`workspace-list-item workspace-list-item--job${isActive ? " is-active" : ""}`}>
                      <button
                        type="button"
                        className={`workspace-list-button workspace-list-button--flat${isActive ? " is-active" : ""}`}
                        onClick={() => setSelectedAgentId(agent.agent_id)}
                      >
                        <div className="workspace-list-button__meta">
                          <strong className="workspace-list-button__title">{agent.display_name}</strong>
                          <small>{agent.agent_id}</small>
                        </div>
                        <div className="workspace-tag-row">
                          <WorkspaceStatusChip tone={isDefault ? "success" : "default"}>
                            {isDefault ? "default" : "registered"}
                          </WorkspaceStatusChip>
                          <WorkspaceStatusChip tone="accent">{agent.default_model_profile}</WorkspaceStatusChip>
                        </div>
                      </button>
                    </article>
                  );
                })
              )}
            </div>
          </div>
        </WorkspaceSectionCard>

        <WorkspaceSectionCard
          title="Selected agent"
          description="Inspect directories, workspace roots, and allowlists before promoting a default."
        >
          {detailBusy ? (
            <p className="chat-muted">Loading agent detail...</p>
          ) : detailRecord === null ? (
            <div className="workspace-empty">Select an agent from the registry to inspect its detail.</div>
          ) : (
            <div className="workspace-stack">
              <div className="workspace-inline">
                <WorkspaceStatusChip tone={selectedAgent?.is_default ? "success" : "default"}>
                  {selectedAgent?.is_default ? "Default agent" : "Registered agent"}
                </WorkspaceStatusChip>
                <WorkspaceStatusChip tone="accent">{detailRecord.default_model_profile}</WorkspaceStatusChip>
              </div>

              <dl className="workspace-key-value-grid">
                <div>
                  <dt>Display name</dt>
                  <dd>{detailRecord.display_name}</dd>
                </div>
                <div>
                  <dt>Agent ID</dt>
                  <dd>{detailRecord.agent_id}</dd>
                </div>
                <div>
                  <dt>Agent dir</dt>
                  <dd>{detailRecord.agent_dir}</dd>
                </div>
                <div>
                  <dt>Created</dt>
                  <dd>{formatUnixMs(detailRecord.created_at_unix_ms)}</dd>
                </div>
                <div>
                  <dt>Updated</dt>
                  <dd>{formatUnixMs(detailRecord.updated_at_unix_ms)}</dd>
                </div>
              </dl>

              <div className="workspace-two-column">
                <div className="workspace-panel workspace-panel--embedded">
                  <div className="workspace-panel__intro">
                    <h3>Workspace roots</h3>
                    <p className="chat-muted">Keep the execution boundary visible and operator-reviewable.</p>
                  </div>
                  <ul className="workspace-bullet-list">
                    {detailRecord.workspace_roots.map((root) => (
                      <li key={root}>{root}</li>
                    ))}
                  </ul>
                </div>

                <div className="workspace-panel workspace-panel--embedded">
                  <div className="workspace-panel__intro">
                    <h3>Allowlists</h3>
                    <p className="chat-muted">No speculative permissions are injected by the wizard.</p>
                  </div>
                  <p><strong>Tools:</strong> {detailRecord.default_tool_allowlist.join(", ") || "No explicit tool allowlist"}</p>
                  <p><strong>Skills:</strong> {detailRecord.default_skill_allowlist.join(", ") || "No explicit skill allowlist"}</p>
                </div>
              </div>

              {!selectedAgent?.is_default && (
                <div className="workspace-inline">
                  <Button onPress={() => void handleSetDefault(detailRecord.agent_id)} isDisabled={agentsBusy}>
                    {agentsBusy ? "Applying..." : "Set as default"}
                  </Button>
                </div>
              )}
            </div>
          )}
        </WorkspaceSectionCard>
      </section>

      <Modal isOpen={wizardOpen} onOpenChange={setWizardOpen}>
        <Modal.Backdrop />
        <Modal.Container placement="center" size="lg">
          <Modal.Dialog>
            <Modal.Header>
              <div className="workspace-stack">
                <h3>Create agent</h3>
                <p className="chat-muted">Create a real registry entry backed by the daemon, not local mock state.</p>
              </div>
            </Modal.Header>
            <Modal.Body>
              <div className="workspace-tab-row" role="tablist" aria-label="Agent wizard steps">
                {WIZARD_STEPS.map((step) => (
                  <button
                    key={step.id}
                    type="button"
                    role="tab"
                    aria-selected={wizardStep === step.id}
                    className={`workspace-tab-button${wizardStep === step.id ? " is-active" : ""}`}
                    onClick={() => setWizardStep(step.id)}
                  >
                    {step.label}
                  </button>
                ))}
              </div>

              <div className="workspace-panel workspace-panel--embedded">
                <div className="workspace-panel__intro">
                  <h3>{WIZARD_STEPS[wizardStep].label}</h3>
                  <p className="chat-muted">{WIZARD_STEPS[wizardStep].description}</p>
                </div>

                {wizardStep === 0 && (
                  <div className="workspace-form-grid">
                    <label>
                      Agent ID
                      <input
                        value={draft.agentId}
                        onChange={(event) => setDraft((current) => ({ ...current, agentId: event.target.value }))}
                        placeholder="review-agent"
                      />
                    </label>
                    <label>
                      Display name
                      <input
                        value={draft.displayName}
                        onChange={(event) => setDraft((current) => ({ ...current, displayName: event.target.value }))}
                        placeholder="Review Agent"
                      />
                    </label>
                  </div>
                )}

                {wizardStep === 1 && (
                  <div className="workspace-stack">
                    <label>
                      Agent dir
                      <input
                        value={draft.agentDir}
                        onChange={(event) => setDraft((current) => ({ ...current, agentDir: event.target.value }))}
                        placeholder="Leave blank for safe state-root defaults"
                      />
                    </label>
                    <label>
                      Workspace roots
                      <textarea
                        rows={4}
                        value={draft.workspaceRoots}
                        onChange={(event) => setDraft((current) => ({ ...current, workspaceRoots: event.target.value }))}
                        placeholder={"workspace\nworkspace-review"}
                      />
                    </label>
                    <label className="console-checkbox-inline">
                      <input
                        type="checkbox"
                        checked={draft.allowAbsolutePaths}
                        onChange={(event) =>
                          setDraft((current) => ({ ...current, allowAbsolutePaths: event.target.checked }))
                        }
                      />
                      Allow absolute paths
                    </label>
                  </div>
                )}

                {wizardStep === 2 && (
                  <div className="workspace-stack">
                    <div className="workspace-form-grid">
                      <label>
                        Default model profile
                        <input
                          value={draft.defaultModelProfile}
                          onChange={(event) =>
                            setDraft((current) => ({ ...current, defaultModelProfile: event.target.value }))
                          }
                          placeholder="gpt-4o-mini"
                        />
                      </label>
                      <label className="console-checkbox-inline">
                        <input
                          type="checkbox"
                          checked={draft.setDefault}
                          onChange={(event) => setDraft((current) => ({ ...current, setDefault: event.target.checked }))}
                        />
                        Set as default agent
                      </label>
                    </div>
                    <label>
                      Tool allowlist
                      <textarea
                        rows={3}
                        value={draft.defaultToolAllowlist}
                        onChange={(event) =>
                          setDraft((current) => ({ ...current, defaultToolAllowlist: event.target.value }))
                        }
                        placeholder={"palyra.echo\npalyra.http.fetch"}
                      />
                    </label>
                    <label>
                      Skill allowlist
                      <textarea
                        rows={3}
                        value={draft.defaultSkillAllowlist}
                        onChange={(event) =>
                          setDraft((current) => ({ ...current, defaultSkillAllowlist: event.target.value }))
                        }
                        placeholder={"acme.echo\nacme.review"}
                      />
                    </label>
                  </div>
                )}

                {wizardStep === 3 && (
                  <dl className="workspace-key-value-grid">
                    <div>
                      <dt>Agent ID</dt>
                      <dd>{draft.agentId.trim() || "n/a"}</dd>
                    </div>
                    <div>
                      <dt>Display name</dt>
                      <dd>{draft.displayName.trim() || "n/a"}</dd>
                    </div>
                    <div>
                      <dt>Agent dir</dt>
                      <dd>{draft.agentDir.trim() || "Auto under state root"}</dd>
                    </div>
                    <div>
                      <dt>Workspace roots</dt>
                      <dd>{resolveWorkspaceRoots(draft).join(", ")}</dd>
                    </div>
                    <div>
                      <dt>Model profile</dt>
                      <dd>{draft.defaultModelProfile.trim() || "Backend default"}</dd>
                    </div>
                    <div>
                      <dt>Tool allowlist</dt>
                      <dd>{parseTextList(draft.defaultToolAllowlist).join(", ") || "none"}</dd>
                    </div>
                    <div>
                      <dt>Skill allowlist</dt>
                      <dd>{parseTextList(draft.defaultSkillAllowlist).join(", ") || "none"}</dd>
                    </div>
                    <div>
                      <dt>Default selection</dt>
                      <dd>{draft.setDefault ? "Set as default" : "Keep current default"}</dd>
                    </div>
                    <div>
                      <dt>Absolute paths</dt>
                      <dd>{draft.allowAbsolutePaths ? "Allowed" : "Disabled"}</dd>
                    </div>
                  </dl>
                )}

                {validationMessage !== null && (
                  <div className="workspace-callout workspace-callout--warning">{validationMessage}</div>
                )}
              </div>
            </Modal.Body>
            <Modal.Footer>
              <div className="console-inline-actions">
                <Button variant="secondary" onPress={closeWizard}>
                  Cancel
                </Button>
                {wizardStep > 0 && (
                  <Button variant="secondary" onPress={() => setWizardStep((wizardStep - 1) as WizardStep)}>
                    Back
                  </Button>
                )}
                {wizardStep < 3 ? (
                  <Button
                    onPress={() => setWizardStep((wizardStep + 1) as WizardStep)}
                    isDisabled={validationMessage !== null}
                  >
                    Next
                  </Button>
                ) : (
                  <Button onPress={() => void handleCreateAgent()} isDisabled={validationMessage !== null || agentsBusy}>
                    {agentsBusy ? "Creating..." : "Create agent"}
                  </Button>
                )}
              </div>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal>
    </main>
  );
}
