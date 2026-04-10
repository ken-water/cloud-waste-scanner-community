import type { ReactNode } from "react";

interface PageShellProps {
  children: ReactNode;
  maxWidthClassName?: string;
  className?: string;
}

export function PageShell({
  children,
  maxWidthClassName = "max-w-6xl",
  className = "space-y-8",
}: PageShellProps) {
  return (
    <div className="cws-page-shell min-h-screen bg-slate-50 p-8 dark:bg-slate-900">
      <div className={`cws-page-shell-inner mx-auto ${maxWidthClassName} ${className}`.trim()}>{children}</div>
    </div>
  );
}
