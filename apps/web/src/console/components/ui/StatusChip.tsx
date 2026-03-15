import { Chip } from "@heroui/react";
import type { PropsWithChildren } from "react";

import { joinClassNames, resolveToneColor, type UiTone } from "./utils";

type StatusChipProps = PropsWithChildren<{
  tone?: UiTone;
}>;

export function StatusChip({
  children,
  tone = "default"
}: StatusChipProps) {
  return (
    <Chip
      className={joinClassNames("workspace-status-chip", tone === "accent" && "workspace-status-chip--accent")}
      color={resolveToneColor(tone)}
      variant={tone === "default" ? "secondary" : "soft"}
    >
      {children}
    </Chip>
  );
}
