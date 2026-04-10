import { useEffect, useState } from "react";

import type { ChatDelegationCatalog, ConsoleApiClient } from "../consoleApi";
import { toErrorMessage } from "./chatShared";

type UseChatPanelBootstrapParams = {
  api: ConsoleApiClient;
  dispose: () => void;
  refreshObjectives: () => Promise<void>;
  refreshSessions: (preferQuery: boolean) => Promise<void>;
  setError: (message: string | null) => void;
};

export function useChatPanelBootstrap({
  api,
  dispose,
  refreshObjectives,
  refreshSessions,
  setError,
}: UseChatPanelBootstrapParams) {
  const [delegationCatalog, setDelegationCatalog] = useState<ChatDelegationCatalog | null>(null);

  useEffect(() => {
    void refreshSessions(true);
    void Promise.all([api.getDelegationCatalog(), refreshObjectives()])
      .then(([delegationResponse]) => {
        setDelegationCatalog(delegationResponse.catalog);
      })
      .catch((error) => {
        setError(toErrorMessage(error));
      });
    return () => {
      dispose();
    };
  }, [api, dispose, refreshObjectives, refreshSessions, setError]);

  return delegationCatalog;
}
