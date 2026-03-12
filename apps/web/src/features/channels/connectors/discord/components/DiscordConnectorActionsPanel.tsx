import type { ConsoleAppState } from "../../../../../console/useConsoleAppState";

type DiscordConnectorActionsPanelProps = {
  app: ConsoleAppState;
  selectedConnectorKind: string | null;
};

export function DiscordConnectorActionsPanel({
  app,
  selectedConnectorKind,
}: DiscordConnectorActionsPanelProps) {
  return (
    <>
      {selectedConnectorKind === "discord" && (
        <>
          <h4>Discord direct verification</h4>
          <form
            className="console-form"
            onSubmit={(event) => void app.sendDiscordTest(event)}
          >
            <div className="console-grid-4">
              <label>
                Target
                <input
                  value={app.channelsDiscordTarget}
                  onChange={(event) =>
                    app.setChannelsDiscordTarget(event.target.value)
                  }
                />
              </label>
              <label>
                Text
                <input
                  value={app.channelsDiscordText}
                  onChange={(event) => app.setChannelsDiscordText(event.target.value)}
                />
              </label>
              <label>
                Auto reaction
                <input
                  value={app.channelsDiscordAutoReaction}
                  onChange={(event) =>
                    app.setChannelsDiscordAutoReaction(event.target.value)
                  }
                />
              </label>
              <label>
                Thread ID
                <input
                  value={app.channelsDiscordThreadId}
                  onChange={(event) =>
                    app.setChannelsDiscordThreadId(event.target.value)
                  }
                />
              </label>
            </div>
            <label className="console-checkbox-inline">
              <input
                type="checkbox"
                checked={app.channelsDiscordConfirm}
                onChange={(event) =>
                  app.setChannelsDiscordConfirm(event.target.checked)
                }
              />
              Confirm Discord outbound test send
            </label>
            <button type="submit" disabled={app.channelsBusy}>
              {app.channelsBusy ? "Sending..." : "Send Discord test"}
            </button>
          </form>
        </>
      )}

      <h4>Discord verify target</h4>
      <div className="console-grid-3">
        <label>
          Target
          <input
            value={app.discordWizardVerifyTarget}
            onChange={(event) =>
              app.setDiscordWizardVerifyTarget(event.target.value)
            }
          />
        </label>
        <label>
          Text
          <input
            value={app.discordWizardVerifyText}
            onChange={(event) => app.setDiscordWizardVerifyText(event.target.value)}
          />
        </label>
        <label className="console-checkbox-inline">
          <input
            type="checkbox"
            checked={app.discordWizardVerifyConfirm}
            onChange={(event) =>
              app.setDiscordWizardVerifyConfirm(event.target.checked)
            }
          />
          Confirm verification send
        </label>
      </div>
      <button
        type="button"
        onClick={() => void app.runDiscordVerification()}
        disabled={app.channelsBusy}
      >
        {app.channelsBusy ? "Verifying..." : "Verify Discord target"}
      </button>
    </>
  );
}
