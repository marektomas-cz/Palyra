import { Card, CardContent, Spinner } from "@heroui/react";

export function ConsoleBootScreen() {
  return (
    <div className="console-root console-root--auth flex min-h-screen items-center justify-center">
      <Card className="workspace-card w-full max-w-lg" variant="secondary">
        <CardContent className="grid gap-4 px-6 py-7 text-center">
          <div className="grid justify-items-center gap-3">
            <Spinner color="current" size="sm" />
            <p className="console-label">Palyra console</p>
          </div>
          <div className="grid gap-2">
            <h1 className="text-2xl font-semibold tracking-tight">Web Dashboard</h1>
            <p className="console-copy">
              Checking the current session and loading the dashboard shell.
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
