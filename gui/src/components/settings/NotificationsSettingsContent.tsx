import {
  AlertTriangle,
  Bell,
  CheckCircle,
  Hash,
  Loader2,
  Mail,
  MessageCircle,
  MessageSquare,
  Pencil,
  Plus,
  Trash2,
  Webhook,
} from "lucide-react";

interface NotificationChannel {
  id: string;
  name: string;
  method: string;
  config: string;
  is_active: boolean;
  proxy_profile_id?: string | null;
  trigger_mode?: string | null;
  min_savings?: number | null;
  min_findings?: number | null;
}

interface NotificationFeedback {
  type: "success" | "error";
  title: string;
  details: string;
}

interface NotificationsSettingsContentProps {
  notificationChannels: NotificationChannel[];
  openAddChannel: () => void;
  normalizeNotificationMethod: (value?: string | null) => string;
  resolveProxySelectionLabel: (value?: string | null) => string;
  resolveNotificationChannelTriggerLabel: (value?: string | null) => string;
  resolveNotificationChannelEffectiveTrigger: (value?: string | null) => string;
  resolveNotificationChannelThresholdLabel: (channel: NotificationChannel) => string;
  handleToggleNotifActive: (channel: NotificationChannel, isActive: boolean) => void | Promise<void>;
  handleTestNotif: (channel: NotificationChannel) => void | Promise<void>;
  openNotifEdit: (channel: NotificationChannel) => void;
  handleDeleteNotif: (id: string) => void | Promise<void>;
  togglingNotifId: string | null;
  testingNotifId: string | null;
  notifTestFeedback: NotificationFeedback | null;
  dismissNotifFeedback: () => void;
}

export function NotificationsSettingsContent({
  notificationChannels,
  openAddChannel,
  normalizeNotificationMethod,
  resolveProxySelectionLabel,
  resolveNotificationChannelTriggerLabel,
  resolveNotificationChannelEffectiveTrigger,
  resolveNotificationChannelThresholdLabel,
  handleToggleNotifActive,
  handleTestNotif,
  openNotifEdit,
  handleDeleteNotif,
  togglingNotifId,
  testingNotifId,
  notifTestFeedback,
  dismissNotifFeedback,
}: NotificationsSettingsContentProps) {
  return (
    <div className="bg-white dark:bg-slate-800 p-8 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm w-full animate-in fade-in slide-in-from-right-4">
      <div className="flex justify-between items-center mb-6">
        <div>
          <h2 className="text-xl font-semibold text-slate-900 dark:text-white">Alert Channels</h2>
          <p className="text-base text-slate-500 dark:text-slate-400">Configure where you want to receive alerts.</p>
        </div>
        <button
          onClick={openAddChannel}
          className="flex items-center px-5 py-3 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 font-medium transition-all text-base shadow-sm"
        >
          <Plus className="w-3 h-3 mr-1" /> Add Channel
        </button>
      </div>

      <div className="space-y-3">
        {notificationChannels.length === 0 && (
          <div className="text-center py-8 text-slate-400 text-lg bg-slate-50 dark:bg-slate-900/30 rounded-xl border border-dashed border-slate-200 dark:border-slate-700">
            No notification channels configured.
          </div>
        )}

        {notificationChannels.map((channel) => {
          const normalizedMethod = normalizeNotificationMethod(channel.method);
          return (
            <div
              key={channel.id}
              className="p-4 border border-slate-200 dark:border-slate-700 rounded-xl bg-slate-50 dark:bg-slate-900/50 flex flex-col gap-3 md:flex-row md:justify-between md:items-center group"
            >
              <div className="flex items-center gap-3">
                <div className={`p-3 rounded-lg shadow-sm border border-slate-200 dark:border-slate-700 ${
                  normalizedMethod === "slack" ? "bg-white dark:bg-slate-800 text-indigo-600 dark:text-indigo-400" :
                  normalizedMethod === "teams" ? "bg-white dark:bg-slate-800 text-blue-600 dark:text-blue-400" :
                  normalizedMethod === "discord" ? "bg-white dark:bg-slate-800 text-indigo-500 dark:text-indigo-400" :
                  normalizedMethod === "telegram" ? "bg-white dark:bg-slate-800 text-sky-500 dark:text-sky-400" :
                  normalizedMethod === "whatsapp" ? "bg-white dark:bg-slate-800 text-emerald-500 dark:text-emerald-400" :
                  normalizedMethod === "email" ? "bg-white dark:bg-slate-800 text-rose-500 dark:text-rose-400" :
                  "bg-white dark:bg-slate-800 text-amber-600 dark:text-amber-400"
                }`}>
                  {normalizedMethod === "slack" ? <Hash className="w-5 h-5" /> :
                    normalizedMethod === "teams" ? <MessageSquare className="w-5 h-5" /> :
                    normalizedMethod === "discord" ? <Webhook className="w-5 h-5" /> :
                    normalizedMethod === "telegram" ? <Plus className="w-5 h-5 rotate-45" /> :
                    normalizedMethod === "whatsapp" ? <MessageCircle className="w-5 h-5" /> :
                    normalizedMethod === "email" ? <Mail className="w-5 h-5" /> :
                    <Webhook className="w-5 h-5" />}
                </div>
                <div>
                  <h4 className="font-bold text-slate-900 dark:text-white">{channel.name}</h4>
                  <p className="text-[10px] uppercase font-bold text-slate-400">{normalizedMethod}</p>
                  <p className={`text-[11px] font-semibold ${channel.is_active ? "text-emerald-600 dark:text-emerald-400" : "text-amber-600 dark:text-amber-400"}`}>
                    {channel.is_active ? "Enabled" : "Paused"}
                  </p>
                  <p className="text-[11px] text-slate-500 dark:text-slate-400">
                    {resolveProxySelectionLabel(channel.proxy_profile_id)}
                  </p>
                  <p className="text-[11px] text-slate-500 dark:text-slate-400">
                    Trigger: {resolveNotificationChannelTriggerLabel(channel.trigger_mode)}
                  </p>
                  {resolveNotificationChannelEffectiveTrigger(channel.trigger_mode) === "waste_only" && (
                    <p className="text-[11px] text-slate-500 dark:text-slate-400">
                      Thresholds: {resolveNotificationChannelThresholdLabel(channel)}
                    </p>
                  )}
                </div>
              </div>
              <div className="flex gap-2 items-center flex-wrap md:justify-end">
                <button
                  onClick={() => handleToggleNotifActive(channel, !channel.is_active)}
                  disabled={togglingNotifId === channel.id}
                  className="inline-flex items-center gap-2 px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-600 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors disabled:opacity-60"
                  title={channel.is_active ? "Pause channel" : "Enable channel"}
                >
                  <span className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${channel.is_active ? "bg-emerald-500" : "bg-slate-300 dark:bg-slate-600"}`}>
                    <span className={`inline-block h-4 w-4 transform rounded-full bg-white transition ${channel.is_active ? "translate-x-4" : "translate-x-1"}`} />
                  </span>
                  <span className="text-sm font-medium">
                    {togglingNotifId === channel.id ? "Updating..." : (channel.is_active ? "On" : "Off")}
                  </span>
                </button>
                <button
                  onClick={() => handleTestNotif(channel)}
                  disabled={testingNotifId === channel.id}
                  className="inline-flex items-center gap-2 px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-600 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors disabled:opacity-60"
                  title="Send Test Notification"
                >
                  {testingNotifId === channel.id ? <Loader2 className="w-4 h-4 animate-spin" /> : <Bell className="w-4 h-4" />}
                  <span className="text-sm font-medium">{testingNotifId === channel.id ? "Testing..." : "Send Test"}</span>
                </button>
                <button onClick={() => openNotifEdit(channel)} className="p-3 text-slate-400 hover:text-indigo-600 transition-colors" title="Edit">
                  <Pencil className="w-5 h-5" />
                </button>
                <button onClick={() => handleDeleteNotif(channel.id)} className="p-3 text-slate-400 hover:text-red-600 transition-colors" title="Delete">
                  <Trash2 className="w-5 h-5" />
                </button>
              </div>
            </div>
          );
        })}
      </div>

      {notifTestFeedback && (
        <div className={`mt-4 p-4 rounded-xl border ${notifTestFeedback.type === "success"
          ? "bg-emerald-50 dark:bg-emerald-900/20 border-emerald-200 dark:border-emerald-800 text-emerald-800 dark:text-emerald-300"
          : "bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800 text-red-800 dark:text-red-300"
        }`}>
          <div className="flex items-start justify-between gap-3">
            <div className="flex items-start gap-2 min-w-0">
              {notifTestFeedback.type === "success"
                ? <CheckCircle className="w-5 h-5 mt-0.5 flex-shrink-0" />
                : <AlertTriangle className="w-5 h-5 mt-0.5 flex-shrink-0" />
              }
              <div className="min-w-0">
                <p className="font-semibold">{notifTestFeedback.title}</p>
                <p className="mt-1 text-sm whitespace-pre-wrap break-words">{notifTestFeedback.details}</p>
              </div>
            </div>
            <button
              onClick={dismissNotifFeedback}
              className="text-xs font-semibold px-2 py-1 rounded border border-current/40 hover:bg-white/20 transition-colors"
            >
              Dismiss
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
