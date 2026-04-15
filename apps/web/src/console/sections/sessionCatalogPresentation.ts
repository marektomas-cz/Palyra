import type { SessionCatalogArtifactRecord, SessionCatalogRecord } from "../../consoleApi";

type SessionCatalogPresentation = {
  familyRootTitle: string;
  familySize: number;
  familySequence: number;
  agentDisplay: string;
  modelDisplay: string;
  touchedFiles: string[];
  activeContextFiles: string[];
  recentArtifacts: SessionCatalogArtifactRecord[];
};

export function buildSessionCatalogPresentation(
  session: SessionCatalogRecord | null | undefined,
): SessionCatalogPresentation {
  return {
    familyRootTitle: session?.family?.root_title ?? session?.title ?? "Unknown",
    familySize: session?.family?.family_size ?? 1,
    familySequence: session?.family?.sequence ?? 1,
    agentDisplay:
      session?.quick_controls?.agent?.display_value ?? session?.agent_id ?? "default agent",
    modelDisplay:
      session?.quick_controls?.model?.display_value ?? session?.model_profile ?? "default model",
    touchedFiles: session?.recap?.touched_files ?? [],
    activeContextFiles: session?.recap?.active_context_files ?? [],
    recentArtifacts: session?.recap?.recent_artifacts ?? [],
  };
}
