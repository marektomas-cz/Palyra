import { useEffect, useState } from "react";

import { consumeChatStarterPrompt } from "./starterPrompts";

type UseStarterPromptHandoffArgs = {
  activeSessionId: string;
  setNotice: (next: string | null) => void;
  updateComposerDraft: (value: string) => void;
};

export function useStarterPromptHandoff({
  activeSessionId,
  setNotice,
  updateComposerDraft,
}: UseStarterPromptHandoffArgs): void {
  const [starterPromptRequest, setStarterPromptRequest] = useState<string | null>(() =>
    consumeChatStarterPrompt(),
  );

  useEffect(() => {
    if (starterPromptRequest === null || activeSessionId.trim().length === 0) {
      return;
    }
    updateComposerDraft(starterPromptRequest);
    setNotice("Starter prompt loaded into the composer.");
    setStarterPromptRequest(null);
  }, [activeSessionId, setNotice, starterPromptRequest, updateComposerDraft]);
}
