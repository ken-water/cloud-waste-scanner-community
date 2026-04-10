import { useState, useEffect, useRef, type ChangeEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { Cloud, Plus, Trash2, Settings as SettingsIcon, Monitor, Moon, Sun, Pencil, Hash, Save, X, Loader2, CheckCircle, Network, AlertTriangle, Bell, RefreshCw, Eye, EyeOff } from "lucide-react";
import { CustomSelect } from "./CustomSelect";
import { CLOUD_PROVIDER_OPTIONS, resolveProviderValue } from "../constants/cloudProviders";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";
import { MetricCard } from "./ui/MetricCard";
import { AccountProfileModal } from "./settings/AccountProfileModal";
import { AccountsSettingsContent } from "./settings/AccountsSettingsContent";
import { ConfirmActionModal } from "./settings/ConfirmActionModal";
import { CoreProviderCredentialFields } from "./settings/CoreProviderCredentialFields";
import { ExtendedProviderCredentialFields } from "./settings/ExtendedProviderCredentialFields";
import { ImportAccountsModal } from "./settings/ImportAccountsModal";
import { NotificationsSettingsContent } from "./settings/NotificationsSettingsContent";
import { PendingDeleteAwsModal } from "./settings/PendingDeleteAwsModal";
import { SettingsToast } from "./settings/SettingsToast";

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

interface ProxyProfile {
  id: string;
  name: string;
  protocol: string;
  host: string;
  port: number;
  auth_username?: string | null;
  auth_password?: string | null;
}

interface NotificationTestResult {
  ok: boolean;
  channel_name: string;
  channel_method: string;
  app_version?: string;
  proxy_mode: string;
  proxy_profile_id?: string | null;
  proxy_scheme?: string | null;
  proxy_url_masked?: string | null;
  stage: string;
  reason_code: string;
  message: string;
  http_status?: number | null;
  duration_ms: number;
  tested_at: number;
  trace?: string[];
}

interface ScanRule {
    id: string;
    name: string;
    description: string;
    enabled: boolean;
    params: string; // JSON string
}

interface CloudImportCandidate {
  id: string;
  provider: string;
  name: string;
  credentials: string;
  region?: string | null;
  source: string;
  import_kind: "aws_local" | "cloud_profile" | string;
}

interface ImportRenameMapping {
  provider: string;
  original: string;
  imported: string;
}

interface ConfirmDialogState {
  title: string;
  message: string;
  confirmLabel: string;
  confirmClassName?: string;
  action: () => Promise<void>;
}

type SettingsTab = "clouds" | "appearance" | "notifications" | "proxies" | "network";

interface SettingsProps {
  initialTab?: SettingsTab;
  pageTitle?: string;
  pageSubtitle?: string;
  showTabStrip?: boolean;
}

export function Settings({
  initialTab = "clouds",
  pageTitle = "Settings",
  pageSubtitle = "Configure your multi-cloud environment and preferences.",
  showTabStrip = true,
}: SettingsProps) {
  const PROXY_CHOICE_GLOBAL = "__global__";
  const PROXY_CHOICE_DIRECT = "__direct__";
  const ACCOUNT_NOTIFICATION_CHOICE_ALL = "__all_channels__";

  const [activeTab, setActiveTab] = useState<SettingsTab>(initialTab);
  const proxyProtocolOptions = [
      { value: "socks5h", label: "SOCKS5H (Recommended)" },
      { value: "socks5", label: "SOCKS5 (Local DNS)" },
      { value: "http", label: "HTTP" },
      { value: "https", label: "HTTPS" },
  ];
  const notificationChannelTriggerOptions = [
      { value: "scan_complete", label: "Scan Complete" },
      { value: "waste_only", label: "Only When Waste Found" },
  ];

  const [awsProfiles, setAwsProfiles] = useState<AwsProfile[]>([]);
  const [cloudProfiles, setCloudProfiles] = useState<CloudProfile[]>([]);
  const [notificationChannels, setNotificationChannels] = useState<NotificationChannel[]>([]);
  const [proxyProfiles, setProxyProfiles] = useState<ProxyProfile[]>([]);
  const [accountProxyAssignments, setAccountProxyAssignments] = useState<Record<string, string>>({});
  const [accountNotificationAssignments, setAccountNotificationAssignments] = useState<Record<string, string[]>>({});

  // Rule Config State
  const [accountRules, setAccountRules] = useState<ScanRule[]>([]);
  const [modalTab, setModalTab] = useState<"credentials" | "rules">("credentials");

  const [showAddModal, setShowAddModal] = useState(false);
  const [showImportModal, setShowImportModal] = useState(false);
  const [showNotifModal, setShowNotifModal] = useState(false);
  const [showProxyModal, setShowProxyModal] = useState(false);
  const [modalMode, setModalMode] = useState<"add" | "edit">("add");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingAwsName, setEditingAwsName] = useState<string | null>(null);
  const [editingAwsAuthType, setEditingAwsAuthType] = useState<string | null>(null);
  const [selectedProvider, setSelectedProvider] = useState("aws");

  const [selectedAccountProxy, setSelectedAccountProxy] = useState(PROXY_CHOICE_DIRECT);
  const [selectedAccountNotifications, setSelectedAccountNotifications] = useState<string[]>([ACCOUNT_NOTIFICATION_CHOICE_ALL]);
  const [proxyForm, setProxyForm] = useState({
      id: "",
      name: "",
      protocol: "socks5h",
      host: "",
      port: "1080",
      authUsername: "",
      authPassword: "",
  });

  // Forms
  const [awsForm, setAwsForm] = useState({ name: "default", key: "", secret: "", region: "us-east-1" });
  const [azureForm, setAzureForm] = useState({ name: "azure-prod", subscription_id: "", tenant_id: "", client_id: "", client_secret: "" });
  const [gcpForm, setGcpForm] = useState({ name: "gcp-prod", json_key: "" });
  const [aliForm, setAliForm] = useState({ name: "ali-prod", key: "", secret: "", region: "cn-hangzhou" });
  const [doForm, setDoForm] = useState({ name: "do-prod", token: "" });
  const [cfForm, setCfForm] = useState({ name: "cf-prod", token: "", account_id: "" });
  const [vultrForm, setVultrForm] = useState({ name: "vultr-prod", api_key: "" });
  const [linodeForm, setLinodeForm] = useState({ name: "linode-prod", token: "" });
  const [hetzForm, setHetzForm] = useState({ name: "hetzner-prod", token: "" });
  const [scwForm, setScwForm] = useState({ name: "scw-prod", token: "", zones: "fr-par-1,nl-ams-1,pl-waw-1" });
  const [exoForm, setExoForm] = useState({ name: "exo-prod", api_key: "", secret_key: "", endpoint: "" });
  const [lwForm, setLwForm] = useState({ name: "leaseweb-prod", token: "", endpoint: "" });
  const [upcForm, setUpcForm] = useState({ name: "upcloud-prod", username: "", password: "", endpoint: "" });
  const [gcoreForm, setGcoreForm] = useState({ name: "gcore-prod", token: "", endpoint: "" });
  const [contaboForm, setContaboForm] = useState({ name: "contabo-prod", token: "", client_id: "", client_secret: "", username: "", password: "", endpoint: "" });
  const [civoForm, setCivoForm] = useState({ name: "civo-prod", token: "", endpoint: "" });
  const [equinixForm, setEquinixForm] = useState({ name: "equinix-prod", token: "", endpoint: "", project_id: "" });
  const [rackspaceForm, setRackspaceForm] = useState({ name: "rackspace-prod", token: "", endpoint: "", project_id: "" });
  const [openstackForm, setOpenstackForm] = useState({ name: "openstack-prod", token: "", endpoint: "", project_id: "" });
  const [wasabiForm, setWasabiForm] = useState({ name: "wasabi-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [backblazeForm, setBackblazeForm] = useState({ name: "backblaze-prod", access_key: "", secret_key: "", region: "us-west-004", endpoint: "" });
  const [idriveForm, setIdriveForm] = useState({ name: "idrive-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [storjForm, setStorjForm] = useState({ name: "storj-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [dreamhostForm, setDreamhostForm] = useState({ name: "dreamhost-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [cloudianForm, setCloudianForm] = useState({ name: "cloudian-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [s3compatibleForm, setS3compatibleForm] = useState({ name: "s3-compatible-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [minioForm, setMinioForm] = useState({ name: "minio-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "http://localhost:9000" });
  const [cephForm, setCephForm] = useState({ name: "ceph-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "http://localhost:7480" });
  const [lyveForm, setLyveForm] = useState({ name: "lyve-prod", access_key: "", secret_key: "", region: "us-west-1", endpoint: "" });
  const [dellForm, setDellForm] = useState({ name: "dell-ecs-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [storagegridForm, setStoragegridForm] = useState({ name: "storagegrid-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [scalityForm, setScalityForm] = useState({ name: "scality-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [hcpForm, setHcpForm] = useState({ name: "hcp-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [qumuloForm, setQumuloForm] = useState({ name: "qumulo-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [nutanixForm, setNutanixForm] = useState({ name: "nutanix-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [flashbladeForm, setFlashbladeForm] = useState({ name: "flashblade-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [greenlakeForm, setGreenlakeForm] = useState({ name: "greenlake-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
  const [ionosForm, setIonosForm] = useState({ name: "ionos-prod", token: "", endpoint: "" });
  const [oracleForm, setOracleForm] = useState({ name: "oracle-prod", tenancy_id: "", user_id: "", fingerprint: "", private_key: "", region: "us-ashburn-1" });
  const [ibmForm, setIbmForm] = useState({ name: "ibm-prod", api_key: "", region: "us-south", cos_endpoint: "", cos_service_instance_id: "" });
  const [ovhForm, setOvhForm] = useState({ name: "ovh-prod", application_key: "", application_secret: "", consumer_key: "", endpoint: "eu", project_id: "" });
  const [huaweiForm, setHuaweiForm] = useState({ name: "hw-prod", access_key: "", secret_key: "", region: "cn-north-4", project_id: "" });
  const [tencentForm, setTencentForm] = useState({ name: "tc-prod", secret_id: "", secret_key: "", region: "ap-guangzhou" });
  const [volcForm, setVolcForm] = useState({ name: "volc-prod", access_key: "", secret_key: "", region: "cn-beijing" });
  const [baiduForm, setBaiduForm] = useState({ name: "baidu-prod", access_key: "", secret_key: "", region: "bj" });
  const [tianyiForm, setTianyiForm] = useState({ name: "ctyun-prod", access_key: "", secret_key: "", region: "cn-east-1" });

  // Notification Form
  const [notifForm, setNotifForm] = useState({
      id: "",
      name: "",
      method: "slack",
      url: "",
      token: "",
      chat_id: "",
      phone_id: "",
      to_phone: "",
      email_to: "",
      is_active: true,
      proxy_profile_id: PROXY_CHOICE_DIRECT,
      trigger_mode: "scan_complete",
      min_savings: "",
      min_findings: "",
  });

  const [theme, setTheme] = useState("dark");
  const [fontSize, setFontSize] = useState("medium");
  const [currency, setCurrency] = useState("USD");
  const [customRate, setCustomRate] = useState("");

  // Network
  const [proxyMode, setProxyMode] = useState("none");
  const [proxyUrl, setProxyUrl] = useState("");
  const [proxyProtocol, setProxyProtocol] = useState("socks5h");
  const [proxyHost, setProxyHost] = useState("");
  const [proxyPort, setProxyPort] = useState("1080");
  const [proxyAuthUsername, setProxyAuthUsername] = useState("");
  const [proxyAuthPassword, setProxyAuthPassword] = useState("");
  const [apiPort, setApiPort] = useState("9123");
  const [apiBindHost, setApiBindHost] = useState("0.0.0.0");
  const [apiLanEnabled, setApiLanEnabled] = useState(true);
  const [apiTlsEnabled, setApiTlsEnabled] = useState(true);
  const [apiAccessToken, setApiAccessToken] = useState("");

  // UI State
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testingAccountId, setTestingAccountId] = useState<string | null>(null);
  const [testingProxyDefault, setTestingProxyDefault] = useState(false);
  const [testingProxyProfileId, setTestingProxyProfileId] = useState<string | null>(null);
  const [testingNotifId, setTestingNotifId] = useState<string | null>(null);
  const [togglingNotifId, setTogglingNotifId] = useState<string | null>(null);
  const [showCredentialSecrets, setShowCredentialSecrets] = useState(false);
  const [showNotificationSecrets, setShowNotificationSecrets] = useState(false);
  const [showProxyDefaultPassword, setShowProxyDefaultPassword] = useState(false);
  const [showProxyProfilePassword, setShowProxyProfilePassword] = useState(false);
  const [pendingDeleteAwsName, setPendingDeleteAwsName] = useState<string | null>(null);
  const [confirmDialog, setConfirmDialog] = useState<ConfirmDialogState | null>(null);
  const [confirmingAction, setConfirmingAction] = useState(false);
  const [discoveringImports, setDiscoveringImports] = useState(false);
  const [importingAccounts, setImportingAccounts] = useState(false);
  const [importCandidates, setImportCandidates] = useState<CloudImportCandidate[]>([]);
  const [selectedImportIds, setSelectedImportIds] = useState<Record<string, boolean>>({});
  const [importInvalidItems, setImportInvalidItems] = useState<string[]>([]);
  const [importExecutionFailures, setImportExecutionFailures] = useState<string[]>([]);
  const [importRenameMappings, setImportRenameMappings] = useState<ImportRenameMapping[]>([]);
  const [importResultSummary, setImportResultSummary] = useState("");
  const [toast, setToast] = useState<{msg: string, type: 'success'|'error'} | null>(null);
  const [notifTestFeedback, setNotifTestFeedback] = useState<{
      type: "success" | "error";
      title: string;
      details: string;
  } | null>(null);

  useEffect(() => {
      setActiveTab(initialTab);
  }, [initialTab]);

  const accountCount = awsProfiles.length + cloudProfiles.length;
  const activeNotificationCount = notificationChannels.filter((channel) => channel.is_active).length;
  const proxyAssignedCount = Object.values(accountProxyAssignments).filter((value) => value && value !== PROXY_CHOICE_DIRECT).length;
  const customNotificationAssignments = Object.values(accountNotificationAssignments).filter((value) => Array.isArray(value) && value.length > 0 && !(value.length === 1 && value[0] === ACCOUNT_NOTIFICATION_CHOICE_ALL)).length;
  const toastTimerRef = useRef<number | null>(null);
  const importFileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadData();
    const savedTheme = localStorage.getItem("theme") || "dark";
    const savedSize = localStorage.getItem("fontSize") || "medium";
    setTheme(savedTheme);
    setFontSize(savedSize);
    applyTheme(savedTheme);
    applyFontSize(savedSize);
  }, []);

  useEffect(() => {
      if (showAddModal && modalMode === "add") {
          fetchProviderRules(selectedProvider);
      }
  }, [showAddModal, modalMode, selectedProvider]);

  useEffect(() => {
      return () => {
          if (toastTimerRef.current !== null) {
              window.clearTimeout(toastTimerRef.current);
              toastTimerRef.current = null;
          }
      };
  }, []);

  const normalizeStoredApiPort = (raw: string): string => {
      const parsed = Number(raw);
      if (!Number.isFinite(parsed)) return "9123";
      const asInt = Math.trunc(parsed);
      if (asInt < 1 || asInt > 65535) return "9123";
      return String(asInt);
  };

  const normalizeApiPortInput = (raw: string): string => {
      const digitsOnly = (raw || "").replace(/\D/g, "");
      if (!digitsOnly) return "";
      const parsed = Number(digitsOnly);
      if (!Number.isFinite(parsed)) return "";
      return String(Math.min(65535, Math.max(1, Math.trunc(parsed))));
  };

  async function loadData() {
    try {
      const aws = await invoke<AwsProfile[]>("list_aws_profiles");
      setAwsProfiles(aws);
      const clouds = await invoke<CloudProfile[]>("list_cloud_profiles");
      setCloudProfiles(clouds);

      const channels = await invoke<NotificationChannel[]>("list_notification_channels");
      setNotificationChannels(channels);
      const proxies = await invoke<ProxyProfile[]>("list_proxy_profiles");
      setProxyProfiles(proxies);
      const accountProxyRaw = await invoke<string>("get_setting", { key: "account_proxy_assignments" });
      if (accountProxyRaw?.trim()) {
          try {
              const parsed = JSON.parse(accountProxyRaw);
              if (parsed && typeof parsed === "object") {
                  setAccountProxyAssignments(parsed as Record<string, string>);
              } else {
                  setAccountProxyAssignments({});
              }
          } catch {
              setAccountProxyAssignments({});
          }
      } else {
          setAccountProxyAssignments({});
      }
      const accountNotificationRaw = await invoke<string>("get_setting", { key: "account_notification_assignments" });
      if (accountNotificationRaw?.trim()) {
          try {
              const parsed = JSON.parse(accountNotificationRaw) as Record<string, unknown>;
              if (parsed && typeof parsed === "object") {
                  const normalized: Record<string, string[]> = {};
                  for (const [accountId, rawValue] of Object.entries(parsed)) {
                      if (Array.isArray(rawValue)) {
                          normalized[accountId] = rawValue.map((item) => String(item || "").trim()).filter(Boolean);
                      } else if (typeof rawValue === "string") {
                          const trimmed = rawValue.trim();
                          normalized[accountId] = trimmed ? [trimmed] : [ACCOUNT_NOTIFICATION_CHOICE_ALL];
                      }
                  }
                  setAccountNotificationAssignments(normalized);
              } else {
                  setAccountNotificationAssignments({});
              }
          } catch {
              setAccountNotificationAssignments({});
          }
      } else {
          setAccountNotificationAssignments({});
      }

      const curr = await invoke<string>("get_setting", { key: "currency" });
      if (curr) setCurrency(curr);
      const rate = await invoke<string>("get_setting", { key: "currency_rate" });
      if (rate) setCustomRate(rate);

      const pMode = await invoke<string>("get_setting", { key: "proxy_mode" });
      const normalizedMode = (pMode || "none").trim().toLowerCase();
      setProxyMode(["custom", "none", "system"].includes(normalizedMode) ? normalizedMode : "none");
      const pUrl = await invoke<string>("get_setting", { key: "proxy_url" });
      if (pUrl) {
        setProxyUrl(pUrl);
        const parsedProxy = parseProxyUrl(pUrl);
        setProxyProtocol(parsedProxy.protocol);
        setProxyHost(parsedProxy.host);
        setProxyPort(parsedProxy.port);
        setProxyAuthUsername(parsedProxy.authUsername);
        setProxyAuthPassword(parsedProxy.authPassword);
      } else {
        setProxyAuthUsername("");
        setProxyAuthPassword("");
      }
      const port = await invoke<string>("get_setting", { key: "api_port" });
      if (port) setApiPort(normalizeStoredApiPort(port));
      let effectiveBindHost = "0.0.0.0";
      const bindHost = await invoke<string>("get_setting", { key: "api_bind_host" });
      if (bindHost) {
        const normalizedHost = bindHost.trim() || "0.0.0.0";
        effectiveBindHost = normalizedHost;
        setApiBindHost(normalizedHost);
        setApiLanEnabled(!(normalizedHost === "127.0.0.1" || normalizedHost === "localhost"));
      }
      const apiTls = await invoke<string>("get_setting", { key: "api_tls_enabled" });
      const loopbackOnlyHost = ["127.0.0.1", "localhost", "::1"].includes(
        effectiveBindHost.trim().toLowerCase(),
      );
      setApiTlsEnabled(parseBoolSetting(apiTls, !loopbackOnlyHost));
      const apiToken = await invoke<string>("get_setting", { key: "api_access_token" });
      if (apiToken) setApiAccessToken(apiToken);
    } catch (e) { console.error(e); }
  }

  function applyTheme(t: string) {
      if (t === 'dark') {
          document.documentElement.classList.add('dark');
          document.body.classList.add('dark');
      } else {
          document.documentElement.classList.remove('dark');
          document.body.classList.remove('dark');
      }
  }

  function applyFontSize(s: string) {
      const root = document.documentElement;
      if (s === 'small') root.style.fontSize = '16px';
      else if (s === 'large') root.style.fontSize = '20px';
      else root.style.fontSize = '18px';
  }

  function handleThemeChange(t: string) {
      setTheme(t);
      localStorage.setItem("theme", t);
      applyTheme(t);
  }

  function handleFontSizeChange(s: string) {
      setFontSize(s);
      localStorage.setItem("fontSize", s);
      applyFontSize(s);
  }

  const showToast = (
      msg: string,
      type: 'success'|'error' = 'success',
      autoHideMs?: number
  ) => {
      if (toastTimerRef.current !== null) {
          window.clearTimeout(toastTimerRef.current);
          toastTimerRef.current = null;
      }
      setToast({ msg, type });

      const hideAfterMs = autoHideMs ?? (type === "success" ? 3000 : 6000);
      if (hideAfterMs > 0) {
          toastTimerRef.current = window.setTimeout(() => {
              setToast(null);
              toastTimerRef.current = null;
          }, hideAfterMs);
      }
  };

  const dismissToast = () => {
      if (toastTimerRef.current !== null) {
          window.clearTimeout(toastTimerRef.current);
          toastTimerRef.current = null;
      }
      setToast(null);
  };

  const normalizeErrorMessage = (err: unknown): string => {
      if (typeof err === "string") return err;
      if (err instanceof Error && err.message) return err.message;
      try {
          const json = JSON.stringify(err);
          if (json && json !== "{}") return json;
      } catch {
          // Ignore serialization failure and fallback below.
      }
      return String(err);
  };

  const normalizeNotificationTestError = (err: unknown): string => {
      const raw = normalizeErrorMessage(err).replace(/^error:\s*/i, "").trim();
      if (!raw) {
          return "Unknown notification error. Please verify channel credentials and network connectivity.";
      }

      const lower = raw.toLowerCase();
      if (lower.includes("timed out") || lower.includes("timeout")) {
          return "Request timed out. Please verify network connectivity and retry.";
      }
      if (
          lower.includes("dns")
          || lower.includes("lookup")
          || lower.includes("name or service not known")
          || lower.includes("could not resolve")
      ) {
          return "Unable to resolve the endpoint hostname. Check the URL, DNS, and proxy settings.";
      }
      if (lower.includes("connection refused")) {
          return "Connection refused by the endpoint. Verify the URL, port, and firewall policy.";
      }
      if (lower.includes("invalid url") || lower.includes("relative url")) {
          return "Invalid endpoint URL format. Please provide a full HTTPS URL.";
      }
      if (lower.includes("http 401") || lower.includes("unauthorized")) {
          return "Authentication failed (HTTP 401). Verify tokens, bot keys, or webhook credentials.";
      }
      if (lower.includes("http 403") || lower.includes("forbidden")) {
          return "Permission denied (HTTP 403). Check channel permissions and token scopes.";
      }
      if (lower.includes("http 404")) {
          return "Endpoint not found (HTTP 404). Verify the webhook or API URL.";
      }
      if (lower.includes("http 429")) {
          return "Rate limit exceeded (HTTP 429). Retry after a short delay.";
      }
      if (lower.includes("http 5")) {
          return "Remote service is temporarily unavailable. Please retry later.";
      }
      if (/[^\x00-\x7F]/.test(raw)) {
          return "The provider returned a non-English error response. Verify channel credentials, destination settings, and network connectivity.";
      }

      return raw;
  };

  const openConfirmDialog = (dialog: ConfirmDialogState) => {
      setConfirmDialog(dialog);
  };

  const runConfirmDialogAction = async () => {
      if (!confirmDialog) return;
      setConfirmingAction(true);
      try {
          await confirmDialog.action();
          setConfirmDialog(null);
      } finally {
          setConfirmingAction(false);
      }
  };

  const generateApiToken = () => {
      if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
          return `cws_${crypto.randomUUID().replace(/-/g, "")}`;
      }
      return `cws_${Math.random().toString(36).slice(2)}${Date.now().toString(36)}`;
  };

  const parseBoolSetting = (raw: string | null | undefined, fallback: boolean) => {
      if (!raw) return fallback;
      const normalized = raw.trim().toLowerCase();
      if (["1", "true", "yes", "on", "enabled"].includes(normalized)) return true;
      if (["0", "false", "no", "off", "disabled"].includes(normalized)) return false;
      return fallback;
  };

  const providerDisplayName = (provider: string) => {
      const normalized = resolveProviderValue(provider);
      return (
          CLOUD_PROVIDER_OPTIONS.find((option) => option.value === normalized)?.label ||
          normalized.toUpperCase()
      );
  };

  const parseProxyUrl = (raw: string) => {
      const fallback = { protocol: "socks5h", host: "", port: "1080", authUsername: "", authPassword: "" };
      const trimmed = (raw || "").trim();
      if (!trimmed) return fallback;

      try {
          const candidate = trimmed.includes("://") ? trimmed : `http://${trimmed}`;
          const parsed = new URL(candidate);
          const rawProtocol = parsed.protocol.replace(":", "").toLowerCase();
          const protocol = ["http", "https", "socks5", "socks5h"].includes(rawProtocol)
              ? rawProtocol
              : fallback.protocol;
          const defaultPort = protocol === "https" ? "443" : (protocol.startsWith("socks5") ? "1080" : "80");
          let authUsername = "";
          let authPassword = "";
          try {
              authUsername = decodeURIComponent(parsed.username || "");
          } catch {
              authUsername = parsed.username || "";
          }
          try {
              authPassword = decodeURIComponent(parsed.password || "");
          } catch {
              authPassword = parsed.password || "";
          }
          return {
              protocol,
              host: parsed.hostname || "",
              port: parsed.port || defaultPort,
              authUsername,
              authPassword,
          };
      } catch {
          const withoutScheme = trimmed.replace(/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//, "");
          const authAndHost = withoutScheme.split("/");
          const authority = authAndHost[0] || "";
          let withoutAuth = authority;
          let authUsername = "";
          let authPassword = "";
          if (authority.includes("@")) {
              const atIndex = authority.lastIndexOf("@");
              const authPart = authority.slice(0, atIndex);
              const hostPart = authority.slice(atIndex + 1);
              withoutAuth = hostPart || authority;
              if (authPart) {
                  const colonIndex = authPart.indexOf(":");
                  const rawUser = colonIndex >= 0 ? authPart.slice(0, colonIndex) : authPart;
                  const rawPass = colonIndex >= 0 ? authPart.slice(colonIndex + 1) : "";
                  try {
                      authUsername = decodeURIComponent(rawUser || "");
                  } catch {
                      authUsername = rawUser || "";
                  }
                  try {
                      authPassword = decodeURIComponent(rawPass || "");
                  } catch {
                      authPassword = rawPass || "";
                  }
              }
          }
          const hostPort = withoutAuth.split("/")[0].trim();
          if (!hostPort) return fallback;

          if (hostPort.startsWith("[") && hostPort.includes("]")) {
              const closing = hostPort.indexOf("]");
              const host = hostPort.slice(1, closing);
              const port = hostPort.slice(closing + 1).replace(/^:/, "") || fallback.port;
              return { protocol: fallback.protocol, host, port, authUsername, authPassword };
          }

          const lastColon = hostPort.lastIndexOf(":");
          if (lastColon > 0) {
              return {
                  protocol: fallback.protocol,
                  host: hostPort.slice(0, lastColon),
                  port: hostPort.slice(lastColon + 1),
                  authUsername,
                  authPassword,
              };
          }
          return { ...fallback, host: hostPort, authUsername, authPassword };
      }
  };

  const encodeProxyUserInfo = (raw: string) =>
      Array.from(new TextEncoder().encode(raw))
          .map((byte) => {
              const ch = String.fromCharCode(byte);
              return /[A-Za-z0-9\-._~]/.test(ch) ? ch : `%${byte.toString(16).toUpperCase().padStart(2, "0")}`;
          })
          .join("");

  const composeProxyUrl = (
      protocol: string,
      host: string,
      port: string,
      authUsername?: string,
      authPassword?: string,
      includeAuth = false,
  ) => {
      const normalizedHost = host.trim();
      const wrappedHost = normalizedHost.includes(":")
          && !normalizedHost.startsWith("[")
          && !normalizedHost.endsWith("]")
          ? `[${normalizedHost}]`
          : normalizedHost;
      if (includeAuth && (authUsername || "").trim()) {
          const encodedUser = encodeProxyUserInfo((authUsername || "").trim());
          const normalizedPassword = (authPassword || "").trim();
          const userInfo = normalizedPassword
              ? `${encodedUser}:${encodeProxyUserInfo(normalizedPassword)}`
              : encodedUser;
          return `${protocol}://${userInfo}@${wrappedHost}:${port.trim()}`;
      }
      return `${protocol}://${wrappedHost}:${port.trim()}`;
  };

  const normalizeProxySelection = (value?: string | null) => {
      const trimmed = (value || "").trim();
      return trimmed || PROXY_CHOICE_GLOBAL;
  };
  const normalizeNotificationMethod = (value?: string | null) => {
      const normalized = (value || "").trim().toLowerCase();
      if (normalized === "custom") return "webhook";
      if (normalized === "email") return "email";
      if (normalized === "slack") return "slack";
      if (normalized === "teams") return "teams";
      if (normalized === "discord") return "discord";
      if (normalized === "telegram") return "telegram";
      if (normalized === "whatsapp") return "whatsapp";
      if (normalized === "webhook") return "webhook";
      return "webhook";
  };
  const parseEmailRecipients = (raw: string) => {
      const tokens = (raw || "").split(/[,;\n]/).map((item) => item.trim()).filter(Boolean);
      const seen = new Set<string>();
      const result: string[] = [];
      for (const token of tokens) {
          if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(token)) {
              continue;
          }
          const key = token.toLowerCase();
          if (seen.has(key)) continue;
          seen.add(key);
          result.push(token);
      }
      return result;
  };
  const normalizeAccountNotificationSelection = (
      value?: string[] | string | null,
      options?: { emptyAsAll?: boolean },
  ) => {
      const emptyAsAll = options?.emptyAsAll ?? true;
      const list = Array.isArray(value)
          ? value
          : (typeof value === "string" ? [value] : []);
      const normalized = Array.from(
          new Set(
              list
                  .map((item) => String(item || "").trim())
                  .filter(Boolean),
          ),
      );
      if (normalized.includes(ACCOUNT_NOTIFICATION_CHOICE_ALL)) {
          return [ACCOUNT_NOTIFICATION_CHOICE_ALL];
      }
      if (normalized.length === 0) {
          return emptyAsAll ? [ACCOUNT_NOTIFICATION_CHOICE_ALL] : [];
      }
      return normalized;
  };
  const isAllNotificationsSelected = (value?: string[] | string | null) =>
      normalizeAccountNotificationSelection(value, { emptyAsAll: false }).includes(ACCOUNT_NOTIFICATION_CHOICE_ALL);
  const toggleAccountNotificationSelection = (channelId: string, checked: boolean) => {
      const current = normalizeAccountNotificationSelection(selectedAccountNotifications, { emptyAsAll: false });
      if (channelId === ACCOUNT_NOTIFICATION_CHOICE_ALL) {
          setSelectedAccountNotifications(checked ? [ACCOUNT_NOTIFICATION_CHOICE_ALL] : []);
          return;
      }
      const base = current.filter((item) => item !== ACCOUNT_NOTIFICATION_CHOICE_ALL);
      const next = checked
          ? Array.from(new Set([...base, channelId]))
          : base.filter((item) => item !== channelId);
      setSelectedAccountNotifications(next);
  };

  const buildJsonProviderPayload = <T extends { name: string }>(form: T) => ({
      name: form.name,
      credentials: JSON.stringify(form),
  });

  const buildSelectedProviderPayload = (): {
      provider: string;
      name: string;
      credentials: string;
      region?: string;
  } => {
      if (selectedProvider === "aws") {
          const trimmedName = (awsForm.name || "").trim() || editingAwsName || "default";
          const trimmedKey = (awsForm.key || "").trim();
          const trimmedSecret = (awsForm.secret || "").trim();
          const trimmedRegion = (awsForm.region || "").trim() || "us-east-1";
          const useSsoReferenceFlow =
              modalMode === "edit"
              && editingAwsAuthType === "sso"
              && !trimmedKey
              && !trimmedSecret;

          if (trimmedKey && trimmedSecret) {
              return {
                  provider: selectedProvider,
                  name: trimmedName,
                  credentials: JSON.stringify({
                      key: trimmedKey,
                      secret: trimmedSecret,
                      auth_type: "access_key",
                  }),
                  region: trimmedRegion,
              };
          }

          if (useSsoReferenceFlow) {
              return {
                  provider: selectedProvider,
                  name: trimmedName,
                  credentials: JSON.stringify({
                      profile: trimmedName,
                      auth_type: "sso",
                  }),
                  region: trimmedRegion,
              };
          }

          throw new Error("AWS Access Key ID and Secret Access Key are required.");
      }

      const providerPayloadBuilders: Record<string, () => { name: string; credentials: string }> = {
          azure: () => ({
              name: azureForm.name,
              credentials: JSON.stringify({
                  subscription_id: azureForm.subscription_id,
                  tenant_id: azureForm.tenant_id,
                  client_id: azureForm.client_id,
                  client_secret: azureForm.client_secret,
              }),
          }),
          gcp: () => ({ name: gcpForm.name, credentials: gcpForm.json_key }),
          alibaba: () => ({
              name: aliForm.name,
              credentials: JSON.stringify({
                  access_key_id: aliForm.key,
                  access_key_secret: aliForm.secret,
                  region_id: aliForm.region,
              }),
          }),
          digitalocean: () => ({ name: doForm.name, credentials: doForm.token }),
          cloudflare: () => ({
              name: cfForm.name,
              credentials: JSON.stringify({
                  token: cfForm.token,
                  account_id: cfForm.account_id,
              }),
          }),
          vultr: () => ({ name: vultrForm.name, credentials: vultrForm.api_key }),
          linode: () => ({ name: linodeForm.name, credentials: linodeForm.token }),
          akamai: () => ({ name: linodeForm.name, credentials: linodeForm.token }),
          hetzner: () => ({ name: hetzForm.name, credentials: hetzForm.token }),
          scaleway: () => buildJsonProviderPayload(scwForm),
          exoscale: () => buildJsonProviderPayload(exoForm),
          leaseweb: () => buildJsonProviderPayload(lwForm),
          upcloud: () => buildJsonProviderPayload(upcForm),
          gcore: () => buildJsonProviderPayload(gcoreForm),
          contabo: () => buildJsonProviderPayload(contaboForm),
          civo: () => buildJsonProviderPayload(civoForm),
          equinix: () => buildJsonProviderPayload(equinixForm),
          rackspace: () => buildJsonProviderPayload(rackspaceForm),
          openstack: () => buildJsonProviderPayload(openstackForm),
          wasabi: () => buildJsonProviderPayload(wasabiForm),
          backblaze: () => buildJsonProviderPayload(backblazeForm),
          idrive: () => buildJsonProviderPayload(idriveForm),
          storj: () => buildJsonProviderPayload(storjForm),
          dreamhost: () => buildJsonProviderPayload(dreamhostForm),
          cloudian: () => buildJsonProviderPayload(cloudianForm),
          s3compatible: () => buildJsonProviderPayload(s3compatibleForm),
          minio: () => buildJsonProviderPayload(minioForm),
          ceph: () => buildJsonProviderPayload(cephForm),
          lyve: () => buildJsonProviderPayload(lyveForm),
          dell: () => buildJsonProviderPayload(dellForm),
          storagegrid: () => buildJsonProviderPayload(storagegridForm),
          scality: () => buildJsonProviderPayload(scalityForm),
          hcp: () => buildJsonProviderPayload(hcpForm),
          qumulo: () => buildJsonProviderPayload(qumuloForm),
          nutanix: () => buildJsonProviderPayload(nutanixForm),
          flashblade: () => buildJsonProviderPayload(flashbladeForm),
          greenlake: () => buildJsonProviderPayload(greenlakeForm),
          ionos: () => buildJsonProviderPayload(ionosForm),
          oracle: () => buildJsonProviderPayload(oracleForm),
          ibm: () => buildJsonProviderPayload(ibmForm),
          ovh: () => buildJsonProviderPayload(ovhForm),
          huawei: () => buildJsonProviderPayload(huaweiForm),
          tencent: () => buildJsonProviderPayload(tencentForm),
          volcengine: () => buildJsonProviderPayload(volcForm),
          baidu: () => buildJsonProviderPayload(baiduForm),
          tianyi: () => buildJsonProviderPayload(tianyiForm),
      };

      const builder = providerPayloadBuilders[selectedProvider];
      if (!builder) {
          throw new Error(`Unsupported provider: ${selectedProvider}`);
      }

      return {
          provider: selectedProvider,
          ...builder(),
      };
  };

  const proxySelectionOptions = [
      { value: PROXY_CHOICE_DIRECT, label: "No Proxy (Direct) (Recommended)" },
      { value: PROXY_CHOICE_GLOBAL, label: "Use Default Proxy Policy" },
      ...proxyProfiles.map((profile) => ({
          value: profile.id,
          label: `${profile.name} (${composeProxyUrl(profile.protocol, profile.host, String(profile.port))})`,
      })),
  ];
  const isDefaultPolicyNoProxy = proxyMode === "none";

  const resolveProxySelectionLabel = (value?: string | null) => {
      const normalized = normalizeProxySelection(value);
      if (normalized === PROXY_CHOICE_GLOBAL) return "Default Proxy Policy";
      if (normalized === PROXY_CHOICE_DIRECT) return "No Proxy (Direct)";
      const matched = proxyProfiles.find((p) => p.id === normalized);
      return matched
          ? `${matched.name} (${composeProxyUrl(matched.protocol, matched.host, String(matched.port))})`
          : "Default Proxy Policy";
  };
  const resolveAccountNotificationLabel = (value?: string[] | string | null) => {
      const normalized = normalizeAccountNotificationSelection(value);
      if (normalized.includes(ACCOUNT_NOTIFICATION_CHOICE_ALL)) return "All Active Channels";
      const labels = normalized
          .map((channelId) => {
              const channel = notificationChannels.find((item) => item.id === channelId);
              return channel
                  ? `${channel.name} (${normalizeNotificationMethod(channel.method)})`
                  : "";
          })
          .filter(Boolean);
      if (labels.length === 0) return "All Active Channels";
      if (labels.length <= 2) return labels.join(", ");
      return `${labels.slice(0, 2).join(", ")} +${labels.length - 2} more`;
  };

  const normalizeNotificationChannelTriggerSelection = (value?: string | null) => {
      const normalized = (value || "").trim().toLowerCase();
      if (normalized === "scan_complete") return "scan_complete";
      if (normalized === "waste_only" || normalized === "waste_found") return "waste_only";
      return "scan_complete";
  };

  const resolveNotificationChannelTriggerLabel = (value?: string | null) => {
      const normalized = normalizeNotificationChannelTriggerSelection(value);
      if (normalized === "scan_complete") return "Scan Complete";
      if (normalized === "waste_only") return "Only When Waste Found";
      return "Scan Complete";
  };

  const resolveNotificationChannelThresholdLabel = (channel: NotificationChannel) => {
      const parts: string[] = [];
      if (typeof channel.min_savings === "number" && Number.isFinite(channel.min_savings) && channel.min_savings > 0) {
          parts.push(`Savings >= ${channel.min_savings.toFixed(2)}`);
      }
      if (typeof channel.min_findings === "number" && Number.isFinite(channel.min_findings) && channel.min_findings > 0) {
          parts.push(`Findings >= ${Math.trunc(channel.min_findings)}`);
      }
      if (!parts.length) return "No thresholds";
      return parts.join(" | ");
  };

  const resolveNotificationChannelEffectiveTrigger = (value?: string | null) => {
      return normalizeNotificationChannelTriggerSelection(value);
  };

  const sanitizeNotificationMinSavingsInput = (raw: string): string => {
      const trimmed = raw.trim();
      if (trimmed === "") return "";
      const parsed = Number(trimmed);
      if (!Number.isFinite(parsed)) return "";
      if (parsed < 0) return "0";
      return raw;
  };

  const sanitizeNotificationMinFindingsInput = (raw: string): string => {
      const trimmed = raw.trim();
      if (trimmed === "") return "";
      const parsed = Number(trimmed);
      if (!Number.isFinite(parsed)) return "";
      if (parsed < 0) return "0";
      return String(Math.trunc(parsed));
  };

  async function persistAccountProxyAssignments(next: Record<string, string>) {
      await invoke("save_setting", {
          key: "account_proxy_assignments",
          value: JSON.stringify(next),
      });
      setAccountProxyAssignments(next);
  }
  async function persistAccountNotificationAssignments(next: Record<string, string[]>) {
      await invoke("save_setting", {
          key: "account_notification_assignments",
          value: JSON.stringify(next),
      });
      setAccountNotificationAssignments(next);
  }

  async function saveAppearance() {
      setSaving(true);
      try {
          await new Promise(r => setTimeout(r, 600));
          await invoke("save_setting", { key: "currency", value: currency });
          await invoke("save_setting", { key: "currency_rate", value: currency === "USD" ? "" : customRate });
          await emit("settings-changed");
          showToast("Display preferences saved!");
      } catch (e) {
          showToast("Failed to save: " + e, "error");
      } finally {
          setSaving(false);
      }
  }

  async function fetchAccountRules(accountId: string) {
      try {
          const rules = await invoke<ScanRule[]>("get_account_rules_config", { accountId });
          setAccountRules(rules);
      } catch (e) {
          console.error("Failed to fetch rules", e);
          setAccountRules([]);
      }
  }

  async function fetchProviderRules(provider: string) {
      try {
          const rules = await invoke<ScanRule[]>("get_provider_rules_config", { provider });
          setAccountRules(rules);
      } catch (e) {
          console.error("Failed to fetch provider rules", e);
          setAccountRules([]);
      }
  }

  async function toggleRule(ruleId: string, enabled: boolean) {
      const previousRules = accountRules;
      const targetRule = accountRules.find(r => r.id === ruleId);
      setAccountRules(prev => prev.map(r => r.id === ruleId ? { ...r, enabled } : r));

      const accountId = modalMode === "edit"
          ? (editingId || (editingAwsName ? `aws_local:${editingAwsName}` : null))
          : null;

      if (!accountId) {
          return;
      }

      try {
          await invoke("update_account_rule_config", {
              accountId,
              ruleId,
              enabled,
              params: targetRule?.params || "{}",
          });
      } catch (e) {
          setAccountRules(previousRules);
          showToast("Failed to update rule: " + e, "error");
      }
  }

  function openEditModal(p: CloudProfile) {
      setModalMode("edit");
      setEditingId(p.id);
      setEditingAwsAuthType(null);
      setSelectedProvider(p.provider);
      setShowCredentialSecrets(false);
      setSelectedAccountProxy(normalizeProxySelection(p.proxy_profile_id || PROXY_CHOICE_DIRECT));
      setSelectedAccountNotifications(
          normalizeAccountNotificationSelection(
              accountNotificationAssignments[p.id] || ACCOUNT_NOTIFICATION_CHOICE_ALL,
          ),
      );
      setModalTab("credentials"); // Reset tab
      fetchAccountRules(p.id); // Load rules

      if (p.provider === 'digitalocean') setDoForm({ name: p.name, token: p.credentials });
      else if (p.provider === 'linode' || p.provider === 'akamai') setLinodeForm({ name: p.name, token: p.credentials });
      else if (p.provider === 'hetzner') setHetzForm({ name: p.name, token: p.credentials });
      else if (p.provider === 'vultr') setVultrForm({ name: p.name, api_key: p.credentials });
      else if (p.provider === 'gcp') setGcpForm({ name: p.name, json_key: p.credentials });
      else {
          try {
              const c = JSON.parse(p.credentials);
              if (p.provider === 'azure') setAzureForm({ name: p.name, ...c });
              if (p.provider === 'alibaba') setAliForm({ name: p.name, key: c.access_key_id, secret: c.access_key_secret, region: c.region_id });
              if (p.provider === 'cloudflare') setCfForm({ name: p.name, ...c });
              if (p.provider === 'oracle') setOracleForm({ name: p.name, ...c });
              if (p.provider === 'ibm') setIbmForm({ name: p.name, api_key: c.api_key || '', region: c.region || 'us-south', cos_endpoint: c.cos_endpoint || '', cos_service_instance_id: c.cos_service_instance_id || '' });
              if (p.provider === 'ovh') setOvhForm({ name: p.name, ...c });
              if (p.provider === 'scaleway') setScwForm({ name: p.name, token: c.token || '', zones: c.zones || 'fr-par-1,nl-ams-1,pl-waw-1' });
              if (p.provider === 'exoscale') setExoForm({ name: p.name, api_key: c.api_key || '', secret_key: c.secret_key || '', endpoint: c.endpoint || '' });
              if (p.provider === 'leaseweb') setLwForm({ name: p.name, token: c.token || '', endpoint: c.endpoint || '' });
              if (p.provider === 'upcloud') setUpcForm({ name: p.name, username: c.username || '', password: c.password || '', endpoint: c.endpoint || '' });
              if (p.provider === 'gcore') setGcoreForm({ name: p.name, token: c.token || '', endpoint: c.endpoint || '' });
              if (p.provider === 'contabo') setContaboForm({ name: p.name, token: c.token || '', client_id: c.client_id || '', client_secret: c.client_secret || '', username: c.username || '', password: c.password || '', endpoint: c.endpoint || '' });
              if (p.provider === 'civo') setCivoForm({ name: p.name, token: c.token || '', endpoint: c.endpoint || '' });
              if (p.provider === 'equinix') setEquinixForm({ name: p.name, token: c.token || '', endpoint: c.endpoint || '', project_id: c.project_id || '' });
              if (p.provider === 'rackspace') setRackspaceForm({ name: p.name, token: c.token || '', endpoint: c.endpoint || '', project_id: c.project_id || '' });
              if (p.provider === 'openstack') setOpenstackForm({ name: p.name, token: c.token || '', endpoint: c.endpoint || '', project_id: c.project_id || '' });
              if (p.provider === 'wasabi') setWasabiForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'backblaze') setBackblazeForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-west-004', endpoint: c.endpoint || '' });
              if (p.provider === 'idrive') setIdriveForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'storj') setStorjForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'dreamhost') setDreamhostForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'cloudian') setCloudianForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 's3compatible') setS3compatibleForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'minio') setMinioForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || 'http://localhost:9000' });
              if (p.provider === 'ceph') setCephForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || 'http://localhost:7480' });
              if (p.provider === 'lyve') setLyveForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-west-1', endpoint: c.endpoint || '' });
              if (p.provider === 'dell') setDellForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'storagegrid') setStoragegridForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'scality') setScalityForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'hcp') setHcpForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'qumulo') setQumuloForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'nutanix') setNutanixForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'flashblade') setFlashbladeForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'greenlake') setGreenlakeForm({ name: p.name, access_key: c.access_key || '', secret_key: c.secret_key || '', region: c.region || 'us-east-1', endpoint: c.endpoint || '' });
              if (p.provider === 'ionos') setIonosForm({ name: p.name, token: c.token || '', endpoint: c.endpoint || '' });
              if (p.provider === 'huawei') setHuaweiForm({ name: p.name, ...c });
              if (p.provider === 'tencent') setTencentForm({ name: p.name, ...c });
              if (p.provider === 'volcengine') setVolcForm({ name: p.name, ...c });
              if (p.provider === 'baidu') setBaiduForm({ name: p.name, ...c });
              if (p.provider === 'tianyi') setTianyiForm({ name: p.name, ...c });
          } catch (e) {
              if (p.provider === 'scaleway') setScwForm({ name: p.name, token: p.credentials, zones: 'fr-par-1,nl-ams-1,pl-waw-1' });
              if (p.provider === 'exoscale') setExoForm({ name: p.name, api_key: '', secret_key: '', endpoint: '' });
              if (p.provider === 'leaseweb') setLwForm({ name: p.name, token: p.credentials, endpoint: '' });
              if (p.provider === 'upcloud') { const [username, password] = p.credentials.split(':'); setUpcForm({ name: p.name, username: username || '', password: password || '', endpoint: '' }); }
              if (p.provider === 'gcore') setGcoreForm({ name: p.name, token: p.credentials, endpoint: '' });
              if (p.provider === 'contabo') setContaboForm({ name: p.name, token: p.credentials, client_id: '', client_secret: '', username: '', password: '', endpoint: '' });
              if (p.provider === 'civo') setCivoForm({ name: p.name, token: p.credentials, endpoint: '' });
              if (p.provider === 'equinix') setEquinixForm({ name: p.name, token: p.credentials, endpoint: '', project_id: '' });
              if (p.provider === 'rackspace') setRackspaceForm({ name: p.name, token: p.credentials, endpoint: '', project_id: '' });
              if (p.provider === 'openstack') setOpenstackForm({ name: p.name, token: p.credentials, endpoint: '', project_id: '' });
              if (p.provider === 'wasabi') setWasabiForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'backblaze') setBackblazeForm({ name: p.name, access_key: '', secret_key: '', region: 'us-west-004', endpoint: '' });
              if (p.provider === 'idrive') setIdriveForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'storj') setStorjForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'dreamhost') setDreamhostForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'cloudian') setCloudianForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 's3compatible') setS3compatibleForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'minio') setMinioForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: 'http://localhost:9000' });
              if (p.provider === 'ceph') setCephForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: 'http://localhost:7480' });
              if (p.provider === 'lyve') setLyveForm({ name: p.name, access_key: '', secret_key: '', region: 'us-west-1', endpoint: '' });
              if (p.provider === 'dell') setDellForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'storagegrid') setStoragegridForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'scality') setScalityForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'hcp') setHcpForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'qumulo') setQumuloForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'nutanix') setNutanixForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'flashblade') setFlashbladeForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'greenlake') setGreenlakeForm({ name: p.name, access_key: '', secret_key: '', region: 'us-east-1', endpoint: '' });
              if (p.provider === 'ionos') setIonosForm({ name: p.name, token: p.credentials, endpoint: '' });
              if (p.provider === 'ibm') setIbmForm({ name: p.name, api_key: '', region: 'us-south', cos_endpoint: '', cos_service_instance_id: '' });
              console.error("Failed to parse credentials", e);
          }
      }
      setShowAddModal(true);
  }

  function openAwsEditModal(p: AwsProfile) {
      setModalMode("edit");
      setEditingId(null);
      setEditingAwsName(p.name);
      setEditingAwsAuthType((p.auth_type || "").trim().toLowerCase() || "access_key");
      setSelectedProvider("aws");
      setShowCredentialSecrets(false);
      setSelectedAccountProxy(
          normalizeProxySelection(accountProxyAssignments[`aws_local:${p.name}`] || PROXY_CHOICE_DIRECT),
      );
      setSelectedAccountNotifications(
          normalizeAccountNotificationSelection(
              accountNotificationAssignments[`aws_local:${p.name}`] || ACCOUNT_NOTIFICATION_CHOICE_ALL,
          ),
      );
      setModalTab("credentials");
      fetchAccountRules(`aws_local:${p.name}`);
      // Use stored key/secret if available
      setAwsForm({ name: p.name, key: p.key || "", secret: p.secret || "", region: p.region });
      setShowAddModal(true);
  }

  function handleDeleteAws(name: string) {
      setPendingDeleteAwsName(name);
  }

  async function confirmDeleteAws() {
      const name = pendingDeleteAwsName;
      if (!name) return;
      try {
          await invoke("delete_aws_profile", { name });
          const accountId = `aws_local:${name}`;
          if (accountProxyAssignments[accountId]) {
              const nextAssignments = { ...accountProxyAssignments };
              delete nextAssignments[accountId];
              await persistAccountProxyAssignments(nextAssignments);
          }
          if (accountNotificationAssignments[accountId]) {
              const nextNotificationAssignments: Record<string, string[]> = { ...accountNotificationAssignments };
              delete nextNotificationAssignments[accountId];
              await persistAccountNotificationAssignments(nextNotificationAssignments);
          }
          await loadData();
          showToast(`AWS profile removed: ${name}`, "success");
      } catch (e) {
          showToast(`Failed to remove AWS profile: ${normalizeErrorMessage(e)}`, "error");
      } finally {
          setPendingDeleteAwsName(null);
      }
  }

  // --- Notification Logic ---

  function openNotifEdit(c: NotificationChannel) {
      const conf = (() => {
          try {
              return JSON.parse(c.config);
          } catch {
              return {};
          }
      })();
      setShowNotificationSecrets(false);
      setNotifForm({
          id: c.id,
          name: c.name,
          method: normalizeNotificationMethod(c.method),
          url: conf.url || "",
          token: conf.token || "",
          chat_id: conf.chat_id || "",
          phone_id: conf.phone_id || "",
          to_phone: conf.to_phone || "",
          email_to: Array.isArray(conf.email_to)
              ? conf.email_to.join(", ")
              : (conf.email_to || conf.email || ""),
          is_active: c.is_active,
          proxy_profile_id: normalizeProxySelection(c.proxy_profile_id || conf.proxy_profile_id),
          trigger_mode: normalizeNotificationChannelTriggerSelection(c.trigger_mode),
          min_savings: typeof c.min_savings === "number" && Number.isFinite(c.min_savings) && c.min_savings > 0 ? String(c.min_savings) : "",
          min_findings: typeof c.min_findings === "number" && Number.isFinite(c.min_findings) && c.min_findings > 0 ? String(Math.trunc(c.min_findings)) : "",
      });
      setShowNotifModal(true);
  }

  async function handleSaveNotif() {
      const validationError = validateNotifForm();
      if (validationError) {
          showToast(validationError, "error");
          return;
      }
      setSaving(true);
      try {
          await invoke("save_notification_channel", {
              channel: buildNotificationChannelFromForm(),
          });

          showToast("Notification channel saved!");
          setShowNotifModal(false);
          setShowNotificationSecrets(false);
          loadData();
          setNotifForm({
              id: "",
              name: "",
              method: "slack",
              url: "",
              token: "",
              chat_id: "",
              phone_id: "",
              to_phone: "",
              email_to: "",
              is_active: true,
              proxy_profile_id: PROXY_CHOICE_DIRECT,
              trigger_mode: "scan_complete",
              min_savings: "",
              min_findings: "",
          });
      } catch(e) {
          showToast("Error saving channel: " + e, "error");
      } finally {
          setSaving(false);
      }
  }

  async function handleDeleteNotif(id: string) {
      openConfirmDialog({
          title: "Delete Notification Channel",
          message: "Delete this notification channel?",
          confirmLabel: "Delete",
          confirmClassName: "bg-rose-600 hover:bg-rose-700 text-white",
          action: async () => {
              try {
                  await invoke("delete_notification_channel", { id });
                  const nextNotificationAssignments: Record<string, string[]> = {};
                  let assignmentsChanged = false;
                  for (const [accountId, channelIds] of Object.entries(accountNotificationAssignments)) {
                      const original = normalizeAccountNotificationSelection(channelIds);
                      const filtered = original.filter((channelId) => channelId !== id);
                      if (filtered.length !== original.length) {
                          assignmentsChanged = true;
                      }
                      if (filtered.length > 0) {
                          nextNotificationAssignments[accountId] = filtered;
                      }
                  }
                  if (assignmentsChanged) {
                      await persistAccountNotificationAssignments(nextNotificationAssignments);
                  }
                  if (normalizeAccountNotificationSelection(selectedAccountNotifications).includes(id)) {
                      setSelectedAccountNotifications([ACCOUNT_NOTIFICATION_CHOICE_ALL]);
                  }
                  loadData();
                  showToast("Channel deleted");
              } catch (e) {
                  showToast("Error deleting: " + e, "error");
              }
          },
      });
  }

  async function handleToggleNotifActive(channel: NotificationChannel, isActive: boolean) {
      setTogglingNotifId(channel.id);
      try {
          const nextChannel: NotificationChannel = {
              ...channel,
              method: normalizeNotificationMethod(channel.method),
              is_active: isActive,
          };
          await invoke("save_notification_channel", { channel: nextChannel });
          setNotificationChannels((prev) =>
              prev.map((c) =>
                  c.id === channel.id
                      ? { ...c, method: normalizeNotificationMethod(c.method), is_active: isActive }
                      : c,
              ),
          );
          showToast(
              isActive
                  ? `Channel enabled: ${channel.name}`
                  : `Channel paused: ${channel.name}`,
          );
      } catch (e) {
          showToast("Failed to update channel status: " + e, "error");
      } finally {
          setTogglingNotifId(null);
      }
  }

  function buildNotificationChannelFromForm(): NotificationChannel {
      const config = JSON.stringify({
          url: notifForm.url,
          token: notifForm.token,
          chat_id: notifForm.chat_id,
          phone_id: notifForm.phone_id,
          to_phone: notifForm.to_phone,
          email_to: notifForm.email_to,
      });
      const generatedId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
          ? crypto.randomUUID()
          : `notif_${Date.now()}`;
      const normalizedProxyId = normalizeProxySelection(notifForm.proxy_profile_id);
      const normalizedTriggerMode = normalizeNotificationChannelTriggerSelection(notifForm.trigger_mode);
      const shouldApplyThresholds = normalizedTriggerMode === "waste_only";
      const parsedMinSavings = notifForm.min_savings.trim() === "" ? null : Number(notifForm.min_savings);
      const parsedMinFindings = notifForm.min_findings.trim() === "" ? null : Number(notifForm.min_findings);
      return {
          id: notifForm.id || generatedId,
          name: notifForm.name,
          method: normalizeNotificationMethod(notifForm.method),
          config,
          is_active: notifForm.is_active,
          proxy_profile_id: normalizedProxyId === PROXY_CHOICE_GLOBAL ? null : normalizedProxyId,
          trigger_mode: normalizedTriggerMode,
          min_savings: shouldApplyThresholds
              && parsedMinSavings !== null
              && Number.isFinite(parsedMinSavings)
              && parsedMinSavings > 0
              ? parsedMinSavings
              : null,
          min_findings: shouldApplyThresholds
              && parsedMinFindings !== null
              && Number.isFinite(parsedMinFindings)
              && parsedMinFindings > 0
              ? Math.trunc(parsedMinFindings)
              : null,
      };
  }

  function validateNotifForm(): string | null {
      if (!notifForm.name.trim()) return "Name is required";
      const method = normalizeNotificationMethod(notifForm.method);
      if (method === "telegram") {
          if (!notifForm.token.trim() || !notifForm.chat_id.trim()) {
              return "Telegram requires Bot Token and Chat ID";
          }
      } else if (method === "whatsapp") {
          if (!notifForm.token.trim() || !notifForm.phone_id.trim() || !notifForm.to_phone.trim()) {
              return "WhatsApp requires Access Token, Phone Number ID, and To Phone Number";
          }
      } else if (method === "email") {
          if (parseEmailRecipients(notifForm.email_to).length === 0) {
              return "Email channel requires at least one valid recipient.";
          }
      } else if (!notifForm.url.trim()) {
          return "Webhook URL is required";
      }
      const effectiveTriggerMode = resolveNotificationChannelEffectiveTrigger(notifForm.trigger_mode);
      if (effectiveTriggerMode === "waste_only") {
          if (notifForm.min_savings.trim() !== "") {
              const parsed = Number(notifForm.min_savings);
              if (!Number.isFinite(parsed) || parsed < 0) {
                  return "Min savings threshold must be a non-negative number.";
              }
          }
          if (notifForm.min_findings.trim() !== "") {
              const parsed = Number(notifForm.min_findings);
              if (!Number.isFinite(parsed) || parsed < 0 || !Number.isInteger(parsed)) {
                  return "Min findings threshold must be a non-negative integer.";
              }
          }
      }
      return null;
  }

  async function handleTestNotif(c: NotificationChannel) {
      setTestingNotifId(c.id);
      try {
          const result = await invoke<NotificationTestResult>("test_notification_channel", { channel: c });
          setNotifTestFeedback({
              type: result.ok ? "success" : "error",
              title: result.ok
                  ? `Notification test sent (${result.channel_name})`
                  : `Notification test failed (${result.channel_name})`,
              details: result.message,
          });
          if (result.ok) {
              showToast("Test notification sent!", "success");
          } else {
              showToast("Notification test failed. See details in Notifications.", "error", 0);
          }
      } catch (e) {
          setNotifTestFeedback({
              type: "error",
              title: `Notification test failed (${c.name})`,
              details: normalizeNotificationTestError(e),
          });
          showToast("Notification test failed. See details in Notifications.", "error", 0);
      } finally {
          setTestingNotifId(null);
      }
  }

  async function handleTestNotifFromModal() {
      const validationError = validateNotifForm();
      if (validationError) {
          showToast(validationError, "error");
          return;
      }

      setTestingNotifId("__modal__");
      try {
          const result = await invoke<NotificationTestResult>("test_notification_channel", {
              channel: buildNotificationChannelFromForm(),
          });
          setNotifTestFeedback({
              type: result.ok ? "success" : "error",
              title: result.ok
                  ? `Notification test sent (${result.channel_name})`
                  : `Notification test failed (${result.channel_name})`,
              details: result.message,
          });
          if (result.ok) {
              showToast("Test notification sent!", "success");
          } else {
              showToast("Notification test failed. See details in Notifications.", "error", 0);
          }
      } catch (e) {
          setNotifTestFeedback({
              type: "error",
              title: `Notification test failed (${notifForm.name || "unsaved channel"})`,
              details: normalizeNotificationTestError(e),
          });
          showToast("Notification test failed. See details in Notifications.", "error", 0);
      } finally {
          setTestingNotifId(null);
      }
  }

  function openProxyModal(profile?: ProxyProfile) {
      setShowProxyProfilePassword(false);
      if (profile) {
          setProxyForm({
              id: profile.id,
              name: profile.name,
              protocol: profile.protocol,
              host: profile.host,
              port: String(profile.port),
              authUsername: profile.auth_username || "",
              authPassword: profile.auth_password || "",
          });
      } else {
          setProxyForm({
              id: "",
              name: "",
              protocol: "socks5h",
              host: "",
              port: "1080",
              authUsername: "",
              authPassword: "",
          });
      }
      setShowProxyModal(true);
  }

  async function handleSaveProxyProfile() {
      const normalizedName = proxyForm.name.trim();
      const normalizedHost = proxyForm.host.trim();
      const normalizedAuthUsername = proxyForm.authUsername.trim();
      const normalizedAuthPassword = proxyForm.authPassword.trim();
      const parsedPort = Number(proxyForm.port);
      if (!normalizedName) {
          showToast("Proxy profile name is required.", "error");
          return;
      }
      if (!normalizedHost) {
          showToast("Proxy host cannot be empty.", "error");
          return;
      }
      if (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535) {
          showToast("Proxy port must be between 1 and 65535.", "error");
          return;
      }
      if (!normalizedAuthUsername && normalizedAuthPassword) {
          showToast("Proxy username is required when password is provided.", "error");
          return;
      }

      setSaving(true);
      try {
          await invoke("save_proxy_profile", {
              id: proxyForm.id || null,
              name: normalizedName,
              protocol: proxyForm.protocol,
              host: normalizedHost,
              port: parsedPort,
              authUsername: normalizedAuthUsername || null,
              authPassword: normalizedAuthPassword || null,
          });
          showToast("Proxy profile saved.");
          setShowProxyProfilePassword(false);
          setShowProxyModal(false);
          await loadData();
      } catch (e) {
          showToast(`Failed to save proxy profile: ${e}`, "error");
      } finally {
          setSaving(false);
      }
  }

  async function handleDeleteProxyProfile(id: string) {
      openConfirmDialog({
          title: "Delete Proxy Profile",
          message: "Delete this proxy profile?",
          confirmLabel: "Delete",
          confirmClassName: "bg-rose-600 hover:bg-rose-700 text-white",
          action: async () => {
              setSaving(true);
              try {
                  await invoke("delete_proxy_profile", { id });
                  const nextAssignments: Record<string, string> = {};
                  for (const [accountId, proxyId] of Object.entries(accountProxyAssignments)) {
                      if (proxyId !== id) {
                          nextAssignments[accountId] = proxyId;
                      }
                  }
                  if (Object.keys(nextAssignments).length !== Object.keys(accountProxyAssignments).length) {
                      await persistAccountProxyAssignments(nextAssignments);
                  }
                  if (normalizeProxySelection(selectedAccountProxy) === id) {
                      setSelectedAccountProxy(PROXY_CHOICE_DIRECT);
                  }
                  if (normalizeProxySelection(notifForm.proxy_profile_id) === id) {
                      setNotifForm((prev) => ({ ...prev, proxy_profile_id: PROXY_CHOICE_DIRECT }));
                  }
                  showToast("Proxy profile deleted.");
                  await loadData();
              } catch (e) {
                  showToast(`Failed to delete proxy profile: ${e}`, "error");
              } finally {
                  setSaving(false);
              }
          },
      });
  }

  async function saveProxyDefaults() {
      setSaving(true);
      try {
          await new Promise(r => setTimeout(r, 300));
          const normalizedProxyHost = (proxyHost || "").trim();
          const normalizedProxyPort = (proxyPort || "").trim();
          const normalizedAuthUsername = (proxyAuthUsername || "").trim();
          const normalizedAuthPassword = (proxyAuthPassword || "").trim();
          const parsedProxyPort = Number(normalizedProxyPort);
          const proxyUrlForSave = (normalizedProxyHost && Number.isInteger(parsedProxyPort) && parsedProxyPort >= 1 && parsedProxyPort <= 65535)
              ? composeProxyUrl(
                  proxyProtocol,
                  normalizedProxyHost,
                  String(parsedProxyPort),
                  normalizedAuthUsername,
                  normalizedAuthPassword,
                  true,
              )
              : proxyUrl;
          if (proxyMode === "custom") {
              if (!normalizedProxyHost) {
                  throw new Error("Proxy host cannot be empty in Custom Proxy mode.");
              }
              if (!Number.isInteger(parsedProxyPort) || parsedProxyPort < 1 || parsedProxyPort > 65535) {
                  throw new Error("Proxy port must be between 1 and 65535.");
              }
              if (!normalizedAuthUsername && normalizedAuthPassword) {
                  throw new Error("Proxy username is required when password is provided.");
              }
          }
          await invoke("save_setting", { key: "proxy_mode", value: proxyMode });
          await invoke("save_setting", { key: "proxy_url", value: proxyUrlForSave });
          await invoke("apply_proxy_settings");
          setProxyUrl(proxyUrlForSave);
          showToast("Default proxy policy saved.");
      } catch (e) {
          showToast("Failed: " + e, "error");
      } finally {
          setSaving(false);
      }
  }

  async function handleTestDefaultProxyPolicy() {
      if (isDefaultPolicyNoProxy) {
          showToast("Proxy testing is unavailable when No Proxy (Direct) is selected.", "error");
          return;
      }
      setTestingProxyDefault(true);
      try {
          let proxyUrlForTest: string | null = null;
          if (proxyMode === "custom") {
              const normalizedHost = (proxyHost || "").trim();
              const normalizedPort = (proxyPort || "").trim();
              const normalizedAuthUsername = (proxyAuthUsername || "").trim();
              const normalizedAuthPassword = (proxyAuthPassword || "").trim();
              const parsedPort = Number(normalizedPort);
              if (!normalizedHost) {
                  throw new Error("Proxy host cannot be empty in Custom Proxy mode.");
              }
              if (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535) {
                  throw new Error("Proxy port must be between 1 and 65535.");
              }
              if (!normalizedAuthUsername && normalizedAuthPassword) {
                  throw new Error("Proxy username is required when password is provided.");
              }
              proxyUrlForTest = composeProxyUrl(
                  proxyProtocol,
                  normalizedHost,
                  String(parsedPort),
                  normalizedAuthUsername,
                  normalizedAuthPassword,
                  true,
              );
          } else {
              const normalizedUrl = (proxyUrl || "").trim();
              proxyUrlForTest = normalizedUrl.length > 0 ? normalizedUrl : null;
          }

          const message = await invoke<string>("test_proxy_connection", {
              proxyMode: proxyMode,
              proxyUrl: proxyUrlForTest,
          });
          showToast(message, "success", 7000);
      } catch (e) {
          showToast(normalizeErrorMessage(e), "error", 12000);
      } finally {
          setTestingProxyDefault(false);
      }
  }

  async function handleTestNamedProxyProfile(profile: ProxyProfile) {
      setTestingProxyProfileId(profile.id);
      try {
          const proxyUrlForTest = composeProxyUrl(
              profile.protocol,
              profile.host,
              String(profile.port),
              profile.auth_username || "",
              profile.auth_password || "",
              true,
          );
          const message = await invoke<string>("test_proxy_connection", {
              proxyMode: "custom",
              proxyUrl: proxyUrlForTest,
          });
          showToast(`${profile.name}: ${message}`, "success", 7000);
      } catch (e) {
          showToast(`${profile.name}: ${normalizeErrorMessage(e)}`, "error", 12000);
      } finally {
          setTestingProxyProfileId(null);
      }
  }

  // --- End Notification Logic ---

  async function handleTestConnection() {
      setTesting(true);
      try {
        const { credentials, region } = buildSelectedProviderPayload();

        const normalizedProxyChoice = normalizeProxySelection(selectedAccountProxy);
        const res = await invoke<string>("test_connection", {
            provider: selectedProvider,
            credentials,
            region: region || undefined,
            silent: false,
            proxyProfileId: normalizedProxyChoice === PROXY_CHOICE_GLOBAL ? null : normalizedProxyChoice,
        });
        showToast(res, "success");
      } catch (e) {
        showToast("Connection Failed: " + e, "error", 12000);
      } finally {
        setTesting(false);
      }
  }

  async function handleQuickTestAwsProfile(profile: AwsProfile) {
      const accountId = `aws:${profile.name}`;
      setTestingAccountId(accountId);
      try {
          const key = (profile.key || "").trim();
          const secret = (profile.secret || "").trim();
          const authType = (profile.auth_type || "").trim().toLowerCase();

          const normalizedProxyChoice = normalizeProxySelection(
              accountProxyAssignments[`aws_local:${profile.name}`] || PROXY_CHOICE_DIRECT,
          );
          const awsCredentials = key && secret
              ? { key, secret, auth_type: "access_key" }
              : { profile: profile.name, auth_type: authType || "profile" };
          const res = await invoke<string>("test_connection", {
              provider: "aws",
              credentials: JSON.stringify(awsCredentials),
              region: (profile.region || "").trim() || "us-east-1",
              silent: false,
              proxyProfileId: normalizedProxyChoice === PROXY_CHOICE_GLOBAL ? null : normalizedProxyChoice,
          });
          showToast(`${profile.name}: ${res}`, "success");
      } catch (e) {
          showToast(`${profile.name}: Connection Failed: ${normalizeErrorMessage(e)}`, "error", 12000);
      } finally {
          setTestingAccountId(null);
      }
  }

  async function handleQuickTestCloudProfile(profile: CloudProfile) {
      const accountId = `cloud:${profile.id}`;
      setTestingAccountId(accountId);
      try {
          const normalizedProxyChoice = normalizeProxySelection(
              profile.proxy_profile_id || PROXY_CHOICE_DIRECT,
          );
          const res = await invoke<string>("test_connection", {
              provider: profile.provider,
              credentials: profile.credentials,
              silent: false,
              proxyProfileId: normalizedProxyChoice === PROXY_CHOICE_GLOBAL ? null : normalizedProxyChoice,
          });
          showToast(`${profile.name}: ${res}`, "success");
      } catch (e) {
          showToast(`${profile.name}: Connection Failed: ${normalizeErrorMessage(e)}`, "error", 12000);
      } finally {
          setTestingAccountId(null);
      }
  }

  async function handleSaveProfile() {
    try {
        if (selectedProvider === "aws") {
            const previousAwsAccountId =
                modalMode === "edit" && editingAwsName ? `aws_local:${editingAwsName}` : null;
            const trimmedKey = (awsForm.key || "").trim();
            const trimmedSecret = (awsForm.secret || "").trim();
            const trimmedRegion = (awsForm.region || "").trim() || "us-east-1";
            const useSsoReferenceFlow =
                modalMode === "edit"
                && editingAwsAuthType === "sso"
                && !trimmedKey
                && !trimmedSecret;

            if (modalMode === 'edit' && editingAwsName && editingAwsName !== awsForm.name) {
                await invoke("delete_aws_profile", { name: editingAwsName });
            }
            if (useSsoReferenceFlow) {
                await invoke("save_aws_profile_reference", {
                    name: awsForm.name,
                    region: trimmedRegion,
                });
            } else {
                if (!trimmedKey || !trimmedSecret) {
                    throw new Error("AWS Access Key ID and Secret Access Key are required for key-based profiles.");
                }
                await invoke("save_aws_profile", {
                    name: awsForm.name,
                    key: trimmedKey,
                    secret: trimmedSecret,
                    region: trimmedRegion,
                });
            }
            const awsAccountId = `aws_local:${awsForm.name}`;
            for (const rule of accountRules) {
                await invoke("update_account_rule_config", {
                    accountId: awsAccountId,
                    ruleId: rule.id,
                    enabled: rule.enabled,
                    params: rule.params,
                });
            }
            const normalizedProxyChoice = normalizeProxySelection(selectedAccountProxy);
            const nextAssignments = { ...accountProxyAssignments };
            if (previousAwsAccountId && previousAwsAccountId !== awsAccountId) {
                delete nextAssignments[previousAwsAccountId];
            }
            nextAssignments[awsAccountId] = normalizedProxyChoice;
            await persistAccountProxyAssignments(nextAssignments);
            const normalizedNotificationChoice = normalizeAccountNotificationSelection(selectedAccountNotifications);
            const nextNotificationAssignments: Record<string, string[]> = { ...accountNotificationAssignments };
            if (previousAwsAccountId && previousAwsAccountId !== awsAccountId) {
                delete nextNotificationAssignments[previousAwsAccountId];
            }
            nextNotificationAssignments[awsAccountId] = normalizedNotificationChoice;
            await persistAccountNotificationAssignments(nextNotificationAssignments);
            setShowAddModal(false);
            loadData();
            resetForms();
            showToast("Account saved successfully.", "success");
            return;
        }
        const { provider, credentials, name } = buildSelectedProviderPayload();

        let targetAccountId: string;
        const normalizedProxyChoice = normalizeProxySelection(selectedAccountProxy);
        const proxyProfileId =
            normalizedProxyChoice === PROXY_CHOICE_GLOBAL ? null : normalizedProxyChoice;
        if (modalMode === 'edit' && editingId) {
            await invoke("update_cloud_profile", {
                id: editingId,
                provider,
                name,
                credentials,
                proxyProfileId,
            });
            targetAccountId = editingId;
        } else {
            targetAccountId = await invoke<string>("save_cloud_profile", {
                provider,
                name,
                credentials,
                proxyProfileId,
            });
        }

        for (const rule of accountRules) {
            await invoke("update_account_rule_config", {
                accountId: targetAccountId,
                ruleId: rule.id,
                enabled: rule.enabled,
                params: rule.params
            });
        }
        const normalizedNotificationChoice = normalizeAccountNotificationSelection(selectedAccountNotifications);
        const nextNotificationAssignments: Record<string, string[]> = { ...accountNotificationAssignments };
        nextNotificationAssignments[targetAccountId] = normalizedNotificationChoice;
        await persistAccountNotificationAssignments(nextNotificationAssignments);

        setShowAddModal(false);
        loadData();
        resetForms();
        showToast("Account saved successfully.", "success");
    } catch (e) {
        showToast("Failed to save account: " + e, "error");
    }
  }

  function resetForms() {
    setAwsForm({ name: "default", key: "", secret: "", region: "us-east-1" });
    setAzureForm({ name: "azure-prod", subscription_id: "", tenant_id: "", client_id: "", client_secret: "" });
    setGcpForm({ name: "gcp-prod", json_key: "" });
    setAliForm({ name: "ali-prod", key: "", secret: "", region: "cn-hangzhou" });
    setDoForm({ name: "do-prod", token: "" });
    setCfForm({ name: "cf-prod", token: "", account_id: "" });
    setVultrForm({ name: "vultr-prod", api_key: "" });
    setLinodeForm({ name: "linode-prod", token: "" });
    setHetzForm({ name: "hetzner-prod", token: "" });
    setScwForm({ name: "scw-prod", token: "", zones: "fr-par-1,nl-ams-1,pl-waw-1" });
    setExoForm({ name: "exo-prod", api_key: "", secret_key: "", endpoint: "" });
    setLwForm({ name: "leaseweb-prod", token: "", endpoint: "" });
    setUpcForm({ name: "upcloud-prod", username: "", password: "", endpoint: "" });
    setGcoreForm({ name: "gcore-prod", token: "", endpoint: "" });
    setContaboForm({ name: "contabo-prod", token: "", client_id: "", client_secret: "", username: "", password: "", endpoint: "" });
    setCivoForm({ name: "civo-prod", token: "", endpoint: "" });
    setEquinixForm({ name: "equinix-prod", token: "", endpoint: "", project_id: "" });
    setRackspaceForm({ name: "rackspace-prod", token: "", endpoint: "", project_id: "" });
    setOpenstackForm({ name: "openstack-prod", token: "", endpoint: "", project_id: "" });
    setWasabiForm({ name: "wasabi-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setBackblazeForm({ name: "backblaze-prod", access_key: "", secret_key: "", region: "us-west-004", endpoint: "" });
    setIdriveForm({ name: "idrive-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setStorjForm({ name: "storj-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setDreamhostForm({ name: "dreamhost-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setCloudianForm({ name: "cloudian-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setS3compatibleForm({ name: "s3-compatible-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setMinioForm({ name: "minio-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "http://localhost:9000" });
    setCephForm({ name: "ceph-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "http://localhost:7480" });
    setLyveForm({ name: "lyve-prod", access_key: "", secret_key: "", region: "us-west-1", endpoint: "" });
    setDellForm({ name: "dell-ecs-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setStoragegridForm({ name: "storagegrid-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setScalityForm({ name: "scality-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setHcpForm({ name: "hcp-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setQumuloForm({ name: "qumulo-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setNutanixForm({ name: "nutanix-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setFlashbladeForm({ name: "flashblade-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setGreenlakeForm({ name: "greenlake-prod", access_key: "", secret_key: "", region: "us-east-1", endpoint: "" });
    setIonosForm({ name: "ionos-prod", token: "", endpoint: "" });
    setOracleForm({ name: "oracle-prod", tenancy_id: "", user_id: "", fingerprint: "", private_key: "", region: "us-ashburn-1" });
    setIbmForm({ name: "ibm-prod", api_key: "", region: "us-south", cos_endpoint: "", cos_service_instance_id: "" });
    setOvhForm({ name: "ovh-prod", application_key: "", application_secret: "", consumer_key: "", endpoint: "eu", project_id: "" });
    setHuaweiForm({ name: "hw-prod", access_key: "", secret_key: "", region: "cn-north-4", project_id: "" });
    setTencentForm({ name: "tc-prod", secret_id: "", secret_key: "", region: "ap-guangzhou" });
    setVolcForm({ name: "volc-prod", access_key: "", secret_key: "", region: "cn-beijing" });
    setBaiduForm({ name: "baidu-prod", access_key: "", secret_key: "", region: "bj" });
    setTianyiForm({ name: "ctyun-prod", access_key: "", secret_key: "", region: "cn-east-1" });
    setSelectedAccountProxy(PROXY_CHOICE_DIRECT);
    setSelectedAccountNotifications([ACCOUNT_NOTIFICATION_CHOICE_ALL]);
    setShowCredentialSecrets(false);

    setAccountRules([]);
    setModalTab("credentials");
    setModalMode("add"); setEditingId(null); setEditingAwsName(null); setEditingAwsAuthType(null); setTesting(false);
  }

  async function handleDeleteCloud(id: string) {
      openConfirmDialog({
          title: "Remove Account",
          message: "Remove this cloud account from local configuration?",
          confirmLabel: "Remove",
          confirmClassName: "bg-rose-600 hover:bg-rose-700 text-white",
          action: async () => {
              try {
                  await invoke("delete_cloud_profile", { id });
                  if (accountNotificationAssignments[id]) {
                      const nextNotificationAssignments = { ...accountNotificationAssignments };
                      delete nextNotificationAssignments[id];
                      await persistAccountNotificationAssignments(nextNotificationAssignments);
                  }
                  loadData();
                  showToast("Account removed.", "success");
              } catch (e) {
                  showToast(`Failed to remove account: ${normalizeErrorMessage(e)}`, "error");
              }
          },
      });
  }

  const normalizeImportCandidates = (raw: CloudImportCandidate[]) => {
      const seen = new Set<string>();
      const normalized: CloudImportCandidate[] = [];
      for (const item of raw) {
          const provider = resolveProviderValue((item.provider || "").trim().toLowerCase());
          const name = (item.name || "").trim();
          const credentials = typeof item.credentials === "string" ? item.credentials.trim() : "";
          if (!provider || !name || !credentials) continue;

          const source = (item.source || "local").trim();
          const importKind = item.import_kind === "aws_local" || provider === "aws"
              ? "aws_local"
              : "cloud_profile";
          const dedupeKey = `${importKind}|${provider}|${name.toLowerCase()}|${source.toLowerCase()}`;
          if (seen.has(dedupeKey)) continue;
          seen.add(dedupeKey);

          const baseId = item.id && item.id.trim().length > 0
              ? item.id.trim()
              : `${importKind}_${provider}_${name}_${source}`
                  .toLowerCase()
                  .replace(/[^a-z0-9]+/g, "_");
          normalized.push({
              id: baseId,
              provider,
              name,
              credentials,
              region: item.region || null,
              source,
              import_kind: importKind,
          });
      }
      return normalized;
  };

  const selectAllImportCandidates = (checked: boolean) => {
      const next: Record<string, boolean> = {};
      for (const candidate of importCandidates) {
          next[candidate.id] = checked;
      }
      setSelectedImportIds(next);
  };

  const openImportModal = async () => {
      setShowImportModal(true);
      setImportCandidates([]);
      setSelectedImportIds({});
      setImportInvalidItems([]);
      setImportExecutionFailures([]);
      setImportRenameMappings([]);
      setImportResultSummary("");
      await discoverImportableAccounts();
  };

  async function discoverImportableAccounts() {
      setDiscoveringImports(true);
      try {
          const candidates = await invoke<CloudImportCandidate[]>("discover_importable_cloud_accounts");
          const normalized = normalizeImportCandidates(candidates || []);
          setImportCandidates(normalized);
          setImportInvalidItems([]);
          setImportExecutionFailures([]);
          setImportRenameMappings([]);
          setImportResultSummary("");
          const defaults: Record<string, boolean> = {};
          for (const candidate of normalized) {
              defaults[candidate.id] = true;
          }
          setSelectedImportIds(defaults);
          showToast(
              normalized.length > 0
                  ? `Discovered ${normalized.length} importable account(s).`
                  : "No importable accounts found in local env/config.",
              "success",
          );
      } catch (e) {
          showToast(`Failed to discover importable accounts: ${normalizeErrorMessage(e)}`, "error");
      } finally {
          setDiscoveringImports(false);
      }
  }

  const parseStructuredImportEntry = (
      entry: Record<string, unknown>,
      sourceLabel: string,
      index: number,
  ): { candidate?: CloudImportCandidate; issue?: string } => {
      const providerRaw = String(entry.provider ?? entry.cloud ?? entry.platform ?? "").trim();
      if (!providerRaw) {
          return { issue: `${sourceLabel} #${index + 1}: missing provider` };
      }

      const provider = resolveProviderValue(providerRaw.toLowerCase());
      const importKindRaw = String(entry.import_kind ?? "").trim().toLowerCase();
      const importKind = importKindRaw === "aws_local" || provider === "aws"
          ? "aws_local"
          : "cloud_profile";
      const name = String(entry.name ?? entry.profile ?? entry.account_name ?? "").trim()
          || `${provider}-import-${index + 1}`;
      const region = String(entry.region ?? entry.aws_region ?? "").trim() || null;
      const authType = String(entry.auth_type ?? entry.auth ?? "").trim().toLowerCase();

      let credentials = "";
      if (typeof entry.credentials === "string") {
          credentials = entry.credentials.trim();
      } else if (entry.credentials && typeof entry.credentials === "object") {
          credentials = JSON.stringify(entry.credentials);
      }

      if (!credentials) {
          if (provider === "aws") {
              const key = String(entry.key ?? entry.access_key ?? entry.aws_access_key_id ?? "").trim();
              const secret = String(
                  entry.secret ?? entry.secret_key ?? entry.aws_secret_access_key ?? "",
              ).trim();
              if (key && secret) {
                  credentials = JSON.stringify({
                      key,
                      secret,
                      region: region || "us-east-1",
                      auth_type: "access_key",
                  });
              } else {
                  const profileName = String(entry.profile ?? entry.aws_profile ?? name).trim();
                  const isSso = authType === "sso" || Boolean(entry.sso_start_url) || Boolean(entry.sso_session);
                  if (profileName && isSso) {
                      credentials = JSON.stringify({
                          profile: profileName,
                          region: region || "us-east-1",
                          auth_type: "sso",
                      });
                  }
              }
          } else if (provider === "azure") {
              const subscription_id = String(entry.subscription_id ?? "").trim();
              const tenant_id = String(entry.tenant_id ?? "").trim();
              const client_id = String(entry.client_id ?? "").trim();
              const client_secret = String(entry.client_secret ?? "").trim();
              if (subscription_id && tenant_id && client_id && client_secret) {
                  credentials = JSON.stringify({
                      subscription_id,
                      tenant_id,
                      client_id,
                      client_secret,
                  });
              }
          } else if (provider === "alibaba") {
              const access_key_id = String(entry.access_key_id ?? entry.key ?? "").trim();
              const access_key_secret = String(entry.access_key_secret ?? entry.secret ?? "").trim();
              const region_id = region || "cn-hangzhou";
              if (access_key_id && access_key_secret) {
                  credentials = JSON.stringify({
                      access_key_id,
                      access_key_secret,
                      region_id,
                  });
              }
          } else if (provider === "tencent") {
              const secret_id = String(entry.secret_id ?? "").trim();
              const secret_key = String(entry.secret_key ?? "").trim();
              if (secret_id && secret_key) {
                  credentials = JSON.stringify({
                      secret_id,
                      secret_key,
                      region: region || "ap-guangzhou",
                  });
              }
          } else if (provider === "huawei") {
              const access_key = String(entry.access_key ?? "").trim();
              const secret_key = String(entry.secret_key ?? "").trim();
              if (access_key && secret_key) {
                  credentials = JSON.stringify({
                      access_key,
                      secret_key,
                      region: region || "cn-north-4",
                      project_id: String(entry.project_id ?? "").trim(),
                  });
              }
          } else if (provider === "digitalocean") {
              credentials = String(entry.token ?? entry.api_token ?? "").trim();
          } else if (provider === "cloudflare") {
              const token = String(entry.token ?? "").trim();
              const account_id = String(entry.account_id ?? "").trim();
              if (token && account_id) {
                  credentials = JSON.stringify({ token, account_id });
              }
          } else if (provider === "gcp") {
              const json_key = entry.json_key;
              if (typeof json_key === "string" && json_key.trim()) {
                  credentials = json_key.trim();
              } else if (json_key && typeof json_key === "object") {
                  credentials = JSON.stringify(json_key);
              }
          }
      }

      if (!credentials) {
          return { issue: `${sourceLabel} #${index + 1} (${name}): missing required credentials` };
      }

      const sourceKey = sourceLabel.toLowerCase().replace(/[^a-z0-9]+/g, "_");
      return {
          candidate: {
          id: `file_${sourceKey}_${index}_${provider}_${name}`
              .toLowerCase()
              .replace(/[^a-z0-9]+/g, "_"),
          provider,
          name,
          credentials,
          region,
          source: sourceLabel,
          import_kind: importKind,
          },
      };
  };

  const parseCsvLine = (line: string): string[] => {
      const cells: string[] = [];
      let current = "";
      let inQuotes = false;
      let i = 0;

      while (i < line.length) {
          const ch = line[i];
          if (ch === '"') {
              if (inQuotes && i + 1 < line.length && line[i + 1] === '"') {
                  current += '"';
                  i += 2;
                  continue;
              }
              inQuotes = !inQuotes;
              i += 1;
              continue;
          }
          if (ch === "," && !inQuotes) {
              cells.push(current.trim());
              current = "";
              i += 1;
              continue;
          }
          current += ch;
          i += 1;
      }
      cells.push(current.trim());
      return cells;
  };

  const parseCsvToEntries = (content: string): Record<string, unknown>[] => {
      const lines = content
          .split(/\r?\n/)
          .map((line) => line.trim())
          .filter((line) => line.length > 0);
      if (lines.length < 2) {
          return [];
      }
      const headers = parseCsvLine(lines[0]).map((header) =>
          header.toLowerCase().replace(/[^a-z0-9]+/g, "_").trim(),
      );
      const rows: Record<string, unknown>[] = [];
      for (let i = 1; i < lines.length; i += 1) {
          const values = parseCsvLine(lines[i]);
          const row: Record<string, unknown> = {};
          headers.forEach((header, idx) => {
              if (!header) return;
              row[header] = (values[idx] || "").trim();
          });
          rows.push(row);
      }
      return rows;
  };

  const encodeBase64Utf8 = (content: string) => {
      const bytes = new TextEncoder().encode(content);
      const chunkSize = 0x8000;
      let binary = "";
      for (let i = 0; i < bytes.length; i += chunkSize) {
          const chunk = bytes.subarray(i, i + chunkSize);
          binary += String.fromCharCode(...chunk);
      }
      return btoa(binary);
  };

  const downloadImportTemplate = async (format: "json" | "csv") => {
      const now = new Date();
      const stamp = `${now.getFullYear()}${String(now.getMonth() + 1).padStart(2, "0")}${String(
          now.getDate(),
      ).padStart(2, "0")}`;

      const jsonTemplate = [
          {
              provider: "aws",
              name: "cws-aws-key-demo",
              import_kind: "aws_local",
              region: "us-east-1",
              key: "AKIAxxxxxxxxxxxx",
              secret: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
          },
          {
              provider: "aws",
              name: "cws-aws-sso-demo",
              import_kind: "aws_local",
              region: "us-east-1",
              profile: "my-aws-sso-profile",
              auth_type: "sso",
          },
          {
              provider: "azure",
              name: "cws-azure-demo",
              import_kind: "cloud_profile",
              subscription_id: "00000000-0000-0000-0000-000000000000",
              tenant_id: "00000000-0000-0000-0000-000000000000",
              client_id: "00000000-0000-0000-0000-000000000000",
              client_secret: "replace-with-your-secret",
          },
          {
              provider: "cloudflare",
              name: "cws-cloudflare-demo",
              import_kind: "cloud_profile",
              token: "replace-with-your-token",
              account_id: "replace-with-your-account-id",
          },
      ];

      const csvTemplate = [
          "provider,name,import_kind,region,key,secret,profile,auth_type,subscription_id,tenant_id,client_id,client_secret,token,account_id,credentials",
          "aws,cws-aws-key-demo,aws_local,us-east-1,AKIAxxxxxxxxxxxx,xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx,,,,,,,,,",
          "aws,cws-aws-sso-demo,aws_local,us-east-1,,,my-aws-sso-profile,sso,,,,,,,",
          "azure,cws-azure-demo,cloud_profile,,,,,,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,replace-with-your-secret,,,",
          "cloudflare,cws-cloudflare-demo,cloud_profile,,,,,,,,,,,replace-with-your-token,replace-with-your-account-id,",
      ].join("\n");

      const content = format === "json"
          ? `${JSON.stringify(jsonTemplate, null, 2)}\n`
          : `${csvTemplate}\n`;
      const fileName =
          format === "json"
              ? `cws-account-import-template-${stamp}.json`
              : `cws-account-import-template-${stamp}.csv`;

      try {
          const base64Data = encodeBase64Utf8(content);
          const savedPath = await invoke<string>("save_export_file", {
              filename: fileName,
              base64Data,
              openAfterSave: false,
          });
          await invoke("reveal_export_file", { path: savedPath });
          showToast(`${format.toUpperCase()} template saved and revealed: ${fileName}`, "success");
          return;
      } catch (err) {
          console.warn("Template save via backend failed, falling back to browser download:", err);
      }

      const mimeType = format === "json" ? "application/json" : "text/csv";
      const blob = new Blob([content], { type: mimeType });
      const href = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = href;
      anchor.download = fileName;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      URL.revokeObjectURL(href);
      showToast(`${format.toUpperCase()} template downloaded: ${fileName}`, "success");
  };

  const applyParsedImportCandidates = (
      validCandidates: CloudImportCandidate[],
      issues: string[],
      sourceText: string,
  ) => {
      const normalized = normalizeImportCandidates(validCandidates);
      setImportCandidates(normalized);
      setImportInvalidItems(issues);
      setImportExecutionFailures([]);
      setImportRenameMappings([]);
      setImportResultSummary(
          `Loaded ${normalized.length} valid account(s) from ${sourceText}. Invalid rows: ${issues.length}.`,
      );
      if (normalized.length === 0) {
          showToast(`No valid accounts found in ${sourceText}.`, "error");
          setSelectedImportIds({});
          return;
      }
      const defaults: Record<string, boolean> = {};
      for (const candidate of normalized) {
          defaults[candidate.id] = true;
      }
      setSelectedImportIds(defaults);
      showToast(
          issues.length > 0
              ? `Loaded ${normalized.length} valid, ${issues.length} invalid from ${sourceText}.`
              : `Loaded ${normalized.length} account(s) from ${sourceText}.`,
          issues.length > 0 ? "error" : "success",
          9000,
      );
  };

  async function handleImportFileSelected(event: ChangeEvent<HTMLInputElement>) {
      const file = event.target.files?.[0];
      event.target.value = "";
      if (!file) return;

      try {
          const text = await file.text();
          const sourceLabel = `file:${file.name}`;
          const isCsv = /\.csv$/i.test(file.name) || file.type.includes("csv");
          const rows: unknown[] = isCsv
              ? parseCsvToEntries(text)
              : (() => {
                  const parsed = JSON.parse(text);
                  return Array.isArray(parsed) ? parsed : [parsed];
              })();

          if (rows.length === 0) {
              showToast(`No rows found in ${file.name}.`, "error");
              setImportCandidates([]);
              setSelectedImportIds({});
              setImportInvalidItems([`No usable rows found in ${sourceLabel}`]);
              setImportExecutionFailures([]);
              setImportRenameMappings([]);
              setImportResultSummary("");
              return;
          }

          const candidates: CloudImportCandidate[] = [];
          const issues: string[] = [];
          rows.forEach((row, index) => {
              if (!row || typeof row !== "object") {
                  issues.push(`${sourceLabel} #${index + 1}: invalid row format`);
                  return;
              }
              const { candidate, issue } = parseStructuredImportEntry(
                  row as Record<string, unknown>,
                  sourceLabel,
                  index,
              );
              if (candidate) {
                  candidates.push(candidate);
              } else if (issue) {
                  issues.push(issue);
              } else {
                  issues.push(`${sourceLabel} #${index + 1}: invalid row`);
              }
          });

          applyParsedImportCandidates(candidates, issues, file.name);
      } catch (e) {
          showToast(`Failed to parse import file: ${normalizeErrorMessage(e)}`, "error");
      }
  }

  const buildUniqueImportedName = (baseName: string, used: Set<string>) => {
      const base = (baseName || "imported-account").trim() || "imported-account";
      let candidate = base;
      let suffix = 2;
      while (used.has(candidate.toLowerCase())) {
          candidate = `${base}-${suffix}`;
          suffix += 1;
      }
      used.add(candidate.toLowerCase());
      return candidate;
  };

  async function handleImportSelectedAccounts() {
      const selected = importCandidates.filter((candidate) => selectedImportIds[candidate.id]);
      if (selected.length === 0) {
          showToast("Select at least one account to import.", "error");
          return;
      }

      setImportingAccounts(true);
      let successCount = 0;
      const failures: string[] = [];
      const renameMappings: ImportRenameMapping[] = [];
      const awsNames = new Set(awsProfiles.map((profile) => profile.name.toLowerCase()));
      const cloudNames = new Set(
          cloudProfiles.map((profile) => `${profile.provider.toLowerCase()}::${profile.name.toLowerCase()}`),
      );

      for (const candidate of selected) {
          try {
              const provider = resolveProviderValue(candidate.provider);
              if (candidate.import_kind === "aws_local" || provider === "aws") {
                  let parsed: Record<string, unknown> = {};
                  try {
                      parsed = JSON.parse(candidate.credentials);
                  } catch {
                      parsed = {};
                  }
                  const key = String(parsed.key ?? parsed.aws_access_key_id ?? "").trim();
                  const secret = String(parsed.secret ?? parsed.aws_secret_access_key ?? "").trim();
                  const region = String(
                      candidate.region ??
                      parsed.region ??
                      parsed.aws_region ??
                      "us-east-1",
                  ).trim() || "us-east-1";
                  const profileRef = String(parsed.profile ?? parsed.aws_profile ?? candidate.name).trim();
                  const authType = String(parsed.auth_type ?? "").trim().toLowerCase();
                  const isSso = authType === "sso";
                  if (key && secret) {
                      const importedName = buildUniqueImportedName(candidate.name, awsNames);
                      await invoke("save_aws_profile", { name: importedName, key, secret, region });
                      if (importedName !== candidate.name) {
                          renameMappings.push({
                              provider: "aws",
                              original: candidate.name,
                              imported: importedName,
                          });
                      }
                      successCount += 1;
                      continue;
                  }
                  if (isSso || profileRef) {
                      const originalName = profileRef || candidate.name;
                      const importedName = buildUniqueImportedName(originalName, awsNames);
                      await invoke("save_aws_profile_reference", { name: importedName, region });
                      if (importedName !== originalName) {
                          renameMappings.push({
                              provider: "aws",
                              original: originalName,
                              imported: importedName,
                          });
                      }
                      successCount += 1;
                      continue;
                  }
                  failures.push(`${candidate.name} (AWS credentials missing key/secret/profile)`);
                  continue;
              }

              const cloudBaseName = (candidate.name || `${provider}-imported`).trim() || `${provider}-imported`;
              let cloudName = cloudBaseName;
              let cloudKey = `${provider.toLowerCase()}::${cloudName.toLowerCase()}`;
              let suffix = 2;
              while (cloudNames.has(cloudKey)) {
                  cloudName = `${cloudBaseName}-${suffix}`;
                  cloudKey = `${provider.toLowerCase()}::${cloudName.toLowerCase()}`;
                  suffix += 1;
              }
              cloudNames.add(cloudKey);

              await invoke<string>("save_cloud_profile", {
                  provider,
                  name: cloudName,
                  credentials: candidate.credentials,
                  proxyProfileId: PROXY_CHOICE_DIRECT,
              });
              if (cloudName !== candidate.name) {
                  renameMappings.push({
                      provider,
                      original: candidate.name,
                      imported: cloudName,
                  });
              }
              successCount += 1;
          } catch (e) {
              failures.push(`${candidate.name} (${normalizeErrorMessage(e)})`);
          }
      }

      setImportingAccounts(false);
      await loadData();
      setImportExecutionFailures(failures);
      setImportRenameMappings(renameMappings);

      const totalFailures = importInvalidItems.length + failures.length;
      const summary = `Imported ${successCount} account(s). Failed ${totalFailures} item(s). Renamed ${renameMappings.length} account(s).`;
      setImportResultSummary(summary);

      if (totalFailures === 0) {
          showToast(`Imported ${successCount} account(s).`, "success");
          return;
      }

      const allIssues = [...importInvalidItems, ...failures];
      const failurePreview = allIssues.slice(0, 3).join(" | ");
      const remaining = allIssues.length > 3 ? ` | +${allIssues.length - 3} more` : "";
      showToast(
          `Imported ${successCount}, failed ${totalFailures}: ${failurePreview}${remaining}`,
          "error",
          14000,
      );
  }

  return (
      <PageShell maxWidthClassName="max-w-6xl" className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-300 transition-colors dark:text-slate-100">
        <PageHeader
          title={pageTitle}
          subtitle={pageSubtitle}
          icon={<SettingsIcon className="w-6 h-6" />}
        />

        {!showTabStrip ? activeTab === "clouds" ? (
          <div className="grid gap-4 mb-8 md:grid-cols-3">
            <MetricCard label="Connected Accounts" value={accountCount} hint={`${awsProfiles.length} AWS profiles + ${cloudProfiles.length} cloud accounts`} icon={<Cloud className="w-5 h-5" />} />
            <MetricCard label="Proxy Assignments" value={proxyAssignedCount} hint="Accounts using a named outbound proxy" icon={<Network className="w-5 h-5" />} />
            <MetricCard label="Custom Notification Routes" value={customNotificationAssignments} hint="Accounts overriding default notification delivery" icon={<Bell className="w-5 h-5" />} />
          </div>
        ) : activeTab === "notifications" ? (
          <div className="grid gap-4 mb-8 md:grid-cols-3">
            <MetricCard label="Channels" value={notificationChannels.length} hint="Total configured destinations" icon={<Bell className="w-5 h-5" />} />
            <MetricCard label="Active" value={activeNotificationCount} hint="Channels enabled for delivery" icon={<CheckCircle className="w-5 h-5" />} />
            <MetricCard label="Proxy-backed" value={notificationChannels.filter((channel) => channel.proxy_profile_id && channel.proxy_profile_id !== PROXY_CHOICE_DIRECT).length} hint="Channels using a named proxy profile" icon={<Network className="w-5 h-5" />} />
          </div>
        ) : activeTab === "proxies" ? (
          <div className="grid gap-4 mb-8 md:grid-cols-3">
            <MetricCard label="Proxy Profiles" value={proxyProfiles.length} hint="Named reusable proxy definitions" icon={<Network className="w-5 h-5" />} />
            <MetricCard label="Account Routes" value={proxyAssignedCount} hint="Accounts explicitly routed through a named proxy" icon={<Cloud className="w-5 h-5" />} />
            <MetricCard label="Notification Routes" value={notificationChannels.filter((channel) => channel.proxy_profile_id && channel.proxy_profile_id !== PROXY_CHOICE_DIRECT).length} hint="Delivery channels using a proxy profile" icon={<Bell className="w-5 h-5" />} />
          </div>
        ) : activeTab === "network" ? (
          <div className="grid gap-4 mb-8 md:grid-cols-3">
            <MetricCard label="Bind Host" value={apiBindHost} hint={apiLanEnabled ? "LAN access enabled" : "Local-only access"} icon={<Monitor className="w-5 h-5" />} />
            <MetricCard label="API Port" value={apiPort} hint={apiTlsEnabled ? "TLS enabled" : "TLS disabled"} icon={<SettingsIcon className="w-5 h-5" />} />
            <MetricCard label="Default Proxy Mode" value={proxyMode === "none" ? "Direct" : proxyMode} hint="Applies to outbound connectivity unless overridden" icon={<Network className="w-5 h-5" />} />
          </div>
        ) : activeTab === "appearance" ? (
          <div className="grid gap-4 mb-8 md:grid-cols-3">
            <MetricCard label="Theme" value={theme === "dark" ? "Dark" : "Light"} hint="Operator display mode" icon={theme === "dark" ? <Moon className="w-5 h-5" /> : <Sun className="w-5 h-5" />} />
            <MetricCard label="Font Size" value={fontSize} hint="Default UI density" icon={<SettingsIcon className="w-5 h-5" />} />
            <MetricCard label="Currency" value={currency} hint="Used for UI and exported reports" icon={<Hash className="w-5 h-5" />} />
          </div>
        ) : null : (
        <div className="flex space-x-6 border-b border-slate-200 dark:border-slate-700 mb-8 overflow-x-auto">
          <button onClick={() => setActiveTab("clouds")} className={`pb-3 px-2 text-lg font-medium border-b-2 transition-all flex items-center gap-2 whitespace-nowrap ${activeTab === "clouds" ? "border-indigo-600 text-indigo-600 dark:text-indigo-400 dark:border-indigo-400" : "border-transparent text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200"}`}>
            <Cloud className="w-5 h-5" /> Accounts
          </button>
          <button onClick={() => setActiveTab("notifications")} className={`pb-3 px-2 text-lg font-medium border-b-2 transition-all flex items-center gap-2 whitespace-nowrap ${activeTab === "notifications" ? "border-indigo-600 text-indigo-600 dark:text-indigo-400 dark:border-indigo-400" : "border-transparent text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200"}`}>
            <Bell className="w-5 h-5" /> Notifications
          </button>
          <button onClick={() => setActiveTab("proxies")} className={`pb-3 px-2 text-lg font-medium border-b-2 transition-all flex items-center gap-2 whitespace-nowrap ${activeTab === "proxies" ? "border-indigo-600 text-indigo-600 dark:text-indigo-400 dark:border-indigo-400" : "border-transparent text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200"}`}>
            <Network className="w-5 h-5" /> Proxy Profiles
          </button>
          <button onClick={() => setActiveTab("network")} className={`pb-3 px-2 text-lg font-medium border-b-2 transition-all flex items-center gap-2 whitespace-nowrap ${activeTab === "network" ? "border-indigo-600 text-indigo-600 dark:text-indigo-400 dark:border-indigo-400" : "border-transparent text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200"}`}>
            <Monitor className="w-5 h-5" /> Local API
          </button>
          <button onClick={() => setActiveTab("appearance")} className={`pb-3 px-2 text-lg font-medium border-b-2 transition-all flex items-center gap-2 whitespace-nowrap ${activeTab === "appearance" ? "border-indigo-600 text-indigo-600 dark:text-indigo-400 dark:border-indigo-400" : "border-transparent text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200"}`}>
            <Monitor className="w-5 h-5" /> Preferences
          </button>
        </div>
        )}

        {activeTab === "notifications" && (
          <NotificationsSettingsContent
            notificationChannels={notificationChannels}
            openAddChannel={() => {
              setNotifForm({
                id: "",
                name: "",
                method: "slack",
                url: "",
                token: "",
                chat_id: "",
                phone_id: "",
                to_phone: "",
                email_to: "",
                is_active: true,
                proxy_profile_id: PROXY_CHOICE_DIRECT,
                trigger_mode: "scan_complete",
                min_savings: "",
                min_findings: "",
              });
              setShowNotificationSecrets(false);
              setShowNotifModal(true);
            }}
            normalizeNotificationMethod={normalizeNotificationMethod}
            resolveProxySelectionLabel={resolveProxySelectionLabel}
            resolveNotificationChannelTriggerLabel={resolveNotificationChannelTriggerLabel}
            resolveNotificationChannelEffectiveTrigger={resolveNotificationChannelEffectiveTrigger}
            resolveNotificationChannelThresholdLabel={resolveNotificationChannelThresholdLabel}
            handleToggleNotifActive={handleToggleNotifActive}
            handleTestNotif={handleTestNotif}
            openNotifEdit={openNotifEdit}
            handleDeleteNotif={handleDeleteNotif}
            togglingNotifId={togglingNotifId}
            testingNotifId={testingNotifId}
            notifTestFeedback={notifTestFeedback}
            dismissNotifFeedback={() => setNotifTestFeedback(null)}
          />
        )}

        {/* Notification Modal */}
        {showNotifModal && (
            <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4">
                <div className="bg-white dark:bg-slate-800 rounded-xl p-6 w-full max-w-xl shadow-2xl animate-in zoom-in-95 border border-slate-200 dark:border-slate-700">
                    <div className="flex justify-between items-center mb-6">
                        <h3 className="text-2xl font-bold text-slate-900 dark:text-white">{notifForm.id ? 'Edit Channel' : 'Add Channel'}</h3>
                        <button onClick={() => {
                            setShowNotificationSecrets(false);
                            setShowNotifModal(false);
                        }} className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-200"><X className="w-5 h-5" /></button>
                    </div>
                    <div className="space-y-4">
                        <div>
                            <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Channel Name</label>
                            <input
                                type="text"
                                className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500"
                                value={notifForm.name}
                                onChange={e => setNotifForm({...notifForm, name: e.target.value})}
                                placeholder="e.g. DevOps Slack"
                            />
                        </div>
                        <div>
                            <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Method</label>
                            <div className="relative">
                                <CustomSelect
                                    value={normalizeNotificationMethod(notifForm.method)}
                                    onChange={val => setNotifForm({...notifForm, method: normalizeNotificationMethod(val)})}
                                    options={[
                                        { value: "slack", label: "Slack Webhook" },
                                        { value: "teams", label: "Microsoft Teams" },
                                        { value: "discord", label: "Discord Webhook" },
                                        { value: "telegram", label: "Telegram Bot" },
                                        { value: "whatsapp", label: "WhatsApp (Meta API)" },
                                        { value: "email", label: "Email Report" },
                                        { value: "webhook", label: "Generic Webhook" }
                                    ]}
                                />
                            </div>
                        </div>
                        <div>
                            <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Channel Status</label>
                            <button
                                type="button"
                                onClick={() => setNotifForm((prev) => ({ ...prev, is_active: !prev.is_active }))}
                                className="w-full flex items-center justify-between p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white"
                            >
                                <span className="text-sm font-semibold">
                                    {notifForm.is_active ? "Enabled (send during scans)" : "Paused (kept but not used)"}
                                </span>
                                <span
                                    className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                                        notifForm.is_active ? "bg-emerald-500" : "bg-slate-300 dark:bg-slate-600"
                                    }`}
                                >
                                    <span
                                        className={`inline-block h-5 w-5 transform rounded-full bg-white transition ${
                                            notifForm.is_active ? "translate-x-5" : "translate-x-1"
                                        }`}
                                    />
                                </span>
                            </button>
                        </div>
                        <div>
                            <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Trigger Policy</label>
                            <div className="relative">
                                <CustomSelect
                                    value={normalizeNotificationChannelTriggerSelection(notifForm.trigger_mode)}
                                    onChange={(value) => setNotifForm({ ...notifForm, trigger_mode: value })}
                                    options={notificationChannelTriggerOptions}
                                />
                            </div>
                            <p className="text-xs text-slate-500 dark:text-slate-400 mt-1">
                                Set this channel to notify after each scan or only when waste is detected.
                            </p>
                        </div>
                        {resolveNotificationChannelEffectiveTrigger(notifForm.trigger_mode) === "waste_only" && (
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                                <div>
                                    <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Min Savings (Optional)</label>
                                    <input
                                        type="number"
                                        min="0"
                                        step="0.01"
                                        className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500"
                                        value={notifForm.min_savings}
                                        onChange={e => setNotifForm({ ...notifForm, min_savings: sanitizeNotificationMinSavingsInput(e.target.value) })}
                                        placeholder="e.g. 20"
                                    />
                                </div>
                                <div>
                                    <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Min Findings (Optional)</label>
                                    <input
                                        type="number"
                                        min="0"
                                        step="1"
                                        className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500"
                                        value={notifForm.min_findings}
                                        onChange={e => setNotifForm({ ...notifForm, min_findings: sanitizeNotificationMinFindingsInput(e.target.value) })}
                                        placeholder="e.g. 3"
                                    />
                                </div>
                            </div>
                        )}
                        <div>
                            <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Proxy</label>
                            <div className="relative">
                                <CustomSelect
                                    value={normalizeProxySelection(notifForm.proxy_profile_id)}
                                    onChange={(value) => setNotifForm({ ...notifForm, proxy_profile_id: value })}
                                    options={proxySelectionOptions}
                                />
                            </div>
                            <p className="text-xs text-slate-500 dark:text-slate-400 mt-1">
                                Choose a dedicated proxy for this channel, or send directly.
                            </p>
                        </div>

                        {normalizeNotificationMethod(notifForm.method) === 'telegram' ? (
                            <>
                                <div>
                                    <div className="mb-1.5 flex items-center justify-between">
                                        <label className="text-base font-bold text-slate-400 uppercase">Bot Token</label>
                                        <button
                                            type="button"
                                            onClick={() => setShowNotificationSecrets((prev) => !prev)}
                                            className="inline-flex items-center gap-1 text-xs font-semibold text-slate-500 hover:text-slate-700 dark:text-slate-300 dark:hover:text-white"
                                            title={showNotificationSecrets ? "Hide secrets" : "Show secrets"}
                                        >
                                            {showNotificationSecrets ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                            {showNotificationSecrets ? "Hide" : "Show"}
                                        </button>
                                    </div>
                                    <input
                                        type={showNotificationSecrets ? "text" : "password"}
                                        className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 font-mono text-base"
                                        value={notifForm.token}
                                        onChange={e => setNotifForm({...notifForm, token: e.target.value})}
                                        placeholder="123456789:ABCDefgh..."
                                    />
                                </div>
                                <div>
                                    <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Chat ID</label>
                                    <input
                                        type="text"
                                        className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 font-mono text-base"
                                        value={notifForm.chat_id}
                                        onChange={e => setNotifForm({...notifForm, chat_id: e.target.value})}
                                        placeholder="-100123456789"
                                    />
                                </div>
                            </>
                        ) : normalizeNotificationMethod(notifForm.method) === 'whatsapp' ? (
                            <>
                                <div>
                                    <div className="mb-1.5 flex items-center justify-between">
                                        <label className="text-base font-bold text-slate-400 uppercase">Access Token</label>
                                        <button
                                            type="button"
                                            onClick={() => setShowNotificationSecrets((prev) => !prev)}
                                            className="inline-flex items-center gap-1 text-xs font-semibold text-slate-500 hover:text-slate-700 dark:text-slate-300 dark:hover:text-white"
                                            title={showNotificationSecrets ? "Hide secrets" : "Show secrets"}
                                        >
                                            {showNotificationSecrets ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                            {showNotificationSecrets ? "Hide" : "Show"}
                                        </button>
                                    </div>
                                    <input
                                        type={showNotificationSecrets ? "text" : "password"}
                                        className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 font-mono text-base"
                                        value={notifForm.token}
                                        onChange={e => setNotifForm({...notifForm, token: e.target.value})}
                                        placeholder="EAAG..."
                                    />
                                </div>
                                <div>
                                    <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Phone Number ID</label>
                                    <input
                                        type="text"
                                        className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 font-mono text-base"
                                        value={notifForm.phone_id}
                                        onChange={e => setNotifForm({...notifForm, phone_id: e.target.value})}
                                        placeholder="100609..."
                                    />
                                </div>
                                <div>
                                    <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">To Phone Number</label>
                                    <input
                                        type="text"
                                        className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 font-mono text-base"
                                        value={notifForm.to_phone}
                                        onChange={e => setNotifForm({...notifForm, to_phone: e.target.value})}
                                        placeholder="15551234567"
                                    />
                                </div>
                            </>
                        ) : normalizeNotificationMethod(notifForm.method) === "email" ? (
                            <div>
                                <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Email Recipients</label>
                                <textarea
                                    rows={4}
                                    className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 font-mono text-base"
                                    value={notifForm.email_to}
                                    onChange={e => setNotifForm({...notifForm, email_to: e.target.value})}
                                    placeholder="ops@example.com, finops@example.com"
                                />
                                <p className="text-xs text-slate-500 dark:text-slate-400 mt-1">
                                    Enter one or multiple recipients, separated by commas or new lines.
                                </p>
                            </div>
                        ) : (
                            <div>
                                <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Webhook URL</label>
                                <input
                                    type="url"
                                    className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 font-mono text-base"
                                    value={notifForm.url}
                                    onChange={e => setNotifForm({...notifForm, url: e.target.value})}
                                    placeholder="https://..."
                                />
                            </div>
                        )}
                    </div>
                    <div className="mt-6 flex flex-wrap justify-end gap-3">
                        <button
                            onClick={handleTestNotifFromModal}
                            disabled={saving || testingNotifId === "__modal__"}
                            className="w-full sm:w-auto flex items-center justify-center px-4 py-3 bg-slate-100 dark:bg-slate-700 text-slate-700 dark:text-slate-200 hover:bg-slate-200 dark:hover:bg-slate-600 rounded-lg font-medium transition-colors disabled:opacity-50"
                        >
                            {testingNotifId === "__modal__" ? <Loader2 className="w-5 h-5 mr-2 animate-spin" /> : <Bell className="w-5 h-5 mr-2" />}
                            {testingNotifId === "__modal__" ? "Testing..." : "Send Test Notification"}
                        </button>
                        <button onClick={() => {
                            setShowNotifModal(false);
                            setShowNotificationSecrets(false);
                            setNotifForm({
                                id: "",
                                name: "",
                                method: "slack",
                                url: "",
                                token: "",
                                chat_id: "",
                                phone_id: "",
                                to_phone: "",
                                email_to: "",
                                is_active: true,
                                proxy_profile_id: PROXY_CHOICE_DIRECT,
                                trigger_mode: "scan_complete",
                                min_savings: "",
                                min_findings: "",
                            });
                        }} className="w-full sm:w-auto px-4 py-3 text-slate-500 hover:text-slate-700 font-medium">Cancel</button>
                        <button onClick={handleSaveNotif} disabled={saving} className="w-full sm:w-auto bg-indigo-600 hover:bg-indigo-700 text-white px-6 py-3 rounded-lg font-bold flex items-center justify-center shadow-lg shadow-indigo-500/20 disabled:opacity-50">
                            {saving ? <Loader2 className="w-5 h-5 animate-spin" /> : "Save Channel"}
                        </button>
                    </div>
                    <div className="mt-3 p-3 bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 text-xs rounded-lg flex items-start">
                        <Monitor className="w-4 h-4 mr-2 flex-shrink-0 mt-0.5" />
                        <p>Testing notification does not save the channel. Save the channel after a successful test.</p>
                    </div>
                    {notifTestFeedback && (
                        <div className={`mt-3 p-3 rounded-lg border ${
                            notifTestFeedback.type === "success"
                                ? "bg-emerald-50 dark:bg-emerald-900/20 border-emerald-200 dark:border-emerald-800 text-emerald-800 dark:text-emerald-300"
                                : "bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800 text-red-800 dark:text-red-300"
                        }`}>
                            <div className="flex items-start justify-between gap-3">
                                <div className="flex items-start gap-2 min-w-0">
                                    {notifTestFeedback.type === "success"
                                        ? <CheckCircle className="w-4 h-4 mt-0.5 flex-shrink-0" />
                                        : <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0" />
                                    }
                                    <div className="min-w-0">
                                        <p className="text-sm font-semibold">{notifTestFeedback.title}</p>
                                        <p className="mt-1 text-xs whitespace-pre-wrap break-words">{notifTestFeedback.details}</p>
                                    </div>
                                </div>
                                <button
                                    onClick={() => setNotifTestFeedback(null)}
                                    className="text-[11px] font-semibold px-2 py-1 rounded border border-current/40 hover:bg-white/20 transition-colors"
                                >
                                    Dismiss
                                </button>
                            </div>
                        </div>
                    )}
                </div>
            </div>
        )}

        {activeTab === "proxies" && (
            <div className="bg-white dark:bg-slate-800 p-8 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm w-full animate-in fade-in slide-in-from-right-4 space-y-8">
                <div>
                    <h2 className="text-xl font-semibold text-slate-900 dark:text-white">Proxy Policies</h2>
                    <p className="text-base text-slate-500">Set the default proxy behavior and reusable named proxies.</p>
                </div>

                <div className="space-y-6 border border-slate-200 dark:border-slate-700 rounded-xl p-5 bg-slate-50 dark:bg-slate-900/30">
                    <h3 className="text-lg font-semibold text-slate-900 dark:text-white">Default Proxy Policy</h3>
                    <div>
                        <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Proxy Mode</label>
                        <div className="relative">
                            <CustomSelect
                                value={proxyMode}
                                onChange={setProxyMode}
                                options={[
                                    { value: "none", label: "No Proxy (Direct) (Recommended)" },
                                    { value: "system", label: "System Default" },
                                    { value: "custom", label: "Custom Proxy" },
                                ]}
                            />
                        </div>
                    </div>

                    {proxyMode === "custom" && (
                        <div className="animate-in fade-in slide-in-from-top-2">
                            <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">Proxy Protocol</label>
                            <CustomSelect
                                value={proxyProtocol}
                                onChange={setProxyProtocol}
                                options={proxyProtocolOptions}
                            />
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-3 mt-3">
                                <div>
                                    <label className="text-sm font-semibold text-slate-500 dark:text-slate-300 mb-1 block">Host</label>
                                    <input
                                        type="text"
                                        placeholder="192.168.1.4"
                                        className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
                                        value={proxyHost}
                                        onChange={e => setProxyHost(e.target.value)}
                                    />
                                </div>
                                <div>
                                    <label className="text-sm font-semibold text-slate-500 dark:text-slate-300 mb-1 block">Port</label>
                                    <input
                                        type="number"
                                        min="1"
                                        max="65535"
                                        placeholder="18794"
                                        className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
                                        value={proxyPort}
                                        onChange={e => setProxyPort(e.target.value)}
                                    />
                                </div>
                            </div>
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-3 mt-3">
                                <div>
                                    <label className="text-sm font-semibold text-slate-500 dark:text-slate-300 mb-1 block">Username (Optional)</label>
                                    <input
                                        type="text"
                                        placeholder="proxy-user"
                                        className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
                                        value={proxyAuthUsername}
                                        onChange={e => setProxyAuthUsername(e.target.value)}
                                    />
                                </div>
                                <div>
                                    <div className="mb-1 flex items-center justify-between">
                                        <label className="text-sm font-semibold text-slate-500 dark:text-slate-300">Password (Optional)</label>
                                        <button
                                            type="button"
                                            onClick={() => setShowProxyDefaultPassword((prev) => !prev)}
                                            className="inline-flex items-center gap-1 text-xs font-semibold text-slate-500 hover:text-slate-700 dark:text-slate-300 dark:hover:text-white"
                                            title={showProxyDefaultPassword ? "Hide password" : "Show password"}
                                        >
                                            {showProxyDefaultPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                            {showProxyDefaultPassword ? "Hide" : "Show"}
                                        </button>
                                    </div>
                                    <input
                                        type={showProxyDefaultPassword ? "text" : "password"}
                                        placeholder="proxy-password"
                                        className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
                                        value={proxyAuthPassword}
                                        onChange={e => setProxyAuthPassword(e.target.value)}
                                    />
                                </div>
                            </div>
                            <div className="mt-3 rounded-md border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800/70 p-3">
                                <p className="text-sm text-slate-700 dark:text-slate-300">
                                    <span className="font-semibold">SOCKS5H</span> resolves DNS on the proxy side and is safer for restricted networks.
                                </p>
                                <p className="text-sm text-slate-600 dark:text-slate-400 mt-1">
                                    <span className="font-semibold">SOCKS5</span> resolves DNS locally first. Use it only when local DNS routing is required.
                                </p>
                                {proxyHost.trim() && proxyPort.trim() && (
                                    <p className="text-sm text-slate-600 dark:text-slate-400 mt-2">
                                        Saved as: <code>{composeProxyUrl(proxyProtocol, proxyHost, proxyPort)}</code>
                                    </p>
                                )}
                                {proxyAuthUsername.trim() && (
                                    <p className="text-sm text-slate-600 dark:text-slate-400 mt-1">
                                        Authentication: enabled ({proxyAuthUsername.trim()})
                                    </p>
                                )}
                            </div>
                        </div>
                    )}

                    <div className="flex justify-end gap-3">
                        <button
                            onClick={handleTestDefaultProxyPolicy}
                            disabled={saving || testingProxyDefault || isDefaultPolicyNoProxy}
                            className="px-6 py-3 rounded-lg font-semibold border border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 disabled:opacity-70 disabled:cursor-not-allowed flex items-center transition-colors"
                        >
                            {testingProxyDefault ? <Loader2 className="w-5 h-5 mr-2 animate-spin" /> : <Network className="w-5 h-5 mr-2" />}
                            {testingProxyDefault ? "Testing..." : "Test Proxy"}
                        </button>
                        <button
                            onClick={saveProxyDefaults}
                            disabled={saving}
                            className="bg-indigo-600 hover:bg-indigo-700 text-white px-6 py-3 rounded-lg font-bold flex items-center transition-colors disabled:opacity-70 disabled:cursor-not-allowed shadow-lg shadow-indigo-500/20"
                        >
                            {saving ? <Loader2 className="w-5 h-5 mr-2 animate-spin" /> : <Save className="w-5 h-5 mr-2" />}
                            Save Default Policy
                        </button>
                    </div>
                    {isDefaultPolicyNoProxy && (
                        <p className="text-sm text-slate-500 dark:text-slate-400">
                            Test Proxy is disabled in No Proxy (Direct) mode.
                        </p>
                    )}
                </div>

                <div className="space-y-3">
                    <div className="flex justify-between items-center">
                        <h3 className="text-lg font-semibold text-slate-900 dark:text-white">Named Proxy Profiles</h3>
                        <button
                            onClick={() => openProxyModal()}
                            className="flex items-center px-4 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 font-medium transition-all text-sm shadow-sm"
                        >
                            <Plus className="w-4 h-4 mr-1" /> Add Proxy
                        </button>
                    </div>
                    {proxyProfiles.length === 0 && (
                        <div className="text-center py-8 text-slate-400 text-base bg-slate-50 dark:bg-slate-900/30 rounded-xl border border-dashed border-slate-200 dark:border-slate-700">
                            No named proxy profiles yet.
                        </div>
                    )}
                    {proxyProfiles.map((profile) => (
                        <div key={profile.id} className="p-4 border border-slate-200 dark:border-slate-700 rounded-xl bg-slate-50 dark:bg-slate-900/50 flex justify-between items-center">
                            <div>
                                <p className="font-bold text-slate-900 dark:text-white">{profile.name}</p>
                                <p className="text-sm text-slate-500 dark:text-slate-400 font-mono">
                                    {composeProxyUrl(profile.protocol, profile.host, String(profile.port))}
                                </p>
                                {profile.auth_username && (
                                    <p className="text-xs text-slate-500 dark:text-slate-400">
                                        Authentication enabled ({profile.auth_username})
                                    </p>
                                )}
                            </div>
                            <div className="flex gap-2">
                                <button
                                    onClick={() => handleTestNamedProxyProfile(profile)}
                                    disabled={testingProxyProfileId !== null}
                                    className="inline-flex items-center gap-1.5 px-3 py-2 text-sm rounded-lg border border-slate-200 dark:border-slate-600 text-slate-600 dark:text-slate-200 hover:bg-white dark:hover:bg-slate-700 disabled:opacity-50 transition-colors"
                                    title="Test Proxy"
                                >
                                    {testingProxyProfileId === profile.id
                                        ? <Loader2 className="w-4 h-4 animate-spin" />
                                        : <Network className="w-4 h-4" />
                                    }
                                    <span>{testingProxyProfileId === profile.id ? "Testing..." : "Test Proxy"}</span>
                                </button>
                                <button onClick={() => openProxyModal(profile)} className="p-3 text-slate-400 hover:text-indigo-600 transition-colors" title="Edit">
                                    <Pencil className="w-5 h-5" />
                                </button>
                                <button onClick={() => handleDeleteProxyProfile(profile.id)} className="p-3 text-slate-400 hover:text-red-600 transition-colors" title="Delete">
                                    <Trash2 className="w-5 h-5" />
                                </button>
                            </div>
                        </div>
                    ))}
                </div>
            </div>
        )}

        {showProxyModal && (
            <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4">
                <div className="bg-white dark:bg-slate-800 rounded-xl p-6 w-full max-w-md shadow-2xl animate-in zoom-in-95 border border-slate-200 dark:border-slate-700">
                    <div className="flex justify-between items-center mb-6">
                        <h3 className="text-2xl font-bold text-slate-900 dark:text-white">{proxyForm.id ? "Edit Proxy" : "Add Proxy"}</h3>
                        <button onClick={() => {
                            setShowProxyProfilePassword(false);
                            setShowProxyModal(false);
                        }} className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-200">
                            <X className="w-5 h-5" />
                        </button>
                    </div>
                    <div className="space-y-4">
                        <input
                            className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
                            value={proxyForm.name}
                            onChange={(e) => setProxyForm({ ...proxyForm, name: e.target.value })}
                            placeholder="Proxy Name (e.g. Office SOCKS)"
                        />
                        <CustomSelect
                            value={proxyForm.protocol}
                            onChange={(value) => setProxyForm({ ...proxyForm, protocol: value })}
                            options={proxyProtocolOptions}
                        />
                        <input
                            className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
                            value={proxyForm.host}
                            onChange={(e) => setProxyForm({ ...proxyForm, host: e.target.value })}
                            placeholder="Host"
                        />
                        <input
                            type="number"
                            min="1"
                            max="65535"
                            className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
                            value={proxyForm.port}
                            onChange={(e) => setProxyForm({ ...proxyForm, port: e.target.value })}
                            placeholder="Port"
                        />
                        <input
                            className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
                            value={proxyForm.authUsername}
                            onChange={(e) => setProxyForm({ ...proxyForm, authUsername: e.target.value })}
                            placeholder="Username (Optional)"
                        />
                        <div>
                            <div className="mb-1 flex items-center justify-between">
                                <label className="text-sm font-semibold text-slate-500 dark:text-slate-300">Password (Optional)</label>
                                <button
                                    type="button"
                                    onClick={() => setShowProxyProfilePassword((prev) => !prev)}
                                    className="inline-flex items-center gap-1 text-xs font-semibold text-slate-500 hover:text-slate-700 dark:text-slate-300 dark:hover:text-white"
                                    title={showProxyProfilePassword ? "Hide password" : "Show password"}
                                >
                                    {showProxyProfilePassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                                    {showProxyProfilePassword ? "Hide" : "Show"}
                                </button>
                            </div>
                            <input
                                type={showProxyProfilePassword ? "text" : "password"}
                                className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
                                value={proxyForm.authPassword}
                                onChange={(e) => setProxyForm({ ...proxyForm, authPassword: e.target.value })}
                                placeholder="Password (Optional)"
                            />
                        </div>
                    </div>
                    <div className="mt-6 flex justify-end gap-3">
                        <button onClick={() => {
                            setShowProxyProfilePassword(false);
                            setShowProxyModal(false);
                        }} className="px-4 py-3 text-slate-500 hover:text-slate-700 font-medium">Cancel</button>
                        <button onClick={handleSaveProxyProfile} disabled={saving} className="bg-indigo-600 hover:bg-indigo-700 text-white px-6 py-3 rounded-lg font-bold flex items-center shadow-lg shadow-indigo-500/20 disabled:opacity-50">
                            {saving ? <Loader2 className="w-5 h-5 animate-spin" /> : "Save Proxy"}
                        </button>
                    </div>
                </div>
            </div>
        )}

        {activeTab === "appearance" && (
            <div className="bg-white dark:bg-slate-800 p-8 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm w-full animate-in fade-in slide-in-from-right-4">
                <h2 className="text-xl font-semibold text-slate-900 dark:text-white mb-6">Display &amp; Localization</h2>
                <div className="space-y-8">
                    <div>
                        <label className="text-lg font-medium text-slate-700 dark:text-slate-300 mb-3 block">Theme</label>
                        <div className="grid grid-cols-2 gap-4">
                            <button onClick={() => handleThemeChange('light')} className={`flex flex-col items-center p-4 border-2 rounded-lg transition-all ${theme === 'light' ? 'border-indigo-600 bg-indigo-50 text-indigo-700 dark:text-indigo-300' : 'border-slate-200 dark:border-slate-700 hover:bg-slate-50 dark:hover:bg-slate-700'}`}><Sun className="w-6 h-6 mb-2" />Light</button>
                            <button onClick={() => handleThemeChange('dark')} className={`flex flex-col items-center p-4 border-2 rounded-lg transition-all ${theme === 'dark' ? 'border-indigo-600 bg-indigo-900/30 text-indigo-400' : 'border-slate-200 dark:border-slate-700 hover:bg-slate-50 dark:hover:bg-slate-700'}`}><Moon className="w-6 h-6 mb-2" />Dark</button>
                        </div>
                    </div>
                    <div>
                        <label className="text-lg font-medium text-slate-700 dark:text-slate-300 mb-1 block">Display Currency (Reports &amp; UI)</label>
                        <p className="text-sm text-slate-500 dark:text-slate-400 mb-3">
                            This changes cost display in dashboards and exported reports only. It does not change your real cloud billing currency.
                        </p>
                        <div className="space-y-4 bg-slate-50 dark:bg-slate-900/50 p-4 rounded-xl border border-slate-100 dark:border-slate-700">
                            <div className="relative">
                                <CustomSelect
                                    value={currency}
                                    onChange={(val) => { setCurrency(val); setCustomRate(""); }}
                                    options={[
                                        { value: "USD", label: "USD ($)" },
                                        { value: "EUR", label: "EUR (€)" },
                                        { value: "GBP", label: "GBP (£)" },
                                        { value: "CNY", label: "CNY (¥)" },
                                        { value: "JPY", label: "JPY (¥)" }
                                    ]}
                                />
                            </div>
                            {currency !== "USD" && (
                                <div>
                                    <label className="text-base text-slate-500 dark:text-slate-400 font-bold uppercase mb-1 block">Custom Rate (Optional)</label>
                                    <input type="number" step="0.01" className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500" value={customRate} onChange={e => setCustomRate(e.target.value)} placeholder="1 USD = ?" />
                                </div>
                            )}
                        </div>
                    </div>
                    <div>
                        <label className="text-lg font-medium text-slate-700 dark:text-slate-300 mb-3 block">Font Size</label>
                        <div className="flex gap-4">
                            <button onClick={() => handleFontSizeChange('small')} className={`flex-1 p-3 border rounded-lg text-lg transition-all ${fontSize === 'small' ? 'border-indigo-600 bg-indigo-50 text-indigo-700 dark:text-indigo-300 dark:bg-indigo-900/30 dark:text-indigo-400' : 'border-slate-200 dark:border-slate-700 hover:bg-slate-50 dark:hover:bg-slate-700'}`}>Small (16px)</button>
                            <button onClick={() => handleFontSizeChange('medium')} className={`flex-1 p-3 border rounded-lg text-lg transition-all ${fontSize === 'medium' ? 'border-indigo-600 bg-indigo-50 text-indigo-700 dark:text-indigo-300 dark:bg-indigo-900/30 dark:text-indigo-400' : 'border-slate-200 dark:border-slate-700 hover:bg-slate-50 dark:hover:bg-slate-700'}`}>Medium (18px)</button>
                            <button onClick={() => handleFontSizeChange('large')} className={`flex-1 p-3 border rounded-lg text-xl transition-all ${fontSize === 'large' ? 'border-indigo-600 bg-indigo-50 text-indigo-700 dark:text-indigo-300 dark:bg-indigo-900/30 dark:text-indigo-400' : 'border-slate-200 dark:border-slate-700 hover:bg-slate-50 dark:hover:bg-slate-700'}`}>Large (20px)</button>
                        </div>
                    </div>
                    <div className="flex justify-end pt-4">
                        <button
                            onClick={saveAppearance}
                            disabled={saving}
                            className="bg-indigo-600 hover:bg-indigo-700 text-white px-6 py-3 rounded-lg font-bold flex items-center transition-colors disabled:opacity-70 disabled:cursor-not-allowed shadow-lg shadow-indigo-500/20"
                        >
                            {saving ? <Loader2 className="w-5 h-5 mr-2 animate-spin" /> : <Save className="w-5 h-5 mr-2" />}
                            {saving ? "Saving..." : "Save Preferences"}
                        </button>
                    </div>
                </div>
            </div>
        )}

        {activeTab === "network" && (
            <div className="bg-white dark:bg-slate-800 p-8 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm w-full animate-in fade-in slide-in-from-right-4">
                <h2 className="text-xl font-semibold text-slate-900 dark:text-white mb-2">Local API Configuration</h2>
                <p className="text-base text-slate-500 mb-6">Configure host binding, port, TLS, and access token for the local API.</p>
                <div className="space-y-6">
                    <div>
                        <div className="w-full p-4 rounded-lg border border-slate-200 bg-slate-50 dark:border-slate-600 dark:bg-slate-700/60">
                            <div className="flex items-start justify-between gap-4">
                                <div>
                                    <label className="text-base font-semibold text-slate-900 dark:text-white block">Allow LAN API Access</label>
                                    <p className="text-sm text-slate-500 dark:text-slate-400 mt-1">
                                        {apiLanEnabled
                                            ? "Other devices on your network can call this API with a valid bearer token."
                                            : "Only this machine can call this API. This is the safer default when LAN access is not needed."}
                                    </p>
                                    <p className="text-xs text-slate-500 dark:text-slate-400 mt-2">
                                        Bind Host: <code>{apiLanEnabled ? "0.0.0.0" : "127.0.0.1"}</code>
                                    </p>
                                </div>
                                <button
                                    type="button"
                                    role="switch"
                                    aria-checked={apiLanEnabled}
                                    onClick={() => {
                                        const nextEnabled = !apiLanEnabled;
                                        setApiLanEnabled(nextEnabled);
                                        setApiBindHost(nextEnabled ? "0.0.0.0" : "127.0.0.1");
                                    }}
                                    className={`relative inline-flex h-7 w-12 shrink-0 items-center rounded-full border transition-colors ${
                                        apiLanEnabled
                                            ? "bg-emerald-500 border-emerald-500"
                                            : "bg-slate-300 border-slate-300 dark:bg-slate-600 dark:border-slate-600"
                                    }`}
                                >
                                    <span
                                        className={`inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform ${
                                            apiLanEnabled ? "translate-x-6" : "translate-x-1"
                                        }`}
                                    />
                                </button>
                            </div>
                        </div>
                    </div>

                    <div>
                        <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">API Listen Host</label>
                        <input
                            type="text"
                            placeholder="0.0.0.0"
                            className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
                            value={apiBindHost}
                            onChange={e => {
                                const nextHost = e.target.value;
                                setApiBindHost(nextHost);
                                const normalizedHost = nextHost.trim().toLowerCase();
                                if (normalizedHost === "127.0.0.1" || normalizedHost === "localhost") {
                                    setApiLanEnabled(false);
                                } else if (normalizedHost.length > 0) {
                                    setApiLanEnabled(true);
                                }
                            }}
                        />
                        <p className="text-base text-slate-500 mt-2">Advanced override. Typical values are <code>0.0.0.0</code> (LAN) or <code>127.0.0.1</code> (local-only).</p>
                    </div>

                    <div>
                        <label className="text-base font-bold text-slate-400 uppercase mb-1.5 block">API Listen Port</label>
                        <input
                            type="text"
                            inputMode="numeric"
                            pattern="[0-9]*"
                            className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400"
                            value={apiPort}
                            onChange={e => setApiPort(normalizeApiPortInput(e.target.value))}
                            onBlur={() => setApiPort((prev) => normalizeStoredApiPort(prev))}
                        />
                        <p className="text-base text-slate-500 mt-2">Example endpoint: <code>{apiTlsEnabled ? "https" : "http"}://192.168.1.20:{apiPort}/v1/scans</code>. Trial plan cannot use API endpoints.</p>
                    </div>

                    <div>
                        <div className="w-full p-4 rounded-lg border border-slate-200 bg-slate-50 dark:border-slate-600 dark:bg-slate-700/60">
                            <div className="flex items-start justify-between gap-4">
                                <div>
                                    <label className="text-base font-semibold text-slate-900 dark:text-white block">Local API HTTPS (Self-Signed)</label>
                                    <p className="text-sm text-slate-500 dark:text-slate-400 mt-1">
                                        {apiTlsEnabled
                                            ? "HTTPS is enabled with a self-signed certificate. Use curl -k or trust the certificate for local scripts."
                                            : "HTTP is enabled. Use only in trusted networks and keep the bearer token private."}
                                    </p>
                                    <p className="text-xs text-slate-500 dark:text-slate-400 mt-2">
                                        Protocol: <code>{apiTlsEnabled ? "HTTPS" : "HTTP"}</code>
                                    </p>
                                </div>
                                <button
                                    type="button"
                                    role="switch"
                                    aria-checked={apiTlsEnabled}
                                    onClick={() => setApiTlsEnabled(!apiTlsEnabled)}
                                    className={`relative inline-flex h-7 w-12 shrink-0 items-center rounded-full border transition-colors ${
                                        apiTlsEnabled
                                            ? "bg-emerald-500 border-emerald-500"
                                            : "bg-slate-300 border-slate-300 dark:bg-slate-600 dark:border-slate-600"
                                    }`}
                                >
                                    <span
                                        className={`inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform ${
                                            apiTlsEnabled ? "translate-x-6" : "translate-x-1"
                                        }`}
                                    />
                                </button>
                            </div>
                        </div>
                    </div>

                    <div>
                        <div className="flex items-center justify-between mb-1.5">
                            <label className="text-base font-bold text-slate-400 uppercase">API Access Token</label>
                            <button
                                type="button"
                                onClick={() => {
                                    const token = generateApiToken();
                                    setApiAccessToken(token);
                                    showToast("New API token generated. Save settings and update your scripts.");
                                }}
                                className="inline-flex items-center gap-1 px-3 py-1.5 rounded-md border border-slate-200 dark:border-slate-600 text-xs font-semibold text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors"
                            >
                                <RefreshCw className="w-3.5 h-3.5" />
                                Rotate Token
                            </button>
                        </div>
                        <input
                            type="text"
                            className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 font-mono"
                            value={apiAccessToken}
                            onChange={e => setApiAccessToken(e.target.value)}
                        />
                        <p className="text-base text-slate-500 mt-2">Non-local callers must send <code>Authorization: Bearer &lt;api_access_token&gt;</code>. API is Pro-only in trial mode.</p>
                    </div>

                    <div className="bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg p-4 flex items-start gap-3">
                        <div className="p-1 bg-amber-100 dark:bg-amber-800 rounded-full text-amber-600 dark:text-amber-400 shrink-0">
                            <AlertTriangle className="w-5 h-5" />
                        </div>
                        <div>
                            <h4 className="text-lg font-bold text-amber-800 dark:text-amber-400">Restart Required</h4>
                            <p className="text-base text-amber-700 dark:text-amber-500 mt-1">Changes to API host, API port, API HTTPS mode, or API token settings will only take effect after fully restarting the application.</p>
                        </div>
                    </div>

                    <div className="flex justify-end pt-4">
                        <button
                            onClick={async () => {
                                setSaving(true);
                                try {
                                    await new Promise(r => setTimeout(r, 600));
                                    const normalizedHost = apiLanEnabled
                                        ? ((apiBindHost || "").trim() || "0.0.0.0")
                                        : "127.0.0.1";
                                    const normalizedToken = (apiAccessToken || "").trim();
                                    if (!normalizedToken) {
                                        throw new Error("API Access Token cannot be empty.");
                                    }
                                    if (normalizedToken.length < 24) {
                                        throw new Error("API Access Token must be at least 24 characters.");
                                    }
                                    if (/\s/.test(normalizedToken)) {
                                        throw new Error("API Access Token cannot contain whitespace.");
                                    }
                                    const parsedPort = Number(apiPort.trim());
                                    if (!Number.isFinite(parsedPort) || !Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535) {
                                        throw new Error("API Listen Port must be between 1 and 65535.");
                                    }
                                    const normalizedPort = String(parsedPort);
                                    setApiPort(normalizedPort);
                                    await invoke("save_setting", { key: "api_bind_host", value: normalizedHost });
                                    await invoke("save_setting", { key: "api_port", value: normalizedPort });
                                    await invoke("save_setting", { key: "api_tls_enabled", value: apiTlsEnabled ? "1" : "0" });
                                    await invoke("save_setting", { key: "api_access_token", value: normalizedToken });
                                    showToast("Local API settings saved!");
                                } catch (e) {
                                    showToast("Failed: " + e, "error");
                                } finally {
                                    setSaving(false);
                                }
                            }}
                            disabled={saving}
                            className="bg-indigo-600 hover:bg-indigo-700 text-white px-6 py-3 rounded-lg font-bold flex items-center transition-colors disabled:opacity-70 disabled:cursor-not-allowed shadow-lg shadow-indigo-500/20"
                        >
                            {saving ? <Loader2 className="w-5 h-5 mr-2 animate-spin" /> : <Save className="w-5 h-5 mr-2" />}
                            {saving ? "Apply Settings" : "Apply Settings"}
                        </button>
                    </div>
                </div>
            </div>
        )}

        {activeTab === "clouds" && (
          <AccountsSettingsContent
            awsProfiles={awsProfiles}
            cloudProfiles={cloudProfiles}
            accountNotificationAssignments={accountNotificationAssignments}
            resolveAccountNotificationLabel={resolveAccountNotificationLabel}
            openImportModal={openImportModal}
            onAddAccount={() => {
              resetForms();
              setShowAddModal(true);
            }}
            handleQuickTestAwsProfile={handleQuickTestAwsProfile}
            handleQuickTestCloudProfile={handleQuickTestCloudProfile}
            openAwsEditModal={openAwsEditModal}
            openEditModal={openEditModal}
            handleDeleteAws={handleDeleteAws}
            handleDeleteCloud={handleDeleteCloud}
            testingAccountId={testingAccountId}
          />
        )}

        <ImportAccountsModal
          open={showImportModal}
          onClose={() => setShowImportModal(false)}
          importFileRef={importFileRef}
          onFileChange={handleImportFileSelected}
          discoverImportableAccounts={discoverImportableAccounts}
          downloadImportTemplate={downloadImportTemplate}
          selectAllImportCandidates={selectAllImportCandidates}
          handleImportSelectedAccounts={handleImportSelectedAccounts}
          discoveringImports={discoveringImports}
          importingAccounts={importingAccounts}
          importCandidates={importCandidates}
          selectedImportIds={selectedImportIds}
          setSelectedImportIds={setSelectedImportIds}
          providerDisplayName={providerDisplayName}
          importResultSummary={importResultSummary}
          importInvalidItems={importInvalidItems}
          importExecutionFailures={importExecutionFailures}
          importRenameMappings={importRenameMappings}
        />

        <AccountProfileModal
          open={showAddModal}
          modalMode={modalMode}
          modalTab={modalTab}
          onClose={() => { setShowCredentialSecrets(false); setShowAddModal(false); }}
          onSetModalTab={setModalTab}
          selectedProvider={selectedProvider}
          onSelectedProviderChange={setSelectedProvider}
          providerOptions={CLOUD_PROVIDER_OPTIONS}
          providerSelectionDisabled={modalMode === "edit"}
          selectedAccountProxy={normalizeProxySelection(selectedAccountProxy)}
          onSelectedAccountProxyChange={setSelectedAccountProxy}
          proxySelectionOptions={proxySelectionOptions}
          selectedAccountNotifications={normalizeAccountNotificationSelection(selectedAccountNotifications, { emptyAsAll: false })}
          isAllNotificationsSelected={isAllNotificationsSelected}
          toggleAccountNotificationSelection={toggleAccountNotificationSelection}
          allChannelsChoiceValue={ACCOUNT_NOTIFICATION_CHOICE_ALL}
          notificationChannels={notificationChannels}
          normalizeNotificationMethod={normalizeNotificationMethod}
          credentialsContent={
            <>
              <CoreProviderCredentialFields
                selectedProvider={selectedProvider}
                showCredentialSecrets={showCredentialSecrets}
                onToggleCredentialSecrets={() => setShowCredentialSecrets((prev) => !prev)}
                awsForm={awsForm}
                setAwsForm={setAwsForm}
                azureForm={azureForm}
                setAzureForm={setAzureForm}
                gcpForm={gcpForm}
                setGcpForm={setGcpForm}
                aliForm={aliForm}
                setAliForm={setAliForm}
                doForm={doForm}
                setDoForm={setDoForm}
                cfForm={cfForm}
                setCfForm={setCfForm}
                vultrForm={vultrForm}
                setVultrForm={setVultrForm}
                linodeForm={linodeForm}
                setLinodeForm={setLinodeForm}
                hetzForm={hetzForm}
                setHetzForm={setHetzForm}
                scwForm={scwForm}
                setScwForm={setScwForm}
                exoForm={exoForm}
                setExoForm={setExoForm}
                lwForm={lwForm}
                setLwForm={setLwForm}
                upcForm={upcForm}
                setUpcForm={setUpcForm}
                gcoreForm={gcoreForm}
                setGcoreForm={setGcoreForm}
                contaboForm={contaboForm}
                setContaboForm={setContaboForm}
                civoForm={civoForm}
                setCivoForm={setCivoForm}
                equinixForm={equinixForm}
                setEquinixForm={setEquinixForm}
                rackspaceForm={rackspaceForm}
                setRackspaceForm={setRackspaceForm}
                openstackForm={openstackForm}
                setOpenstackForm={setOpenstackForm}
              />
              <ExtendedProviderCredentialFields
                selectedProvider={selectedProvider}
                showCredentialSecrets={showCredentialSecrets}
                onToggleCredentialSecrets={() => setShowCredentialSecrets((prev) => !prev)}
                wasabiForm={wasabiForm}
                setWasabiForm={setWasabiForm}
                backblazeForm={backblazeForm}
                setBackblazeForm={setBackblazeForm}
                idriveForm={idriveForm}
                setIdriveForm={setIdriveForm}
                storjForm={storjForm}
                setStorjForm={setStorjForm}
                dreamhostForm={dreamhostForm}
                setDreamhostForm={setDreamhostForm}
                cloudianForm={cloudianForm}
                setCloudianForm={setCloudianForm}
                s3compatibleForm={s3compatibleForm}
                setS3compatibleForm={setS3compatibleForm}
                minioForm={minioForm}
                setMinioForm={setMinioForm}
                cephForm={cephForm}
                setCephForm={setCephForm}
                lyveForm={lyveForm}
                setLyveForm={setLyveForm}
                dellForm={dellForm}
                setDellForm={setDellForm}
                storagegridForm={storagegridForm}
                setStoragegridForm={setStoragegridForm}
                scalityForm={scalityForm}
                setScalityForm={setScalityForm}
                hcpForm={hcpForm}
                setHcpForm={setHcpForm}
                qumuloForm={qumuloForm}
                setQumuloForm={setQumuloForm}
                nutanixForm={nutanixForm}
                setNutanixForm={setNutanixForm}
                flashbladeForm={flashbladeForm}
                setFlashbladeForm={setFlashbladeForm}
                greenlakeForm={greenlakeForm}
                setGreenlakeForm={setGreenlakeForm}
                ionosForm={ionosForm}
                setIonosForm={setIonosForm}
                oracleForm={oracleForm}
                setOracleForm={setOracleForm}
                ibmForm={ibmForm}
                setIbmForm={setIbmForm}
                ovhForm={ovhForm}
                setOvhForm={setOvhForm}
                huaweiForm={huaweiForm}
                setHuaweiForm={setHuaweiForm}
                tencentForm={tencentForm}
                setTencentForm={setTencentForm}
                volcForm={volcForm}
                setVolcForm={setVolcForm}
                baiduForm={baiduForm}
                setBaiduForm={setBaiduForm}
                tianyiForm={tianyiForm}
                setTianyiForm={setTianyiForm}
              />
                    </>
          }
          accountRules={accountRules}
          onToggleRule={toggleRule}
          testing={testing}
          onTestConnection={handleTestConnection}
          onSave={handleSaveProfile}
        />

        <ConfirmActionModal
          isOpen={!!confirmDialog}
          title={confirmDialog?.title || "Confirm Action"}
          message={confirmDialog?.message || ""}
          confirmLabel={confirmDialog?.confirmLabel || "Confirm"}
          confirmClassName={confirmDialog?.confirmClassName}
          confirmingAction={confirmingAction}
          onCancel={() => setConfirmDialog(null)}
          onConfirm={runConfirmDialogAction}
        />

        <PendingDeleteAwsModal
          open={!!pendingDeleteAwsName}
          profileName={pendingDeleteAwsName}
          onCancel={() => setPendingDeleteAwsName(null)}
          onConfirm={confirmDeleteAws}
        />

        <SettingsToast
          open={!!toast}
          type={toast?.type || "success"}
          message={toast?.msg || ""}
          onDismiss={dismissToast}
        />
      </PageShell>
  );
}
