function storageKey(scope: string): string {
  return `palyra.console.guidance.${scope}.hidden`;
}

export function readGuidanceHidden(scope: string): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  try {
    return window.localStorage.getItem(storageKey(scope)) === "true";
  } catch {
    return false;
  }
}

export function writeGuidanceHidden(scope: string, hidden: boolean): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    if (hidden) {
      window.localStorage.setItem(storageKey(scope), "true");
    } else {
      window.localStorage.removeItem(storageKey(scope));
    }
  } catch {
    // Ignore storage failures; guidance still works for the current session.
  }
}
