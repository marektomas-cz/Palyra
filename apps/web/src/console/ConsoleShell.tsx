import { Button, Card, CardContent, CardHeader, Chip } from "@heroui/react";
import type { ReactNode } from "react";

import { ConsoleSidebarNav } from "./components/layout/ConsoleSidebarNav";
import { InlineNotice, KeyValueList, StatusChip } from "./components/ui";
import { getNavigationEntry, getNavigationGroupLabel } from "./navigation";
import type { ConsoleAppState } from "./useConsoleAppState";

type ConsoleShellProps = {
  app: ConsoleAppState;
  children: ReactNode;
};

function formatSessionExpiry(unixMs: number): string {
  return new Intl.DateTimeFormat("sv-SE", {
    dateStyle: "short",
    timeStyle: "medium",
    timeZone: "UTC",
  })
    .format(new Date(unixMs))
    .replace(",", "");
}

export function ConsoleShell({ app, children }: ConsoleShellProps) {
  const session = app.session;
  if (session === null) {
    return null;
  }
  const currentEntry = getNavigationEntry(app.section);
  const groupLabel = getNavigationGroupLabel(app.section);

  return (
    <div className="console-root">
      <header className="console-shell-header">
        <Card className="workspace-card flex-1" variant="secondary">
          <CardContent className="grid gap-4 p-4 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-start">
            <div className="grid gap-2">
              <p className="console-label">{currentEntry.label}</p>
              <div className="grid gap-1">
                <h1 className="text-2xl font-semibold tracking-tight">
                  Web Dashboard Operator Surface
                </h1>
                <p className="console-copy">
                  {groupLabel} domain focused on {currentEntry.detail.toLowerCase()}.
                </p>
              </div>
            </div>

            <div className="grid gap-3">
              <div className="console-shell__meta">
                <StatusChip tone="success">Authenticated</StatusChip>
                <Chip variant="secondary">{groupLabel}</Chip>
                <Chip variant="secondary">
                  Expires {formatSessionExpiry(session.expires_at_unix_ms)} UTC
                </Chip>
              </div>
              <div className="console-shell__actions">
                <Button
                  size="sm"
                  variant="secondary"
                  onPress={() =>
                    app.setTheme((current) => (current === "light" ? "dark" : "light"))
                  }
                >
                  Theme: {app.theme}
                </Button>
                <Button
                  isDisabled={app.logoutBusy}
                  size="sm"
                  variant="ghost"
                  onPress={() => void app.signOut()}
                >
                  {app.logoutBusy ? "Signing out..." : "Sign out"}
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      </header>

      <div className="console-shell-grid">
        <aside className="console-sidebar-card" aria-label="Dashboard domains">
          <ConsoleSidebarNav currentSection={app.section} onSelect={app.setSection} />
        </aside>

        <section className="console-shell__content">
          <Card className="workspace-card" variant="default">
            <CardHeader className="flex flex-col items-start gap-3 px-4 pb-0 pt-4 sm:flex-row sm:items-start sm:justify-between">
              <div>
                <p className="text-sm font-semibold">Session context</p>
                <p className="text-xs text-muted">
                  Principal, device, and disclosure controls stay compact and secondary to the page.
                </p>
              </div>
              <Button
                aria-label="Reveal sensitive values"
                size="sm"
                variant={app.revealSensitiveValues ? "secondary" : "ghost"}
                onPress={() => app.setRevealSensitiveValues((current) => !current)}
              >
                Reveal sensitive values: {app.revealSensitiveValues ? "On" : "Off"}
              </Button>
            </CardHeader>
            <CardContent className="p-4 pt-4">
              <KeyValueList
                className="console-session-grid"
                items={[
                  { label: "Principal", value: session.principal },
                  { label: "Device", value: session.device_id },
                  { label: "Channel", value: session.channel ?? "none" },
                  { label: "Transport", value: "Cookie session + CSRF" },
                ]}
              />
            </CardContent>
          </Card>
          {app.notice !== null ? (
            <InlineNotice title="Action result" tone="success">
              {app.notice}
            </InlineNotice>
          ) : null}
          {app.error !== null ? (
            <InlineNotice title="Action blocked" tone="danger">
              {app.error}
            </InlineNotice>
          ) : null}
          {children}
        </section>
      </div>
    </div>
  );
}
