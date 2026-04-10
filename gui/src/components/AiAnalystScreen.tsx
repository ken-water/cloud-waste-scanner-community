import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Bot, CalendarRange, Cpu, Download, ExternalLink, FileText, RefreshCw, Send, Sparkles, Wallet } from "lucide-react";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";
import { MetricCard } from "./ui/MetricCard";
import { exportBlobWithTauriFallback, exportTextWithTauriFallback, revealExportedFileInFolder } from "../utils/fileExport";
import { drawPdfBrandHeader, drawPdfFooterSiteLink } from "../utils/pdfBranding";
import { loadPdfRuntime } from "../utils/pdfRuntime";

interface AiBreakdownRow {
  key: string;
  label: string;
  estimated_monthly_waste: number;
  findings: number;
  share_pct: number;
  delta_monthly_waste?: number | null;
  delta_findings?: number | null;
}

interface AiAnalystSummary {
  window_days: number;
  basis: string;
  latest_scan_id: number | null;
  latest_scan_at: number | null;
  scan_count_in_window: number;
  total_monthly_waste: number;
  total_findings: number;
  previous_scan_id?: number | null;
  previous_scan_at?: number | null;
  previous_total_monthly_waste?: number | null;
  previous_total_findings?: number | null;
  delta_monthly_waste?: number | null;
  delta_findings?: number | null;
  scanned_accounts: string[];
  accounts: AiBreakdownRow[];
  providers: AiBreakdownRow[];
  resource_types: AiBreakdownRow[];
  notes: string[];
}

interface AiAnalystDrilldownRow {
  account_id?: string | null;
  account_name?: string | null;
  provider: string;
  region: string;
  resource_type: string;
  resource_id: string;
  details: string;
  action_type: string;
  estimated_monthly_waste: number;
}

interface AiAnalystDrilldownResponse {
  window_days: number;
  basis: string;
  latest_scan_id?: number | null;
  latest_scan_at?: number | null;
  dimension: string;
  selected_key: string;
  selected_label: string;
  total_monthly_waste: number;
  total_findings: number;
  rows: AiAnalystDrilldownRow[];
  notes: string[];
}

interface ChatMessage {
  role: "assistant" | "user";
  content: string;
  actions?: ChatAction[];
  recommendation?: string;
}

interface DrilldownSelection {
  dimension: "account" | "provider" | "resource_type";
  key: string;
  label: string;
}

interface ChatAction {
  type: "drilldown" | "navigate_current_findings" | "navigate_resource_inventory" | "export_drilldown_csv" | "export_drilldown_pdf";
  label: string;
  selection?: DrilldownSelection;
  params?: any;
}

interface AiAnalystScreenProps {
  onNavigate: (tab: string, params?: any) => void;
}

const WINDOW_OPTIONS = [7, 30, 90];

const SUGGESTED_PROMPTS = [
  "Which cloud provider had the highest potential waste in the last 7 days?",
  "Which providers got worse versus the prior scan?",
  "Which accounts got worse versus the prior scan?",
  "Which resource types got worse versus the prior scan?",
  "What resource types are driving the most waste right now?",
  "Summarize the latest scan in plain English.",
  "What can you answer today?",
];

const PROMPT_GROUPS = [
  {
    title: "Cost Concentration",
    prompts: [
      "Which cloud provider had the highest potential waste in the last 30 days?",
      "What resource types are driving the most waste right now?",
      "Which account has the highest attributed waste right now?",
    ],
  },
  {
    title: "Change Detection",
    prompts: [
      "Which providers got worse versus the prior scan?",
      "Which accounts got worse versus the prior scan?",
      "Which resource types got worse versus the prior scan?",
    ],
  },
  {
    title: "Operator Summary",
    prompts: [
      "Summarize the latest scan in plain English.",
      "What can you answer today?",
      "What changed in the last 7 days?",
    ],
  },
];

type PresetActionId = "top_waste" | "low_risk_actions" | "weekly_brief";

const PRESET_ACTIONS: Array<{ id: PresetActionId; title: string; description: string; cta: string }> = [
  {
    id: "top_waste",
    title: "Top Waste Snapshot",
    description: "List current highest waste providers with concentration and direct drill-down actions.",
    cta: "Run Top Waste",
  },
  {
    id: "low_risk_actions",
    title: "Low-Risk Actions",
    description: "Find likely safe cleanup candidates first, then open exact rows for execution review.",
    cta: "Run Low-Risk",
  },
  {
    id: "weekly_brief",
    title: "Weekly Brief",
    description: "Generate an operator-ready weekly summary with change signal and priority next step.",
    cta: "Build Weekly Brief",
  },
];

function formatMoney(value: number) {
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: value >= 1000 ? 0 : 2,
  }).format(value);
}

function formatDateTime(unixTs: number | null | undefined) {
  if (!unixTs) return "No scan stored";
  return new Date(unixTs * 1000).toLocaleString();
}

function formatTopList(rows: AiBreakdownRow[], limit = 3) {
  return rows
    .slice(0, limit)
    .map((row) => `${row.label} ${formatMoney(row.estimated_monthly_waste)}/month`)
    .join(", ");
}

function worsenedRows(rows: AiBreakdownRow[], limit = 3) {
  return rows
    .filter((row) => typeof row.delta_monthly_waste === "number" && row.delta_monthly_waste > 0)
    .sort((a, b) => (b.delta_monthly_waste || 0) - (a.delta_monthly_waste || 0))
    .slice(0, limit);
}

function improvedRows(rows: AiBreakdownRow[], limit = 3) {
  return rows
    .filter((row) => typeof row.delta_monthly_waste === "number" && row.delta_monthly_waste < 0)
    .sort((a, b) => (a.delta_monthly_waste || 0) - (b.delta_monthly_waste || 0))
    .slice(0, limit);
}

function csvEscape(value: unknown) {
  const text = String(value ?? "");
  const escaped = text.replace(/"/g, '""');
  return /[",\n]/.test(escaped) ? `"${escaped}"` : escaped;
}

function formatDeltaCurrency(value: number | null | undefined) {
  if (typeof value !== "number") return "No prior scan";
  const prefix = value > 0 ? "+" : "";
  return `${prefix}${formatMoney(value)}`;
}

function formatDeltaCount(value: number | null | undefined) {
  if (typeof value !== "number") return "No prior scan";
  const prefix = value > 0 ? "+" : "";
  return `${prefix}${value}`;
}

function buildTopWasteSnapshot(summary: AiAnalystSummary): { content: string; recommendation: string; actions: ChatAction[] } {
  const topProviders = summary.providers.slice(0, 3);
  const providerLines = topProviders.length
    ? topProviders
      .map((row, idx) => `${idx + 1}. ${row.label} — ${formatMoney(row.estimated_monthly_waste)}/month (${row.findings} findings, ${row.share_pct.toFixed(1)}%)`)
      .join("\n")
    : "No provider ranking available in this window.";
  const topProvider = topProviders[0];
  const actions: ChatAction[] = [];
  if (topProvider) {
    actions.push({
      type: "drilldown",
      label: `Inspect ${topProvider.label}`,
      selection: { dimension: "provider", key: topProvider.key, label: topProvider.label },
    });
    actions.push({
      type: "navigate_current_findings",
      label: "Open Scan Results",
      params: { provider: topProvider.label, showOnlyDeleteActions: true, aiContext: `AI Analyst: top waste ${topProvider.label}` },
    });
    actions.push({
      type: "export_drilldown_csv",
      label: "Export CSV",
      selection: { dimension: "provider", key: topProvider.key, label: topProvider.label },
    });
    actions.push({
      type: "export_drilldown_pdf",
      label: "Export PDF",
      selection: { dimension: "provider", key: topProvider.key, label: topProvider.label },
    });
  }

  return {
    content: [
      `Top Waste Snapshot (${summary.window_days}d)`,
      `Total: ${summary.total_findings} findings · ${formatMoney(summary.total_monthly_waste)}/month`,
      "",
      "Provider ranking:",
      providerLines,
    ].join("\n"),
    recommendation: topProvider
      ? `Start with ${topProvider.label}. It has the largest current concentration and is usually the fastest leverage point for this window.`
      : "Run a completed scan to generate provider concentration first.",
    actions,
  };
}

function isLikelyLowRiskRow(row: AiAnalystDrilldownRow): boolean {
  const action = String(row.action_type || "").toLowerCase();
  const details = String(row.details || "").toLowerCase();
  const resource = String(row.resource_id || "").toLowerCase();
  const safeAction = /(delete|remove|release|detach|cleanup|terminate)/.test(action);
  const riskyText = /(prod|production|critical|stateful|database|db|primary|controller|cluster master|control plane)/.test(
    `${details} ${resource}`,
  );
  return safeAction && !riskyText;
}

function buildAnswer(question: string, summary: AiAnalystSummary): string {
  if (!summary.total_findings || (!summary.providers.length && !summary.resource_types.length)) {
    return `I do not have enough local scan history for the selected ${summary.window_days}-day window yet. Run a scan first, then ask again.`;
  }

  const input = question.trim().toLowerCase();
  const topProvider = summary.providers[0];
  const topAccount = summary.accounts[0];
  const secondProvider = summary.providers[1];
  const topResourceType = summary.resource_types[0];
  const latestScanLabel = formatDateTime(summary.latest_scan_at);

  if (!input || /(help|what can you answer|capabilities|can you do)/.test(input)) {
    return `I can answer top accounts, providers, waste mix by resource type, and latest-scan summaries using local data only. Right now the strongest signal is ${topProvider?.label ?? "no provider"} at ${topProvider ? `${formatMoney(topProvider.estimated_monthly_waste)}/month` : "n/a"} from the latest scan in your ${summary.window_days}-day window.`;
  }

  if (/(latest scan|summary|summarize|overview|what happened)/.test(input)) {
    const compareText =
      typeof summary.delta_monthly_waste === "number" && typeof summary.delta_findings === "number"
        ? ` Versus the prior completed scan, waste moved ${formatDeltaCurrency(summary.delta_monthly_waste)} and findings moved ${formatDeltaCount(summary.delta_findings)}.`
        : "";
    return `The latest scan inside the last ${summary.window_days} days ran at ${latestScanLabel}. It found ${summary.total_findings} potential waste findings worth about ${formatMoney(summary.total_monthly_waste)}/month. Top provider: ${topProvider?.label ?? "n/a"}. Top resource type: ${topResourceType?.label ?? "n/a"}.${compareText}`;
  }

  if (/(worse|worsened|got worse|increase|increased|up|regression|regressed)/.test(input)) {
    const targetRows = /(account|workspace|subscription|tenant)/.test(input)
      ? worsenedRows(summary.accounts, 3)
      : /(resource type|service|category|kind)/.test(input)
        ? worsenedRows(summary.resource_types, 3)
        : worsenedRows(summary.providers, 3);
    const label = /(account|workspace|subscription|tenant)/.test(input)
      ? "account"
      : /(resource type|service|category|kind)/.test(input)
        ? "resource-type"
        : "provider";
    if (!targetRows.length) {
      return `Compared with the prior completed scan in this ${summary.window_days}-day window, I do not see any ${label} bucket with higher potential waste.`;
    }
    const text = targetRows
      .map((row) => `${row.label} ${formatDeltaCurrency(row.delta_monthly_waste)}`)
      .join(", ");
    return `Compared with the prior completed scan, the ${label} buckets that worsened most are ${text}.`;
  }

  if (/(better|improved|got better|decrease|decreased|down|reduced)/.test(input)) {
    const targetRows = /(account|workspace|subscription|tenant)/.test(input)
      ? improvedRows(summary.accounts, 3)
      : /(resource type|service|category|kind)/.test(input)
        ? improvedRows(summary.resource_types, 3)
        : improvedRows(summary.providers, 3);
    const label = /(account|workspace|subscription|tenant)/.test(input)
      ? "account"
      : /(resource type|service|category|kind)/.test(input)
        ? "resource-type"
        : "provider";
    if (!targetRows.length) {
      return `Compared with the prior completed scan in this ${summary.window_days}-day window, I do not see any ${label} bucket with lower potential waste yet.`;
    }
    const text = targetRows
      .map((row) => `${row.label} ${formatDeltaCurrency(row.delta_monthly_waste)}`)
      .join(", ");
    return `Compared with the prior completed scan, the ${label} buckets that improved most are ${text}.`;
  }

  if (/(resource type|service|category|kind)/.test(input)) {
    return `The largest waste bucket is ${topResourceType?.label ?? "n/a"} at ${topResourceType ? `${formatMoney(topResourceType.estimated_monthly_waste)}/month` : "n/a"} across ${topResourceType?.findings ?? 0} findings. The next buckets are ${formatTopList(summary.resource_types.slice(1), 2) || "not available yet"}.`;
  }

  if (/(provider|cloud|vendor|platform)/.test(input) || /(highest|top|most)/.test(input)) {
    const nextText = secondProvider
      ? ` ${secondProvider.label} is next at ${formatMoney(secondProvider.estimated_monthly_waste)}/month.`
      : "";
    return `In the latest scan inside the last ${summary.window_days} days, ${topProvider?.label ?? "n/a"} had the highest potential waste at ${topProvider ? `${formatMoney(topProvider.estimated_monthly_waste)}/month` : "n/a"} across ${topProvider?.findings ?? 0} findings.${nextText} Ask for resource-type breakdown if you want the main drivers.`;
  }

  if (/(account|workspace|subscription|tenant)/.test(input)) {
    const nextText = summary.accounts[1]
      ? ` ${summary.accounts[1].label} is next at ${formatMoney(summary.accounts[1].estimated_monthly_waste)}/month.`
      : "";
    return `In the latest scan inside the last ${summary.window_days} days, ${topAccount?.label ?? "n/a"} had the highest attributed potential waste at ${topAccount ? `${formatMoney(topAccount.estimated_monthly_waste)}/month` : "n/a"} across ${topAccount?.findings ?? 0} findings.${nextText}`;
  }

  return `From the latest scan in the last ${summary.window_days} days, the headline is ${summary.total_findings} findings worth about ${formatMoney(summary.total_monthly_waste)}/month. Top providers: ${formatTopList(summary.providers)}. Top resource types: ${formatTopList(summary.resource_types)}.`;
}

function buildRecommendation(question: string, summary: AiAnalystSummary): string | undefined {
  const input = question.trim().toLowerCase();
  const topProvider = summary.providers[0];
  const topAccount = summary.accounts[0];
  const topResourceType = summary.resource_types[0];

  if (/(worse|worsened|got worse|increase|regression|regressed)/.test(input)) {
    if (/(account|workspace|subscription|tenant)/.test(input) && topAccount) {
      return `Next step: open Scan Results filtered to ${topAccount.label}, confirm whether the increase is concentrated in delete candidates, then export a review pack for that owner.`;
    }
    if (/(resource type|service|category|kind)/.test(input) && topResourceType) {
      return `Next step: inspect ${topResourceType.label}, verify which providers are driving it, then export a resource-type drill-down if remediation ownership is split.`;
    }
    if (topProvider) {
      return `Next step: inspect ${topProvider.label}, validate whether the increase is one account or multiple accounts, then hand off either through Scan Results or Resource Inventory.`;
    }
  }

  if (/(better|improved|got better|decrease|decreased|down|reduced)/.test(input)) {
    const improvedTarget = /(account|workspace|subscription|tenant)/.test(input)
      ? improvedRows(summary.accounts, 1)[0]
      : /(resource type|service|category|kind)/.test(input)
        ? improvedRows(summary.resource_types, 1)[0]
        : improvedRows(summary.providers, 1)[0];
    if (improvedTarget) {
      return `Next step: inspect ${improvedTarget.label} and confirm the reduction is durable rather than a one-scan fluctuation. If it is stable, use the same owner pattern elsewhere.`;
    }
  }

  if (/(account|workspace|subscription|tenant)/.test(input) && topAccount) {
    return `Next step: open ${topAccount.label} in Scan Results, select the highest-waste rows first, and export only the subset that the account owner needs.`;
  }

  if (/(resource type|service|category|kind)/.test(input) && topResourceType) {
    return `Next step: inspect ${topResourceType.label}, sort by provider and account, then decide whether this is a cleanup queue or a governance pattern.`;
  }

  if (topProvider) {
    return `Next step: start with ${topProvider.label}, open the drill-down, then decide whether to continue in Scan Results for exact rows or in Resource Inventory for broader owner allocation.`;
  }

  return undefined;
}

async function buildCrossWindowAnswer(question: string): Promise<{ content: string; recommendation?: string } | null> {
  const input = question.trim().toLowerCase();
  if (!/(7\s*d|7\s*day).*(30\s*d|30\s*day)|(30\s*d|30\s*day).*(7\s*d|7\s*day)|compare.*7.*30|compare.*30.*7/.test(input)) {
    return null;
  }

  const [summary7, summary30] = await Promise.all([
    invoke<AiAnalystSummary>("get_ai_analyst_summary", { windowDays: 7 }),
    invoke<AiAnalystSummary>("get_ai_analyst_summary", { windowDays: 30 }),
  ]);

  const provider7 = summary7.providers[0];
  const provider30 = summary30.providers[0];
  const type7 = summary7.resource_types[0];
  const type30 = summary30.resource_types[0];

  const content = [
    `7-day window: ${summary7.total_findings} findings worth about ${formatMoney(summary7.total_monthly_waste)}/month.`,
    `Top provider: ${provider7?.label ?? "n/a"}. Top resource type: ${type7?.label ?? "n/a"}.`,
    `30-day window: ${summary30.total_findings} findings worth about ${formatMoney(summary30.total_monthly_waste)}/month.`,
    `Top provider: ${provider30?.label ?? "n/a"}. Top resource type: ${type30?.label ?? "n/a"}.`,
  ].join(" ");

  return {
    content,
    recommendation: `Next step: use the 7-day view for immediate triage and the 30-day view for ownership patterns. If the same provider leads both windows, start there first.`,
  };
}

function buildActions(question: string, summary: AiAnalystSummary): ChatAction[] {
  const input = question.trim().toLowerCase();
  const topProvider = summary.providers[0];
  const topAccount = summary.accounts[0];
  const topResourceType = summary.resource_types[0];
  const actions: ChatAction[] = [];

  if (/(worse|worsened|got worse|increase|increased|up|regression|regressed)/.test(input)) {
    const isAccount = /(account|workspace|subscription|tenant)/.test(input);
    const isResourceType = /(resource type|service|category|kind)/.test(input);
    const worsenedTarget = isAccount
      ? worsenedRows(summary.accounts, 1)[0]
      : isResourceType
        ? worsenedRows(summary.resource_types, 1)[0]
        : worsenedRows(summary.providers, 1)[0];
    if (worsenedTarget) {
      actions.push({
        type: "drilldown",
        label: `Inspect ${worsenedTarget.label}`,
        selection: {
          dimension: isAccount ? "account" : isResourceType ? "resource_type" : "provider",
          key: worsenedTarget.key,
          label: worsenedTarget.label,
        },
      });
      actions.push({
        type: "navigate_current_findings",
        label: "Open Scan Results",
        params: isAccount
          ? { accountKey: worsenedTarget.key, showOnlyDeleteActions: true }
          : isResourceType
            ? { resourceType: worsenedTarget.label, showOnlyDeleteActions: true }
            : { provider: worsenedTarget.label, showOnlyDeleteActions: true },
      });
      actions.push({
        type: "export_drilldown_csv",
        label: "Export CSV",
        selection: {
          dimension: isAccount ? "account" : isResourceType ? "resource_type" : "provider",
          key: worsenedTarget.key,
          label: worsenedTarget.label,
        },
      });
      actions.push({
        type: "export_drilldown_pdf",
        label: "Export PDF",
        selection: {
          dimension: isAccount ? "account" : isResourceType ? "resource_type" : "provider",
          key: worsenedTarget.key,
          label: worsenedTarget.label,
        },
      });
    }
    return actions.slice(0, 4);
  }

  if (/(better|improved|got better|decrease|decreased|down|reduced)/.test(input)) {
    const isAccount = /(account|workspace|subscription|tenant)/.test(input);
    const isResourceType = /(resource type|service|category|kind)/.test(input);
    const improvedTarget = isAccount
      ? improvedRows(summary.accounts, 1)[0]
      : isResourceType
        ? improvedRows(summary.resource_types, 1)[0]
        : improvedRows(summary.providers, 1)[0];
    if (improvedTarget) {
      actions.push({
        type: "drilldown",
        label: `Inspect ${improvedTarget.label}`,
        selection: {
          dimension: isAccount ? "account" : isResourceType ? "resource_type" : "provider",
          key: improvedTarget.key,
          label: improvedTarget.label,
        },
      });
      actions.push({
        type: "navigate_current_findings",
        label: "Open Scan Results",
        params: isAccount
          ? { accountKey: improvedTarget.key, aiContext: `AI Analyst: improvement review for ${improvedTarget.label}` }
          : isResourceType
            ? { resourceType: improvedTarget.label, aiContext: `AI Analyst: improvement review for ${improvedTarget.label}` }
            : { provider: improvedTarget.label, aiContext: `AI Analyst: improvement review for ${improvedTarget.label}` },
      });
    }
    return actions.slice(0, 2);
  }

  if (/(account|workspace|subscription|tenant)/.test(input) && topAccount) {
    actions.push({
      type: "drilldown",
      label: `Inspect ${topAccount.label}`,
      selection: { dimension: "account", key: topAccount.key, label: topAccount.label },
    });
    actions.push({
      type: "navigate_current_findings",
      label: "Open Scan Results",
      params: { accountKey: topAccount.key },
    });
    actions.push({
      type: "export_drilldown_csv",
      label: "Export CSV",
      selection: { dimension: "account", key: topAccount.key, label: topAccount.label },
    });
    actions.push({
      type: "export_drilldown_pdf",
      label: "Export PDF",
      selection: { dimension: "account", key: topAccount.key, label: topAccount.label },
    });
    return actions.slice(0, 4);
  }

  if (/(resource type|service|category|kind)/.test(input) && topResourceType) {
    actions.push({
      type: "drilldown",
      label: `Inspect ${topResourceType.label}`,
      selection: { dimension: "resource_type", key: topResourceType.key, label: topResourceType.label },
    });
    actions.push({
      type: "navigate_current_findings",
      label: "Open Scan Results",
      params: { resourceType: topResourceType.label },
    });
    actions.push({
      type: "export_drilldown_csv",
      label: "Export CSV",
      selection: { dimension: "resource_type", key: topResourceType.key, label: topResourceType.label },
    });
    actions.push({
      type: "export_drilldown_pdf",
      label: "Export PDF",
      selection: { dimension: "resource_type", key: topResourceType.key, label: topResourceType.label },
    });
    return actions.slice(0, 4);
  }

  if (topProvider) {
    actions.push({
      type: "drilldown",
      label: `Inspect ${topProvider.label}`,
      selection: { dimension: "provider", key: topProvider.key, label: topProvider.label },
    });
    actions.push({
      type: "navigate_current_findings",
      label: "Open Scan Results",
      params: { provider: topProvider.label },
    });
    actions.push({
      type: "export_drilldown_csv",
      label: "Export CSV",
      selection: { dimension: "provider", key: topProvider.key, label: topProvider.label },
    });
    actions.push({
      type: "export_drilldown_pdf",
      label: "Export PDF",
      selection: { dimension: "provider", key: topProvider.key, label: topProvider.label },
    });
    actions.push({
      type: "navigate_resource_inventory",
      label: "Open Resource Inventory",
    });
  }

  if (topAccount) {
    actions.push({
      type: "drilldown",
      label: `Top Account: ${topAccount.label}`,
      selection: { dimension: "account", key: topAccount.key, label: topAccount.label },
    });
  }

  return actions.slice(0, 5);
}

export function AiAnalystScreen({ onNavigate }: AiAnalystScreenProps) {
  const [windowDays, setWindowDays] = useState(30);
  const [showOnlyWorsened, setShowOnlyWorsened] = useState(false);
  const [summary, setSummary] = useState<AiAnalystSummary | null>(null);
  const [loading, setLoading] = useState(false);
  const [question, setQuestion] = useState("");
  const [messages, setMessages] = useState<ChatMessage[]>([
    {
      role: "assistant",
      content:
        "Ask about provider concentration, top waste categories, or the latest scan summary. Answers are computed from local scan data first.",
    },
  ]);
  const [drilldown, setDrilldown] = useState<AiAnalystDrilldownResponse | null>(null);
  const [drilldownSelection, setDrilldownSelection] = useState<DrilldownSelection | null>(null);
  const [drilldownLoading, setDrilldownLoading] = useState(false);
  const [presetLoadingId, setPresetLoadingId] = useState<PresetActionId | null>(null);

  const exportDrilldownCsv = async () => {
    if (!drilldown?.rows?.length) return;
    const filename = `ai-analyst-${drilldown.dimension}-${drilldown.selected_key}-${new Date().toISOString().slice(0, 10)}.csv`;
    const lines = [
      ["Dimension", drilldown.dimension],
      ["Selection", drilldown.selected_label],
      ["Window Days", drilldown.window_days],
      ["Total Findings", drilldown.total_findings],
      ["Total Monthly Waste USD", drilldown.total_monthly_waste.toFixed(2)],
      [],
      ["Account", "Provider", "Region", "Resource Type", "Resource ID", "Action", "Waste USD / Month", "Details"],
      ...drilldown.rows.map((row) => [
        row.account_name || row.account_id || "",
        row.provider,
        row.region,
        row.resource_type,
        row.resource_id,
        row.action_type,
        row.estimated_monthly_waste.toFixed(2),
        row.details,
      ]),
    ]
      .map((row) => row.map(csvEscape).join(","))
      .join("\n");

    const savedPath = await exportTextWithTauriFallback(`\uFEFF${lines}`, filename, "text/csv;charset=utf-8;", { openAfterSave: false });
    if (savedPath) {
      await revealExportedFileInFolder(savedPath);
    }
  };

  const exportDrilldownPdf = async () => {
    if (!drilldown?.rows?.length) return;
    const { jsPDF, autoTable } = await loadPdfRuntime();
    const doc = new jsPDF({ orientation: "landscape", unit: "mm", format: "a4" });
    const pageWidth = doc.internal.pageSize.getWidth();
    const pageHeight = doc.internal.pageSize.getHeight();
    const headerBottomY = drawPdfBrandHeader(doc, {
      title: `AI Analyst Drill-Down: ${drilldown.selected_label}`,
      generatedAt: new Date(),
      extraLines: [
        `Dimension: ${drilldown.dimension}`,
        `Window: ${drilldown.window_days} days`,
      ],
    });

    doc.setFontSize(11);
    doc.setTextColor(60, 60, 60);
    doc.text(
      `Findings: ${drilldown.total_findings}    Potential waste: ${formatMoney(drilldown.total_monthly_waste)} / month`,
      14,
      headerBottomY + 8,
    );

    autoTable(doc, {
      startY: headerBottomY + 14,
      head: [["Account", "Provider", "Region", "Type", "Resource", "Action", "Waste / Mo", "Details"]],
      body: drilldown.rows.map((row) => [
        row.account_name || row.account_id || "-",
        row.provider,
        row.region,
        row.resource_type,
        row.resource_id,
        row.action_type,
        formatMoney(row.estimated_monthly_waste),
        row.details,
      ]),
      styles: {
        fontSize: 8,
        cellPadding: 2.4,
        overflow: "linebreak",
      },
      headStyles: {
        fillColor: [79, 70, 229],
        textColor: [255, 255, 255],
      },
      columnStyles: {
        7: { cellWidth: 78 },
      },
      margin: { left: 14, right: 14, bottom: 16 },
      didDrawPage: () => {
        const pageNumber = doc.getCurrentPageInfo().pageNumber;
        const totalPages = doc.getNumberOfPages();
        drawPdfFooterSiteLink(doc, pageWidth, pageHeight, pageNumber, totalPages);
      },
    });

    const blob = doc.output("blob");
    const filename = `ai-analyst-${drilldown.dimension}-${drilldown.selected_key}-${new Date().toISOString().slice(0, 10)}.pdf`;
    const savedPath = await exportBlobWithTauriFallback(blob, filename, { openAfterSave: false });
    if (savedPath) {
      await revealExportedFileInFolder(savedPath);
    }
  };

  const selectionToCurrentFindingsParams = (selection: DrilldownSelection) => (
    selection.dimension === "provider"
      ? { provider: selection.label, aiContext: `AI Analyst: ${selection.label}` }
      : selection.dimension === "account"
        ? { accountKey: selection.key, aiContext: `AI Analyst: ${selection.label}` }
        : { resourceType: selection.label, aiContext: `AI Analyst: ${selection.label}` }
  );

  const ensureDrilldownLoaded = async (selection: DrilldownSelection) => {
    const matchesCurrent =
      drilldown &&
      drilldownSelection &&
      drilldownSelection.dimension === selection.dimension &&
      drilldownSelection.key === selection.key;
    if (matchesCurrent) {
      return drilldown;
    }
    setDrilldownSelection(selection);
    setDrilldownLoading(true);
    try {
      const data = await invoke<AiAnalystDrilldownResponse>("get_ai_analyst_drilldown", {
        windowDays,
        dimension: selection.dimension,
        key: selection.key,
      });
      setDrilldown(data);
      return data;
    } finally {
      setDrilldownLoading(false);
    }
  };

  const loadDrilldown = async (selection: DrilldownSelection) => {
    await ensureDrilldownLoaded(selection);
  };

  const runChatAction = async (action: ChatAction) => {
    if (action.type === "drilldown" && action.selection) {
      await loadDrilldown(action.selection);
      return;
    }
    if (action.type === "navigate_current_findings") {
      onNavigate("current_findings", action.params);
      return;
    }
    if (action.type === "navigate_resource_inventory") {
      onNavigate("resource_inventory");
      return;
    }
    if (action.type === "export_drilldown_csv" && action.selection) {
      const data = await ensureDrilldownLoaded(action.selection);
      if (data?.rows?.length) {
        await exportDrilldownCsv();
      }
      return;
    }
    if (action.type === "export_drilldown_pdf" && action.selection) {
      const data = await ensureDrilldownLoaded(action.selection);
      if (data?.rows?.length) {
        await exportDrilldownPdf();
      }
    }
  };

  const refresh = async () => {
    setLoading(true);
    try {
      const data = await invoke<AiAnalystSummary>("get_ai_analyst_summary", { windowDays });
      setSummary(data);
      setDrilldown(null);
      setDrilldownSelection(null);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setMessages([
        {
          role: "assistant",
          content: `I could not load local AI summary data: ${message}`,
        },
      ]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
  }, [windowDays]);

  const submitQuestion = async (nextQuestion?: string) => {
    const raw = (nextQuestion ?? question).trim();
    if (!raw) return;
    const crossWindow = await buildCrossWindowAnswer(raw);
    if (crossWindow) {
      setMessages((current) => [
        ...current,
        { role: "user", content: raw },
        { role: "assistant", content: crossWindow.content, recommendation: crossWindow.recommendation },
      ]);
      setQuestion("");
      return;
    }
    const activeSummary = summary;
    const answer = activeSummary
      ? buildAnswer(raw, activeSummary)
      : "I do not have a local summary loaded yet. Refresh and try again.";
    const recommendation = activeSummary ? buildRecommendation(raw, activeSummary) : undefined;
    const actions = activeSummary ? buildActions(raw, activeSummary) : [];
    setMessages((current) => [
      ...current,
      { role: "user", content: raw },
      { role: "assistant", content: answer, actions, recommendation },
    ]);
    setQuestion("");
  };

  const runPresetAction = async (presetId: PresetActionId) => {
    const activeSummary = summary;
    if (!activeSummary) {
      setMessages((current) => [
        ...current,
        { role: "assistant", content: "No local analyst summary is loaded yet. Click Refresh first, then run this shortcut." },
      ]);
      return;
    }

    setPresetLoadingId(presetId);
    try {
      if (presetId === "top_waste") {
        const payload = buildTopWasteSnapshot(activeSummary);
        setMessages((current) => [
          ...current,
          { role: "assistant", content: payload.content, recommendation: payload.recommendation, actions: payload.actions },
        ]);
        return;
      }

      if (presetId === "weekly_brief") {
        const worsenedProvider = worsenedRows(activeSummary.providers, 1)[0];
        const worsenedAccount = worsenedRows(activeSummary.accounts, 1)[0];
        const worsenedType = worsenedRows(activeSummary.resource_types, 1)[0];
        const briefLines = [
          `Weekly Brief (${activeSummary.window_days}d)`,
          `Latest scan: ${formatDateTime(activeSummary.latest_scan_at)}`,
          `Total: ${activeSummary.total_findings} findings · ${formatMoney(activeSummary.total_monthly_waste)}/month`,
          `Change vs prior scan: waste ${formatDeltaCurrency(activeSummary.delta_monthly_waste)} · findings ${formatDeltaCount(activeSummary.delta_findings)}`,
          "",
          `Top provider: ${activeSummary.providers[0]?.label ?? "n/a"} (${activeSummary.providers[0] ? `${formatMoney(activeSummary.providers[0].estimated_monthly_waste)}/month` : "n/a"})`,
          `Top account: ${activeSummary.accounts[0]?.label ?? "n/a"} (${activeSummary.accounts[0]?.findings ?? 0} findings)`,
          `Top resource type: ${activeSummary.resource_types[0]?.label ?? "n/a"} (${activeSummary.resource_types[0] ? `${formatMoney(activeSummary.resource_types[0].estimated_monthly_waste)}/month` : "n/a"})`,
        ];
        if (worsenedProvider || worsenedAccount || worsenedType) {
          briefLines.push("", "Worsening signal:");
          if (worsenedProvider) briefLines.push(`- Provider: ${worsenedProvider.label} (${formatDeltaCurrency(worsenedProvider.delta_monthly_waste)})`);
          if (worsenedAccount) briefLines.push(`- Account: ${worsenedAccount.label} (${formatDeltaCurrency(worsenedAccount.delta_monthly_waste)})`);
          if (worsenedType) briefLines.push(`- Resource type: ${worsenedType.label} (${formatDeltaCurrency(worsenedType.delta_monthly_waste)})`);
        }
        const actions: ChatAction[] = [];
        if (worsenedProvider) {
          actions.push({
            type: "drilldown",
            label: `Inspect ${worsenedProvider.label}`,
            selection: { dimension: "provider", key: worsenedProvider.key, label: worsenedProvider.label },
          });
        } else if (activeSummary.providers[0]) {
          actions.push({
            type: "drilldown",
            label: `Inspect ${activeSummary.providers[0].label}`,
            selection: { dimension: "provider", key: activeSummary.providers[0].key, label: activeSummary.providers[0].label },
          });
        }
        actions.push({ type: "navigate_current_findings", label: "Open Scan Results", params: { showOnlyDeleteActions: true, aiContext: "AI Analyst: weekly brief" } });
        setMessages((current) => [
          ...current,
          {
            role: "assistant",
            content: briefLines.join("\n"),
            recommendation: "Use this brief as the weekly review opener, then assign cleanup ownership from the top worsening signal.",
            actions,
          },
        ]);
        return;
      }

      const providerCandidates = activeSummary.providers.slice(0, 3);
      const lowRiskRows: Array<AiAnalystDrilldownRow & { providerLabel: string; providerKey: string }> = [];

      for (const provider of providerCandidates) {
        try {
          const data = await invoke<AiAnalystDrilldownResponse>("get_ai_analyst_drilldown", {
            windowDays,
            dimension: "provider",
            key: provider.key,
          });
          for (const row of data.rows) {
            if (isLikelyLowRiskRow(row)) {
              lowRiskRows.push({
                ...row,
                providerLabel: provider.label,
                providerKey: provider.key,
              });
            }
          }
        } catch {
          // Keep going: one provider fetch failure should not block the full shortlist.
        }
      }

      lowRiskRows.sort((a, b) => b.estimated_monthly_waste - a.estimated_monthly_waste);
      const shortlist = lowRiskRows.slice(0, 6);
      if (!shortlist.length) {
        setMessages((current) => [
          ...current,
          {
            role: "assistant",
            content: `Low-Risk Actions (${activeSummary.window_days}d)\nNo clear low-risk candidates were found in top provider drill-downs. This usually means the remaining findings need owner confirmation before cleanup.`,
            recommendation: "Start from Top Waste Snapshot, then inspect by provider and mark ownership before action.",
            actions: buildTopWasteSnapshot(activeSummary).actions,
          },
        ]);
        return;
      }

      const lines = shortlist.map(
        (row, idx) =>
          `${idx + 1}. ${row.providerLabel} · ${row.resource_id} · ${row.action_type} · ${formatMoney(row.estimated_monthly_waste)}/month`,
      );
      const first = shortlist[0];
      setMessages((current) => [
        ...current,
        {
          role: "assistant",
          content: [`Low-Risk Actions (${activeSummary.window_days}d)`, ...lines].join("\n"),
          recommendation:
            "Validate ownership and business dependency first. If no dependency is found, batch these actions into the current cleanup window.",
          actions: [
            {
              type: "drilldown",
              label: `Inspect ${first.providerLabel}`,
              selection: { dimension: "provider", key: first.providerKey, label: first.providerLabel },
            },
            {
              type: "navigate_current_findings",
              label: "Open Scan Results",
              params: { provider: first.providerLabel, showOnlyDeleteActions: true, aiContext: `AI Analyst: low-risk ${first.providerLabel}` },
            },
            {
              type: "navigate_resource_inventory",
              label: "Open Resource Inventory",
            },
          ],
        },
      ]);
    } finally {
      setPresetLoadingId(null);
    }
  };

  const topProvider = summary?.providers?.[0];
  const topAccount = summary?.accounts?.[0];
  const topResourceType = summary?.resource_types?.[0];
  const worsenedProviders = useMemo(() => worsenedRows(summary?.providers ?? [], 3), [summary]);
  const worsenedAccounts = useMemo(() => worsenedRows(summary?.accounts ?? [], 3), [summary]);
  const worsenedResourceTypes = useMemo(() => worsenedRows(summary?.resource_types ?? [], 3), [summary]);
  const visibleAccounts = useMemo(
    () => (showOnlyWorsened ? worsenedRows(summary?.accounts ?? [], 6) : (summary?.accounts ?? []).slice(0, 6)),
    [showOnlyWorsened, summary],
  );
  const visibleProviders = useMemo(
    () => (showOnlyWorsened ? worsenedRows(summary?.providers ?? [], 6) : (summary?.providers ?? []).slice(0, 6)),
    [showOnlyWorsened, summary],
  );
  const visibleResourceTypes = useMemo(
    () => (showOnlyWorsened ? worsenedRows(summary?.resource_types ?? [], 6) : (summary?.resource_types ?? []).slice(0, 6)),
    [showOnlyWorsened, summary],
  );
  const rankingScopeDescription = showOnlyWorsened
    ? "Showing only buckets whose estimated monthly waste increased versus the prior completed scan."
    : "Showing the full current ranking from the latest completed scan in this window.";
  const previousScanLabel = summary?.previous_scan_at ? formatDateTime(summary.previous_scan_at) : "No prior completed scan";
  const includedAccountsLabel = useMemo(() => {
    if (!summary?.scanned_accounts?.length) return "No account labels in the current window";
    const preview = summary.scanned_accounts.slice(0, 3).join(", ");
    const extra = summary.scanned_accounts.length > 3 ? ` +${summary.scanned_accounts.length - 3} more` : "";
    return `${preview}${extra}`;
  }, [summary]);
  const executiveHeadline = useMemo(() => {
    if (!summary || !topProvider) {
      return "Run a scan to generate a local analyst summary.";
    }
    return `${topProvider.label} is the largest current waste concentration at ${formatMoney(topProvider.estimated_monthly_waste)}/month.`;
  }, [summary, topProvider]);
  const executiveChangeLine = useMemo(() => {
    if (!summary) return "No prior scan comparison available yet.";
    if (typeof summary.delta_monthly_waste !== "number") {
      return "No prior completed scan exists for this comparison window yet.";
    }
    if (summary.delta_monthly_waste > 0) {
      return `Potential waste increased by ${formatDeltaCurrency(summary.delta_monthly_waste)} versus the prior completed scan.`;
    }
    if (summary.delta_monthly_waste < 0) {
      return `Potential waste improved by ${formatDeltaCurrency(summary.delta_monthly_waste)} versus the prior completed scan.`;
    }
    return "Potential waste is flat versus the prior completed scan.";
  }, [summary]);
  const executiveNextStep = useMemo(() => {
    if (worsenedProviders[0]) {
      return `Start with ${worsenedProviders[0].label}. It is the clearest worsening signal in this window.`;
    }
    if (topAccount) {
      return `Start with ${topAccount.label}. It is currently the highest-account concentration in this window.`;
    }
    if (topResourceType) {
      return `Start with ${topResourceType.label}. It is the leading waste driver in this window.`;
    }
    return "Run another completed scan to unlock stronger comparisons and recommendations.";
  }, [topAccount, topResourceType, worsenedProviders]);

  return (
    <PageShell maxWidthClassName="max-w-none" className="space-y-8">
        <PageHeader
          title="AI Analyst"
          subtitle="Ask plain-language questions against local scan history. The app computes the numbers first, then turns them into operator-ready answers."
          icon={<Bot className="h-6 w-6" />}
          actions={
            <div className="flex items-center gap-3">
              <div className="inline-flex rounded-xl border border-slate-200 bg-white p-1 dark:border-slate-700 dark:bg-slate-800">
                {WINDOW_OPTIONS.map((days) => (
                  <button
                    key={days}
                    onClick={() => setWindowDays(days)}
                    className={`rounded-lg px-3 py-2 text-sm font-semibold transition ${
                      windowDays === days
                        ? "bg-indigo-600 text-white"
                        : "text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-700"
                    }`}
                  >
                    {days}d
                  </button>
                ))}
              </div>
              <button
                onClick={refresh}
                className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-4 py-2.5 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
                Refresh
              </button>
              <button
                onClick={() => onNavigate("ai_settings")}
                className="inline-flex items-center gap-2 rounded-xl bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-indigo-700"
              >
                <Bot className="h-4 w-4" />
                AI Settings
              </button>
            </div>
          }
        />

        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-5">
          <MetricCard
            label="Window"
            value={`${windowDays} Days`}
            hint={summary?.basis === "current_findings_fallback" ? "Using latest scan results because no historical scan was found in this window." : "Using the latest stored scan inside the selected window."}
            icon={<CalendarRange className="h-5 w-5" />}
          />
          <MetricCard
            label="Latest Scan"
            value={summary?.latest_scan_at ? new Date(summary.latest_scan_at * 1000).toLocaleDateString() : "No Data"}
            hint={summary?.latest_scan_at ? formatDateTime(summary.latest_scan_at) : "Run a scan to populate the analyst"}
            icon={<Sparkles className="h-5 w-5" />}
          />
          <MetricCard
            label="Top Account"
            value={topAccount?.label ?? "No Data"}
            hint={topAccount ? `${formatMoney(topAccount.estimated_monthly_waste)}/month across ${topAccount.findings} findings` : "No account ranking available yet"}
            icon={<Wallet className="h-5 w-5" />}
          />
          <MetricCard
            label="Top Provider"
            value={topProvider?.label ?? "No Data"}
            hint={topProvider ? `${formatMoney(topProvider.estimated_monthly_waste)}/month across ${topProvider.findings} findings` : "No provider ranking available"}
            icon={<Wallet className="h-5 w-5" />}
          />
          <MetricCard
            label="Top Resource Type"
            value={topResourceType?.label ?? "No Data"}
            hint={topResourceType ? `${formatMoney(topResourceType.estimated_monthly_waste)}/month across ${topResourceType.findings} findings` : "No resource-type ranking available"}
            icon={<Cpu className="h-5 w-5" />}
          />
        </div>

        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-2">
          <MetricCard
            label="Waste Vs Prior Scan"
            value={formatDeltaCurrency(summary?.delta_monthly_waste)}
            hint={`Previous scan: ${previousScanLabel}`}
          />
          <MetricCard
            label="Findings Vs Prior Scan"
            value={formatDeltaCount(summary?.delta_findings)}
            hint={
              typeof summary?.previous_total_findings === "number"
                ? `Previous findings: ${summary.previous_total_findings}`
                : "Run at least two completed scans to compare changes."
            }
          />
        </div>

        <div className="rounded-3xl border border-slate-200 bg-gradient-to-br from-slate-950 via-slate-900 to-indigo-950 p-6 text-white shadow-sm dark:border-slate-700">
          <div className="grid gap-5 xl:grid-cols-[minmax(0,1.5fr)_minmax(320px,1fr)]">
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.24em] text-indigo-200/80">Executive Summary</p>
              <h2 className="mt-3 text-2xl font-semibold tracking-tight">{executiveHeadline}</h2>
              <p className="mt-3 max-w-3xl text-sm leading-7 text-slate-200">{executiveChangeLine}</p>
              <p className="mt-3 max-w-3xl text-sm leading-7 text-slate-300">{executiveNextStep}</p>
              <div className="mt-5 flex flex-wrap gap-2">
                <button
                  onClick={() => submitQuestion("Summarize the latest scan in plain English.")}
                  className="rounded-full bg-white px-4 py-2 text-sm font-semibold text-slate-900 transition hover:bg-slate-100"
                >
                  Build Operator Summary
                </button>
                <button
                  onClick={() => submitQuestion("Which providers got worse versus the prior scan?")}
                  className="rounded-full border border-white/20 bg-white/10 px-4 py-2 text-sm font-semibold text-white transition hover:bg-white/15"
                >
                  Show Worsening Providers
                </button>
              </div>
            </div>
            <div className="grid gap-3 sm:grid-cols-3 xl:grid-cols-1">
              <div className="rounded-2xl border border-white/10 bg-white/5 px-4 py-4 backdrop-blur-sm">
                <p className="text-xs font-semibold uppercase tracking-[0.2em] text-slate-300">Top Provider</p>
                <p className="mt-2 text-lg font-semibold">{topProvider?.label ?? "No data"}</p>
                <p className="mt-1 text-sm text-slate-300">{topProvider ? `${formatMoney(topProvider.estimated_monthly_waste)}/month` : "Run a scan first"}</p>
              </div>
              <div className="rounded-2xl border border-white/10 bg-white/5 px-4 py-4 backdrop-blur-sm">
                <p className="text-xs font-semibold uppercase tracking-[0.2em] text-slate-300">Top Account</p>
                <p className="mt-2 text-lg font-semibold">{topAccount?.label ?? "No data"}</p>
                <p className="mt-1 text-sm text-slate-300">{topAccount ? `${topAccount.findings} findings` : "No account concentration yet"}</p>
              </div>
              <div className="rounded-2xl border border-white/10 bg-white/5 px-4 py-4 backdrop-blur-sm">
                <p className="text-xs font-semibold uppercase tracking-[0.2em] text-slate-300">Top Resource Type</p>
                <p className="mt-2 text-lg font-semibold">{topResourceType?.label ?? "No data"}</p>
                <p className="mt-1 text-sm text-slate-300">{topResourceType ? `${formatMoney(topResourceType.estimated_monthly_waste)}/month` : "No type ranking yet"}</p>
              </div>
            </div>
          </div>
        </div>

        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
          <div className="grid gap-4 md:grid-cols-3">
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Trust Model</p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                Answers are generated from local scan history. No cloud credentials or raw database export leave this machine.
              </p>
            </div>
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Good Questions</p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                Ask for top providers, top resource types, or a summary of the latest scan window. Current account coverage: {includedAccountsLabel}.
              </p>
            </div>
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Current Limits</p>
              <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                Change analysis compares the latest completed scan with the previous completed scan in the same window. It does not imply continuous time-series sampling.
              </p>
            </div>
          </div>
        </div>

        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
          <div className="mb-4 flex items-center justify-between">
            <div>
              <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Operator Shortcuts</p>
              <h2 className="mt-1 text-lg font-semibold text-slate-900 dark:text-white">Run One Click AI Workflows</h2>
            </div>
            <p className="text-xs text-slate-500 dark:text-slate-400">Built from local scan data</p>
          </div>
          <div className="grid gap-3 md:grid-cols-3">
            {PRESET_ACTIONS.map((preset) => (
              <div
                key={preset.id}
                className="rounded-2xl border border-slate-200 bg-slate-50 p-4 dark:border-slate-600 dark:bg-slate-900"
              >
                <p className="text-sm font-semibold text-slate-900 dark:text-white">{preset.title}</p>
                <p className="mt-2 text-xs leading-5 text-slate-600 dark:text-slate-300">{preset.description}</p>
                <button
                  onClick={() => void runPresetAction(preset.id)}
                  disabled={presetLoadingId !== null}
                  className="mt-4 inline-flex items-center gap-2 rounded-xl bg-indigo-600 px-3 py-2 text-xs font-semibold text-white transition hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {presetLoadingId === preset.id ? <RefreshCw className="h-3.5 w-3.5 animate-spin" /> : <Sparkles className="h-3.5 w-3.5" />}
                  {preset.cta}
                </button>
              </div>
            ))}
          </div>
        </div>

        <div className="flex items-center justify-between rounded-2xl border border-slate-200 bg-white px-5 py-3 shadow-sm dark:border-slate-700 dark:bg-slate-800">
          <div>
            <p className="text-sm font-semibold text-slate-900 dark:text-white">Ranking Scope</p>
            <p className="text-sm text-slate-500 dark:text-slate-400">{rankingScopeDescription}</p>
          </div>
          <button
            onClick={() => setShowOnlyWorsened((value) => !value)}
            className={`rounded-xl px-4 py-2 text-sm font-semibold transition ${
              showOnlyWorsened
                ? "bg-rose-600 text-white hover:bg-rose-700"
                : "border border-slate-200 bg-white text-slate-700 hover:bg-slate-50 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
            }`}
          >
            {showOnlyWorsened ? "Showing Worsened Only" : "Show Only Worsened"}
          </button>
        </div>

        <div className="grid gap-6 xl:grid-cols-[minmax(0,1.6fr)_minmax(320px,1fr)]">
          <div className="rounded-2xl border border-slate-200 bg-white shadow-sm dark:border-slate-700 dark:bg-slate-800">
            <div className="border-b border-slate-200 p-5 dark:border-slate-700">
              <div className="space-y-4">
                {PROMPT_GROUPS.map((group) => (
                  <div key={group.title}>
                    <p className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-400 dark:text-slate-500">{group.title}</p>
                    <div className="mt-2 flex flex-wrap gap-2">
                      {group.prompts.map((prompt) => (
                        <button
                          key={prompt}
                          onClick={() => submitQuestion(prompt)}
                          className="rounded-full border border-slate-200 bg-slate-50 px-3 py-1.5 text-sm text-slate-700 transition hover:border-indigo-300 hover:text-indigo-700 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-300 dark:hover:border-indigo-500/40 dark:hover:text-indigo-300"
                        >
                          {prompt}
                        </button>
                      ))}
                    </div>
                  </div>
                ))}
                <div className="border-t border-slate-200 pt-4 dark:border-slate-700">
                  <div className="flex flex-wrap gap-2">
                    {SUGGESTED_PROMPTS.map((prompt) => (
                      <button
                        key={prompt}
                        onClick={() => submitQuestion(prompt)}
                        className="rounded-full border border-slate-200 bg-white px-3 py-1.5 text-xs font-semibold text-slate-600 transition hover:border-indigo-300 hover:text-indigo-700 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-300 dark:hover:border-indigo-500/40 dark:hover:text-indigo-300"
                      >
                        {prompt}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            </div>

            <div className="space-y-4 p-5">
              <div className="max-h-[28rem] space-y-4 overflow-y-auto pr-1">
                {messages.map((message, index) => (
                  <div
                    key={`${message.role}-${index}`}
                    className={`rounded-2xl px-4 py-3 ${
                      message.role === "assistant"
                        ? "bg-slate-50 text-slate-800 dark:bg-slate-900 dark:text-slate-100"
                        : "ml-auto max-w-[85%] bg-indigo-600 text-white"
                    }`}
                  >
                    <p className="text-xs font-semibold uppercase tracking-[0.2em] opacity-70">
                      {message.role === "assistant" ? "AI Analyst" : "You"}
                    </p>
                    <p className="mt-2 whitespace-pre-wrap text-sm leading-6">{message.content}</p>
                    {message.role === "assistant" && message.recommendation ? (
                      <div className="mt-3 rounded-xl border border-indigo-100 bg-indigo-50 px-3 py-2 text-sm text-indigo-800 dark:border-indigo-500/20 dark:bg-indigo-500/10 dark:text-indigo-200">
                        <span className="font-semibold">Recommended next step:</span> {message.recommendation}
                      </div>
                    ) : null}
                    {message.role === "assistant" && message.actions?.length ? (
                      <div className="mt-3 flex flex-wrap gap-2">
                        {message.actions.map((action) => (
                          <button
                            key={`${index}-${action.label}`}
                            onClick={() => void runChatAction(action)}
                            className="rounded-full border border-slate-200 bg-white px-3 py-1.5 text-xs font-semibold text-slate-700 transition hover:border-indigo-300 hover:text-indigo-700 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-200 dark:hover:border-indigo-500/40 dark:hover:text-indigo-300"
                          >
                            {action.label}
                          </button>
                        ))}
                      </div>
                    ) : null}
                  </div>
                ))}
              </div>

              <div className="rounded-2xl border border-slate-200 bg-slate-50 p-3 dark:border-slate-700 dark:bg-slate-900">
                <textarea
                  value={question}
                  onChange={(event) => setQuestion(event.target.value)}
                  placeholder="Ask a local question, for example: Which cloud provider had the highest potential waste in the last 30 days?"
                  className="min-h-[110px] w-full resize-none bg-transparent px-2 py-2 text-sm leading-6 text-slate-900 outline-none placeholder:text-slate-400 dark:text-white dark:placeholder:text-slate-500"
                />
                <div className="mt-3 flex items-center justify-between gap-3">
                  <p className="text-xs text-slate-500 dark:text-slate-400">
                    Local summary first. Model wording can be added later without changing the underlying numbers.
                  </p>
                  <button
                    onClick={() => submitQuestion()}
                    className="inline-flex items-center gap-2 rounded-xl bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-indigo-700"
                  >
                    <Send className="h-4 w-4" />
                    Ask
                  </button>
                </div>
              </div>
            </div>
          </div>

          <div className="space-y-6">
            <div className="rounded-2xl border border-slate-200 bg-white shadow-sm dark:border-slate-700 dark:bg-slate-800">
              <div className="border-b border-slate-200 px-5 py-4 dark:border-slate-700">
                <h2 className="text-lg font-semibold text-slate-900 dark:text-white">Account Ranking</h2>
              </div>
              <div className="divide-y divide-slate-100 dark:divide-slate-700">
                {visibleAccounts.map((row, index) => (
                  <button
                    key={row.key}
                    onClick={() => loadDrilldown({ dimension: "account", key: row.key, label: row.label })}
                    className="flex w-full items-center justify-between gap-4 px-5 py-4 text-left transition hover:bg-slate-50 dark:hover:bg-slate-900"
                  >
                    <div className="min-w-0">
                      <p className="text-sm font-semibold text-slate-900 dark:text-white">
                        {index + 1}. {row.label}
                      </p>
                      <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                        {row.findings} findings · {row.share_pct.toFixed(1)}% of window total
                        {showOnlyWorsened && typeof row.delta_monthly_waste === "number" ? ` · ${formatDeltaCurrency(row.delta_monthly_waste)} vs prior scan` : ""}
                      </p>
                    </div>
                    <p className="text-sm font-semibold text-slate-900 dark:text-white">
                      {formatMoney(row.estimated_monthly_waste)}
                    </p>
                  </button>
                ))}
                {!visibleAccounts.length ? (
                  <div className="px-5 py-6 text-sm text-slate-500 dark:text-slate-400">
                    {showOnlyWorsened ? "No account bucket worsened versus the prior completed scan." : "No account ranking available yet."}
                  </div>
                ) : null}
              </div>
            </div>

            <div className="rounded-2xl border border-slate-200 bg-white shadow-sm dark:border-slate-700 dark:bg-slate-800">
              <div className="border-b border-slate-200 px-5 py-4 dark:border-slate-700">
                <h2 className="text-lg font-semibold text-slate-900 dark:text-white">Provider Ranking</h2>
              </div>
              <div className="divide-y divide-slate-100 dark:divide-slate-700">
                {visibleProviders.map((row, index) => (
                  <button
                    key={row.key}
                    onClick={() => loadDrilldown({ dimension: "provider", key: row.key, label: row.label })}
                    className="flex w-full items-center justify-between gap-4 px-5 py-4 text-left transition hover:bg-slate-50 dark:hover:bg-slate-900"
                  >
                    <div className="min-w-0">
                      <p className="text-sm font-semibold text-slate-900 dark:text-white">
                        {index + 1}. {row.label}
                      </p>
                      <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                        {row.findings} findings · {row.share_pct.toFixed(1)}% of window total
                        {showOnlyWorsened && typeof row.delta_monthly_waste === "number" ? ` · ${formatDeltaCurrency(row.delta_monthly_waste)} vs prior scan` : ""}
                      </p>
                    </div>
                    <p className="text-sm font-semibold text-slate-900 dark:text-white">
                      {formatMoney(row.estimated_monthly_waste)}
                    </p>
                  </button>
                ))}
                {!visibleProviders.length ? (
                  <div className="px-5 py-6 text-sm text-slate-500 dark:text-slate-400">
                    {showOnlyWorsened ? "No provider bucket worsened versus the prior completed scan." : "No provider ranking available yet."}
                  </div>
                ) : null}
              </div>
            </div>

            <div className="rounded-2xl border border-slate-200 bg-white shadow-sm dark:border-slate-700 dark:bg-slate-800">
              <div className="border-b border-slate-200 px-5 py-4 dark:border-slate-700">
                <h2 className="text-lg font-semibold text-slate-900 dark:text-white">Worsened Since Prior Scan</h2>
              </div>
              <div className="divide-y divide-slate-100 dark:divide-slate-700">
                {worsenedProviders.map((row, index) => (
                  <button
                    key={`worsened-${row.key}`}
                    onClick={() => loadDrilldown({ dimension: "provider", key: row.key, label: row.label })}
                    className="flex w-full items-center justify-between gap-4 px-5 py-4 text-left transition hover:bg-slate-50 dark:hover:bg-slate-900"
                  >
                    <div className="min-w-0">
                      <p className="text-sm font-semibold text-slate-900 dark:text-white">
                        {index + 1}. {row.label}
                      </p>
                      <p className="mt-1 text-xs text-rose-600 dark:text-rose-300">
                        {formatDeltaCurrency(row.delta_monthly_waste)} vs prior scan
                      </p>
                    </div>
                    <p className="text-sm font-semibold text-slate-900 dark:text-white">
                      {formatMoney(row.estimated_monthly_waste)}
                    </p>
                  </button>
                ))}
                {!worsenedProviders.length ? (
                  <div className="px-5 py-6 text-sm text-slate-500 dark:text-slate-400">
                    No provider bucket worsened versus the prior completed scan.
                  </div>
                ) : null}
              </div>
            </div>

            <div className="rounded-2xl border border-slate-200 bg-white shadow-sm dark:border-slate-700 dark:bg-slate-800">
              <div className="border-b border-slate-200 px-5 py-4 dark:border-slate-700">
                <h2 className="text-lg font-semibold text-slate-900 dark:text-white">Accounts Worsened Since Prior Scan</h2>
              </div>
              <div className="divide-y divide-slate-100 dark:divide-slate-700">
                {worsenedAccounts.map((row, index) => (
                  <button
                    key={`worsened-account-${row.key}`}
                    onClick={() => loadDrilldown({ dimension: "account", key: row.key, label: row.label })}
                    className="flex w-full items-center justify-between gap-4 px-5 py-4 text-left transition hover:bg-slate-50 dark:hover:bg-slate-900"
                  >
                    <div className="min-w-0">
                      <p className="text-sm font-semibold text-slate-900 dark:text-white">
                        {index + 1}. {row.label}
                      </p>
                      <p className="mt-1 text-xs text-rose-600 dark:text-rose-300">
                        {formatDeltaCurrency(row.delta_monthly_waste)} vs prior scan
                      </p>
                    </div>
                    <p className="text-sm font-semibold text-slate-900 dark:text-white">
                      {formatMoney(row.estimated_monthly_waste)}
                    </p>
                  </button>
                ))}
                {!worsenedAccounts.length ? (
                  <div className="px-5 py-6 text-sm text-slate-500 dark:text-slate-400">
                    No account bucket worsened versus the prior completed scan.
                  </div>
                ) : null}
              </div>
            </div>

            <div className="rounded-2xl border border-slate-200 bg-white shadow-sm dark:border-slate-700 dark:bg-slate-800">
              <div className="border-b border-slate-200 px-5 py-4 dark:border-slate-700">
                <h2 className="text-lg font-semibold text-slate-900 dark:text-white">Resource Types Worsened Since Prior Scan</h2>
              </div>
              <div className="divide-y divide-slate-100 dark:divide-slate-700">
                {worsenedResourceTypes.map((row, index) => (
                  <button
                    key={`worsened-resource-type-${row.key}`}
                    onClick={() => loadDrilldown({ dimension: "resource_type", key: row.key, label: row.label })}
                    className="flex w-full items-center justify-between gap-4 px-5 py-4 text-left transition hover:bg-slate-50 dark:hover:bg-slate-900"
                  >
                    <div className="min-w-0">
                      <p className="text-sm font-semibold text-slate-900 dark:text-white">
                        {index + 1}. {row.label}
                      </p>
                      <p className="mt-1 text-xs text-rose-600 dark:text-rose-300">
                        {formatDeltaCurrency(row.delta_monthly_waste)} vs prior scan
                      </p>
                    </div>
                    <p className="text-sm font-semibold text-slate-900 dark:text-white">
                      {formatMoney(row.estimated_monthly_waste)}
                    </p>
                  </button>
                ))}
                {!worsenedResourceTypes.length ? (
                  <div className="px-5 py-6 text-sm text-slate-500 dark:text-slate-400">
                    No resource-type bucket worsened versus the prior completed scan.
                  </div>
                ) : null}
              </div>
            </div>

            <div className="rounded-2xl border border-slate-200 bg-white shadow-sm dark:border-slate-700 dark:bg-slate-800">
              <div className="border-b border-slate-200 px-5 py-4 dark:border-slate-700">
                <h2 className="text-lg font-semibold text-slate-900 dark:text-white">Resource-Type Ranking</h2>
              </div>
              <div className="divide-y divide-slate-100 dark:divide-slate-700">
                {visibleResourceTypes.map((row, index) => (
                  <button
                    key={row.key}
                    onClick={() => loadDrilldown({ dimension: "resource_type", key: row.key, label: row.label })}
                    className="flex w-full items-center justify-between gap-4 px-5 py-4 text-left transition hover:bg-slate-50 dark:hover:bg-slate-900"
                  >
                    <div className="min-w-0">
                      <p className="text-sm font-semibold text-slate-900 dark:text-white">
                        {index + 1}. {row.label}
                      </p>
                      <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                        {row.findings} findings · {row.share_pct.toFixed(1)}% of window total
                        {showOnlyWorsened && typeof row.delta_monthly_waste === "number" ? ` · ${formatDeltaCurrency(row.delta_monthly_waste)} vs prior scan` : ""}
                      </p>
                    </div>
                    <p className="text-sm font-semibold text-slate-900 dark:text-white">
                      {formatMoney(row.estimated_monthly_waste)}
                    </p>
                  </button>
                ))}
                {!visibleResourceTypes.length ? (
                  <div className="px-5 py-6 text-sm text-slate-500 dark:text-slate-400">
                    {showOnlyWorsened ? "No resource-type bucket worsened versus the prior completed scan." : "No resource-type ranking available yet."}
                  </div>
                ) : null}
              </div>
            </div>

            <div className="rounded-2xl border border-slate-200 bg-white shadow-sm dark:border-slate-700 dark:bg-slate-800">
              <div className="border-b border-slate-200 px-5 py-4 dark:border-slate-700">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <h2 className="text-lg font-semibold text-slate-900 dark:text-white">Drill-Down Findings</h2>
                    <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
                      {drilldownSelection
                        ? `${drilldownSelection.dimension.replace("_", " ")}: ${drilldownSelection.label}`
                        : "Select an account, provider, or resource type to inspect exact findings."}
                    </p>
                  </div>
                  {drilldown ? (
                    <div className="text-right">
                      <p className="text-sm font-semibold text-slate-900 dark:text-white">{formatMoney(drilldown.total_monthly_waste)}</p>
                      <p className="text-xs text-slate-500 dark:text-slate-400">{drilldown.total_findings} findings</p>
                    </div>
                  ) : null}
                </div>
                {drilldownSelection ? (
                  <div className="mt-3 flex flex-wrap gap-2">
                    <button
                      onClick={() => void exportDrilldownCsv()}
                      className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm font-semibold text-slate-700 transition hover:bg-slate-50 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                    >
                      <Download className="h-4 w-4" />
                      Export CSV
                    </button>
                    <button
                      onClick={() => void exportDrilldownPdf()}
                      className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm font-semibold text-slate-700 transition hover:bg-slate-50 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                    >
                      <FileText className="h-4 w-4" />
                      Export PDF
                    </button>
                    <button
                      onClick={() => {
                        onNavigate("current_findings", selectionToCurrentFindingsParams(drilldownSelection));
                      }}
                      className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm font-semibold text-slate-700 transition hover:bg-slate-50 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                    >
                      <ExternalLink className="h-4 w-4" />
                      Open Scan Results
                    </button>
                    <button
                      onClick={() => onNavigate("resource_inventory")}
                      className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm font-semibold text-slate-700 transition hover:bg-slate-50 dark:border-slate-600 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                    >
                      <ExternalLink className="h-4 w-4" />
                      Open Resource Inventory
                    </button>
                  </div>
                ) : null}
              </div>

              {drilldownLoading ? (
                <div className="px-5 py-8 text-sm text-slate-500 dark:text-slate-400">Loading findings…</div>
              ) : drilldown?.rows?.length ? (
                <div className="overflow-x-auto">
                  <table className="min-w-full text-sm">
                    <thead className="bg-slate-50 dark:bg-slate-900/60">
                      <tr className="text-left text-slate-500 dark:text-slate-400">
                        <th className="px-5 py-3 font-semibold">Resource</th>
                        <th className="px-5 py-3 font-semibold">Provider</th>
                        <th className="px-5 py-3 font-semibold">Region</th>
                        <th className="px-5 py-3 font-semibold">Type</th>
                        <th className="px-5 py-3 font-semibold">Action</th>
                        <th className="px-5 py-3 text-right font-semibold">Waste / Mo</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100 dark:divide-slate-700">
                      {drilldown.rows.slice(0, 12).map((row) => (
                        <tr key={`${row.provider}-${row.resource_id}-${row.region}`} className="align-top">
                          <td className="px-5 py-4">
                            <p className="font-semibold text-slate-900 dark:text-white">{row.resource_id}</p>
                            <p className="mt-1 max-w-md text-xs leading-5 text-slate-500 dark:text-slate-400">{row.details || "No detail text stored."}</p>
                          </td>
                          <td className="px-5 py-4 text-slate-700 dark:text-slate-200">{row.provider}</td>
                          <td className="px-5 py-4 text-slate-700 dark:text-slate-200">{row.region}</td>
                          <td className="px-5 py-4 text-slate-700 dark:text-slate-200">{row.resource_type}</td>
                          <td className="px-5 py-4 text-slate-700 dark:text-slate-200">{row.action_type}</td>
                          <td className="px-5 py-4 text-right font-semibold text-slate-900 dark:text-white">
                            {formatMoney(row.estimated_monthly_waste)}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <div className="px-5 py-8 text-sm text-slate-500 dark:text-slate-400">
                  {drilldownSelection
                    ? "No matching findings are available for this drill-down in the selected window."
                    : "No drill-down selected yet."}
                </div>
              )}
            </div>
          </div>
        </div>
    </PageShell>
  );
}
