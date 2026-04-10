import { type ReactNode } from "react";
import { Loader2, Monitor, Network, X } from "lucide-react";
import { CustomSelect } from "../CustomSelect";

interface ScanRule {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
  params: string;
}

interface SelectOption {
  value: string;
  label: string;
}

interface NotificationChannelOption {
  id: string;
  name: string;
  method: string;
}

interface AccountProfileModalProps {
  open: boolean;
  modalMode: "add" | "edit";
  modalTab: "credentials" | "rules";
  onClose: () => void;
  onSetModalTab: (tab: "credentials" | "rules") => void;
  selectedProvider: string;
  onSelectedProviderChange: (value: string) => void;
  providerOptions: SelectOption[];
  providerSelectionDisabled: boolean;
  selectedAccountProxy: string;
  onSelectedAccountProxyChange: (value: string) => void;
  proxySelectionOptions: SelectOption[];
  selectedAccountNotifications: string[];
  isAllNotificationsSelected: (value?: string[] | string | null) => boolean;
  toggleAccountNotificationSelection: (channelId: string, checked: boolean) => void;
  allChannelsChoiceValue: string;
  notificationChannels: NotificationChannelOption[];
  normalizeNotificationMethod: (value?: string | null) => string;
  credentialsContent: ReactNode;
  accountRules: ScanRule[];
  onToggleRule: (ruleId: string, enabled: boolean) => void;
  testing: boolean;
  onTestConnection: () => void | Promise<void>;
  onSave: () => void | Promise<void>;
}

export function AccountProfileModal({
  open,
  modalMode,
  modalTab,
  onClose,
  onSetModalTab,
  selectedProvider,
  onSelectedProviderChange,
  providerOptions,
  providerSelectionDisabled,
  selectedAccountProxy,
  onSelectedAccountProxyChange,
  proxySelectionOptions,
  selectedAccountNotifications,
  isAllNotificationsSelected,
  toggleAccountNotificationSelection,
  allChannelsChoiceValue,
  notificationChannels,
  normalizeNotificationMethod,
  credentialsContent,
  accountRules,
  onToggleRule,
  testing,
  onTestConnection,
  onSave,
}: AccountProfileModalProps) {
  if (!open) {
    return null;
  }

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4">
      <div className="bg-white dark:bg-slate-800 rounded-xl p-6 w-full max-w-3xl shadow-2xl animate-in zoom-in-95 border border-slate-200 dark:border-slate-700">
        <div className="flex justify-between items-center mb-6">
          <div>
            <h3 className="text-2xl font-bold text-slate-900 dark:text-white">
              {modalMode === "edit" ? "Edit Account" : "Add Account"}
            </h3>
            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
              Configure one provider at a time, validate connectivity locally, then apply scan policy.
            </p>
          </div>
          <button onClick={onClose} className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-200">
            <X className="w-5 h-5" />
          </button>
        </div>
        <div className="flex border-b border-slate-200 dark:border-slate-700 mb-4">
          <button
            onClick={() => onSetModalTab("credentials")}
            className={`px-4 py-2 font-bold text-sm transition-colors ${modalTab === "credentials" ? "text-indigo-600 border-b-2 border-indigo-600 dark:text-indigo-400 dark:border-indigo-400" : "text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200"}`}
          >
            Credentials
          </button>
          <button
            onClick={() => onSetModalTab("rules")}
            className={`px-4 py-2 font-bold text-sm transition-colors ${modalTab === "rules" ? "text-indigo-600 border-b-2 border-indigo-600 dark:text-indigo-400 dark:border-indigo-400" : "text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200"}`}
          >
            Scanning Rules
          </button>
        </div>

        <div className="space-y-4 h-[420px] overflow-y-auto px-1 pb-4">
          {modalTab === "credentials" ? (
            <>
              <div className="grid gap-3 rounded-2xl border border-slate-200 bg-slate-50 p-4 dark:border-slate-700 dark:bg-slate-900/40 md:grid-cols-3">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
                    Storage Boundary
                  </p>
                  <p className="mt-2 text-sm text-slate-700 dark:text-slate-200">
                    Credentials stay on this machine. Cloud Waste Scanner does not relay them elsewhere.
                  </p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
                    Validation Path
                  </p>
                  <p className="mt-2 text-sm text-slate-700 dark:text-slate-200">
                    Test Connection uses the same proxy selection and immediate credential payload shown below.
                  </p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
                    Operator Outcome
                  </p>
                  <p className="mt-2 text-sm text-slate-700 dark:text-slate-200">
                    Save the account only after test success, then tune scanning rules for that environment.
                  </p>
                </div>
              </div>
              <div>
                <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Provider</label>
                <div className="relative">
                  <CustomSelect
                    value={selectedProvider}
                    onChange={onSelectedProviderChange}
                    disabled={providerSelectionDisabled}
                    searchable
                    searchPlaceholder="Search provider..."
                    options={providerOptions}
                  />
                </div>
              </div>
              <div>
                <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Account Proxy</label>
                <div className="relative">
                  <CustomSelect
                    value={selectedAccountProxy}
                    onChange={onSelectedAccountProxyChange}
                    options={proxySelectionOptions}
                  />
                </div>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-1">
                  Default is direct access (no proxy). Select a proxy only when this account requires one.
                </p>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-1">
                  Test Connection uses the proxy selected here immediately, even before you save.
                </p>
              </div>
              <div>
                <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Account Notification Channel</label>
                <div className="rounded-lg border border-slate-200 dark:border-slate-600 bg-white dark:bg-slate-700 p-3 space-y-2 max-h-44 overflow-y-auto">
                  <label className="flex items-center gap-2 text-sm font-semibold text-slate-700 dark:text-slate-200">
                    <input
                      type="checkbox"
                      className="w-4 h-4 accent-indigo-600"
                      checked={isAllNotificationsSelected(selectedAccountNotifications)}
                      onChange={(event) => toggleAccountNotificationSelection(allChannelsChoiceValue, event.target.checked)}
                    />
                    Use all active channels (Recommended)
                  </label>
                  {notificationChannels.length === 0 ? (
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      No channels configured yet. This account will use all active channels when they are added.
                    </p>
                  ) : (
                    notificationChannels.map((channel) => (
                      <label key={`account-notif-${channel.id}`} className="flex items-center gap-2 text-sm text-slate-700 dark:text-slate-200">
                        <input
                          type="checkbox"
                          className="w-4 h-4 accent-indigo-600"
                          checked={selectedAccountNotifications.includes(channel.id)}
                          onChange={(event) => toggleAccountNotificationSelection(channel.id, event.target.checked)}
                        />
                        <span>{channel.name}</span>
                        <span className="text-xs text-slate-500 dark:text-slate-400">
                          ({normalizeNotificationMethod(channel.method)})
                        </span>
                      </label>
                    ))
                  )}
                </div>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-1">
                  Supports multi-select. Select one or more channels, or keep all active channels.
                </p>
              </div>
              {credentialsContent}
            </>
          ) : (
            <div className="space-y-3">
              {accountRules.length === 0 ? (
                <div className="text-center py-12 text-slate-400 dark:text-slate-500 italic">
                  No configurable rules found for this provider.
                </div>
              ) : (
                accountRules.map((rule) => (
                  <div key={rule.id} className="flex items-center justify-between p-4 border border-slate-200 dark:border-slate-700 rounded-xl bg-white dark:bg-slate-700/30 hover:border-indigo-200 dark:hover:border-indigo-800 transition-colors">
                    <div className="flex-1 pr-4">
                      <p className="font-bold text-slate-900 dark:text-white text-sm">{rule.name}</p>
                      <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">{rule.description}</p>
                    </div>
                    <div
                      onClick={() => onToggleRule(rule.id, !rule.enabled)}
                      className={`w-12 h-6 rounded-full cursor-pointer relative transition-all duration-300 ${rule.enabled ? "bg-indigo-600" : "bg-slate-300 dark:bg-slate-600"}`}
                    >
                      <div className={`w-4 h-4 bg-white rounded-full absolute top-1 shadow-sm transition-all duration-300 ${rule.enabled ? "left-7" : "left-1"}`} />
                    </div>
                  </div>
                ))
              )}
              <div className="mt-4 p-3 bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 text-xs rounded-lg flex items-start">
                <Monitor className="w-4 h-4 mr-2 flex-shrink-0 mt-0.5" />
                <p>Enabled rules will be applied during the next scan. Disabling a rule skips that check entirely for this account.</p>
              </div>
            </div>
          )}
        </div>
        <div className="mt-8 flex flex-wrap justify-end gap-3">
          <button
            onClick={onTestConnection}
            disabled={testing}
            className="flex items-center px-4 py-3 bg-slate-100 dark:bg-slate-700 text-slate-700 dark:text-slate-200 hover:bg-slate-200 dark:hover:bg-slate-600 rounded-lg font-medium transition-colors disabled:opacity-50"
          >
            {testing ? <Loader2 className="w-5 h-5 mr-2 animate-spin" /> : <Network className="w-5 h-5 mr-2" />}
            {testing ? "Testing..." : "Test Connection"}
          </button>
          <button onClick={onClose} className="px-4 py-3 text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-white transition-colors font-medium">Cancel</button>
          <button onClick={onSave} className="bg-indigo-600 hover:bg-indigo-700 text-white px-6 py-3 rounded-lg font-bold transition-all shadow-lg shadow-indigo-500/20">Save</button>
        </div>
        <div className="mt-4 p-3 bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 text-xs rounded-lg flex items-start">
          <Monitor className="w-4 h-4 mr-2 flex-shrink-0 mt-0.5" />
          <p>Testing connection does not consume scan quota, and saving an account only updates the local credential store.</p>
        </div>
      </div>
    </div>
  );
}
