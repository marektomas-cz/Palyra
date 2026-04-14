export const CHAT_STARTER_PROMPT_HANDOFF_KEY = "palyra.chat.starterPrompt";

export const FIRST_SUCCESS_PROMPTS = [
  "Summarize the current runtime posture and tell me what needs attention first.",
  "Verify my provider and model setup, then list anything still blocking a real run.",
  "Give me a safe first operator workflow I can run end-to-end from this environment.",
] as const;

export function queueChatStarterPrompt(prompt: string): void {
  if (typeof window === "undefined") {
    return;
  }
  const trimmed = prompt.trim();
  if (trimmed.length === 0) {
    return;
  }
  window.sessionStorage.setItem(CHAT_STARTER_PROMPT_HANDOFF_KEY, trimmed);
}

export function consumeChatStarterPrompt(): string | null {
  if (typeof window === "undefined") {
    return null;
  }
  const prompt = window.sessionStorage.getItem(CHAT_STARTER_PROMPT_HANDOFF_KEY);
  if (prompt === null) {
    return null;
  }
  window.sessionStorage.removeItem(CHAT_STARTER_PROMPT_HANDOFF_KEY);
  const trimmed = prompt.trim();
  return trimmed.length > 0 ? trimmed : null;
}
