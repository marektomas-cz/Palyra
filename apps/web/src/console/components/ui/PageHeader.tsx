import { Card, CardContent } from "@heroui/react";
import type { ReactNode } from "react";

import { joinClassNames } from "./utils";

type PageHeaderProps = {
  eyebrow?: string;
  title: string;
  headingLabel?: string;
  description: string;
  status?: ReactNode;
  actions?: ReactNode;
  className?: string;
};

export function PageHeader({
  eyebrow,
  title,
  headingLabel,
  description,
  status,
  actions,
  className,
}: PageHeaderProps) {
  return (
    <Card
      className={joinClassNames("workspace-card workspace-page-header-card", className)}
      variant="secondary"
    >
      <CardContent className="workspace-page-header">
        <div className="workspace-page-header__copy">
          {eyebrow !== undefined && <p className="console-label">{eyebrow}</p>}
          <div className="workspace-page-header__title-block">
            <h2 aria-label={headingLabel}>{title}</h2>
            <p className="console-copy">{description}</p>
          </div>
          {status !== undefined && <div className="workspace-chip-row">{status}</div>}
        </div>
        {actions !== undefined && <div className="workspace-page-header__actions">{actions}</div>}
      </CardContent>
    </Card>
  );
}
