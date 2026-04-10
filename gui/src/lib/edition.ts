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

export type RuntimeEntitlementKey = keyof RuntimeEntitlements;

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
