import { Button, Card, CardContent, CardHeader, Chip } from "@heroui/react";
import type { ReactNode } from "react";

import { ConsoleSidebarNav } from "./components/layout/ConsoleSidebarNav";
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
    timeZone: "UTC"
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
    <div className="console-root min-h-screen px-4 py-6">
      <header className="mb-5">
        <Card className="border border-white/30 bg-white/80 shadow-2xl shadow-slate-900/10 backdrop-blur-xl dark:border-white/10 dark:bg-slate-950/75">
          <CardContent className="gap-6 px-5 py-5 lg:flex-row lg:items-center lg:justify-between">
            <div className="space-y-3">
              <p className="console-label">Palyra / M56</p>
              <div className="space-y-2">
                <h1 className="text-3xl font-semibold tracking-tight text-slate-950 dark:text-slate-50">
                  Web Dashboard Operator Surface
                </h1>
                <p className="max-w-3xl text-sm leading-6 text-slate-600 dark:text-slate-300">
                  {groupLabel} workspace focused on {currentEntry.label.toLowerCase()}.
                </p>
              </div>
            </div>

            <div className="flex w-full max-w-lg flex-col items-start gap-4 lg:items-end">
              <div className="flex flex-wrap items-center gap-2">
                <Chip color="success" variant="soft">
                  Authenticated
                </Chip>
                <Chip variant="soft">{groupLabel}</Chip>
                <Chip variant="secondary">
                  Expires {formatSessionExpiry(session.expires_at_unix_ms)} UTC
                </Chip>
              </div>
              <div className="flex flex-wrap items-center gap-3">
                <Button
                  variant="secondary"
                  onPress={() =>
                    app.setTheme((current) => (current === "light" ? "dark" : "light"))
                  }
                >
                  Theme: {app.theme}
                </Button>
                <Button
                  variant="danger-soft"
                  isDisabled={app.logoutBusy}
                  onPress={() => void app.signOut()}
                >
                  {app.logoutBusy ? "Signing out..." : "Sign out"}
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      </header>

      <div className="grid gap-5 xl:grid-cols-[320px_minmax(0,1fr)]">
        <aside aria-label="Dashboard domains">
          <ConsoleSidebarNav currentSection={app.section} onSelect={app.setSection} />
        </aside>

        <section className="space-y-4">
          <Card className="border border-white/25 bg-white/65 shadow-none dark:border-white/10 dark:bg-slate-950/60">
            <CardHeader className="flex flex-col items-start gap-2 px-4 pb-0 pt-4 sm:flex-row sm:items-center sm:justify-between">
              <div>
                <p className="text-sm font-semibold text-slate-950 dark:text-slate-50">
                  Session context
                </p>
                <p className="text-xs text-slate-500 dark:text-slate-400">
                  Principal, device, and disclosure controls for this workspace.
                </p>
              </div>
              <Button
                aria-label="Reveal sensitive values"
                variant={app.revealSensitiveValues ? "primary" : "ghost"}
                onPress={() => app.setRevealSensitiveValues((current) => !current)}
              >
                Reveal sensitive values: {app.revealSensitiveValues ? "On" : "Off"}
              </Button>
            </CardHeader>
            <CardContent className="flex flex-wrap items-center gap-2 px-4 py-4 text-sm">
              <Chip variant="secondary">{session.principal}</Chip>
              <Chip variant="secondary">{session.device_id}</Chip>
              <Chip variant="secondary">{session.channel ?? "no channel"}</Chip>
              <Chip variant="soft">Cookie session + CSRF</Chip>
            </CardContent>
          </Card>
          {app.notice !== null ? (
            <Card className="border border-success/20 bg-success/10 shadow-none">
              <CardContent className="px-4 py-3 text-sm text-success-700 dark:text-success-300">
                {app.notice}
              </CardContent>
            </Card>
          ) : null}
          {app.error !== null ? (
            <Card
              className="border border-danger/20 bg-danger/10 shadow-none"
              role="alert"
              aria-live="polite"
            >
              <CardContent className="gap-1 px-4 py-3">
                <h2 className="text-sm font-semibold text-danger-700 dark:text-danger-300">
                  Action blocked
                </h2>
                <p className="text-sm text-danger-700 dark:text-danger-300">{app.error}</p>
              </CardContent>
            </Card>
          ) : null}
          {children}
        </section>
      </div>
    </div>
  );
}
