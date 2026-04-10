import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, ChevronLeft, ChevronRight, Copy, ExternalLink, FileText, FolderOpen, RefreshCw } from "lucide-react";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";
import { MetricCard } from "./ui/MetricCard";
import { exportTextWithTauriFallback } from "../utils/fileExport";
import { canAccessTabByPlan, readRuntimePlanTypeFromStorage } from "../lib/edition";

interface SystemLogOverview {
  path: string;
  exists: boolean;
  size_bytes: number;
  updated_at: number | null;
  total_lines: number;
  error_lines: number;
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

const levelOptions = ["all", "error", "warn", "info", "debug"];
const PAGE_SIZE = 100;

export function SystemLogsScreen() {
  const canOpenAuditLog = canAccessTabByPlan("audit_log", readRuntimePlanTypeFromStorage());
  const [response, setResponse] = useState<SystemLogResponse | null>(null);
  const [query, setQuery] = useState("");
  const [level, setLevel] = useState("all");
  const [area, setArea] = useState("all");
  const [page, setPage] = useState(1);
  const [selectedRows, setSelectedRows] = useState<number[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<SystemLogResponse>("read_system_logs");
      setResponse(data);
      setSelectedRows([]);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const areaOptions = useMemo(() => {
    const values = new Set<string>();
    for (const record of response?.records ?? []) {
      if (record.area && record.area !== "general") {
        values.add(record.area);
      }
    }
    return ["all", ...Array.from(values).sort()];
  }, [response]);

  const filteredRecords = useMemo(() => {
    const matched = (response?.records ?? []).filter((record) => {
      if (level !== "all" && record.level !== level) return false;
      if (area !== "all" && record.area !== area) return false;
      if (!query.trim()) return true;
      const haystack = `${record.timestamp ?? ""} ${record.level} ${record.area} ${record.event} ${record.message} ${record.raw}`.toLowerCase();
      return haystack.includes(query.trim().toLowerCase());
    });
    return matched.sort((a, b) => b.line_number - a.line_number);
  }, [area, level, query, response]);

  const totalPages = Math.max(1, Math.ceil(filteredRecords.length / PAGE_SIZE));
  const currentPage = Math.min(page, totalPages);
  const pagedRecords = useMemo(() => {
    const start = (currentPage - 1) * PAGE_SIZE;
    return filteredRecords.slice(start, start + PAGE_SIZE);
  }, [currentPage, filteredRecords]);
  const selectedRecords = pagedRecords.filter((record) => selectedRows.includes(record.line_number));
  const visibleRowCount = pagedRecords.length;
  const filteredRowCount = filteredRecords.length;
  const selectedRowCount = selectedRecords.length;
  const visibleErrorCount = filteredRecords.filter((record) => record.level === "error").length;
  const visibleWarnCount = filteredRecords.filter((record) => record.level === "warn").length;
  const visibleInfoCount = filteredRecords.filter((record) => record.level === "info").length;

  useEffect(() => {
    setPage(1);
    setSelectedRows([]);
  }, [query, level, area, response]);

  useEffect(() => {
    if (page > totalPages) {
      setPage(totalPages);
    }
  }, [page, totalPages]);

  const toggleRow = (lineNumber: number) => {
    setSelectedRows((current) =>
      current.includes(lineNumber) ? current.filter((value) => value !== lineNumber) : [...current, lineNumber]
    );
  };

  const selectAllVisible = () => {
    if (pagedRecords.length === 0) {
      setSelectedRows([]);
      return;
    }
    if (selectedRecords.length === pagedRecords.length) {
      setSelectedRows([]);
      return;
    }
    setSelectedRows(pagedRecords.map((record) => record.line_number));
  };

  const copySelected = async () => {
    const lines = (selectedRecords.length > 0 ? selectedRecords : pagedRecords).map((record) => record.raw).join("\n");
    await navigator.clipboard.writeText(lines);
  };

  const exportLogs = async () => {
    const lines = (selectedRecords.length > 0 ? selectedRecords : pagedRecords).map((record) => record.raw).join("\n");
    await exportTextWithTauriFallback(lines, "cws-system-logs.txt", "text/plain;charset=utf-8;");
  };

  const exportLogsCsv = async () => {
    const rows = selectedRecords.length > 0 ? selectedRecords : pagedRecords;
    const csv = [
      ["line_number", "timestamp", "level", "area", "event", "message"],
      ...rows.map((record) => [
        String(record.line_number),
        record.timestamp || "",
        record.level,
        record.area,
        record.event,
        record.message.replace(/"/g, '""'),
      ]),
    ]
      .map((row) => row.map((cell) => /[",\n]/.test(cell) ? `"${cell}"` : cell).join(","))
      .join("\n");
    await exportTextWithTauriFallback(`\uFEFF${csv}`, "cws-system-logs.csv", "text/csv;charset=utf-8;");
  };

  const openFolder = async () => {
    await invoke("open_system_log_location");
  };

  const openFile = async () => {
    await invoke("open_system_log_file");
  };

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatUpdatedAt = (unixTs: number | null) => {
    if (!unixTs) return "Not written yet";
    return new Date(unixTs * 1000).toLocaleString();
  };

  return (
    <PageShell maxWidthClassName="max-w-none">
        <PageHeader
          title="System Logs"
          subtitle="Inspect `cws.log` without leaving the app. This is the first place to check startup failures, proxy issues, and runtime exits."
          icon={<FileText className="h-6 w-6" />}
          actions={
            <>
              <button
                onClick={load}
                className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-2.5 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
                Refresh
              </button>
              <button
                onClick={openFolder}
                className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-2.5 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <FolderOpen className="h-4 w-4" />
                Open Location
              </button>
              <button
                onClick={openFile}
                className="inline-flex items-center gap-2 rounded-xl bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-indigo-700"
              >
                <ExternalLink className="h-4 w-4" />
                Open Raw File
              </button>
            </>
          }
        />

        {error ? (
          <div className="rounded-2xl border border-red-200 bg-red-50 p-4 text-red-800 dark:border-red-900/60 dark:bg-red-950/30 dark:text-red-300">
            <div className="flex items-start gap-3">
              <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0" />
              <div>
                <p className="font-semibold">Unable to load system logs</p>
                <p className="mt-1 text-sm">{error}</p>
              </div>
            </div>
          </div>
        ) : null}

        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <MetricCard label="Log File" value={response?.overview.exists ? "Present" : "Missing"} hint={response?.overview.path ?? "No log path resolved"} />
          <MetricCard label="File Size" value={formatSize(response?.overview.size_bytes ?? 0)} hint="Current active log size" />
          <MetricCard label="Entries" value={response?.overview.total_lines ?? 0} hint="Parsed lines in the active log file" />
          <MetricCard label="Error Lines" value={response?.overview.error_lines ?? 0} hint={`Last updated ${formatUpdatedAt(response?.overview.updated_at ?? null)}`} />
        </div>

        <div className="flex flex-wrap gap-2">
          {[
            { id: "all", label: "All", count: visibleRowCount },
            { id: "error", label: "Errors", count: visibleErrorCount },
            { id: "warn", label: "Warnings", count: visibleWarnCount },
            { id: "info", label: "Info", count: visibleInfoCount },
          ].map((item) => (
            <button
              key={item.id}
              onClick={() => setLevel(item.id)}
              className={`rounded-full px-3 py-1.5 text-xs font-semibold transition ${
                level === item.id
                  ? "bg-indigo-600 text-white"
                  : "border border-slate-200 bg-white text-slate-600 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300 dark:hover:bg-slate-700"
              }`}
            >
              {item.label} {item.count}
            </button>
          ))}
        </div>

        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
          <div className="grid gap-4 md:grid-cols-3">
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Best Use</p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                Start here when startup, proxy, update, or runtime behavior looks wrong and you need the raw machine trail.
              </p>
            </div>
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Current Slice</p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                {visibleRowCount} visible rows, {visibleErrorCount} errors, {visibleWarnCount} warnings. {selectedRowCount > 0 ? `${selectedRowCount} selected for copy/export.` : "No rows selected yet."}
              </p>
            </div>
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Operator Workflow</p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                {canOpenAuditLog
                  ? "Filter by area or level, copy the relevant lines, then move to Audit Log if you need operator accountability around the same event."
                  : "Filter by area or level, and copy relevant lines for investigation. Operator audit trails require Enterprise edition."}
              </p>
            </div>
          </div>
        </div>

        <div className="rounded-2xl border border-slate-200 bg-white shadow-sm dark:border-slate-700 dark:bg-slate-800">
          <div className="border-b border-slate-200 p-5 dark:border-slate-700">
            <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_180px_180px_auto_auto_auto]">
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Search by message, area, event, or raw line"
                className="rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-900 outline-none ring-0 transition focus:border-indigo-400 dark:border-slate-600 dark:bg-slate-900 dark:text-white"
              />
              <select
                value={level}
                onChange={(event) => setLevel(event.target.value)}
                className="rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-900 outline-none transition focus:border-indigo-400 dark:border-slate-600 dark:bg-slate-900 dark:text-white"
              >
                {levelOptions.map((option) => (
                  <option key={option} value={option}>
                    {option === "all" ? "All Levels" : option.toUpperCase()}
                  </option>
                ))}
              </select>
              <select
                value={area}
                onChange={(event) => setArea(event.target.value)}
                className="rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-900 outline-none transition focus:border-indigo-400 dark:border-slate-600 dark:bg-slate-900 dark:text-white"
              >
                {areaOptions.map((option) => (
                  <option key={option} value={option}>
                    {option === "all" ? "All Areas" : option}
                  </option>
                ))}
              </select>
              <button
                onClick={copySelected}
                className="inline-flex items-center justify-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <Copy className="h-4 w-4" />
                Copy Selected
              </button>
              <button
                onClick={exportLogs}
                className="inline-flex items-center justify-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <FileText className="h-4 w-4" />
                Export TXT
              </button>
              <button
                onClick={exportLogsCsv}
                className="inline-flex items-center justify-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <FileText className="h-4 w-4" />
                Export CSV
              </button>
            </div>
            <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-slate-500 dark:text-slate-400">
              <span className="inline-flex items-center rounded-full bg-slate-100 px-2.5 py-1 font-semibold dark:bg-slate-700">
                Page Rows {visibleRowCount}
              </span>
              <span className="inline-flex items-center rounded-full bg-slate-100 px-2.5 py-1 font-semibold dark:bg-slate-700">
                Matched {filteredRowCount}
              </span>
              <span className="inline-flex items-center rounded-full bg-slate-100 px-2.5 py-1 font-semibold dark:bg-slate-700">
                Selected {selectedRowCount}
              </span>
              <span>
                Copy and export use selected rows first; if nothing is selected, they use the current page.
              </span>
            </div>
          </div>

          <div className="max-h-[calc(100vh-21rem)] overflow-auto">
            <table className="min-w-full table-fixed text-left text-sm">
              <thead className="sticky top-0 bg-slate-50 dark:bg-slate-900/95">
                <tr className="border-b border-slate-200 text-xs uppercase tracking-[0.18em] text-slate-500 dark:border-slate-700 dark:text-slate-400">
                  <th className="w-12 px-4 py-3">
                    <input
                      type="checkbox"
                      checked={pagedRecords.length > 0 && selectedRecords.length === pagedRecords.length}
                      onChange={selectAllVisible}
                    />
                  </th>
                  <th className="w-52 px-4 py-3">Time</th>
                  <th className="w-24 px-4 py-3">Level</th>
                  <th className="w-36 px-4 py-3">Area</th>
                  <th className="w-56 px-4 py-3">Event</th>
                  <th className="px-4 py-3">Message</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100 dark:divide-slate-700/70">
                {pagedRecords.length === 0 ? (
                  <tr>
                    <td colSpan={6} className="px-4 py-10 text-center text-slate-500 dark:text-slate-400">
                      No log lines matched the current filters.
                    </td>
                  </tr>
                ) : (
                  pagedRecords.map((record) => (
                    <tr key={record.line_number} className="align-top hover:bg-slate-50 dark:hover:bg-slate-900/60">
                      <td className="px-4 py-4">
                        <input
                          type="checkbox"
                          checked={selectedRows.includes(record.line_number)}
                          onChange={() => toggleRow(record.line_number)}
                        />
                      </td>
                      <td className="px-4 py-4 whitespace-nowrap text-slate-500 dark:text-slate-400">
                        {record.timestamp ?? "-"}
                      </td>
                      <td className="px-4 py-4">
                        <span className={`rounded-full px-2.5 py-1 text-xs font-semibold uppercase ${
                          record.level === "error"
                            ? "bg-rose-100 text-rose-700 dark:bg-rose-900/30 dark:text-rose-300"
                            : record.level === "warn"
                              ? "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300"
                              : "bg-slate-100 text-slate-700 dark:bg-slate-700 dark:text-slate-200"
                        }`}>
                          {record.level}
                        </span>
                      </td>
                      <td className="px-4 py-4 font-medium text-slate-700 dark:text-slate-200">{record.area}</td>
                      <td className="px-4 py-4 whitespace-pre-wrap break-words text-slate-600 dark:text-slate-300">{record.event}</td>
                      <td className="px-4 py-4 whitespace-pre-wrap break-words font-mono text-xs leading-5 text-slate-600 dark:text-slate-300">{record.message}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>

          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-slate-200 px-4 py-3 text-xs text-slate-500 dark:border-slate-700 dark:text-slate-400">
            <span>
              Showing page {currentPage} of {totalPages} · 100 rows per page
            </span>
            <div className="flex items-center gap-2">
              <button
                onClick={() => setPage((value) => Math.max(1, value - 1))}
                disabled={currentPage === 1}
                className="inline-flex items-center gap-1 rounded-lg border border-slate-200 bg-white px-3 py-2 font-semibold text-slate-600 transition hover:bg-slate-100 disabled:cursor-not-allowed disabled:opacity-50 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <ChevronLeft className="h-4 w-4" />
                Previous
              </button>
              <button
                onClick={() => setPage((value) => Math.min(totalPages, value + 1))}
                disabled={currentPage >= totalPages}
                className="inline-flex items-center gap-1 rounded-lg border border-slate-200 bg-white px-3 py-2 font-semibold text-slate-600 transition hover:bg-slate-100 disabled:cursor-not-allowed disabled:opacity-50 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                Next
                <ChevronRight className="h-4 w-4" />
              </button>
            </div>
          </div>
        </div>
    </PageShell>
  );
}
