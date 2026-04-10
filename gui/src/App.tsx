import { Suspense, lazy, useState, useEffect } from "react";
import { Sidebar } from "./components/Sidebar";
import { invoke } from "@tauri-apps/api/core";

const Dashboard = lazy(() => import("./components/Dashboard").then((mod) => ({ default: mod.Dashboard })));
const MonitorScreen = lazy(() => import("./components/MonitorScreen").then((mod) => ({ default: mod.MonitorScreen })));
const GovernanceScreen = lazy(() => import("./components/GovernanceScreen").then((mod) => ({ default: mod.GovernanceScreen })));
const ResourcesTable = lazy(() => import("./components/ResourcesTable").then((mod) => ({ default: mod.ResourcesTable })));
const ProviderResourcesScreen = lazy(() => import("./components/ProviderResourcesScreen").then((mod) => ({ default: mod.ProviderResourcesScreen })));
const Settings = lazy(() => import("./components/Settings").then((mod) => ({ default: mod.Settings })));
const LogsScreen = lazy(() => import("./components/LogsScreen").then((mod) => ({ default: mod.LogsScreen })));
const HistoryScreen = lazy(() => import("./components/HistoryScreen").then((mod) => ({ default: mod.HistoryScreen })));
const FeedbackManager = lazy(() => import("./components/FeedbackManager").then((mod) => ({ default: mod.FeedbackManager })));
const SettingsHubScreen = lazy(() => import("./components/SettingsHubScreen").then((mod) => ({ default: mod.SettingsHubScreen })));
const SystemLogsScreen = lazy(() => import("./components/SystemLogsScreen").then((mod) => ({ default: mod.SystemLogsScreen })));
const SupportHubScreen = lazy(() => import("./components/SupportHubScreen").then((mod) => ({ default: mod.SupportHubScreen })));
const AiAnalystScreen = lazy(() => import("./components/AiAnalystScreen").then((mod) => ({ default: mod.AiAnalystScreen })));
const AiSettingsScreen = lazy(() => import("./components/AiSettingsScreen").then((mod) => ({ default: mod.AiSettingsScreen })));

function App() {
  const [activeTab, setActiveTab] = useState("overview");
  const [activeTabParams, setActiveTabParams] = useState<any>(null);
  const [isCompactDesktop, setIsCompactDesktop] = useState(false);
  const [isBelowRecommendedWidth, setIsBelowRecommendedWidth] = useState(false);

  const normalizeTab = (tab: string) => {
    switch (tab) {
      case "dashboard":
        return "overview";
      case "scan_results":
        return "current_findings";
      case "resources":
        return "resource_inventory";
      case "monitor":
        return "health_metrics";
      case "logs":
        return "audit_log";
      case "settings":
        return "configuration";
      case "scan_history":
      case "scan-history":
        return "history";
      default:
        return tab;
    }
  };

  useEffect(() => {
      // Track Tab Changes
      invoke("track_event", { event: "app_view_tab", meta: { tab: activeTab } }).catch(console.error);
  }, [activeTab]);

  const handleNavigate = (tab: string, params?: any) => {
      setActiveTab(normalizeTab(tab));
      setActiveTabParams(params ?? null);
  };

  useEffect(() => {
      // Apply theme and font size on mount
      const theme = localStorage.getItem("theme") || "dark";
      if (theme === 'dark') {
          document.documentElement.classList.add('dark');
          document.body.classList.add('dark');
      } else {
          document.documentElement.classList.remove('dark');
          document.body.classList.remove('dark');
      }

      const size = localStorage.getItem("fontSize") || "medium";
      const root = document.documentElement;
      if (size === 'small') root.style.fontSize = '16px';
      else if (size === 'large') root.style.fontSize = '20px';
      else root.style.fontSize = '18px';
  }, []);

  useEffect(() => {
      const updateViewportFlags = () => {
          const width = window.innerWidth;
          setIsCompactDesktop(width < 1280);
          setIsBelowRecommendedWidth(width < 1200);
      };

      updateViewportFlags();
      window.addEventListener("resize", updateViewportFlags);
      return () => window.removeEventListener("resize", updateViewportFlags);
  }, []);

  return (
    <div className={`cws-app-shell flex h-screen bg-white dark:bg-slate-900 overflow-hidden font-sans ${isCompactDesktop ? "cws-compact-desktop" : ""}`}>
      <Sidebar 
        currentTab={activeTab} 
        onTabChange={(tab) => handleNavigate(tab)}
      />
      
      <main className="cws-app-main flex-1 overflow-auto bg-slate-50 dark:bg-slate-900 transition-colors duration-300">
        {isBelowRecommendedWidth ? (
          <div className="cws-viewport-advisory border-b border-amber-200 bg-amber-50 px-4 py-2 text-xs font-medium text-amber-800 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-200">
            Best experience starts at 1200px width. Expand the window or switch to full-screen for dense audit workflows.
          </div>
        ) : null}
        <Suspense fallback={<ScreenFallback />}>
          {activeTab === 'overview' && <Dashboard onNavigate={handleNavigate} />}
          {activeTab === 'governance' && <GovernanceScreen />}
          {activeTab === 'health_metrics' && <MonitorScreen />}
          {activeTab === 'ai_analyst' && <AiAnalystScreen onNavigate={handleNavigate} />}
          {activeTab === 'current_findings' && <ResourcesTable initialFilter={activeTabParams} />}
          {activeTab === 'resource_inventory' && <ProviderResourcesScreen />}
          {activeTab === 'history' && <HistoryScreen />}
          {activeTab === 'audit_log' && <LogsScreen initialFilter={activeTabParams} />}
          {activeTab === 'system_logs' && <SystemLogsScreen />}
          {activeTab === 'support_center' && <SupportHubScreen onNavigate={(tab) => handleNavigate(tab)} />}
          {activeTab === 'feedback' && <FeedbackManager />}
          {activeTab === 'configuration' && <SettingsHubScreen onNavigate={(tab) => handleNavigate(tab)} />}
          {activeTab === 'accounts' && <Settings initialTab="clouds" pageTitle="Accounts" pageSubtitle="Manage cloud credentials, provider imports, account-level rules, and delivery targets for each environment." showTabStrip={false} />}
          {activeTab === 'notifications' && <Settings initialTab="notifications" pageTitle="Notifications" pageSubtitle="Control alert delivery, escalation thresholds, and outbound channels used after each scan." showTabStrip={false} />}
          {activeTab === 'network_proxy' && <Settings initialTab="proxies" pageTitle="Proxy Profiles" pageSubtitle="Define named proxy routes and assign direct or proxied egress per account and notification channel." showTabStrip={false} />}
          {activeTab === 'local_api' && <Settings initialTab="network" pageTitle="Local API" pageSubtitle="Configure bind host, TLS posture, bearer token access, and LAN exposure for the embedded API." showTabStrip={false} />}
          {activeTab === 'preferences' && <Settings initialTab="appearance" pageTitle="Preferences" pageSubtitle="Set operator theme, typography size, and reporting currency used across the desktop experience." showTabStrip={false} />}
          {activeTab === 'ai_settings' && <AiSettingsScreen />}
        </Suspense>
      </main>
    </div>
  );
}

function ScreenFallback() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-slate-50 text-sm font-medium text-slate-500 dark:bg-slate-900 dark:text-slate-400">
      Loading workspace...
    </div>
  );
}

export default App;
