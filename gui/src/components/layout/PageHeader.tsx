import type { ReactNode } from "react";

interface PageHeaderProps {
  title: string;
  subtitle?: string;
  icon?: ReactNode;
  actions?: ReactNode;
}

export function PageHeader({ title, subtitle, icon, actions }: PageHeaderProps) {
  return (
    <div className="cws-page-header flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
      <div className="min-w-0">
        <div className="flex items-center gap-3">
          {icon ? (
            <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-indigo-50 text-indigo-600 dark:bg-indigo-500/15 dark:text-indigo-300">
              {icon}
            </div>
          ) : null}
          <div className="min-w-0">
            <h1 className="text-3xl font-bold tracking-tight text-slate-900 dark:text-white">{title}</h1>
            {subtitle ? (
              <p className="mt-2 max-w-3xl text-base text-slate-500 dark:text-slate-400">{subtitle}</p>
            ) : null}
          </div>
        </div>
      </div>
      {actions ? <div className="cws-page-header-actions flex shrink-0 items-center gap-3">{actions}</div> : null}
    </div>
  );
}
