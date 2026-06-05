export type RuntimeEdition = "community" | "team" | "enterprise";
const RUNTIME_PLAN_STORAGE_KEY = "cws_runtime_plan_type";
const RUNTIME_EDITION_STORAGE_KEY = "cws_runtime_edition";

export interface RuntimeEntitlements {
  local_scan: boolean;
  basic_report: boolean;
  resource_details: boolean;
  local_api: boolean;
  team_workspace: boolean;
  scheduled_audits: boolean;
  audit_log: boolean;
  sso: boolean;
  scim: boolean;
}

export interface RuntimeCapabilityFlags {
  discovery_and_evidence: boolean;
  local_api_and_exports: boolean;
  team_governance_execution: boolean;
  scheduled_governance: boolean;
  enterprise_audit: boolean;
  enterprise_identity: boolean;
}

export interface RuntimeCapabilitySnapshot {
  plan_type: string;
  edition: RuntimeEdition;
  is_trial: boolean;
  entitlements: RuntimeEntitlements;
  capabilities: RuntimeCapabilityFlags;
}

export type RuntimeEntitlementKey = keyof RuntimeEntitlements;
export type ProductCapabilityKey =
  | "discovery_and_evidence"
  | "local_api_and_exports"
  | "team_governance_execution"
  | "scheduled_governance"
  | "enterprise_audit"
  | "enterprise_identity";

const COMMUNITY: RuntimeEntitlements = {
  local_scan: true,
  basic_report: true,
  resource_details: true,
  local_api: true,
  team_workspace: false,
  scheduled_audits: false,
  audit_log: false,
  sso: false,
  scim: false,
};

const TRIAL: RuntimeEntitlements = {
  local_scan: true,
  basic_report: true,
  resource_details: false,
  local_api: false,
  team_workspace: false,
  scheduled_audits: false,
  audit_log: false,
  sso: false,
  scim: false,
};

const TEAM: RuntimeEntitlements = {
  local_scan: true,
  basic_report: true,
  resource_details: true,
  local_api: true,
  team_workspace: true,
  scheduled_audits: true,
  audit_log: false,
  sso: false,
  scim: false,
};

const ENTERPRISE: RuntimeEntitlements = {
  local_scan: true,
  basic_report: true,
  resource_details: true,
  local_api: true,
  team_workspace: true,
  scheduled_audits: true,
  audit_log: true,
  sso: true,
  scim: true,
};

export function normalizeRuntimePlanType(raw: string | null | undefined): string {
  const value = (raw || "").trim().toLowerCase();
  if (value === "subscription") return "monthly";
  if (value === "per-use") return "starter";
  return value;
}

export function resolveRuntimeEdition(planTypeRaw: string | null | undefined): RuntimeEdition {
  const plan = normalizeRuntimePlanType(planTypeRaw);
  if (["enterprise", "advanced", "site"].includes(plan)) return "enterprise";
  if (["team", "monthly", "yearly", "lifetime", "pro"].includes(plan)) return "team";
  return "community";
}

export function entitlementsForPlan(planTypeRaw: string | null | undefined): RuntimeEntitlements {
  const plan = normalizeRuntimePlanType(planTypeRaw);
  if (plan === "trial") return TRIAL;
  const edition = resolveRuntimeEdition(plan);
  if (edition === "enterprise") return ENTERPRISE;
  if (edition === "team") return TEAM;
  return COMMUNITY;
}

export function readRuntimePlanTypeFromStorage(): string {
  return normalizeRuntimePlanType(localStorage.getItem(RUNTIME_PLAN_STORAGE_KEY));
}

export function readRuntimeEditionFromStorage(): RuntimeEdition {
  return resolveRuntimeEdition(readRuntimePlanTypeFromStorage());
}

const TAB_REQUIREMENTS: Record<string, RuntimeEntitlementKey | null> = {
  local_api: "local_api",
  audit_log: "audit_log",
  history: "resource_details",
};

export function formatEditionLabel(edition: RuntimeEdition): string {
  if (edition === "team") return "Team";
  if (edition === "enterprise") return "Enterprise";
  return "Community";
}

export function teamWorkspaceGateMessage(): string {
  return "Team unlocks org structure, owner directory, lifecycle workflow, and handoff coordination. Enterprise includes the same governance execution layer plus centralized identity and audit controls.";
}

export function scheduledAuditsGateMessage(): string {
  return "Scheduled audits belong to the Team governance execution layer. Enterprise includes the same scheduling capability plus centralized identity and audit controls.";
}

export function auditLogGateMessage(): string {
  return "Audit Log belongs to the Enterprise centralized control layer for operator accountability, identity, and compliance review.";
}

export function entitlementHintForTab(tabRaw: string): string {
  const tab = (tabRaw || "").trim().toLowerCase();
  if (tab === "audit_log") return auditLogGateMessage();
  if (tab === "local_api") return "Local API is available in Community and above when local API access is enabled.";
  if (tab === "history") return "Detailed history review is available in Community and above when scan history is enabled locally.";
  return "Upgrade required";
}

export function capabilityEnabled(
  capability: ProductCapabilityKey,
  planTypeRaw: string | null | undefined,
): boolean {
  const entitlements = entitlementsForPlan(planTypeRaw);
  switch (capability) {
    case "discovery_and_evidence":
      return entitlements.local_scan && entitlements.basic_report && entitlements.resource_details;
    case "local_api_and_exports":
      return entitlements.local_api;
    case "team_governance_execution":
      return entitlements.team_workspace;
    case "scheduled_governance":
      return entitlements.scheduled_audits;
    case "enterprise_audit":
      return entitlements.audit_log;
    case "enterprise_identity":
      return entitlements.sso && entitlements.scim;
    default:
      return false;
  }
}

export function capabilityFlagsForPlan(planTypeRaw: string | null | undefined): RuntimeCapabilityFlags {
  return {
    discovery_and_evidence: capabilityEnabled("discovery_and_evidence", planTypeRaw),
    local_api_and_exports: capabilityEnabled("local_api_and_exports", planTypeRaw),
    team_governance_execution: capabilityEnabled("team_governance_execution", planTypeRaw),
    scheduled_governance: capabilityEnabled("scheduled_governance", planTypeRaw),
    enterprise_audit: capabilityEnabled("enterprise_audit", planTypeRaw),
    enterprise_identity: capabilityEnabled("enterprise_identity", planTypeRaw),
  };
}

export function buildRuntimeCapabilitySnapshotFromPlan(
  planTypeRaw: string | null | undefined,
): RuntimeCapabilitySnapshot {
  const plan_type = normalizeRuntimePlanType(planTypeRaw);
  const edition = resolveRuntimeEdition(planTypeRaw);
  const is_trial = plan_type === "trial";
  const entitlements = entitlementsForPlan(planTypeRaw);
  const capabilities = capabilityFlagsForPlan(planTypeRaw);
  return {
    plan_type,
    edition,
    is_trial,
    entitlements,
    capabilities,
  };
}

export interface EditionCapabilityRow {
  key: ProductCapabilityKey;
  label: string;
  description: string;
  community: boolean;
  team: boolean;
  enterprise: boolean;
}

export const EDITION_CAPABILITY_MATRIX: EditionCapabilityRow[] = [
  {
    key: "discovery_and_evidence",
    label: "Discovery and evidence",
    description: "Local scans, findings review, history, and evidence exports on one machine.",
    community: true,
    team: true,
    enterprise: true,
  },
  {
    key: "local_api_and_exports",
    label: "Local API and automation",
    description: "Embedded local API, OpenAPI-driven automation, and local export workflows.",
    community: true,
    team: true,
    enterprise: true,
  },
  {
    key: "team_governance_execution",
    label: "Team governance execution",
    description: "Org units, owner directory, lifecycle workflow, and handoff coordination.",
    community: false,
    team: true,
    enterprise: true,
  },
  {
    key: "scheduled_governance",
    label: "Scheduled governance",
    description: "Recurring governance reviews and scheduled audit workflows.",
    community: false,
    team: true,
    enterprise: true,
  },
  {
    key: "enterprise_audit",
    label: "Enterprise audit controls",
    description: "Operator audit log and centralized accountability controls.",
    community: false,
    team: false,
    enterprise: true,
  },
  {
    key: "enterprise_identity",
    label: "Enterprise identity",
    description: "SSO, SCIM, and centralized identity lifecycle integration.",
    community: false,
    team: false,
    enterprise: true,
  },
];

export function requiredEntitlementForTab(tabRaw: string): RuntimeEntitlementKey | null {
  const tab = (tabRaw || "").trim().toLowerCase();
  return TAB_REQUIREMENTS[tab] ?? null;
}

export function canAccessTabByPlan(
  tabRaw: string,
  planTypeRaw: string | null | undefined,
): boolean {
  const required = requiredEntitlementForTab(tabRaw);
  if (!required) return true;
  const entitlements = entitlementsForPlan(planTypeRaw);
  return !!entitlements[required];
}

export function canAccessTabFromStorage(tabRaw: string): boolean {
  return canAccessTabByPlan(tabRaw, readRuntimePlanTypeFromStorage());
}

export function persistRuntimePlanToStorage(planTypeRaw: string | null | undefined): {
  planType: string;
  edition: RuntimeEdition;
} {
  const planType = normalizeRuntimePlanType(planTypeRaw);
  const edition = resolveRuntimeEdition(planType);
  localStorage.setItem(RUNTIME_PLAN_STORAGE_KEY, planType);
  localStorage.setItem(RUNTIME_EDITION_STORAGE_KEY, edition);
  return { planType, edition };
}

export function clearRuntimePlanFromStorage(): void {
  localStorage.removeItem(RUNTIME_PLAN_STORAGE_KEY);
  localStorage.removeItem(RUNTIME_EDITION_STORAGE_KEY);
}
