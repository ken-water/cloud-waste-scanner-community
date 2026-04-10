import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Activity, Server, Cpu, Search, Wifi, BarChart3, Clock3 } from "lucide-react";
import { CustomSelect } from "./CustomSelect";
import { CLOUD_PROVIDER_FILTER_OPTIONS, matchesProviderFilter } from "../constants/cloudProviders";
import { PageHeader } from "./layout/PageHeader";
import { formatTrendLabelByWindow, getTrendTickLimit } from "../utils/chartWindow";
import { MetricCard } from "./ui/MetricCard";
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
  ArcElement,
} from "chart.js";
import { Doughnut, Bar, Line } from "react-chartjs-2";

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title,
  Tooltip,
  Legend,
  ArcElement
);

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

interface MonitorSnapshot {
  collected_at: number;
  total_resources: number;
  idle_resources: number;
  high_load_resources: number;
}

const AUTO_REFRESH_SECONDS = 120;
const STALE_AFTER_SECONDS = 30 * 60;

function formatAge(seconds: number): string {
  if (seconds < 60) {
    return `${seconds}s ago`;
  }
  if (seconds < 3600) {
    return `${Math.floor(seconds / 60)}m ago`;
  }
  return `${Math.floor(seconds / 3600)}h ago`;
}

function sourceLabel(source?: string): string {
  if (source?.endsWith("_live")) {
    return "Live";
  }

  switch (source) {
    case "aws_cloudwatch":
      return "Live AWS";
    case "profile_config":
      return "Account";
    case "profile_probe":
      return "Probe";
    case "latest_scan":
      return "Scan";
    case "demo":
      return "Demo";
    default:
      return source || "Unknown";
  }
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
          boxWidth: 9,
          boxHeight: 9,
          usePointStyle: true,
          pointStyle: "circle" as const,
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

export function MonitorScreen() {
  const [metrics, setMetrics] = useState<ResourceMetric[]>([]);
  const [snapshots, setSnapshots] = useState<MonitorSnapshot[]>([]);
  const [loading, setLoading] = useState(false);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [filterProvider, setFilterProvider] = useState("All");
  const [search, setSearch] = useState("");
  const [nowTs, setNowTs] = useState(Math.floor(Date.now() / 1000));
  const [windowDays, setWindowDays] = useState<number>(30);

  useEffect(() => {
    void loadMonitorData(false, windowDays);

    const refreshTimer = window.setInterval(() => {
      void loadMonitorData(true, windowDays);
    }, AUTO_REFRESH_SECONDS * 1000);

    const clockTimer = window.setInterval(() => {
      setNowTs(Math.floor(Date.now() / 1000));
    }, 30 * 1000);

    return () => {
      window.clearInterval(refreshTimer);
      window.clearInterval(clockTimer);
    };
  }, [windowDays]);

  async function loadMonitorData(silent = true, selectedWindowDays = windowDays) {
    if (!silent) {
      setLoading(true);
    }

    try {
      const isDemo = localStorage.getItem("cws_is_demo_mode") === "true";
      const [data, trend] = await Promise.all([
        invoke<ResourceMetric[]>("get_resource_metrics", { demoMode: isDemo }),
        invoke<MonitorSnapshot[]>("get_monitor_snapshots", { demoMode: isDemo, windowDays: selectedWindowDays }),
      ]);

      setMetrics(data);
      setSnapshots(trend);
      setNowTs(Math.floor(Date.now() / 1000));
    } catch (e) {
      console.error(e);
    } finally {
      if (!silent) {
        setLoading(false);
      }
    }
  }

  async function refreshMetrics() {
    setLoading(true);
    setRefreshError(null);
    try {
      const isDemo = localStorage.getItem("cws_is_demo_mode") === "true";
      if (isDemo) {
        await loadMonitorData(true);
        return;
      }

      const newMetrics = await invoke<ResourceMetric[]>("collect_metrics", {
        awsProfile: null,
        awsRegion: null,
      });
      setMetrics(newMetrics);

      const trend = await invoke<MonitorSnapshot[]>("get_monitor_snapshots", {
        demoMode: false,
        windowDays,
      });
      setSnapshots(trend);
      setNowTs(Math.floor(Date.now() / 1000));
    } catch (e) {
      setRefreshError("Collection failed: " + e);
    } finally {
      setLoading(false);
    }
  }

  const filtered = useMemo(() => {
    return metrics.filter((metric) => {
      const matchProvider = matchesProviderFilter(filterProvider, metric.provider);
      const searchBlob = [metric.id, metric.name, metric.account_id, metric.resource_type]
        .filter(Boolean)
        .join(" ")
        .toLowerCase();
      const matchSearch = searchBlob.includes(search.toLowerCase());
      return matchProvider && matchSearch;
    });
  }, [metrics, filterProvider, search]);

  const latestUpdatedAt = useMemo(() => {
    return metrics.reduce((max, metric) => {
      if (!metric.updated_at) {
        return max;
      }
      return metric.updated_at > max ? metric.updated_at : max;
    }, 0);
  }, [metrics]);

  const staleSeconds = latestUpdatedAt > 0 ? Math.max(0, nowTs - latestUpdatedAt) : null;
  const stale = staleSeconds !== null && staleSeconds > STALE_AFTER_SECONDS;

  const stats = useMemo(() => {
    const cpuMetrics = filtered.filter((m) => m.cpu_utilization !== undefined && m.cpu_utilization !== null);
    const active = filtered.filter((m) => {
      const status = m.status.toLowerCase();
      return status.includes("running") || status.includes("active") || status.includes("connected") || status.includes("configured");
    }).length;
    const idle = cpuMetrics.filter((m) => (m.cpu_utilization || 0) < 2).length;
    const highLoad = cpuMetrics.filter((m) => (m.cpu_utilization || 0) > 80).length;
    const accountRows = filtered.filter((m) => m.resource_type === "Connected Account").length;

    return {
      active,
      idle,
      highLoad,
      total: filtered.length,
      cpuSeries: cpuMetrics.length,
      accountRows,
    };
  }, [filtered]);

  const loadDistData = useMemo(() => {
    const cpuMetrics = filtered.filter((m) => m.cpu_utilization !== undefined && m.cpu_utilization !== null);
    return {
      labels: ["Idle (<2%)", "Low (2-20%)", "Normal (20-80%)", "High (>80%)"],
      datasets: [
        {
          data: [
            cpuMetrics.filter((m) => (m.cpu_utilization || 0) < 2).length,
            cpuMetrics.filter((m) => (m.cpu_utilization || 0) >= 2 && (m.cpu_utilization || 0) < 20).length,
            cpuMetrics.filter((m) => (m.cpu_utilization || 0) >= 20 && (m.cpu_utilization || 0) <= 80).length,
            cpuMetrics.filter((m) => (m.cpu_utilization || 0) > 80).length,
          ],
          backgroundColor: ["#10b981", "#3b82f6", "#f59e0b", "#ef4444"],
          borderWidth: 0,
        },
      ],
    };
  }, [filtered]);

  const isDark = document.documentElement.classList.contains("dark");
  const axisTextColor = isDark ? "#94a3b8" : "#64748b";
  const gridColor = isDark ? "rgba(51,65,85,0.4)" : "rgba(148,163,184,0.2)";

  const trendData = useMemo(() => {
    return {
      labels: snapshots.map((point) => formatTrendLabelByWindow(point.collected_at, windowDays)),
      datasets: [
        {
          label: "Total",
          data: snapshots.map((point) => point.total_resources),
          borderColor: "#4f46e5",
          backgroundColor: "rgba(79,70,229,0.12)",
          borderWidth: 1.6,
          pointRadius: 1.1,
          pointHoverRadius: 2.2,
          tension: 0.26,
          fill: true,
        },
        {
          label: "Idle",
          data: snapshots.map((point) => point.idle_resources),
          borderColor: "#10b981",
          backgroundColor: "rgba(16,185,129,0.1)",
          borderWidth: 1.4,
          pointRadius: 1,
          pointHoverRadius: 2.1,
          tension: 0.22,
          fill: false,
        },
        {
          label: "High Load",
          data: snapshots.map((point) => point.high_load_resources),
          borderColor: "#ef4444",
          backgroundColor: "rgba(239,68,68,0.1)",
          borderWidth: 1.3,
          pointRadius: 1,
          pointHoverRadius: 2.1,
          tension: 0.2,
          fill: false,
        },
      ],
    };
  }, [snapshots, windowDays]);

  const providerFootprintData = useMemo(() => {
    const counts = new Map<string, number>();
    filtered.forEach((metric) => {
      const key = String(metric.provider || "Unknown");
      counts.set(key, (counts.get(key) || 0) + 1);
    });
    const sorted = Array.from(counts.entries()).sort((a, b) => b[1] - a[1]).slice(0, 8);
    return {
      labels: sorted.map((item) => item[0]),
      datasets: [
        {
          label: "Resources",
          data: sorted.map((item) => item[1]),
          backgroundColor: isDark ? "rgba(99,102,241,0.85)" : "rgba(79,70,229,0.78)",
          borderRadius: 4,
          borderWidth: 0,
          barPercentage: 0.72,
          categoryPercentage: 0.76,
        },
      ],
    };
  }, [filtered, isDark]);

  const donutOptions = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: {
        position: "right" as const,
        labels: {
          boxWidth: 10,
          usePointStyle: true,
          pointStyle: "circle" as const,
          color: axisTextColor,
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
    cutout: "78%",
  };

  const trendOptions = {
    ...buildEnterpriseChartOptions(axisTextColor, gridColor),
    scales: {
      ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales,
      x: {
        ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales.x,
        ticks: {
          ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales.x.ticks,
          maxTicksLimit: getTrendTickLimit(windowDays),
        },
      },
    },
  };

  const providerFootprintOptions = {
    ...buildEnterpriseChartOptions(axisTextColor, gridColor),
    plugins: {
      legend: { display: false },
    },
    scales: {
      x: {
        ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales.x,
        ticks: {
          ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales.x.ticks,
          maxRotation: 0,
          autoSkip: true,
        },
        grid: { display: false, drawBorder: false },
      },
      y: {
        ...buildEnterpriseChartOptions(axisTextColor, gridColor).scales.y,
      },
    },
  };

  return (
    <div className="p-8 space-y-6 pb-24 bg-slate-50 dark:bg-slate-900 dark:text-slate-100 transition-colors duration-300">
      <PageHeader
        title="Health Metrics"
        subtitle="Live AWS telemetry, provider inventory signals, and latest scan-derived utilization context."
        icon={<Activity className="h-6 w-6" />}
        actions={
          <div className="flex flex-col items-end gap-2">
            <div className="flex flex-wrap items-center justify-end gap-2">
              {[7, 30, 90].map((days) => (
                <button
                  key={days}
                  onClick={() => setWindowDays(days)}
                  className={`rounded-lg border px-3 py-2 text-sm font-semibold transition-colors ${
                    windowDays === days
                      ? "border-indigo-500 bg-indigo-600 text-white"
                      : "border-slate-300 bg-white text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
                  }`}
                >
                  {days} Days
                </button>
              ))}
              <button
                onClick={refreshMetrics}
                disabled={loading}
                className="w-full sm:w-auto px-4 py-2 bg-indigo-600 text-white rounded-lg font-medium hover:bg-indigo-700 disabled:opacity-50 transition-colors flex items-center justify-center shadow-lg shadow-indigo-500/20"
              >
                {loading ? <Activity className="w-4 h-4 mr-2 animate-spin" /> : <Server className="w-4 h-4 mr-2" />}
                {loading ? "Collecting..." : "Refresh Metrics"}
              </button>
            </div>
            <p className="text-xs text-slate-500 dark:text-slate-400 max-w-[520px] text-right">
              Refresh Metrics calls <span className="font-mono">collect_metrics</span> and may consume cloud metrics API quota or billing units depending on provider.
            </p>
          </div>
        }
      />

      <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
        <div className="grid gap-4 md:grid-cols-3">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Coverage Model</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              This view blends live AWS metrics, account health rows, and scan-derived resource signals into one operator surface.
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Collection Cost</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              Manual refresh can consume provider-side metrics API quota or billing units. Use it when freshness matters, not on every click.
            </p>
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Current Focus</p>
            <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
              {filterProvider === "All" ? "All providers are visible." : `${filterProvider} is selected.`} Search is {search.trim() ? "narrowing the dataset." : "showing the full visible dataset."}
            </p>
          </div>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2 text-xs">
        <span
          className={`inline-flex items-center px-2.5 py-1 rounded-full font-semibold ${
            stale
              ? "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300"
              : "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300"
          }`}
        >
          <Clock3 className="w-3.5 h-3.5 mr-1" /> {stale ? "Stale Data" : "Fresh Data"}
        </span>
        {latestUpdatedAt > 0 && staleSeconds !== null && (
          <span className="text-slate-500 dark:text-slate-400">
            Last updated {formatAge(staleSeconds)}
          </span>
        )}
        <span className="text-slate-500 dark:text-slate-400">
          Auto-refresh every {AUTO_REFRESH_SECONDS}s
        </span>
      </div>

      {refreshError && (
        <div className="rounded-lg border border-rose-200 dark:border-rose-800 bg-rose-50 dark:bg-rose-900/20 px-4 py-3 text-sm font-medium text-rose-700 dark:text-rose-300">
          {refreshError}
        </div>
      )}

      <div className="grid grid-cols-1 xl:grid-cols-4 gap-6">
        <div className="space-y-4">
          <MetricCard
            label="Active Signals"
            value={<>{stats.active} <span className="text-sm font-medium text-slate-400">/ {stats.total}</span></>}
            hint="Rows currently passing the active signal filter."
            icon={<Server className="w-5 h-5" />}
          />
          <MetricCard
            label="Idle CPU Resources"
            value={<span className="text-emerald-600 dark:text-emerald-400">{stats.idle}</span>}
            hint="Resources with CPU under 2% in the visible set."
            icon={<Activity className="w-5 h-5" />}
          />
          <MetricCard
            label="Connected Accounts"
            value={stats.accountRows}
            hint="Account health rows currently represented in monitor data."
            icon={<Wifi className="w-5 h-5" />}
          />
        </div>

        <div className="xl:col-span-3 grid grid-cols-1 xl:grid-cols-3 gap-6">
          <div className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
            <h3 className="text-lg font-bold text-slate-800 dark:text-slate-100 mb-1 flex items-center">
              <BarChart3 className="w-5 h-5 mr-2 text-indigo-500" /> CPU Load Distribution
            </h3>
            <p className="text-sm text-slate-500 dark:text-slate-400 mb-3">
              Based on {stats.cpuSeries} resources with available CPU metrics.
            </p>
            <div className="h-64 flex items-center justify-center">
              <Doughnut data={loadDistData} options={donutOptions} />
            </div>
          </div>

          <div className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
            <h3 className="text-lg font-bold text-slate-800 dark:text-slate-100 mb-1 flex items-center">
              <BarChart3 className="w-5 h-5 mr-2 text-indigo-500" /> Health Metrics Trend ({windowDays}d)
            </h3>
            <p className="text-sm text-slate-500 dark:text-slate-400 mb-3">
              Snapshot trend for total, idle, and high-load resources.
            </p>
            <div className="h-64">
              {snapshots.length > 0 ? (
                <Line data={trendData} options={trendOptions} />
              ) : (
                <div className="h-full flex items-center justify-center text-sm text-slate-400">No trend snapshots yet. Click Refresh Metrics.</div>
              )}
            </div>
          </div>

          <div className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
            <h3 className="text-lg font-bold text-slate-800 dark:text-slate-100 mb-1 flex items-center">
              <BarChart3 className="w-5 h-5 mr-2 text-indigo-500" /> Provider Footprint
            </h3>
            <p className="text-sm text-slate-500 dark:text-slate-400 mb-3">
              Resource volume by provider for the current filtered view.
            </p>
            <div className="h-64">
              {providerFootprintData.labels.length > 0 ? (
                <Bar data={providerFootprintData} options={providerFootprintOptions} />
              ) : (
                <div className="h-full flex items-center justify-center text-sm text-slate-400">No provider data available.</div>
              )}
            </div>
          </div>
        </div>
      </div>

      <div className="flex flex-col lg:flex-row gap-4 items-stretch lg:items-center bg-white dark:bg-slate-800 p-4 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
          <input
            className="w-full pl-10 pr-4 py-2 border border-slate-200 dark:border-slate-600 rounded-lg text-sm bg-slate-50 dark:bg-slate-900 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 transition-all"
            placeholder="Search ID, resource, account..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <div className="relative w-full lg:w-80">
          <CustomSelect
            value={filterProvider}
            onChange={setFilterProvider}
            searchable
            searchPlaceholder="Search provider..."
            options={CLOUD_PROVIDER_FILTER_OPTIONS}
          />
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 pb-4">
        {filtered.map((metric) => {
          const statusValue = metric.status.toLowerCase();
          const statusDotClass =
            statusValue.includes("running") || statusValue.includes("active") || statusValue.includes("connected")
              ? "bg-emerald-500 animate-pulse"
              : statusValue.includes("error") || statusValue.includes("missing")
              ? "bg-red-500"
              : "bg-slate-300";

          const metricAge = metric.updated_at > 0 ? Math.max(0, nowTs - metric.updated_at) : null;

          return (
            <div key={`${metric.provider}:${metric.id}:${metric.account_id || "none"}`} className="bg-white dark:bg-slate-800 p-5 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm transition-all hover:shadow-lg group">
              <div className="flex justify-between items-start mb-3 gap-2">
                <span className="text-[10px] font-bold uppercase tracking-wider px-2 py-1 rounded bg-slate-100 text-slate-600 dark:bg-slate-700 dark:text-slate-300">
                  {metric.provider} • {metric.region}
                </span>
                <div className={`w-2.5 h-2.5 rounded-full ${statusDotClass}`}></div>
              </div>

              <div className="flex items-center justify-between gap-2 mb-1">
                <h3 className="font-bold text-sm truncate text-slate-900 dark:text-white group-hover:text-indigo-600 transition-colors" title={metric.name || metric.id}>
                  {metric.name || metric.id}
                </h3>
                <span className="text-[10px] px-2 py-0.5 rounded-full bg-indigo-50 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-300 font-semibold">
                  {sourceLabel(metric.source)}
                </span>
              </div>

              <p className="text-xs font-mono text-slate-400 mb-2 truncate">{metric.id}</p>
              <p className="text-[11px] text-slate-500 dark:text-slate-400 mb-3">
                {metric.resource_type} • {metric.status}
              </p>

              {metric.account_id && (
                <p className="text-[11px] text-slate-500 dark:text-slate-400 mb-3 truncate">Account: {metric.account_id}</p>
              )}

              <div className="grid grid-cols-2 gap-4 border-t border-slate-100 dark:border-slate-700 pt-4">
                <div>
                  <div className="text-[10px] text-slate-400 uppercase font-bold mb-1 flex items-center">
                    <Cpu className="w-3 h-3 mr-1" /> CPU Avg
                  </div>
                  <div
                    className={`text-lg font-bold ${
                      (metric.cpu_utilization || 0) > 80
                        ? "text-red-600"
                        : metric.cpu_utilization !== undefined && metric.cpu_utilization !== null && (metric.cpu_utilization || 0) < 5
                        ? "text-emerald-600"
                        : "text-slate-700 dark:text-slate-200"
                    }`}
                  >
                    {metric.cpu_utilization !== undefined && metric.cpu_utilization !== null ? `${metric.cpu_utilization.toFixed(1)}%` : "-"}
                  </div>
                </div>
                <div className="text-right">
                  <div className="text-[10px] text-slate-400 uppercase font-bold mb-1 flex items-center justify-end">
                    <Wifi className="w-3 h-3 mr-1" /> Net In
                  </div>
                  <div className="text-lg font-bold text-slate-700 dark:text-slate-200">
                    {metric.network_in_mb !== undefined && metric.network_in_mb !== null ? `${metric.network_in_mb.toFixed(1)} MB` : "-"}
                  </div>
                </div>
              </div>

              {metricAge !== null && (
                <p className="mt-3 text-[11px] text-slate-400">Updated {formatAge(metricAge)}</p>
              )}
            </div>
          );
        })}

        {filtered.length === 0 && !loading && (
          <div className="col-span-full p-16 text-center">
            <div className="w-20 h-20 bg-slate-100 dark:bg-slate-800 rounded-full flex items-center justify-center mx-auto mb-4">
              <Server className="w-10 h-10 text-slate-300 dark:text-slate-400" />
            </div>
            <h3 className="text-lg font-bold text-slate-900 dark:text-white">No Health Metrics Yet</h3>
            <p className="text-slate-500 dark:text-slate-400 mt-2">Connect accounts and run scan or refresh metrics to build your health timeline.</p>
          </div>
        )}
      </div>
    </div>
  );
}
