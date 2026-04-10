import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CheckSquare, ChevronRight, Download, FileText, Info, RefreshCw, Square, Table2 } from "lucide-react";
import { useCurrency } from "../hooks/useCurrency";
import { CLOUD_PROVIDER_OPTIONS, resolveProviderValue } from "../constants/cloudProviders";
import { exportBlobWithTauriFallback, exportTextWithTauriFallback, revealExportedFileInFolder } from "../utils/fileExport";
import { drawPdfBrandHeader, drawPdfFooterSiteLink } from "../utils/pdfBranding";
import { loadPdfRuntime } from "../utils/pdfRuntime";
import { PageHeader } from "./layout/PageHeader";
import { MetricCard } from "./ui/MetricCard";

interface WastedResource {
  id: string;
  provider: string;
  region: string;
  resource_type: string;
  details: string;
  estimated_monthly_cost: number;
  action_type: string;
  account_id?: string | null;
  account_name?: string | null;
}

interface ResourceMetric {
  id: string;
  provider: string;
  region: string;
  resource_type: string;
  name?: string;
  status: string;
  cpu_utilization?: number;
  network_in_mb?: number;
  connections?: number;
  updated_at: number;
  source?: string;
  account_id?: string;
}

type SummarySortKey =
  | "provider"
  | "resourceCount"
  | "estimatedCost"
  | "wasteCount"
  | "wasteAmount"
  | "quantityRatio"
  | "amountRatio";

type SummarySortDirection = "asc" | "desc";

type DetailColumnKey =
  | "provider"
  | "region"
  | "resource_type"
  | "id"
  | "estimated_monthly_cost"
  | "action_type"
  | "details";

interface ProviderSummaryRow {
  providerKey: string;
  providerLabel: string;
  resourceCount: number;
  estimatedCost: number;
  wasteCount: number;
  wasteAmount: number;
  quantityRatio: number;
  amountRatio: number;
}

interface AccountSummaryRow {
  accountKey: string;
  accountLabel: string;
  findingCount: number;
  wasteAmount: number;
  providers: number;
  rows: WastedResource[];
}

interface DetailRowWithKey {
  key: string;
  row: WastedResource;
}

const DETAIL_COLUMNS: Array<{ key: DetailColumnKey; label: string }> = [
  { key: "provider", label: "Provider" },
  { key: "region", label: "Region" },
  { key: "resource_type", label: "Type" },
  { key: "id", label: "Resource ID" },
  { key: "estimated_monthly_cost", label: "Potential Waste ($/mo)" },
  { key: "action_type", label: "Suggested Action" },
  { key: "details", label: "Details" },
];

const PROVIDER_LABEL_MAP = new Map(
  CLOUD_PROVIDER_OPTIONS.map((item) => [normalizeProviderKey(item.value), item.label]),
);

function normalizeProviderKey(input: string): string {
  return (input || "").toLowerCase().replace(/[^a-z0-9]/g, "");
}

function resolveProviderKey(providerRaw: string): string {
  return normalizeProviderKey(resolveProviderValue(providerRaw || "unknown"));
}

function resolveProviderLabel(providerRaw: string): string {
  const key = resolveProviderKey(providerRaw);
  const fromCatalog = PROVIDER_LABEL_MAP.get(key);
  if (fromCatalog) {
    return fromCatalog;
  }
  const fallback = (providerRaw || "").trim();
  return fallback || "Unknown";
}

function csvSafe(value: string): string {
  if (value.includes(",") || value.includes("\"") || value.includes("\n")) {
    return `"${value.replace(/"/g, "\"\"")}"`;
  }
  return value;
}

function formatRatio(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}

function toDetailRowKey(row: WastedResource, index: number): string {
  return [
    resolveProviderKey(row.provider),
    row.id,
    row.region,
    row.resource_type,
    String(row.estimated_monthly_cost || 0),
    row.action_type,
    String(index),
  ].join("|");
}

function getColumnValue(row: WastedResource, key: DetailColumnKey): string {
  if (key === "estimated_monthly_cost") {
    return Number(row.estimated_monthly_cost || 0).toFixed(2);
  }
  return String(row[key] ?? "");
}

export function ProviderResourcesScreen() {
  const { format } = useCurrency();
  const detailSectionRef = useRef<HTMLDivElement | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [metrics, setMetrics] = useState<ResourceMetric[]>([]);
  const [scanResults, setScanResults] = useState<WastedResource[]>([]);
  const [sortKey, setSortKey] = useState<SummarySortKey>("wasteAmount");
  const [sortDirection, setSortDirection] = useState<SummarySortDirection>("desc");
  const [selectedProviderKeys, setSelectedProviderKeys] = useState<Set<string>>(new Set());
  const [selectedDetailKeys, setSelectedDetailKeys] = useState<Set<string>>(new Set());
  const [selectedAccountKey, setSelectedAccountKey] = useState<string>("");
  const [detailQuery, setDetailQuery] = useState("");
  const [selectedColumns, setSelectedColumns] = useState<DetailColumnKey[]>([
    "provider",
    "region",
    "resource_type",
    "id",
    "estimated_monthly_cost",
    "action_type",
    "details",
  ]);
  const [exportNotice, setExportNotice] = useState<string>("");

  useEffect(() => {
    void loadData();
  }, []);

  async function loadData() {
    setLoading(true);
    setError(null);
    try {
      const isDemo = localStorage.getItem("cws_is_demo_mode") === "true";
      const [metricRows, findings] = await Promise.all([
        invoke<ResourceMetric[]>("get_resource_metrics", { demoMode: isDemo }),
        invoke<WastedResource[]>("get_enriched_scan_results"),
      ]);
      setMetrics(metricRows);
      setScanResults(findings);
      setSelectedProviderKeys(new Set());
      setSelectedDetailKeys(new Set());
      setSelectedAccountKey("");
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  function handleSort(nextKey: SummarySortKey) {
    if (sortKey === nextKey) {
      setSortDirection((prev) => (prev === "desc" ? "asc" : "desc"));
      return;
    }
    setSortKey(nextKey);
    setSortDirection("desc");
  }

  const inventoryRows = useMemo(() => {
    return metrics.filter((row) => {
      if (resolveProviderKey(row.provider) === "summary") return false;
      if (row.resource_type === "Connected Account") return false;
      return (row.source || "").toLowerCase() !== "latest_scan";
    });
  }, [metrics]);

  const findingsRows = useMemo(() => {
    return scanResults.filter((row) => resolveProviderKey(row.provider) !== "summary");
  }, [scanResults]);

  const providerSummary = useMemo<ProviderSummaryRow[]>(() => {
    const bucket = new Map<
      string,
      {
        providerLabel: string;
        resourceCount: number;
        wasteCount: number;
        wasteAmount: number;
      }
    >();

    for (const row of inventoryRows) {
      const providerKey = resolveProviderKey(row.provider);
      if (!providerKey) continue;
      const current = bucket.get(providerKey) || {
        providerLabel: resolveProviderLabel(row.provider),
        resourceCount: 0,
        wasteCount: 0,
        wasteAmount: 0,
      };
      current.resourceCount += 1;
      bucket.set(providerKey, current);
    }

    for (const row of findingsRows) {
      const providerKey = resolveProviderKey(row.provider);
      if (!providerKey) continue;
      const current = bucket.get(providerKey) || {
        providerLabel: resolveProviderLabel(row.provider),
        resourceCount: 0,
        wasteCount: 0,
        wasteAmount: 0,
      };
      current.wasteCount += 1;
      current.wasteAmount += Number(row.estimated_monthly_cost || 0);
      bucket.set(providerKey, current);
    }

    const totalWasteAmount = Array.from(bucket.values()).reduce((sum, item) => sum + item.wasteAmount, 0);
    const rows = Array.from(bucket.entries()).map(([providerKey, item]) => {
      const effectiveResourceCount = Math.max(item.resourceCount, item.wasteCount);
      const estimatedCost =
        item.wasteCount > 0 && effectiveResourceCount > 0
          ? (item.wasteAmount / item.wasteCount) * effectiveResourceCount
          : 0;
      const quantityRatio = effectiveResourceCount > 0 ? item.wasteCount / effectiveResourceCount : 0;
      const amountRatio = totalWasteAmount > 0 ? item.wasteAmount / totalWasteAmount : 0;
      return {
        providerKey,
        providerLabel: item.providerLabel,
        resourceCount: effectiveResourceCount,
        estimatedCost,
        wasteCount: item.wasteCount,
        wasteAmount: item.wasteAmount,
        quantityRatio,
        amountRatio,
      };
    });

    rows.sort((a, b) => {
      const direction = sortDirection === "asc" ? 1 : -1;
      if (sortKey === "provider") {
        return direction * a.providerLabel.localeCompare(b.providerLabel);
      }
      return direction * ((a[sortKey] as number) - (b[sortKey] as number));
    });
    return rows;
  }, [inventoryRows, findingsRows, sortDirection, sortKey]);

  const providerSummaryKeys = useMemo(
    () => providerSummary.map((row) => row.providerKey),
    [providerSummary],
  );

  useEffect(() => {
    if (selectedProviderKeys.size === 0) return;
    const allowed = new Set(providerSummaryKeys);
    setSelectedProviderKeys((prev) => {
      const next = new Set(Array.from(prev).filter((key) => allowed.has(key)));
      return next.size === prev.size ? prev : next;
    });
  }, [providerSummaryKeys, selectedProviderKeys.size]);

  const activeProviderKeySet = useMemo(() => {
    if (selectedProviderKeys.size > 0) {
      return new Set(selectedProviderKeys);
    }
    return new Set(providerSummaryKeys);
  }, [providerSummaryKeys, selectedProviderKeys]);

  const detailRows = useMemo(() => {
    let rows = findingsRows;
    if (activeProviderKeySet.size > 0) {
      rows = rows.filter((row) => activeProviderKeySet.has(resolveProviderKey(row.provider)));
    }
    if (selectedAccountKey) {
      rows = rows.filter((row) => String(row.account_id || row.account_name || "unattributed") === selectedAccountKey);
    }
    const keyword = detailQuery.trim().toLowerCase();
    if (!keyword) return rows;
    return rows.filter((row) =>
      [row.id, row.provider, row.region, row.resource_type, row.details, row.action_type]
        .join(" ")
        .toLowerCase()
        .includes(keyword),
    );
  }, [activeProviderKeySet, detailQuery, findingsRows, selectedAccountKey]);

  const detailRowsWithKey = useMemo<DetailRowWithKey[]>(
    () => detailRows.map((row, index) => ({ key: toDetailRowKey(row, index), row })),
    [detailRows],
  );

  useEffect(() => {
    if (selectedDetailKeys.size === 0) return;
    const allowed = new Set(detailRowsWithKey.map((item) => item.key));
    setSelectedDetailKeys((prev) => {
      const next = new Set(Array.from(prev).filter((key) => allowed.has(key)));
      return next.size === prev.size ? prev : next;
    });
  }, [detailRowsWithKey, selectedDetailKeys.size]);

  const selectedProviderLabel = useMemo(() => {
    if (selectedProviderKeys.size === 0) return "All Providers";
    if (selectedProviderKeys.size === 1) {
      const key = Array.from(selectedProviderKeys)[0];
      return providerSummary.find((item) => item.providerKey === key)?.providerLabel || "Selected Provider";
    }
    return `${selectedProviderKeys.size} Providers Selected`;
  }, [providerSummary, selectedProviderKeys]);
  const selectedProviderCount = selectedProviderKeys.size || providerSummary.length;
  const selectedFindingCount = selectedDetailKeys.size || detailRowsWithKey.length;
  const activeWasteAmount = detailRows.reduce((sum, row) => sum + Number(row.estimated_monthly_cost || 0), 0);
  const accountSummary = useMemo<AccountSummaryRow[]>(() => {
    const bucket = new Map<string, { label: string; findingCount: number; wasteAmount: number; providerKeys: Set<string> }>();
    for (const row of detailRows) {
      const accountKey = String(row.account_id || row.account_name || "unattributed");
      const label = String(row.account_name || row.account_id || "Unattributed");
      const current = bucket.get(accountKey) || {
        label,
        findingCount: 0,
        wasteAmount: 0,
        providerKeys: new Set<string>(),
      };
      current.findingCount += 1;
      current.wasteAmount += Number(row.estimated_monthly_cost || 0);
      current.providerKeys.add(resolveProviderKey(row.provider));
      bucket.set(accountKey, current);
    }
    return Array.from(bucket.entries())
      .map(([accountKey, item]) => ({
        accountKey,
        accountLabel: item.label,
        findingCount: item.findingCount,
        wasteAmount: item.wasteAmount,
        providers: item.providerKeys.size,
        rows: detailRows.filter((row) => String(row.account_id || row.account_name || "unattributed") === accountKey),
      }))
      .sort((a, b) => b.wasteAmount - a.wasteAmount)
      .slice(0, 6);
  }, [detailRows]);

  async function exportRowsAsCsv(sourceRows: WastedResource[], label: string) {
    const columns = DETAIL_COLUMNS.filter((item) => selectedColumns.includes(item.key));
    if (!sourceRows.length || !columns.length) return;
    const lines: string[] = [];
    lines.push(columns.map((item) => item.label).join(","));
    for (const row of sourceRows) {
      lines.push(columns.map((column) => csvSafe(getColumnValue(row, column.key))).join(","));
    }
    const stamp = new Date().toISOString().slice(0, 10);
    const filename = `provider-resources-${label}-${stamp}.csv`;
    const savedPath = await exportTextWithTauriFallback(`\uFEFF${lines.join("\n")}`, filename, "text/csv;charset=utf-8;", { openAfterSave: false });
    if (savedPath) {
      await revealExportedFileInFolder(savedPath);
    }
    setExportNotice(`CSV exported for ${label} (${sourceRows.length} rows).`);
    window.setTimeout(() => setExportNotice(""), 2600);
  }

  async function exportRowsAsPdf(sourceRows: WastedResource[], label: string) {
    const columns = DETAIL_COLUMNS.filter((item) => selectedColumns.includes(item.key));
    if (!sourceRows.length || !columns.length) return;
    const { jsPDF, autoTable } = await loadPdfRuntime();
    const doc = new jsPDF({
      orientation: columns.length >= 6 ? "landscape" : "portrait",
      unit: "mm",
      format: "a4",
    });
    const pageWidth = doc.internal.pageSize.getWidth();
    const pageHeight = doc.internal.pageSize.getHeight();
    const headerBottomY = drawPdfBrandHeader(doc, {
      title: `Resource Export: ${label}`,
      generatedAt: new Date(),
      extraLines: [`Rows: ${sourceRows.length}`],
    });
    autoTable(doc, {
      startY: headerBottomY + 6,
      head: [columns.map((item) => item.label)],
      body: sourceRows.map((row) => columns.map((column) => getColumnValue(row, column.key))),
      theme: "grid",
      headStyles: { fillColor: [79, 70, 229] },
      styles: { fontSize: 8 },
      margin: { left: 12, right: 12, bottom: 18 },
    });
    const totalPages = doc.getNumberOfPages();
    for (let page = 1; page <= totalPages; page += 1) {
      doc.setPage(page);
      drawPdfFooterSiteLink(doc, pageWidth, pageHeight, page, totalPages);
    }
    const filename = `provider-resources-${label}-${new Date().toISOString().slice(0, 10)}.pdf`;
    const blob = doc.output("blob");
    const savedPath = await exportBlobWithTauriFallback(blob, filename, { openAfterSave: false });
    if (savedPath) {
      await revealExportedFileInFolder(savedPath);
    }
    setExportNotice(`PDF exported for ${label} (${sourceRows.length} rows).`);
    window.setTimeout(() => setExportNotice(""), 2600);
  }

  function toggleColumn(column: DetailColumnKey) {
    setSelectedColumns((prev) => {
      if (prev.includes(column)) {
        if (prev.length === 1) return prev;
        return prev.filter((item) => item !== column);
      }
      return [...prev, column];
    });
  }

  function toggleProviderSelection(providerKey: string) {
    setSelectedProviderKeys((prev) => {
      const next = new Set(prev);
      if (next.has(providerKey)) {
        next.delete(providerKey);
      } else {
        next.add(providerKey);
      }
      return next;
    });
    setSelectedDetailKeys(new Set());
    detailSectionRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  function toggleSelectAllProviders() {
    if (providerSummaryKeys.length === 0) return;
    setSelectedDetailKeys(new Set());
    setSelectedProviderKeys((prev) => {
      if (prev.size === providerSummaryKeys.length) {
        return new Set();
      }
      return new Set(providerSummaryKeys);
    });
  }

  function toggleDetailSelection(detailKey: string) {
    setSelectedDetailKeys((prev) => {
      const next = new Set(prev);
      if (next.has(detailKey)) {
        next.delete(detailKey);
      } else {
        next.add(detailKey);
      }
      return next;
    });
  }

  function toggleSelectAllDetails() {
    const visibleKeys = detailRowsWithKey.map((item) => item.key);
    if (visibleKeys.length === 0) return;
    setSelectedDetailKeys((prev) => {
      if (prev.size === visibleKeys.length) {
        return new Set();
      }
      return new Set(visibleKeys);
    });
  }

  function getExportRows(): WastedResource[] {
    if (selectedDetailKeys.size > 0) {
      return detailRowsWithKey
        .filter((item) => selectedDetailKeys.has(item.key))
        .map((item) => item.row);
    }
    return detailRowsWithKey.map((item) => item.row);
  }

  async function exportCsv() {
    const sourceRows = getExportRows();
    if (sourceRows.length === 0) {
      setExportNotice("No rows to export.");
      window.setTimeout(() => setExportNotice(""), 2600);
      return;
    }
    const columns = DETAIL_COLUMNS.filter((item) => selectedColumns.includes(item.key));
    if (columns.length === 0) {
      setExportNotice("Select at least one export column.");
      window.setTimeout(() => setExportNotice(""), 2600);
      return;
    }
    const lines: string[] = [];
    lines.push(columns.map((item) => item.label).join(","));
    for (const row of sourceRows) {
      lines.push(
        columns.map((column) => csvSafe(getColumnValue(row, column.key))).join(","),
      );
    }
    const stamp = new Date().toISOString().slice(0, 10);
    const filename = `provider-resources-${stamp}.csv`;
    try {
      const savedPath = await exportTextWithTauriFallback(
        `\uFEFF${lines.join("\n")}`,
        filename,
        "text/csv;charset=utf-8;",
        { openAfterSave: false },
      );
      if (savedPath) {
        await revealExportedFileInFolder(savedPath);
      }
      setExportNotice(`CSV exported (${sourceRows.length} rows).`);
      window.setTimeout(() => setExportNotice(""), 2600);
    } catch (err) {
      setExportNotice(`CSV export failed: ${String(err)}`);
      window.setTimeout(() => setExportNotice(""), 3600);
    }
  }

  async function exportPdf() {
    const sourceRows = getExportRows();
    if (sourceRows.length === 0) {
      setExportNotice("No rows to export.");
      window.setTimeout(() => setExportNotice(""), 2600);
      return;
    }
    const columns = DETAIL_COLUMNS.filter((item) => selectedColumns.includes(item.key));
    if (columns.length === 0) {
      setExportNotice("Select at least one export column.");
      window.setTimeout(() => setExportNotice(""), 2600);
      return;
    }
    try {
      const { jsPDF, autoTable } = await loadPdfRuntime();
      const doc = new jsPDF({
        orientation: columns.length >= 6 ? "landscape" : "portrait",
        unit: "mm",
        format: "a4",
      });
      const pageWidth = doc.internal.pageSize.getWidth();
      const pageHeight = doc.internal.pageSize.getHeight();
      const headerBottomY = drawPdfBrandHeader(doc, {
        title: "Provider Resources Export",
        generatedAt: new Date(),
        extraLines: [
          `Providers: ${selectedProviderLabel}`,
          `Rows: ${sourceRows.length}`,
        ],
      });

      autoTable(doc, {
        startY: headerBottomY + 6,
        head: [columns.map((item) => item.label)],
        body: sourceRows.map((row) => columns.map((column) => getColumnValue(row, column.key))),
        theme: "grid",
        headStyles: { fillColor: [79, 70, 229] },
        styles: { fontSize: 8 },
        margin: { left: 12, right: 12, bottom: 18 },
      });

      const totalPages = doc.getNumberOfPages();
      for (let page = 1; page <= totalPages; page += 1) {
        doc.setPage(page);
        drawPdfFooterSiteLink(doc, pageWidth, pageHeight, page, totalPages);
      }

      const filename = `provider-resources-${new Date().toISOString().slice(0, 10)}.pdf`;
      const blob = doc.output("blob");
      const savedPath = await exportBlobWithTauriFallback(blob, filename, { openAfterSave: false });
      if (savedPath) {
        await revealExportedFileInFolder(savedPath);
      }
      setExportNotice(`PDF exported (${sourceRows.length} rows).`);
      window.setTimeout(() => setExportNotice(""), 2600);
    } catch (err) {
      setExportNotice(`PDF export failed: ${String(err)}`);
      window.setTimeout(() => setExportNotice(""), 3600);
    }
  }

  return (
    <div className="p-8 space-y-6 pb-24 bg-slate-50 dark:bg-slate-900 min-h-screen dark:text-slate-100 transition-colors duration-300">
      <PageHeader
        title="Resource Inventory"
        subtitle="Provider-level inventory, potential waste, and share breakdown for the latest data set."
        icon={<Table2 className="h-6 w-6" />}
        actions={
          <div className="flex flex-wrap items-center gap-2">
            <button
              onClick={() => void loadData()}
              className="inline-flex items-center gap-2 rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-semibold text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
            >
              <RefreshCw size={16} /> Refresh
            </button>
            <button
              onClick={() => void exportCsv()}
              className="inline-flex items-center gap-2 rounded-lg border border-indigo-200 bg-indigo-50 px-3 py-2 text-sm font-semibold text-indigo-700 hover:bg-indigo-100 dark:border-indigo-700/70 dark:bg-indigo-900/30 dark:text-indigo-200 dark:hover:bg-indigo-800/40"
            >
              <Download size={16} /> Export CSV
            </button>
            <button
              onClick={() => void exportPdf()}
              className="inline-flex items-center gap-2 rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-sm font-semibold text-emerald-700 hover:bg-emerald-100 dark:border-emerald-700/70 dark:bg-emerald-900/30 dark:text-emerald-200 dark:hover:bg-emerald-800/40"
            >
              <FileText size={16} /> Export PDF
            </button>
            {(selectedProviderKeys.size > 0 || selectedDetailKeys.size > 0) && (
              <button
                onClick={() => {
                  setSelectedProviderKeys(new Set());
                  setSelectedDetailKeys(new Set());
                }}
                className="inline-flex items-center gap-2 rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-semibold text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                Clear Selection
              </button>
            )}
          </div>
        }
      />

      <p className="inline-flex items-center gap-2 rounded-lg border border-indigo-200 bg-indigo-50 px-3 py-1.5 text-xs font-medium text-indigo-700 dark:border-indigo-800 dark:bg-indigo-900/20 dark:text-indigo-300">
        <Info className="h-3.5 w-3.5" />
        Estimated Spend uses extrapolation from detected findings. Refresh Metrics can trigger cloud metrics API calls.
      </p>

      {exportNotice && (
        <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-2 text-sm font-semibold text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-900/20 dark:text-emerald-300">
          {exportNotice}
        </div>
      )}

      {error && (
        <div className="rounded-lg border border-rose-200 bg-rose-50 px-4 py-3 text-sm font-medium text-rose-700 dark:border-rose-900/60 dark:bg-rose-900/20 dark:text-rose-300">
          Failed to load provider summary: {error}
        </div>
      )}

      <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="grid gap-4 md:grid-cols-3">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Use This For</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              Compare provider-level inventory, waste concentration, and share of impact before assigning owners or exporting a subset.
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Current Slice</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              {selectedProviderCount} providers in scope, {selectedFindingCount} findings in detail view, potential waste {format(activeWasteAmount)}/mo.
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Operator Workflow</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              Select providers first, narrow the detailed table second, then export only the rows and columns that the responsible owner needs.
            </p>
          </div>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <MetricCard
          label="Providers In Scope"
          value={selectedProviderCount}
          hint="Selected providers, or all providers when no selection is active."
        />
        <MetricCard
          label="Detail Rows"
          value={detailRowsWithKey.length}
          hint="Visible findings after provider and keyword filtering."
        />
        <MetricCard
          label="Potential Waste"
          value={<span className="text-rose-600 dark:text-rose-300">{format(activeWasteAmount)}</span>}
          hint="Combined monthly waste amount for the current detail slice."
        />
      </div>

      <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="flex flex-wrap items-start justify-between gap-4 border-b border-slate-200 pb-4 dark:border-slate-700">
          <div>
            <h2 className="text-lg font-semibold text-slate-900 dark:text-white">Account Concentration</h2>
            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
              Highest-impact accounts inside the current provider slice. Use this before assigning owners or exporting an owner-specific packet.
            </p>
          </div>
          <span className="rounded-full bg-slate-100 px-3 py-1 text-xs font-semibold uppercase tracking-[0.18em] text-slate-500 dark:bg-slate-700 dark:text-slate-300">
            Top 6 Accounts
          </span>
        </div>
        <div className="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {!accountSummary.length ? (
            <p className="text-sm text-slate-500 dark:text-slate-400">No account attribution is available for the current slice yet.</p>
          ) : accountSummary.map((row) => (
            <div
              key={row.accountKey}
              onClick={() => {
                setSelectedAccountKey((current) => current === row.accountKey ? "" : row.accountKey);
                detailSectionRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
              }}
              className={`cursor-pointer rounded-2xl border p-4 text-left transition hover:border-indigo-300 dark:hover:border-indigo-500/40 ${
                selectedAccountKey === row.accountKey
                  ? "border-indigo-300 bg-indigo-50 dark:border-indigo-500/40 dark:bg-indigo-900/20"
                  : "border-slate-200 bg-slate-50 dark:border-slate-700 dark:bg-slate-900/40"
              }`}
            >
              <p className="text-sm font-semibold text-slate-900 dark:text-white">{row.accountLabel}</p>
              <p className="mt-2 text-2xl font-bold text-rose-600 dark:text-rose-300">{format(row.wasteAmount)}</p>
              <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">{row.findingCount} findings across {row.providers} providers</p>
              <div className="mt-3 flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={(event) => {
                    event.stopPropagation();
                    void exportRowsAsCsv(row.rows, row.accountKey);
                  }}
                  className="inline-flex items-center gap-1 rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
                >
                  <Download className="h-3.5 w-3.5" />
                  CSV
                </button>
                <button
                  type="button"
                  onClick={(event) => {
                    event.stopPropagation();
                    void exportRowsAsPdf(row.rows, row.accountKey);
                  }}
                  className="inline-flex items-center gap-1 rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 text-xs font-semibold text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
                >
                  <FileText className="h-3.5 w-3.5" />
                  PDF
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="rounded-lg border border-slate-200 bg-white px-4 py-2 text-xs text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300">
        Selected providers: <span className="font-semibold">{selectedProviderKeys.size || providerSummary.length}</span>
        {" · "}
        Selected findings: <span className="font-semibold">{selectedDetailKeys.size || detailRowsWithKey.length}</span>
        {" · "}
        Account rows: <span className="font-semibold">{accountSummary.length}</span>
        {selectedAccountKey ? (
          <>
            {" · "}
            Active account filter: <span className="font-semibold">{accountSummary.find((row) => row.accountKey === selectedAccountKey)?.accountLabel || selectedAccountKey}</span>
          </>
        ) : null}
      </div>

      <div className="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="overflow-x-auto">
          <table className="min-w-full divide-y divide-slate-200 text-sm dark:divide-slate-700">
            <thead className="bg-slate-50 dark:bg-slate-900/40">
              <tr className="text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-300">
                <th className="px-4 py-3">
                  <button
                    onClick={toggleSelectAllProviders}
                    className="inline-flex items-center text-slate-600 dark:text-slate-300"
                    title="Select all providers"
                  >
                    {providerSummary.length > 0 && selectedProviderKeys.size === providerSummary.length
                      ? <CheckSquare className="h-4 w-4" />
                      : <Square className="h-4 w-4" />}
                  </button>
                </th>
                <th className="px-4 py-3 cursor-pointer" onClick={() => handleSort("provider")}>Provider</th>
                <th className="px-4 py-3 text-right cursor-pointer" onClick={() => handleSort("resourceCount")}>Resources</th>
                <th className="px-4 py-3 text-right cursor-pointer" onClick={() => handleSort("estimatedCost")}>Estimated Spend</th>
                <th className="px-4 py-3 text-right cursor-pointer" onClick={() => handleSort("wasteCount")}>Potential Waste Count</th>
                <th className="px-4 py-3 text-right cursor-pointer" onClick={() => handleSort("wasteAmount")}>Potential Waste Amount</th>
                <th className="px-4 py-3 text-right cursor-pointer" onClick={() => handleSort("quantityRatio")}>Quantity Ratio</th>
                <th className="px-4 py-3 text-right cursor-pointer" onClick={() => handleSort("amountRatio")}>Amount Ratio</th>
                <th className="px-4 py-3 text-right">Details</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 dark:divide-slate-700">
              {!loading && providerSummary.map((row) => {
                const isSelected = selectedProviderKeys.has(row.providerKey);
                return (
                  <tr
                    key={row.providerKey}
                    onClick={() => toggleProviderSelection(row.providerKey)}
                    className={`cursor-pointer transition-colors ${
                      isSelected
                        ? "bg-indigo-50/70 dark:bg-indigo-900/20"
                        : "hover:bg-slate-50 dark:hover:bg-slate-700/40"
                    }`}
                  >
                    <td className="px-4 py-3">
                      {isSelected ? (
                        <CheckSquare className="h-4 w-4 text-indigo-600 dark:text-indigo-400" />
                      ) : (
                        <Square className="h-4 w-4 text-slate-400" />
                      )}
                    </td>
                    <td className="px-4 py-3 font-semibold text-slate-900 dark:text-white">{row.providerLabel}</td>
                    <td className="px-4 py-3 text-right font-medium text-slate-700 dark:text-slate-200">{row.resourceCount.toLocaleString()}</td>
                    <td className="px-4 py-3 text-right font-medium text-slate-700 dark:text-slate-200">{format(row.estimatedCost)}</td>
                    <td className="px-4 py-3 text-right font-medium text-slate-700 dark:text-slate-200">{row.wasteCount.toLocaleString()}</td>
                    <td className="px-4 py-3 text-right font-semibold text-rose-600 dark:text-rose-300">{format(row.wasteAmount)}</td>
                    <td className="px-4 py-3 text-right font-medium text-slate-700 dark:text-slate-200">{formatRatio(row.quantityRatio)}</td>
                    <td className="px-4 py-3 text-right font-medium text-slate-700 dark:text-slate-200">{formatRatio(row.amountRatio)}</td>
                    <td className="px-4 py-3 text-right text-indigo-600 dark:text-indigo-300">
                      <span className="inline-flex items-center gap-1 text-xs font-semibold">
                        View <Table2 size={14} />
                      </span>
                    </td>
                  </tr>
                );
              })}
              {!loading && providerSummary.length === 0 && (
                <tr>
                  <td colSpan={9} className="px-6 py-8 text-center text-slate-400 dark:text-slate-500">
                    No provider data yet. Run a scan and refresh metrics first.
                  </td>
                </tr>
              )}
              {loading && (
                <tr>
                  <td colSpan={9} className="px-6 py-8 text-center text-slate-400 dark:text-slate-500 animate-pulse">
                    Loading provider summary...
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      <div ref={detailSectionRef} className="rounded-xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h3 className="text-xl font-bold text-slate-900 dark:text-white flex items-center gap-2">
              <ChevronRight className="h-4 w-4 text-indigo-500" />
              Provider Detail: {selectedProviderLabel}
            </h3>
            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
              Upper and lower tables both support multi-select. Export uses selected findings first.
            </p>
          </div>
          <input
            value={detailQuery}
            onChange={(event) => setDetailQuery(event.target.value)}
            placeholder="Search resource ID, type, details..."
            className="w-full max-w-sm rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-700 outline-none focus:border-indigo-500 focus:ring-2 focus:ring-indigo-200 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-100 dark:focus:border-indigo-400 dark:focus:ring-indigo-700/40"
          />
        </div>

        <div className="mt-4 rounded-lg border border-slate-200 bg-slate-50 p-3 dark:border-slate-700 dark:bg-slate-900/30">
          <p className="text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-300 mb-2">Export Columns</p>
          <div className="flex flex-wrap gap-3">
            {DETAIL_COLUMNS.map((column) => {
              const checked = selectedColumns.includes(column.key);
              return (
                <label key={column.key} className="inline-flex items-center gap-2 text-sm text-slate-700 dark:text-slate-200 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={() => toggleColumn(column.key)}
                    className="h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-500 dark:border-slate-600 dark:bg-slate-800"
                  />
                  {column.label}
                </label>
              );
            })}
          </div>
        </div>

        <div className="mt-4 overflow-x-auto">
          <table className="min-w-full divide-y divide-slate-200 text-sm dark:divide-slate-700">
            <thead className="bg-slate-50 dark:bg-slate-900/40">
              <tr className="text-left text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-300">
                <th className="px-4 py-3">
                  <button
                    onClick={toggleSelectAllDetails}
                    className="inline-flex items-center text-slate-600 dark:text-slate-300"
                    title="Select all findings"
                  >
                    {detailRowsWithKey.length > 0 && selectedDetailKeys.size === detailRowsWithKey.length
                      ? <CheckSquare className="h-4 w-4" />
                      : <Square className="h-4 w-4" />}
                  </button>
                </th>
                <th className="px-4 py-3">Provider</th>
                <th className="px-4 py-3">Region</th>
                <th className="px-4 py-3">Type</th>
                <th className="px-4 py-3">Resource ID</th>
                <th className="px-4 py-3 text-right">Potential Waste</th>
                <th className="px-4 py-3">Suggested Action</th>
                <th className="px-4 py-3">Details</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 dark:divide-slate-700">
              {detailRowsWithKey.map((item) => {
                const { row, key } = item;
                const selected = selectedDetailKeys.has(key);
                return (
                  <tr
                    key={key}
                    onClick={() => toggleDetailSelection(key)}
                    className={`cursor-pointer ${selected ? "bg-indigo-50/60 dark:bg-indigo-900/20" : "hover:bg-slate-50 dark:hover:bg-slate-700/40"}`}
                  >
                    <td className="px-4 py-3">
                      {selected ? (
                        <CheckSquare className="h-4 w-4 text-indigo-600 dark:text-indigo-400" />
                      ) : (
                        <Square className="h-4 w-4 text-slate-400" />
                      )}
                    </td>
                    <td className="px-4 py-3 font-medium text-slate-700 dark:text-slate-100">{resolveProviderLabel(row.provider)}</td>
                    <td className="px-4 py-3 text-slate-600 dark:text-slate-300">{row.region}</td>
                    <td className="px-4 py-3 text-slate-600 dark:text-slate-300">{row.resource_type}</td>
                    <td className="px-4 py-3 font-mono text-sm text-slate-700 dark:text-slate-200">{row.id}</td>
                    <td className="px-4 py-3 text-right font-semibold text-rose-600 dark:text-rose-300">{format(Number(row.estimated_monthly_cost || 0))}</td>
                    <td className="px-4 py-3 text-slate-600 dark:text-slate-300">{row.action_type || "Review"}</td>
                    <td className="px-4 py-3 text-slate-600 dark:text-slate-300">{row.details}</td>
                  </tr>
                );
              })}
              {detailRowsWithKey.length === 0 && (
                <tr>
                  <td colSpan={8} className="px-6 py-6 text-center text-slate-400 dark:text-slate-500">
                    No detailed rows for current selection.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
