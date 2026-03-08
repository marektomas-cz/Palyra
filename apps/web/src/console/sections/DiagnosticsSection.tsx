import { ConsoleSectionHeader } from "../components/ConsoleSectionHeader";
import { PrettyJsonBlock, type JsonObject } from "../shared";

type DiagnosticsSectionProps = {
  app: {
    diagnosticsBusy: boolean;
    diagnosticsSnapshot: JsonObject | null;
    revealSensitiveValues: boolean;
    refreshDiagnostics: () => Promise<void>;
  };
};

export function DiagnosticsSection({ app }: DiagnosticsSectionProps) {
  return (
    <main className="console-card">
      <ConsoleSectionHeader
        title="Diagnostics"
        actions={(
          <button type="button" onClick={() => void app.refreshDiagnostics()} disabled={app.diagnosticsBusy}>
            {app.diagnosticsBusy ? "Refreshing..." : "Refresh"}
          </button>
        )}
      />
      {app.diagnosticsSnapshot === null ? (
        <p>No diagnostics loaded.</p>
      ) : (
        <>
          <section className="console-subpanel">
            <h3>Model Provider + Rate Limits</h3>
            <PrettyJsonBlock
              value={{
                model_provider: app.diagnosticsSnapshot["model_provider"] ?? null,
                rate_limits: app.diagnosticsSnapshot["rate_limits"] ?? null
              }}
              revealSensitiveValues={app.revealSensitiveValues}
            />
          </section>
          <section className="console-subpanel">
            <h3>Auth Profile Health</h3>
            <PrettyJsonBlock
              value={app.diagnosticsSnapshot["auth_profiles"] ?? null}
              revealSensitiveValues={app.revealSensitiveValues}
            />
          </section>
          <section className="console-subpanel">
            <h3>Browserd Status</h3>
            <PrettyJsonBlock
              value={app.diagnosticsSnapshot["browserd"] ?? null}
              revealSensitiveValues={app.revealSensitiveValues}
            />
          </section>
          <section className="console-subpanel">
            <h3>Media Pipeline</h3>
            <PrettyJsonBlock
              value={app.diagnosticsSnapshot["media"] ?? null}
              revealSensitiveValues={app.revealSensitiveValues}
            />
          </section>
        </>
      )}
    </main>
  );
}
