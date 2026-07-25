import type { ReactNode } from "react";

interface PageHeaderProps {
  title: string;
  subtitle?: string;
  icon?: ReactNode;
  actions?: ReactNode;
}

export function PageHeader({ title, subtitle, icon, actions }: PageHeaderProps) {
  return (
    <div className="cws-page-header flex flex-col gap-5 border-b border-slate-200/80 pb-6 dark:border-slate-800/80 md:flex-row md:items-start md:justify-between">
      <div className="min-w-0 md:flex-1">
        <div className="flex items-center gap-3">
          {icon ? (
            <div className="flex h-11 w-11 items-center justify-center rounded-xl border border-slate-200 bg-white text-teal-700 shadow-sm dark:border-slate-800 dark:bg-slate-950 dark:text-teal-300">
              {icon}
            </div>
          ) : null}
          <div className="min-w-0 flex-1">
            <h1 className="text-3xl font-semibold tracking-tight text-slate-950 dark:text-slate-50">{title}</h1>
            {subtitle ? (
              <p className="mt-2 max-w-3xl text-sm leading-6 text-slate-600 dark:text-slate-300">{subtitle}</p>
            ) : null}
          </div>
        </div>
      </div>
      {actions ? <div className="cws-page-header-actions flex min-w-0 shrink-0 items-center gap-3 md:max-w-[46%]">{actions}</div> : null}
    </div>
  );
}
