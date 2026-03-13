import { useEffect, useRef } from "react";
import { Navigate, useLocation } from "react-router-dom";

import { ConsoleSectionContent } from "./ConsoleSectionContent";
import { ConsoleShell } from "./ConsoleShell";
import { findSectionByPath, getSectionPath } from "./navigation";
import type { ConsoleAppState } from "./useConsoleAppState";

type ConsoleRoutedShellProps = {
  app: ConsoleAppState;
};

export function ConsoleRoutedShell({ app }: ConsoleRoutedShellProps) {
  const location = useLocation();
  const syncedPathnameRef = useRef<string | null>(null);
  const normalizedPathname =
    location.pathname.endsWith("/") && location.pathname.length > 1
      ? location.pathname.slice(0, -1)
      : location.pathname;
  const nextSection = findSectionByPath(location.pathname);

  useEffect(() => {
    if (normalizedPathname === syncedPathnameRef.current) {
      return;
    }

    syncedPathnameRef.current = normalizedPathname;
    if (nextSection !== null && app.section !== nextSection) {
      app.setSection(nextSection);
    }
  }, [app.section, app.setSection, nextSection, normalizedPathname]);

  if (nextSection === null) {
    return <Navigate to={getSectionPath(app.section)} replace />;
  }

  return (
    <ConsoleShell app={app}>
      <ConsoleSectionContent app={app} />
    </ConsoleShell>
  );
}
