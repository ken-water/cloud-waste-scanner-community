import { useEffect, useState } from "react";
import { Bot, Database, Globe2, Save, ShieldCheck } from "lucide-react";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";
import { MetricCard } from "./ui/MetricCard";

const STORAGE_KEYS = {
  mode: "cws_ai_mode",
  endpoint: "cws_ai_endpoint",
  model: "cws_ai_model",
  apiKey: "cws_ai_api_key",
  external: "cws_ai_allow_external",
};

export function AiSettingsScreen() {
  const [mode, setMode] = useState("local_summary_only");
  const [endpoint, setEndpoint] = useState("");
  const [model, setModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [allowExternal, setAllowExternal] = useState(false);
  const [savedNotice, setSavedNotice] = useState("");

  useEffect(() => {
    setMode(localStorage.getItem(STORAGE_KEYS.mode) || "local_summary_only");
    setEndpoint(localStorage.getItem(STORAGE_KEYS.endpoint) || "");
    setModel(localStorage.getItem(STORAGE_KEYS.model) || "");
    setApiKey(localStorage.getItem(STORAGE_KEYS.apiKey) || "");
    setAllowExternal(localStorage.getItem(STORAGE_KEYS.external) === "1");
  }, []);

  const save = () => {
    localStorage.setItem(STORAGE_KEYS.mode, mode);
    localStorage.setItem(STORAGE_KEYS.endpoint, endpoint.trim());
    localStorage.setItem(STORAGE_KEYS.model, model.trim());
    localStorage.setItem(STORAGE_KEYS.apiKey, apiKey.trim());
    localStorage.setItem(STORAGE_KEYS.external, allowExternal ? "1" : "0");
    setSavedNotice("AI settings saved locally for this operator machine.");
    window.setTimeout(() => setSavedNotice(""), 3000);
  };

  return (
      <PageShell maxWidthClassName="max-w-6xl" className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-300 transition-colors dark:text-slate-100">
        <PageHeader
          title="AI Runtime Settings"
          subtitle="Define how local AI device utilization scanning should behave. Current runtime scan and recommendations are local-first and do not require external models."
          icon={<Bot className="h-6 w-6" />}
          actions={
            <button
              onClick={save}
              className="inline-flex items-center gap-2 rounded-xl bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-indigo-700"
            >
              <Save className="h-4 w-4" />
              Save
            </button>
          }
        />

        <div className="grid gap-4 md:grid-cols-3">
          <MetricCard
            label="Current Mode"
            value={mode === "local_summary_only" ? "Local Only" : "Hybrid"}
            hint="Operator-facing wording can evolve later; statistics remain locally computed first."
            icon={<ShieldCheck className="h-5 w-5" />}
          />
          <MetricCard
            label="Credential Posture"
            value="Never Sent"
            hint="Cloud credentials and raw findings stay on this machine."
            icon={<Database className="h-5 w-5" />}
          />
          <MetricCard
            label="External Model"
            value={allowExternal ? "Allowed" : "Disabled"}
            hint="External models are optional and not yet required for current AI Analyst workflows."
            icon={<Globe2 className="h-5 w-5" />}
          />
        </div>

        <div className="rounded-2xl border border-slate-200 bg-white p-6 shadow-sm dark:border-slate-700 dark:bg-slate-800">
          <div className="grid gap-6 md:grid-cols-2">
            <label className="space-y-2">
              <span className="text-sm font-semibold text-slate-900 dark:text-white">AI Operating Mode</span>
              <select
                value={mode}
                onChange={(event) => setMode(event.target.value)}
                className="w-full rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-900 outline-none focus:border-indigo-400 dark:border-slate-600 dark:bg-slate-900 dark:text-white"
              >
                <option value="local_summary_only">Local Summary Only</option>
                <option value="hybrid_wording">Hybrid Wording Layer</option>
              </select>
              <p className="text-sm text-slate-500 dark:text-slate-400">
                `Local Summary Only` keeps all reasoning and wording in-app. `Hybrid Wording Layer` reserves a path for future OpenAI-compatible wording help.
              </p>
            </label>

            <label className="space-y-2">
              <span className="text-sm font-semibold text-slate-900 dark:text-white">Model Name</span>
              <input
                value={model}
                onChange={(event) => setModel(event.target.value)}
                placeholder="e.g. gpt-4.1-mini or local-model-name"
                className="w-full rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-900 outline-none focus:border-indigo-400 dark:border-slate-600 dark:bg-slate-900 dark:text-white"
              />
              <p className="text-sm text-slate-500 dark:text-slate-400">
                Stored locally now. This is preparation for a later model integration pass.
              </p>
            </label>

            <label className="space-y-2">
              <span className="text-sm font-semibold text-slate-900 dark:text-white">API Key / Token</span>
              <input
                value={apiKey}
                onChange={(event) => setApiKey(event.target.value)}
                type="password"
                placeholder="Stored locally only"
                className="w-full rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-900 outline-none focus:border-indigo-400 dark:border-slate-600 dark:bg-slate-900 dark:text-white"
              />
              <p className="text-sm text-slate-500 dark:text-slate-400">
                Reserved for a later wording-assistant integration. This value stays in local browser storage on this operator machine.
              </p>
            </label>

            <label className="space-y-2 md:col-span-2">
              <span className="text-sm font-semibold text-slate-900 dark:text-white">OpenAI-Compatible Endpoint</span>
              <input
                value={endpoint}
                onChange={(event) => setEndpoint(event.target.value)}
                placeholder="https://your-endpoint.example/v1"
                className="w-full rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-900 outline-none focus:border-indigo-400 dark:border-slate-600 dark:bg-slate-900 dark:text-white"
              />
              <p className="text-sm text-slate-500 dark:text-slate-400">
                Not active yet. When enabled later, only aggregated summaries should be sent, never cloud credentials or raw database dumps.
              </p>
            </label>

            <label className="flex items-start gap-3 md:col-span-2">
              <input
                type="checkbox"
                checked={allowExternal}
                onChange={(event) => setAllowExternal(event.target.checked)}
                className="mt-1 h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-500"
              />
              <span>
                <span className="block text-sm font-semibold text-slate-900 dark:text-white">Allow future external wording models</span>
                <span className="mt-1 block text-sm text-slate-500 dark:text-slate-400">
                  This does not activate any network call today. It only records operator policy for a later integration pass.
                </span>
              </span>
            </label>
          </div>
        </div>

        {savedNotice ? (
          <p className="text-sm font-medium text-emerald-600 dark:text-emerald-400">{savedNotice}</p>
        ) : null}
      </PageShell>
  );
}
