const FIRST_SUCCESS_STORAGE_KEY = "palyra.desktop.firstSuccess.completed";

export function readDesktopFirstSuccessCompleted(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  try {
    return window.localStorage.getItem(FIRST_SUCCESS_STORAGE_KEY) === "true";
  } catch {
    return false;
  }
}

export function markDesktopFirstSuccessCompleted(): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(FIRST_SUCCESS_STORAGE_KEY, "true");
  } catch {
    // Ignore local preference persistence failures.
  }
}
