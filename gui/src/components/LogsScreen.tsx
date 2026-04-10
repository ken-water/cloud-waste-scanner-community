import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronLeft, ChevronRight, Calendar } from "lucide-react";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";
import { MetricCard } from "./ui/MetricCard";

interface AuditLog {
  id: number;
  action: string;
  target: string;
  details: string;
  created_at: number;
}

interface LogsScreenProps {
  initialFilter?: any;
}

export function LogsScreen({ initialFilter }: LogsScreenProps) {
  const [logs, setLogs] = useState<AuditLog[]>([]);
  const [loadError, setLoadError] = useState("");
  const [page, setPage] = useState(1);
  const [date, setDate] = useState<Date | null>(null);
  const [dateInput, setDateInput] = useState("");
  const nativeDateInputRef = useRef<HTMLInputElement | null>(null);
  const [actionFilter, setActionFilter] = useState(initialFilter?.filter === 'cleanup' ? 'CLEANUP' : 'ALL');

  useEffect(() => {
      if (initialFilter?.filter === 'cleanup') {
          setActionFilter('CLEANUP');
      }
  }, [initialFilter]);

  useEffect(() => {
    fetchLogs();
  }, [page, date]);

  async function fetchLogs() {
    let from = null;
    let to = null;

    if (date) {
        const fromDate = new Date(date);
        const toDate = new Date(date);
        from = Math.floor(fromDate.setHours(0, 0, 0, 0) / 1000);
        to = Math.floor(toDate.setHours(23, 59, 59, 999) / 1000);
    }

    try {
      const data = await invoke<AuditLog[]>("get_audit_logs", { 
          dateFrom: from, 
          dateTo: to, 
          page 
      });
      setLogs(data);
      setLoadError("");
    } catch (e) {
      console.error(e);
      const errorText =
        typeof e === "string"
          ? e
          : e && typeof e === "object" && "message" in e
            ? String((e as { message?: unknown }).message || "")
            : "Failed to load audit logs.";
      setLoadError(errorText || "Failed to load audit logs.");
      setLogs([]);
    }
  }

  const filteredLogs = logs.filter(l => actionFilter === 'ALL' || l.action === actionFilter);
  const cleanupCount = filteredLogs.filter((log) => log.action === "CLEANUP").length;
  const scanCount = filteredLogs.filter((log) => log.action === "SCAN").length;
  const configCount = filteredLogs.filter((log) => log.action === "CONFIG").length;
  const latestTimestamp = filteredLogs[0]?.created_at ?? logs[0]?.created_at ?? null;
  const formatTimestamp = (unixTs: number | null) => {
    if (!unixTs) return "No activity yet";
    const d = new Date(unixTs * 1000);
    const pad = (n: number) => n.toString().padStart(2, '0');
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
  };

  const formatDateInputValue = (value: Date | null) => {
    if (!value) return "";
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${value.getFullYear()}-${pad(value.getMonth() + 1)}-${pad(value.getDate())}`;
  };

  const parseDateInput = (value: string): Date | null => {
    const normalized = value.trim();
    if (!normalized) return null;
    if (!/^\d{4}-\d{2}-\d{2}$/.test(normalized)) return null;
    const [y, m, d] = normalized.split("-").map(Number);
    const candidate = new Date(y, m - 1, d);
    if (
      Number.isNaN(candidate.getTime()) ||
      candidate.getFullYear() !== y ||
      candidate.getMonth() !== m - 1 ||
      candidate.getDate() !== d
    ) {
      return null;
    }
    return candidate;
  };

  return (
    <PageShell maxWidthClassName="max-w-none" className="space-y-6 pb-24 h-full min-h-screen flex flex-col dark:text-slate-100 transition-colors duration-300">
      <PageHeader
        title="Audit Log"
        subtitle="Track operator actions, cleanups, scans, and configuration changes."
        actions={
          <div className="flex items-center gap-4">
            <select 
              value={actionFilter}
              onChange={(e) => setActionFilter(e.target.value)}
              className="pl-3 pr-8 py-2 border border-slate-300 dark:border-slate-600 rounded-lg text-sm bg-white dark:bg-slate-700 text-slate-900 dark:text-white focus:ring-2 focus:ring-indigo-500 outline-none appearance-none"
            >
              <option value="ALL">All Actions</option>
              <option value="SCAN">Scans</option>
              <option value="CLEANUP">Cleanups</option>
              <option value="CONNECT">Connections</option>
              <option value="CONFIG">Config Changes</option>
            </select>
            <div className="relative">
              <Calendar className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400 z-10" />
              <input
                ref={nativeDateInputRef}
                type="date"
                value={formatDateInputValue(date)}
                max={formatDateInputValue(new Date())}
                onChange={(event) => {
                  const value = event.target.value;
                  const parsed = value ? new Date(`${value}T00:00:00`) : null;
                  setDate(parsed);
                  setDateInput(value);
                  setPage(1);
                }}
                className="sr-only"
                tabIndex={-1}
                aria-hidden="true"
              />
              <input
                type="text"
                inputMode="numeric"
                placeholder="YYYY-MM-DD"
                value={dateInput}
                onChange={(event) => {
                  const value = event.target.value;
                  setDateInput(value);
                  const parsed = parseDateInput(value);
                  setDate(parsed);
                  setPage(1);
                }}
                onBlur={() => {
                  const parsed = parseDateInput(dateInput);
                  if (!dateInput.trim()) {
                    setDate(null);
                    setDateInput("");
                    return;
                  }
                  if (parsed) {
                    const normalized = formatDateInputValue(parsed);
                    setDate(parsed);
                    setDateInput(normalized);
                  }
                }}
                className="w-48 rounded-lg border border-slate-300 bg-white py-2 pl-10 pr-10 text-sm text-slate-900 outline-none focus:ring-2 focus:ring-indigo-500 dark:border-slate-600 dark:bg-slate-700 dark:text-white"
              />
              <button
                type="button"
                onClick={() => {
                  const target = nativeDateInputRef.current;
                  if (!target) return;
                  if (typeof target.showPicker === "function") {
                    target.showPicker();
                  } else {
                    target.focus();
                    target.click();
                  }
                }}
                className="absolute right-2 top-1/2 -translate-y-1/2 rounded px-1.5 py-1 text-xs font-semibold text-slate-500 hover:bg-slate-100 hover:text-slate-700 dark:text-slate-300 dark:hover:bg-slate-600 dark:hover:text-white"
                title="Open calendar"
              >
                <Calendar className="h-4 w-4" />
              </button>
              {date && (
                <button
                  type="button"
                  onClick={() => {
                    setDate(null);
                    setDateInput("");
                    setPage(1);
                  }}
                  className="absolute right-9 top-1/2 -translate-y-1/2 rounded px-1.5 py-1 text-xs font-semibold text-slate-500 hover:bg-slate-100 hover:text-slate-700 dark:text-slate-300 dark:hover:bg-slate-600 dark:hover:text-white"
                >
                  Clear
                </button>
              )}
            </div>
          </div>
        }
      />

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label="Visible Rows"
          value={filteredLogs.length}
          hint="Rows after current action and date filters."
        />
        <MetricCard
          label="Scans"
          value={scanCount}
          hint="Visible scan runs in the current slice."
        />
        <MetricCard
          label="Changes"
          value={configCount}
          hint={`${cleanupCount} cleanup events visible in the current slice.`}
        />
        <MetricCard
          label="Last Event"
          value={latestTimestamp ? formatTimestamp(latestTimestamp).slice(11) : "None"}
          hint={formatTimestamp(latestTimestamp)}
        />
      </div>

      <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="grid gap-4 md:grid-cols-3">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">What This Tracks</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              Operator actions such as scans, cleanups, connection checks, and configuration changes.
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Best Pairing</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              Use Audit Log for accountability and use System Logs when you need runtime failure details.
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Current Slice</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              {actionFilter === "ALL" ? "Showing all action families." : `Focused on ${actionFilter.toLowerCase()} events.`} {date ? "A date filter is active." : "No date filter is active."}
            </p>
          </div>
        </div>
      </div>

      {loadError ? (
        <div className="rounded-xl border border-amber-300 bg-amber-50 px-4 py-3 text-sm font-medium text-amber-900 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-200">
          {loadError}
        </div>
      ) : null}

      <div className="flex-1 bg-white dark:bg-slate-800 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm overflow-hidden flex flex-col transition-colors">
        <div className="overflow-auto flex-1">
            <table className="w-full text-left text-sm">
                <thead className="bg-slate-50 dark:bg-slate-700/50 border-b border-slate-200 dark:border-slate-700 text-slate-500 dark:text-slate-400 font-semibold uppercase tracking-wider text-xs sticky top-0">
                    <tr>
                        <th className="px-6 py-3 w-48">Timestamp</th>
                        <th className="px-6 py-3 w-32">Action</th>
                        <th className="px-6 py-3 w-48">Target</th>
                        <th className="px-6 py-3">Details</th>
                    </tr>
                </thead>
                <tbody className="divide-y divide-slate-100 dark:divide-slate-700/50">
                    {filteredLogs.length === 0 && (
                        <tr><td colSpan={4} className="p-8 text-center text-slate-400 dark:text-slate-500">No logs found for this period.</td></tr>
                    )}
                    {filteredLogs.map(log => (
                        <tr key={log.id} className="hover:bg-slate-50 dark:hover:bg-slate-700/50 transition-colors">
                            <td className="px-6 py-3 text-slate-500 dark:text-slate-400 text-sm opacity-80 whitespace-nowrap">
                                {formatTimestamp(log.created_at)}
                            </td>
                            <td className="px-6 py-3">
                                <span className={`px-2 py-1 rounded text-xs font-bold uppercase ${
                                    log.action === 'CLEANUP' ? 'bg-red-100 text-red-700 dark:text-red-300 dark:bg-red-900/30 dark:text-red-400' : 
                                    log.action === 'SCAN' ? 'bg-indigo-100 text-indigo-700 dark:text-indigo-300 dark:bg-indigo-900/30 dark:text-indigo-400' :
                                    'bg-slate-100 text-slate-600 dark:bg-slate-700 dark:text-slate-300'
                                }`}>{log.action}</span>
                            </td>
                            <td className="px-6 py-3 font-medium text-slate-700 dark:text-slate-200 whitespace-nowrap">{log.target}</td>
                            <td className="px-6 py-3 text-slate-600 dark:text-slate-400">{log.details}</td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
        
        {/* Pagination Footer */}
        <div className="border-t border-slate-200 dark:border-slate-700 p-4 bg-slate-50 dark:bg-slate-700/30 flex justify-between items-center text-xs text-slate-500 dark:text-slate-400">
            <span>Showing page {page} (50 items/page)</span>
            <div className="flex gap-2">
                <button 
                    onClick={() => setPage(Math.max(1, page - 1))}
                    disabled={page === 1}
                    className="p-2 border border-slate-300 dark:border-slate-600 rounded hover:bg-white dark:hover:bg-slate-700 disabled:opacity-50 disabled:hover:bg-transparent transition-colors"
                >
                    <ChevronLeft className="w-4 h-4 text-slate-600 dark:text-slate-300" />
                </button>
                <button 
                    onClick={() => setPage(page + 1)}
                    disabled={logs.length < 50}
                    className="p-2 border border-slate-300 dark:border-slate-600 rounded hover:bg-white dark:hover:bg-slate-700 disabled:opacity-50 disabled:hover:bg-transparent transition-colors"
                >
                    <ChevronRight className="w-4 h-4 text-slate-600 dark:text-slate-300" />
                </button>
            </div>
        </div>
      </div>
    </PageShell>
  );
}
