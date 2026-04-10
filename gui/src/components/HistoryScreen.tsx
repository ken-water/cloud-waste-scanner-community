import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Trash2, FileText, ChevronRight, History, X, CheckCircle, RefreshCw, Leaf } from "lucide-react";
import { useCurrency } from "../hooks/useCurrency";
import { Modal } from "./Modal";
import { estimateAggregateCo2e, estimateResourceCo2e, formatCo2eKg, formatCo2eTonsFromKg, ESG_METHODOLOGY_NOTE, ESG_DISCLAIMER_NOTE } from "../utils/esg";
import { drawPdfBrandHeader, drawPdfFooterSiteLink, formatPdfDateTime } from "../utils/pdfBranding";
import { exportBlobWithTauriFallback, revealExportedFileInFolder } from "../utils/fileExport";
import { CLOUD_PROVIDER_OPTIONS, resolveProviderValue } from "../constants/cloudProviders";
import { loadPdfRuntime } from "../utils/pdfRuntime";
import { PageHeader } from "./layout/PageHeader";
import { MetricCard } from "./ui/MetricCard";

interface ScanHistoryItem {
    id: number;
    scanned_at: number;
    total_waste: number;
    resource_count: number;
    status: string;
    results_json: string;
    scan_meta?: string;
}

interface WastedResource {
    id: string;
    provider: string;
    region: string;
    resource_type: string;
    details: string;
    estimated_monthly_cost: number;
    action_type: string;
}

interface ScanMeta {
    scanned_accounts?: string[];
}

interface ProviderRuleTemplate {
    id: string;
    name: string;
}

interface CoverageProviderMeta {
    key: string;
    display: string;
}

const PROVIDER_ORDER_INDEX = new Map(
    CLOUD_PROVIDER_OPTIONS.map((provider, index) => [provider.value, index])
);

function safeParse<T>(raw: string | undefined, fallback: T): T {
    if (!raw) return fallback;
    try {
        return JSON.parse(raw) as T;
    } catch {
        return fallback;
    }
}

function normalizeProviderSource(raw: string): string {
    const input = String(raw || "").trim();
    if (!input) return "";
    const accountPrefix = input.match(/^([^(]+)\s*\(/);
    if (accountPrefix?.[1]) return accountPrefix[1].trim();
    return input;
}

function titleCaseWords(input: string): string {
    return input
        .split(/\s+/)
        .filter(Boolean)
        .map((part) => part.charAt(0).toUpperCase() + part.slice(1).toLowerCase())
        .join(" ");
}

function resolveCoverageProvider(raw: string): CoverageProviderMeta {
    const source = normalizeProviderSource(raw);
    const normalizedValue = resolveProviderValue(source);
    const option = CLOUD_PROVIDER_OPTIONS.find((provider) => provider.value === normalizedValue);
    if (option) {
        return { key: option.value, display: option.label };
    }

    if (!source) {
        return { key: "unknown", display: "Unknown Provider" };
    }

    const token = source.toLowerCase().replace(/[^a-z0-9]/g, "");
    const display = titleCaseWords(source.replace(/[_-]+/g, " "));
    return { key: token || "unknown", display };
}

function collectCoverageProviders(resources: WastedResource[], accounts: string[]): CoverageProviderMeta[] {
    const byKey = new Map<string, CoverageProviderMeta>();
    for (const account of accounts) {
        const provider = resolveCoverageProvider(account);
        byKey.set(provider.key, provider);
    }
    for (const resource of resources) {
        const provider = resolveCoverageProvider(resource.provider);
        byKey.set(provider.key, provider);
    }

    return Array.from(byKey.values()).sort((a, b) => {
        const idxA = PROVIDER_ORDER_INDEX.get(a.key);
        const idxB = PROVIDER_ORDER_INDEX.get(b.key);
        if (typeof idxA === "number" && typeof idxB === "number") return idxA - idxB;
        if (typeof idxA === "number") return -1;
        if (typeof idxB === "number") return 1;
        return a.display.localeCompare(b.display);
    });
}

function sanitizePdfText(value: unknown): string {
    return String(value ?? "")
        .normalize("NFKD")
        .replace(/[^\x20-\x7E]/g, " ")
        .replace(/\s+/g, " ")
        .trim();
}

function formatDateTime(value: Date | number): string {
    const date = value instanceof Date ? value : new Date(value);
    if (Number.isNaN(date.getTime())) return "-";
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function normalizeScanTimestampMs(raw: number): number {
    const abs = Math.abs(raw);
    // Backward-compatible: accept both seconds and milliseconds in history rows.
    // >= 1e12 is already in milliseconds (roughly 2001+).
    if (abs >= 1_000_000_000_000) return raw;
    return raw * 1000;
}

function formatDateForFilename(value: number): string {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return "unknown-date";
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function compareScanRuns(current: ScanHistoryItem | null, previous: ScanHistoryItem | null) {
    if (!current || !previous) {
        return {
            wasteDelta: null as number | null,
            findingDelta: null as number | null,
            accountDelta: null as number | null,
        };
    }
    const currentMeta = safeParse<ScanMeta>(current.scan_meta, {});
    const previousMeta = safeParse<ScanMeta>(previous.scan_meta, {});
    return {
        wasteDelta: Number(current.total_waste || 0) - Number(previous.total_waste || 0),
        findingDelta: Number(current.resource_count || 0) - Number(previous.resource_count || 0),
        accountDelta: (currentMeta.scanned_accounts || []).length - (previousMeta.scanned_accounts || []).length,
    };
}

function buildProviderComparison(currentResources: WastedResource[], previousResources: WastedResource[]) {
    const bucket = new Map<string, { current: number; previous: number }>();
    for (const row of currentResources) {
        const key = row.provider || "Unknown";
        const current = bucket.get(key) || { current: 0, previous: 0 };
        current.current += Number(row.estimated_monthly_cost || 0);
        bucket.set(key, current);
    }
    for (const row of previousResources) {
        const key = row.provider || "Unknown";
        const current = bucket.get(key) || { current: 0, previous: 0 };
        current.previous += Number(row.estimated_monthly_cost || 0);
        bucket.set(key, current);
    }
    return Array.from(bucket.entries())
        .map(([provider, values]) => ({
            provider,
            current: values.current,
            previous: values.previous,
            delta: values.current - values.previous,
        }))
        .filter((row) => row.current > 0 || row.previous > 0)
        .sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta))
        .slice(0, 4);
}

function inferRuleKeywords(ruleId: string, ruleName: string): string[] {
    const source = `${ruleId} ${ruleName}`.toLowerCase();
    const keywords = new Set<string>();
    const include = (word: string) => keywords.add(word);

    if (/instance|vm|server|host|vps|droplet|compute|cvm|ecs|bcc|linode|node/.test(source)) {
        ["instance", "vm", "server", "host", "vps", "droplet", "compute", "cvm", "ecs", "bcc", "linode"].forEach(include);
    }
    if (/disk|volume|ebs|evs|cbs|cds|block|boot/.test(source)) {
        ["disk", "volume", "ebs", "evs", "cbs", "cds", "block", "boot"].forEach(include);
    }
    if (/ip|eip|fip|floating|elastic|public/.test(source)) {
        ["ip", "eip", "floating ip", "elastic ip", "public ip", "reserved ip"].forEach(include);
    }
    if (/snapshot|ami/.test(source)) {
        ["snapshot", "ami", "image"].forEach(include);
    }
    if (/lb|balancer|slb|clb|elb|blb|nodebalancer|gateway|nat/.test(source)) {
        ["balancer", "slb", "clb", "elb", "blb", "nodebalancer", "gateway", "nat"].forEach(include);
    }
    if (/rds|db|sql|database|redis|cdb|postgres|mysql/.test(source)) {
        ["rds", "db", "database", "sql", "redis", "cdb", "postgres", "mysql"].forEach(include);
    }
    if (/bucket|object|oss|bos|obs|tos|oos|cos|s3|r2/.test(source)) {
        ["bucket", "object", "oss", "bos", "obs", "tos", "oos", "cos", "s3", "r2", "storage"].forEach(include);
    }
    if (/multipart/.test(source)) {
        ["multipart"].forEach(include);
    }
    if (/version/.test(source)) {
        ["version"].forEach(include);
    }
    if (/lifecycle/.test(source)) {
        ["lifecycle"].forEach(include);
    }
    if (/log/.test(source)) {
        ["log"].forEach(include);
    }
    if (/dns/.test(source)) {
        ["dns"].forEach(include);
    }
    if (/worker/.test(source)) {
        ["worker"].forEach(include);
    }
    if (/tunnel/.test(source)) {
        ["tunnel"].forEach(include);
    }
    if (/page/.test(source)) {
        ["page"].forEach(include);
    }

    return Array.from(keywords);
}

function countRuleIssues(providerResources: WastedResource[], rule: ProviderRuleTemplate): number {
    if (providerResources.length === 0) return 0;

    if (rule.id.startsWith("fallback_type::")) {
        const resourceType = rule.id.split("::")[1] || "";
        return providerResources.filter((resource) => resource.resource_type === resourceType).length;
    }

    const keywords = inferRuleKeywords(rule.id, rule.name);
    if (keywords.length === 0) {
        return providerResources.length;
    }

    return providerResources.filter((resource) => {
        const haystack = `${resource.resource_type} ${resource.details}`.toLowerCase();
        return keywords.some((keyword) => haystack.includes(keyword));
    }).length;
}

function buildFallbackRules(providerResources: WastedResource[]): ProviderRuleTemplate[] {
    const resourceTypes = Array.from(
        new Set(providerResources.map((resource) => resource.resource_type).filter(Boolean))
    );
    if (resourceTypes.length > 0) {
        return resourceTypes.map((resourceType) => ({
            id: `fallback_type::${resourceType}`,
            name: resourceType,
        }));
    }
    return [
        { id: "fallback_compute", name: "Compute Resources" },
        { id: "fallback_storage", name: "Storage Resources" },
        { id: "fallback_network", name: "Network Resources" },
        { id: "fallback_database", name: "Database Resources" },
    ];
}

async function fetchProviderRules(providerKey: string): Promise<ProviderRuleTemplate[]> {
    try {
        const data = await invoke<Array<{ id?: string; name?: string }>>(
            "get_provider_rules_config",
            { provider: providerKey }
        );
        return (Array.isArray(data) ? data : [])
            .map((item) => ({
                id: String(item?.id || "").trim(),
                name: String(item?.name || "").trim(),
            }))
            .filter((rule) => rule.id && rule.name);
    } catch (err) {
        console.error(`Failed to load provider rule templates for ${providerKey}`, err);
        return [];
    }
}

async function buildCoverageRows(resources: WastedResource[], accounts: string[]): Promise<string[][]> {
    const providers = collectCoverageProviders(resources, accounts);
    const providerRules = await Promise.all(
        providers.map(async (provider) => ({
            provider,
            rules: await fetchProviderRules(provider.key),
        }))
    );

    const rows: string[][] = [];
    for (const item of providerRules) {
        const providerResources = resources.filter(
            (resource) => resolveCoverageProvider(resource.provider).key === item.provider.key
        );
        const rules = item.rules.length > 0 ? item.rules : buildFallbackRules(providerResources);
        for (const rule of rules) {
            const issues = countRuleIssues(providerResources, rule);
            rows.push([
                item.provider.display,
                rule.name,
                issues > 0 ? `${issues} issues` : "Clean",
            ]);
        }
    }

    return rows;
}

export function HistoryScreen() {
    const [history, setHistory] = useState<ScanHistoryItem[]>([]);
    const [selectedScan, setSelectedScan] = useState<ScanHistoryItem | null>(null);
    const { format: formatCurrency } = useCurrency();
    const [loading, setLoading] = useState(true);
    const [, setError] = useState<string | null>(null);
    const [exportError, setExportError] = useState<string | null>(null);
    const [deleteError, setDeleteError] = useState<string | null>(null);
    const [pendingDeleteId, setPendingDeleteId] = useState<number | null>(null);
    const [deleting, setDeleting] = useState(false);
    const [pendingExportScan, setPendingExportScan] = useState<ScanHistoryItem | null>(null);
    const [historyExportIncludeChecklistColumns, setHistoryExportIncludeChecklistColumns] = useState(true);
    const [exportingPdf, setExportingPdf] = useState(false);
    const [selectedCoverageRows, setSelectedCoverageRows] = useState<string[][]>([]);
    const [selectedCoverageLoading, setSelectedCoverageLoading] = useState(false);

    useEffect(() => {
        loadHistory();
    }, []);

    useEffect(() => {
        let cancelled = false;

        if (!selectedScan) {
            setSelectedCoverageRows([]);
            setSelectedCoverageLoading(false);
            return () => {
                cancelled = true;
            };
        }

        const resources = safeParse<WastedResource[]>(selectedScan.results_json, []);
        const meta = safeParse<ScanMeta>(selectedScan.scan_meta, {});
        const accounts = meta.scanned_accounts || [];

        setSelectedCoverageLoading(true);
        buildCoverageRows(resources, accounts)
            .then((rows) => {
                if (!cancelled) {
                    setSelectedCoverageRows(rows);
                }
            })
            .catch((err) => {
                console.error("Failed to build coverage rows", err);
                if (!cancelled) {
                    setSelectedCoverageRows([]);
                }
            })
            .finally(() => {
                if (!cancelled) {
                    setSelectedCoverageLoading(false);
                }
            });

        return () => {
            cancelled = true;
        };
    }, [selectedScan]);

    const showExportError = (text: string) => {
        setExportError(text);
        window.setTimeout(() => setExportError(null), 6000);
    };

    const revealSavedExport = async (savedPath: string | null | undefined) => {
        const resolvedPath = String(savedPath ?? "").trim();
        if (!resolvedPath) {
            throw new Error("PDF exported, but no local path was returned.");
        }
        await revealExportedFileInFolder(resolvedPath);
    };

    const loadHistory = async () => {
        setLoading(true);
        setError(null);
        try {
            const data = await invoke<ScanHistoryItem[]>("get_scan_history");
            setHistory(data);
        } catch (e) {
            const msg = String(e);
            setError(msg);
            console.error(e);
        } finally {
            setLoading(false);
        }
    };

    const handleDelete = async (e: React.MouseEvent, id: number) => {
        e.stopPropagation();
        setDeleteError(null);
        setPendingDeleteId(id);
    };

    const confirmDelete = async () => {
        if (pendingDeleteId === null) return;
        setDeleting(true);
        try {
            await invoke("delete_scan_history", { id: pendingDeleteId });
            setHistory(prev => prev.filter(item => item.id !== pendingDeleteId));
            if (selectedScan?.id === pendingDeleteId) setSelectedScan(null);
            setPendingDeleteId(null);
        } catch (e) {
            setDeleteError("Failed to delete: " + e);
        } finally {
            setDeleting(false);
        }
    };

    const handleExportPDF = async (scan: ScanHistoryItem, includeExecutionColumns: boolean) => {
        setExportingPdf(true);
        try {
            const scannedAtMs = normalizeScanTimestampMs(scan.scanned_at);
            const resources: WastedResource[] = safeParse<WastedResource[]>(scan.results_json, []);
            const meta = safeParse<ScanMeta>(scan.scan_meta, {});
            const accounts = meta.scanned_accounts || [];
            const coverageRows = await buildCoverageRows(resources, accounts);
            const co2e = estimateAggregateCo2e(resources);
            const { jsPDF, autoTable } = await loadPdfRuntime();
            const doc = new jsPDF({
                orientation: includeExecutionColumns ? "landscape" : "portrait",
                unit: "mm",
                format: "a4",
            });
            const docAny = doc as { lastAutoTable?: { finalY?: number } };
            const pageWidth = doc.internal.pageSize.getWidth();
            const pageHeight = doc.internal.pageSize.getHeight();
            const formatPdfCurrency = (amount: number) => sanitizePdfText(formatCurrency(amount));
            const pageContentBottomY = pageHeight - 20;
            const pageTopY = 18;
            const getLastTableY = (fallback: number) => Math.max(docAny.lastAutoTable?.finalY ?? 0, fallback);
            let cursorY = 0;
            const startSection = (title: string, minBodyHeight = 18) => {
                const anchorY = Math.max(cursorY, docAny.lastAutoTable?.finalY ?? 0);
                const nextY = anchorY + 12;
                const sectionHeight = 7 + minBodyHeight;
                if (nextY + sectionHeight > pageContentBottomY) {
                    doc.addPage();
                    cursorY = pageTopY;
                } else {
                    cursorY = nextY;
                }
                doc.setFontSize(14);
                doc.setTextColor(40, 40, 40);
                doc.text(title, 14, cursorY);
                return cursorY + 5;
            };
            
            // Header
            const headerBottomY = drawPdfBrandHeader(doc, {
                title: "Cloud Waste Audit Report",
                generatedAt: new Date(),
                extraLines: [`Scan Date: ${formatPdfDateTime(scannedAtMs)}`],
            });
            
            // Summary Box
            const summaryY = headerBottomY + 4;
            doc.setDrawColor(200); doc.setFillColor(250, 250, 252);
            doc.roundedRect(14, summaryY, 180, 34, 3, 3, "FD");
            
            doc.setFontSize(10); doc.setTextColor(100); doc.text("Potential Monthly Savings", 20, summaryY + 8);
            doc.setFontSize(16); doc.setTextColor(0, 150, 0); doc.text(formatPdfCurrency(resources.reduce((sum, r) => sum + r.estimated_monthly_cost, 0)), 20, summaryY + 18);
            
            doc.setFontSize(10); doc.setTextColor(100); doc.text("Issues Found", 90, summaryY + 8);
            doc.setFontSize(16); doc.setTextColor(200, 50, 50); doc.text(resources.length.toString(), 90, summaryY + 18);

            doc.setFontSize(10); doc.setTextColor(100); doc.text("Accounts Scanned", 150, summaryY + 8);
            doc.setFontSize(16); doc.setTextColor(40, 40, 40); doc.text(accounts.length.toString(), 150, summaryY + 18);
            doc.setFontSize(9); doc.setTextColor(15, 118, 110);
            doc.text(
                `Estimated CO2e Reduction: ${sanitizePdfText(formatCo2eKg(co2e.totalMonthlyCo2eKg))} / month (${sanitizePdfText(formatCo2eTonsFromKg(co2e.totalAnnualCo2eKg))} / year)`,
                20,
                summaryY + 29
            );

            // 1) Coverage Scope (accounts)
            cursorY = summaryY + 50;
            doc.setFontSize(14); doc.setTextColor(40, 40, 40); doc.text("1) Scan Coverage", 14, cursorY);
            autoTable(doc, {
                startY: cursorY + 5,
                head: [["Account"]],
                body:
                    accounts.length > 0
                        ? accounts.map((account) => [sanitizePdfText(account)])
                        : [["No account metadata available"]],
                theme: "grid",
                headStyles: { fillColor: [71, 85, 105] },
                styles: { fontSize: 9 },
                margin: { left: 14, right: 14, bottom: 18 },
            });
            cursorY = getLastTableY(cursorY + 5);

            // 2) Coverage report (same logic as right panel)
            const coverageStartY = startSection("2) Coverage Report", 24);
            autoTable(doc, {
                startY: coverageStartY,
                head: [["Provider", "Check", "Status"]],
                body:
                    coverageRows.length > 0
                        ? coverageRows.map((row) => row.map((value) => sanitizePdfText(value)))
                        : [["N/A", "No checks", "No data"]],
                theme: "grid",
                headStyles: { fillColor: [99, 102, 241] },
                styles: { fontSize: 9 },
                margin: { left: 14, right: 14, bottom: 18 },
            });
            cursorY = getLastTableY(coverageStartY);

            // 3) ESG impact
            const esgStartY = startSection("3) ESG Impact (Estimated)", 28);
            autoTable(doc, {
                startY: esgStartY,
                head: [["Metric", "Value"]],
                body: [
                    ["Monthly CO2e Reduction", sanitizePdfText(formatCo2eKg(co2e.totalMonthlyCo2eKg))],
                    ["Annual CO2e Reduction", sanitizePdfText(formatCo2eTonsFromKg(co2e.totalAnnualCo2eKg))],
                    ["Methodology", sanitizePdfText(ESG_METHODOLOGY_NOTE)],
                    ["Disclaimer", sanitizePdfText(ESG_DISCLAIMER_NOTE)],
                ],
                theme: "grid",
                headStyles: { fillColor: [15, 118, 110] },
                styles: { fontSize: 9 },
                margin: { left: 14, right: 14, bottom: 18 },
                columnStyles: {
                    0: { cellWidth: 45 },
                    1: { cellWidth: Math.max(80, pageWidth - 61) },
                },
            });
            cursorY = getLastTableY(esgStartY);

            // 4) Detailed findings checklist (single checklist table, optional extra columns)
            const findingsTitle = includeExecutionColumns
                ? "4) Detailed Findings Checklist"
                : "4) Detailed Findings";
            const findingsStartY = startSection(findingsTitle, resources.length === 0 ? 12 : 24);

            if (resources.length === 0) {
                doc.setFontSize(11); doc.setTextColor(0, 150, 0);
                doc.text("Clean Bill of Health: No wasted resources detected.", 14, findingsStartY + 5);
                cursorY = findingsStartY + 8;
            } else {
                const findingsHead = includeExecutionColumns
                    ? [["ID", "Provider", "Type", "Details", "Cost", "CO2e/mo", "Recommended", "Custom", "Notes"]]
                    : [["ID", "Provider", "Type", "Details", "Action", "Cost", "CO2e/mo"]];
                const findingsBody = resources.map((resource) => (
                    includeExecutionColumns
                        ? [
                            sanitizePdfText(resource.id),
                            sanitizePdfText(resource.provider),
                            sanitizePdfText(resource.resource_type),
                            sanitizePdfText(resource.details).slice(0, 88),
                            formatPdfCurrency(resource.estimated_monthly_cost),
                            sanitizePdfText(formatCo2eKg(estimateResourceCo2e(resource).monthlyCo2eKg)),
                            sanitizePdfText(resource.action_type || "Review"),
                            "",
                            "",
                        ]
                        : [
                            sanitizePdfText(resource.id),
                            sanitizePdfText(resource.provider),
                            sanitizePdfText(resource.resource_type),
                            sanitizePdfText(resource.details).slice(0, 60),
                            sanitizePdfText(resource.action_type || "n/a"),
                            formatPdfCurrency(resource.estimated_monthly_cost),
                            sanitizePdfText(formatCo2eKg(estimateResourceCo2e(resource).monthlyCo2eKg)),
                        ]
                ));
                autoTable(doc, {
                    startY: findingsStartY,
                    head: findingsHead,
                    body: findingsBody,
                    theme: "grid",
                    headStyles: { fillColor: [79, 70, 229] },
                    styles: { fontSize: 8 },
                    margin: { left: 14, right: 14, bottom: 18 },
                    columnStyles: includeExecutionColumns
                        ? {
                            0: { cellWidth: 40 },
                            1: { cellWidth: 18 },
                            2: { cellWidth: 22 },
                            3: { cellWidth: 58 },
                            4: { cellWidth: 16, halign: "right" },
                            5: { cellWidth: 20, halign: "right" },
                            6: { cellWidth: 36, cellPadding: { left: 6, right: 1.5, top: 1.5, bottom: 1.5 } },
                            7: { cellWidth: 16, cellPadding: { left: 6, right: 1.5, top: 1.5, bottom: 1.5 } },
                            8: { cellWidth: 43, cellPadding: { left: 2.2, right: 1.5, top: 1.5, bottom: 1.5 } },
                        }
                        : {
                            0: { cellWidth: 32 },
                            1: { cellWidth: 18 },
                            2: { cellWidth: 22 },
                            3: { cellWidth: 46 },
                            4: { cellWidth: 18 },
                            5: { cellWidth: 20, halign: "right" },
                            6: { cellWidth: 24, halign: "right" },
                        },
                    didParseCell: (data) => {
                        if (!includeExecutionColumns || data.section !== "body") return;
                        if (![6, 7, 8].includes(data.column.index)) return;
                        data.cell.text = [""];
                    },
                    didDrawCell: (data) => {
                        if (!includeExecutionColumns || data.section !== "body") return;
                        if (![6, 7, 8].includes(data.column.index)) return;
                        if (data.column.index === 6 || data.column.index === 7) {
                            const boxSize = Math.min(2.8, data.cell.height - 2.2);
                            const boxX = data.cell.x + 1.8;
                            const boxY = data.cell.y + (data.cell.height - boxSize) / 2;
                            doc.setDrawColor(120);
                            doc.rect(boxX, boxY, boxSize, boxSize);
                            if (data.column.index !== 6) {
                                return;
                            }
                            const contentX = boxX + boxSize + 1.8;
                            const rowRaw = Array.isArray(data.row.raw) ? data.row.raw : [];
                            const actionText = String(rowRaw[6] ?? "").trim();
                            if (actionText) {
                                doc.setFontSize(8);
                                doc.setTextColor(71, 85, 105);
                                doc.text(actionText, contentX, data.cell.y + data.cell.height / 2 + 1.4);
                            }
                            return;
                        }
                    },
                });
                cursorY = getLastTableY(findingsStartY);
            }

            const totalPages = doc.getNumberOfPages();
            for (let page = 1; page <= totalPages; page++) {
                doc.setPage(page);
                drawPdfFooterSiteLink(doc, pageWidth, pageHeight, page, totalPages);
            }
            
            const filename = `audit_report_${formatDateForFilename(scannedAtMs)}.pdf`;
            const blob = doc.output("blob");
            const savedPath = await exportBlobWithTauriFallback(blob, filename, { openAfterSave: false });
            await revealSavedExport(savedPath);
            setPendingExportScan(null);
        } catch (err) {
            console.error("Failed to export history PDF", err);
            showExportError(`PDF export failed: ${String(err)}`);
        } finally {
            setExportingPdf(false);
        }
    };

    if (loading) return <div className="p-8 text-center text-slate-500 animate-pulse">Loading history...</div>;

    const selectedResources = selectedScan ? safeParse<WastedResource[]>(selectedScan.results_json, []) : [];
    const selectedMeta = selectedScan ? safeParse<ScanMeta>(selectedScan.scan_meta, {}) : {};
    const selectedAccounts = selectedMeta.scanned_accounts || [];
    const selectedCo2e = estimateAggregateCo2e(selectedResources);
    const totalHistoricalSavings = history.reduce((sum, item) => sum + Number(item.total_waste || 0), 0);
    const selectedIndex = selectedScan ? history.findIndex((item) => item.id === selectedScan.id) : -1;
    const previousScan = selectedIndex >= 0 ? history[selectedIndex + 1] ?? null : null;
    const comparison = compareScanRuns(selectedScan, previousScan);
    const previousResources = previousScan ? safeParse<WastedResource[]>(previousScan.results_json, []) : [];
    const providerComparison = buildProviderComparison(selectedResources, previousResources);
    const formatDelta = (value: number | null, suffix = "") => {
        if (value === null) return "No prior run";
        const prefix = value > 0 ? "+" : "";
        return `${prefix}${value}${suffix}`;
    };

    return (
        <div className="flex flex-col h-screen bg-slate-50 dark:bg-slate-900 transition-colors duration-300 overflow-hidden">
            {/* Top Full-Width Header */}
            <div className="p-6 border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 flex justify-between items-center shrink-0">
                <div className="w-full">
                    <PageHeader
                        title="Scan History"
                        subtitle="Review prior scans, reopen packaged findings, and export handoff reports from past runs."
                        icon={<History className="w-6 h-6" />}
                        actions={
                            <button 
                                onClick={loadHistory}
                                className="p-2 text-slate-400 hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800"
                                title="Refresh History"
                            >
                                <RefreshCw className="w-5 h-5" />
                            </button>
                        }
                    />
                    {exportError && (
                        <p className="mt-2 text-sm font-medium text-rose-600 dark:text-rose-400">
                            {exportError}
                        </p>
                    )}
                    {deleteError && (
                        <p className="mt-2 text-sm font-medium text-rose-600 dark:text-rose-400">
                            {deleteError}
                        </p>
                    )}
                    <div className="mt-6 rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
                        <div className="grid gap-4 md:grid-cols-3">
                            <div>
                                <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Use This For</p>
                                <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                                    Reopen previous scans, compare historical runs, and export dated review packs for responsible teams.
                                </p>
                            </div>
                            <div>
                                <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Current Slice</p>
                                <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                                    {history.length} stored runs. {selectedScan ? `Selected run has ${selectedResources.length} findings across ${selectedAccounts.length} accounts.` : "No run selected yet."}
                                </p>
                            </div>
                            <div>
                                <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Operator Workflow</p>
                                <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                                    Choose a run from the left, inspect detailed findings on the right, then export the packaged PDF when a dated handoff is needed.
                                </p>
                            </div>
                        </div>
                    </div>
                    <div className="mt-4 grid gap-4 md:grid-cols-3">
                        <MetricCard
                            label="Stored Runs"
                            value={history.length}
                            hint="Historical scans currently available on this machine."
                        />
                        <MetricCard
                            label="Historical Savings"
                            value={<span className="text-emerald-600 dark:text-emerald-400">{formatCurrency(totalHistoricalSavings)}</span>}
                            hint="Sum of potential monthly savings across all stored runs."
                        />
                        <MetricCard
                            label="Selected Run"
                            value={selectedScan ? selectedResources.length : 0}
                            hint={selectedScan ? `${selectedAccounts.length} scanned accounts in the selected run.` : "Pick a run to inspect details."}
                        />
                    </div>
                </div>
            </div>

            {/* Split Content Area */}
            <div className="flex flex-1 overflow-hidden">
                {/* List Panel */}
                <div className={`w-full md:w-1/3 border-r border-slate-200 dark:border-slate-800 flex flex-col bg-slate-50 dark:bg-slate-900 ${selectedScan ? 'hidden md:flex' : 'flex'}`}>
                    <div className="flex-1 overflow-y-auto p-4 space-y-3">
                        {history.length === 0 ? (
                            <div className="text-center p-8 text-slate-400 dark:text-slate-500 italic">No history found.</div>
                        ) : (
                            history.map(item => (
                                <div 
                                    key={item.id}
                                    onClick={() => setSelectedScan(item)}
                                    className={`p-4 rounded-xl border cursor-pointer transition-all hover:shadow-md group relative ${
                                        selectedScan?.id === item.id 
                                        ? 'bg-indigo-50 dark:bg-indigo-900/20 border-indigo-200 dark:border-indigo-800 ring-1 ring-indigo-500' 
                                        : 'bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-700 hover:border-indigo-300 dark:hover:border-indigo-700'
                                    }`}
                                >
                                    <div className="flex justify-between items-start mb-2">
                                        <span className="text-sm font-bold text-slate-700 dark:text-slate-200">
                                            {formatDateTime(normalizeScanTimestampMs(item.scanned_at))}
                                        </span>
                                        <button 
                                            onClick={(e) => handleDelete(e, item.id)}
                                            className="p-1 text-slate-400 hover:text-red-500 transition-colors rounded hover:bg-red-50 dark:hover:bg-red-900/20"
                                            title="Delete Record"
                                        >
                                            <Trash2 className="w-4 h-4" />
                                        </button>
                                    </div>
                                    <div className="flex justify-between items-center text-sm">
                                        <span className="text-slate-500 dark:text-slate-400">{item.resource_count} items</span>
                                        <span className="font-bold text-green-600 dark:text-green-400">{formatCurrency(item.total_waste)}</span>
                                    </div>
                                    <ChevronRight className={`absolute right-4 top-1/2 -translate-y-1/2 w-5 h-5 text-slate-300 dark:text-slate-600 transition-transform ${selectedScan?.id === item.id ? 'translate-x-1 text-indigo-400' : ''}`} />
                                </div>
                            ))
                        )}
                    </div>
                </div>

                {/* Detail Panel */}
                <div className={`flex-1 flex flex-col bg-white dark:bg-slate-900 h-full ${!selectedScan ? 'hidden md:flex' : 'flex'}`}>
                    {selectedScan ? (
                        <>
                            <div className="p-4 border-b border-slate-200 dark:border-slate-800 flex justify-between items-center bg-slate-50/50 dark:bg-slate-900/50">
                                <div className="flex items-center gap-4">
                                    <button onClick={() => setSelectedScan(null)} className="md:hidden p-2 -ml-2 text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-full">
                                        <ChevronRight className="w-5 h-5 rotate-180" />
                                    </button>
                                    <div>
                                        <h3 className="text-lg font-bold text-slate-900 dark:text-white">Scan Details</h3>
                                        <p className="text-xs text-slate-500 dark:text-slate-400">{formatDateTime(normalizeScanTimestampMs(selectedScan.scanned_at))}</p>
                                    </div>
                                </div>
                                <div className="flex gap-2">
                                    <button 
                                        onClick={() => {
                                            setHistoryExportIncludeChecklistColumns(true);
                                            setPendingExportScan(selectedScan);
                                        }}
                                        disabled={exportingPdf}
                                        className="px-4 py-2 bg-indigo-50 dark:bg-indigo-900/20 text-indigo-700 dark:text-indigo-400 border border-indigo-100 dark:border-indigo-800 rounded-lg text-sm font-bold hover:bg-indigo-100 dark:hover:bg-indigo-900/40 flex items-center transition-colors"
                                    >
                                        <FileText className="w-4 h-4 mr-2" />
                                        {exportingPdf ? "Exporting..." : "Export PDF"}
                                    </button>
                                    <button onClick={() => setSelectedScan(null)} className="p-2 text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 hidden md:block">
                                        <X className="w-5 h-5" />
                                    </button>
                                </div>
                            </div>
                            <div className="flex-1 overflow-auto p-6 bg-slate-50 dark:bg-slate-900/50 space-y-6">
                                {/* 1. Summary Cards */}
                                <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
                                    <div className="bg-white dark:bg-slate-800 p-4 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
                                        <p className="text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-1">Potential Savings</p>
                                        <p className="text-2xl font-bold text-green-600 dark:text-green-400">{formatCurrency(selectedScan.total_waste)}<span className="text-sm text-slate-400 font-normal">/mo</span></p>
                                    </div>
                                    <div className="bg-white dark:bg-slate-800 p-4 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
                                        <p className="text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-1 flex items-center">
                                            <Leaf className="w-3 h-3 mr-1 text-teal-600 dark:text-teal-400" />
                                            Estimated CO2e Reduction
                                        </p>
                                        <p className="text-2xl font-bold text-teal-600 dark:text-teal-400">{formatCo2eKg(selectedCo2e.totalMonthlyCo2eKg)}<span className="text-sm text-slate-400 font-normal">/mo</span></p>
                                    </div>
                                    <div className="bg-white dark:bg-slate-800 p-4 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
                                        <p className="text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-1">Issues Found</p>
                                        <p className="text-2xl font-bold text-slate-900 dark:text-white">{selectedScan.resource_count}</p>
                                    </div>
                                    <div className="bg-white dark:bg-slate-800 p-4 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
                                        <p className="text-xs font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wider mb-1">Accounts Scanned</p>
                                        <p className="text-2xl font-bold text-indigo-600 dark:text-indigo-400">
                                            {selectedAccounts.length}
                                        </p>
                                    </div>
                                </div>
                                <div className="text-xs text-slate-500 dark:text-slate-400 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl px-4 py-3">
                                    Estimated annual impact: <span className="font-semibold text-slate-700 dark:text-slate-200">{formatCo2eTonsFromKg(selectedCo2e.totalAnnualCo2eKg)}</span>. {ESG_METHODOLOGY_NOTE} {ESG_DISCLAIMER_NOTE}
                                </div>

                                <div className="rounded-2xl border border-slate-200 bg-gradient-to-br from-slate-950 via-slate-900 to-indigo-950 px-5 py-5 text-white shadow-sm dark:border-slate-700">
                                    <p className="text-xs font-semibold uppercase tracking-[0.22em] text-indigo-200/80">Run Summary</p>
                                    <h4 className="mt-3 text-xl font-semibold">
                                        {selectedResources.length > 0
                                            ? `${selectedResources.length} findings worth about ${formatCurrency(selectedScan.total_waste)}/month were identified in this run.`
                                            : "This run completed without recorded findings."}
                                    </h4>
                                    <p className="mt-3 text-sm leading-7 text-slate-200">
                                        {providerComparison[0]
                                            ? `${providerComparison[0].provider} moved most versus the prior run, with a change of ${providerComparison[0].delta > 0 ? "+" : ""}${formatCurrency(providerComparison[0].delta)}.`
                                            : "No prior-run comparison is available yet for provider-level movement."}
                                    </p>
                                    <p className="mt-2 text-sm leading-7 text-slate-300">
                                        Recommended next step: export the packaged PDF if this run needs owner handoff, or compare the detailed findings below before closing the review.
                                    </p>
                                </div>

                                <div className="bg-white dark:bg-slate-800 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm p-4">
                                    <div className="flex flex-wrap items-start justify-between gap-4">
                                        <div>
                                            <h4 className="font-bold text-slate-900 dark:text-white">Comparison Versus Prior Stored Run</h4>
                                            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
                                                Quick change summary against the next older scan stored on this machine.
                                            </p>
                                        </div>
                                        <span className="text-xs uppercase tracking-[0.18em] text-slate-400 dark:text-slate-500">
                                            {previousScan ? formatDateTime(normalizeScanTimestampMs(previousScan.scanned_at)) : "No prior run"}
                                        </span>
                                    </div>
                                    <div className="mt-4 grid gap-3 md:grid-cols-3">
                                        <div className="rounded-xl bg-slate-50 px-4 py-3 dark:bg-slate-900/50">
                                            <p className="text-xs font-semibold uppercase tracking-[0.16em] text-slate-400 dark:text-slate-500">Potential Waste</p>
                                            <p className={`mt-2 text-xl font-bold ${typeof comparison.wasteDelta === "number" && comparison.wasteDelta > 0 ? "text-rose-600 dark:text-rose-300" : "text-slate-900 dark:text-white"}`}>
                                                {comparison.wasteDelta === null ? "No prior run" : `${comparison.wasteDelta > 0 ? "+" : ""}${formatCurrency(comparison.wasteDelta)}`}
                                            </p>
                                        </div>
                                        <div className="rounded-xl bg-slate-50 px-4 py-3 dark:bg-slate-900/50">
                                            <p className="text-xs font-semibold uppercase tracking-[0.16em] text-slate-400 dark:text-slate-500">Findings</p>
                                            <p className={`mt-2 text-xl font-bold ${typeof comparison.findingDelta === "number" && comparison.findingDelta > 0 ? "text-rose-600 dark:text-rose-300" : "text-slate-900 dark:text-white"}`}>
                                                {formatDelta(comparison.findingDelta)}
                                            </p>
                                        </div>
                                        <div className="rounded-xl bg-slate-50 px-4 py-3 dark:bg-slate-900/50">
                                            <p className="text-xs font-semibold uppercase tracking-[0.16em] text-slate-400 dark:text-slate-500">Accounts</p>
                                            <p className={`mt-2 text-xl font-bold ${typeof comparison.accountDelta === "number" && comparison.accountDelta > 0 ? "text-indigo-600 dark:text-indigo-300" : "text-slate-900 dark:text-white"}`}>
                                                {formatDelta(comparison.accountDelta)}
                                            </p>
                                        </div>
                                    </div>
                                </div>

                                <div className="bg-white dark:bg-slate-800 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm p-4">
                                    <div className="flex flex-wrap items-start justify-between gap-4">
                                        <div>
                                            <h4 className="font-bold text-slate-900 dark:text-white">Provider Shift Summary</h4>
                                            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
                                                Largest provider-level waste changes versus the previous stored run.
                                            </p>
                                        </div>
                                    </div>
                                    <div className="mt-4 grid gap-3 md:grid-cols-2">
                                        {!providerComparison.length ? (
                                            <p className="text-sm text-slate-500 dark:text-slate-400">No provider comparison is available yet.</p>
                                        ) : providerComparison.map((row) => (
                                            <div key={row.provider} className="rounded-xl bg-slate-50 px-4 py-3 dark:bg-slate-900/50">
                                                <p className="text-sm font-semibold text-slate-900 dark:text-white">{row.provider}</p>
                                                <p className={`mt-2 text-lg font-bold ${row.delta > 0 ? "text-rose-600 dark:text-rose-300" : "text-emerald-600 dark:text-emerald-300"}`}>
                                                    {row.delta > 0 ? "+" : ""}{formatCurrency(row.delta)}
                                                </p>
                                                <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                                                    Current {formatCurrency(row.current)} vs prior {formatCurrency(row.previous)}
                                                </p>
                                            </div>
                                        ))}
                                    </div>
                                </div>

                                {/* 2. Coverage & Scope */}
                                <div className="bg-white dark:bg-slate-800 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm overflow-hidden">
                                    <div className="p-4 border-b border-slate-100 dark:border-slate-700/50 bg-slate-50/50 dark:bg-slate-800/50">
                                        <h4 className="font-bold text-slate-900 dark:text-white flex items-center">
                                            <CheckCircle className="w-4 h-4 mr-2 text-indigo-500" /> Coverage Report
                                        </h4>
                                    </div>
                                    <div className="p-4">
                                        {selectedCoverageLoading ? (
                                            <div className="text-sm text-slate-500 dark:text-slate-400">Loading coverage checks...</div>
                                        ) : selectedCoverageRows.length === 0 ? (
                                            <div className="text-xs text-slate-400 italic">
                                                No coverage checks available for this scan record.
                                            </div>
                                        ) : (
                                            <div className="space-y-4">
                                                {Array.from(
                                                    selectedCoverageRows.reduce((acc, row) => {
                                                        const provider = row[0] || "Unknown Provider";
                                                        const check = row[1] || "Unknown Check";
                                                        const status = row[2] || "Clean";
                                                        if (!acc.has(provider)) acc.set(provider, []);
                                                        acc.get(provider)!.push([check, status] as [string, string]);
                                                        return acc;
                                                    }, new Map<string, Array<[string, string]>>())
                                                ).map(([provider, checks]) => (
                                                    <div key={provider} className="border border-slate-100 dark:border-slate-700 rounded-lg p-3">
                                                        <h5 className="font-bold text-sm text-slate-700 dark:text-slate-300 mb-2 border-b border-slate-100 dark:border-slate-700 pb-1">
                                                            {provider} Checks
                                                        </h5>
                                                        <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                                                            {checks.map(([check, status]) => {
                                                                const hasIssues = /\bissues?\b/i.test(status) && !/^0\b/.test(status.trim());
                                                                return (
                                                                    <div key={`${provider}-${check}`} className="flex justify-between items-center text-xs bg-slate-50 dark:bg-slate-900/50 p-2 rounded">
                                                                        <span className="text-slate-600 dark:text-slate-400">{check}</span>
                                                                        {hasIssues ? (
                                                                            <span className="text-red-500 font-bold text-xs flex items-center">
                                                                                <X className="w-3 h-3 mr-1" /> {status}
                                                                            </span>
                                                                        ) : (
                                                                            <span className="text-green-500 font-bold text-xs flex items-center">
                                                                                <CheckCircle className="w-3 h-3 mr-1" /> {status}
                                                                            </span>
                                                                        )}
                                                                    </div>
                                                                );
                                                            })}
                                                        </div>
                                                    </div>
                                                ))}
                                            </div>
                                        )}
                                    </div>
                                </div>

                                {/* 3. Detailed Findings */}
                                {selectedResources.length > 0 ? (
                                    <div className="bg-white dark:bg-slate-800 rounded-xl shadow-sm border border-slate-200 dark:border-slate-700 overflow-hidden">
                                        <div className="p-4 border-b border-slate-100 dark:border-slate-700/50 bg-slate-50/50 dark:bg-slate-800/50">
                                            <h4 className="font-bold text-slate-900 dark:text-white flex items-center">
                                                <Trash2 className="w-4 h-4 mr-2 text-red-500" /> Identified Waste
                                            </h4>
                                        </div>
                                        <table className="w-full text-left text-sm">
                                            <thead className="bg-slate-50 dark:bg-slate-700/50 text-slate-900 dark:text-white font-semibold border-b border-slate-200 dark:border-slate-700">
                                                <tr>
                                                    <th className="px-6 py-4">ID</th>
                                                    <th className="px-6 py-4">Provider</th>
                                                    <th className="px-6 py-4">Type</th>
                                                    <th className="px-6 py-4">Details</th>
                                                    <th className="px-6 py-4 text-right">Cost</th>
                                                    <th className="px-6 py-4 text-right">Est. CO2e/mo</th>
                                                </tr>
                                            </thead>
                                            <tbody className="divide-y divide-slate-100 dark:divide-slate-700/50">
                                                {selectedResources.map((r: WastedResource, i: number) => (
                                                    <tr key={i} className="hover:bg-slate-50 dark:hover:bg-slate-700/30 transition-colors">
                                                        <td className="px-6 py-4 font-mono text-xs text-slate-500 dark:text-slate-400 select-all">{r.id}</td>
                                                        <td className="px-6 py-4">
                                                            <span className={`px-2 py-1 rounded text-[10px] font-bold border ${
                                                                r.provider === "AWS" ? "bg-orange-50 dark:bg-orange-900/20 text-orange-700 dark:text-orange-400 border-orange-100 dark:border-orange-800" : 
                                                                r.provider === "Azure" ? "bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-400 border-blue-100 dark:border-blue-800" : 
                                                                "bg-slate-100 dark:bg-slate-700 text-slate-600 dark:text-slate-300 border-slate-200 dark:border-slate-600"
                                                            }`}>{r.provider}</span>
                                                        </td>
                                                        <td className="px-6 py-4 text-slate-900 dark:text-white font-medium">{r.resource_type}</td>
                                                        <td className="px-6 py-4 text-slate-500 dark:text-slate-400 max-w-xs truncate" title={r.details}>{r.details}</td>
                                                        <td className="px-6 py-4 text-right font-bold text-slate-900 dark:text-white">{formatCurrency(r.estimated_monthly_cost)}</td>
                                                        <td className="px-6 py-4 text-right font-semibold text-teal-700 dark:text-teal-300">{formatCo2eKg(estimateResourceCo2e(r).monthlyCo2eKg)}</td>
                                                    </tr>
                                                ))}
                                            </tbody>
                                        </table>
                                    </div>
                                ) : (
                                    <div className="p-8 text-center bg-white dark:bg-slate-800 rounded-xl border border-slate-200 dark:border-slate-700">
                                        <div className="w-16 h-16 bg-green-50 dark:bg-green-900/20 rounded-full flex items-center justify-center mx-auto mb-4">
                                            <CheckCircle className="w-8 h-8 text-green-500" />
                                        </div>
                                        <h3 className="font-bold text-slate-900 dark:text-white">Clean Bill of Health</h3>
                                        <p className="text-slate-500 dark:text-slate-400 text-sm mt-1">No wasted resources were identified in this scan.</p>
                                    </div>
                                )}
                            </div>
                        </>
                    ) : (
                        <div className="flex-1 flex flex-col items-center justify-center text-slate-400 dark:text-slate-500 bg-slate-50 dark:bg-slate-900/50">
                            <div className="w-24 h-24 bg-slate-100 dark:bg-slate-800 rounded-full flex items-center justify-center mb-4">
                                <History className="w-10 h-10 text-slate-300 dark:text-slate-600" />
                            </div>
                            <h3 className="text-lg font-medium text-slate-900 dark:text-white mb-1">Select a scan record</h3>
                            <p className="max-w-xs text-center">Click on any history item from the list to view its details and export reports.</p>
                        </div>
                    )}
                </div>
            </div>

            <Modal
                isOpen={pendingExportScan !== null}
                onClose={() => {
                    if (!exportingPdf) {
                        setPendingExportScan(null);
                    }
                }}
                title="Export PDF Options"
                footer={
                    <div className="flex gap-2">
                        <button
                            onClick={() => setPendingExportScan(null)}
                            disabled={exportingPdf}
                            className="px-4 py-2 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 rounded-lg font-medium disabled:opacity-50"
                        >
                            Cancel
                        </button>
                        <button
                            onClick={() => {
                                if (!pendingExportScan) return;
                                void handleExportPDF(pendingExportScan, historyExportIncludeChecklistColumns);
                            }}
                            disabled={exportingPdf || !pendingExportScan}
                            className="px-4 py-2 rounded-lg bg-indigo-600 text-white hover:bg-indigo-700 font-medium disabled:opacity-60"
                        >
                            {exportingPdf ? "Exporting..." : "Export PDF"}
                        </button>
                    </div>
                }
            >
                <div className="space-y-3">
                    <label className="flex items-start gap-2 rounded-xl border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-900 p-3">
                        <input
                            type="checkbox"
                            className="mt-0.5 h-4 w-4 accent-indigo-600"
                            checked={historyExportIncludeChecklistColumns}
                            onChange={(event) => setHistoryExportIncludeChecklistColumns(event.target.checked)}
                            disabled={exportingPdf}
                        />
                        <span className="text-sm text-slate-700 dark:text-slate-200">
                            Add checklist columns
                        </span>
                    </label>
                </div>
            </Modal>

            <Modal
                isOpen={pendingDeleteId !== null}
                onClose={() => {
                    if (!deleting) {
                        setPendingDeleteId(null);
                    }
                }}
                title="Delete History Record"
                footer={
                    <div className="flex gap-2">
                        <button
                            onClick={() => setPendingDeleteId(null)}
                            disabled={deleting}
                            className="px-4 py-2 text-slate-600 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 rounded-lg font-medium disabled:opacity-50"
                        >
                            Cancel
                        </button>
                        <button
                            onClick={confirmDelete}
                            disabled={deleting}
                            className="px-4 py-2 rounded-lg bg-rose-600 text-white hover:bg-rose-700 font-medium disabled:opacity-60"
                        >
                            {deleting ? "Deleting..." : "Delete"}
                        </button>
                    </div>
                }
            >
                <p className="text-sm text-slate-600 dark:text-slate-300">
                    Delete this history record from local storage? This cannot be undone.
                </p>
            </Modal>
        </div>
    );
}
