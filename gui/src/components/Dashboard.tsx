import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  DollarSign,
  Trash2,
  AlertTriangle,
  Play,
  TestTube,
  Download,
  TrendingUp,
  CheckCircle,
  Activity,
  ShieldCheck,
  Bell,
  Network,
  Server,
  Database,
  History,
  ClipboardList,
  Bot,
} from "lucide-react";
import { Modal } from "./Modal";
import { ScanWizard } from "./ScanWizard";
import { CustomSelect } from "./CustomSelect";
import { useCurrency } from "../hooks/useCurrency";
import { SavingsChart } from "./SavingsChart";
import { PageHeader } from "./layout/PageHeader";
import { formatTrendLabelByWindow, getTrendTickLimit } from "../utils/chartWindow";
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  ArcElement,
  BarElement,
  Title,
  Tooltip,
  Legend,
} from 'chart.js';
import { Bar, Doughnut, Line } from 'react-chartjs-2';

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  ArcElement,
  BarElement,
  Title,
  Tooltip,
  Legend
);

interface Stats {
  total_savings: number;
  wasted_resource_count: number;
  cleanup_count: number;
  history: [number, number][]; // [timestamp, cumulative_amount]
}

interface UpdateProgressDetail {
  stage?: string;
  progress?: number;
  downloaded_bytes?: number;
  total_bytes?: number | null;
  url?: string;
  message?: string;
}

interface ProxyProfile {
  id: string;
  name: string;
  protocol: string;
  host: string;
  port: number;
}

interface WastedResource {
  id?: string;
  provider?: string;
  region?: string;
  resource_type?: string;
  details?: string;
  estimated_monthly_cost: number;
  action_type?: string;
}

interface ResourceMetric {
  status: string;
  cpu_utilization?: number;
}

interface MonitorSnapshot {
  collected_at: number;
  total_resources: number;
  idle_resources: number;
  high_load_resources: number;
}

interface GovernanceErrorCategoryRow {
  label: string;
  count: number;
}

interface GovernanceDailyPoint {
  day_ts: number;
  day_label: string;
  findings: number;
  scan_runs: number;
  check_success_rate_pct: number;
}

interface GovernanceStatsResponse {
  scorecard: {
    scan_runs: number;
    findings: number;
    scan_check_success_rate_pct: number;
    active_accounts: number;
    scan_checks_failed: number;
  };
  daily?: GovernanceDailyPoint[];
  error_taxonomy: {
    categories: GovernanceErrorCategoryRow[];
  };
}

interface CloudProfileSummary {
  id: string;
}

interface AwsProfileSummary {
  name: string;
}

interface NotificationChannelSummary {
  is_active: boolean;
}

interface MonitorSummary {
  active: number;
  total: number;
  idle: number;
  highLoad: number;
  trendDelta: number;
  lastCollectedAt: number | null;
}

interface GovernanceSummary {
  scanRuns: number;
  findings: number;
  successRatePct: number;
  activeAccounts: number;
  failedChecks: number;
  topErrorLabel: string;
  topErrorCount: number;
}

interface SettingsSummary {
  cloudAccounts: number;
  awsProfiles: number;
  notificationsActive: number;
  proxyProfiles: number;
  proxyMode: string;
  apiTlsEnabled: boolean;
  apiBindHost: string;
}

interface AiAnalystSummary {
  window_days: number;
  latest_scan_at: number | null;
  total_monthly_waste: number;
  total_findings: number;
  delta_monthly_waste?: number | null;
  accounts: Array<{ label: string; estimated_monthly_waste: number; delta_monthly_waste?: number | null }>;
  providers: Array<{ label: string; estimated_monthly_waste: number; delta_monthly_waste?: number | null }>;
  resource_types: Array<{ label: string; estimated_monthly_waste: number; delta_monthly_waste?: number | null }>;
}

interface DashboardProps {
  onNavigate: (tab: string, params?: any) => void;
}

function buildEnterpriseChartOptions(axisTextColor: string, gridColor: string) {
  return {
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
        displayColors: true,
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
        grid: { color: gridColor, drawBorder: false },
        ticks: { color: axisTextColor, maxRotation: 0, autoSkip: true },
        border: { display: false },
      },
      y: {
        beginAtZero: true,
        grid: { color: gridColor, drawBorder: false },
        ticks: { color: axisTextColor, precision: 0 },
        border: { display: false },
      },
    },
  };
}

export function Dashboard({ onNavigate }: DashboardProps) {
  const PROXY_CHOICE_GLOBAL = "__global__";
  const PROXY_CHOICE_DIRECT = "__direct__";
  const UPDATE_PROXY_SETTING_KEY = "cws_update_proxy_choice";
  const DEMO_MODE_KEY = "cws_is_demo_mode";
  const DEMO_BACKUP_SCAN_RESULTS_KEY = "demo_backup_scan_results_v1";

  const [stats, setStats] = useState<Stats>({ total_savings: 0, wasted_resource_count: 0, cleanup_count: 0, history: [] });
  const [dashboardWindowDays, setDashboardWindowDays] = useState<number>(30);
  const [monitorSnapshots, setMonitorSnapshots] = useState<MonitorSnapshot[]>([]);
  const [governanceDaily, setGovernanceDaily] = useState<GovernanceDailyPoint[]>([]);
  const [governanceErrorCategories, setGovernanceErrorCategories] = useState<GovernanceErrorCategoryRow[]>([]);
  const [monitorSummary, setMonitorSummary] = useState<MonitorSummary>({
    active: 0,
    total: 0,
    idle: 0,
    highLoad: 0,
    trendDelta: 0,
    lastCollectedAt: null,
  });
  const [governanceSummary, setGovernanceSummary] = useState<GovernanceSummary>({
    scanRuns: 0,
    findings: 0,
    successRatePct: 0,
    activeAccounts: 0,
    failedChecks: 0,
    topErrorLabel: "No failures",
    topErrorCount: 0,
  });
  const [settingsSummary, setSettingsSummary] = useState<SettingsSummary>({
    cloudAccounts: 0,
    awsProfiles: 0,
    notificationsActive: 0,
    proxyProfiles: 0,
    proxyMode: "none",
    apiTlsEnabled: true,
    apiBindHost: "0.0.0.0",
  });
  const [aiSummary, setAiSummary] = useState<AiAnalystSummary | null>(null);
  const [loadingInsights, setLoadingInsights] = useState(false);
  const [isDemoMode, setIsDemoMode] = useState(false);
  const [hasPreDemoBackup, setHasPreDemoBackup] = useState(false);
  const [updateAvailable, setUpdateAvailable] = useState<string | null>(null);
  const [updateUrl] = useState<string | null>(null);
  const [updateUrls] = useState<string[]>([]);
  const [proxyProfiles, setProxyProfiles] = useState<ProxyProfile[]>([]);
  const [updateProxyChoice, setUpdateProxyChoice] = useState(PROXY_CHOICE_DIRECT);
  const [isUpdating, setIsUpdating] = useState(false);
  const [showUpdateProgressModal, setShowUpdateProgressModal] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [downloadTotalBytes, setDownloadTotalBytes] = useState<number | null>(null);
  const [downloadStatus, setDownloadStatus] = useState("Preparing download...");
  const [updateStage, setUpdateStage] = useState("idle");
  
  const [showCelebration, setShowCelebration] = useState(false);
  const [actionFeedback, setActionFeedback] = useState<{
    type: "success" | "error";
    text: string;
  } | null>(null);
  const [confirmDialog, setConfirmDialog] = useState<{
    title: string;
    body: string;
    confirmText: string;
    confirmClassName: string;
    action: () => Promise<void>;
  } | null>(null);
  const [confirmingAction, setConfirmingAction] = useState(false);

  // Modal State
  const [isModalOpen, setModalOpen] = useState(false);
  const [modalConfig, setModalConfig] = useState({ title: "", body: "", confirmText: "Open Configuration", onConfirm: () => {} });
  
  // Scan Wizard State
  const [showWizard, setShowWizard] = useState(false);
  const [wizardDemoMode, setWizardDemoMode] = useState(false);

  const { format, currency, currentRate } = useCurrency();
  // Simple check for dark mode based on class
  const isDark = document.documentElement.classList.contains('dark');

  const normalizedProgress = Math.max(0, Math.min(100, downloadProgress));
  const progressLabel = normalizedProgress < 10
      ? normalizedProgress.toFixed(1)
      : normalizedProgress.toFixed(0);
  const isLaunchingInstaller =
      updateStage === "download_complete" ||
      updateStage === "installer_started" ||
      normalizedProgress >= 100;
  const canCancelUpdate = isUpdating && !isLaunchingInstaller;

  function formatBytes(bytes?: number | null) {
      if (typeof bytes !== "number" || !Number.isFinite(bytes) || bytes < 0) return "-";
      const mb = bytes / (1024 * 1024);
      if (mb < 1024) return `${mb.toFixed(mb >= 100 ? 0 : 1)} MB`;
      return `${(mb / 1024).toFixed(2)} GB`;
  }

  function formatPct(value: number) {
      if (!Number.isFinite(value)) return "0.0%";
      return `${value.toFixed(1)}%`;
  }

  function formatDeltaMoney(value?: number | null) {
      if (typeof value !== "number") return "No prior scan";
      const prefix = value > 0 ? "+" : "";
      return `${prefix}${format(value)}`;
  }

  function formatRelativeTime(ts: number | null) {
      if (!ts) return "No sample";
      const diff = Math.max(0, Math.floor(Date.now() / 1000) - ts);
      if (diff < 60) return `${diff}s ago`;
      if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
      return `${Math.floor(diff / 3600)}h ago`;
  }

  function normalizeUpdateProxyChoice(value?: string | null) {
      const normalized = (value || "").trim();
      if (!normalized) return PROXY_CHOICE_DIRECT;
      return normalized;
  }

  async function loadUpdateProxyProfiles() {
      try {
          const profiles = await invoke<ProxyProfile[]>("list_proxy_profiles");
          const normalizedProfiles = Array.isArray(profiles) ? profiles : [];
          setProxyProfiles(normalizedProfiles);

          const saved = normalizeUpdateProxyChoice(localStorage.getItem(UPDATE_PROXY_SETTING_KEY));
          const isKnownChoice =
              saved === PROXY_CHOICE_GLOBAL ||
              saved === PROXY_CHOICE_DIRECT ||
              normalizedProfiles.some((p) => p.id === saved);

          const effectiveChoice = isKnownChoice ? saved : PROXY_CHOICE_DIRECT;
          setUpdateProxyChoice(effectiveChoice);
          if (effectiveChoice !== saved) {
              localStorage.setItem(UPDATE_PROXY_SETTING_KEY, effectiveChoice);
          }
      } catch (e) {
          console.error("Failed to load proxy profiles for updater", e);
          setProxyProfiles([]);
          setUpdateProxyChoice(PROXY_CHOICE_DIRECT);
      }
  }

  function handleChangeUpdateProxyChoice(value: string) {
      const normalized = normalizeUpdateProxyChoice(value);
      setUpdateProxyChoice(normalized);
      localStorage.setItem(UPDATE_PROXY_SETTING_KEY, normalized);
  }

  async function refreshDemoBackupStatus() {
      try {
          const raw = await invoke<string>("get_setting", { key: DEMO_BACKUP_SCAN_RESULTS_KEY });
          setHasPreDemoBackup((raw || "").trim().length > 0);
      } catch {
          setHasPreDemoBackup(false);
      }
  }

  async function clearDemoBackup() {
      await invoke("save_setting", { key: DEMO_BACKUP_SCAN_RESULTS_KEY, value: "" });
      setHasPreDemoBackup(false);
  }

  async function backupCurrentStateForDemo() {
      const existing = await invoke<string>("get_setting", { key: DEMO_BACKUP_SCAN_RESULTS_KEY });
      if ((existing || "").trim().length > 0) {
          setHasPreDemoBackup(true);
          return;
      }
      const currentResources = await invoke<WastedResource[]>("get_scan_results");
      await invoke("save_setting", {
          key: DEMO_BACKUP_SCAN_RESULTS_KEY,
          value: JSON.stringify(currentResources || []),
      });
      setHasPreDemoBackup(true);
  }

  async function restoreStateFromDemoBackup() {
      const raw = await invoke<string>("get_setting", { key: DEMO_BACKUP_SCAN_RESULTS_KEY });
      if (!raw || !raw.trim()) {
          return false;
      }
      let parsed: WastedResource[] = [];
      try {
          parsed = JSON.parse(raw);
      } catch {
          throw new Error("Saved pre-demo snapshot is invalid.");
      }
      await invoke("replace_scan_results", { resources: parsed });
      await clearDemoBackup();
      return true;
  }

  async function loadDashboardInsights(demo = false, windowDays = dashboardWindowDays) {
      setLoadingInsights(true);
      try {
          const [
              metrics,
              snapshots,
              governance,
              ai,
              clouds,
              awsProfiles,
              channels,
              proxies,
              proxyModeRaw,
              apiTlsRaw,
              apiBindHostRaw,
          ] = await Promise.all([
              invoke<ResourceMetric[]>("get_resource_metrics", { demoMode: demo }),
              invoke<MonitorSnapshot[]>("get_monitor_snapshots", { demoMode: demo, windowDays }),
              invoke<GovernanceStatsResponse>("get_governance_stats", { windowDays, demoMode: demo }),
              invoke<AiAnalystSummary>("get_ai_analyst_summary", { windowDays }).catch(() => null as AiAnalystSummary | null),
              invoke<CloudProfileSummary[]>("list_cloud_profiles"),
              invoke<AwsProfileSummary[]>("list_aws_profiles"),
              invoke<NotificationChannelSummary[]>("list_notification_channels"),
              invoke<ProxyProfile[]>("list_proxy_profiles"),
              invoke<string>("get_setting", { key: "proxy_mode" }),
              invoke<string>("get_setting", { key: "api_tls_enabled" }),
              invoke<string>("get_setting", { key: "api_bind_host" }),
          ]);

          const metricRows = Array.isArray(metrics) ? metrics : [];
          const snapshotRows = Array.isArray(snapshots) ? snapshots : [];
          setMonitorSnapshots(snapshotRows);
          const latestSnapshot = snapshotRows.length > 0 ? snapshotRows[snapshotRows.length - 1] : null;
          const prevSnapshot = snapshotRows.length > 1 ? snapshotRows[snapshotRows.length - 2] : null;
          const active = metricRows.filter((metric) => {
              const status = (metric.status || "").toLowerCase();
              return (
                  status.includes("running")
                  || status.includes("active")
                  || status.includes("connected")
                  || status.includes("configured")
              );
          }).length;
          const idle = metricRows.filter((metric) => {
              return typeof metric.cpu_utilization === "number" && metric.cpu_utilization < 2;
          }).length;
          const highLoad = metricRows.filter((metric) => {
              return typeof metric.cpu_utilization === "number" && metric.cpu_utilization > 80;
          }).length;
          setMonitorSummary({
              active,
              total: latestSnapshot?.total_resources ?? metricRows.length,
              idle: latestSnapshot?.idle_resources ?? idle,
              highLoad: latestSnapshot?.high_load_resources ?? highLoad,
              trendDelta: latestSnapshot && prevSnapshot
                  ? latestSnapshot.total_resources - prevSnapshot.total_resources
                  : 0,
              lastCollectedAt: latestSnapshot?.collected_at ?? null,
          });

          const topErrorRows = (governance?.error_taxonomy?.categories || [])
              .filter((item) => item.count > 0)
              .sort((a, b) => b.count - a.count);
          setGovernanceDaily(Array.isArray(governance?.daily) ? governance.daily : []);
          setGovernanceErrorCategories(topErrorRows.slice(0, 6));
          const topError = topErrorRows[0];
          setGovernanceSummary({
              scanRuns: governance?.scorecard?.scan_runs || 0,
              findings: governance?.scorecard?.findings || 0,
              successRatePct: governance?.scorecard?.scan_check_success_rate_pct || 0,
              activeAccounts: governance?.scorecard?.active_accounts || 0,
              failedChecks: governance?.scorecard?.scan_checks_failed || 0,
              topErrorLabel: topError?.label || "No failures",
              topErrorCount: topError?.count || 0,
          });
          setAiSummary(ai || null);

          const normalizedProxyMode = (proxyModeRaw || "none").trim().toLowerCase();
          const apiTlsNormalized = (apiTlsRaw || "").trim().toLowerCase();
          const apiTlsEnabled = !apiTlsNormalized
              ? true
              : ["1", "true", "yes", "on"].includes(apiTlsNormalized);
          setSettingsSummary({
              cloudAccounts: Array.isArray(clouds) ? clouds.length : 0,
              awsProfiles: Array.isArray(awsProfiles) ? awsProfiles.length : 0,
              notificationsActive: (Array.isArray(channels) ? channels : []).filter((item) => item.is_active).length,
              proxyProfiles: Array.isArray(proxies) ? proxies.length : 0,
              proxyMode: normalizedProxyMode || "none",
              apiTlsEnabled,
              apiBindHost: (apiBindHostRaw || "").trim() || "0.0.0.0",
          });
      } catch (e) {
          console.error("Failed to load dashboard insights", e);
          setMonitorSnapshots([]);
          setGovernanceDaily([]);
          setGovernanceErrorCategories([]);
          setAiSummary(null);
      } finally {
          setLoadingInsights(false);
      }
  }

  const axisTextColor = isDark ? "#94a3b8" : "#64748b";
  const gridColor = isDark ? "rgba(51,65,85,0.38)" : "rgba(148,163,184,0.26)";

  const barData = useMemo(() => ({
      labels: ["Compute", "Storage", "Database", "Network"],
      datasets: [
          {
              label: "Estimated Savings Mix",
              data: [
                  stats.total_savings * 0.35,
                  stats.total_savings * 0.3,
                  stats.total_savings * 0.2,
                  stats.total_savings * 0.15,
              ],
              backgroundColor: [
                  isDark ? "rgba(99, 102, 241, 0.9)" : "rgba(79, 70, 229, 0.84)",
                  isDark ? "rgba(59, 130, 246, 0.86)" : "rgba(37, 99, 235, 0.78)",
                  isDark ? "rgba(16, 185, 129, 0.84)" : "rgba(5, 150, 105, 0.72)",
                  isDark ? "rgba(245, 158, 11, 0.84)" : "rgba(217, 119, 6, 0.72)",
              ],
              borderRadius: 6,
              borderWidth: 0,
              barPercentage: 0.72,
              categoryPercentage: 0.74,
          },
      ],
  }), [isDark, stats.total_savings]);

  const barOptions: any = useMemo(() => ({
      ...buildEnterpriseChartOptions(axisTextColor, gridColor),
      plugins: {
          ...buildEnterpriseChartOptions(axisTextColor, gridColor).plugins,
          legend: { display: false },
          tooltip: {
              backgroundColor: isDark ? "#1e293b" : "#ffffff",
              titleColor: isDark ? "#f8fafc" : "#0f172a",
              bodyColor: isDark ? "#cbd5e1" : "#334155",
              borderColor: isDark ? "#334155" : "#e2e8f0",
              borderWidth: 1,
              padding: 10,
              callbacks: {
                  label: (ctx: any) => format(Number(ctx.raw || 0)),
              },
          },
      },
      scales: {
          x: {
              ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales.x,
              grid: { display: false, drawBorder: false },
          },
          y: {
              ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales.y,
              ticks: { color: axisTextColor, callback: (value: number) => format(Number(value || 0)) },
          },
      },
  }), [axisTextColor, format, gridColor, isDark]);

  const monitorTrendData: any = useMemo(() => ({
          labels: monitorSnapshots.map((point) => formatTrendLabelByWindow(point.collected_at, dashboardWindowDays)),
      datasets: [
          {
              label: "Total",
              data: monitorSnapshots.map((point) => point.total_resources),
              borderColor: "#4f46e5",
              backgroundColor: "rgba(79,70,229,0.12)",
              borderWidth: 1.6,
              pointRadius: 1.2,
              pointHoverRadius: 2.4,
              tension: 0.28,
              fill: true,
          },
          {
              label: "Idle",
              data: monitorSnapshots.map((point) => point.idle_resources),
              borderColor: "#059669",
              backgroundColor: "rgba(5,150,105,0.12)",
              borderWidth: 1.4,
              pointRadius: 1,
              pointHoverRadius: 2.2,
              tension: 0.26,
              fill: false,
          },
          {
              label: "High Load",
              data: monitorSnapshots.map((point) => point.high_load_resources),
              borderColor: "#dc2626",
              backgroundColor: "rgba(220,38,38,0.12)",
              borderWidth: 1.3,
              pointRadius: 1,
              pointHoverRadius: 2.2,
              tension: 0.24,
              fill: false,
          },
      ],
  }), [monitorSnapshots, dashboardWindowDays]);

  const monitorTrendOptions: any = useMemo(() => ({
      ...buildEnterpriseChartOptions(axisTextColor, gridColor),
      scales: {
          x: {
              ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales.x,
              ticks: { ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales.x.ticks, maxTicksLimit: getTrendTickLimit(dashboardWindowDays) },
          },
          y: buildEnterpriseChartOptions(axisTextColor, gridColor).scales.y,
      },
  }), [axisTextColor, dashboardWindowDays, gridColor]);

  const governanceTrendData: any = useMemo(() => ({
      labels: governanceDaily.map((point) => point.day_label),
      datasets: [
          {
              label: "Findings",
              data: governanceDaily.map((point) => point.findings),
              borderColor: "#f97316",
              backgroundColor: "rgba(249,115,22,0.1)",
              borderWidth: 1.5,
              pointRadius: 1.1,
              pointHoverRadius: 2.2,
              tension: 0.25,
              fill: true,
              yAxisID: "y",
          },
          {
              label: "Success %",
              data: governanceDaily.map((point) => point.check_success_rate_pct),
              borderColor: "#0ea5e9",
              backgroundColor: "rgba(14,165,233,0.1)",
              borderWidth: 1.4,
              pointRadius: 0.9,
              pointHoverRadius: 2.1,
              tension: 0.24,
              fill: false,
              yAxisID: "y1",
          },
      ],
  }), [governanceDaily]);

  const governanceTrendOptions: any = useMemo(() => ({
      ...buildEnterpriseChartOptions(axisTextColor, gridColor),
      scales: {
          x: {
              ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales.x,
              ticks: { ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales.x.ticks, maxTicksLimit: 10 },
          },
          y: buildEnterpriseChartOptions(axisTextColor, gridColor).scales.y,
          y1: {
              position: "right",
              beginAtZero: true,
              min: 0,
              max: 100,
              ticks: { color: axisTextColor, callback: (value: number) => `${value}%` },
              grid: { drawOnChartArea: false },
              border: { display: false },
          },
      },
  }), [axisTextColor, gridColor]);

  const governanceErrorMixData = useMemo(() => ({
      labels: governanceErrorCategories.map((item) => item.label),
      datasets: [
          {
              data: governanceErrorCategories.map((item) => item.count),
              backgroundColor: [
                  "#4f46e5",
                  "#f97316",
                  "#06b6d4",
                  "#10b981",
                  "#ef4444",
                  "#6366f1",
              ],
              borderColor: isDark ? "#0f172a" : "#ffffff",
              borderWidth: 1.2,
          },
      ],
  }), [governanceErrorCategories, isDark]);

  const governanceErrorMixOptions: any = useMemo(() => ({
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
          legend: {
              position: "right" as const,
              align: "center" as const,
              labels: {
                  color: axisTextColor,
                  boxWidth: 10,
                  usePointStyle: true,
                  pointStyle: "circle" as const,
                  padding: 14,
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
      cutout: "72%",
  }), [axisTextColor]);

  useEffect(() => {
    const isDemo = localStorage.getItem(DEMO_MODE_KEY) === "true";
    setIsDemoMode(isDemo);
    setWizardDemoMode(isDemo);
    loadStats(isDemo);
    loadDashboardInsights(isDemo);
    refreshDemoBackupStatus();

    checkUpdate();
    loadUpdateProxyProfiles();
    
    const unlistenUpdate = listen("update-progress", (event) => {
        const payload = event.payload as number | UpdateProgressDetail;
        if (typeof payload === "number" && Number.isFinite(payload)) {
            setDownloadProgress(Math.max(0, Math.min(100, payload)));
            return;
        }
        if (payload && typeof payload === "object" && typeof payload.progress === "number") {
            setDownloadProgress(Math.max(0, Math.min(100, payload.progress)));
        }
    });

    const unlistenUpdateDetail = listen("update-progress-detail", (event) => {
        const payload = event.payload as UpdateProgressDetail;
        if (!payload || typeof payload !== "object") return;

        if (typeof payload.progress === "number" && Number.isFinite(payload.progress)) {
            setDownloadProgress(Math.max(0, Math.min(100, payload.progress)));
        }

        if (typeof payload.stage === "string" && payload.stage.trim().length > 0) {
            setUpdateStage(payload.stage.trim());
        }

        if (typeof payload.downloaded_bytes === "number" && Number.isFinite(payload.downloaded_bytes)) {
            setDownloadedBytes(Math.max(0, payload.downloaded_bytes));
        }

        if (typeof payload.total_bytes === "number" && Number.isFinite(payload.total_bytes) && payload.total_bytes > 0) {
            setDownloadTotalBytes(payload.total_bytes);
        } else if (payload.total_bytes === null || payload.total_bytes === 0) {
            setDownloadTotalBytes(null);
        }

        const message = (payload.message || "").trim();
        if (message) setDownloadStatus(message);
    });
    
    return () => {
        unlistenUpdate.then(f => f());
        unlistenUpdateDetail.then(f => f());
    };
  }, []);

  async function handleUpdate() {
      if (!updateUrl) return;
      setDownloadProgress(0);
      setDownloadedBytes(0);
      setDownloadTotalBytes(null);
      setDownloadStatus("Preparing download...");
      setUpdateStage("preparing");
      setShowUpdateProgressModal(true);
      setIsUpdating(true);
      try {
          const proxyChoiceForInvoke =
              updateProxyChoice === PROXY_CHOICE_GLOBAL ? null : updateProxyChoice;
          await new Promise(r => setTimeout(r, 100));
          await invoke("download_and_install_update", {
              url: updateUrl,
              candidateUrls: updateUrls,
              proxyChoice: proxyChoiceForInvoke,
          });
          setDownloadProgress(100);
          setUpdateStage("installer_started");
          setDownloadStatus("Installer launched. Continue in the installer window.");
          await new Promise(r => setTimeout(r, 900));
      } catch (e) {
          const errText = String(e || "");
          if (errText.toLowerCase().includes("canceled by user")) {
              setUpdateStage("canceled");
              setDownloadStatus("Download canceled.");
          } else {
              setUpdateStage("failed");
              const detail = errText.trim();
              setDownloadStatus(
                  detail ? `Update failed: ${detail}` : "Update failed."
              );
          }
      } finally {
          setIsUpdating(false);
      }
  }

  async function handleCancelUpdate() {
      if (!isUpdating) return;
      if (!canCancelUpdate) return;
      setUpdateStage("canceling");
      setDownloadStatus("Canceling download...");
      try {
          await invoke("cancel_update_download");
      } catch (e) {
          console.error("Cancel update failed", e);
      } finally {
          setIsUpdating(false);
      }
  }

  async function loadStats(demo = false) {
    try {
      const s = await invoke<Stats>("get_dashboard_stats", { demoMode: demo });
      if (s) setStats(s);
    } catch (e) {
      console.error(e);
    }
  }

  async function checkUpdate() {
      return;
  }

  function showGuidance(title: string, body: string, targetTab: string, confirmText = "Open Configuration") {
      setModalConfig({
          title,
          body,
          confirmText,
          onConfirm: () => {
              setModalOpen(false);
              onNavigate(targetTab);
          }
      });
      setModalOpen(true);
  }

  function showActionFeedback(text: string, type: "success" | "error" = "success", autoHideMs = 6000) {
      setActionFeedback({ type, text });
      if (autoHideMs > 0) {
          window.setTimeout(() => {
              setActionFeedback((prev) => (prev?.text === text ? null : prev));
          }, autoHideMs);
      }
  }

  function openConfirmDialog(config: {
      title: string;
      body: string;
      confirmText: string;
      confirmClassName: string;
      action: () => Promise<void>;
  }) {
      setConfirmDialog(config);
  }

  async function runConfirmedAction() {
      if (!confirmDialog) return;
      setConfirmingAction(true);
      try {
          await confirmDialog.action();
          setConfirmDialog(null);
      } finally {
          setConfirmingAction(false);
      }
  }

  async function handleResetData() {
      if (isDemoMode && hasPreDemoBackup) {
          openConfirmDialog({
              title: "Exit Demo Mode",
              body: "Restore your pre-demo scan state and exit demo mode?",
              confirmText: "Restore",
              confirmClassName: "bg-indigo-600 text-white hover:bg-indigo-700",
              action: async () => {
                  try {
                      const restored = await restoreStateFromDemoBackup();
                      localStorage.removeItem(DEMO_MODE_KEY);
                      setIsDemoMode(false);
                      setWizardDemoMode(false);
                      await Promise.all([loadStats(false), loadDashboardInsights(false)]);
                      if (!restored) {
                          showActionFeedback("No previous snapshot found. Demo mode has been cleared.", "success");
                          return;
                      }
                      showActionFeedback("Restored your pre-demo scan state.");
                  } catch (e) {
                      showActionFeedback("Failed to restore previous state: " + e, "error");
                  }
              },
          });
          return;
      }

      openConfirmDialog({
          title: "Reset Scan Data",
          body: "Clear all scan results? This cannot be undone.",
          confirmText: "Clear Data",
          confirmClassName: "bg-rose-600 text-white hover:bg-rose-700",
          action: async () => {
              try {
                  await invoke("clear_scan_results");
                  localStorage.removeItem(DEMO_MODE_KEY);
                  setIsDemoMode(false);
                  setWizardDemoMode(false);
                  await clearDemoBackup();
                  await Promise.all([loadStats(false), loadDashboardInsights(false)]);
                  showActionFeedback("All scan results were cleared.");
              } catch (e) {
                  showActionFeedback("Failed to clear data: " + e, "error");
              }
          },
      });
  }

  function ignoreUpdate() {
      if (updateAvailable) {
          localStorage.setItem("cws_ignored_version", updateAvailable);
          setUpdateAvailable(null);
      }
  }

  async function handleScan(demoMode = false) {
    if (!demoMode) {
        try {
            const clouds = await invoke<any[]>("list_cloud_profiles");
            const aws = await invoke<any[]>("list_aws_profiles");
            if (clouds.length === 0 && aws.length === 0) {
                showGuidance("No Cloud Accounts", "You haven't connected any cloud accounts yet. Please add an account to start scanning.", "settings", "Add Account");
                return;
            }
        } catch(e) {
            console.error("Failed to check accounts", e);
        }
    }

    if (demoMode) {
        try {
            if (!isDemoMode) {
                await backupCurrentStateForDemo();
            }
        } catch (e) {
            showActionFeedback("Failed to create demo snapshot: " + e, "error");
            return;
        }
        localStorage.setItem(DEMO_MODE_KEY, "true");
        setIsDemoMode(true);
        await Promise.all([loadStats(true), loadDashboardInsights(true)]);
    } else {
        localStorage.removeItem(DEMO_MODE_KEY);
        setIsDemoMode(false);
    }

    setWizardDemoMode(demoMode);
    setShowWizard(true);
  }

  const resetButtonLabel = isDemoMode && hasPreDemoBackup ? "Exit Demo (Restore)" : "Reset Data";
  const updateProxyOptions = [
      { value: PROXY_CHOICE_DIRECT, label: "Direct (No Proxy) (Recommended)" },
      { value: PROXY_CHOICE_GLOBAL, label: "Default Network Policy" },
      ...proxyProfiles.map((profile) => ({
          value: profile.id,
          label: `${profile.name} (${profile.protocol}://${profile.host}:${profile.port})`,
      })),
  ];

  return (
    <div className="p-8 space-y-8 bg-slate-50 dark:bg-slate-900 min-h-screen text-left transition-colors duration-300">
      <ScanWizard 
        isOpen={showWizard} 
        onClose={() => setShowWizard(false)}
        demoMode={wizardDemoMode}
        onScanComplete={async (results) => {
            setShowWizard(false);
            await Promise.all([loadStats(wizardDemoMode), loadDashboardInsights(wizardDemoMode)]);
            if (!wizardDemoMode) {
                try {
                    await clearDemoBackup();
                } catch (e) {
                    console.error("Failed to clear demo backup", e);
                }
                localStorage.removeItem(DEMO_MODE_KEY);
                setIsDemoMode(false);
            }

            if (results.length === 0) {
                setShowCelebration(true);
            } else {
                onNavigate("scan_results");
            }
        }}
      />

      <Modal 
        isOpen={isModalOpen} 
        onClose={() => setModalOpen(false)} 
        title={modalConfig.title}
        footer={
            <div className="flex gap-2">
                <button onClick={() => setModalOpen(false)} className="px-4 py-3 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 rounded-lg font-medium">Cancel</button>
                <button onClick={modalConfig.onConfirm} className="px-4 py-3 bg-indigo-600 text-white hover:bg-indigo-700 rounded-lg font-medium">{modalConfig.confirmText}</button>
            </div>
        }
      >
        <p className="dark:text-slate-300">{modalConfig.body}</p>
      </Modal>

      {/* Celebration Modal */}
      <Modal
        isOpen={showCelebration}
        onClose={() => setShowCelebration(false)}
        title=""
        footer={
            <div className="w-full flex justify-center pb-4">
                <button 
                    onClick={() => setShowCelebration(false)}
                    className="bg-indigo-600 hover:bg-indigo-700 text-white px-8 py-3.5 rounded-full font-bold shadow-lg shadow-indigo-500/30 transition-all transform hover:-translate-y-1"
                >
                    Awesome!
                </button>
            </div>
        }
      >
          <div className="flex flex-col items-center justify-center py-10 text-center space-y-6">
              <div className="w-24 h-24 bg-green-50 dark:bg-green-900/20 rounded-full flex items-center justify-center mb-2 animate-bounce">
                  <CheckCircle className="w-12 h-12 text-green-500" />
              </div>
              <div className="space-y-2">
                  <h2 className="text-3xl font-black text-slate-900 dark:text-white">Clean Bill of Health!</h2>
                  <p className="text-slate-500 dark:text-slate-400 max-w-xs mx-auto">
                      We scanned your selected accounts and found <strong>zero</strong> wasted resources based on your current scanning rules.
                  </p>
              </div>
              <div className="bg-slate-50 dark:bg-slate-800 p-4 rounded-xl border border-slate-100 dark:border-slate-700 text-lg text-slate-600 dark:text-slate-300">
                  <p>Tip: Adjust your <strong>Scanning Rules</strong> in Configuration if you want stricter detection.</p>
              </div>
          </div>
      </Modal>

      <PageHeader
        title="Dashboard"
        subtitle="Operating view across latest scan results, governance posture, and monitor signals."
        icon={<Activity className="h-6 w-6" />}
        actions={
          <div className="flex flex-col items-end gap-2">
            <div className="flex flex-wrap items-center justify-end gap-2">
              {[7, 30, 90].map((days) => (
                <button
                  key={days}
                  onClick={() => {
                    setDashboardWindowDays(days);
                    void loadDashboardInsights(isDemoMode, days);
                  }}
                  className={`rounded-lg border px-3 py-2 text-sm font-semibold transition-colors ${
                    dashboardWindowDays === days
                      ? "border-indigo-500 bg-indigo-600 text-white"
                      : "border-slate-300 bg-white text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
                  }`}
                >
                  {days} Days
                </button>
              ))}
              <button
                onClick={handleResetData}
                className="inline-flex items-center rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
              >
                {resetButtonLabel}
              </button>
              <button
                onClick={() => handleScan(true)}
                className="inline-flex items-center gap-2 rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-sm font-semibold text-emerald-700 transition-colors hover:bg-emerald-100 disabled:opacity-50 dark:border-emerald-700/70 dark:bg-emerald-900/30 dark:text-emerald-200 dark:hover:bg-emerald-800/40"
              >
                <TestTube className="h-4 w-4" />
                Try Demo
              </button>
              <button
                onClick={() => handleScan(false)}
                className="inline-flex min-w-[156px] items-center justify-center gap-2 rounded-lg border border-indigo-500 bg-indigo-600 px-3 py-2 text-sm font-semibold text-white transition-colors hover:bg-indigo-700 disabled:opacity-50"
              >
                <Play className="h-4 w-4" />
                Start Cloud Scan
              </button>
            </div>
            <p className="text-xs text-slate-500 dark:text-slate-400 max-w-[520px] text-right">
              Scan fairness: if no cloud data is collected due to connectivity or credential configuration issues, this attempt is not counted as an effective scan run.
            </p>
          </div>
        }
      />

      <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="grid gap-4 md:grid-cols-3">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">What This Page Does</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              Summarize the current operating picture before you drill into findings, inventory, governance, or runtime health.
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Recommended Flow</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              Start a scan, review scan results, package exports for owners, then validate execution quality in Governance and Health Metrics.
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Current Mode</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              {isDemoMode ? "Demo mode is active." : "Live local state is active."} Insight window: {dashboardWindowDays} days. Edition: Community.
            </p>
          </div>
        </div>
      </div>

      {isDemoMode && (
        <div className="inline-flex items-center gap-2 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 text-amber-700 dark:text-amber-300 px-3 py-2 rounded-lg text-sm font-medium">
          Demo mode active{hasPreDemoBackup ? " · Reset will restore your previous state." : "."}
        </div>
      )}
      {actionFeedback && (
        <div
          className={`max-w-2xl px-3 py-2 rounded-lg text-sm font-medium border ${
            actionFeedback.type === "error"
              ? "bg-rose-50 dark:bg-rose-900/20 border-rose-200 dark:border-rose-800 text-rose-700 dark:text-rose-300"
              : "bg-emerald-50 dark:bg-emerald-900/20 border-emerald-200 dark:border-emerald-800 text-emerald-700 dark:text-emerald-300"
          }`}
        >
          {actionFeedback.text}
        </div>
      )}

      {updateAvailable && (
          <div className="bg-indigo-600 text-white px-6 py-3 rounded-lg shadow-lg flex justify-between items-center animate-in slide-in-from-top-4 fade-in duration-500">
              <div className="flex items-center">
                  <Download className="w-5 h-5 mr-3 animate-bounce" />
                  <div>
                      <p className="font-bold">New Version Available ({updateAvailable})</p>
                      <p className="text-indigo-200 text-lg">Update now to get the latest features and security fixes.</p>
                  </div>
              </div>
              <div className="flex flex-col items-end gap-2">
                  <div className="w-72">
                      <CustomSelect
                          value={updateProxyChoice}
                          onChange={handleChangeUpdateProxyChoice}
                          options={updateProxyOptions}
                          disabled={isUpdating}
                      />
                  </div>
                  <p className="text-xs text-indigo-100">Update Proxy Route</p>
                  <div className="flex gap-3">
                      <button
                        onClick={ignoreUpdate}
                        className="text-white/80 hover:text-white px-4 py-3 font-medium text-lg transition-colors underline decoration-transparent hover:decoration-white"
                      >
                          Ignore
                      </button>
                      <button
                        onClick={handleUpdate}
                        disabled={isUpdating}
                        className="bg-white text-indigo-600 px-4 py-3 rounded-lg font-bold text-lg hover:bg-indigo-50 transition-colors disabled:opacity-50"
                      >
                          {isUpdating ? `Downloading ${progressLabel}%` : "Install Update"}
                      </button>
                  </div>
              </div>
          </div>
      )}

      {/* Update Progress Modal */}
      <Modal
        isOpen={showUpdateProgressModal}
        onClose={() => {
            if (isUpdating) {
                handleCancelUpdate();
                return;
            }
            setShowUpdateProgressModal(false);
        }}
        title={
            isUpdating
                ? "Downloading Update"
                : updateStage === "failed"
                    ? "Update Failed"
                    : updateStage === "canceled"
                        ? "Download Canceled"
                        : "Update Status"
        }
        footer={
            isUpdating ? (
                canCancelUpdate ? (
                    <button
                      onClick={handleCancelUpdate}
                      className="px-4 py-2 rounded-lg border border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors"
                    >
                      Stop Download
                    </button>
                ) : (
                    <span className="text-sm text-slate-500 dark:text-slate-400">Preparing installer...</span>
                )
            ) : (
                <div className="flex gap-2 ml-auto">
                    {(updateStage === "failed" || updateStage === "canceled") && (
                        <button
                          onClick={handleUpdate}
                          className="px-4 py-2 rounded-lg bg-indigo-600 text-white hover:bg-indigo-700 transition-colors"
                        >
                          Retry
                        </button>
                    )}
                    <button
                      onClick={() => setShowUpdateProgressModal(false)}
                      className="px-4 py-2 rounded-lg border border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors"
                    >
                      Close
                    </button>
                </div>
            )
        }
      >
          <div className="space-y-4">
              <div className="w-full bg-slate-200 rounded-full h-5">
                  <div className="bg-indigo-600 h-5 rounded-full" style={{ width: `${normalizedProgress}%` }}></div>
              </div>
              <p className="text-center text-lg text-slate-500">{downloadStatus}</p>
              <p className="text-center text-sm text-slate-400">
                  {downloadTotalBytes && downloadTotalBytes > 0
                      ? `${formatBytes(downloadedBytes)} / ${formatBytes(downloadTotalBytes)} (${progressLabel}%)`
                      : `${formatBytes(downloadedBytes)} downloaded`}
              </p>
          </div>
      </Modal>

      <Modal
        isOpen={!!confirmDialog}
        onClose={() => {
            if (!confirmingAction) {
                setConfirmDialog(null);
            }
        }}
        title={confirmDialog?.title || "Confirm Action"}
        footer={
            <div className="flex gap-2">
                <button
                  onClick={() => setConfirmDialog(null)}
                  disabled={confirmingAction}
                  className="px-4 py-2 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 rounded-lg font-medium disabled:opacity-50"
                >
                  Cancel
                </button>
                <button
                  onClick={runConfirmedAction}
                  disabled={confirmingAction}
                  className={`px-4 py-2 rounded-lg font-medium disabled:opacity-60 ${confirmDialog?.confirmClassName || "bg-indigo-600 text-white hover:bg-indigo-700"}`}
                >
                  {confirmingAction ? "Processing..." : (confirmDialog?.confirmText || "Confirm")}
                </button>
            </div>
        }
      >
          <p className="text-sm text-slate-600 dark:text-slate-300">
              {confirmDialog?.body}
          </p>
      </Modal>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div 
            onClick={() => onNavigate("current_findings")}
            className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm transition-all duration-200 hover:shadow-lg hover:-translate-y-1 cursor-pointer group"
        >
          <div className="flex items-center justify-between mb-2">
            <h3 className="text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider group-hover:text-indigo-600 transition-colors">Monthly Savings</h3>
            <div className="p-2 bg-green-50 dark:bg-green-900/20 rounded-lg text-green-600 dark:text-green-400"><DollarSign className="h-4 w-4" /></div>
          </div>
          <div className="min-w-0">
            <span className="block text-[clamp(1.5rem,2.8vw,2.05rem)] leading-tight font-bold text-slate-900 dark:text-white tracking-tight break-all">
              {format(stats?.total_savings || 0)}
            </span>
            <span className="mt-1 inline-flex items-center rounded-full bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300 px-2 py-0.5 text-xs font-bold uppercase tracking-wider">
              Potential
            </span>
          </div>
          {currency !== "USD" && (
              <div className="mt-1 text-[10px] text-slate-400">
                  <span>≈ {currentRate} {currency}/USD. </span>
              </div>
          )}
        </div>

        <div 
            onClick={() => onNavigate("current_findings")}
            className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm transition-all duration-200 hover:shadow-lg hover:-translate-y-1 cursor-pointer group"
        >
          <div className="flex items-center justify-between mb-2">
            <h3 className="text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider group-hover:text-red-600 transition-colors">Wasted Resources</h3>
            <div className="p-2 bg-red-50 dark:bg-red-900/20 rounded-lg text-red-600 dark:text-red-400"><AlertTriangle className="h-4 w-4" /></div>
          </div>
          <div className="text-3xl font-bold text-slate-900 dark:text-white tracking-tight">{stats?.wasted_resource_count || 0}</div>
          <p className="text-xs text-slate-400 mt-1">Found across all regions</p>
        </div>

        <div 
            onClick={() => onNavigate("audit_log", { filter: 'cleanup' })}
            className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm transition-all duration-200 hover:shadow-lg hover:-translate-y-1 cursor-pointer group"
        >
          <div className="flex items-center justify-between mb-2">
            <h3 className="text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider group-hover:text-indigo-600 transition-colors">Historical Cleanups</h3>
            <div className="p-2 bg-indigo-50 dark:bg-indigo-900/20 rounded-lg text-indigo-600 dark:text-indigo-400"><Trash2 className="h-4 w-4" /></div>
          </div>
          <div className="text-3xl font-bold text-slate-900 dark:text-white tracking-tight">{stats?.cleanup_count || 0}</div>
          <p className="text-xs text-slate-400 mt-1">Total actions taken</p>
        </div>
      </div>

      <div className="rounded-2xl border border-slate-200 bg-white p-6 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="flex flex-col gap-2 md:flex-row md:items-end md:justify-between">
          <div>
            <h2 className="text-xl font-semibold text-slate-900 dark:text-white">Operating Workflow</h2>
            <p className="text-sm text-slate-500 dark:text-slate-400">
              Use the product in this order when you need a clean handoff from discovery to review.
            </p>
          </div>
          <p className="text-xs uppercase tracking-[0.18em] text-slate-400 dark:text-slate-500">Recommended order</p>
        </div>
        <div className="mt-5 grid gap-4 xl:grid-cols-4">
          <button
            onClick={() => onNavigate("current_findings")}
            className="rounded-2xl border border-slate-200 bg-slate-50 p-5 text-left transition-all hover:-translate-y-0.5 hover:border-indigo-300 hover:bg-white dark:border-slate-700 dark:bg-slate-900/40 dark:hover:border-indigo-500/40 dark:hover:bg-slate-900"
          >
            <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-indigo-50 text-indigo-600 dark:bg-indigo-500/15 dark:text-indigo-300">
              <ClipboardList className="h-5 w-5" />
            </div>
            <p className="mt-4 text-xs font-semibold uppercase tracking-[0.18em] text-slate-400 dark:text-slate-500">Step 1</p>
            <h3 className="mt-2 text-lg font-semibold text-slate-900 dark:text-white">Review Scan Results</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500 dark:text-slate-400">
              Inspect the latest scan results, select action candidates, and prepare PDF or CSV handoff.
            </p>
          </button>

          <button
            onClick={() => onNavigate("resource_inventory")}
            className="rounded-2xl border border-slate-200 bg-slate-50 p-5 text-left transition-all hover:-translate-y-0.5 hover:border-indigo-300 hover:bg-white dark:border-slate-700 dark:bg-slate-900/40 dark:hover:border-indigo-500/40 dark:hover:bg-slate-900"
          >
            <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-indigo-50 text-indigo-600 dark:bg-indigo-500/15 dark:text-indigo-300">
              <Database className="h-5 w-5" />
            </div>
            <p className="mt-4 text-xs font-semibold uppercase tracking-[0.18em] text-slate-400 dark:text-slate-500">Step 2</p>
            <h3 className="mt-2 text-lg font-semibold text-slate-900 dark:text-white">Check Inventory</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500 dark:text-slate-400">
              Confirm provider-level volume, estimated spend, and relative waste concentration before escalation.
            </p>
          </button>

          <button
            onClick={() => onNavigate("history")}
            className="rounded-2xl border border-slate-200 bg-slate-50 p-5 text-left transition-all hover:-translate-y-0.5 hover:border-indigo-300 hover:bg-white dark:border-slate-700 dark:bg-slate-900/40 dark:hover:border-indigo-500/40 dark:hover:bg-slate-900"
          >
            <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-indigo-50 text-indigo-600 dark:bg-indigo-500/15 dark:text-indigo-300">
              <History className="h-5 w-5" />
            </div>
            <p className="mt-4 text-xs font-semibold uppercase tracking-[0.18em] text-slate-400 dark:text-slate-500">Step 3</p>
            <h3 className="mt-2 text-lg font-semibold text-slate-900 dark:text-white">Reopen Scan History</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500 dark:text-slate-400">
              Compare against prior runs, reopen packaged findings, and export a dated report for responsible owners.
            </p>
          </button>

          <button
            onClick={() => onNavigate("governance")}
            className="rounded-2xl border border-slate-200 bg-slate-50 p-5 text-left transition-all hover:-translate-y-0.5 hover:border-indigo-300 hover:bg-white dark:border-slate-700 dark:bg-slate-900/40 dark:hover:border-indigo-500/40 dark:hover:bg-slate-900"
          >
            <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-indigo-50 text-indigo-600 dark:bg-indigo-500/15 dark:text-indigo-300">
              <ShieldCheck className="h-5 w-5" />
            </div>
            <p className="mt-4 text-xs font-semibold uppercase tracking-[0.18em] text-slate-400 dark:text-slate-500">Step 4</p>
            <h3 className="mt-2 text-lg font-semibold text-slate-900 dark:text-white">Track Governance</h3>
            <p className="mt-2 text-sm leading-6 text-slate-500 dark:text-slate-400">
              Use governance trends to validate execution quality, recurring waste, and owner coverage over time.
            </p>
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
        <div className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm h-80 transition-colors">
          <h3 className="text-lg font-bold text-slate-900 dark:text-white mb-1">Cost Breakdown</h3>
          <p className="text-sm text-slate-500 dark:text-slate-400 mb-3">Estimated savings mix across workload categories.</p>
          <div className="h-60">
            <Bar options={barOptions} data={barData} />
          </div>
        </div>
        <div className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm h-80 flex flex-col transition-colors">
          <h3 className="text-lg font-bold text-slate-900 dark:text-white mb-1 flex items-center">
              <TrendingUp className="w-5 h-5 mr-2 text-emerald-500" /> Cumulative Savings
          </h3>
          <p className="text-sm text-slate-500 dark:text-slate-400 mb-3">Value captured by completed cleanup actions.</p>
          <div className="flex-1 min-h-0">
            {stats.history.length > 0 ? (
                <SavingsChart data={stats.history} isDark={isDark} />
            ) : (
                <div className="h-full flex flex-col items-center justify-center text-center opacity-50">
                    <TrendingUp className="w-12 h-12 text-slate-300 dark:text-slate-400 mb-2" />
                    <p className="text-lg text-slate-500 dark:text-slate-400">Perform cleanups to see your savings grow.</p>
                </div>
            )}
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-4 gap-6">
        <div className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm h-80">
          <h3 className="text-lg font-bold text-slate-900 dark:text-white mb-1">Health Metrics Trend ({dashboardWindowDays}d)</h3>
          <p className="text-sm text-slate-500 dark:text-slate-400 mb-3">Total, idle, and high-load resources over time.</p>
          <div className="h-60">
            {monitorSnapshots.length > 0 ? (
                <Line options={monitorTrendOptions} data={monitorTrendData} />
            ) : (
                <div className="h-full flex items-center justify-center text-sm text-slate-400">
                    No monitor snapshots yet.
                </div>
            )}
          </div>
        </div>

        <div className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm h-80">
          <h3 className="text-lg font-bold text-slate-900 dark:text-white mb-1">Governance Trend ({dashboardWindowDays}d)</h3>
          <p className="text-sm text-slate-500 dark:text-slate-400 mb-3">Findings volume and scan check success trend.</p>
          <div className="h-60">
            {governanceDaily.length > 0 ? (
                <Line options={governanceTrendOptions} data={governanceTrendData} />
            ) : (
                <div className="h-full flex items-center justify-center text-sm text-slate-400">
                    No governance trend data yet.
                </div>
            )}
          </div>
        </div>

        <div className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm h-80">
          <h3 className="text-lg font-bold text-slate-900 dark:text-white mb-1">Failure Mix</h3>
          <p className="text-sm text-slate-500 dark:text-slate-400 mb-3">Top failed check categories in current window.</p>
          <div className="h-60">
            {governanceErrorCategories.length > 0 ? (
                <Doughnut options={governanceErrorMixOptions} data={governanceErrorMixData} />
            ) : (
                <div className="h-full flex items-center justify-center text-sm text-slate-400">
                    No failed check categories.
                </div>
            )}
          </div>
        </div>

        <div
          onClick={() => onNavigate("ai_analyst")}
          className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm h-80 transition-all hover:shadow-lg cursor-pointer"
        >
          <h3 className="text-lg font-bold text-slate-900 dark:text-white mb-1 flex items-center gap-2">
            <Bot className="w-5 h-5 text-indigo-500" /> AI Analyst Snapshot
          </h3>
          <p className="text-sm text-slate-500 dark:text-slate-400 mb-3">Top local waste concentration and latest change signal for this window.</p>
          <div className="grid grid-cols-2 gap-3 text-sm">
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Top Provider</div>
              <div className="font-bold text-slate-900 dark:text-white">{aiSummary?.providers?.[0]?.label || "No data"}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Top Account</div>
              <div className="font-bold text-slate-900 dark:text-white">{aiSummary?.accounts?.[0]?.label || "No data"}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Top Resource Type</div>
              <div className="font-bold text-slate-900 dark:text-white">{aiSummary?.resource_types?.[0]?.label || "No data"}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Potential Waste</div>
              <div className="font-bold text-rose-600 dark:text-rose-300">{format(aiSummary?.total_monthly_waste || 0)}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Vs Prior Scan</div>
              <div className="font-bold text-indigo-600 dark:text-indigo-300">{formatDeltaMoney(aiSummary?.delta_monthly_waste)}</div>
            </div>
          </div>
          <p className="mt-3 text-xs text-slate-500 dark:text-slate-400">
            {aiSummary?.latest_scan_at
              ? `Latest local summary ${new Date(aiSummary.latest_scan_at * 1000).toLocaleString()}.`
              : "Run a scan first to populate the AI analyst summary."}
          </p>
          <div className="mt-3 space-y-2">
            <p className="text-xs font-semibold uppercase tracking-[0.16em] text-slate-400 dark:text-slate-500">Worsening Signals</p>
            <div className="flex flex-wrap gap-2">
              {(aiSummary?.providers?.filter((row) => typeof row.delta_monthly_waste === "number" && row.delta_monthly_waste > 0).slice(0, 3) || []).map((row) => (
                <span key={`provider-${row.label}`} className="inline-flex items-center rounded-full bg-rose-50 px-2.5 py-1 text-xs font-semibold text-rose-700 dark:bg-rose-900/20 dark:text-rose-300">
                  Provider: {row.label}
                </span>
              ))}
              {(aiSummary?.accounts?.filter((row) => typeof row.delta_monthly_waste === "number" && row.delta_monthly_waste > 0).slice(0, 3) || []).map((row) => (
                <span key={`account-${row.label}`} className="inline-flex items-center rounded-full bg-amber-50 px-2.5 py-1 text-xs font-semibold text-amber-700 dark:bg-amber-900/20 dark:text-amber-300">
                  Account: {row.label}
                </span>
              ))}
              {(aiSummary?.resource_types?.filter((row) => typeof row.delta_monthly_waste === "number" && row.delta_monthly_waste > 0).slice(0, 3) || []).map((row) => (
                <span key={`type-${row.label}`} className="inline-flex items-center rounded-full bg-indigo-50 px-2.5 py-1 text-xs font-semibold text-indigo-700 dark:bg-indigo-900/20 dark:text-indigo-300">
                  Type: {row.label}
                </span>
              ))}
            </div>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-3 gap-6">
        <div
          onClick={() => onNavigate("health_metrics")}
          className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm transition-all hover:shadow-lg cursor-pointer"
        >
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
              <Activity className="w-5 h-5 text-indigo-500" /> Health Snapshot
            </h3>
            <span className="text-xs text-slate-400">{loadingInsights ? "Refreshing..." : "Open Health Metrics"}</span>
          </div>
          <div className="grid grid-cols-2 gap-3 text-sm">
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Active</div>
              <div className="font-bold text-slate-900 dark:text-white">{monitorSummary.active}/{monitorSummary.total}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Idle CPU</div>
              <div className="font-bold text-emerald-600 dark:text-emerald-400">{monitorSummary.idle}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">High Load</div>
              <div className="font-bold text-rose-600 dark:text-rose-400">{monitorSummary.highLoad}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Trend</div>
              <div className={`font-bold ${monitorSummary.trendDelta >= 0 ? "text-indigo-600 dark:text-indigo-300" : "text-amber-600 dark:text-amber-300"}`}>
                {monitorSummary.trendDelta >= 0 ? "+" : ""}{monitorSummary.trendDelta}
              </div>
            </div>
          </div>
          <p className="mt-3 text-xs text-slate-500 dark:text-slate-400">
            Last sample: {formatRelativeTime(monitorSummary.lastCollectedAt)}
          </p>
        </div>

        <div
          onClick={() => onNavigate("governance")}
          className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm transition-all hover:shadow-lg cursor-pointer"
        >
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
              <ShieldCheck className="w-5 h-5 text-indigo-500" /> Governance Snapshot
            </h3>
            <span className="text-xs text-slate-400">Open Governance</span>
          </div>
          <div className="grid grid-cols-2 gap-3 text-sm">
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Execution Quality</div>
              <div className="font-bold text-slate-900 dark:text-white">{formatPct(governanceSummary.successRatePct)}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Findings</div>
              <div className="font-bold text-rose-600 dark:text-rose-400">{governanceSummary.findings}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Scan Runs</div>
              <div className="font-bold text-slate-900 dark:text-white">{governanceSummary.scanRuns}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Active Accounts</div>
              <div className="font-bold text-indigo-600 dark:text-indigo-300">{governanceSummary.activeAccounts}</div>
            </div>
          </div>
          <p className="mt-3 text-xs text-slate-500 dark:text-slate-400">
            Top failure: {governanceSummary.topErrorLabel} ({governanceSummary.topErrorCount}) · Failed checks {governanceSummary.failedChecks}
          </p>
        </div>

        <div
          onClick={() => onNavigate("configuration")}
          className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm transition-all hover:shadow-lg cursor-pointer"
        >
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
              <Server className="w-5 h-5 text-indigo-500" /> Configuration Snapshot
            </h3>
            <span className="text-xs text-slate-400">Open Configuration</span>
          </div>
          <div className="grid grid-cols-2 gap-3 text-sm">
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Connected Accounts</div>
              <div className="font-bold text-slate-900 dark:text-white">{settingsSummary.cloudAccounts + settingsSummary.awsProfiles}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Notifications</div>
              <div className="font-bold text-emerald-600 dark:text-emerald-400 flex items-center gap-1">
                <Bell className="w-3.5 h-3.5" /> {settingsSummary.notificationsActive} active
              </div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Proxy Profiles</div>
              <div className="font-bold text-slate-900 dark:text-white">{settingsSummary.proxyProfiles}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Network</div>
              <div className="font-bold text-indigo-600 dark:text-indigo-300 flex items-center gap-1">
                <Network className="w-3.5 h-3.5" /> {settingsSummary.proxyMode}
              </div>
            </div>
          </div>
          <p className="mt-3 text-xs text-slate-500 dark:text-slate-400">
            API TLS: {settingsSummary.apiTlsEnabled ? "enabled" : "disabled"} · Bind {settingsSummary.apiBindHost}
          </p>
        </div>

        <div
          onClick={() => onNavigate("support_center")}
          className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm transition-all hover:shadow-lg cursor-pointer"
        >
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
              <Bell className="w-5 h-5 text-indigo-500" /> Support Snapshot
            </h3>
            <span className="text-xs text-slate-400">Open Support Center</span>
          </div>
          <div className="grid grid-cols-2 gap-3 text-sm">
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Notifications</div>
              <div className="font-bold text-emerald-600 dark:text-emerald-400">{settingsSummary.notificationsActive} active</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Proxy Mode</div>
              <div className="font-bold text-indigo-600 dark:text-indigo-300">{settingsSummary.proxyMode}</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Edition</div>
              <div className="font-bold text-slate-900 dark:text-white">Community</div>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-slate-700/50 px-3 py-2">
              <div className="text-slate-500 dark:text-slate-400 text-xs uppercase">Logs</div>
              <div className="font-bold text-slate-900 dark:text-white">Audit + System</div>
            </div>
          </div>
          <p className="mt-3 text-xs text-slate-500 dark:text-slate-400">
            Centralize runtime diagnostics, operator actions, and product feedback.
          </p>
        </div>
      </div>
    </div>
  );
}
