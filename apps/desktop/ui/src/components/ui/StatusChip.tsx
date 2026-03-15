import { Chip } from "@heroui/react";
import type { PropsWithChildren } from "react";

import { joinClassNames, resolveToneColor, type UiTone } from "./utils";

type StatusChipProps = PropsWithChildren<{
  tone?: UiTone;
  className?: string;
}>;

export function StatusChip({
  children,
  tone = "default",
  className
}: StatusChipProps) {
  return (
    <Chip
      className={joinClassNames(
        "desktop-status-chip",
        tone === "accent" && "desktop-status-chip--accent",
        className
      )}
      color={resolveToneColor(tone)}
      variant={tone === "default" ? "secondary" : "soft"}
    >
      {children}
    </Chip>
  );
}
