import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertCircle, RefreshCw, Loader2, CheckSquare, Square, Download, FileText, Search, ClipboardList, Play, EyeOff, Leaf } from "lucide-react";
import { Modal } from "./Modal";
import { useCurrency } from "../hooks/useCurrency";
import { CustomSelect } from "./CustomSelect";
import { CLOUD_PROVIDER_FILTER_OPTIONS, matchesProviderFilter, resolveProviderValue } from "../constants/cloudProviders";
import { estimateAggregateCo2e, estimateResourceCo2e, formatCo2eKg, formatCo2eTonsFromKg, ESG_METHODOLOGY_NOTE, ESG_DISCLAIMER_NOTE } from "../utils/esg";
import { drawPdfBrandHeader, drawPdfFooterSiteLink } from "../utils/pdfBranding";
import { exportBlobWithTauriFallback, exportTextWithTauriFallback, revealExportedFileInFolder } from "../utils/fileExport";
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

interface ResourcesTableProps {
  initialFilter?: any;
}

function normalizeWorsenedFlag(value: unknown): boolean {
  return value === true || value === "true" || value === 1 || value === "1";
}

export function ResourcesTable({ initialFilter }: ResourcesTableProps) {
  type ConfirmAction = "execute_plan" | "mark_handled";

  const [resources, setResources] = useState<WastedResource[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState("");
  const [filterProvider, setFilterProvider] = useState("All");
  const [filterAccountKey, setFilterAccountKey] = useState("");
  const [filterResourceType, setFilterResourceType] = useState("");
  const [showOnlyDeleteActions, setShowOnlyDeleteActions] = useState(false);
  const [aiContext, setAiContext] = useState("");
  
  // Cleanup & Plan State
  const [isPlanModalOpen, setPlanModalOpen] = useState(false);
  const [isExecuting, setExecuting] = useState(false);
  const [executionResult, setExecutionResult] = useState<string | null>(null);
  const [confirmAction, setConfirmAction] = useState<ConfirmAction | null>(null);
  const [confirmingAction, setConfirmingAction] = useState(false);
  const [actionNotice, setActionNotice] = useState<{ type: "success" | "error"; text: string } | null>(null);

  // Export State
  const [isExportModalOpen, setExportModalOpen] = useState(false);
  const [exportConfig, setExportConfig] = useState({
      title: "Cloud Waste Scan Report",
      company: "",
      preparedBy: "",
      includeExecutionColumns: true,
  });
  const [exportError, setExportError] = useState<string | null>(null);
  
  const isDemo = localStorage.getItem("cws_is_demo_mode") === "true";
  const { format } = useCurrency();

  const showExportError = (text: string) => {
      setExportError(text);
      window.setTimeout(() => setExportError(null), 6000);
  };

  const showActionNotice = (text: string, type: "success" | "error" = "success", autoHideMs = 6000) => {
      setActionNotice({ type, text });
      if (autoHideMs > 0) {
          window.setTimeout(() => {
              setActionNotice((prev) => (prev?.text === text ? null : prev));
          }, autoHideMs);
      }
  };

  const revealSavedExport = async (savedPath: string | null | undefined, label: "PDF" | "CSV") => {
      const resolvedPath = String(savedPath ?? "").trim();
      if (!resolvedPath) {
          throw new Error(`${label} exported, but no local path was returned.`);
      }
      await revealExportedFileInFolder(resolvedPath);
  };

  useEffect(() => {
    if (initialFilter?.provider) {
        setFilterProvider(resolveProviderValue(initialFilter.provider));
    }
    setFilterAccountKey(String(initialFilter?.accountKey || ""));
    setFilterResourceType(String(initialFilter?.resourceType || ""));
    setShowOnlyDeleteActions(normalizeWorsenedFlag(initialFilter?.showOnlyDeleteActions));
    setAiContext(String(initialFilter?.aiContext || ""));
    fetchData();
  }, [initialFilter]);

  async function fetchData() {
    setLoading(true);
    try {
      const data = await invoke<WastedResource[]>("get_enriched_scan_results");
      setResources(data);
      setSelectedIds(new Set());
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  }

  const toggleSelect = (id: string) => {
      const resource = resources.find(r => r.id === id);
      // Allow selecting DELETE and ARCHIVE (though archive is manual for now, let's treat it same for planning)
      if (!resource) return;

      const newSet = new Set(selectedIds);
      if (newSet.has(id)) newSet.delete(id);
      else newSet.add(id);
      setSelectedIds(newSet);
  };

  const toggleSelectAll = () => {
      // Select all filtered resources
      if (selectedIds.size === filteredResources.length && selectedIds.size > 0) {
          setSelectedIds(new Set());
      } else {
          setSelectedIds(new Set(filteredResources.map(r => r.id)));
      }
  };

  const getSelectedResources = () => {
      return resources.filter(r => selectedIds.has(r.id));
  };

  const filteredResources = resources.filter(r => {
      const matchesSearch = r.id.toLowerCase().includes(searchQuery.toLowerCase()) || 
                            r.resource_type.toLowerCase().includes(searchQuery.toLowerCase()) ||
                            r.details.toLowerCase().includes(searchQuery.toLowerCase()) ||
                            String(r.account_name || "").toLowerCase().includes(searchQuery.toLowerCase()) ||
                            String(r.account_id || "").toLowerCase().includes(searchQuery.toLowerCase());
      const matchesProvider = matchesProviderFilter(filterProvider, r.provider);
      const matchesAccount =
          !filterAccountKey || String(r.account_id || "").toLowerCase() === filterAccountKey.toLowerCase();
      const matchesResourceType =
          !filterResourceType || String(r.resource_type || "").toLowerCase() === filterResourceType.toLowerCase();
      const matchesSuggestedAction =
          !showOnlyDeleteActions || String(r.action_type || "").toUpperCase() === "DELETE";
      return matchesSearch && matchesProvider && matchesAccount && matchesResourceType && matchesSuggestedAction;
  });

  const handleSearchQueryChange = (value: string) => {
      setSearchQuery(value);
  };
  const filteredSavings = filteredResources.reduce((sum, item) => sum + item.estimated_monthly_cost, 0);
  const filteredCo2e = estimateAggregateCo2e(filteredResources);
  const visibleAccountCount = new Set(
      filteredResources.map((row) => String(row.account_id || row.account_name || "unattributed"))
  ).size;
  const deleteCandidateCount = filteredResources.filter((row) => String(row.action_type || "").toUpperCase() === "DELETE").length;
  const rightsizeCandidateCount = filteredResources.filter((row) => String(row.action_type || "").toUpperCase() === "RIGHTSIZE").length;

  // Stage 1: Generate Plan
  const handleGeneratePlan = () => {
      if (selectedIds.size === 0) return;
      setPlanModalOpen(true);
      setExecutionResult(null);
  };

  // Stage 2: Execute Plan
  const executePlanNow = async () => {
      setExecuting(true);
      const toCleanup = getSelectedResources().filter(r => r.action_type === 'DELETE'); // Only automate DELETE for now
      
      try {
          await invoke("confirm_cleanup", { resources: toCleanup, demoMode: isDemo });
          setExecutionResult("Success: Cleanup executed successfully.");
          
          // Refresh list
          setResources(prev => prev.filter(r => !selectedIds.has(r.id) || r.action_type !== 'DELETE'));
          setSelectedIds(new Set());
          setTimeout(() => setPlanModalOpen(false), 2000);
      } catch (e) {
          setExecutionResult("Error: " + e);
      } finally {
          setExecuting(false);
      }
  };

  const handleExecutePlan = () => {
      // Avoid stacked modals with the same z-index causing confirm dialog to appear behind.
      setPlanModalOpen(false);
      setConfirmAction("execute_plan");
  };

  const markHandledNow = async () => {
      try {
          if (isDemo) {
              setResources(prev => prev.filter(r => !selectedIds.has(r.id)));
              setSelectedIds(new Set());
              showActionNotice("Selected demo resources were hidden from this list.");
              return;
          }
          for (const id of selectedIds) {
              const r = resources.find(res => res.id === id);
              if (r) {
                  await invoke("mark_resource_handled", { id: r.id, provider: r.provider, note: "Manual" });
              }
          }
          setResources(prev => prev.filter(r => !selectedIds.has(r.id)));
          setSelectedIds(new Set());
          showActionNotice("Selected resources were marked as handled.");
      } catch (e) {
          showActionNotice("Failed to mark resources as handled: " + e, "error");
      }
  };

  const handleMarkHandled = () => {
      setConfirmAction("mark_handled");
  };

  const runConfirmedAction = async () => {
      if (!confirmAction) return;
      setConfirmingAction(true);
      try {
          if (confirmAction === "execute_plan") {
              await executePlanNow();
          } else {
              await markHandledNow();
          }
          setConfirmAction(null);
      } finally {
          setConfirmingAction(false);
      }
  };

  const handleExportCSV = async () => {
      if (resources.length === 0) return; 
      try {
          const wrapCsvText = (value: string, width = 72) => {
              const normalized = String(value || "").replace(/\s+/g, " ").trim();
              if (!normalized) return "";
              const words = normalized.split(" ");
              const lines: string[] = [];
              let current = "";
              for (const word of words) {
                  const candidate = current ? `${current} ${word}` : word;
                  if (candidate.length <= width) {
                      current = candidate;
                  } else {
                      if (current) lines.push(current);
                      current = word;
                  }
              }
              if (current) lines.push(current);
              return lines.join("\n");
          };
          const csvEscape = (value: unknown) => {
              const text = String(value ?? "");
              const escaped = text.replace(/"/g, '""');
              if (/[",\n]/.test(escaped)) {
                  return `"${escaped}"`;
              }
              return escaped;
          };
          const toDateTime = (date: Date) => {
              const pad = (n: number) => String(n).padStart(2, "0");
              return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
          };

          const generatedAt = new Date();
          const aggregate = estimateAggregateCo2e(resources);
          const totalMonthlySavings = resources.reduce((sum, r) => sum + r.estimated_monthly_cost, 0);

          const summaryRows = [
              ["Report", "Cloud Waste Scanner Resource Report"],
              ["Generated At", toDateTime(generatedAt)],
              ["Total Resources", resources.length],
              ["Total Monthly Savings (USD)", totalMonthlySavings.toFixed(2)],
              ["Estimated Monthly CO2e (kg)", aggregate.totalMonthlyCo2eKg.toFixed(2)],
              ["Estimated Annual CO2e (kg)", aggregate.totalAnnualCo2eKg.toFixed(2)],
          ];

          const headers = [
              "Provider",
              "Region",
              "Resource Type",
              "ID",
              "Details",
              "Action",
              "Monthly Cost (USD)",
              "Estimated CO2e Monthly (kg)",
              "Estimated CO2e Annual (kg)",
              "ESG Profile",
              "ESG Emission Factor (kg/USD/month)",
              "ESG Action Multiplier",
          ];

          const rows = resources.map((r) => {
              const esg = estimateResourceCo2e(r);
              return [
                  r.provider,
                  r.region,
                  r.resource_type,
                  r.id,
                  wrapCsvText(r.details, 58),
                  r.action_type,
                  r.estimated_monthly_cost.toFixed(2),
                  esg.monthlyCo2eKg.toFixed(2),
                  esg.annualCo2eKg.toFixed(2),
                  esg.profile,
                  esg.emissionFactorKgPerUsd.toFixed(3),
                  esg.actionMultiplier.toFixed(2),
              ];
          });

          const summaryLines = summaryRows.map((row) => row.map(csvEscape).join(","));
          const dataLines = rows.map((row) => row.map(csvEscape).join(","));
          const csvContent = [
              ...summaryLines,
              "",
              headers.map(csvEscape).join(","),
              ...dataLines,
          ].join("\n");

          const filename = `cloud_waste_report_${generatedAt.toISOString().slice(0,10)}.csv`;
          const savedPath = await exportTextWithTauriFallback(
              `\uFEFF${csvContent}`,
              filename,
              "text/csv;charset=utf-8;",
              { openAfterSave: false }
          );
          await revealSavedExport(savedPath, "CSV");
      } catch (err) {
          console.error("Failed to export CSV", err);
          showExportError(`CSV export failed: ${String(err)}`);
      }
  };

  const performExportPDF = async (items = resources, filename = "report") => {
      const includeExecutionColumns = Boolean(exportConfig.includeExecutionColumns);
      const { jsPDF, autoTable } = await loadPdfRuntime();
      const doc = new jsPDF({
          orientation: includeExecutionColumns ? "landscape" : "portrait",
          unit: "mm",
          format: "a4",
      });
      const docAny = doc as { lastAutoTable?: { finalY?: number } };
      const pageWidth = doc.internal.pageSize.getWidth();
      const pageHeight = doc.internal.pageSize.getHeight();
      const headerBottomY = drawPdfBrandHeader(doc, {
          title: exportConfig.title,
          generatedAt: new Date(),
          extraLines: [
              exportConfig.company ? `Company: ${exportConfig.company}` : "",
              exportConfig.preparedBy ? `Prepared By: ${exportConfig.preparedBy}` : "",
          ],
      });
      
      const totalSavings = items.reduce((sum, r) => sum + r.estimated_monthly_cost, 0);
      const totalCo2e = estimateAggregateCo2e(items);
      doc.setFontSize(14); doc.setTextColor(0, 150, 0);
      const startY = headerBottomY + 7;
      doc.text(`Total Potential Savings: ${format(totalSavings)} / month`, 14, startY);
      doc.setFontSize(12); doc.setTextColor(15, 118, 110);
      doc.text(
          `Estimated CO2e Reduction: ${formatCo2eKg(totalCo2e.totalMonthlyCo2eKg)} / month (${formatCo2eTonsFromKg(totalCo2e.totalAnnualCo2eKg)} / year)`,
          14,
          startY + 7
      );

      const tableHead = includeExecutionColumns
          ? [['Provider', 'Region', 'Type', 'ID', 'Cost', 'Est. CO2e/mo', 'Recommended', 'Custom', 'Notes']]
          : [['Provider', 'Region', 'Type', 'ID', 'Suggested Action', 'Cost', 'Est. CO2e/mo']];
      const placeholderItem: WastedResource = {
          id: "N/A",
          provider: "N/A",
          region: "N/A",
          resource_type: "N/A",
          details: "",
          estimated_monthly_cost: 0,
          action_type: "Review",
      };
      const tableRows = (items.length > 0 ? items : [placeholderItem]).map((resource) => (
          includeExecutionColumns
              ? [
                  resource.provider,
                  resource.region,
                  resource.resource_type,
                  resource.id,
                  format(resource.estimated_monthly_cost),
                  formatCo2eKg(estimateResourceCo2e(resource).monthlyCo2eKg),
                  resource.action_type || "Review",
                  "",
                  "",
              ]
              : [
                  resource.provider,
                  resource.region,
                  resource.resource_type,
                  resource.id,
                  resource.action_type,
                  format(resource.estimated_monthly_cost),
                  formatCo2eKg(estimateResourceCo2e(resource).monthlyCo2eKg),
              ]
      ));
      autoTable(doc, {
          startY: startY + 12,
          head: tableHead,
          body: tableRows,
          theme: 'grid',
          headStyles: { fillColor: [79, 70, 229] },
          styles: { fontSize: 8 },
          margin: { left: 14, right: 14, bottom: 18 },
          columnStyles: includeExecutionColumns
              ? {
                  0: { cellWidth: 21 },
                  1: { cellWidth: 22 },
                  2: { cellWidth: 24 },
                  3: { cellWidth: 50 },
                  4: { cellWidth: 18, halign: "right" },
                  5: { cellWidth: 22, halign: "right" },
                  6: { cellWidth: 40, cellPadding: { left: 6, right: 1.5, top: 1.5, bottom: 1.5 } },
                  7: { cellWidth: 22, cellPadding: { left: 6, right: 1.5, top: 1.5, bottom: 1.5 } },
                  8: { cellWidth: 50, cellPadding: { left: 2.2, right: 1.5, top: 1.5, bottom: 1.5 } },
              }
              : undefined,
          didParseCell: (data) => {
              if (!includeExecutionColumns || data.section !== "body") return;
              if (![6, 7, 8].includes(data.column.index)) return;
              data.cell.text = [""];
          },
          didDrawCell: (data) => {
              if (!includeExecutionColumns || data.section !== "body") return;
              if (![6, 7, 8].includes(data.column.index)) return;
              if (data.column.index === 6 || data.column.index === 7) {
                  const boxSize = Math.min(2.8, data.cell.height - 2.2);
                  const boxX = data.cell.x + 1.8;
                  const boxY = data.cell.y + (data.cell.height - boxSize) / 2;
                  doc.setDrawColor(120);
                  doc.rect(boxX, boxY, boxSize, boxSize);
                  if (data.column.index !== 6) {
                      return;
                  }
                  const contentX = boxX + boxSize + 1.8;
                  const rowRaw = Array.isArray(data.row.raw) ? data.row.raw : [];
                  const actionText = String(rowRaw[6] ?? "").trim();
                  if (actionText) {
                      doc.setFontSize(8);
                      doc.setTextColor(71, 85, 105);
                      doc.text(actionText, contentX, data.cell.y + data.cell.height / 2 + 1.4);
                  }
                  return;
              }
          },
      });
      let notesY = (docAny.lastAutoTable?.finalY ?? startY + 12) + 8;
      const noteLines = doc.splitTextToSize(`${ESG_METHODOLOGY_NOTE} ${ESG_DISCLAIMER_NOTE}`, pageWidth - 28);
      const notesHeight = noteLines.length * 4 + 4;
      if (notesY + notesHeight > pageHeight - 16) {
          doc.addPage();
          notesY = 20;
      }
      doc.setFontSize(9);
      doc.setTextColor(100);
      doc.text(noteLines, 14, notesY);

      const totalPages = doc.getNumberOfPages();
      for (let page = 1; page <= totalPages; page++) {
          doc.setPage(page);
          drawPdfFooterSiteLink(doc, pageWidth, pageHeight, page, totalPages);
      }

      const pdfFilename = `${filename}_${new Date().toISOString().slice(0,10)}.pdf`;
      let exportOk = false;
      try {
          const blob = doc.output("blob");
          const savedPath = await exportBlobWithTauriFallback(blob, pdfFilename, { openAfterSave: false });
          await revealSavedExport(savedPath, "PDF");
          exportOk = true;
          setExportModalOpen(false);
      } catch (downloadErr) {
          console.error("PDF export failed", downloadErr);
          showExportError(`PDF export failed: ${String(downloadErr)}`);
      }

      // Telemetry: Feature Heatmap
      if (exportOk) {
          invoke("track_event", { 
              event: "app_feature_used", 
              meta: { feature: "export_pdf", filename: filename, item_count: items.length } 
          }).catch(console.error);
      }
  };

  const selectedResources = getSelectedResources();
  const totalSavings = resources.reduce((sum, r) => sum + r.estimated_monthly_cost, 0);
  const totalCo2e = estimateAggregateCo2e(resources);
  const totalSelectedSavings = selectedResources.reduce((sum, r) => sum + r.estimated_monthly_cost, 0);
  const totalSelectedCo2e = estimateAggregateCo2e(selectedResources);

  if (!loading && resources.length === 0) {
    return (
        <div className="p-8 space-y-6 bg-slate-50 dark:bg-slate-900 min-h-screen transition-colors duration-300">
            <div className="flex justify-between items-center">
                <h2 className="text-2xl font-bold text-slate-900 dark:text-white">Identified Waste</h2>
                <button onClick={fetchData} className="p-2 text-slate-400 hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors">
                    <RefreshCw className="w-5 h-5" />
                </button>
            </div>
            <div className="p-12 text-center text-slate-500 dark:text-slate-400 bg-white dark:bg-slate-800 rounded-xl border border-slate-200 dark:border-slate-700">
                <div className="w-16 h-16 bg-slate-100 dark:bg-slate-700 rounded-full flex items-center justify-center mx-auto mb-4 text-slate-400">
                    <AlertCircle className="w-8 h-8" />
                </div>
                <h2 className="text-xl font-semibold mb-2 text-slate-900 dark:text-white">No Issues Found</h2>
                <p className="text-slate-500 dark:text-slate-400">Your cloud infrastructure is clean, or you haven't run a scan yet.</p>
            </div>
        </div>
    );
  }

  return (
    <div className="p-8 space-y-6 pb-24 bg-slate-50 dark:bg-slate-900 min-h-screen dark:text-slate-100 transition-colors duration-300">
      {isDemo && (
          <div className="bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 text-amber-800 dark:text-amber-400 px-4 py-3 rounded-lg flex items-center shadow-sm">
              <AlertCircle className="w-5 h-5 mr-2 text-amber-600 dark:text-amber-500" />
              <div><span className="font-bold">Demo Mode Active:</span> You are viewing simulated data.</div>
          </div>
      )}

      <PageHeader
        title="Scan Results"
        subtitle="Review actionable findings from the latest scan, then prepare cleanup or owner handoff."
        icon={<ClipboardList className="w-6 h-6" />}
        actions={
          <div className="flex gap-2">
            <button onClick={() => setExportModalOpen(true)} className="flex items-center px-3 py-2 bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-100 dark:border-indigo-800 text-indigo-700 dark:text-indigo-400 rounded-lg hover:bg-indigo-100 dark:hover:bg-indigo-900/40 text-sm font-medium transition-colors">
              <FileText className="w-4 h-4 mr-2" /> PDF Report
            </button>
            <button onClick={handleExportCSV} className="flex items-center px-3 py-2 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-200 rounded-lg hover:bg-slate-50 dark:hover:bg-slate-700 text-sm font-medium transition-colors">
              <Download className="w-4 h-4 mr-2" /> CSV
            </button>
            <button onClick={fetchData} className="p-2 text-slate-400 hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors rounded-lg hover:bg-slate-100 dark:hover:bg-slate-700 border border-transparent hover:border-slate-200 dark:hover:border-slate-600">
              <RefreshCw className="w-5 h-5" />
            </button>
          </div>
        }
      />

      {exportError && (
        <p className="text-sm font-medium text-rose-600 dark:text-rose-400">
          {exportError}
        </p>
      )}
      {actionNotice && (
        <p
          className={`text-sm font-medium ${
            actionNotice.type === "error"
              ? "text-rose-600 dark:text-rose-400"
              : "text-emerald-600 dark:text-emerald-400"
          }`}
        >
          {actionNotice.text}
        </p>
      )}
      {aiContext && (
        <div className="rounded-xl border border-indigo-200 bg-indigo-50 px-4 py-3 text-sm text-indigo-800 shadow-sm dark:border-indigo-900/40 dark:bg-indigo-950/30 dark:text-indigo-200">
          <span className="font-semibold">Opened from AI Analyst:</span> {aiContext}
        </div>
      )}

      <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="grid gap-4 md:grid-cols-3">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Use This For</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              Review the latest actionable findings, choose what should be ignored or executed, and prepare a handoff package.
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Current Slice</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              {filteredResources.length} visible findings, {selectedIds.size} selected, estimated {format(filteredSavings)}/mo and {formatCo2eKg(filteredCo2e.totalMonthlyCo2eKg)}/mo CO2e in the current filtered set.
              {showOnlyDeleteActions ? " Delete-only review is active." : ""}
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Operator Workflow</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              Filter and search first, then select findings, export PDF or CSV if review is needed, or generate a cleanup plan when execution is ready.
            </p>
          </div>
        </div>
      </div>

      {/* Filters */}
      <div className="flex flex-col md:flex-row gap-4 bg-white dark:bg-slate-800 p-4 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm transition-colors">
          <div className="relative flex-1">
              <Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
              <input
                  type="text"
                  placeholder="Search by ID, type or details..."
                  className="w-full pl-10 pr-4 py-2 border border-slate-200 dark:border-slate-600 rounded-lg text-sm bg-white dark:bg-slate-700 text-slate-900 dark:text-white focus:ring-2 focus:ring-indigo-500 outline-none placeholder-slate-400"
                  value={searchQuery}
                  onChange={(e) => handleSearchQueryChange(e.target.value)}
              />
          </div>
          <div className="flex items-center gap-2 w-full md:w-auto">
              <span className="text-xs font-bold text-slate-400 uppercase">Provider:</span>
              <div className="relative w-full sm:w-80">
                  <CustomSelect value={filterProvider} onChange={setFilterProvider} searchable searchPlaceholder="Search provider..."
                    options={CLOUD_PROVIDER_FILTER_OPTIONS}
                  />
              </div>
          </div>
          <label className="inline-flex items-center gap-2 rounded-lg border border-slate-200 px-3 py-2 text-sm font-medium text-slate-700 dark:border-slate-700 dark:text-slate-200">
              <input
                  type="checkbox"
                  checked={showOnlyDeleteActions}
                  onChange={(e) => setShowOnlyDeleteActions(e.target.checked)}
                  className="h-4 w-4 rounded border-slate-300 text-indigo-600 focus:ring-indigo-500"
              />
              Delete candidates only
          </label>
      </div>

      {(filterAccountKey || filterResourceType || showOnlyDeleteActions) && (
          <div className="flex flex-wrap items-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-3 text-sm text-slate-600 shadow-sm dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300">
              {filterAccountKey ? (
                  <span className="inline-flex items-center rounded-full bg-slate-100 px-3 py-1 font-semibold dark:bg-slate-700">
                      Account Filter: {filterAccountKey}
                  </span>
              ) : null}
              {filterResourceType ? (
                  <span className="inline-flex items-center rounded-full bg-slate-100 px-3 py-1 font-semibold dark:bg-slate-700">
                      Resource Type: {filterResourceType}
                  </span>
              ) : null}
              {showOnlyDeleteActions ? (
                  <span className="inline-flex items-center rounded-full bg-rose-50 px-3 py-1 font-semibold text-rose-700 dark:bg-rose-900/20 dark:text-rose-300">
                      Action Filter: DELETE
                  </span>
              ) : null}
          </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="bg-white dark:bg-slate-800 p-4 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
              <p className="text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-1">Potential Savings</p>
              <p className="text-2xl font-bold text-green-600 dark:text-green-400">{format(totalSavings)}<span className="text-sm text-slate-400 font-normal">/mo</span></p>
          </div>
          <div className="bg-white dark:bg-slate-800 p-4 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
              <p className="text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-1 flex items-center">
                  <Leaf className="w-3 h-3 mr-1 text-teal-600 dark:text-teal-400" /> Estimated CO2e Reduction
              </p>
              <p className="text-2xl font-bold text-teal-600 dark:text-teal-400">{formatCo2eKg(totalCo2e.totalMonthlyCo2eKg)}<span className="text-sm text-slate-400 font-normal">/mo</span></p>
          </div>
          <div className="bg-white dark:bg-slate-800 p-4 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
              <p className="text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-1">Estimated Annual CO2e</p>
              <p className="text-2xl font-bold text-slate-900 dark:text-white">{formatCo2eTonsFromKg(totalCo2e.totalAnnualCo2eKg)}</p>
          </div>
      </div>
      <div className="grid gap-4 md:grid-cols-3">
          <MetricCard
              label="Visible Findings"
              value={filteredResources.length}
              hint={showOnlyDeleteActions ? "Rows after provider, keyword, and delete-action filters." : "Rows after provider and keyword filters."}
          />
          <MetricCard
              label="Selected Findings"
              value={selectedIds.size}
              hint="Selection drives plan generation and ignore actions."
          />
          <MetricCard
              label="Filtered Savings"
              value={<span className="text-emerald-600 dark:text-emerald-400">{format(filteredSavings)}</span>}
              hint={`Filtered CO2e: ${formatCo2eKg(filteredCo2e.totalMonthlyCo2eKg)}/mo`}
          />
      </div>
      <div className="grid gap-4 md:grid-cols-3">
          <MetricCard
              label="Accounts In Scope"
              value={visibleAccountCount}
              hint="Distinct accounts represented in the current filtered slice."
          />
          <MetricCard
              label="Delete Candidates"
              value={deleteCandidateCount}
              hint="Rows currently marked with DELETE as the suggested action."
          />
          <MetricCard
              label="Rightsize Candidates"
              value={rightsizeCandidateCount}
              hint="Rows currently marked with RIGHTSIZE as the suggested action."
          />
      </div>
      <div className="text-xs text-slate-500 dark:text-slate-400 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl px-4 py-3">
          {ESG_METHODOLOGY_NOTE} {ESG_DISCLAIMER_NOTE}
      </div>
      
      {/* Table */}
      <div className="bg-white dark:bg-slate-800 rounded-xl shadow-sm border border-slate-200 dark:border-slate-700 overflow-hidden transition-colors">
        <div className="overflow-x-auto">
            <table className="w-full text-left text-sm text-slate-600 dark:text-slate-300">
            <thead className="bg-slate-50 dark:bg-slate-700/50 text-slate-900 dark:text-white font-semibold border-b border-slate-200 dark:border-slate-700">
                <tr>
                <th className="px-6 py-4 w-12">
                    <button onClick={toggleSelectAll} className="flex items-center text-slate-400 hover:text-slate-600 dark:hover:text-slate-200">
                        {selectedIds.size > 0 && selectedIds.size === filteredResources.length ? <CheckSquare className="w-5 h-5 text-indigo-600 dark:text-indigo-400" /> : <Square className="w-5 h-5" />}
                    </button>
                </th>
                <th className="px-6 py-4">Resource ID</th>
                <th className="px-6 py-4">Provider</th>
                <th className="px-6 py-4">Account</th>
                <th className="px-6 py-4">Type</th>
                <th className="px-6 py-4">Action</th>
                <th className="px-6 py-4">Details</th>
                <th className="px-6 py-4 text-right">Monthly Cost</th>
                <th className="px-6 py-4 text-right">Est. CO2e/mo</th>
                </tr>
            </thead>
            <tbody className="divide-y divide-slate-100 dark:divide-slate-700/50">
                {loading && (
                    <tr>
                        <td colSpan={9} className="p-8 text-center text-slate-500 dark:text-slate-400 animate-pulse">
                            Loading resources...
                        </td>
                    </tr>
                )}
                {!loading && filteredResources.map((r) => (
                <tr key={r.id} className={`hover:bg-slate-50 dark:hover:bg-slate-700/50 transition-colors ${selectedIds.has(r.id) ? 'bg-indigo-50/50 dark:bg-indigo-900/20' : ''}`}>
                    <td className="px-6 py-4">
                        <button onClick={() => toggleSelect(r.id)} className="flex items-center">
                            {selectedIds.has(r.id) ? <CheckSquare className="w-5 h-5 text-indigo-600 dark:text-indigo-400" /> : <Square className="w-5 h-5 text-slate-300 dark:text-slate-400" />}
                        </button>
                    </td>
                    <td className="px-6 py-4 text-sm text-slate-700 dark:text-slate-300 select-all">{r.id}</td>
                    <td className="px-6 py-4">
                        <span className={`px-2 py-1 rounded-md text-xs font-bold border ${ 
                            r.provider === "AWS" ? "bg-orange-50 dark:bg-orange-900/20 text-orange-700 dark:text-orange-400 border-orange-100 dark:border-orange-800" : 
                            r.provider === "Azure" ? "bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-300 dark:text-blue-400 border-blue-100 dark:border-blue-800" : 
                            r.provider === "GCP" ? "bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300 dark:text-red-400 border-red-100 dark:border-red-800" :
                            "bg-slate-100 dark:bg-slate-700 text-slate-600 dark:text-slate-300 border-slate-200 dark:border-slate-600"
                        }`}>
                            {r.provider}
                        </span>
                    </td>
                    <td className="px-6 py-4 text-sm text-slate-600 dark:text-slate-300">
                        {r.account_name || r.account_id || "Unattributed"}
                    </td>
                    <td className="px-6 py-4 font-medium text-slate-900 dark:text-white">{r.resource_type}</td>
                    <td className="px-6 py-4">
                        <span className={`px-2 py-1 rounded text-[10px] font-bold uppercase tracking-wider ${ 
                            r.action_type === 'DELETE' ? 'bg-red-100 text-red-700 dark:text-red-300 dark:bg-red-900/30 dark:text-red-400' :
                            r.action_type === 'RIGHTSIZE' ? 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400' :
                            r.action_type === 'ARCHIVE' ? 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-400' :
                            'bg-blue-100 text-blue-700 dark:text-blue-300'
                        }`}>
                            {r.action_type}
                        </span>
                    </td>
                    <td className="px-6 py-4 text-slate-500 dark:text-slate-400 max-w-xs truncate" title={r.details}>{r.details}</td>
                    <td className="px-6 py-4 text-right font-bold text-slate-900 dark:text-white">{format(r.estimated_monthly_cost)}</td>
                    <td className="px-6 py-4 text-right font-semibold text-teal-700 dark:text-teal-300">{formatCo2eKg(estimateResourceCo2e(r).monthlyCo2eKg)}</td>
                </tr>
                ))}
                {!loading && filteredResources.length === 0 && (
                    <tr><td colSpan={9} className="p-8 text-center text-slate-400 dark:text-slate-500 italic">No resources match your search or filters.</td></tr>
                )}
            </tbody>
            </table>
        </div>
      </div>

      {/* Floating Action Bar */}
      {selectedIds.size > 0 && (
          <div className="fixed bottom-8 left-1/2 transform -translate-x-1/2 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 shadow-2xl rounded-full px-6 py-3 flex items-center gap-6 animate-in slide-in-from-bottom-10 fade-in duration-300 z-40">
              <div className="text-sm font-medium text-slate-700 dark:text-slate-200">
                  <span className="font-bold text-slate-900 dark:text-white">{selectedIds.size}</span> items selected 
                  <span className="mx-2 text-slate-300 dark:text-slate-400">|</span> 
                  Est. Savings: <span className="text-green-600 dark:text-green-400 font-bold">{format(totalSelectedSavings)}/mo</span>
                  <span className="mx-2 text-slate-300 dark:text-slate-400">|</span>
                  Est. CO2e: <span className="text-teal-600 dark:text-teal-400 font-bold">{formatCo2eKg(totalSelectedCo2e.totalMonthlyCo2eKg)}/mo</span>
              </div>
              <button 
                onClick={handleMarkHandled}
                className="bg-slate-200 dark:bg-slate-700 text-slate-700 dark:text-slate-200 px-4 py-2 rounded-full text-sm font-bold hover:bg-slate-300 dark:hover:bg-slate-600 transition-all flex items-center mr-2"
              >
                  <EyeOff className="w-4 h-4 mr-2" />
                  Ignore
              </button>
              <button 
                onClick={handleGeneratePlan}
                className="bg-indigo-600 text-white px-4 py-2 rounded-full text-sm font-bold hover:bg-indigo-700 shadow-md hover:shadow-lg transition-all flex items-center"
              >
                  <ClipboardList className="w-4 h-4 mr-2" />
                  Generate Plan
              </button>
          </div>
      )}

      {/* Execution Plan Modal */}
      <Modal
        isOpen={!!confirmAction}
        onClose={() => {
            if (!confirmingAction) {
                setConfirmAction(null);
            }
        }}
        title={confirmAction === "execute_plan" ? "Confirm Cleanup Execution" : "Confirm Mark as Handled"}
        footer={
            <div className="flex gap-2">
                <button
                  onClick={() => setConfirmAction(null)}
                  disabled={confirmingAction}
                  className="px-4 py-2 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 rounded-lg font-medium disabled:opacity-50"
                >
                  Cancel
                </button>
                <button
                  onClick={runConfirmedAction}
                  disabled={confirmingAction}
                  className={`px-4 py-2 rounded-lg font-medium text-white disabled:opacity-60 ${
                      confirmAction === "execute_plan"
                          ? "bg-rose-600 hover:bg-rose-700"
                          : "bg-indigo-600 hover:bg-indigo-700"
                  }`}
                >
                  {confirmingAction
                      ? "Processing..."
                      : (confirmAction === "execute_plan" ? "Execute Cleanup" : "Mark as Handled")}
                </button>
            </div>
        }
      >
          <p className="text-sm text-slate-600 dark:text-slate-300">
              {confirmAction === "execute_plan"
                  ? "Run cleanup on selected resources? DELETE actions are permanent."
                  : "Mark selected resources as handled and hide them from future scans?"}
          </p>
      </Modal>

      <Modal
        isOpen={isPlanModalOpen}
        onClose={() => setPlanModalOpen(false)}
        title="Execution Plan Review"
        footer={
            <div className="flex gap-2 w-full justify-between items-center">
                <div className="text-sm text-slate-500">
                    {executionResult && <span className={executionResult.startsWith('Success') ? 'text-green-600 font-bold' : 'text-red-600 font-bold'}>{executionResult}</span>}
                </div>
                <div className="flex gap-2">
                    <button 
                        onClick={() => performExportPDF(getSelectedResources(), "execution_plan")} 
                        className="px-4 py-2 border border-slate-200 dark:border-slate-700 text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800 rounded-lg font-medium flex items-center"
                    >
                        <Download className="w-4 h-4 mr-2" /> Download PDF
                    </button>
                    <button 
                        onClick={handleExecutePlan} 
                        className="px-4 py-2 bg-indigo-600 text-white hover:bg-indigo-700 rounded-lg font-medium flex items-center disabled:opacity-50 disabled:cursor-not-allowed"
                        disabled={isExecuting || !!executionResult}
                    >
                        {isExecuting ? <Loader2 className="w-4 h-4 mr-2 animate-spin" /> : <Play className="w-4 h-4 mr-2" />}
                        {isExecuting ? "Executing..." : "Execute via API"}
                    </button>
                </div>
            </div>
        }
      >
          <div className="space-y-4">
              <div className="p-4 bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-100 dark:border-indigo-800 rounded-lg text-indigo-800 dark:text-indigo-300 text-sm">
                  <p className="font-bold flex items-center mb-1"><ClipboardList className="w-4 h-4 mr-2" /> Step 1: Review Plan</p>
                  <p>Please review the list below. You can download this plan as a PDF for internal approval before execution.</p>
              </div>
              
              <div>
                  <p className="text-sm font-medium text-slate-700 dark:text-slate-300 mb-2">Planned Actions ({selectedResources.length}):</p>
                  <div className="bg-slate-50 dark:bg-slate-900/50 rounded-lg border border-slate-200 dark:border-slate-700 max-h-64 overflow-y-auto p-2">
                      <table className="w-full text-xs text-left">
                          <thead className="text-slate-400 font-bold border-b border-slate-200 dark:border-slate-700">
                              <tr>
                                  <th className="p-2">Action</th>
                                  <th className="p-2">Resource</th>
                                  <th className="p-2 text-right">Savings</th>
                                  <th className="p-2 text-right">CO2e</th>
                              </tr>
                          </thead>
                          <tbody className="divide-y divide-slate-100 dark:divide-slate-800">
                              {selectedResources.map(r => (
                                  <tr key={r.id}>
                                      <td className={`p-2 font-bold ${r.action_type === 'DELETE' ? 'text-red-600' : 'text-indigo-600'}`}>{r.action_type}</td>
                                      <td className="p-2 text-slate-600 dark:text-slate-400">{r.id} ({r.resource_type})</td>
                                      <td className="p-2 text-right font-mono">{format(r.estimated_monthly_cost)}</td>
                                      <td className="p-2 text-right font-mono text-teal-600 dark:text-teal-400">{formatCo2eKg(estimateResourceCo2e(r).monthlyCo2eKg)}</td>
                                  </tr>
                              ))}
                          </tbody>
                      </table>
                  </div>
              </div>
          </div>
      </Modal>

      {/* Export Config Modal (Existing) */}
      <Modal
        isOpen={isExportModalOpen}
        onClose={() => setExportModalOpen(false)}
        title="Export Report Configuration"
        footer={
            <div className="flex gap-2 w-full justify-end">
                <button 
                    onClick={() => setExportModalOpen(false)} 
                    className="px-4 py-2 text-slate-600 hover:text-slate-800 dark:text-slate-400 dark:hover:text-slate-200 rounded-lg font-medium transition-colors"
                >
                    Cancel
                </button>
                <button 
                    onClick={() => performExportPDF()} 
                    className="px-6 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg font-bold flex items-center shadow-md hover:shadow-lg transition-all"
                >
                    <Download className="w-4 h-4 mr-2" />
                    Download PDF
                </button>
            </div>
        }
      >
          <div className="space-y-5">
              <p className="text-sm text-slate-500 dark:text-slate-400 leading-relaxed">
                  Customize the report header for your stakeholders. This information will appear at the top of the generated PDF.
              </p>
              
              <div>
                  <label className="text-xs font-bold text-slate-400 dark:text-slate-500 uppercase mb-2 block">Report Title</label>
                  <input 
                    type="text" 
                    value={exportConfig.title} 
                    onChange={e => setExportConfig({...exportConfig, title: e.target.value})}
                    className="w-full p-3 border border-slate-200 dark:border-slate-700 rounded-xl text-sm bg-slate-50 dark:bg-slate-900 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent transition-all"
                  />
              </div>

              <div>
                  <label className="text-xs font-bold text-slate-400 dark:text-slate-500 uppercase mb-2 block">Company / Organization</label>
                  <input 
                    type="text" 
                    placeholder="e.g. Acme Corp"
                    value={exportConfig.company} 
                    onChange={e => setExportConfig({...exportConfig, company: e.target.value})}
                    className="w-full p-3 border border-slate-200 dark:border-slate-700 rounded-xl text-sm bg-slate-50 dark:bg-slate-900 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent placeholder-slate-400 transition-all"
                  />
              </div>

              <div>
                  <label className="text-xs font-bold text-slate-400 dark:text-slate-500 uppercase mb-2 block">Prepared By</label>
                  <input 
                    type="text" 
                    placeholder="Your Name"
                    value={exportConfig.preparedBy} 
                    onChange={e => setExportConfig({...exportConfig, preparedBy: e.target.value})}
                    className="w-full p-3 border border-slate-200 dark:border-slate-700 rounded-xl text-sm bg-slate-50 dark:bg-slate-900 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent placeholder-slate-400 transition-all"
                  />
              </div>
              <div>
                  <label className="text-xs font-bold text-slate-400 dark:text-slate-500 uppercase mb-2 block">Add checklist columns</label>
                  <label className="flex items-start gap-2 rounded-xl border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-900 p-3">
                      <input
                          type="checkbox"
                          className="mt-0.5 h-4 w-4 accent-indigo-600"
                          checked={Boolean(exportConfig.includeExecutionColumns)}
                          onChange={(event) =>
                              setExportConfig({ ...exportConfig, includeExecutionColumns: event.target.checked })
                          }
                      />
                      <span className="text-sm text-slate-700 dark:text-slate-200">Add checklist columns</span>
                  </label>
              </div>
          </div>
      </Modal>
    </div>
  );
}
