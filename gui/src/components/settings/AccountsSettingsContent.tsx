import { Cloud, Loader2, Network, Pencil, Plus, Trash2, Upload } from "lucide-react";

interface AwsProfile {
  name: string;
  region: string;
  key?: string;
  secret?: string;
  auth_type?: string;
}

interface CloudProfile {
  id: string;
  provider: string;
  name: string;
  credentials: string;
  proxy_profile_id?: string | null;
}

interface AccountsSettingsContentProps {
  awsProfiles: AwsProfile[];
  cloudProfiles: CloudProfile[];
  accountNotificationAssignments: Record<string, string[]>;
  resolveAccountNotificationLabel: (value?: string[] | string | null) => string;
  openImportModal: () => void | Promise<void>;
  onAddAccount: () => void;
  handleQuickTestAwsProfile: (profile: AwsProfile) => void | Promise<void>;
  handleQuickTestCloudProfile: (profile: CloudProfile) => void | Promise<void>;
  openAwsEditModal: (profile: AwsProfile) => void;
  openEditModal: (profile: CloudProfile) => void;
  handleDeleteAws: (name: string) => void | Promise<void>;
  handleDeleteCloud: (id: string) => void | Promise<void>;
  testingAccountId: string | null;
}

export function AccountsSettingsContent({
  awsProfiles,
  cloudProfiles,
  accountNotificationAssignments,
  resolveAccountNotificationLabel,
  openImportModal,
  onAddAccount,
  handleQuickTestAwsProfile,
  handleQuickTestCloudProfile,
  openAwsEditModal,
  openEditModal,
  handleDeleteAws,
  handleDeleteCloud,
  testingAccountId,
}: AccountsSettingsContentProps) {
  const totalAccounts = awsProfiles.length + cloudProfiles.length;
  const routedAccounts = Object.values(accountNotificationAssignments).filter(
    (channels) => Array.isArray(channels) && channels.length > 0,
  ).length;

  return (
    <div className="space-y-6 animate-in fade-in slide-in-from-right-4">
      <div className="flex justify-between items-center">
        <div>
          <h2 className="text-xl font-semibold text-slate-900 dark:text-white">Connected Accounts</h2>
          <p className="text-lg text-slate-500 dark:text-slate-400">Manage access to your cloud providers.</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={openImportModal}
            className="flex items-center px-4 py-3 bg-white dark:bg-slate-800 text-slate-700 dark:text-slate-100 rounded-lg border border-slate-200 dark:border-slate-600 hover:bg-slate-50 dark:hover:bg-slate-700 font-medium transition-all shadow-sm"
          >
            <Upload className="w-5 h-5 mr-1" /> Import Accounts
          </button>
          <button
            onClick={onAddAccount}
            className="flex items-center px-4 py-3 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 font-medium transition-all shadow-sm"
          >
            <Plus className="w-5 h-5 mr-1" /> Add Account
          </button>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
          <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
            Connected
          </p>
          <p className="mt-3 text-3xl font-semibold text-slate-900 dark:text-white">{totalAccounts}</p>
          <p className="mt-2 text-sm text-slate-500 dark:text-slate-400">
            {awsProfiles.length} AWS local profiles and {cloudProfiles.length} cloud API accounts.
          </p>
        </div>
        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
          <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">
            Notification Routing
          </p>
          <p className="mt-3 text-3xl font-semibold text-slate-900 dark:text-white">{routedAccounts}</p>
          <p className="mt-2 text-sm text-slate-500 dark:text-slate-400">
            Accounts already mapped to explicit channels or all-active delivery.
          </p>
        </div>
        <div className="rounded-2xl border border-indigo-200 bg-indigo-50/70 p-5 shadow-sm dark:border-indigo-500/30 dark:bg-indigo-500/10">
          <p className="text-xs font-semibold uppercase tracking-[0.22em] text-indigo-700 dark:text-indigo-300">
            Operator Note
          </p>
          <p className="mt-3 text-sm leading-6 text-indigo-700 dark:text-indigo-200">
            Connection tests stay local and do not consume scan quota. Use them before handing an account to scheduled scans.
          </p>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-4">
        {totalAccounts === 0 && (
          <div className="rounded-2xl border border-dashed border-slate-300 bg-slate-50 p-8 text-center dark:border-slate-700 dark:bg-slate-800/50">
            <p className="text-lg font-semibold text-slate-900 dark:text-white">No accounts connected yet</p>
            <p className="mt-2 text-sm text-slate-500 dark:text-slate-400">
              Start with one provider, verify the connection locally, then expand coverage account by account.
            </p>
          </div>
        )}

        {awsProfiles.map((profile) => (
          <div
            key={`aws-${profile.name}`}
            className="bg-white dark:bg-slate-800 p-4 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm flex justify-between items-center transition-colors"
          >
            <div className="flex items-center">
              <div className="w-12 h-12 bg-orange-50 dark:bg-orange-900/20 rounded-lg flex items-center justify-center text-orange-600 mr-4 border border-orange-100 dark:border-orange-800 flex-shrink-0">
                <Cloud className="w-6 h-6" />
              </div>
              <div className="min-w-0">
                <p className="font-bold text-slate-900 dark:text-white truncate">{profile.name}</p>
                <p className="text-base text-slate-500 dark:text-slate-400 font-mono mt-0.5 uppercase">
                  AWS • {profile.region} • {((profile.auth_type || "").trim().toLowerCase() === "sso") ? "SSO" : "KEY"}
                </p>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-1">
                  Notify: {resolveAccountNotificationLabel(accountNotificationAssignments[`aws_local:${profile.name}`])}
                </p>
              </div>
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => handleQuickTestAwsProfile(profile)}
                disabled={testingAccountId !== null}
                className="p-3 text-slate-400 hover:text-emerald-600 dark:hover:text-emerald-400 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                title="Test Connection"
                aria-label={`Test connection for ${profile.name}`}
              >
                {testingAccountId === `aws:${profile.name}` ? <Loader2 className="w-5 h-5 animate-spin" /> : <Network className="w-5 h-5" />}
              </button>
              <button
                onClick={() => openAwsEditModal(profile)}
                className="p-3 text-slate-400 hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors"
              >
                <Pencil className="w-5 h-5" />
              </button>
              <button
                onClick={() => handleDeleteAws(profile.name)}
                className="p-3 text-slate-400 hover:text-red-600 dark:hover:text-red-400 transition-colors"
              >
                <Trash2 className="w-5 h-5" />
              </button>
            </div>
          </div>
        ))}

        {cloudProfiles.map((profile) => (
          <div
            key={profile.id}
            className="bg-white dark:bg-slate-800 p-4 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm flex justify-between items-center transition-colors"
          >
            <div className="flex items-center">
              <div className={`w-12 h-12 rounded-lg flex items-center justify-center mr-4 border flex-shrink-0 ${profile.provider === "azure" ? "bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 border-blue-100 dark:border-blue-800" : profile.provider === "gcp" ? "bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 border-red-100 dark:border-red-800" : profile.provider === "cloudflare" ? "bg-orange-50 dark:bg-orange-900/20 text-orange-500 dark:text-orange-400 border-orange-100 dark:border-orange-800" : profile.provider === "vultr" ? "bg-cyan-50 dark:bg-cyan-900/20 text-cyan-500 dark:text-cyan-400 border-cyan-100 dark:border-cyan-800" : "bg-cyan-50 dark:bg-cyan-900/20 text-cyan-600 dark:text-cyan-400 border-cyan-100 dark:border-cyan-800"}`}>
                <Cloud className="w-6 h-6" />
              </div>
              <div className="min-w-0">
                <p className="font-bold text-slate-900 dark:text-white truncate">{profile.name}</p>
                <p className="text-base text-slate-500 dark:text-slate-400 font-mono mt-0.5 uppercase">{profile.provider}</p>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-1">
                  Notify: {resolveAccountNotificationLabel(accountNotificationAssignments[profile.id])}
                </p>
              </div>
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => handleQuickTestCloudProfile(profile)}
                disabled={testingAccountId !== null}
                className="p-3 text-slate-400 hover:text-emerald-600 dark:hover:text-emerald-400 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                title="Test Connection"
                aria-label={`Test connection for ${profile.name}`}
              >
                {testingAccountId === `cloud:${profile.id}` ? <Loader2 className="w-5 h-5 animate-spin" /> : <Network className="w-5 h-5" />}
              </button>
              <button
                onClick={() => openEditModal(profile)}
                className="p-3 text-slate-400 hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors"
              >
                <Pencil className="w-5 h-5" />
              </button>
              <button
                onClick={() => handleDeleteCloud(profile.id)}
                className="p-3 text-slate-400 hover:text-red-600 dark:hover:text-red-400 transition-colors"
              >
                <Trash2 className="w-5 h-5" />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
