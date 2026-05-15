import { Cloud, Bell, Network, Monitor, Settings as SettingsIcon, Bot } from "lucide-react";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";
import { MetricCard } from "./ui/MetricCard";
import {
  canAccessTabByPlan,
  readRuntimePlanTypeFromStorage,
  requiredEntitlementForTab,
} from "../lib/edition";

interface SettingsHubScreenProps {
  onNavigate: (tab: string) => void;
}

const groups = [
  {
    title: "Configuration",
    items: [
      {
        id: "accounts",
        label: "Accounts",
        description: "Manage cloud credentials, provider imports, and scan rules per account.",
        icon: Cloud,
      },
      {
        id: "notifications",
        label: "Notifications",
        description: "Route scan summaries and waste alerts to Slack, Telegram, email, or webhooks.",
        icon: Bell,
      },
      {
        id: "network_proxy",
        label: "Proxy Profiles",
        description: "Define direct, HTTP, HTTPS, or SOCKS proxy paths for accounts and outbound notifications.",
        icon: Network,
      },
      {
        id: "preferences",
        label: "Preferences",
        description: "Control theme, font size, and reporting currency for operators and exports.",
        icon: SettingsIcon,
      },
      {
        id: "ai_settings",
        label: "AI Runtime Settings",
        description: "Configure local GPU runtime scan posture and optional external endpoint policy.",
        icon: Bot,
      },
      {
        id: "local_api",
        label: "Local API",
        description: "Adjust bind host, TLS, token policy, and LAN exposure for the embedded API.",
        icon: Monitor,
      },
    ],
  },
];

export function SettingsHubScreen({ onNavigate }: SettingsHubScreenProps) {
  const itemCount = groups.flatMap((group) => group.items).length;
  const runtimePlanType = readRuntimePlanTypeFromStorage();
  const entitlementHintByTab: Record<string, string> = {
    local_api: "Upgrade required",
  };

  return (
    <PageShell maxWidthClassName="max-w-6xl">
        <PageHeader
          title="Configuration"
          subtitle="Open the exact control surface you need instead of working through one oversized settings page."
          icon={<SettingsIcon className="h-6 w-6" />}
        />

        <div className="grid gap-4 md:grid-cols-3">
          <MetricCard
            label="Control Surfaces"
            value={itemCount}
            hint="Dedicated pages instead of one overloaded settings sheet."
          />
          <MetricCard
            label="Access Model"
            value="Local"
            hint="Credentials, delivery settings, and API policy stay managed on this machine."
          />
          <MetricCard
            label="Operator Goal"
            value="Clarity"
            hint="Open one operational concern at a time and make changes with less cross-screen drift."
          />
        </div>

        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
          <div className="grid gap-4 md:grid-cols-3">
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
                Start Here
              </p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                Configure Accounts first, then route Notifications, then apply Proxy or Local API policy if your environment requires it.
              </p>
            </div>
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
                Change Scope
              </p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                Each page isolates one configuration domain so operators do not mix delivery, credentials, and network controls in one session.
              </p>
            </div>
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
                Recommended Order
              </p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                Validate one account locally, confirm notifications, then widen coverage across the rest of your cloud estate.
              </p>
            </div>
          </div>
        </div>

        {groups.map((group) => (
          <section key={group.title} className="space-y-4">
            <h2 className="text-sm font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
              {group.title}
            </h2>
            <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
              {group.items.map((item) => {
                const Icon = item.icon;
                const requiredEntitlement = requiredEntitlementForTab(item.id);
                const allowed = canAccessTabByPlan(item.id, runtimePlanType);
                const lockHint = !allowed && requiredEntitlement
                  ? (entitlementHintByTab[item.id] || `${requiredEntitlement} required`)
                  : "";
                return (
                  <button
                    key={item.id}
                    onClick={() => {
                      if (!allowed) {
                        onNavigate("license");
                        return;
                      }
                      onNavigate(item.id);
                    }}
                    aria-disabled={!allowed}
                    title={lockHint}
                    className={`rounded-2xl border p-6 text-left shadow-sm transition-all dark:border-slate-700 dark:bg-slate-800 ${
                      allowed
                        ? "border-slate-200 bg-white hover:-translate-y-0.5 hover:border-indigo-300 hover:shadow-md dark:hover:border-indigo-500/40"
                        : "cursor-not-allowed border-slate-200 bg-slate-100/70 opacity-70 dark:bg-slate-800/50"
                    }`}
                  >
                    <div className={`flex h-11 w-11 items-center justify-center rounded-2xl ${
                      allowed
                        ? "bg-indigo-50 text-indigo-600 dark:bg-indigo-500/15 dark:text-indigo-300"
                        : "bg-slate-200 text-slate-500 dark:bg-slate-700 dark:text-slate-300"
                    }`}>
                      <Icon className="h-5 w-5" />
                    </div>
                    <h3 className="mt-5 text-xl font-semibold text-slate-900 dark:text-white">{item.label}</h3>
                    <p className="mt-2 text-sm leading-6 text-slate-500 dark:text-slate-400">{item.description}</p>
                    {!allowed && (
                      <p className="mt-4 inline-flex rounded-full border border-amber-300 bg-amber-50 px-3 py-1 text-xs font-semibold text-amber-800 dark:border-amber-400/40 dark:bg-amber-500/10 dark:text-amber-200">
                        {lockHint || "Upgrade required"}
                      </p>
                    )}
                  </button>
                );
              })}
            </div>
          </section>
        ))}
    </PageShell>
  );
}
