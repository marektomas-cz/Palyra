import { DiscordOnboardingHighlights, toPrettyJson } from "../../../../../console/shared";
import type { ConsoleAppState } from "../../../../../console/useConsoleAppState";

type DiscordOnboardingPanelProps = {
  app: ConsoleAppState;
};

export function DiscordOnboardingPanel({
  app,
}: DiscordOnboardingPanelProps) {
  return (
    <section className="console-subpanel">
      <div className="console-subpanel__header">
        <div>
          <h3>Discord onboarding wizard</h3>
          <p className="chat-muted">
            Probe, apply, and verify the live Discord connector contract without
            falling back to manual config edits.
          </p>
        </div>
      </div>
      <div className="console-grid-4">
        <label>
          Account ID
          <input
            value={app.discordWizardAccountId}
            onChange={(event) =>
              app.setDiscordWizardAccountId(event.target.value)
            }
          />
        </label>
        <label>
          Mode
          <select
            value={app.discordWizardMode}
            onChange={(event) =>
              app.setDiscordWizardMode(
                event.target.value === "remote_vps" ? "remote_vps" : "local"
              )
            }
          >
            <option value="local">local</option>
            <option value="remote_vps">remote_vps</option>
          </select>
        </label>
        <label>
          Bot token
          <input
            value={app.discordWizardToken}
            onChange={(event) => app.setDiscordWizardToken(event.target.value)}
            placeholder="Never persisted in config plaintext"
          />
        </label>
        <label>
          Verify channel ID
          <input
            value={app.discordWizardVerifyChannelId}
            onChange={(event) =>
              app.setDiscordWizardVerifyChannelId(event.target.value)
            }
          />
        </label>
      </div>
      <div className="console-grid-4">
        <label>
          Inbound scope
          <select
            value={app.discordWizardScope}
            onChange={(event) =>
              app.setDiscordWizardScope(
                event.target.value as
                  | "dm_only"
                  | "allowlisted_guild_channels"
                  | "open_guild_channels"
              )
            }
          >
            <option value="dm_only">dm_only</option>
            <option value="allowlisted_guild_channels">
              allowlisted_guild_channels
            </option>
            <option value="open_guild_channels">open_guild_channels</option>
          </select>
        </label>
        <label>
          Allow from
          <input
            value={app.discordWizardAllowFrom}
            onChange={(event) => app.setDiscordWizardAllowFrom(event.target.value)}
          />
        </label>
        <label>
          Deny from
          <input
            value={app.discordWizardDenyFrom}
            onChange={(event) => app.setDiscordWizardDenyFrom(event.target.value)}
          />
        </label>
        <label>
          Concurrency
          <input
            value={app.discordWizardConcurrency}
            onChange={(event) =>
              app.setDiscordWizardConcurrency(event.target.value)
            }
          />
        </label>
      </div>
      <div className="console-inline-actions">
        <label className="console-checkbox-inline">
          <input
            type="checkbox"
            checked={app.discordWizardRequireMention}
            onChange={(event) =>
              app.setDiscordWizardRequireMention(event.target.checked)
            }
          />
          Require mention
        </label>
        <label>
          Broadcast strategy
          <select
            value={app.discordWizardBroadcast}
            onChange={(event) =>
              app.setDiscordWizardBroadcast(
                event.target.value as "deny" | "mention_only" | "allow"
              )
            }
          >
            <option value="deny">deny</option>
            <option value="mention_only">mention_only</option>
            <option value="allow">allow</option>
          </select>
        </label>
        <button
          type="button"
          onClick={() => void app.runDiscordPreflight()}
          disabled={app.discordWizardBusy}
        >
          {app.discordWizardBusy ? "Running..." : "Run preflight"}
        </button>
        <button
          type="button"
          onClick={() => void app.applyDiscordOnboarding()}
          disabled={app.discordWizardBusy}
        >
          {app.discordWizardBusy ? "Applying..." : "Apply onboarding"}
        </button>
      </div>
      {app.discordWizardPreflight !== null && (
        <DiscordOnboardingHighlights
          title="Preflight highlights"
          payload={app.discordWizardPreflight}
        />
      )}
      {app.discordWizardPreflight !== null && (
        <pre>
          {toPrettyJson(app.discordWizardPreflight, app.revealSensitiveValues)}
        </pre>
      )}
      {app.discordWizardApply !== null && (
        <pre>{toPrettyJson(app.discordWizardApply, app.revealSensitiveValues)}</pre>
      )}
    </section>
  );
}
