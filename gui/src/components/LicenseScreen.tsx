import { Check, ShieldCheck } from "lucide-react";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";
import {
  EDITION_CAPABILITY_MATRIX,
  capabilityEnabled,
  formatEditionLabel,
  readRuntimePlanTypeFromStorage,
  resolveRuntimeEdition,
} from "../lib/edition";

export default function LicenseScreen() {
  const runtimePlanType = readRuntimePlanTypeFromStorage();
  const runtimeEdition = resolveRuntimeEdition(runtimePlanType);

  return (
    <PageShell maxWidthClassName="max-w-6xl" className="space-y-6">
      <PageHeader
        title="Community Edition"
        subtitle="This build is the local-first discovery and evidence layer. Remote license activation and online entitlement checks are disabled."
        icon={<ShieldCheck className="h-6 w-6" />}
      />
      <section className="rounded-2xl border border-slate-200 bg-white p-6 text-sm leading-6 text-slate-700 shadow-sm dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200">
        <p className="font-semibold text-slate-900 dark:text-white">Local-first execution</p>
        <p className="mt-2">
          Community edition keeps scan execution, credentials, and findings on this machine. No checkout flow,
          remote activation, or hosted license endpoint is required.
        </p>
      </section>

      <section className="rounded-2xl border border-slate-200 bg-white p-6 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
              Capability Matrix
            </p>
            <h2 className="mt-2 text-xl font-semibold text-slate-900 dark:text-white">
              Community discovers. Team executes. Enterprise centralizes.
            </h2>
            <p className="mt-2 text-sm leading-6 text-slate-600 dark:text-slate-300">
              Current runtime edition: <span className="font-semibold text-slate-900 dark:text-white">{formatEditionLabel(runtimeEdition)}</span>
            </p>
          </div>
        </div>

        <div className="mt-6 overflow-hidden rounded-2xl border border-slate-200 dark:border-slate-700">
          <div className="grid grid-cols-[minmax(0,2.2fr)_110px_110px_130px] border-b border-slate-200 bg-slate-50 text-sm font-semibold text-slate-700 dark:border-slate-700 dark:bg-slate-900/40 dark:text-slate-200">
            <div className="px-4 py-3">Capability</div>
            <div className="px-4 py-3 text-center">Community</div>
            <div className="px-4 py-3 text-center">Team</div>
            <div className="px-4 py-3 text-center">Enterprise</div>
          </div>
          {EDITION_CAPABILITY_MATRIX.map((row) => (
            <div
              key={row.key}
              className="grid grid-cols-[minmax(0,2.2fr)_110px_110px_130px] border-b border-slate-200 last:border-b-0 dark:border-slate-700"
            >
              <div className="px-4 py-4">
                <div className="font-semibold text-slate-900 dark:text-white">{row.label}</div>
                <div className="mt-1 text-sm leading-6 text-slate-500 dark:text-slate-400">{row.description}</div>
              </div>
              {[
                { enabled: row.community, label: "Community" },
                { enabled: row.team, label: "Team" },
                { enabled: row.enterprise, label: "Enterprise" },
              ].map((cell) => (
                <div key={cell.label} className="flex items-center justify-center px-4 py-4">
                  {cell.enabled ? (
                    <span className="inline-flex h-8 w-8 items-center justify-center rounded-full bg-emerald-50 text-emerald-600 dark:bg-emerald-500/10 dark:text-emerald-300">
                      <Check className="h-4 w-4" />
                    </span>
                  ) : (
                    <span className="text-sm font-semibold text-slate-300 dark:text-slate-600">-</span>
                  )}
                </div>
              ))}
            </div>
          ))}
        </div>

        <div className="mt-5 rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-600 dark:border-slate-700 dark:bg-slate-900/40 dark:text-slate-300">
          Active on this machine now:
          <span className="ml-2 font-semibold text-slate-900 dark:text-white">
            {EDITION_CAPABILITY_MATRIX.filter((row) => capabilityEnabled(row.key, runtimePlanType)).map((row) => row.label).join(" · ")}
          </span>
        </div>
      </section>
    </PageShell>
  );
}
