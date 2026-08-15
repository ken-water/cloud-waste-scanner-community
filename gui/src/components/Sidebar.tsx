import { useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import appIcon from "../assets/cws-app-icon.svg";
import {
  LayoutDashboard,
  Database,
  Server,
  Cloud,
  Settings,
  History,
  MessageSquare,
  LifeBuoy,
  CircleDot,
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
      title: 'Scan',
      items: [
        { id: 'overview', label: 'First Scan', icon: LayoutDashboard },
        { id: 'current_findings', label: 'Scan Results', icon: Server },
        { id: 'resource_inventory', label: 'Resource Inventory', icon: Database },
        { id: 'history', label: 'Scan History', icon: History },
      ],
    },
    {
      title: 'Connect',
      items: [
        { id: 'accounts', label: 'Accounts', icon: Cloud },
        { id: 'configuration', label: 'Settings', icon: Settings },
      ],
    },
    {
      title: 'Help',
      items: [
        { id: 'support_center', label: 'Support Center', icon: LifeBuoy },
        { id: 'feedback', label: 'Feedback', icon: MessageSquare },
      ],
    },
  ];

  return (
    <div className="cws-app-sidebar w-64 bg-white text-slate-900 dark:bg-slate-950 dark:text-white flex flex-col h-full border-r border-slate-200 dark:border-slate-800 transition-colors duration-300">
      <div className="cws-sidebar-brand border-b border-slate-200 p-5 dark:border-slate-800">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center overflow-hidden rounded-xl bg-slate-950 shadow-sm ring-1 ring-slate-800 dark:bg-slate-950 dark:ring-slate-700">
            <img src={appIcon} alt="" aria-hidden="true" className="h-10 w-10" />
          </div>
          <div className="min-w-0">
            <span className="block truncate text-base font-bold tracking-tight text-slate-950 dark:text-white">Cloud Waste Scanner</span>
            <span className="mt-0.5 block text-xs font-semibold text-slate-500 dark:text-slate-400">Local scan workspace</span>
          </div>
        </div>
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
                        ? 'bg-slate-900 text-white shadow-lg shadow-slate-900/[0.12] dark:border-teal-500/30 dark:bg-teal-500/[0.12] dark:text-teal-100 dark:shadow-teal-950/20'
                        : 'text-slate-600 hover:bg-slate-100 hover:text-slate-950 dark:text-slate-400 dark:hover:bg-slate-900 dark:hover:text-white'
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

      <div className="border-t border-slate-200 p-4 dark:border-slate-800">
        <div className="rounded-xl border border-slate-200 bg-slate-50 p-3 text-xs text-slate-600 shadow-sm dark:border-slate-800 dark:bg-slate-900 dark:text-slate-400 transition-colors duration-300">
          <div className="flex items-center justify-between gap-2">
            <p className="font-bold text-slate-900 dark:text-slate-200">Local beta</p>
            <span className="inline-flex items-center gap-1 rounded-full bg-emerald-100 px-2 py-1 text-[10px] font-bold uppercase tracking-wide text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300">
              <CircleDot className="h-3 w-3" /> Local
            </span>
          </div>
          <p className="mt-2 text-[11px] leading-5 text-slate-500 dark:text-slate-400">
            Scan credentials, evidence, and exports stay on this machine.
          </p>
          <div className="mt-3 flex justify-between border-t border-slate-200 pt-2 dark:border-slate-800">
              <span>Version</span>
              <span className="font-semibold text-slate-700 dark:text-slate-300">v{version || "..."}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
