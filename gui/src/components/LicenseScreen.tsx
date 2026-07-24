import { Check, Download, KeyRound, MessageSquare, ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openCheckoutUrl } from "../utils/checkout";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";
import {
  buildRuntimeCapabilitySnapshotFromPlan,
  persistRuntimePlanToStorage,
  readRuntimePlanTypeFromStorage,
  type RuntimeCapabilitySnapshot,
} from "../lib/edition";

type LicensePayload = {
  id: string;
  user: string;
  l_type: "trial" | "trial_expired" | "starter" | "monthly" | "yearly" | "lifetime" | "subscription" | "per_use";
  expires_at?: number | null;
  max_hosts?: number | null;
};

type LicenseStatus = {
  valid: boolean;
  message?: string | null;
  plan_type?: string | null;
  customer_email?: string | null;
  latest_order_ref?: string | null;
  latest_order_plan?: string | null;
};

const BETA_ACTIONS = [
  {
    id: "download",
    name: "Download latest build",
    href: "https://cloud-waste-scanner.com/download/",
    note: "Use the current Windows, Linux, or macOS public beta build.",
  },
  {
    id: "feedback",
    name: "Send first-scan feedback",
    href: "https://cloud-waste-scanner.com/feedback.html",
    note: "Tell us what worked, what blocked setup, and which findings were useful.",
  },
  {
    id: "beta",
    name: "Read beta terms",
    href: "https://cloud-waste-scanner.com/license.html",
    note: "Review the public beta access boundary and future packaging note.",
  },
];

function formatPlanLabel(plan: string | null | undefined): string {
  const normalized = (plan || "").trim().toLowerCase();
  if (normalized === "monthly" || normalized === "subscription") return "Monthly";
  if (normalized === "yearly") return "Yearly";
  if (normalized === "lifetime") return "Lifetime";
  if (normalized === "starter" || normalized === "per_use") return "Starter";
  if (normalized === "public_beta" || normalized === "beta") return "Public Beta";
  if (normalized === "trial") return "7-day Trial";
  if (normalized === "trial_expired") return "Trial expired";
  return "Not activated";
}

function formatUnixDate(value: number | null | undefined): string {
  if (!value) return "No expiry";
  return new Date(value * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "2-digit",
  });
}

export default function LicenseScreen() {
  const runtimePlanType = readRuntimePlanTypeFromStorage();
  const [snapshot, setSnapshot] = useState<RuntimeCapabilitySnapshot>(
    buildRuntimeCapabilitySnapshotFromPlan(runtimePlanType),
  );
  const [licenseKey, setLicenseKey] = useState("");
  const [payload, setPayload] = useState<LicensePayload | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  const refreshSnapshot = async () => {
    try {
      const value = await invoke<RuntimeCapabilitySnapshot>("get_runtime_capability_snapshot");
      persistRuntimePlanToStorage(value.plan_type);
      setSnapshot(value);
    } catch {
      setSnapshot(buildRuntimeCapabilitySnapshotFromPlan(readRuntimePlanTypeFromStorage()));
    }
  };

  useEffect(() => {
    let mounted = true;
    const load = async () => {
      try {
        const key = await invoke<string>("load_license_file");
        if (!mounted) return;
        setLicenseKey(key.trim());
        if (key.trim()) {
          const parsed = await invoke<LicensePayload>("validate_license_key", { key });
          if (!mounted) return;
          setPayload(parsed);
          persistRuntimePlanToStorage(parsed.l_type);
        }
      } catch {
        if (!mounted) return;
        setPayload(null);
      } finally {
        if (mounted) void refreshSnapshot();
      }
    };
    void load();
    return () => {
      mounted = false;
    };
  }, []);

  const activateLicense = async () => {
    const key = licenseKey.trim();
    if (!key) {
      setMessage("Paste the license key you received after purchase.");
      return;
    }
    setIsBusy(true);
    setMessage(null);
    try {
      const parsed = await invoke<LicensePayload>("validate_license_key", { key });
      const status = await invoke<LicenseStatus>("check_license_status", { key });
      await invoke("save_license_file", { key });
      setPayload(parsed);
      persistRuntimePlanToStorage(status.plan_type || parsed.l_type);
      setMessage(`${formatPlanLabel(status.plan_type || parsed.l_type)} license activated.`);
      await refreshSnapshot();
    } catch (error) {
      setPayload(null);
      setMessage(`Activation failed: ${String(error)}`);
    } finally {
      setIsBusy(false);
    }
  };

  const activePlan = payload?.l_type || snapshot.plan_type;
  const isActivated = !!payload;
  const isTrial = snapshot.plan_type === "trial" && !isActivated;
  const isTrialExpired = snapshot.plan_type === "trial_expired" && !isActivated;

  return (
    <PageShell maxWidthClassName="max-w-6xl" className="space-y-6">
      <PageHeader
        title="License"
        subtitle="Cloud Waste Scanner is free during the public beta while we collect first-scan feedback."
        icon={<ShieldCheck className="h-6 w-6" />}
      />

      <section className="rounded-2xl border border-slate-200 bg-white p-6 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="grid gap-4 md:grid-cols-3">
          <div className="rounded-xl border border-slate-200 bg-slate-50 p-4 dark:border-slate-700 dark:bg-slate-900/40">
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Current Plan</p>
            <p className="mt-2 text-2xl font-bold text-slate-900 dark:text-white">{formatPlanLabel(activePlan)}</p>
            <p className="mt-2 text-sm text-slate-600 dark:text-slate-300">
              {isActivated
                ? "License is installed on this machine."
                : isTrial
                  ? "Full-feature trial is active on this machine."
                  : isTrialExpired
                    ? "Legacy trial state detected; public beta access is now enabled after refresh."
                    : "Public beta access is active on this machine."}
            </p>
          </div>
          <div className="rounded-xl border border-slate-200 bg-slate-50 p-4 dark:border-slate-700 dark:bg-slate-900/40">
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Licensed Email</p>
            <p className="mt-2 text-lg font-bold text-slate-900 dark:text-white">{payload?.user || "-"}</p>
            <p className="mt-2 text-sm text-slate-600 dark:text-slate-300">Only shown when a legacy signed license is installed.</p>
          </div>
          <div className="rounded-xl border border-slate-200 bg-slate-50 p-4 dark:border-slate-700 dark:bg-slate-900/40">
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Expiry</p>
            <p className="mt-2 text-lg font-bold text-slate-900 dark:text-white">{formatUnixDate(payload?.expires_at || snapshot.trial_expires_at)}</p>
            <p className="mt-2 text-sm text-slate-600 dark:text-slate-300">
              {isTrial
                ? `${snapshot.trial_days_remaining ?? 0} day${snapshot.trial_days_remaining === 1 ? "" : "s"} remaining.`
                : isTrialExpired
                  ? "Legacy trial state."
                  : "Public beta access has no fixed expiry in this build."}
            </p>
          </div>
        </div>
      </section>

      {(isTrial || isTrialExpired) && (
        <section className={`rounded-2xl border p-5 text-sm leading-6 shadow-sm ${isTrialExpired ? "border-amber-300 bg-amber-50 text-amber-900 dark:border-amber-500/40 dark:bg-amber-500/10 dark:text-amber-100" : "border-emerald-200 bg-emerald-50 text-emerald-900 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-100"}`}>
          <p className="font-bold">{isTrial ? "Legacy trial state detected." : "Legacy expired trial state detected."}</p>
          <p className="mt-1">
            {isTrial
              ? "Public beta access replaces the short trial model in this build."
              : "Restart or refresh the license screen to enable public beta access."}
          </p>
        </section>
      )}

      <section className="rounded-2xl border border-slate-200 bg-white p-6 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="flex items-start gap-3">
          <span className="rounded-xl bg-emerald-50 p-3 text-emerald-700 dark:bg-emerald-500/10 dark:text-emerald-300">
            <KeyRound className="h-5 w-5" />
          </span>
          <div className="flex-1">
            <h2 className="text-xl font-semibold text-slate-900 dark:text-white">Legacy license activation</h2>
            <p className="mt-1 text-sm leading-6 text-slate-600 dark:text-slate-300">
              Public beta access does not require a license key. This field remains available only for older signed keys.
            </p>
            <textarea
              value={licenseKey}
              onChange={(event) => setLicenseKey(event.target.value)}
              className="mt-4 h-28 w-full rounded-xl border border-slate-200 bg-white p-3 font-mono text-sm text-slate-800 outline-none focus:border-emerald-500 focus:ring-2 focus:ring-emerald-500/20 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100"
              placeholder="Paste license key here"
            />
            <div className="mt-4 flex flex-wrap items-center gap-3">
              <button
                type="button"
                onClick={() => void activateLicense()}
                disabled={isBusy}
                className="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-emerald-700 disabled:cursor-not-allowed disabled:opacity-60"
              >
                {isBusy ? "Activating..." : "Activate License"}
              </button>
              {message && <span className="text-sm text-slate-600 dark:text-slate-300">{message}</span>}
            </div>
          </div>
        </div>
      </section>

      <section className="grid gap-4 md:grid-cols-3">
        {BETA_ACTIONS.map((action) => (
          <article key={action.id} className="rounded-2xl border border-slate-200 bg-white p-6 shadow-sm dark:border-slate-700 dark:bg-slate-800">
            <div className="flex items-center justify-between gap-3">
              <h3 className="text-lg font-bold text-slate-900 dark:text-white">{action.name}</h3>
              {action.id === "feedback" ? <MessageSquare className="h-5 w-5 text-emerald-600 dark:text-emerald-300" /> : <Download className="h-5 w-5 text-emerald-600 dark:text-emerald-300" />}
            </div>
            <p className="mt-4 min-h-[72px] text-sm leading-6 text-slate-600 dark:text-slate-300">{action.note}</p>
            <button
              type="button"
              onClick={() => void openCheckoutUrl(action.href)}
              className="mt-5 w-full rounded-lg border border-emerald-600 px-4 py-2 text-sm font-semibold text-emerald-700 hover:bg-emerald-50 dark:border-emerald-400 dark:text-emerald-200 dark:hover:bg-emerald-500/10"
            >
              Open
            </button>
          </article>
        ))}
      </section>

      <section className="rounded-2xl border border-slate-200 bg-white p-6 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <h2 className="text-xl font-semibold text-slate-900 dark:text-white">Included during public beta</h2>
        <div className="mt-4 grid gap-3 md:grid-cols-2">
          {[
            "Read-only local scans for AWS, Azure, GCP, Alibaba Cloud, DigitalOcean, Kubernetes, containers, and AI runtime evidence.",
            "Local reports, exports, governance workflow, owner assignment, scheduled audits, and local API automation.",
            "Credentials and inventory evidence stay on the operator machine.",
            "Public beta access is free while we learn from real first-scan usage and feedback.",
          ].map((item) => (
            <div key={item} className="flex gap-3 rounded-xl bg-slate-50 p-4 text-sm text-slate-700 dark:bg-slate-900/40 dark:text-slate-200">
              <Check className="mt-0.5 h-4 w-4 shrink-0 text-emerald-600 dark:text-emerald-300" />
              <span>{item}</span>
            </div>
          ))}
        </div>
      </section>
    </PageShell>
  );
}
