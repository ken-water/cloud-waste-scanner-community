import { ShieldCheck } from "lucide-react";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";

export default function LicenseScreen() {
  return (
    <PageShell maxWidthClassName="max-w-6xl" className="space-y-6">
      <PageHeader
        title="Community Mode"
        subtitle="This build runs fully local. Remote license activation and online entitlement checks are disabled."
        icon={<ShieldCheck className="h-6 w-6" />}
      />
      <section className="rounded-2xl border border-slate-200 bg-white p-6 text-sm leading-6 text-slate-700 shadow-sm dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200">
        <p className="font-semibold text-slate-900 dark:text-white">Local-first execution</p>
        <p className="mt-2">
          Community edition keeps scan execution, credentials, and findings on this machine. No checkout flow,
          remote activation, or hosted license endpoint is required.
        </p>
      </section>
    </PageShell>
  );
}
