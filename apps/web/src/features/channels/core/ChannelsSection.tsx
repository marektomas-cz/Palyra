import { ConsoleSectionHeader } from "../../../console/components/ConsoleSectionHeader";
import {
  channelConnectorAvailability,
  readBool,
  readObject,
  readString,
  toPrettyJson,
  type JsonObject,
} from "../../../console/shared";
import type { ConsoleAppState } from "../../../console/useConsoleAppState";
import { DiscordConnectorActionsPanel } from "../connectors/discord/components/DiscordConnectorActionsPanel";
import { DiscordOnboardingPanel } from "../connectors/discord/components/DiscordOnboardingPanel";

function displayScalar(value: unknown, fallback = "n/a"): string {
  if (typeof value === "string") {
    return value.trim().length > 0 ? value : fallback;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return fallback;
}

export function ChannelsSection({ app }: { app: ConsoleAppState }) {
  const selectedStatusPayload: JsonObject = app.channelsSelectedStatus ?? {};
  const selectedConnector =
    readObject(selectedStatusPayload, "connector") ?? selectedStatusPayload;
  const selectedOperations = readObject(selectedStatusPayload, "operations");
  const selectedQueue =
    selectedOperations !== null ? readObject(selectedOperations, "queue") : null;
  const selectedSaturation =
    selectedOperations !== null
      ? readObject(selectedOperations, "saturation")
      : null;
  const selectedDiscordOps =
    selectedOperations !== null ? readObject(selectedOperations, "discord") : null;
  const selectedHealthRefresh = readObject(selectedStatusPayload, "health_refresh");
  const selectedConnectorKind = readString(selectedConnector, "kind");

  return (
    <main className="console-card">
      <ConsoleSectionHeader
        title="Channels and Router"
        description="Operate Discord onboarding, connector health, router previews, pairing codes, and delivery diagnostics from the canonical dashboard surface."
        actions={
          <button
            type="button"
            onClick={() => void app.refreshChannels()}
            disabled={app.channelsBusy}
          >
            {app.channelsBusy ? "Refreshing..." : "Refresh channels"}
          </button>
        }
      />

      <DiscordOnboardingPanel app={app} />

      <div className="console-table-wrap">
        <table className="console-table">
          <thead>
            <tr>
              <th>Connector ID</th>
              <th>Kind</th>
              <th>Availability</th>
              <th>Enabled</th>
              <th>Readiness</th>
              <th>Liveness</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody>
            {app.channelsConnectors.length === 0 && (
              <tr>
                <td colSpan={7}>No channel connectors configured.</td>
              </tr>
            )}
            {app.channelsConnectors.map((connector) => {
              const connectorId = readString(connector, "connector_id") ?? "(missing)";
              const enabled = readBool(connector, "enabled");
              return (
                <tr key={connectorId}>
                  <td>{connectorId}</td>
                  <td>{readString(connector, "kind") ?? "-"}</td>
                  <td>{channelConnectorAvailability(connector)}</td>
                  <td>{enabled ? "yes" : "no"}</td>
                  <td>{readString(connector, "readiness") ?? "-"}</td>
                  <td>{readString(connector, "liveness") ?? "-"}</td>
                  <td className="console-action-cell">
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => void app.selectChannelConnector(connectorId)}
                    >
                      Select
                    </button>
                    <button
                      type="button"
                      onClick={() => void app.toggleConnector(connector, !enabled)}
                    >
                      {enabled ? "Disable" : "Enable"}
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <section className="console-grid-2">
        <article className="console-subpanel">
          <h3>Selected connector status</h3>
          {app.channelsSelectedStatus === null ? (
            <p>Select a connector to inspect status and routing.</p>
          ) : (
            <pre>
              {toPrettyJson(
                app.channelsSelectedStatus,
                app.revealSensitiveValues
              )}
            </pre>
          )}
        </article>
        <article className="console-subpanel">
          <div className="console-subpanel__header">
            <div>
              <h3>Recovery controls</h3>
              <p className="chat-muted">
                Queue pause, forced drain, health refresh, and dead-letter replay
                stay colocated with live connector telemetry.
              </p>
            </div>
          </div>
          <div className="console-grid-3">
            <button
              type="button"
              className="secondary"
              onClick={() => void app.pauseChannelQueue()}
              disabled={app.channelsBusy}
            >
              {app.channelsBusy ? "Working..." : "Pause queue"}
            </button>
            <button
              type="button"
              className="secondary"
              onClick={() => void app.resumeChannelQueue()}
              disabled={app.channelsBusy}
            >
              {app.channelsBusy ? "Working..." : "Resume queue"}
            </button>
            <button
              type="button"
              className="secondary"
              onClick={() => void app.drainChannelQueue()}
              disabled={app.channelsBusy}
            >
              {app.channelsBusy ? "Working..." : "Force drain queue"}
            </button>
          </div>
          <div className="console-grid-2">
            <label>
              Health refresh verify channel
              <input
                value={app.discordWizardVerifyChannelId}
                onChange={(event) =>
                  app.setDiscordWizardVerifyChannelId(event.target.value)
                }
              />
            </label>
            <div className="console-inline-actions">
              <button
                type="button"
                onClick={() => void app.refreshChannelHealth()}
                disabled={app.channelsBusy}
              >
                {app.channelsBusy ? "Refreshing..." : "Run health refresh"}
              </button>
            </div>
          </div>
          {selectedQueue === null && selectedHealthRefresh === null ? (
            <p>No recovery telemetry loaded yet.</p>
          ) : (
            <>
              {selectedQueue !== null && (
                <ul className="console-compact-list">
                  <li>
                    Queue paused: {readBool(selectedQueue, "paused") ? "yes" : "no"}
                  </li>
                  <li>
                    Pause reason:{" "}
                    {readString(selectedQueue, "pause_reason") ?? "n/a"}
                  </li>
                  <li>
                    Pending / due / claimed:{" "}
                    {displayScalar(selectedQueue.pending_outbox, "0")} /{" "}
                    {displayScalar(selectedQueue.due_outbox, "0")} /{" "}
                    {displayScalar(selectedQueue.claimed_outbox, "0")}
                  </li>
                  <li>Dead letters: {displayScalar(selectedQueue.dead_letters, "0")}</li>
                  <li>
                    Saturation: {readString(selectedSaturation ?? {}, "state") ?? "n/a"}
                  </li>
                  <li>
                    Auth failure:{" "}
                    {readString(selectedOperations ?? {}, "last_auth_failure") ?? "none"}
                  </li>
                  <li>
                    Permission gap:{" "}
                    {readString(selectedDiscordOps ?? {}, "last_permission_failure") ??
                      "none"}
                  </li>
                </ul>
              )}
              {selectedHealthRefresh !== null && (
                <pre>
                  {toPrettyJson(selectedHealthRefresh, app.revealSensitiveValues)}
                </pre>
              )}
            </>
          )}
        </article>
        <article className="console-subpanel">
          <div className="console-subpanel__header">
            <div>
              <h3>Connector logs and dead letters</h3>
              <p className="chat-muted">
                Delivery diagnostics stay on the dashboard so operators can inspect
                failures without switching surfaces.
              </p>
            </div>
          </div>
          <div className="console-grid-2">
            <label>
              Selected connector
              <input value={app.channelsSelectedConnectorId} readOnly />
            </label>
            <label>
              Log limit
              <input
                value={app.channelsLogsLimit}
                onChange={(event) => app.setChannelsLogsLimit(event.target.value)}
              />
            </label>
          </div>
          {app.channelsEvents.length === 0 && app.channelsDeadLetters.length === 0 ? (
            <p>No connector logs loaded.</p>
          ) : (
            <>
              {app.channelsDeadLetters.length > 0 && (
                <div className="console-inline-actions">
                  {app.channelsDeadLetters.map((deadLetter) => {
                    const deadLetterId =
                      typeof deadLetter.dead_letter_id === "number"
                        ? deadLetter.dead_letter_id
                        : Number(deadLetter.dead_letter_id ?? Number.NaN);
                    if (!Number.isFinite(deadLetterId)) {
                      return null;
                    }
                    return (
                      <div key={deadLetterId} className="console-inline-actions">
                        <span>Dead letter {deadLetterId}</span>
                        <button
                          type="button"
                          className="secondary"
                          onClick={() =>
                            void app.replayChannelDeadLetter(deadLetterId)
                          }
                          disabled={app.channelsBusy}
                        >
                          Replay
                        </button>
                        <button
                          type="button"
                          className="secondary"
                          onClick={() =>
                            void app.discardChannelDeadLetter(deadLetterId)
                          }
                          disabled={app.channelsBusy}
                        >
                          Discard
                        </button>
                      </div>
                    );
                  })}
                </div>
              )}
              <pre>
                {toPrettyJson(
                  {
                    events: app.channelsEvents,
                    dead_letters: app.channelsDeadLetters,
                  },
                  app.revealSensitiveValues
                )}
              </pre>
            </>
          )}
        </article>
      </section>

      <section className="console-grid-2">
        <article className="console-subpanel">
          <div className="console-subpanel__header">
            <div>
              <h3>Router rules and warnings</h3>
              <p className="chat-muted">
                Preview route acceptance, inspect current config hash, and keep
                warning output visible before enabling broader message ingress.
              </p>
            </div>
          </div>
          <p>
            <strong>Config hash:</strong> {app.channelRouterConfigHash || "n/a"}
          </p>
          {app.channelRouterWarnings.length === 0 ? (
            <p>No router warnings published.</p>
          ) : (
            <ul className="console-compact-list">
              {app.channelRouterWarnings.map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
          )}
          {app.channelRouterRules === null ? (
            <p>No router rules loaded.</p>
          ) : (
            <pre>
              {toPrettyJson(app.channelRouterRules, app.revealSensitiveValues)}
            </pre>
          )}
        </article>
        <article className="console-subpanel">
          <h3>Route preview</h3>
          <form
            className="console-form"
            onSubmit={(event) => void app.previewChannelRouter(event)}
          >
            <div className="console-grid-3">
              <label>
                Channel
                <input
                  value={app.channelRouterPreviewChannel}
                  onChange={(event) =>
                    app.setChannelRouterPreviewChannel(event.target.value)
                  }
                />
              </label>
              <label>
                Text
                <input
                  value={app.channelRouterPreviewText}
                  onChange={(event) =>
                    app.setChannelRouterPreviewText(event.target.value)
                  }
                />
              </label>
              <label>
                Conversation ID
                <input
                  value={app.channelRouterPreviewConversationId}
                  onChange={(event) =>
                    app.setChannelRouterPreviewConversationId(event.target.value)
                  }
                />
              </label>
            </div>
            <div className="console-grid-4">
              <label>
                Sender identity
                <input
                  value={app.channelRouterPreviewSenderIdentity}
                  onChange={(event) =>
                    app.setChannelRouterPreviewSenderIdentity(event.target.value)
                  }
                />
              </label>
              <label>
                Sender display
                <input
                  value={app.channelRouterPreviewSenderDisplay}
                  onChange={(event) =>
                    app.setChannelRouterPreviewSenderDisplay(event.target.value)
                  }
                />
              </label>
              <label>
                Max payload bytes
                <input
                  value={app.channelRouterPreviewMaxPayloadBytes}
                  onChange={(event) =>
                    app.setChannelRouterPreviewMaxPayloadBytes(event.target.value)
                  }
                />
              </label>
              <div className="console-inline-actions">
                <label className="console-checkbox-inline">
                  <input
                    type="checkbox"
                    checked={app.channelRouterPreviewSenderVerified}
                    onChange={(event) =>
                      app.setChannelRouterPreviewSenderVerified(event.target.checked)
                    }
                  />
                  Sender verified
                </label>
                <label className="console-checkbox-inline">
                  <input
                    type="checkbox"
                    checked={app.channelRouterPreviewIsDirectMessage}
                    onChange={(event) =>
                      app.setChannelRouterPreviewIsDirectMessage(event.target.checked)
                    }
                  />
                  Direct message
                </label>
                <label className="console-checkbox-inline">
                  <input
                    type="checkbox"
                    checked={app.channelRouterPreviewRequestedBroadcast}
                    onChange={(event) =>
                      app.setChannelRouterPreviewRequestedBroadcast(
                        event.target.checked
                      )
                    }
                  />
                  Requested broadcast
                </label>
              </div>
            </div>
            <button type="submit" disabled={app.channelsBusy}>
              {app.channelsBusy ? "Previewing..." : "Preview route"}
            </button>
          </form>
          {app.channelRouterPreviewResult === null ? (
            <p>No route preview computed.</p>
          ) : (
            <pre>
              {toPrettyJson(
                app.channelRouterPreviewResult,
                app.revealSensitiveValues
              )}
            </pre>
          )}
        </article>
      </section>

      <section className="console-grid-2">
        <article className="console-subpanel">
          <h3>Router pairing codes</h3>
          <form
            className="console-form"
            onSubmit={(event) => void app.mintChannelRouterPairingCode(event)}
          >
            <div className="console-grid-3">
              <label>
                Filter channel
                <input
                  value={app.channelRouterPairingsFilterChannel}
                  onChange={(event) =>
                    app.setChannelRouterPairingsFilterChannel(event.target.value)
                  }
                />
              </label>
              <label>
                Mint channel
                <input
                  value={app.channelRouterMintChannel}
                  onChange={(event) =>
                    app.setChannelRouterMintChannel(event.target.value)
                  }
                />
              </label>
              <label>
                Issued by
                <input
                  value={app.channelRouterMintIssuedBy}
                  onChange={(event) =>
                    app.setChannelRouterMintIssuedBy(event.target.value)
                  }
                />
              </label>
            </div>
            <div className="console-grid-2">
              <label>
                TTL ms
                <input
                  value={app.channelRouterMintTtlMs}
                  onChange={(event) =>
                    app.setChannelRouterMintTtlMs(event.target.value)
                  }
                />
              </label>
              <div className="console-inline-actions">
                <button
                  type="button"
                  className="secondary"
                  onClick={() => void app.refreshChannelRouterPairings()}
                  disabled={app.channelsBusy}
                >
                  Refresh pairings
                </button>
                <button type="submit" disabled={app.channelsBusy}>
                  {app.channelsBusy ? "Minting..." : "Mint pairing code"}
                </button>
              </div>
            </div>
          </form>
          {app.channelRouterMintResult !== null && (
            <pre>
              {toPrettyJson(app.channelRouterMintResult, app.revealSensitiveValues)}
            </pre>
          )}
          {app.channelRouterPairings.length === 0 ? (
            <p>No pairings loaded.</p>
          ) : (
            <pre>
              {toPrettyJson(app.channelRouterPairings, app.revealSensitiveValues)}
            </pre>
          )}
        </article>

        <article className="console-subpanel">
          <h3>Connector test send</h3>
          <form
            className="console-form"
            onSubmit={(event) => void app.sendChannelTest(event)}
          >
            <div className="console-grid-4">
              <label>
                Text
                <input
                  value={app.channelsTestText}
                  onChange={(event) => app.setChannelsTestText(event.target.value)}
                />
              </label>
              <label>
                Conversation ID
                <input
                  value={app.channelsTestConversationId}
                  onChange={(event) =>
                    app.setChannelsTestConversationId(event.target.value)
                  }
                />
              </label>
              <label>
                Sender ID
                <input
                  value={app.channelsTestSenderId}
                  onChange={(event) =>
                    app.setChannelsTestSenderId(event.target.value)
                  }
                />
              </label>
              <label>
                Sender display
                <input
                  value={app.channelsTestSenderDisplay}
                  onChange={(event) =>
                    app.setChannelsTestSenderDisplay(event.target.value)
                  }
                />
              </label>
            </div>
            <div className="console-inline-actions">
              <label className="console-checkbox-inline">
                <input
                  type="checkbox"
                  checked={app.channelsTestCrashOnce}
                  onChange={(event) =>
                    app.setChannelsTestCrashOnce(event.target.checked)
                  }
                />
                Simulate crash once
              </label>
              <label className="console-checkbox-inline">
                <input
                  type="checkbox"
                  checked={app.channelsTestDirectMessage}
                  onChange={(event) =>
                    app.setChannelsTestDirectMessage(event.target.checked)
                  }
                />
                Direct message
              </label>
              <label className="console-checkbox-inline">
                <input
                  type="checkbox"
                  checked={app.channelsTestBroadcast}
                  onChange={(event) =>
                    app.setChannelsTestBroadcast(event.target.checked)
                  }
                />
                Broadcast
              </label>
              <button type="submit" disabled={app.channelsBusy}>
                {app.channelsBusy ? "Sending..." : "Send connector test"}
              </button>
            </div>
          </form>

          <DiscordConnectorActionsPanel
            app={app}
            selectedConnectorKind={selectedConnectorKind}
          />
        </article>
      </section>
    </main>
  );
}
