import { useState } from "react";

import type { JsonObject } from "../../../../console/shared";

const DEFAULT_DISCORD_TARGET = "channel:";
const DEFAULT_DISCORD_TEST_TEXT = "palyra discord test message";
const DEFAULT_DISCORD_ACCOUNT_ID = "default";
const DEFAULT_DISCORD_CONCURRENCY = "2";

export function useDiscordChannelState() {
  const [channelsDiscordTarget, setChannelsDiscordTarget] =
    useState(DEFAULT_DISCORD_TARGET);
  const [channelsDiscordText, setChannelsDiscordText] = useState(
    DEFAULT_DISCORD_TEST_TEXT
  );
  const [channelsDiscordAutoReaction, setChannelsDiscordAutoReaction] = useState("");
  const [channelsDiscordThreadId, setChannelsDiscordThreadId] = useState("");
  const [channelsDiscordConfirm, setChannelsDiscordConfirm] = useState(false);
  const [discordWizardBusy, setDiscordWizardBusy] = useState(false);
  const [discordWizardAccountId, setDiscordWizardAccountId] = useState(
    DEFAULT_DISCORD_ACCOUNT_ID
  );
  const [discordWizardMode, setDiscordWizardMode] = useState<"local" | "remote_vps">(
    "local"
  );
  const [discordWizardToken, setDiscordWizardToken] = useState("");
  const [discordWizardScope, setDiscordWizardScope] = useState<
    "dm_only" | "allowlisted_guild_channels" | "open_guild_channels"
  >("dm_only");
  const [discordWizardAllowFrom, setDiscordWizardAllowFrom] = useState("");
  const [discordWizardDenyFrom, setDiscordWizardDenyFrom] = useState("");
  const [discordWizardRequireMention, setDiscordWizardRequireMention] = useState(true);
  const [discordWizardBroadcast, setDiscordWizardBroadcast] = useState<
    "deny" | "mention_only" | "allow"
  >("deny");
  const [discordWizardConcurrency, setDiscordWizardConcurrency] = useState(
    DEFAULT_DISCORD_CONCURRENCY
  );
  const [discordWizardConfirmOpen, setDiscordWizardConfirmOpen] = useState(false);
  const [discordWizardVerifyChannelId, setDiscordWizardVerifyChannelId] = useState("");
  const [discordWizardPreflight, setDiscordWizardPreflight] =
    useState<JsonObject | null>(null);
  const [discordWizardApply, setDiscordWizardApply] = useState<JsonObject | null>(null);
  const [discordWizardVerifyTarget, setDiscordWizardVerifyTarget] =
    useState(DEFAULT_DISCORD_TARGET);
  const [discordWizardVerifyText, setDiscordWizardVerifyText] = useState(
    DEFAULT_DISCORD_TEST_TEXT
  );
  const [discordWizardVerifyConfirm, setDiscordWizardVerifyConfirm] = useState(false);

  function resetDiscordChannelState(): void {
    setChannelsDiscordTarget(DEFAULT_DISCORD_TARGET);
    setChannelsDiscordText(DEFAULT_DISCORD_TEST_TEXT);
    setChannelsDiscordAutoReaction("");
    setChannelsDiscordThreadId("");
    setChannelsDiscordConfirm(false);
    setDiscordWizardBusy(false);
    setDiscordWizardAccountId(DEFAULT_DISCORD_ACCOUNT_ID);
    setDiscordWizardMode("local");
    setDiscordWizardToken("");
    setDiscordWizardScope("dm_only");
    setDiscordWizardAllowFrom("");
    setDiscordWizardDenyFrom("");
    setDiscordWizardRequireMention(true);
    setDiscordWizardBroadcast("deny");
    setDiscordWizardConcurrency(DEFAULT_DISCORD_CONCURRENCY);
    setDiscordWizardConfirmOpen(false);
    setDiscordWizardVerifyChannelId("");
    setDiscordWizardPreflight(null);
    setDiscordWizardApply(null);
    setDiscordWizardVerifyTarget(DEFAULT_DISCORD_TARGET);
    setDiscordWizardVerifyText(DEFAULT_DISCORD_TEST_TEXT);
    setDiscordWizardVerifyConfirm(false);
  }

  return {
    channelsDiscordTarget,
    setChannelsDiscordTarget,
    channelsDiscordText,
    setChannelsDiscordText,
    channelsDiscordAutoReaction,
    setChannelsDiscordAutoReaction,
    channelsDiscordThreadId,
    setChannelsDiscordThreadId,
    channelsDiscordConfirm,
    setChannelsDiscordConfirm,
    discordWizardBusy,
    setDiscordWizardBusy,
    discordWizardAccountId,
    setDiscordWizardAccountId,
    discordWizardMode,
    setDiscordWizardMode,
    discordWizardToken,
    setDiscordWizardToken,
    discordWizardScope,
    setDiscordWizardScope,
    discordWizardAllowFrom,
    setDiscordWizardAllowFrom,
    discordWizardDenyFrom,
    setDiscordWizardDenyFrom,
    discordWizardRequireMention,
    setDiscordWizardRequireMention,
    discordWizardBroadcast,
    setDiscordWizardBroadcast,
    discordWizardConcurrency,
    setDiscordWizardConcurrency,
    discordWizardConfirmOpen,
    setDiscordWizardConfirmOpen,
    discordWizardVerifyChannelId,
    setDiscordWizardVerifyChannelId,
    discordWizardPreflight,
    setDiscordWizardPreflight,
    discordWizardApply,
    setDiscordWizardApply,
    discordWizardVerifyTarget,
    setDiscordWizardVerifyTarget,
    discordWizardVerifyText,
    setDiscordWizardVerifyText,
    discordWizardVerifyConfirm,
    setDiscordWizardVerifyConfirm,
    resetDiscordChannelState,
  };
}
