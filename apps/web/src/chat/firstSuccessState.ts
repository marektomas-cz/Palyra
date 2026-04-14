const FIRST_SUCCESS_STORAGE_KEY = "palyra.firstSuccess.completed";

export function readFirstSuccessCompleted(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  try {
    return window.localStorage.getItem(FIRST_SUCCESS_STORAGE_KEY) === "true";
  } catch {
    return false;
  }
}

export function markFirstSuccessCompleted(): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(FIRST_SUCCESS_STORAGE_KEY, "true");
  } catch {
    // Ignore local preference persistence failures.
  }
}
