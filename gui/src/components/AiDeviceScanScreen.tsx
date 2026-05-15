import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Bot, Play, RefreshCw } from "lucide-react";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";

type Finding = Record<string, any>;

export function AiDeviceScanScreen() {
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string>("");
  const [results, setResults] = useState<Finding[]>([]);
  const [lastRunAt, setLastRunAt] = useState<string>("");

  const aiFindings = useMemo(
    () =>
      results.filter((item) => {
        const p = String(item?.provider || "").toLowerCase();
        return p.includes("ai-runtime") || p.includes("ai");
      }),
    [results]
  );

  const runAiScan = async () => {
    setRunning(true);
    setError("");
    try {
      const scanResults = await invoke<Finding[]>("run_scan", {
        licenseKey: null,
        awsProfile: null,
        awsRegion: null,
        selectedAccounts: [],
        demoMode: false,
        includeKubernetes: false,
        kubeconfigPath: null,
        kubeContext: null,
        includeAiRuntime: true,
      });
      setResults(scanResults || []);
      setLastRunAt(new Date().toLocaleString());
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <PageShell maxWidthClassName="max-w-6xl" className="space-y-6">
      <PageHeader
        title="AI Device Utilization Scan"
        subtitle="Local-first GPU runtime probe for utilization, memory pressure, and power efficiency signals."
        icon={<Bot className="h-6 w-6" />}
        actions={
          <button
            onClick={runAiScan}
            disabled={running}
            className="inline-flex items-center gap-2 rounded-xl bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-indigo-700 disabled:opacity-60"
          >
            {running ? <RefreshCw className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
            {running ? "Scanning..." : "Run AI Device Scan"}
          </button>
        }
      />

      {error ? (
        <div className="rounded-xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-200">
          {error}
        </div>
      ) : null}

      <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="flex items-center justify-between">
          <p className="text-sm font-semibold text-slate-900 dark:text-white">Latest AI Runtime Findings</p>
          <p className="text-xs text-slate-500 dark:text-slate-400">{lastRunAt ? `Last run: ${lastRunAt}` : "No scan executed yet"}</p>
        </div>
        <div className="mt-4 space-y-3">
          {aiFindings.length === 0 ? (
            <p className="text-sm text-slate-500 dark:text-slate-400">Run AI device scan to collect local GPU utilization findings.</p>
          ) : (
            aiFindings.slice(0, 30).map((item, idx) => (
              <div key={`${item?.resource_id || item?.resource_name || "ai"}-${idx}`} className="rounded-xl border border-slate-200 bg-slate-50 p-3 dark:border-slate-700 dark:bg-slate-900/40">
                <p className="text-sm font-semibold text-slate-900 dark:text-white">
                  {item?.resource_name || item?.resource_id || "GPU Device"}
                </p>
                <p className="mt-1 text-xs text-slate-600 dark:text-slate-300">{item?.message || item?.recommendation || "AI runtime governance signal detected."}</p>
              </div>
            ))
          )}
        </div>
      </div>
    </PageShell>
  );
}

