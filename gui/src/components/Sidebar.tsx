import { useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import {
  LayoutDashboard,
  Building2,
  Database,
  Server,
  Cloud,
  Settings,
  ShieldCheck,
  History,
  Activity,
  MessageSquare,
  ClipboardList,
  Bell,
  Network,
  Monitor,
  FileText,
  LifeBuoy,
  Bot,
} from 'lucide-react';

interface SidebarProps {
  currentTab: string;
  onTabChange: (tab: string) => void;
}

export function Sidebar({ currentTab, onTabChange }: SidebarProps) {
  const [version, setVersion] = useState("");

  useEffect(() => {
      const init = async () => {
          const v = await getVersion();
          setVersion(v);
      };
      init();

      return () => { 
      };
  }, []);
  
  const menuGroups = [
    {
      title: 'Operations',
      items: [
        { id: 'overview', label: 'Dashboard', icon: LayoutDashboard },
        { id: 'current_findings', label: 'Scan Results', icon: Server },
        { id: 'resource_inventory', label: 'Resource Inventory', icon: Database },
        { id: 'history', label: 'Scan History', icon: History },
      ],
    },
    {
      title: 'Analysis',
      items: [
        { id: 'governance', label: 'Governance', icon: Building2 },
        { id: 'health_metrics', label: 'Health Metrics', icon: Activity },
        { id: 'ai_analyst', label: 'AI Device Scan', icon: Bot },
      ],
    },
    {
      title: 'Configuration',
      items: [
        { id: 'configuration', label: 'Configuration', icon: Settings },
        { id: 'accounts', label: 'Accounts', icon: Cloud },
        { id: 'notifications', label: 'Notifications', icon: Bell },
        { id: 'network_proxy', label: 'Proxy Profiles', icon: Network },
        { id: 'ai_settings', label: 'AI Runtime Settings', icon: Bot },
        { id: 'local_api', label: 'Local API', icon: Monitor },
        { id: 'preferences', label: 'Preferences', icon: Settings },
      ],
    },
    {
      title: 'Support',
      items: [
        { id: 'support_center', label: 'Support Center', icon: LifeBuoy },
        { id: 'audit_log', label: 'Audit Log', icon: ClipboardList },
        { id: 'system_logs', label: 'System Logs', icon: FileText },
        { id: 'feedback', label: 'Feedback', icon: MessageSquare },
      ],
    },
  ];

  return (
    <div className="cws-app-sidebar w-64 bg-white text-slate-900 dark:bg-slate-900 dark:text-white flex flex-col h-full border-r border-slate-200 dark:border-slate-800 transition-colors duration-300">
      <div className="p-6 flex items-center space-x-2 border-b border-slate-200 dark:border-slate-800">
        <ShieldCheck className="h-6 w-6 text-indigo-600 dark:text-indigo-400" />
        <span className="font-bold text-lg tracking-tight text-slate-900 dark:text-white">CWS Community</span>
      </div>
      
      <nav className="flex-1 space-y-6 overflow-y-auto p-4">
        {menuGroups.map((group) => (
          <div key={group.title}>
            <p className="cws-nav-group-title mb-2 px-3 text-[11px] font-semibold uppercase tracking-[0.24em] text-slate-400 dark:text-slate-500">
              {group.title}
            </p>
            <div className="space-y-2">
              {group.items.map((item) => {
                const Icon = item.icon;
                const isActive = currentTab === item.id;
                return (
                  <button
                    key={item.id}
                    onClick={() => onTabChange(item.id)}
                    className={`cws-menu-button w-full flex items-center gap-2.5 px-3 py-2.5 rounded-lg transition-all duration-200 ${
                      isActive
                        ? 'bg-indigo-500 text-white shadow-lg shadow-indigo-500/20'
                        : 'text-slate-600 hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-white'
                    }`}
                  >
                    <Icon className="h-5 w-5 shrink-0" />
                    <div className="min-w-0 flex-1 text-left">
                      <span className="cws-menu-label block truncate whitespace-nowrap font-medium">{item.label}</span>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>
        ))}
      </nav>

      <div className="p-4 border-t border-slate-200 dark:border-slate-800">
        <div className="bg-slate-100 rounded-lg p-3 text-xs text-slate-600 dark:bg-slate-800/50 dark:text-slate-400 transition-colors duration-300">
          <p className="font-semibold text-slate-800 dark:text-slate-300 mb-1">Community Edition</p>
          <div className="flex justify-between items-center">
              <span>Status:</span>
              <span className="text-emerald-500 font-bold">Local Active</span>
          </div>
          <div className="flex justify-between items-center mt-1">
              <span>Version:</span>
              <span>v{version}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
