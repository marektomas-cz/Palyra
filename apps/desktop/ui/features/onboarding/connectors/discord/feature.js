export function createDiscordOnboardingFeature({
  ui,
  invoke,
  desktopState,
  discordWizardState,
  setActionMessage,
  renderList,
  setLabeledStatus,
  asString,
  asPositiveNumber,
  formatUnixMs,
  normalizeEmptyToNull,
  refreshOnboardingStatus,
  refreshAllData
}) {
  function parseCommaSeparatedList(raw) {
    return String(raw ?? "")
      .split(",")
      .map((item) => item.trim())
      .filter((item) => item.length > 0);
  }

  function normalizeDiscordConnectorId(accountId) {
    const normalized = String(accountId ?? "").trim().toLowerCase();
    return normalized.length > 0 ? `discord:${normalized}` : "discord:default";
  }

  function renderDiscordChecklist(discord) {
    const items = [];
    const saturation = asString(discord.saturation_state, "unknown");
    const pendingOutbox = asPositiveNumber(discord.pending_outbox);
    const dueOutbox = asPositiveNumber(discord.due_outbox);
    const claimedOutbox = asPositiveNumber(discord.claimed_outbox);
    const deadLetters = asPositiveNumber(discord.dead_letters);

    if (discord.enabled !== true) {
      items.push("Enable the Discord connector from the dashboard before verification.");
    }
    if (discord.authenticated !== true) {
      items.push("Authenticate the Discord connector in the dashboard.");
    }
    if (asString(discord.readiness, "unknown") !== "ready") {
      items.push(`Resolve readiness state: ${asString(discord.readiness, "unknown")}.`);
    }
    if (asString(discord.liveness, "unknown") !== "running") {
      items.push(`Connector runtime is ${asString(discord.liveness, "unknown")}.`);
    }
    if (saturation !== "healthy" && saturation !== "unknown") {
      items.push(`Connector operations are ${saturation}.`);
    }
    if (discord.queue_paused === true) {
      const reason = asString(discord.pause_reason, "");
      items.push(
        reason.length > 0
          ? `Queue is paused: ${reason}. Resume or drain it from the dashboard recovery controls.`
          : "Queue is paused. Resume or drain it from the dashboard recovery controls."
      );
    }
    if (pendingOutbox !== null || dueOutbox !== null || claimedOutbox !== null) {
      items.push(
        `Queue depth: pending=${pendingOutbox ?? 0}, due=${dueOutbox ?? 0}, claimed=${claimedOutbox ?? 0}.`
      );
    }
    if (deadLetters !== null) {
      items.push(`Dead letters waiting for replay or discard: ${deadLetters}.`);
    }
    if (hasText(discord.auth_failure_hint)) {
      items.push(`Latest auth failure: ${discord.auth_failure_hint}.`);
    }
    if (hasText(discord.permission_gap_hint)) {
      items.push(`Discord permission gap: ${discord.permission_gap_hint}.`);
    }
    if (hasText(discord.health_refresh_status) && discord.health_refresh_status !== "unknown") {
      const warningCount = asPositiveNumber(discord.health_refresh_warning_count) ?? 0;
      let detail = `Health refresh ${discord.health_refresh_status}`;
      if (warningCount > 0) {
        detail += ` (${warningCount} warning${warningCount === 1 ? "" : "s"})`;
      }
      if (hasText(discord.health_refresh_detail)) {
        detail += `: ${discord.health_refresh_detail}`;
      } else {
        detail += ".";
      }
      items.push(detail);
    }
    if (items.length === 0) {
      items.push("Discord connector looks ready for a desktop verification test send.");
    }
    renderList(ui.discordChecklist, items, "Discord status will appear after the next snapshot.");
  }

  function buildDiscordReadinessSummary(discord) {
    const parts = [`${asString(discord.readiness, "unknown")} / ${asString(discord.liveness, "unknown")}`];
    const saturation = asString(discord.saturation_state, "unknown");
    if (saturation !== "healthy" && saturation !== "unknown") {
      parts.push(`ops=${saturation}`);
    }
    if (discord.queue_paused === true) {
      parts.push("queue paused");
    }
    if (asPositiveNumber(discord.dead_letters) !== null) {
      parts.push(`dead_letters=${discord.dead_letters}`);
    }
    return parts.join(" | ");
  }

  function buildDiscordLastErrorSummary(discord) {
    if (hasText(discord.last_error)) {
      return discord.last_error;
    }
    if (hasText(discord.auth_failure_hint)) {
      return `Auth hint: ${discord.auth_failure_hint}`;
    }
    if (hasText(discord.permission_gap_hint)) {
      return `Permission hint: ${discord.permission_gap_hint}`;
    }
    if (asPositiveNumber(discord.dead_letters) !== null) {
      return `${discord.dead_letters} dead letter(s) are waiting for operator recovery.`;
    }
    return "None";
  }

  function resolveDiscordBadgeStatus(discord) {
    if (discord.enabled !== true) {
      return "unknown";
    }
    if (
      discord.authenticated !== true ||
      hasText(discord.auth_failure_hint) ||
      hasText(discord.permission_gap_hint) ||
      discord.queue_paused === true
    ) {
      return "degraded";
    }
    const saturation = asString(discord.saturation_state, "unknown");
    if (saturation !== "healthy" && saturation !== "unknown") {
      return "degraded";
    }
    if (asPositiveNumber(discord.dead_letters) !== null) {
      return "degraded";
    }
    return "healthy";
  }

  function applyDiscordDefaultsFromOnboarding(status) {
    const defaults = status?.discord_defaults;
    if (!defaults || typeof defaults !== "object" || discordWizardState.formDirty) {
      return;
    }
    if (ui.discordTokenInput === document.activeElement) {
      return;
    }

    ui.discordFormAccountId.value = asString(defaults.account_id, "default");
    ui.discordFormMode.value = asString(defaults.mode, "local");
    ui.discordFormScope.value = asString(defaults.inbound_scope, "dm_only");
    ui.discordFormAllowFrom.value = Array.isArray(defaults.allow_from)
      ? defaults.allow_from.join(", ")
      : "";
    ui.discordFormDenyFrom.value = Array.isArray(defaults.deny_from)
      ? defaults.deny_from.join(", ")
      : "";
    ui.discordFormRequireMention.checked = defaults.require_mention !== false;
    ui.discordFormConcurrency.value = String(defaults.concurrency_limit ?? 2);
    ui.discordFormBroadcast.value = asString(defaults.broadcast_strategy, "deny");
    ui.discordFormConfirmOpen.checked = defaults.confirm_open_guild_channels === true;
    ui.discordFormVerifyChannelId.value = asString(defaults.verify_channel_id, "");
    if (!hasText(ui.discordVerifyTarget.value)) {
      ui.discordVerifyTarget.value = defaults.verify_channel_id
        ? `channel:${defaults.verify_channel_id}`
        : asString(defaults.last_verified_target, "");
    }
  }

  function collectDiscordPayload() {
    return {
      accountId: normalizeEmptyToNull(ui.discordFormAccountId.value),
      token: ui.discordTokenInput.value,
      mode: ui.discordFormMode.value,
      inboundScope: ui.discordFormScope.value,
      allowFrom: parseCommaSeparatedList(ui.discordFormAllowFrom.value),
      denyFrom: parseCommaSeparatedList(ui.discordFormDenyFrom.value),
      requireMention: ui.discordFormRequireMention.checked,
      concurrencyLimit: Number.parseInt(ui.discordFormConcurrency.value, 10) || 2,
      broadcastStrategy: ui.discordFormBroadcast.value,
      confirmOpenGuildChannels: ui.discordFormConfirmOpen.checked,
      verifyChannelId: normalizeEmptyToNull(ui.discordFormVerifyChannelId.value)
    };
  }

  function setDiscordWizardState(label, status, detail) {
    setLabeledStatus(ui.discordVerifyStatus, label, status);
    ui.discordActionDetail.textContent = detail;
  }

  function renderDiscordWizardWarnings(warnings) {
    renderList(
      ui.discordWizardWarnings,
      Array.isArray(warnings) ? warnings : [],
      "No Discord onboarding warnings yet."
    );
  }

  function appendLabeledResult(items, label, value) {
    if (value === null || value === undefined) {
      return;
    }
    if (Array.isArray(value)) {
      if (value.length === 0) {
        return;
      }
      items.push(`${label}: ${value.join(", ")}`);
      return;
    }
    const text = String(value).trim();
    if (text.length === 0) {
      return;
    }
    items.push(`${label}: ${text}`);
  }

  function renderDiscordResultCards(status = desktopState.lastOnboarding) {
    const preflight = discordWizardState.preflight;
    const apply = discordWizardState.apply;
    const verification = discordWizardState.verification;

    const preflightItems = [];
    if (preflight && typeof preflight === "object") {
      appendLabeledResult(preflightItems, "Connector", preflight.connector_id);
      appendLabeledResult(preflightItems, "Account", preflight.account_id);
      appendLabeledResult(
        preflightItems,
        "Bot",
        [preflight.bot_username, preflight.bot_id].filter(Boolean).join(" / ")
      );
      appendLabeledResult(
        preflightItems,
        "Inbound",
        preflight.inbound_alive === true ? "reachable" : "not yet reachable"
      );
      appendLabeledResult(preflightItems, "Invite", preflight.invite_url_template);
      appendLabeledResult(preflightItems, "Required permissions", preflight.required_permissions);
      appendLabeledResult(preflightItems, "Security defaults", preflight.security_defaults);
    }

    const preflightWarnings = [
      ...(Array.isArray(preflight?.warnings) ? preflight.warnings : []),
      ...(Array.isArray(preflight?.policy_warnings) ? preflight.policy_warnings : [])
    ];
    setLabeledStatus(
      ui.discordPreflightBadge,
      preflight ? (preflightWarnings.length > 0 ? "review" : "ready") : "waiting",
      preflight ? (preflightWarnings.length > 0 ? "degraded" : "healthy") : "unknown"
    );
    renderList(
      ui.discordPreflightResults,
      preflightItems,
      "Run Discord preflight to inspect bot identity, invite, and policy guidance."
    );

    const applyItems = [];
    if (apply && typeof apply === "object") {
      appendLabeledResult(applyItems, "Connector", apply.connector_id);
      appendLabeledResult(applyItems, "Config path", apply.config_path);
      appendLabeledResult(applyItems, "Config created", apply.config_created === true ? "yes" : "no");
      appendLabeledResult(
        applyItems,
        "Connector enabled",
        apply.connector_enabled === true ? "yes" : "no"
      );
      appendLabeledResult(
        applyItems,
        "Inbound",
        apply.inbound_alive === true ? "alive" : "not yet alive"
      );
      appendLabeledResult(
        applyItems,
        "Readiness / liveness",
        `${asString(apply.readiness, "unknown")} / ${asString(apply.liveness, "unknown")}`
      );
      appendLabeledResult(applyItems, "Token vault ref", apply.token_vault_ref);
    }
    if (status?.discord_verified) {
      appendLabeledResult(applyItems, "Last verified target", status.discord_last_verified_target);
      appendLabeledResult(
        applyItems,
        "Last verified at",
        formatUnixMs(status.discord_last_verified_at_unix_ms)
      );
    } else if (verification && typeof verification === "object") {
      appendLabeledResult(applyItems, "Latest verify target", verification.target);
      appendLabeledResult(applyItems, "Delivered", verification.delivered);
      appendLabeledResult(applyItems, "Message", verification.message);
    }

    const applyWarnings = [
      ...(Array.isArray(apply?.warnings) ? apply.warnings : []),
      ...(Array.isArray(apply?.policy_warnings) ? apply.policy_warnings : []),
      ...(Array.isArray(apply?.inbound_monitor_warnings) ? apply.inbound_monitor_warnings : [])
    ];
    const applyLabel = status?.discord_verified
      ? "verified"
      : apply
        ? applyWarnings.length > 0
          ? "applied"
          : "ready"
        : "waiting";
    const applyStatus = status?.discord_verified
      ? "healthy"
      : apply
        ? applyWarnings.length > 0
          ? "degraded"
          : "healthy"
        : "unknown";
    setLabeledStatus(ui.discordApplyBadge, applyLabel, applyStatus);
    renderList(
      ui.discordApplyResults,
      applyItems,
      "Apply the connector and run a test send to capture readiness details."
    );
  }

  async function runDiscordPreflight() {
    const token = ui.discordTokenInput.value.trim();
    if (token.length === 0) {
      setActionMessage("Discord bot token is required for preflight.", true);
      ui.discordTokenInput.focus();
      return;
    }

    ui.discordPreflightBtn.disabled = true;
    setDiscordWizardState(
      "running",
      "unknown",
      "Running Discord preflight against the local control plane."
    );
    try {
      const response = await invoke("run_discord_onboarding_preflight_command", {
        payload: collectDiscordPayload()
      });
      discordWizardState.preflight = response;
      discordWizardState.formDirty = false;
      const warnings = [
        ...(Array.isArray(response.warnings) ? response.warnings : []),
        ...(Array.isArray(response.policy_warnings) ? response.policy_warnings : [])
      ];
      renderDiscordWizardWarnings(warnings);
      setDiscordWizardState(
        "preflight ok",
        warnings.length > 0 ? "degraded" : "healthy",
        `Discord preflight OK for ${asString(response.bot_username, "bot")} (${asString(response.bot_id, "unknown id")}).`
      );
      renderDiscordResultCards();
      await refreshOnboardingStatus();
    } catch (error) {
      setDiscordWizardState(
        "preflight failed",
        "degraded",
        `Discord preflight failed: ${String(error)}`
      );
      renderDiscordWizardWarnings([`Discord preflight failed: ${String(error)}`]);
      setActionMessage(`Discord preflight failed: ${String(error)}`, true);
    } finally {
      ui.discordPreflightBtn.disabled = false;
    }
  }

  async function applyDiscordOnboardingFlow() {
    const token = ui.discordTokenInput.value.trim();
    if (token.length === 0) {
      setActionMessage("Discord bot token is required to apply onboarding.", true);
      ui.discordTokenInput.focus();
      return;
    }

    ui.discordApplyBtn.disabled = true;
    setDiscordWizardState(
      "applying",
      "unknown",
      "Applying Discord connector config to the local install."
    );
    try {
      const response = await invoke("apply_discord_onboarding_command", {
        payload: collectDiscordPayload()
      });
      discordWizardState.apply = response;
      discordWizardState.formDirty = false;
      const warnings = [
        ...(Array.isArray(response.warnings) ? response.warnings : []),
        ...(Array.isArray(response.policy_warnings) ? response.policy_warnings : []),
        ...(Array.isArray(response.inbound_monitor_warnings)
          ? response.inbound_monitor_warnings
          : [])
      ];
      renderDiscordWizardWarnings(warnings);
      setDiscordWizardState(
        "applied",
        warnings.length > 0 ? "degraded" : "healthy",
        `Discord connector ${asString(response.connector_id, "discord:default")} applied.`
      );
      ui.discordTokenInput.value = "";
      renderDiscordResultCards();
      await refreshAllData({ preserveMessage: true });
      setActionMessage(
        `Discord onboarding applied for ${asString(response.connector_id, "discord:default")}.`
      );
    } catch (error) {
      setDiscordWizardState(
        "apply failed",
        "degraded",
        `Discord apply failed: ${String(error)}`
      );
      renderDiscordWizardWarnings([`Discord apply failed: ${String(error)}`]);
      setActionMessage(`Discord apply failed: ${String(error)}`, true);
    } finally {
      ui.discordApplyBtn.disabled = false;
    }
  }

  async function runDiscordVerification() {
    const connectorId = normalizeDiscordConnectorId(ui.discordFormAccountId.value);
    const target = ui.discordVerifyTarget.value.trim();
    if (target.length === 0) {
      setActionMessage("Discord verification target is required.", true);
      ui.discordVerifyTarget.focus();
      return;
    }

    ui.discordVerifyBtn.disabled = true;
    setDiscordWizardState("verifying", "unknown", "Sending Discord verification message.");
    try {
      const response = await invoke("verify_discord_connector_command", {
        payload: {
          connectorId,
          target,
          text: normalizeEmptyToNull(ui.discordVerifyText.value)
        }
      });
      discordWizardState.verification = response;
      renderDiscordWizardWarnings([]);
      setDiscordWizardState(
        "verified",
        "healthy",
        asString(response.message, "Discord verification dispatched.")
      );
      renderDiscordResultCards();
      await refreshAllData({ preserveMessage: true });
      setActionMessage(asString(response.message, "Discord verification dispatched."));
    } catch (error) {
      setDiscordWizardState(
        "verify failed",
        "degraded",
        `Discord verification failed: ${String(error)}`
      );
      renderDiscordWizardWarnings([`Discord verification failed: ${String(error)}`]);
      setActionMessage(`Discord verification failed: ${String(error)}`, true);
    } finally {
      ui.discordVerifyBtn.disabled = false;
    }
  }

  function bindDiscordInputs() {
    ui.discordPreflightBtn.addEventListener("click", runDiscordPreflight);
    ui.discordApplyBtn.addEventListener("click", applyDiscordOnboardingFlow);
    ui.discordVerifyBtn.addEventListener("click", runDiscordVerification);
    for (const field of [
      ui.discordFormAccountId,
      ui.discordFormMode,
      ui.discordFormScope,
      ui.discordFormVerifyChannelId,
      ui.discordFormConcurrency,
      ui.discordFormBroadcast,
      ui.discordFormAllowFrom,
      ui.discordFormDenyFrom,
      ui.discordVerifyTarget,
      ui.discordVerifyText
    ]) {
      field.addEventListener("input", () => {
        discordWizardState.formDirty = true;
      });
      field.addEventListener("change", () => {
        discordWizardState.formDirty = true;
      });
    }
    ui.discordFormRequireMention.addEventListener("change", () => {
      discordWizardState.formDirty = true;
    });
    ui.discordFormConfirmOpen.addEventListener("change", () => {
      discordWizardState.formDirty = true;
    });
  }

  return {
    renderDiscordChecklist,
    buildDiscordReadinessSummary,
    buildDiscordLastErrorSummary,
    resolveDiscordBadgeStatus,
    applyDiscordDefaultsFromOnboarding,
    setDiscordWizardState,
    renderDiscordResultCards,
    bindDiscordInputs
  };
}

function hasText(value) {
  return typeof value === "string" && value.trim().length > 0;
}
