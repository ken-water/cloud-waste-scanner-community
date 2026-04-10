export interface WasteResourceForEsg {
    provider?: string;
    resource_type: string;
    details?: string;
    estimated_monthly_cost: number;
    action_type?: string;
}

interface EsgProfile {
    key: string;
    pattern: RegExp;
    kgPerUsdMonth: number;
}

export interface ResourceEsgEstimate {
    monthlyCo2eKg: number;
    annualCo2eKg: number;
    emissionFactorKgPerUsd: number;
    actionMultiplier: number;
    profile: string;
}

export interface AggregateCo2eEstimate {
    totalMonthlyCo2eKg: number;
    totalAnnualCo2eKg: number;
    resourceCount: number;
}

const RESOURCE_PROFILES: EsgProfile[] = [
    { key: "gpu_compute", pattern: /\bgpu\b|machine learning|ml\b|ai\b/, kgPerUsdMonth: 0.9 },
    { key: "database", pattern: /\brds\b|database|\bsql\b|postgres|mysql|redis|cache|warehouse/, kgPerUsdMonth: 0.68 },
    { key: "compute", pattern: /\bvm\b|instance|compute|container|cluster|node|kubernetes|pod|server/, kgPerUsdMonth: 0.54 },
    { key: "storage", pattern: /volume|disk|storage|snapshot|bucket|\bs3\b|blob|filesystem|file system/, kgPerUsdMonth: 0.22 },
    { key: "network", pattern: /\bip\b|nat|gateway|balancer|network|egress|bandwidth/, kgPerUsdMonth: 0.3 },
    { key: "serverless", pattern: /lambda|function|serverless|app service/, kgPerUsdMonth: 0.26 },
];

const DEFAULT_FACTOR_KG_PER_USD_MONTH = 0.4;

const ACTION_MULTIPLIER: Record<string, number> = {
    DELETE: 1,
    RIGHTSIZE: 0.65,
    ARCHIVE: 0.55,
    STOP: 0.8,
    PAUSE: 0.8,
};

export const ESG_METHODOLOGY_NOTE =
    "Estimated CO2e uses weighted factors by resource type (compute/storage/network/database) and recommendation action impact.";

export const ESG_DISCLAIMER_NOTE =
    "ESG values are planning estimates, not audited emissions. Use them for trend tracking and decision support.";

function normalizeCost(cost: number): number {
    if (!Number.isFinite(cost) || cost <= 0) return 0;
    return cost;
}

function resolveFactor(resource: WasteResourceForEsg): { factor: number; profile: string } {
    const haystack = `${resource.provider ?? ""} ${resource.resource_type} ${resource.details ?? ""}`.toLowerCase();
    const matched = RESOURCE_PROFILES.find((profile) => profile.pattern.test(haystack));
    if (matched) {
        return { factor: matched.kgPerUsdMonth, profile: matched.key };
    }
    return { factor: DEFAULT_FACTOR_KG_PER_USD_MONTH, profile: "default" };
}

function resolveActionMultiplier(actionType?: string): number {
    if (!actionType) return 0.7;
    return ACTION_MULTIPLIER[actionType.toUpperCase()] ?? 0.7;
}

export function estimateResourceCo2e(resource: WasteResourceForEsg): ResourceEsgEstimate {
    const cost = normalizeCost(resource.estimated_monthly_cost);
    const { factor, profile } = resolveFactor(resource);
    const actionMultiplier = resolveActionMultiplier(resource.action_type);
    const monthlyCo2eKg = cost * factor * actionMultiplier;

    return {
        monthlyCo2eKg,
        annualCo2eKg: monthlyCo2eKg * 12,
        emissionFactorKgPerUsd: factor,
        actionMultiplier,
        profile,
    };
}

export function estimateAggregateCo2e(resources: WasteResourceForEsg[]): AggregateCo2eEstimate {
    const totalMonthlyCo2eKg = resources.reduce((sum, resource) => {
        return sum + estimateResourceCo2e(resource).monthlyCo2eKg;
    }, 0);

    return {
        totalMonthlyCo2eKg,
        totalAnnualCo2eKg: totalMonthlyCo2eKg * 12,
        resourceCount: resources.length,
    };
}

export function formatCo2eKg(value: number, digits = 1): string {
    return `${value.toFixed(digits)} kg CO2e`;
}

export function formatCo2eTonsFromKg(valueKg: number, digits = 2): string {
    return `${(valueKg / 1000).toFixed(digits)} t CO2e`;
}
