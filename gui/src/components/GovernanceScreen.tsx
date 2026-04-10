import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Building2, RefreshCw, Copy, FileText, Download, Leaf, ShieldCheck, Server, Wallet, BarChart3, AlertTriangle } from "lucide-react";
import { useCurrency } from "../hooks/useCurrency";
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title,
  Tooltip,
  Legend,
} from "chart.js";
import { Line, Bar } from "react-chartjs-2";
import { exportBlobWithTauriFallback, exportTextWithTauriFallback, revealExportedFileInFolder } from "../utils/fileExport";
import { drawPdfBrandHeader, drawPdfFooterSiteLink, formatPdfDateTime } from "../utils/pdfBranding";
import { loadPdfRuntime } from "../utils/pdfRuntime";
import { PageHeader } from "./layout/PageHeader";
import { MetricCard } from "./ui/MetricCard";

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title,
  Tooltip,
  Legend
);

interface GovernanceScorecard {
  scan_runs: number;
  findings: number;
  positive_scan_runs: number;
  positive_scan_rate_pct: number;
  identified_savings: number;
  estimated_co2e_kg_monthly: number;
  avg_savings_per_scan: number;
  avg_findings_per_scan: number;
  active_accounts: number;
  active_providers: number;
  scan_checks_attempted: number;
  scan_checks_succeeded: number;
  scan_checks_failed: number;
  scan_check_success_rate_pct: number;
  last_scan_at: number | null;
}

interface GovernanceDailyPoint {
  day_ts: number;
  day_label: string;
  day_date: string;
  scan_runs: number;
  positive_scan_runs: number;
  findings: number;
  savings: number;
  estimated_co2e_kg_monthly: number;
  scan_checks_attempted: number;
  scan_checks_succeeded: number;
  scan_checks_failed: number;
  check_success_rate_pct: number;
}

interface GovernanceProviderRow {
  provider: string;
  scan_runs: number;
  findings: number;
  savings: number;
  estimated_co2e_kg_monthly: number;
  positive_scan_runs: number;
}

interface GovernanceAccountRow {
  account: string;
  scan_runs: number;
  coverage_pct: number;
}

interface GovernanceErrorCategoryRow {
  category: string;
  label: string;
  count: number;
  ratio_pct: number;
}

interface GovernanceErrorTaxonomy {
  taxonomy_version: string;
  total_failed_checks: number;
  categories: GovernanceErrorCategoryRow[];
}

interface GovernanceStatsResponse {
  generated_at: number;
  window_days: number;
  window_start_ts: number;
  window_end_ts: number;
  scorecard: GovernanceScorecard;
  daily: GovernanceDailyPoint[];
  providers: GovernanceProviderRow[];
  accounts: GovernanceAccountRow[];
  error_taxonomy: GovernanceErrorTaxonomy;
}

function formatUtcDateTime(ts: number | null | undefined): string {
  if (!ts) return "-";
  const date = new Date(ts * 1000);
  if (Number.isNaN(date.getTime())) return "-";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getUTCFullYear()}-${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())} ${pad(date.getUTCHours())}:${pad(date.getUTCMinutes())} UTC`;
}

function formatPct(value: number): string {
  if (!Number.isFinite(value)) return "0.0%";
  return `${value.toFixed(1)}%`;
}

function formatKg(value: number): string {
  if (!Number.isFinite(value)) return "0.0 kg";
  return `${value.toFixed(1)} kg`;
}

function sanitizePdfText(value: unknown): string {
  return String(value ?? "")
    .normalize("NFKD")
    .replace(/[^\x20-\x7E]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function formatDateForFilename(value: number): string {
  const date = new Date(value * 1000);
  if (Number.isNaN(date.getTime())) return "unknown-date";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getUTCFullYear()}-${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())}`;
}

function csvEscape(value: unknown): string {
  const text = String(value ?? "");
  const escaped = text.replace(/"/g, '""');
  if (/[",\n]/.test(escaped)) {
    return `"${escaped}"`;
  }
  return escaped;
}

export function GovernanceScreen() {
  const [windowDays, setWindowDays] = useState<number>(30);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [copyHint, setCopyHint] = useState<string>("");
  const [data, setData] = useState<GovernanceStatsResponse | null>(null);
  const { format: formatCurrency } = useCurrency();

  const loadGovernance = useCallback(async (silent = false) => {
    if (!silent) {
      setLoading(true);
    }
    setError(null);
    try {
      const isDemo = localStorage.getItem("cws_is_demo_mode") === "true";
      const res = await invoke<GovernanceStatsResponse>("get_governance_stats", {
        windowDays,
        demoMode: isDemo,
      });
      setData(res);
    } catch (err) {
      setError(String(err));
    } finally {
      if (!silent) {
        setLoading(false);
      }
    }
  }, [windowDays]);

  useEffect(() => {
    void loadGovernance(false);
  }, [loadGovernance]);

  const score = data?.scorecard;
  const providers = data?.providers || [];
  const accounts = data?.accounts || [];
  const daily = data?.daily || [];
  const errorTaxonomy = data?.error_taxonomy;
  const isDemo = localStorage.getItem("cws_is_demo_mode") === "true";

  const isDark = document.documentElement.classList.contains("dark");
  const axisTextColor = isDark ? "#94a3b8" : "#64748b";
  const gridColor = isDark ? "rgba(51, 65, 85, 0.45)" : "rgba(148, 163, 184, 0.25)";
  const chartBaseOptions = useMemo(() => ({
    responsive: true,
    maintainAspectRatio: false,
    interaction: { mode: "index" as const, intersect: false },
    plugins: {
      legend: {
        position: "top" as const,
        align: "start" as const,
        labels: {
          color: axisTextColor,
          usePointStyle: true,
          pointStyle: "circle" as const,
          boxWidth: 9,
          boxHeight: 9,
          padding: 16,
          font: {
            size: 11,
            weight: 600 as const,
          },
        },
      },
      tooltip: {
        backgroundColor: "rgba(15, 23, 42, 0.96)",
        titleColor: "#f8fafc",
        bodyColor: "#cbd5e1",
        borderColor: "rgba(148, 163, 184, 0.18)",
        borderWidth: 1,
        padding: 10,
      },
    },
    elements: {
      line: {
        borderCapStyle: "round" as const,
        borderJoinStyle: "round" as const,
      },
      point: {
        hoverBorderWidth: 2,
      },
    },
    scales: {
      x: {
        ticks: { color: axisTextColor, maxRotation: 0, autoSkip: true },
        grid: { color: gridColor, drawBorder: false },
        border: { display: false },
      },
      y: {
        beginAtZero: true,
        ticks: { color: axisTextColor },
        grid: { color: gridColor, drawBorder: false },
        border: { display: false },
      },
    },
  }), [axisTextColor, gridColor]);

  const savingsTrendData: any = useMemo(() => ({
    labels: daily.map((item) => item.day_label),
    datasets: [
      {
        label: "Identified Savings",
        data: daily.map((item) => Number(item.savings || 0)),
        borderColor: "#2563eb",
        backgroundColor: "rgba(37, 99, 235, 0.18)",
        yAxisID: "y",
        tension: 0.3,
        fill: true,
      },
      {
        label: "Estimated CO2e (kg/mo)",
        data: daily.map((item) => Number(item.estimated_co2e_kg_monthly || 0)),
        borderColor: "#059669",
        backgroundColor: "rgba(5, 150, 105, 0.16)",
        yAxisID: "y1",
        tension: 0.3,
        fill: true,
      },
    ],
  }), [daily]);

  const savingsTrendOptions: any = useMemo(() => ({
    ...chartBaseOptions,
    plugins: {
      ...chartBaseOptions.plugins,
      tooltip: {
        callbacks: {
          label: (ctx: any) => {
            if (ctx.dataset?.label?.includes("Savings")) {
              return `${ctx.dataset.label}: ${formatCurrency(Number(ctx.raw || 0))}`;
            }
            return `${ctx.dataset.label}: ${Number(ctx.raw || 0).toFixed(2)} kg`;
          },
        },
      },
    },
    scales: {
      x: {
        ...chartBaseOptions.scales.x,
      },
      y: {
        ...chartBaseOptions.scales.y,
        ticks: {
          color: axisTextColor,
          callback: (value: number) => formatCurrency(Number(value || 0)),
        },
      },
      y1: {
        position: "right",
        ticks: {
          color: axisTextColor,
          callback: (value: number) => `${Number(value || 0).toFixed(0)} kg`,
        },
        grid: { drawOnChartArea: false },
        border: { display: false },
      },
    },
  }), [axisTextColor, chartBaseOptions, formatCurrency]);

  const activityTrendData: any = useMemo(() => ({
    labels: daily.map((item) => item.day_label),
    datasets: [
      {
        label: "Scan Runs",
        data: daily.map((item) => item.scan_runs),
        backgroundColor: "rgba(99, 102, 241, 0.7)",
        borderRadius: 4,
      },
      {
        label: "Findings",
        data: daily.map((item) => item.findings),
        backgroundColor: "rgba(239, 68, 68, 0.7)",
        borderRadius: 4,
      },
      {
        label: "Failed Checks",
        data: daily.map((item) => item.scan_checks_failed),
        backgroundColor: "rgba(245, 158, 11, 0.7)",
        borderRadius: 4,
      },
    ],
  }), [daily]);

  const activityTrendOptions: any = useMemo(() => ({
    ...chartBaseOptions,
  }), [chartBaseOptions]);

  const recommendationLines = useMemo(() => {
    if (!score) return [];
    const lines: string[] = [];
    if (score.scan_check_success_rate_pct < 90) {
      lines.push("Improve credential and network reliability to raise scan check success above 90%.");
    }
    if (score.positive_scan_rate_pct > 35) {
      lines.push("High positive scan rate indicates recurring waste; assign top-provider owners in weekly review.");
    }
    if (score.active_accounts < 3) {
      lines.push("Coverage is narrow; onboard more production accounts to avoid governance blind spots.");
    }
    if ((errorTaxonomy?.total_failed_checks || 0) > 0) {
      const topCategory = (errorTaxonomy?.categories || []).reduce<GovernanceErrorCategoryRow | null>((best, item) => {
        if (item.count <= 0) return best;
        if (!best || item.count > best.count) return item;
        return best;
      }, null);
      if (topCategory) {
        lines.push(`Most failures are ${topCategory.label.toLowerCase()} (${topCategory.count} checks). Assign an owner and close root causes this sprint.`);
      }
    }
    if (lines.length === 0) {
      lines.push("Governance baseline is stable. Keep weekly review cadence and track provider-level closure trends.");
    }
    return lines;
  }, [score, errorTaxonomy]);

  const topErrorCategories = useMemo(() => {
    const rows = (errorTaxonomy?.categories || [])
      .filter((item) => item.count > 0)
      .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label));
    return rows.slice(0, 5);
  }, [errorTaxonomy]);

  const copyWeeklyPack = async () => {
    if (!data || !score) return;
    const topProviders = providers
      .slice(0, 5)
      .map((item, index) => `${index + 1}. ${item.provider}: ${formatCurrency(item.savings)} / ${item.findings} findings`)
      .join("\n");
    const topAccounts = accounts
      .slice(0, 5)
      .map((item, index) => `${index + 1}. ${item.account}: ${item.scan_runs} runs (${formatPct(item.coverage_pct)})`)
      .join("\n");
    const topErrors = topErrorCategories
      .map((item, index) => `${index + 1}. ${item.label}: ${item.count} (${formatPct(item.ratio_pct)})`)
      .join("\n");

    const text = [
      `Cloud Waste Scanner Governance Weekly Pack (${data.window_days}d)`,
      `Generated: ${formatUtcDateTime(data.generated_at)}`,
      "",
      `Scan Runs: ${score.scan_runs}`,
      `Findings: ${score.findings}`,
      `Identified Savings: ${formatCurrency(score.identified_savings)}`,
      `Estimated CO2e Reduction: ${formatKg(score.estimated_co2e_kg_monthly)} / month`,
      `Positive Scan Rate: ${formatPct(score.positive_scan_rate_pct)}`,
      `Scan Check Success Rate: ${formatPct(score.scan_check_success_rate_pct)}`,
      `Failed Checks (Taxonomy v${errorTaxonomy?.taxonomy_version || "1"}): ${errorTaxonomy?.total_failed_checks || 0}`,
      "",
      "Top Providers:",
      topProviders || "No provider data",
      "",
      "Top Account Coverage:",
      topAccounts || "No account data",
      "",
      "Top Error Categories:",
      topErrors || "No failed checks in selected window",
      "",
      "Recommendations:",
      ...recommendationLines.map((line, index) => `${index + 1}. ${line}`),
    ].join("\n");

    try {
      await navigator.clipboard.writeText(text);
      setCopyHint("Weekly governance pack copied.");
    } catch {
      setCopyHint("Copy failed. Please allow clipboard permission.");
    }
    window.setTimeout(() => setCopyHint(""), 2500);
  };

  const exportGovernancePdf = async () => {
    if (!data || !score) return;
    try {
      const { jsPDF, autoTable } = await loadPdfRuntime();
      const doc = new jsPDF();
      const docAny = doc as { lastAutoTable?: { finalY?: number } };
      const pageBottomY = 272;
      const pageTopY = 18;
      let cursorY = 0;
      const getLastTableY = (fallback: number) => docAny.lastAutoTable?.finalY ?? fallback;
      const startSection = (title: string, minBodyHeight = 18) => {
        const nextY = getLastTableY(cursorY) + 12;
        const sectionHeight = 7 + minBodyHeight;
        if (nextY + sectionHeight > pageBottomY) {
          doc.addPage();
          cursorY = pageTopY;
        } else {
          cursorY = nextY;
        }
        doc.setFontSize(14);
        doc.setTextColor(40, 40, 40);
        doc.text(title, 14, cursorY);
        return cursorY + 5;
      };

      const headerBottomY = drawPdfBrandHeader(doc, {
        title: "Governance Weekly Report",
        generatedAt: new Date(),
        extraLines: [
          `Window: ${data.window_days} days`,
          `Period: ${formatPdfDateTime(data.window_start_ts * 1000)} to ${formatPdfDateTime(data.window_end_ts * 1000)}`,
        ],
      });

      const summaryStart = headerBottomY + 6;
      autoTable(doc, {
        startY: summaryStart,
        head: [["Metric", "Value"]],
        body: [
          ["Scan Runs", sanitizePdfText(score.scan_runs)],
          ["Findings", sanitizePdfText(score.findings)],
          ["Positive Scans", sanitizePdfText(`${score.positive_scan_runs} (${formatPct(score.positive_scan_rate_pct)})`)],
          ["Identified Savings", sanitizePdfText(formatCurrency(score.identified_savings))],
          ["Estimated CO2e / month", sanitizePdfText(formatKg(score.estimated_co2e_kg_monthly))],
          ["Execution Quality", sanitizePdfText(formatPct(score.scan_check_success_rate_pct))],
          ["Failed Checks (Taxonomy)", sanitizePdfText(`${errorTaxonomy?.total_failed_checks || 0} (v${errorTaxonomy?.taxonomy_version || "1"})`)],
          ["Coverage", sanitizePdfText(`${score.active_accounts} accounts / ${score.active_providers} providers`)],
          ["Last Scan", sanitizePdfText(formatUtcDateTime(score.last_scan_at))],
        ],
        theme: "grid",
        headStyles: { fillColor: [79, 70, 229] },
        styles: { fontSize: 9 },
        margin: { left: 14, right: 14, bottom: 18 },
        columnStyles: {
          0: { cellWidth: 62 },
          1: { cellWidth: 120 },
        },
      });
      cursorY = getLastTableY(summaryStart);

      const trendStart = startSection("Daily Trend (Governance Execution)", 36);
      autoTable(doc, {
        startY: trendStart,
        head: [["Date", "Scans", "Findings", "Savings", "CO2e", "Check Success"]],
        body: daily.map((item) => [
          sanitizePdfText(item.day_date),
          sanitizePdfText(item.scan_runs),
          sanitizePdfText(item.findings),
          sanitizePdfText(formatCurrency(item.savings)),
          sanitizePdfText(formatKg(item.estimated_co2e_kg_monthly)),
          sanitizePdfText(formatPct(item.check_success_rate_pct)),
        ]),
        theme: "grid",
        headStyles: { fillColor: [51, 65, 85] },
        styles: { fontSize: 8 },
        margin: { left: 14, right: 14, bottom: 18 },
        columnStyles: {
          0: { cellWidth: 30 },
          1: { cellWidth: 20, halign: "right" },
          2: { cellWidth: 24, halign: "right" },
          3: { cellWidth: 36, halign: "right" },
          4: { cellWidth: 30, halign: "right" },
          5: { cellWidth: 40, halign: "right" },
        },
      });
      cursorY = getLastTableY(trendStart);

      const errorStart = startSection("Error Taxonomy (Standardized Categories)", 24);
      autoTable(doc, {
        startY: errorStart,
        head: [["Category", "Failed Checks", "Ratio"]],
        body: (topErrorCategories.length > 0 ? topErrorCategories : [{ label: "No failed checks", count: 0, ratio_pct: 0 }]).map((item: any) => [
          sanitizePdfText(item.label),
          sanitizePdfText(item.count),
          sanitizePdfText(formatPct(item.ratio_pct)),
        ]),
        theme: "grid",
        headStyles: { fillColor: [180, 83, 9] },
        styles: { fontSize: 8 },
        margin: { left: 14, right: 14, bottom: 18 },
        columnStyles: {
          0: { cellWidth: 96 },
          1: { cellWidth: 36, halign: "right" },
          2: { cellWidth: 56, halign: "right" },
        },
      });
      cursorY = getLastTableY(errorStart);

      const providerStart = startSection("Provider Breakdown", 24);
      autoTable(doc, {
        startY: providerStart,
        head: [["Provider", "Scans", "Findings", "Savings", "CO2e", "Positive"]],
        body: providers.map((item) => [
          sanitizePdfText(item.provider),
          sanitizePdfText(item.scan_runs),
          sanitizePdfText(item.findings),
          sanitizePdfText(formatCurrency(item.savings)),
          sanitizePdfText(formatKg(item.estimated_co2e_kg_monthly)),
          sanitizePdfText(item.positive_scan_runs),
        ]),
        theme: "grid",
        headStyles: { fillColor: [15, 118, 110] },
        styles: { fontSize: 8 },
        margin: { left: 14, right: 14, bottom: 18 },
        columnStyles: {
          0: { cellWidth: 50 },
          1: { cellWidth: 20, halign: "right" },
          2: { cellWidth: 24, halign: "right" },
          3: { cellWidth: 34, halign: "right" },
          4: { cellWidth: 30, halign: "right" },
          5: { cellWidth: 24, halign: "right" },
        },
      });
      cursorY = getLastTableY(providerStart);

      const accountStart = startSection("Account Coverage and Recommendations", 24);
      autoTable(doc, {
        startY: accountStart,
        head: [["Account", "Runs", "Coverage"]],
        body: accounts.slice(0, 20).map((item) => [
          sanitizePdfText(item.account),
          sanitizePdfText(item.scan_runs),
          sanitizePdfText(formatPct(item.coverage_pct)),
        ]),
        theme: "grid",
        headStyles: { fillColor: [99, 102, 241] },
        styles: { fontSize: 8 },
        margin: { left: 14, right: 14, bottom: 18 },
        columnStyles: {
          0: { cellWidth: 126 },
          1: { cellWidth: 20, halign: "right" },
          2: { cellWidth: 42, halign: "right" },
        },
      });
      cursorY = getLastTableY(accountStart);

      const recStart = startSection("Recommendations", 24);
      autoTable(doc, {
        startY: recStart,
        head: [["#", "Guidance"]],
        body: recommendationLines.map((line, index) => [
          sanitizePdfText(index + 1),
          sanitizePdfText(line),
        ]),
        theme: "grid",
        headStyles: { fillColor: [217, 119, 6] },
        styles: { fontSize: 8 },
        margin: { left: 14, right: 14, bottom: 18 },
        columnStyles: {
          0: { cellWidth: 12, halign: "right" },
          1: { cellWidth: 156 },
        },
      });

      const totalPages = doc.getNumberOfPages();
      for (let page = 1; page <= totalPages; page += 1) {
        doc.setPage(page);
        const pageWidth = doc.internal.pageSize.getWidth();
        const pageHeight = doc.internal.pageSize.getHeight();
        drawPdfFooterSiteLink(doc, pageWidth, pageHeight, page, totalPages);
      }

      const filename = `governance_weekly_pack_${formatDateForFilename(data.generated_at)}.pdf`;
      const blob = doc.output("blob");
      const savedPath = await exportBlobWithTauriFallback(blob, filename, { openAfterSave: false });
      if (savedPath) {
        await revealExportedFileInFolder(savedPath);
      }
      setCopyHint("Governance PDF exported.");
      window.setTimeout(() => setCopyHint(""), 2500);
    } catch (err) {
      setCopyHint(`Governance PDF export failed: ${String(err)}`);
      window.setTimeout(() => setCopyHint(""), 4000);
    }
  };

  const exportGovernanceCsv = async () => {
    if (!data || !score) return;
    try {
      const lines: string[] = [];
      lines.push("Section,Metric,Value");
      lines.push(`Summary,Window Days,${csvEscape(data.window_days)}`);
      lines.push(`Summary,Generated At,${csvEscape(formatUtcDateTime(data.generated_at))}`);
      lines.push(`Summary,Scan Runs,${csvEscape(score.scan_runs)}`);
      lines.push(`Summary,Findings,${csvEscape(score.findings)}`);
      lines.push(`Summary,Positive Scan Rate,${csvEscape(formatPct(score.positive_scan_rate_pct))}`);
      lines.push(`Summary,Identified Savings,${csvEscape(formatCurrency(score.identified_savings))}`);
      lines.push(`Summary,Estimated CO2e Monthly,${csvEscape(formatKg(score.estimated_co2e_kg_monthly))}`);
      lines.push(`Summary,Execution Quality,${csvEscape(formatPct(score.scan_check_success_rate_pct))}`);
      lines.push(`Summary,Failed Checks,${csvEscape(errorTaxonomy?.total_failed_checks || 0)}`);
      lines.push(`Summary,Coverage,${csvEscape(`${score.active_accounts} accounts / ${score.active_providers} providers`)}`);
      lines.push("");

      lines.push("Daily Trend");
      lines.push("Date,Scan Runs,Findings,Savings,Estimated CO2e,Check Success");
      for (const item of daily) {
        lines.push(
          [
            csvEscape(item.day_date),
            csvEscape(item.scan_runs),
            csvEscape(item.findings),
            csvEscape(formatCurrency(item.savings)),
            csvEscape(formatKg(item.estimated_co2e_kg_monthly)),
            csvEscape(formatPct(item.check_success_rate_pct)),
          ].join(","),
        );
      }
      lines.push("");

      lines.push("Provider Breakdown");
      lines.push("Provider,Scan Runs,Findings,Savings,Estimated CO2e,Positive Runs");
      for (const item of providers) {
        lines.push(
          [
            csvEscape(item.provider),
            csvEscape(item.scan_runs),
            csvEscape(item.findings),
            csvEscape(formatCurrency(item.savings)),
            csvEscape(formatKg(item.estimated_co2e_kg_monthly)),
            csvEscape(item.positive_scan_runs),
          ].join(","),
        );
      }
      lines.push("");

      lines.push("Account Coverage");
      lines.push("Account,Scan Runs,Coverage");
      for (const item of accounts) {
        lines.push(
          [
            csvEscape(item.account),
            csvEscape(item.scan_runs),
            csvEscape(formatPct(item.coverage_pct)),
          ].join(","),
        );
      }
      lines.push("");

      lines.push("Error Taxonomy");
      lines.push("Category,Failed Checks,Ratio");
      for (const item of topErrorCategories) {
        lines.push(
          [
            csvEscape(item.label),
            csvEscape(item.count),
            csvEscape(formatPct(item.ratio_pct)),
          ].join(","),
        );
      }
      if (topErrorCategories.length === 0) {
        lines.push("No failed checks,0,0.0%");
      }
      lines.push("");

      lines.push("Recommendations");
      lines.push("No,Guidance");
      recommendationLines.forEach((line, index) => {
        lines.push(`${index + 1},${csvEscape(line)}`);
      });

      const filename = `governance_weekly_pack_${formatDateForFilename(data.generated_at)}.csv`;
      const savedPath = await exportTextWithTauriFallback(
        `\uFEFF${lines.join("\n")}`,
        filename,
        "text/csv;charset=utf-8;",
        { openAfterSave: false },
      );
      if (savedPath) {
        await revealExportedFileInFolder(savedPath);
      }
      setCopyHint("Governance CSV exported.");
      window.setTimeout(() => setCopyHint(""), 2500);
    } catch (err) {
      setCopyHint(`Governance CSV export failed: ${String(err)}`);
      window.setTimeout(() => setCopyHint(""), 4000);
    }
  };

  if (loading) {
    return <div className="p-8 text-center text-slate-500 dark:text-slate-400 animate-pulse">Loading governance view...</div>;
  }

  if (error) {
    return (
      <div className="p-8 space-y-4 bg-slate-50 dark:bg-slate-900 min-h-screen">
        <div className="rounded-xl border border-rose-200 bg-rose-50 px-5 py-4 text-sm text-rose-700 dark:border-rose-900/60 dark:bg-rose-900/20 dark:text-rose-300">
          Failed to load governance view: {error}
        </div>
        <button
          onClick={() => {
            void loadGovernance(false);
          }}
          className="inline-flex items-center gap-2 rounded-lg bg-indigo-600 px-4 py-2 font-semibold text-white hover:bg-indigo-700"
        >
          <RefreshCw size={16} /> Retry
        </button>
      </div>
    );
  }

  return (
    <div className="p-8 space-y-6 bg-slate-50 dark:bg-slate-900 min-h-screen transition-colors duration-300">
      <PageHeader
        title="Governance"
        subtitle="Enterprise-focused governance metrics for ownership, execution quality, savings, and ESG impact."
        icon={<Building2 className="h-6 w-6" />}
        actions={
          <div className="flex flex-col items-end gap-2">
            <div className="flex flex-wrap items-center justify-end gap-2">
              {[7, 30, 90].map((days) => (
                <button
                  key={days}
                  onClick={() => setWindowDays(days)}
                  className={`rounded-lg border px-3 py-2 text-sm font-semibold transition-colors ${windowDays === days
                    ? "border-indigo-500 bg-indigo-600 text-white"
                    : "border-slate-300 bg-white text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
                    }`}
                >
                  {days} Days
                </button>
              ))}

              <button
                onClick={() => {
                  void loadGovernance(false);
                }}
                className="inline-flex items-center gap-2 rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-semibold text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <RefreshCw size={16} /> Refresh
              </button>

              <button
                onClick={copyWeeklyPack}
                className="inline-flex items-center gap-2 rounded-lg border border-indigo-200 bg-indigo-50 px-3 py-2 text-sm font-semibold text-indigo-700 hover:bg-indigo-100 dark:border-indigo-700/70 dark:bg-indigo-900/30 dark:text-indigo-200 dark:hover:bg-indigo-800/40"
              >
                <Copy size={16} /> Copy Weekly Pack
              </button>

              <button
                onClick={exportGovernancePdf}
                className="flex items-center rounded-lg border border-indigo-100 bg-indigo-50 px-3 py-2 text-sm font-medium text-indigo-700 transition-colors hover:bg-indigo-100 dark:border-indigo-800 dark:bg-indigo-900/20 dark:text-indigo-400 dark:hover:bg-indigo-900/40"
              >
                <FileText className="w-4 h-4 mr-2" /> PDF Report
              </button>
              <button
                onClick={exportGovernanceCsv}
                className="flex items-center rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm font-medium text-slate-700 transition-colors hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <Download className="w-4 h-4 mr-2" /> CSV
              </button>
            </div>
            <p className="max-w-[560px] text-right text-xs text-slate-500 dark:text-slate-400">
              Governance summarizes execution quality, owner coverage, and identified savings over the selected operating window.
            </p>
          </div>
        }
      />

      {isDemo && (
        <p className="inline-flex items-center rounded-lg border border-amber-200 bg-amber-50 px-3 py-1.5 text-xs font-semibold text-amber-700 dark:border-amber-800 dark:bg-amber-900/20 dark:text-amber-300">
          Demo mode data set active
        </p>
      )}

      {copyHint && (
        <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-2 text-sm font-semibold text-emerald-700 dark:border-emerald-900/60 dark:bg-emerald-900/20 dark:text-emerald-300">
          {copyHint}
        </div>
      )}

      <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="grid gap-4 md:grid-cols-3">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Use This For</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              Review whether scans are executing reliably, whether ownership is broad enough, and whether savings recur over time.
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Weekly Pack</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              Copy Weekly Pack for fast status sharing, or export PDF and CSV when finance or platform owners need a more formal handoff.
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Current Mode</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              {isDemo ? "Demo data is active." : "Live local data is active."} Window: {windowDays} days. Generated at {formatUtcDateTime(data?.generated_at)}.
            </p>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <MetricCard label="Scan Runs" value={score?.scan_runs || 0} hint={`Window: ${data?.window_days || 0} days`} icon={<Server className="h-5 w-5" />} />
        <MetricCard label="Findings" value={<span className="text-rose-600 dark:text-rose-400">{score?.findings || 0}</span>} hint={`Avg/scan: ${(score?.avg_findings_per_scan || 0).toFixed(2)}`} icon={<AlertTriangle className="h-5 w-5" />} />
        <MetricCard label="Identified Savings" value={<span className="text-emerald-600 dark:text-emerald-400">{formatCurrency(score?.identified_savings || 0)}</span>} hint={`Avg/scan: ${formatCurrency(score?.avg_savings_per_scan || 0)}`} icon={<Wallet className="h-5 w-5" />} />
        <MetricCard label="Estimated CO2e" value={<span className="text-emerald-700 dark:text-emerald-300">{formatKg(score?.estimated_co2e_kg_monthly || 0)}</span>} hint="Per month (estimated)" icon={<Leaf className="h-5 w-5" />} />
        <MetricCard label="Positive Scans" value={<span className="text-indigo-600 dark:text-indigo-400">{score?.positive_scan_runs || 0} <span className="text-base">({formatPct(score?.positive_scan_rate_pct || 0)})</span></span>} hint="Scan runs with waste found" icon={<Building2 className="h-5 w-5" />} />
        <MetricCard label="Execution Quality" value={formatPct(score?.scan_check_success_rate_pct || 0)} hint={`Checks: ${score?.scan_checks_succeeded || 0}/${score?.scan_checks_attempted || 0}`} icon={<ShieldCheck className="h-5 w-5" />} />
        <MetricCard label="Coverage" value={`${score?.active_accounts || 0} accounts`} hint={`Providers: ${score?.active_providers || 0}`} icon={<Server className="h-5 w-5" />} />
        <MetricCard label="Last Scan" value={<span className="text-base">{formatUtcDateTime(score?.last_scan_at)}</span>} hint={`Generated: ${formatUtcDateTime(data?.generated_at)}`} icon={<RefreshCw className="h-5 w-5" />} />
      </div>

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        <div className="rounded-xl border border-slate-200 bg-white p-5 dark:border-slate-800 dark:bg-slate-800/50">
          <h3 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
            <Wallet className="h-5 w-5 text-indigo-500" />
            Savings and ESG Trend
          </h3>
          <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">Daily trend for identified savings and estimated CO2e reduction.</p>
          <div className="mt-4 h-72">
            <Line data={savingsTrendData} options={savingsTrendOptions} />
          </div>
        </div>

        <div className="rounded-xl border border-slate-200 bg-white p-5 dark:border-slate-800 dark:bg-slate-800/50">
          <h3 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
            <BarChart3 className="h-5 w-5 text-indigo-500" />
            Throughput and Reliability
          </h3>
          <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">Daily scan runs, findings and failed checks for governance execution tracking.</p>
          <div className="mt-4 h-72">
            <Bar data={activityTrendData} options={activityTrendOptions} />
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-3">
        <div className="xl:col-span-2 rounded-xl border border-slate-200 bg-white p-5 dark:border-slate-800 dark:bg-slate-800/50 overflow-auto">
          <h3 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
            <Server className="h-5 w-5 text-indigo-500" />
            Provider Breakdown
          </h3>
          <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">Prioritize ownership by provider based on recurring savings and findings volume.</p>

          <table className="mt-4 min-w-full text-sm">
            <thead className="text-slate-500 uppercase text-xs">
              <tr>
                <th className="text-left py-2 pr-4">Provider</th>
                <th className="text-left py-2 pr-4">Scan Runs</th>
                <th className="text-left py-2 pr-4">Findings</th>
                <th className="text-left py-2 pr-4">Savings</th>
                <th className="text-left py-2 pr-4">CO2e</th>
                <th className="text-left py-2">Positive Runs</th>
              </tr>
            </thead>
            <tbody>
              {providers.length === 0 && (
                <tr>
                  <td colSpan={6} className="py-4 text-slate-400">No provider data in selected window.</td>
                </tr>
              )}
              {providers.map((row) => (
                <tr key={row.provider} className="border-t border-slate-100 dark:border-slate-800">
                  <td className="py-2 pr-4 font-semibold text-slate-900 dark:text-white">{row.provider}</td>
                  <td className="py-2 pr-4 text-slate-700 dark:text-slate-300">{row.scan_runs}</td>
                  <td className="py-2 pr-4 text-slate-700 dark:text-slate-300">{row.findings}</td>
                  <td className="py-2 pr-4 text-emerald-600 dark:text-emerald-400">{formatCurrency(row.savings)}</td>
                  <td className="py-2 pr-4 text-emerald-700 dark:text-emerald-300">{formatKg(row.estimated_co2e_kg_monthly)}</td>
                  <td className="py-2 text-indigo-600 dark:text-indigo-300">{row.positive_scan_runs}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <div className="rounded-xl border border-slate-200 bg-white p-5 dark:border-slate-800 dark:bg-slate-800/50">
          <h3 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
            <ShieldCheck className="h-5 w-5 text-indigo-500" />
            Account Coverage
          </h3>
          <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">Weekly meeting anchor: verify which account owners are continuously covered.</p>

          <div className="mt-4 max-h-72 overflow-auto space-y-2">
            {accounts.length === 0 && (
              <div className="text-sm text-slate-400">No scanned account metadata available.</div>
            )}
            {accounts.map((row) => (
              <div key={row.account} className="rounded-lg border border-slate-200 px-3 py-2 dark:border-slate-700">
                <div className="text-sm font-semibold text-slate-900 dark:text-white truncate" title={row.account}>{row.account}</div>
                <div className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                  {row.scan_runs} runs · {formatPct(row.coverage_pct)} coverage
                </div>
              </div>
            ))}
          </div>

          <div className="mt-5 rounded-lg border border-amber-200 bg-amber-50 px-3 py-3 text-xs text-amber-800 dark:border-amber-900/60 dark:bg-amber-900/20 dark:text-amber-300 space-y-2">
            <div className="font-semibold">Governance Recommendations</div>
            {recommendationLines.map((line, idx) => (
              <div key={`gov-rec-${idx}`} className="leading-relaxed">{idx + 1}. {line}</div>
            ))}
          </div>

          <div className="mt-4 rounded-lg border border-rose-200 bg-rose-50 px-3 py-3 text-xs text-rose-800 dark:border-rose-900/60 dark:bg-rose-900/20 dark:text-rose-300 space-y-2">
            <div className="font-semibold flex items-center gap-1.5">
              <AlertTriangle size={13} /> Error Taxonomy (Standardized)
            </div>
            <div className="text-[11px] opacity-90">
              Failed checks: {errorTaxonomy?.total_failed_checks || 0} · Taxonomy v{errorTaxonomy?.taxonomy_version || "1"}
            </div>
            {topErrorCategories.length === 0 && (
              <div className="leading-relaxed">No failed checks in selected window.</div>
            )}
            {topErrorCategories.map((item) => (
              <div key={`gov-err-${item.category}`} className="leading-relaxed">
                {item.label}: {item.count} ({formatPct(item.ratio_pct)})
              </div>
            ))}
          </div>

          <div className="mt-4 grid grid-cols-1 gap-2 text-xs text-slate-600 dark:text-slate-400">
            <div className="rounded-md bg-slate-100 px-3 py-2 dark:bg-slate-800/70 flex items-center gap-2">
              <Leaf size={14} className="text-emerald-500" /> ESG: Estimated impact only, intended for trend tracking.
            </div>
            <div className="rounded-md bg-slate-100 px-3 py-2 dark:bg-slate-800/70 flex items-center gap-2">
              <Server size={14} className="text-indigo-500" /> Governance: focus on owner assignment for top provider rows.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
