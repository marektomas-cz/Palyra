import { Card, CardContent, CardHeader } from "@heroui/react";
import type { PropsWithChildren, ReactNode } from "react";

import { joinClassNames } from "./utils";

type SectionCardProps = PropsWithChildren<{
  title: string;
  description?: string;
  actions?: ReactNode;
  className?: string;
  footer?: ReactNode;
  variant?: "transparent" | "default" | "secondary" | "tertiary";
}>;

export function SectionCard({
  title,
  description,
  actions,
  className,
  footer,
  variant = "default",
  children
}: SectionCardProps) {
  return (
    <Card className={joinClassNames("workspace-card workspace-section-card", className)} variant={variant}>
      <CardHeader className="workspace-section-card__header">
        <div className="workspace-section-card__copy">
          <h3>{title}</h3>
          {description !== undefined && <p className="chat-muted">{description}</p>}
        </div>
        {actions !== undefined && <div className="workspace-section-card__actions">{actions}</div>}
      </CardHeader>
      <CardContent className="workspace-section-card__body">
        {children}
        {footer !== undefined && <div className="workspace-section-card__footer">{footer}</div>}
      </CardContent>
    </Card>
  );
}
