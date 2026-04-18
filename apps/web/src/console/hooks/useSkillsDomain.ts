import { type FormEvent, useState } from "react";

import type { ConsoleApiClient, JsonValue } from "../../consoleApi";
import {
  emptyToUndefined,
  readObject,
  readString,
  skillMetadata,
  toErrorMessage,
  toJsonObjectArray,
  type JsonObject,
} from "../shared";

type UseSkillsDomainArgs = {
  api: ConsoleApiClient;
  setError: (value: string | null) => void;
  setNotice: (value: string | null) => void;
  refreshLearningQueue: () => Promise<void>;
};

type PluginUpsertPayload = {
  plugin_id: string;
  skill_id: string;
  skill_version?: string;
  tool_id?: string;
  module_path?: string;
  entrypoint?: string;
  enabled?: boolean;
  capability_profile?: JsonValue;
  operator?: JsonValue;
  config?: JsonValue;
  clear_config?: boolean;
};

function pluginIdFromEntry(entry: JsonObject): string | null {
  const binding = readObject(entry, "binding");
  return binding === null ? null : readString(binding, "plugin_id");
}

function buildPluginUpsertPayload(
  detail: JsonObject,
  config: JsonValue | undefined,
  clearConfig = false,
): PluginUpsertPayload {
  const binding = readObject(detail, "binding");
  if (binding === null) {
    throw new Error("Plugin detail is missing binding metadata.");
  }
  const pluginId = readString(binding, "plugin_id");
  const skillId = readString(binding, "skill_id");
  if (pluginId === null || skillId === null) {
    throw new Error("Plugin binding is missing plugin_id or skill_id.");
  }
  return {
    plugin_id: pluginId,
    skill_id: skillId,
    skill_version: readString(binding, "skill_version") ?? undefined,
    tool_id: readString(binding, "tool_id") ?? undefined,
    module_path: readString(binding, "module_path") ?? undefined,
    entrypoint: readString(binding, "entrypoint") ?? undefined,
    enabled: binding["enabled"] === true,
    capability_profile: readObject(binding, "capability_profile") ?? undefined,
    operator: readObject(binding, "operator") ?? undefined,
    config,
    clear_config: clearConfig ? true : undefined,
  };
}

export function useSkillsDomain({
  api,
  setError,
  setNotice,
  refreshLearningQueue,
}: UseSkillsDomainArgs) {
  const [skillsBusy, setSkillsBusy] = useState(false);
  const [skillsEntries, setSkillsEntries] = useState<JsonObject[]>([]);
  const [pluginEntries, setPluginEntries] = useState<JsonObject[]>([]);
  const [selectedPluginId, setSelectedPluginId] = useState("");
  const [selectedPluginDetail, setSelectedPluginDetail] = useState<JsonObject | null>(null);
  const [skillProcedureCandidates, setSkillProcedureCandidates] = useState<JsonObject[]>([]);
  const [skillBuilderCandidates, setSkillBuilderCandidates] = useState<JsonObject[]>([]);
  const [lastSkillPromotion, setLastSkillPromotion] = useState<JsonObject | null>(null);
  const [skillArtifactPath, setSkillArtifactPath] = useState("");
  const [skillAllowTofu, setSkillAllowTofu] = useState(true);
  const [skillAllowUntrusted, setSkillAllowUntrusted] = useState(false);
  const [skillReason, setSkillReason] = useState("");
  const [skillBuilderPrompt, setSkillBuilderPrompt] = useState("");
  const [skillBuilderName, setSkillBuilderName] = useState("");

  async function refreshSkills(): Promise<void> {
    setSkillsBusy(true);
    setError(null);
    try {
      const [response, pluginResponse, candidateResponse, builderResponse] = await Promise.all([
        api.listSkills(),
        api.listPlugins(),
        api.listLearningCandidates(
          new URLSearchParams([
            ["candidate_kind", "procedure"],
            ["limit", "24"],
          ]),
        ),
        api.listSkillBuilderCandidates(),
      ]);
      setSkillsEntries(toJsonObjectArray(response.entries));
      const nextPluginEntries = toJsonObjectArray(pluginResponse.entries as unknown as JsonValue[]);
      setPluginEntries(nextPluginEntries);
      const preferredPluginId =
        nextPluginEntries.find((entry) => pluginIdFromEntry(entry) === selectedPluginId) !==
        undefined
          ? selectedPluginId
          : pluginIdFromEntry(nextPluginEntries[0] ?? {});
      if (preferredPluginId !== null && preferredPluginId.trim().length > 0) {
        const pluginDetail = await api.getPlugin(preferredPluginId);
        setSelectedPluginId(preferredPluginId);
        setSelectedPluginDetail(pluginDetail as unknown as JsonObject);
      } else {
        setSelectedPluginId("");
        setSelectedPluginDetail(null);
      }
      setSkillProcedureCandidates(
        toJsonObjectArray(candidateResponse.candidates as unknown as JsonValue[]).filter(
          (candidate) => readString(candidate, "candidate_kind") === "procedure",
        ),
      );
      setSkillBuilderCandidates(
        toJsonObjectArray(builderResponse.entries as unknown as JsonValue[]),
      );
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setSkillsBusy(false);
    }
  }

  async function selectPlugin(pluginId: string): Promise<void> {
    const trimmed = pluginId.trim();
    if (trimmed.length === 0) {
      setSelectedPluginId("");
      setSelectedPluginDetail(null);
      return;
    }
    setSkillsBusy(true);
    setError(null);
    try {
      const response = await api.getPlugin(trimmed);
      setSelectedPluginId(trimmed);
      setSelectedPluginDetail(response as unknown as JsonObject);
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setSkillsBusy(false);
    }
  }

  async function checkPlugin(pluginId?: string): Promise<void> {
    const targetId = (pluginId ?? selectedPluginId).trim();
    if (targetId.length === 0) {
      setError("Select a plugin first.");
      return;
    }
    setSkillsBusy(true);
    setError(null);
    try {
      await api.checkPlugin(targetId);
      setNotice(`Plugin '${targetId}' checked.`);
      await refreshSkills();
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setSkillsBusy(false);
    }
  }

  async function savePluginConfig(pluginId: string, configDocument: string): Promise<void> {
    const trimmedPluginId = pluginId.trim();
    if (trimmedPluginId.length === 0) {
      setError("Select a plugin first.");
      return;
    }
    if (selectedPluginDetail === null) {
      setError("Plugin detail is unavailable.");
      return;
    }
    const trimmedDocument = configDocument.trim();
    if (trimmedDocument.length === 0) {
      setError("Config JSON cannot be empty.");
      return;
    }
    let parsed: JsonValue;
    try {
      parsed = JSON.parse(trimmedDocument) as JsonValue;
    } catch {
      setError("Config JSON must be a valid JSON object.");
      return;
    }
    if (parsed === null || Array.isArray(parsed) || typeof parsed !== "object") {
      setError("Config JSON must be a JSON object.");
      return;
    }
    setSkillsBusy(true);
    setError(null);
    try {
      await api.upsertPlugin(buildPluginUpsertPayload(selectedPluginDetail, parsed));
      setNotice(`Plugin config for '${trimmedPluginId}' saved.`);
      await refreshSkills();
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setSkillsBusy(false);
    }
  }

  async function clearPluginConfig(pluginId: string): Promise<void> {
    const trimmedPluginId = pluginId.trim();
    if (trimmedPluginId.length === 0) {
      setError("Select a plugin first.");
      return;
    }
    if (selectedPluginDetail === null) {
      setError("Plugin detail is unavailable.");
      return;
    }
    setSkillsBusy(true);
    setError(null);
    try {
      await api.upsertPlugin(buildPluginUpsertPayload(selectedPluginDetail, undefined, true));
      setNotice(`Plugin config for '${trimmedPluginId}' cleared.`);
      await refreshSkills();
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setSkillsBusy(false);
    }
  }

  async function togglePluginEnabled(pluginId: string, enabled: boolean): Promise<void> {
    const trimmed = pluginId.trim();
    if (trimmed.length === 0) {
      setError("Select a plugin first.");
      return;
    }
    setSkillsBusy(true);
    setError(null);
    try {
      if (enabled) {
        await api.enablePlugin(trimmed);
      } else {
        await api.disablePlugin(trimmed);
      }
      setNotice(`Plugin '${trimmed}' ${enabled ? "enabled" : "disabled"}.`);
      await refreshSkills();
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setSkillsBusy(false);
    }
  }

  async function installSkill(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (skillArtifactPath.trim().length === 0) {
      setError("Artifact path cannot be empty.");
      return;
    }
    setSkillsBusy(true);
    setError(null);
    try {
      await api.installSkill({
        artifact_path: skillArtifactPath.trim(),
        allow_tofu: skillAllowTofu,
        allow_untrusted: skillAllowUntrusted,
      });
      setSkillArtifactPath("");
      setNotice("Skill installed.");
      await refreshSkills();
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setSkillsBusy(false);
    }
  }

  async function executeSkillAction(
    entry: JsonObject,
    action: "verify" | "audit" | "quarantine" | "enable",
  ): Promise<void> {
    const metadata = skillMetadata(entry);
    if (metadata === null) {
      setError("Skill entry is missing record metadata.");
      return;
    }

    setSkillsBusy(true);
    setError(null);
    try {
      if (action === "verify") {
        await api.verifySkill(metadata.skillId, { version: metadata.version, allow_tofu: false });
      }
      if (action === "audit") {
        await api.auditSkill(metadata.skillId, {
          version: metadata.version,
          allow_tofu: false,
          quarantine_on_fail: true,
        });
      }
      if (action === "quarantine") {
        await api.quarantineSkill({
          skill_id: metadata.skillId,
          version: metadata.version,
          reason: emptyToUndefined(skillReason),
        });
      }
      if (action === "enable") {
        await api.enableSkill({
          skill_id: metadata.skillId,
          version: metadata.version,
          reason: emptyToUndefined(skillReason),
        });
      }
      setNotice(`Skill action '${action}' completed.`);
      await refreshSkills();
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setSkillsBusy(false);
    }
  }

  async function promoteProcedureCandidate(candidateId: string): Promise<void> {
    setSkillsBusy(true);
    setError(null);
    try {
      const response = await api.promoteProcedureCandidate(candidateId);
      setLastSkillPromotion(response.skill as unknown as JsonObject);
      setNotice("Procedure candidate promoted into a quarantined skill scaffold.");
      await refreshSkills();
      await refreshLearningQueue();
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setSkillsBusy(false);
    }
  }

  async function createSkillBuilderCandidate(event?: FormEvent<HTMLFormElement>): Promise<void> {
    event?.preventDefault();
    if (skillBuilderPrompt.trim().length === 0) {
      setError("Builder prompt cannot be empty.");
      return;
    }
    setSkillsBusy(true);
    setError(null);
    try {
      const response = await api.createSkillBuilderCandidate({
        prompt: skillBuilderPrompt.trim(),
        name: emptyToUndefined(skillBuilderName),
        review_notes: emptyToUndefined(skillReason),
      });
      setSkillBuilderPrompt("");
      setSkillBuilderName("");
      setLastSkillPromotion(response.skill as unknown as JsonObject);
      setNotice("Builder candidate created in quarantine.");
      await refreshSkills();
    } catch (failure) {
      setError(toErrorMessage(failure));
    } finally {
      setSkillsBusy(false);
    }
  }

  function resetSkillsDomain(): void {
    setSkillsBusy(false);
    setSkillsEntries([]);
    setPluginEntries([]);
    setSelectedPluginId("");
    setSelectedPluginDetail(null);
    setSkillProcedureCandidates([]);
    setSkillBuilderCandidates([]);
    setLastSkillPromotion(null);
    setSkillArtifactPath("");
    setSkillAllowTofu(true);
    setSkillAllowUntrusted(false);
    setSkillReason("");
    setSkillBuilderPrompt("");
    setSkillBuilderName("");
  }

  return {
    skillsBusy,
    skillsEntries,
    pluginEntries,
    selectedPluginId,
    selectedPluginDetail,
    skillProcedureCandidates,
    skillBuilderCandidates,
    lastSkillPromotion,
    skillArtifactPath,
    setSkillArtifactPath,
    skillAllowTofu,
    setSkillAllowTofu,
    skillAllowUntrusted,
    setSkillAllowUntrusted,
    skillReason,
    setSkillReason,
    skillBuilderPrompt,
    setSkillBuilderPrompt,
    skillBuilderName,
    setSkillBuilderName,
    refreshSkills,
    selectPlugin,
    checkPlugin,
    savePluginConfig,
    clearPluginConfig,
    togglePluginEnabled,
    installSkill,
    executeSkillAction,
    promoteProcedureCandidate,
    createSkillBuilderCandidate,
    resetSkillsDomain,
  };
}
