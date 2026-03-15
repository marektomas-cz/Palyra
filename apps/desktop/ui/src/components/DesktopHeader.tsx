import { Skeleton } from "@heroui/react";

import type { ControlCenterSnapshot } from "../lib/desktopApi";
import { formatUnixMs, overallTone } from "./desktopPresentation";
import { KeyValueList, PageHeader, StatusChip } from "./ui";

type DesktopHeaderProps = {
  snapshot: ControlCenterSnapshot;
  loading: boolean;
};

export function DesktopHeader({ snapshot, loading }: DesktopHeaderProps) {
  return (
    <PageHeader
      eyebrow="Desktop Control Center"
      title="Launch the runtime, verify posture, then open the dashboard."
      description="Desktop stays intentionally short: lifecycle controls, runtime health, and the fastest handoff into the full operator console."
      status={
        <>
          <StatusChip tone={overallTone(snapshot.overall_status)}>
            {snapshot.overall_status}
          </StatusChip>
          <StatusChip tone="accent">{snapshot.quick_facts.dashboard_access_mode}</StatusChip>
          <StatusChip tone={loading ? "warning" : "success"}>
            {loading ? "Refreshing snapshot" : "Snapshot ready"}
          </StatusChip>
        </>
      }
      actions={
        <div className="desktop-header-summary">
          <KeyValueList
            items={[
              {
                label: "Overall state",
                value: loading ? (
                  <Skeleton className="desktop-skeleton desktop-skeleton--chip" />
                ) : (
                  <StatusChip tone={overallTone(snapshot.overall_status)}>
                    {snapshot.overall_status}
                  </StatusChip>
                )
              },
              {
                label: "Dashboard mode",
                value: loading ? (
                  <Skeleton className="desktop-skeleton desktop-skeleton--text" />
                ) : (
                  <strong>{snapshot.quick_facts.dashboard_access_mode}</strong>
                )
              },
              {
                label: "Last snapshot",
                value: loading ? (
                  <Skeleton className="desktop-skeleton desktop-skeleton--text" />
                ) : (
                  <strong>{formatUnixMs(snapshot.generated_at_unix_ms)}</strong>
                )
              }
            ]}
          />
        </div>
      }
    />
  );
}
