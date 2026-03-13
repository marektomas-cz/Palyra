import { Card, CardContent } from "@heroui/react";

export function ConsoleBootScreen() {
  return (
    <div className="console-root flex min-h-screen items-center justify-center px-4 py-8">
      <Card className="w-full max-w-xl border border-white/30 bg-white/75 shadow-2xl shadow-slate-900/10 backdrop-blur-xl dark:border-white/10 dark:bg-slate-950/70">
        <CardContent className="gap-4 px-8 py-10 text-center">
          <p className="console-label">Palyra / M56</p>
          <h1 className="text-3xl font-semibold tracking-tight text-slate-950 dark:text-slate-50">
            Web Dashboard
          </h1>
          <p className="text-base text-slate-600 dark:text-slate-300">
            Checking existing session and preparing the operator workspace.
          </p>
        </CardContent>
      </Card>
    </div>
  );
}
