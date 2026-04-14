import { useState } from "react";

import { markFirstSuccessCompleted, readFirstSuccessCompleted } from "./firstSuccessState";
import { readGuidanceHidden, writeGuidanceHidden } from "../console/guidancePreferences";

const STARTER_PROMPTS_SCOPE = "chat-starter-prompts";

export function useStarterPromptGuidance() {
  const [starterPromptsHidden, setStarterPromptsHidden] = useState(() =>
    readGuidanceHidden(STARTER_PROMPTS_SCOPE),
  );
  const [firstSuccessCompleted, setFirstSuccessCompleted] = useState(() =>
    readFirstSuccessCompleted(),
  );

  return {
    firstSuccessCompleted,
    starterPromptsHidden,
    markFirstSuccessCompleted() {
      setFirstSuccessCompleted(true);
      markFirstSuccessCompleted();
    },
    hideStarterPrompts() {
      setStarterPromptsHidden(true);
      writeGuidanceHidden(STARTER_PROMPTS_SCOPE, true);
    },
    showStarterPrompts() {
      setStarterPromptsHidden(false);
      writeGuidanceHidden(STARTER_PROMPTS_SCOPE, false);
    },
  };
}
