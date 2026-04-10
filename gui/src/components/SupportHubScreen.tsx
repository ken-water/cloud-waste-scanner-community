import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ClipboardList, ExternalLink, FileText, LifeBuoy, MessageSquare, RefreshCw } from "lucide-react";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";
import { MetricCard } from "./ui/MetricCard";
import { exportTextWithTauriFallback, revealExportedFileInFolder } from "../utils/fileExport";
import { canAccessTabByPlan, readRuntimePlanTypeFromStorage } from "../lib/edition";

interface SupportHubScreenProps {
  onNavigate: (tab: string) => void;
}

interface SystemLogOverview {
  path: string;
  exists: boolean;
  size_bytes: number;
  updated_at: number | null;
  total_lines: number;
  error_lines: number;
}

interface AuditLog {
  id: number;
  action: string;
  target: string;
  details: string;
  created_at: number;
}

interface FeedbackHistoryItem {
  id: number;
  type: string;
  message: string;
  date: number;
  status: string;
}

interface SystemLogRecord {
  line_number: number;
  timestamp: string | null;
  level: string;
  area: string;
  event: string;
  message: string;
  raw: string;
}

interface SystemLogResponse {
  overview: SystemLogOverview;
  records: SystemLogRecord[];
}

interface SupportSnapshot {
  generated_at: number;
  app_version: string;
  runtime_plan_type?: string | null;
  license_present: boolean;
  system_log: SystemLogOverview;
  recent_log_records: SystemLogRecord[];
  audit_rows: number;
  feedback_records: number;
  settings: Record<string, string>;
}

export function SupportHubScreen({ onNavigate }: SupportHubScreenProps) {
  const runtimePlanType = readRuntimePlanTypeFromStorage();
  const canOpenAuditLog = canAccessTabByPlan("audit_log", runtimePlanType);
  const [auditCount, setAuditCount] = useState(0);
  const [systemLogOverview, setSystemLogOverview] = useState<SystemLogOverview | null>(null);
  const [feedbackCount, setFeedbackCount] = useState(0);
  const [lastFeedbackStatus, setLastFeedbackStatus] = useState("No local submissions");
  const [recentErrors, setRecentErrors] = useState<SystemLogRecord[]>([]);
  const [supportSnapshot, setSupportSnapshot] = useState<SupportSnapshot | null>(null);
  const [loading, setLoading] = useState(false);
  const [snapshotNotice, setSnapshotNotice] = useState("");
  const [auditGateNotice, setAuditGateNotice] = useState("");

  const load = async () => {
    setLoading(true);
    try {
      const [logOverviewRes, auditRowsRes, logResponseRes, snapshotRes] = await Promise.allSettled([
        invoke<SystemLogOverview>("get_system_log_overview"),
        invoke<AuditLog[]>("get_audit_logs", { page: 1 }),
        invoke<SystemLogResponse>("read_system_logs"),
        invoke<SupportSnapshot>("get_support_snapshot"),
      ]);

      if (logOverviewRes.status === "fulfilled") {
        setSystemLogOverview(logOverviewRes.value);
      }
      if (snapshotRes.status === "fulfilled") {
        setSupportSnapshot(snapshotRes.value);
      }
      if (auditRowsRes.status === "fulfilled") {
        setAuditCount(auditRowsRes.value.length);
        setAuditGateNotice("");
      } else {
        setAuditCount(0);
        const raw = String(auditRowsRes.reason || "");
        if (raw.toLowerCase().includes("enterprise")) {
          setAuditGateNotice("Audit Log requires Enterprise edition.");
        } else {
          setAuditGateNotice("Audit Log is currently unavailable.");
        }
      }
      const records =
        logResponseRes.status === "fulfilled" ? logResponseRes.value?.records || [] : [];
      const errorRows = records
        .filter((row) => row.level === "error" || row.level === "warn")
        .slice(0, 5);
      setRecentErrors(errorRows);
    } catch (err) {
      console.warn("Support hub refresh failed", err);
    } finally {
      try {
        const raw = localStorage.getItem("cws_feedback_history") || "[]";
        const parsed = JSON.parse(raw) as FeedbackHistoryItem[];
        setFeedbackCount(parsed.length);
        if (parsed[0]?.status) {
          setLastFeedbackStatus(parsed[0].status);
        }
      } catch {
        setFeedbackCount(0);
        setLastFeedbackStatus("No local submissions");
      }
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const formatUpdatedAt = (unixTs: number | null | undefined) => {
    if (!unixTs) return "Not written yet";
    return new Date(unixTs * 1000).toLocaleString();
  };

  const latestFeedbackLabel =
    lastFeedbackStatus && lastFeedbackStatus !== "No local submissions"
      ? lastFeedbackStatus.replace(/_/g, " ")
      : "No local submissions";

  const latestErrorLabel = useMemo(() => {
    if (!recentErrors.length) return "No recent warning or error lines";
    const top = recentErrors[0];
    return `${top.level.toUpperCase()} · ${top.area || "general"}`;
  }, [recentErrors]);

  const buildSupportSnapshot = () => {
    return [
      `Cloud Waste Scanner Support Snapshot`,
      `Generated: ${new Date().toLocaleString()}`,
      "",
      `App Version: ${supportSnapshot?.app_version || "-"}`,
      `Runtime Plan: ${supportSnapshot?.runtime_plan_type || "-"}`,
      `Local License Present: ${supportSnapshot?.license_present ? "yes" : "no"}`,
      `Audit Rows: ${auditCount}`,
      `Feedback Records: ${feedbackCount}`,
      `Latest Feedback Status: ${lastFeedbackStatus}`,
      `System Log Exists: ${systemLogOverview?.exists ? "yes" : "no"}`,
      `System Log Path: ${systemLogOverview?.path || "-"}`,
      `System Log Updated: ${formatUpdatedAt(systemLogOverview?.updated_at)}`,
      `System Log Total Lines: ${systemLogOverview?.total_lines ?? 0}`,
      `System Log Error Lines: ${systemLogOverview?.error_lines ?? 0}`,
      `API Bind Host: ${supportSnapshot?.settings?.api_bind_host || "-"}`,
      `API Port: ${supportSnapshot?.settings?.api_port || "-"}`,
      `API TLS Enabled: ${supportSnapshot?.settings?.api_tls_enabled || "-"}`,
      `Proxy Mode: ${supportSnapshot?.settings?.proxy_mode || "-"}`,
      `Notification Trigger Mode: ${supportSnapshot?.settings?.notification_trigger_mode || "-"}`,
      "",
      `Recent Runtime Signals:`,
      ...((supportSnapshot?.recent_log_records?.length ? supportSnapshot.recent_log_records : recentErrors).length
        ? (supportSnapshot?.recent_log_records?.length ? supportSnapshot.recent_log_records : recentErrors).map((row) => `- ${row.timestamp || `line ${row.line_number}`} | ${row.level.toUpperCase()} | ${row.area || "general"} | ${row.message || row.raw}`)
        : ["- No recent warning or error lines"]),
    ].join("\n");
  };

  const copySupportSnapshot = async () => {
    try {
      await navigator.clipboard.writeText(buildSupportSnapshot());
      setSnapshotNotice("Support snapshot copied.");
    } catch {
      setSnapshotNotice("Copy failed. Please allow clipboard permissions.");
    }
    window.setTimeout(() => setSnapshotNotice(""), 2200);
  };

  const exportSupportSnapshot = async () => {
    const savedPath = await exportTextWithTauriFallback(
      buildSupportSnapshot(),
      "cws-support-snapshot.txt",
      "text/plain;charset=utf-8;",
      { openAfterSave: false }
    );
    if (savedPath) {
      await revealExportedFileInFolder(savedPath);
    }
  };

  const exportRecentErrors = async () => {
    const lines = recentErrors.map((row) => row.raw).join("\n");
    if (!lines.trim()) return;
    const savedPath = await exportTextWithTauriFallback(lines, "cws-support-errors.txt", "text/plain;charset=utf-8;", { openAfterSave: false });
    if (savedPath) {
      await revealExportedFileInFolder(savedPath);
    }
  };

  const exportRecentErrorsCsv = async () => {
    if (!recentErrors.length) return;
    const lines = [
      ["line_number", "timestamp", "level", "area", "event", "message"],
      ...recentErrors.map((row) => [
        String(row.line_number),
        row.timestamp || "",
        row.level,
        row.area,
        row.event,
        row.message.replace(/"/g, '""'),
      ]),
    ]
      .map((row) => row.map((cell) => /[",\n]/.test(cell) ? `"${cell}"` : cell).join(","))
      .join("\n");
    const savedPath = await exportTextWithTauriFallback(`\uFEFF${lines}`, "cws-support-errors.csv", "text/csv;charset=utf-8;", { openAfterSave: false });
    if (savedPath) {
      await revealExportedFileInFolder(savedPath);
    }
  };

  return (
    <PageShell maxWidthClassName="max-w-6xl">
        <PageHeader
          title="Support Center"
          subtitle="Use the right support surface for operator actions, runtime diagnostics, and product requests."
          icon={<LifeBuoy className="h-6 w-6" />}
          actions={
            <div className="flex flex-wrap items-center gap-2">
              <button
                onClick={load}
                className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-2.5 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
                Refresh
              </button>
              <button
                onClick={() => void copySupportSnapshot()}
                className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-2.5 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <ClipboardList className="h-4 w-4" />
                Copy Snapshot
              </button>
              <button
                onClick={() => void exportSupportSnapshot()}
                className="inline-flex items-center gap-2 rounded-xl bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-indigo-700"
              >
                <FileText className="h-4 w-4" />
                Export Snapshot
              </button>
            </div>
          }
        />
        {snapshotNotice ? (
          <div className="rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-2 text-sm font-medium text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-300">
            {snapshotNotice}
          </div>
        ) : null}
        {auditGateNotice ? (
          <div className="rounded-xl border border-amber-300 bg-amber-50 px-4 py-2 text-sm font-medium text-amber-900 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-200">
            {auditGateNotice}
          </div>
        ) : null}

        <div className="grid gap-4 md:grid-cols-3">
          <MetricCard
            label="Audit Rows"
            value={auditCount}
            hint={canOpenAuditLog ? "Most recent operator events available from the local audit store." : "Enterprise edition required for Audit Log."}
            icon={<ClipboardList className="h-5 w-5" />}
          />
          <MetricCard
            label="System Log Errors"
            value={systemLogOverview?.error_lines ?? 0}
            hint={`${latestErrorLabel} · Updated ${formatUpdatedAt(systemLogOverview?.updated_at)}`}
            icon={<FileText className="h-5 w-5" />}
          />
          <MetricCard
            label="Feedback Records"
            value={feedbackCount}
            hint={`Latest local status: ${lastFeedbackStatus}`}
            icon={<MessageSquare className="h-5 w-5" />}
          />
        </div>

        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
          <div className="grid gap-4 md:grid-cols-3">
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
                Investigation Flow
              </p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                {canOpenAuditLog
                  ? "Start with System Logs for runtime failures, then use Audit Log to confirm the operator action trail."
                  : "Start with System Logs for runtime failures. Audit Log trails are available on Enterprise edition."}
              </p>
            </div>
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
                Product Loop
              </p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                Use Feedback only for product gaps or workflow friction that should be tracked beyond one machine.
              </p>
            </div>
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
                Latest Local Status
              </p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                Feedback lane: {latestFeedbackLabel}. Log file updated: {formatUpdatedAt(systemLogOverview?.updated_at)}.
              </p>
            </div>
          </div>
        </div>

        <div className="grid gap-4 lg:grid-cols-3">
          <button
            onClick={() => {
              if (!canOpenAuditLog) {
                onNavigate("license");
                return;
              }
              onNavigate("audit_log");
            }}
            aria-disabled={!canOpenAuditLog}
            title={canOpenAuditLog ? "" : "Enterprise edition required"}
            className={`rounded-2xl border p-6 text-left shadow-sm transition-all dark:border-slate-700 dark:bg-slate-800 ${
              canOpenAuditLog
                ? "border-slate-200 bg-white hover:-translate-y-0.5 hover:border-indigo-300 hover:shadow-md dark:hover:border-indigo-500/40"
                : "cursor-not-allowed border-slate-200 bg-slate-100/70 opacity-70 dark:bg-slate-800/50"
            }`}
          >
            <div className={`flex h-11 w-11 items-center justify-center rounded-2xl ${
              canOpenAuditLog
                ? "bg-indigo-50 text-indigo-600 dark:bg-indigo-500/15 dark:text-indigo-300"
                : "bg-slate-200 text-slate-500 dark:bg-slate-700 dark:text-slate-300"
            }`}>
              <ClipboardList className="h-5 w-5" />
            </div>
            <h3 className="mt-5 text-xl font-semibold text-slate-900 dark:text-white">Audit Log</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500 dark:text-slate-400">
              {canOpenAuditLog
                ? "Review who ran scans, triggered cleanups, or changed configuration on this machine."
                : "Enterprise edition required to review operator audit trails."}
            </p>
          </button>

          <button
            onClick={() => onNavigate("system_logs")}
            className="rounded-2xl border border-slate-200 bg-white p-6 text-left shadow-sm transition-all hover:-translate-y-0.5 hover:border-indigo-300 hover:shadow-md dark:border-slate-700 dark:bg-slate-800 dark:hover:border-indigo-500/40"
          >
            <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-indigo-50 text-indigo-600 dark:bg-indigo-500/15 dark:text-indigo-300">
              <FileText className="h-5 w-5" />
            </div>
            <h3 className="mt-5 text-xl font-semibold text-slate-900 dark:text-white">System Logs</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500 dark:text-slate-400">
              Inspect `cws.log` for startup failures, proxy problems, update issues, and runtime exits.
            </p>
          </button>

          <button
            onClick={() => onNavigate("feedback")}
            className="rounded-2xl border border-slate-200 bg-white p-6 text-left shadow-sm transition-all hover:-translate-y-0.5 hover:border-indigo-300 hover:shadow-md dark:border-slate-700 dark:bg-slate-800 dark:hover:border-indigo-500/40"
          >
            <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-indigo-50 text-indigo-600 dark:bg-indigo-500/15 dark:text-indigo-300">
              <MessageSquare className="h-5 w-5" />
            </div>
            <h3 className="mt-5 text-xl font-semibold text-slate-900 dark:text-white">Feedback</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500 dark:text-slate-400">
              Submit product gaps, workflow friction, and feature requests with local submission history.
            </p>
          </button>
        </div>

        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
          <div className="flex flex-wrap items-start justify-between gap-4 border-b border-slate-200 pb-4 dark:border-slate-700">
            <div>
              <h3 className="text-lg font-semibold text-slate-900 dark:text-white">Recent Runtime Signals</h3>
              <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
                Latest warning and error lines from `cws.log`. Use this preview before opening the full system log screen.
              </p>
            </div>
            <div className="flex flex-wrap gap-2">
              <button
                onClick={() => onNavigate("system_logs")}
                className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm font-semibold text-slate-700 transition hover:bg-slate-50 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
              >
                <ExternalLink className="h-4 w-4" />
                Open System Logs
              </button>
              <button
                onClick={() => void exportRecentErrors()}
                disabled={!recentErrors.length}
                className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm font-semibold text-slate-700 transition hover:bg-slate-50 disabled:opacity-40 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
              >
                <FileText className="h-4 w-4" />
                Export TXT
              </button>
              <button
                onClick={() => void exportRecentErrorsCsv()}
                disabled={!recentErrors.length}
                className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm font-semibold text-slate-700 transition hover:bg-slate-50 disabled:opacity-40 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
              >
                <FileText className="h-4 w-4" />
                Export CSV
              </button>
            </div>
          </div>
          <div className="mt-4 space-y-3">
            {!recentErrors.length ? (
              <p className="text-sm text-slate-500 dark:text-slate-400">No recent warning or error lines found in the active system log.</p>
            ) : recentErrors.map((row) => (
              <div key={row.line_number} className="rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 dark:border-slate-700 dark:bg-slate-900/50">
                <div className="flex flex-wrap items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em]">
                  <span className={`${row.level === "error" ? "text-rose-600 dark:text-rose-300" : "text-amber-600 dark:text-amber-300"}`}>
                    {row.level}
                  </span>
                  <span className="text-slate-400 dark:text-slate-500">{row.area || "general"}</span>
                  <span className="text-slate-400 dark:text-slate-500">{row.timestamp || `line ${row.line_number}`}</span>
                </div>
                <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">{row.message || row.raw}</p>
              </div>
            ))}
          </div>
        </div>
    </PageShell>
  );
}
