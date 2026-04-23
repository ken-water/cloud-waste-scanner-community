#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai_analyst_runtime;
mod aws_utils;
mod db;
mod demo_data;
mod governance_helpers;
mod license;
mod license_runtime;
mod local_api_runtime;
mod notification_runtime;
mod proxy_helpers;
mod proxy_runtime;
mod runtime_helpers;
mod scan_runtime;
mod schedule_runtime;

use cloud_waste_scanner_core::akamai::AkamaiScanner;
use cloud_waste_scanner_core::alibaba::AlibabaScanner;
use cloud_waste_scanner_core::azure::AzureScanner;
use cloud_waste_scanner_core::cloudflare::CloudflareScanner;
use cloud_waste_scanner_core::gcp::GcpScanner;
use cloud_waste_scanner_core::linode::LinodeScanner;
use cloud_waste_scanner_core::oracle::OracleScanner;
use cloud_waste_scanner_core::vultr::VultrScanner;
use cloud_waste_scanner_core::{
    akamai, backblaze, baidu, ceph, civo, cloudflare, cloudian, contabo, dell, dreamhost, equinix,
    exoscale, flashblade, gcore, generic_s3, greenlake, hcp, hetzner, huawei, ibm, idrive, ionos,
    leaseweb, linode, lyve, minio, models::WastedResource, nutanix, openstack, ovh, qumulo,
    rackspace, scaleway, scality, storagegrid, storj, tencent, tianyi, upcloud, volcengine, wasabi,
    NotificationChannel, Policy, ScanPolicy, Scanner,
};

use ai_analyst_runtime::{answer_local_question, AiAnalystLocalAnswer};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_cloudwatch::Client as CwClient;
use aws_sdk_ec2::{config::Region, Client as Ec2Client};
use aws_sdk_elasticloadbalancingv2::Client as ElbClient;
use aws_sdk_rds::Client as RdsClient;
use aws_sdk_s3::Client as S3Client;
use axum::{
    body::Body,
    extract::{ConnectInfo, DefaultBodyLimit, Path as AxumPath, Query, State},
    http::{header, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use base64::Engine as _;
use chrono::{TimeZone, Utc};
use futures_util::StreamExt;
use license::LicenseType;
use license_runtime::{
    fetch_runtime_license_policy, persist_runtime_plan_type_from_status, read_runtime_plan_type,
    resolve_effective_license_key_from_text,
};
use local_api_runtime::{list_schedules_sorted, prepare_job_queue_for_new_scan, remove_schedule};
use notification_runtime::{
    build_channel_routing_plan, evaluate_channel_dispatch, normalize_channel_for_save,
    normalize_notification_method_for_storage, validate_notification_method, ChannelSkipReason,
};
use rcgen::generate_simple_self_signed;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io::Write;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex as AsyncMutex, RwLock};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use governance_helpers::{
    classify_error_text_category, governance_error_category_catalog,
    governance_error_category_label, normalize_governance_account_label,
    normalize_governance_error_category_key, normalize_governance_provider,
    normalize_governance_window_days, parse_governance_error_bucket_counts, parse_meta_i64,
};
use proxy_helpers::{
    compose_proxy_url_from_parts, extract_aws_region_hint, mask_proxy_url,
    normalize_account_proxy_choice, normalize_proxy_mode, proxy_endpoint_display,
    proxy_scheme_from_url,
};
use proxy_runtime::{
    apply_proxy_choice_with_guard, apply_proxy_env_with_guard, configure_proxy_env,
    load_account_notification_assignments, load_account_proxy_assignments,
    normalize_custom_proxy_url, resolve_proxy_runtime,
};
use runtime_helpers::{
    build_runtime_entitlements,
    calculate_follow_up_next_run, calculate_initial_next_run,
    normalize_channel_min_findings_for_storage, normalize_channel_min_savings_for_storage,
    normalize_channel_trigger_mode_for_storage, normalize_enqueue_error_message,
    normalize_transport_error_detail, parse_notification_channel_email_recipients,
    resolve_effective_notification_trigger_mode, summarize_error_text, summarize_for_trial,
    trial_gate_message, validate_scan_request,
};
use scan_runtime::{
    build_aws_local_profile_map, compact_scan_error, filter_cloud_profiles_by_selection,
    push_credential_precheck_failure, resolve_aws_profiles_to_scan,
    summarize_credential_precheck_failures,
};
use schedule_runtime::{
    load_schedules_from_db as load_schedules_from_store,
    persist_schedules_to_db as persist_schedules_to_store,
};

struct AppState {
    db_path: std::path::PathBuf,
    pending_consume_lock: Arc<AsyncMutex<()>>,
}

#[derive(Clone)]
struct LocalApiState {
    app_handle: tauri::AppHandle,
    app_version: String,
    bind_host: String,
    port: u16,
    api_access_token: String,
    api_enabled: bool,
    tls_enabled: bool,
    jobs: Arc<RwLock<HashMap<String, ApiScanJob>>>,
    schedules: Arc<RwLock<HashMap<String, ApiSchedule>>>,
    auth_rate_buckets: Arc<RwLock<HashMap<String, VecDeque<i64>>>>,
    reports: Arc<RwLock<HashMap<String, ApiReportArtifactStored>>>,
    report_dir: std::path::PathBuf,
}

const API_MAX_REQUEST_BODY_BYTES: usize = 64 * 1024;
pub(crate) const API_MAX_SCAN_JOBS_ACTIVE: usize = 6;
pub(crate) const API_MAX_SCAN_JOBS_STORED: usize = 250;
const API_MAX_AUTH_FAILURES_PER_MIN: usize = 30;
const API_MIN_ACCESS_TOKEN_LEN: usize = 24;
const API_MAX_OPTIONAL_FIELD_LEN: usize = 256;
const API_MAX_LICENSE_KEY_LEN: usize = 1024;
const API_MAX_SELECTED_ACCOUNTS: usize = 64;
const API_MAX_ACCOUNT_ID_LEN: usize = 128;
const API_MAX_REPORT_EMAILS: usize = 5;
const API_MAX_EMAIL_LEN: usize = 254;
pub(crate) const API_RATE_LIMIT_PREFIX: &str = "rate_limited:";
const PENDING_CONSUME_SETTING_KEY: &str = "pending_license_consume_events";
const MAX_PENDING_CONSUME_EVENTS: usize = 64;
const MAX_PENDING_CONSUME_FLUSH_PER_PASS: u32 = 64;
const ACCOUNT_PROXY_ASSIGNMENTS_SETTING_KEY: &str = "account_proxy_assignments";
const ACCOUNT_NOTIFICATION_ASSIGNMENTS_SETTING_KEY: &str = "account_notification_assignments";
const API_WEBHOOKS_SETTING_KEY: &str = "api_webhooks_json";
const PROXY_CHOICE_GLOBAL: &str = "__global__";
const PROXY_CHOICE_DIRECT: &str = "__direct__";
const ACCOUNT_NOTIFICATION_CHOICE_ALL: &str = "__all_channels__";
const UPDATE_PROBE_RANGE: &str = "bytes=0-262143";
const UPDATE_PROBE_TIMEOUT_SECS: u64 = 6;
const UPDATE_PROBE_TOTAL_TIMEOUT_SECS: u64 = 6;
const UPDATE_PROBE_TARGET_SAMPLE_BYTES: usize = 64 * 1024;
const UPDATE_CONNECT_TIMEOUT_SECS: u64 = 15;
const UPDATE_DOWNLOAD_TIMEOUT_SECS: u64 = 900;
const UPDATE_UNKNOWN_TOTAL_FALLBACK_BYTES: u64 = 40 * 1024 * 1024;
const UPDATE_STREAM_POLL_TIMEOUT_SECS: u64 = 2;
const UPDATE_STREAM_STALL_FAILOVER_SECS: u64 = 14;
const UPDATE_STREAM_STALL_ABORT_SECS: u64 = 40;
const UPDATE_CANCEL_REASON: &str = "__update_canceled_by_user__";
const FIRST_PARTY_API_BASES: [&str; 0] = [];
const FIRST_PARTY_API_ROUTE_COOLDOWN_SECS: i64 = 45;
const NOTIFICATION_TRIGGER_MODE_SETTING_KEY: &str = "notification_trigger_mode";
const NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE: &str = "scan_complete";
const NOTIFICATION_TRIGGER_MODE_WASTE_ONLY: &str = "waste_only";
static UPDATE_DOWNLOAD_CANCELED: AtomicBool = AtomicBool::new(false);
static STARTUP_MONO: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
static FIRST_PARTY_API_ROUTE_COOLDOWN_UNTIL: std::sync::OnceLock<
    std::sync::Mutex<HashMap<&'static str, i64>>,
> = std::sync::OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingConsumeEvent {
    license_key: String,
    machine_id: String,
    created_at: i64,
    #[serde(default = "default_pending_scans")]
    pending_scans: u32,
    #[serde(default)]
    attempts: u32,
}

fn default_pending_scans() -> u32 {
    1
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub(crate) struct ApiScanRequest {
    pub(crate) license_key: Option<String>,
    pub(crate) aws_profile: Option<String>,
    pub(crate) aws_region: Option<String>,
    pub(crate) selected_accounts: Option<Vec<String>>,
    pub(crate) demo_mode: Option<bool>,
    pub(crate) report_emails: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
struct ApiScanAccepted {
    scan_id: String,
    status: String,
    message: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiScanListQuery {
    status: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiHistoryQuery {
    status: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct ApiCapabilityGroup {
    name: String,
    status: String,
    routes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiSettingsGeneralPatchRequest {
    currency: Option<String>,
    api_timeout_seconds: Option<u64>,
    notification_trigger_mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiSettingsNetworkPatchRequest {
    proxy_mode: Option<String>,
    proxy_url: Option<String>,
    api_bind_host: Option<String>,
    api_port: Option<u16>,
    api_tls_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiSettingsGlobalPolicyPatchRequest {
    cpu_percent: Option<f64>,
    network_mb: Option<f64>,
    lookback_days: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiSettingsScanPolicyPatchRequest {
    global: Option<ApiSettingsGlobalPolicyPatchRequest>,
    provider_policies: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct ApiCloudAccountRecord {
    id: String,
    provider: String,
    name: String,
    timeout_seconds: Option<i64>,
    policy_custom: Option<String>,
    proxy_profile_id: Option<String>,
    created_at: i64,
    credentials_masked: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiCloudAccountCreateRequest {
    provider: String,
    name: String,
    credentials: String,
    timeout_seconds: Option<i64>,
    policy_custom: Option<String>,
    proxy_profile_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiCloudAccountUpdateRequest {
    provider: Option<String>,
    name: Option<String>,
    credentials: Option<String>,
    timeout_seconds: Option<i64>,
    policy_custom: Option<String>,
    proxy_profile_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiCloudAccountTestRequest {
    provider: String,
    credentials: String,
    region: Option<String>,
    proxy_profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AiAnalystBreakdownRow {
    key: String,
    label: String,
    estimated_monthly_waste: f64,
    findings: i64,
    share_pct: f64,
    delta_monthly_waste: Option<f64>,
    delta_findings: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScanFindingAttribution {
    account_id: String,
    account_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AiAnalystSummary {
    window_days: i64,
    basis: String,
    latest_scan_id: Option<i64>,
    latest_scan_at: Option<i64>,
    scan_count_in_window: usize,
    total_monthly_waste: f64,
    total_findings: i64,
    previous_scan_id: Option<i64>,
    previous_scan_at: Option<i64>,
    previous_total_monthly_waste: Option<f64>,
    previous_total_findings: Option<i64>,
    delta_monthly_waste: Option<f64>,
    delta_findings: Option<i64>,
    scanned_accounts: Vec<String>,
    accounts: Vec<AiAnalystBreakdownRow>,
    providers: Vec<AiAnalystBreakdownRow>,
    resource_types: Vec<AiAnalystBreakdownRow>,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AiAnalystDrilldownRow {
    account_id: Option<String>,
    account_name: Option<String>,
    provider: String,
    region: String,
    resource_type: String,
    resource_id: String,
    details: String,
    action_type: String,
    estimated_monthly_waste: f64,
}

#[derive(Debug, Clone, Serialize)]
struct AiAnalystDrilldownResponse {
    window_days: i64,
    basis: String,
    latest_scan_id: Option<i64>,
    latest_scan_at: Option<i64>,
    dimension: String,
    selected_key: String,
    selected_label: String,
    total_monthly_waste: f64,
    total_findings: i64,
    rows: Vec<AiAnalystDrilldownRow>,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct EnrichedScanResult {
    id: String,
    provider: String,
    region: String,
    resource_type: String,
    details: String,
    estimated_monthly_cost: f64,
    action_type: String,
    account_id: Option<String>,
    account_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ApiProxyRecord {
    id: String,
    name: String,
    protocol: String,
    host: String,
    port: i64,
    auth_username: Option<String>,
    has_auth_password: bool,
    created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiProxyCreateRequest {
    name: String,
    protocol: String,
    host: String,
    port: i64,
    auth_username: Option<String>,
    auth_password: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiProxyUpdateRequest {
    name: Option<String>,
    protocol: Option<String>,
    host: Option<String>,
    port: Option<i64>,
    auth_username: Option<String>,
    auth_password: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiProxyTestRequest {
    proxy_mode: Option<String>,
    proxy_url: Option<String>,
    proxy_profile_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiNotificationChannelCreateRequest {
    id: Option<String>,
    name: String,
    method: String,
    config: String,
    is_active: Option<bool>,
    proxy_profile_id: Option<String>,
    trigger_mode: Option<String>,
    min_savings: Option<f64>,
    min_findings: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiNotificationChannelUpdateRequest {
    name: Option<String>,
    method: Option<String>,
    config: Option<String>,
    is_active: Option<bool>,
    proxy_profile_id: Option<String>,
    trigger_mode: Option<String>,
    min_savings: Option<f64>,
    min_findings: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiNotificationTestRequest {
    channel_id: Option<String>,
    channel: Option<ApiNotificationChannelCreateRequest>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiNotificationPolicyPatchRequest {
    notification_trigger_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApiScanJob {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) trigger_source: Option<String>,
    pub(crate) created_at: i64,
    pub(crate) started_at: Option<i64>,
    pub(crate) finished_at: Option<i64>,
    pub(crate) resources_found: Option<usize>,
    pub(crate) estimated_monthly_savings: Option<f64>,
    pub(crate) selected_accounts: Option<Vec<String>>,
    pub(crate) error: Option<String>,
    pub(crate) report_email_status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ApiScanProgressSnapshot {
    scan_id: String,
    status: String,
    phase: String,
    progress_percent: f64,
    indeterminate: bool,
    message: String,
    started_at: Option<i64>,
    finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct ApiScanHistorySummary {
    id: i64,
    scanned_at: i64,
    total_waste: f64,
    resource_count: i64,
    status: String,
    results_count: usize,
    scan_meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct ApiScanHistoryDetail {
    id: i64,
    scanned_at: i64,
    total_waste: f64,
    resource_count: i64,
    status: String,
    results: serde_json::Value,
    scan_meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiMarkHandledRequest {
    provider: Option<String>,
    note: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiGenerateReportRequest {
    format: Option<String>,
    scan_history_id: Option<i64>,
    include_esg: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiReportListQuery {
    format: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiGovernanceQuery {
    window_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct ApiReportArtifact {
    report_id: String,
    format: String,
    content_type: String,
    filename: String,
    size_bytes: u64,
    created_at: i64,
    scan_history_id: Option<i64>,
    include_esg: bool,
}

#[derive(Debug, Clone)]
struct ApiReportArtifactStored {
    meta: ApiReportArtifact,
    file_path: std::path::PathBuf,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiEventQuery {
    page: Option<i64>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiWebhookConfig {
    id: String,
    name: String,
    url: String,
    events: Vec<String>,
    is_active: bool,
    created_at: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ApiWebhookCreateRequest {
    name: Option<String>,
    url: String,
    events: Option<Vec<String>>,
    is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
struct ApiAccountSummary {
    id: String,
    provider: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
struct ApiScheduleRequest {
    name: Option<String>,
    enabled: Option<bool>,
    run_at: Option<i64>,
    interval_minutes: Option<i64>,
    timezone: Option<String>,
    scan: ApiScanRequest,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ApiSchedule {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) run_at: i64,
    pub(crate) interval_minutes: Option<i64>,
    pub(crate) timezone: Option<String>,
    pub(crate) next_run_at: Option<i64>,
    pub(crate) last_run_at: Option<i64>,
    pub(crate) last_scan_id: Option<String>,
    pub(crate) last_error: Option<String>,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) scan: ApiScanRequest,
}

type ApiError = (StatusCode, Json<serde_json::Value>);

fn api_error(status: StatusCode, message: impl Into<String>) -> ApiError {
    (status, Json(serde_json::json!({ "error": message.into() })))
}

#[derive(Debug, Serialize)]
struct ScanReportEmailPayload {
    license_key: String,
    to: Vec<String>,
    scan_id: String,
    summary: String,
    resources_found: usize,
    total_savings: f64,
    findings: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct TrialStartResult {
    status: String,
    plan_type: String,
    trial_expires_at: Option<i64>,
    quota: Option<i64>,
    max_quota: Option<i64>,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct NotificationTestDiagnostics {
    ok: bool,
    channel_name: String,
    channel_method: String,
    app_version: String,
    proxy_mode: String,
    proxy_profile_id: Option<String>,
    proxy_scheme: Option<String>,
    proxy_url_masked: Option<String>,
    stage: String,
    reason_code: String,
    message: String,
    http_status: Option<u16>,
    duration_ms: u64,
    tested_at: i64,
    trace: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CloudImportCandidate {
    id: String,
    provider: String,
    name: String,
    credentials: String,
    region: Option<String>,
    source: String,
    import_kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct GovernanceScorecard {
    scan_runs: usize,
    findings: usize,
    positive_scan_runs: usize,
    positive_scan_rate_pct: f64,
    identified_savings: f64,
    estimated_co2e_kg_monthly: f64,
    avg_savings_per_scan: f64,
    avg_findings_per_scan: f64,
    active_accounts: usize,
    active_providers: usize,
    scan_checks_attempted: i64,
    scan_checks_succeeded: i64,
    scan_checks_failed: i64,
    scan_check_success_rate_pct: f64,
    last_scan_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct GovernanceDailyPoint {
    day_ts: i64,
    day_label: String,
    day_date: String,
    scan_runs: usize,
    positive_scan_runs: usize,
    findings: usize,
    savings: f64,
    estimated_co2e_kg_monthly: f64,
    scan_checks_attempted: i64,
    scan_checks_succeeded: i64,
    scan_checks_failed: i64,
    check_success_rate_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
struct GovernanceProviderRow {
    provider: String,
    scan_runs: usize,
    findings: usize,
    savings: f64,
    estimated_co2e_kg_monthly: f64,
    positive_scan_runs: usize,
}

#[derive(Debug, Clone, Serialize)]
struct GovernanceAccountRow {
    account: String,
    scan_runs: usize,
    coverage_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
struct GovernanceErrorCategoryRow {
    category: String,
    label: String,
    count: i64,
    ratio_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
struct GovernanceErrorTaxonomy {
    taxonomy_version: String,
    total_failed_checks: i64,
    categories: Vec<GovernanceErrorCategoryRow>,
}

#[derive(Debug, Clone, Serialize)]
struct GovernanceStatsResponse {
    generated_at: i64,
    window_days: i64,
    window_start_ts: i64,
    window_end_ts: i64,
    scorecard: GovernanceScorecard,
    daily: Vec<GovernanceDailyPoint>,
    providers: Vec<GovernanceProviderRow>,
    accounts: Vec<GovernanceAccountRow>,
    error_taxonomy: GovernanceErrorTaxonomy,
}

#[derive(Debug, Clone, Serialize)]
struct GovernanceTrendResponse {
    generated_at: i64,
    window_days: i64,
    window_start_ts: i64,
    window_end_ts: i64,
    daily: Vec<GovernanceDailyPoint>,
}

#[derive(Debug, Clone, Serialize)]
struct GovernanceErrorTaxonomyResponse {
    generated_at: i64,
    window_days: i64,
    window_start_ts: i64,
    window_end_ts: i64,
    error_taxonomy: GovernanceErrorTaxonomy,
}

#[derive(Debug, Default, Clone)]
struct GovernanceDailyAgg {
    scan_runs: usize,
    positive_scan_runs: usize,
    findings: usize,
    savings: f64,
    scan_checks_attempted: i64,
    scan_checks_succeeded: i64,
    scan_checks_failed: i64,
}

#[derive(Debug, Default, Clone)]
struct GovernanceProviderAgg {
    scan_runs: usize,
    findings: usize,
    savings: f64,
    positive_scan_runs: usize,
}

fn read_env_trimmed(keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn sanitize_import_id_part(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
}

fn push_import_candidate(
    candidates: &mut Vec<CloudImportCandidate>,
    seen: &mut HashSet<String>,
    provider: &str,
    name: String,
    credentials: String,
    region: Option<String>,
    source: String,
    import_kind: &str,
) {
    let normalized_name = name.trim();
    if normalized_name.is_empty() {
        return;
    }
    let normalized_provider = provider.trim().to_lowercase();
    let dedupe_key = format!(
        "{}|{}|{}|{}",
        import_kind,
        normalized_provider,
        source.to_lowercase(),
        normalized_name.to_lowercase()
    );
    if !seen.insert(dedupe_key) {
        return;
    }

    let id = format!(
        "{}_{}_{}_{}",
        sanitize_import_id_part(import_kind),
        sanitize_import_id_part(&normalized_provider),
        sanitize_import_id_part(&source),
        sanitize_import_id_part(normalized_name)
    );
    candidates.push(CloudImportCandidate {
        id,
        provider: normalized_provider,
        name: normalized_name.to_string(),
        credentials,
        region,
        source,
        import_kind: import_kind.to_string(),
    });
}

fn read_file_if_exists(path: &std::path::Path) -> Option<String> {
    if !path.exists() || !path.is_file() {
        return None;
    }
    std::fs::read_to_string(path)
        .ok()
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty())
}

fn load_gcp_adc_json_candidates() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();

    if let Some(path_str) = read_env_trimmed(&["GOOGLE_APPLICATION_CREDENTIALS"]) {
        let path = std::path::PathBuf::from(path_str.clone());
        if let Some(content) = read_file_if_exists(&path) {
            out.push((
                format!("env:GOOGLE_APPLICATION_CREDENTIALS ({})", path_str),
                content,
            ));
        }
    }

    if let Some(raw_json) =
        read_env_trimmed(&["GOOGLE_SERVICE_ACCOUNT_JSON", "GCP_SERVICE_ACCOUNT_JSON"])
    {
        out.push(("env:GCP_SERVICE_ACCOUNT_JSON".to_string(), raw_json));
    }

    if let Some(config_dir) = dirs::config_dir() {
        let adc_path = config_dir
            .join("gcloud")
            .join("application_default_credentials.json");
        if let Some(content) = read_file_if_exists(&adc_path) {
            out.push((format!("file:{}", adc_path.display()), content));
        }
    }

    out
}

fn now_unix_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn round_two(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn generate_demo_governance_history(window_days: Option<i64>) -> Vec<db::ScanHistoryItem> {
    governance_helpers::generate_demo_governance_history(window_days, now_unix_ts())
}

fn push_notification_trace(
    trace: &mut Vec<String>,
    started: &std::time::Instant,
    message: impl Into<String>,
) {
    trace.push(format!(
        "{} ms | {}",
        started.elapsed().as_millis(),
        message.into()
    ));
}

fn summarize_trace_entries(trace: &[String], max_entries: usize) -> String {
    if trace.is_empty() {
        return "-".to_string();
    }
    if trace.len() <= max_entries {
        return trace.join(" || ");
    }
    let mut entries = trace.iter().take(max_entries).cloned().collect::<Vec<_>>();
    entries.push(format!("... +{} more", trace.len() - max_entries));
    entries.join(" || ")
}

fn extract_notification_probe_url(channel: &NotificationChannel) -> Option<String> {
    match channel.method.as_str() {
        "telegram" => Some("https://api.telegram.org".to_string()),
        "whatsapp" => Some("https://graph.facebook.com".to_string()),
        _ => {
            let parsed = serde_json::from_str::<serde_json::Value>(&channel.config).ok()?;
            parsed
                .get("url")
                .and_then(|value| value.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        }
    }
}

fn render_probe_origin(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(parsed) => {
            let host = parsed.host_str().unwrap_or("-");
            let scheme = parsed.scheme();
            let port = parsed.port().map(|p| format!(":{}", p)).unwrap_or_default();
            format!("{}://{}{}", scheme, host, port)
        }
        Err(_) => url.to_string(),
    }
}

fn extract_probe_host_port(url: &str) -> Option<(String, u16)> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_string();
    let port = parsed.port_or_known_default().unwrap_or(443);
    Some((host, port))
}

fn socks5_reply_code_message(code: u8) -> &'static str {
    match code {
        0x00 => "succeeded",
        0x01 => "general SOCKS server failure",
        0x02 => "connection not allowed by ruleset",
        0x03 => "network unreachable",
        0x04 => "host unreachable",
        0x05 => "connection refused",
        0x06 => "TTL expired",
        0x07 => "command not supported",
        0x08 => "address type not supported",
        _ => "unknown SOCKS reply code",
    }
}

async fn socks5_tunnel_probe(
    proxy_url: &str,
    target_host: &str,
    target_port: u16,
) -> Result<(), String> {
    let parsed = reqwest::Url::parse(proxy_url)
        .map_err(|e| format!("invalid proxy URL for SOCKS probe: {}", e))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "socks5" && scheme != "socks5h" {
        return Err(format!(
            "SOCKS probe skipped: unsupported proxy scheme {}",
            scheme
        ));
    }

    let proxy_host = parsed
        .host_str()
        .ok_or_else(|| "SOCKS probe: proxy host missing".to_string())?;
    let proxy_port = parsed
        .port_or_known_default()
        .ok_or_else(|| "SOCKS probe: proxy port missing".to_string())?;
    let username = parsed.username().to_string();
    let password = parsed.password().unwrap_or_default().to_string();
    let use_remote_dns = scheme == "socks5h";

    let mut stream = tokio::time::timeout(
        std::time::Duration::from_secs(4),
        tokio::net::TcpStream::connect((proxy_host, proxy_port)),
    )
    .await
    .map_err(|_| "SOCKS probe timeout: cannot connect to proxy endpoint".to_string())?
    .map_err(|e| format!("SOCKS probe: proxy TCP connect failed: {}", e))?;

    let methods: Vec<u8> = if username.is_empty() {
        vec![0x00]
    } else {
        vec![0x00, 0x02]
    };
    let mut hello = vec![0x05, methods.len() as u8];
    hello.extend_from_slice(&methods);
    stream
        .write_all(&hello)
        .await
        .map_err(|e| format!("SOCKS probe: failed to send greeting: {}", e))?;

    let mut method_resp = [0_u8; 2];
    stream
        .read_exact(&mut method_resp)
        .await
        .map_err(|e| format!("SOCKS probe: failed to read greeting response: {}", e))?;
    if method_resp[0] != 0x05 {
        return Err(format!(
            "SOCKS probe: invalid greeting version {}",
            method_resp[0]
        ));
    }

    match method_resp[1] {
        0x00 => {}
        0x02 => {
            let uname = username.as_bytes();
            let pass = password.as_bytes();
            if uname.is_empty() || uname.len() > 255 || pass.len() > 255 {
                return Err(
                    "SOCKS probe: invalid username/password length for proxy auth".to_string(),
                );
            }
            let mut auth_req = Vec::with_capacity(3 + uname.len() + pass.len());
            auth_req.push(0x01);
            auth_req.push(uname.len() as u8);
            auth_req.extend_from_slice(uname);
            auth_req.push(pass.len() as u8);
            auth_req.extend_from_slice(pass);
            stream
                .write_all(&auth_req)
                .await
                .map_err(|e| format!("SOCKS probe: failed to send auth request: {}", e))?;
            let mut auth_resp = [0_u8; 2];
            stream
                .read_exact(&mut auth_resp)
                .await
                .map_err(|e| format!("SOCKS probe: failed to read auth response: {}", e))?;
            if auth_resp[1] != 0x00 {
                return Err(format!(
                    "SOCKS probe: proxy auth failed (status={})",
                    auth_resp[1]
                ));
            }
        }
        0xFF => {
            return Err("SOCKS probe: proxy rejected all auth methods".to_string());
        }
        other => {
            return Err(format!(
                "SOCKS probe: unsupported auth method selected by proxy ({})",
                other
            ));
        }
    }

    let mut connect_req = Vec::new();
    connect_req.extend_from_slice(&[0x05, 0x01, 0x00]);
    if use_remote_dns {
        let host_bytes = target_host.as_bytes();
        if host_bytes.is_empty() || host_bytes.len() > 255 {
            return Err("SOCKS probe: target host length is invalid".to_string());
        }
        connect_req.push(0x03);
        connect_req.push(host_bytes.len() as u8);
        connect_req.extend_from_slice(host_bytes);
    } else {
        let mut resolved = tokio::net::lookup_host((target_host, target_port))
            .await
            .map_err(|e| format!("SOCKS probe: local DNS failed for {}: {}", target_host, e))?;
        let addr = resolved
            .next()
            .ok_or_else(|| format!("SOCKS probe: no DNS answers for {}", target_host))?;
        match addr.ip() {
            std::net::IpAddr::V4(v4) => {
                connect_req.push(0x01);
                connect_req.extend_from_slice(&v4.octets());
            }
            std::net::IpAddr::V6(v6) => {
                connect_req.push(0x04);
                connect_req.extend_from_slice(&v6.octets());
            }
        }
    }
    connect_req.push((target_port >> 8) as u8);
    connect_req.push((target_port & 0xFF) as u8);
    stream
        .write_all(&connect_req)
        .await
        .map_err(|e| format!("SOCKS probe: failed to send CONNECT request: {}", e))?;

    let mut connect_resp_head = [0_u8; 4];
    stream
        .read_exact(&mut connect_resp_head)
        .await
        .map_err(|e| format!("SOCKS probe: failed to read CONNECT response header: {}", e))?;
    if connect_resp_head[0] != 0x05 {
        return Err(format!(
            "SOCKS probe: invalid CONNECT response version {}",
            connect_resp_head[0]
        ));
    }
    if connect_resp_head[1] != 0x00 {
        return Err(format!(
            "SOCKS probe: CONNECT rejected (code={} {})",
            connect_resp_head[1],
            socks5_reply_code_message(connect_resp_head[1])
        ));
    }

    let atyp = connect_resp_head[3];
    let addr_len = match atyp {
        0x01 => 4,
        0x04 => 16,
        0x03 => {
            let mut len_buf = [0_u8; 1];
            stream
                .read_exact(&mut len_buf)
                .await
                .map_err(|e| format!("SOCKS probe: failed to read domain length: {}", e))?;
            len_buf[0] as usize
        }
        _ => {
            return Err(format!(
                "SOCKS probe: unsupported bind address type {}",
                atyp
            ))
        }
    };
    let mut discard = vec![0_u8; addr_len + 2];
    stream
        .read_exact(&mut discard)
        .await
        .map_err(|e| format!("SOCKS probe: failed to read bind address: {}", e))?;

    Ok(())
}

async fn probe_notification_target(
    proxy_mode: &str,
    proxy_url: &str,
    target_url: &str,
) -> Result<u16, String> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(4))
        .timeout(std::time::Duration::from_secs(8));

    if proxy_mode == "custom" {
        let proxy = reqwest::Proxy::all(proxy_url)
            .map_err(|e| format!("Invalid proxy configuration for probe: {}", e))?;
        builder = builder.proxy(proxy);
    } else if proxy_mode == "none" {
        builder = builder.no_proxy();
    }

    let client = builder
        .build()
        .map_err(|e| format!("Failed to build probe HTTP client: {}", e))?;

    client
        .head(target_url)
        .send()
        .await
        .map(|resp| resp.status().as_u16())
        .map_err(|e| e.to_string())
}

fn map_scan_enqueue_error(err: String) -> ApiError {
    if let Some(message) = err.strip_prefix(API_RATE_LIMIT_PREFIX) {
        api_error(StatusCode::TOO_MANY_REQUESTS, message.trim())
    } else {
        api_error(StatusCode::BAD_REQUEST, err)
    }
}

async fn allow_local_rate_limit(
    buckets: &Arc<RwLock<HashMap<String, VecDeque<i64>>>>,
    key: &str,
    window_secs: i64,
    max_hits: usize,
) -> bool {
    let now = now_unix_ts();
    let mut map = buckets.write().await;
    let queue = map.entry(key.to_string()).or_default();
    while let Some(ts) = queue.front().copied() {
        if now - ts > window_secs {
            let _ = queue.pop_front();
        } else {
            break;
        }
    }
    if queue.len() >= max_hits {
        return false;
    }
    queue.push_back(now);
    true
}

fn resolve_effective_license_key(app_handle: &tauri::AppHandle) -> Result<String, String> {
    let local_key = load_license_file(app_handle.clone())?;
    resolve_effective_license_key_from_text(&local_key)
}

async fn persist_schedules_to_db(state: &LocalApiState) -> Result<(), String> {
    let app_state = state.app_handle.state::<AppState>();
    let snapshot: Vec<ApiSchedule> = {
        let schedules = state.schedules.read().await;
        schedules.values().cloned().collect()
    };
    persist_schedules_to_store(&app_state.db_path, &snapshot).await
}

async fn load_schedules_from_db(
    state: &LocalApiState,
) -> Result<HashMap<String, ApiSchedule>, String> {
    let app_state = state.app_handle.state::<AppState>();
    load_schedules_from_store(&app_state.db_path).await
}

async fn enqueue_scan_job(
    state: &LocalApiState,
    payload: ApiScanRequest,
    trigger_source: Option<String>,
) -> Result<ApiScanAccepted, String> {
    validate_scan_request(&payload)?;

    let demo_mode = payload.demo_mode.unwrap_or(false);
    let scan_id = Uuid::new_v4().to_string();
    let created_at = now_unix_ts();

    {
        let mut jobs = state.jobs.write().await;
        prepare_job_queue_for_new_scan(&mut jobs)?;

        jobs.insert(
            scan_id.clone(),
            ApiScanJob {
                id: scan_id.clone(),
                status: "queued".to_string(),
                trigger_source: trigger_source.clone(),
                created_at,
                started_at: None,
                finished_at: None,
                resources_found: None,
                estimated_monthly_savings: None,
                selected_accounts: payload.selected_accounts.clone(),
                error: None,
                report_email_status: None,
            },
        );
    }

    let task_state = state.clone();
    let scan_id_for_task = scan_id.clone();
    tauri::async_runtime::spawn(async move {
        {
            let mut jobs = task_state.jobs.write().await;
            if let Some(job) = jobs.get_mut(&scan_id_for_task) {
                job.status = "running".to_string();
                job.started_at = Some(now_unix_ts());
            }
        }

        let resolved_license_key = if demo_mode {
            None
        } else {
            match resolve_effective_license_key(&task_state.app_handle) {
                Ok(key) => Some(key),
                Err(err) => {
                    let mut jobs = task_state.jobs.write().await;
                    if let Some(job) = jobs.get_mut(&scan_id_for_task) {
                        job.status = "failed".to_string();
                        job.finished_at = Some(now_unix_ts());
                        job.error = Some(err);
                    }
                    return;
                }
            }
        };

        let scan_result = run_scan(
            task_state.app_handle.clone(),
            resolved_license_key.clone(),
            payload.aws_profile.clone(),
            payload.aws_region.clone(),
            payload.selected_accounts.clone(),
            demo_mode,
        )
        .await;

        match scan_result {
            Ok(results) => {
                let total_savings: f64 = results.iter().map(|r| r.estimated_monthly_cost).sum();
                let resources_found = results.len();
                let report_email_status = if let Some(recipients) = payload.report_emails.clone() {
                    if recipients.is_empty() {
                        Some("skipped: empty report_emails".to_string())
                    } else if demo_mode {
                        Some("skipped: demo_mode enabled".to_string())
                    } else if let Some(key) = resolved_license_key.as_deref() {
                        match send_scan_report_email(key, &recipients, &scan_id_for_task, &results)
                            .await
                        {
                            Ok(_) => Some("sent".to_string()),
                            Err(err) => Some(format!("failed: {}", err)),
                        }
                    } else {
                        Some("skipped: no active local license".to_string())
                    }
                } else {
                    None
                };

                let mut jobs = task_state.jobs.write().await;
                if let Some(job) = jobs.get_mut(&scan_id_for_task) {
                    job.status = "completed".to_string();
                    job.finished_at = Some(now_unix_ts());
                    job.resources_found = Some(resources_found);
                    job.estimated_monthly_savings = Some(total_savings);
                    job.report_email_status = report_email_status;
                }
                drop(jobs);
                dispatch_scan_webhooks(
                    &task_state,
                    &scan_id_for_task,
                    "completed",
                    Some(resources_found),
                    Some(total_savings),
                    None,
                )
                .await;
            }
            Err(err) => {
                let error_message = err.clone();
                let mut jobs = task_state.jobs.write().await;
                if let Some(job) = jobs.get_mut(&scan_id_for_task) {
                    job.status = "failed".to_string();
                    job.finished_at = Some(now_unix_ts());
                    job.error = Some(err);
                }
                drop(jobs);
                dispatch_scan_webhooks(
                    &task_state,
                    &scan_id_for_task,
                    "failed",
                    None,
                    None,
                    Some(error_message),
                )
                .await;
            }
        }
    });

    Ok(ApiScanAccepted {
        scan_id,
        status: "queued".to_string(),
        message: "Scan job accepted".to_string(),
    })
}

async fn run_due_schedule(state: &LocalApiState, schedule_id: &str) {
    let now = now_unix_ts();

    let schedule_to_run = {
        let mut schedules = state.schedules.write().await;
        let Some(schedule) = schedules.get_mut(schedule_id) else {
            return;
        };

        if !schedule.enabled {
            return;
        }

        let Some(next_run_at) = schedule.next_run_at else {
            return;
        };

        if next_run_at > now {
            return;
        }

        schedule.last_run_at = Some(now);
        schedule.updated_at = now;
        schedule.last_error = None;
        schedule.next_run_at =
            calculate_follow_up_next_run(next_run_at, schedule.interval_minutes, now);
        if schedule.next_run_at.is_none() {
            schedule.enabled = false;
        }

        schedule.clone()
    };

    if let Err(err) = persist_schedules_to_db(state).await {
        eprintln!("Failed to persist schedules before run: {}", err);
    }

    let source = Some(format!("schedule:{}", schedule_id));
    let trigger_result = enqueue_scan_job(state, schedule_to_run.scan.clone(), source).await;

    {
        let mut schedules = state.schedules.write().await;
        if let Some(schedule) = schedules.get_mut(schedule_id) {
            match trigger_result {
                Ok(accepted) => {
                    schedule.last_scan_id = Some(accepted.scan_id);
                    schedule.last_error = None;
                }
                Err(err) => {
                    schedule.last_error = Some(normalize_enqueue_error_message(&err));
                }
            }
            schedule.updated_at = now_unix_ts();
        }
    }

    if let Err(err) = persist_schedules_to_db(state).await {
        eprintln!("Failed to persist schedules after run: {}", err);
    }
}

async fn scheduler_loop(state: LocalApiState) {
    loop {
        let now = now_unix_ts();
        let due_ids: Vec<String> = {
            let schedules = state.schedules.read().await;
            schedules
                .iter()
                .filter_map(|(id, schedule)| {
                    if schedule.enabled
                        && schedule
                            .next_run_at
                            .map(|next| next <= now)
                            .unwrap_or(false)
                    {
                        Some(id.clone())
                    } else {
                        None
                    }
                })
                .collect()
        };

        for schedule_id in due_ids {
            run_due_schedule(&state, &schedule_id).await;
        }

        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
    }
}

async fn send_scan_report_email(
    license_key: &str,
    to: &[String],
    scan_id: &str,
    results: &[WastedResource],
) -> Result<(), String> {
    let recipients: Vec<String> = to
        .iter()
        .map(|email| email.trim().to_string())
        .filter(|email| !email.is_empty())
        .collect();

    if recipients.is_empty() {
        return Err("No valid report recipients provided".to_string());
    }

    let total_savings: f64 = results.iter().map(|r| r.estimated_monthly_cost).sum();
    let findings: Vec<serde_json::Value> = results
        .iter()
        .take(50)
        .map(|item| {
            serde_json::json!({
                "provider": item.provider,
                "region": item.region,
                "resource_type": item.resource_type,
                "details": item.details,
                "estimated_monthly_cost": item.estimated_monthly_cost,
                "action_type": item.action_type,
            })
        })
        .collect();

    let payload = ScanReportEmailPayload {
        license_key: license_key.to_string(),
        to: recipients,
        scan_id: scan_id.to_string(),
        summary: format!(
            "Scheduled scan {} completed with {} findings and ${:.2}/mo potential savings.",
            scan_id,
            results.len(),
            total_savings
        ),
        resources_found: results.len(),
        total_savings,
        findings,
    };

    let response = post_first_party_api_json(
        "/api/send-scan-report",
        &serde_json::to_value(&payload).map_err(|e| e.to_string())?,
        std::time::Duration::from_secs(3),
        std::time::Duration::from_secs(10),
    )
    .await
    .map_err(|e| format!("Failed to call report API: {}", e))?;

    if !response.status().is_success() {
        let code = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Report API returned {}: {}", code, body));
    }

    Ok(())
}

#[derive(Deserialize)]
struct AzureCreds {
    subscription_id: String,
    tenant_id: String,
    client_id: String,
    client_secret: String,
}

fn to_str_err<T, E: std::fmt::Display>(res: Result<T, E>) -> Result<T, String> {
    res.map_err(|e| e.to_string())
}

fn set_or_remove_env(key: &str, value: Option<&str>) {
    if let Some(v) = value {
        std::env::set_var(key, v);
    } else {
        std::env::remove_var(key);
    }
}

#[derive(Debug, Clone)]
struct AwsEnvSnapshot {
    profile: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    session_token: Option<String>,
    security_token: Option<String>,
    region: Option<String>,
    default_region: Option<String>,
}

struct AwsEnvGuard {
    snapshot: AwsEnvSnapshot,
    _lock_guard: tokio::sync::OwnedMutexGuard<()>,
}

impl Drop for AwsEnvGuard {
    fn drop(&mut self) {
        restore_aws_env(&self.snapshot);
    }
}

fn capture_aws_env() -> AwsEnvSnapshot {
    AwsEnvSnapshot {
        profile: std::env::var("AWS_PROFILE").ok(),
        access_key_id: std::env::var("AWS_ACCESS_KEY_ID").ok(),
        secret_access_key: std::env::var("AWS_SECRET_ACCESS_KEY").ok(),
        session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
        security_token: std::env::var("AWS_SECURITY_TOKEN").ok(),
        region: std::env::var("AWS_REGION").ok(),
        default_region: std::env::var("AWS_DEFAULT_REGION").ok(),
    }
}

fn restore_aws_env(snapshot: &AwsEnvSnapshot) {
    set_or_remove_env("AWS_PROFILE", snapshot.profile.as_deref());
    set_or_remove_env("AWS_ACCESS_KEY_ID", snapshot.access_key_id.as_deref());
    set_or_remove_env(
        "AWS_SECRET_ACCESS_KEY",
        snapshot.secret_access_key.as_deref(),
    );
    set_or_remove_env("AWS_SESSION_TOKEN", snapshot.session_token.as_deref());
    set_or_remove_env("AWS_SECURITY_TOKEN", snapshot.security_token.as_deref());
    set_or_remove_env("AWS_REGION", snapshot.region.as_deref());
    set_or_remove_env("AWS_DEFAULT_REGION", snapshot.default_region.as_deref());
}

async fn apply_aws_env_with_guard(
    profile: Option<&str>,
    access_key_id: Option<&str>,
    secret_access_key: Option<&str>,
    region: Option<&str>,
) -> AwsEnvGuard {
    static AWS_ENV_LOCK: std::sync::OnceLock<std::sync::Arc<tokio::sync::Mutex<()>>> =
        std::sync::OnceLock::new();
    let lock = AWS_ENV_LOCK
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let lock_guard = lock.lock_owned().await;

    let snapshot = capture_aws_env();

    std::env::remove_var("AWS_PROFILE");
    std::env::remove_var("AWS_ACCESS_KEY_ID");
    std::env::remove_var("AWS_SECRET_ACCESS_KEY");
    std::env::remove_var("AWS_SESSION_TOKEN");
    std::env::remove_var("AWS_SECURITY_TOKEN");
    std::env::remove_var("AWS_REGION");
    std::env::remove_var("AWS_DEFAULT_REGION");

    set_or_remove_env("AWS_PROFILE", profile);
    set_or_remove_env("AWS_ACCESS_KEY_ID", access_key_id);
    set_or_remove_env("AWS_SECRET_ACCESS_KEY", secret_access_key);
    set_or_remove_env("AWS_REGION", region);
    set_or_remove_env("AWS_DEFAULT_REGION", region);

    AwsEnvGuard {
        snapshot,
        _lock_guard: lock_guard,
    }
}

async fn precheck_proxy_connectivity(proxy_url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(proxy_url).map_err(|_| {
        "Invalid proxy URL format. Use a full URL such as http://127.0.0.1:7890".to_string()
    })?;

    let host = parsed
        .host_str()
        .ok_or_else(|| "Proxy URL is missing host".to_string())?;
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| "Proxy URL is missing port".to_string())?;

    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect((host, port)),
    )
    .await
    {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(err)) => {
            let normalized = normalize_transport_error_detail(&err.to_string());
            Err(format!(
                "Cannot connect to proxy endpoint {}:{} ({})",
                host, port, normalized
            ))
        }
        Err(_) => Err(format!(
            "Timed out while connecting to proxy endpoint {}:{}",
            host, port
        )),
    }
}

fn first_party_route_cooldown_map() -> &'static std::sync::Mutex<HashMap<&'static str, i64>> {
    FIRST_PARTY_API_ROUTE_COOLDOWN_UNTIL.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn mark_first_party_route_failed(base: &'static str) {
    if let Ok(mut map) = first_party_route_cooldown_map().lock() {
        map.insert(base, now_unix_ts() + FIRST_PARTY_API_ROUTE_COOLDOWN_SECS);
    }
}

fn mark_first_party_route_healthy(base: &'static str) {
    if let Ok(mut map) = first_party_route_cooldown_map().lock() {
        map.remove(base);
    }
}

fn ordered_first_party_api_bases() -> Vec<&'static str> {
    let now = now_unix_ts();
    let mut preferred = Vec::new();
    let mut cooling_down = Vec::new();
    if let Ok(map) = first_party_route_cooldown_map().lock() {
        for base in FIRST_PARTY_API_BASES {
            let cooldown_until = map.get(base).copied().unwrap_or(0);
            if cooldown_until > now {
                cooling_down.push(base);
            } else {
                preferred.push(base);
            }
        }
    } else {
        preferred.extend(FIRST_PARTY_API_BASES);
    }
    preferred.extend(cooling_down);
    if preferred.is_empty() {
        FIRST_PARTY_API_BASES.to_vec()
    } else {
        preferred
    }
}

fn should_retry_first_party_status(status: reqwest::StatusCode) -> bool {
    status.is_server_error()
        || status == reqwest::StatusCode::FORBIDDEN
        || status == reqwest::StatusCode::NOT_FOUND
        || status == reqwest::StatusCode::METHOD_NOT_ALLOWED
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
}

async fn post_first_party_api_json(
    path: &str,
    payload: &serde_json::Value,
    connect_timeout: std::time::Duration,
    timeout: std::time::Duration,
) -> Result<reqwest::Response, String> {
    let ordered = ordered_first_party_api_bases();
    let total = ordered.len();
    let mut errors: Vec<String> = Vec::new();

    for (idx, base) in ordered.into_iter().enumerate() {
        let url = format!("{}{}", base, path);
        let client = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(timeout)
            .build()
            .map_err(|e| format!("Failed to initialize API client: {}", e))?;

        let response = match client.post(&url).json(payload).send().await {
            Ok(resp) => resp,
            Err(err) => {
                mark_first_party_route_failed(base);
                errors.push(format!("{} => network error: {}", base, err));
                continue;
            }
        };

        let status = response.status();
        if should_retry_first_party_status(status) && idx + 1 < total {
            mark_first_party_route_failed(base);
            errors.push(format!("{} => HTTP {}", base, status.as_u16()));
            continue;
        }

        mark_first_party_route_healthy(base);
        return Ok(response);
    }

    Err(format!(
        "All first-party API routes failed: {}",
        errors.join(" | ")
    ))
}

fn format_connection_failure_message(
    provider_name: &str,
    stage: &str,
    reason_code: &str,
    proxy_mode: &str,
    proxy_url: &str,
    detail: &str,
) -> String {
    format!(
        "{} connection failed. Stage: {} | Reason: {} | Proxy: {} / {} | {}",
        provider_name,
        stage,
        reason_code,
        proxy_mode,
        proxy_endpoint_display(proxy_mode, proxy_url),
        detail
    )
}

fn classify_cloud_connectivity_failure(
    raw_error: &str,
    proxy_mode: &str,
) -> (String, String, String) {
    let raw = raw_error.trim();
    let lower = raw.to_lowercase();

    if lower.contains("invalidclienttokenid")
        || lower.contains("invalid access key")
        || lower.contains("the security token included in the request is invalid")
    {
        return (
            "provider_auth".to_string(),
            "invalid_access_key".to_string(),
            "Credentials were rejected by the provider. Verify access key and secret.".to_string(),
        );
    }
    if lower.contains("signaturedoesnotmatch")
        || lower.contains("request signature we calculated does not match")
        || lower.contains("incomplete signature")
        || lower.contains("credential should be scoped")
    {
        return (
            "provider_auth".to_string(),
            "signature_mismatch".to_string(),
            "Signature verification failed. Verify secret key, region, and local system time."
                .to_string(),
        );
    }
    if lower.contains("expiredtoken") || lower.contains("request has expired") {
        return (
            "provider_auth".to_string(),
            "token_expired".to_string(),
            "Credential token is expired. Refresh credentials and retry.".to_string(),
        );
    }
    if lower.contains("authfailure")
        || lower.contains("unauthorizedoperation")
        || lower.contains("access denied")
        || lower.contains("not authorized")
    {
        return (
            "provider_auth".to_string(),
            "access_denied".to_string(),
            "Credentials are valid but missing required permissions for this API call.".to_string(),
        );
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        let stage = if proxy_mode == "custom" {
            "proxy_connect"
        } else {
            "target_connect"
        };
        return (
            stage.to_string(),
            "network_timeout".to_string(),
            "Network timeout occurred before the provider responded. Verify proxy and outbound connectivity."
                .to_string(),
        );
    }
    if lower.contains("连接超时") || lower.contains("连接尝试失败") {
        let stage = if proxy_mode == "custom" {
            "proxy_connect"
        } else {
            "target_connect"
        };
        return (
            stage.to_string(),
            "network_timeout".to_string(),
            "Network timeout occurred before the provider responded. Verify proxy and outbound connectivity."
                .to_string(),
        );
    }
    if lower.contains("dns")
        || lower.contains("lookup")
        || lower.contains("name or service not known")
        || lower.contains("could not resolve")
        || lower.contains("找不到主机")
        || lower.contains("无法解析")
        || lower.contains("名称解析")
    {
        return (
            "target_dns".to_string(),
            "dns_resolution_failed".to_string(),
            "Failed to resolve provider endpoint hostname. Check DNS and proxy routing."
                .to_string(),
        );
    }
    if lower.contains("connection refused")
        || lower.contains("actively refused")
        || lower.contains("积极拒绝")
        || lower.contains("目标计算机积极拒绝")
    {
        let stage = if proxy_mode == "custom" {
            "proxy_connect"
        } else {
            "target_connect"
        };
        return (
            stage.to_string(),
            "connection_refused".to_string(),
            "Connection was refused. Verify proxy endpoint or outbound firewall policy."
                .to_string(),
        );
    }
    if (lower.contains("unexpected eof") && lower.contains("handshake"))
        || (lower.contains("eof") && lower.contains("tls"))
        || (lower.contains("eof") && lower.contains("ssl"))
    {
        let stage = if proxy_mode == "custom" {
            "proxy_tunnel"
        } else {
            "target_tls"
        };
        let reason_code = if proxy_mode == "custom" {
            "proxy_tls_eof"
        } else {
            "tls_eof_handshake"
        };
        let message = if proxy_mode == "custom" {
            "Proxy tunnel closed during TLS handshake (unexpected EOF). Verify proxy endpoint, authentication, and outbound HTTPS access."
        } else {
            "TLS handshake ended unexpectedly (EOF). Verify network middleboxes and certificate interception settings."
        };
        return (
            stage.to_string(),
            reason_code.to_string(),
            message.to_string(),
        );
    }
    if lower.contains("certificate") || lower.contains("tls") || lower.contains("ssl") {
        let stage = if proxy_mode == "custom" {
            "proxy_tunnel"
        } else {
            "target_tls"
        };
        return (
            stage.to_string(),
            "tls_handshake_failed".to_string(),
            "TLS handshake failed. Verify certificate trust chain and TLS interception policy."
                .to_string(),
        );
    }
    if lower.contains("proxy") || lower.contains("tunnel") {
        return (
            "proxy_connect".to_string(),
            "proxy_connect_failed".to_string(),
            "Failed to establish proxy tunnel. Verify proxy URL, credentials, and reachability."
                .to_string(),
        );
    }
    if lower.contains("dispatch failure") {
        let stage = if proxy_mode == "custom" {
            "proxy_tunnel"
        } else {
            "target_connect"
        };
        return (
            stage.to_string(),
            "dispatch_failure".to_string(),
            "HTTP request dispatch failed before receiving a provider response. Verify proxy route, DNS, and outbound HTTPS access."
                .to_string(),
        );
    }

    (
        "provider_request".to_string(),
        "request_failed".to_string(),
        format!("Provider request failed: {}", raw),
    )
}

fn extract_http_status_code(raw: &str) -> Option<u16> {
    let lower = raw.to_lowercase();
    for marker in ["http ", "status "] {
        if let Some(idx) = lower.find(marker) {
            let tail = &lower[(idx + marker.len())..];
            let digits: String = tail
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .take(3)
                .collect();
            if digits.len() == 3 {
                if let Ok(code) = digits.parse::<u16>() {
                    return Some(code);
                }
            }
        }
    }
    None
}

fn classify_notification_test_failure(
    raw_error: &str,
    proxy_mode: &str,
) -> (String, String, Option<u16>, String) {
    let raw = raw_error.trim();
    let lower = raw.to_lowercase();
    let http_status = extract_http_status_code(raw);

    if lower.contains("missing required field") {
        return (
            "config_validate".to_string(),
            "config_missing_field".to_string(),
            None,
            "Channel configuration is incomplete. Verify required notification fields.".to_string(),
        );
    }
    if lower.contains("unsupported notification method") {
        return (
            "config_validate".to_string(),
            "unsupported_method".to_string(),
            None,
            "Unsupported notification method. Recreate the channel with a supported method."
                .to_string(),
        );
    }
    if lower.contains("invalid proxy url") || lower.contains("proxy url is missing") {
        return (
            "proxy_validate".to_string(),
            "proxy_invalid_url".to_string(),
            None,
            "Proxy URL is invalid. Verify scheme, host, and port.".to_string(),
        );
    }
    if lower.contains("invalid url") || lower.contains("relative url") {
        return (
            "config_validate".to_string(),
            "channel_invalid_url".to_string(),
            None,
            "Channel endpoint URL is invalid. Provide a full HTTPS URL.".to_string(),
        );
    }

    if let Some(code) = http_status {
        let (stage, reason_code, message) = match code {
            400 => (
                "target_http",
                "target_bad_request",
                "Remote endpoint rejected the payload (HTTP 400). Verify channel formatting.",
            ),
            401 => (
                "target_auth",
                "target_unauthorized",
                "Authentication failed (HTTP 401). Verify tokens, bot keys, or webhook credentials.",
            ),
            403 => (
                "target_auth",
                "target_forbidden",
                "Permission denied (HTTP 403). Verify channel permissions and token scopes.",
            ),
            404 => (
                "target_http",
                "target_not_found",
                "Endpoint not found (HTTP 404). Verify webhook or API URL.",
            ),
            407 => (
                "proxy_auth",
                "proxy_auth_required",
                "Proxy authentication failed (HTTP 407). Verify proxy credentials.",
            ),
            429 => (
                "target_http",
                "target_rate_limited",
                "Remote service rate-limited this request (HTTP 429). Retry later.",
            ),
            500..=599 => (
                "target_http",
                "target_server_error",
                "Remote notification service is unavailable (5xx). Retry later.",
            ),
            _ => (
                "target_http",
                "target_http_error",
                "Remote endpoint returned an HTTP error. Check endpoint settings and credentials.",
            ),
        };
        return (
            stage.to_string(),
            reason_code.to_string(),
            Some(code),
            message.to_string(),
        );
    }

    if lower.contains("timed out") || lower.contains("timeout") {
        let via_proxy_tunnel = proxy_mode == "custom"
            && (lower.contains("proxy") || lower.contains("socks") || lower.contains("tunnel"));
        let stage = if via_proxy_tunnel {
            "proxy_tunnel"
        } else {
            "target_connect"
        };
        let message = if via_proxy_tunnel {
            "Network timeout occurred while establishing the proxy tunnel. Verify proxy protocol, credentials, and outbound HTTPS access."
        } else if proxy_mode == "custom" {
            "Proxy endpoint is reachable, but the request timed out before the remote notification endpoint responded. Verify proxy relay policy, DNS mode, and outbound HTTPS access to the target API."
        } else {
            "Network timeout occurred. Verify remote endpoint reachability."
        };
        return (
            stage.to_string(),
            "network_timeout".to_string(),
            None,
            message.to_string(),
        );
    }
    if lower.contains("dns")
        || lower.contains("lookup")
        || lower.contains("name or service not known")
        || lower.contains("could not resolve")
    {
        return (
            "target_dns".to_string(),
            "dns_resolution_failed".to_string(),
            None,
            "Failed to resolve endpoint hostname. Check DNS, proxy, and endpoint URL.".to_string(),
        );
    }
    if lower.contains("connection refused") {
        let stage = if proxy_mode == "custom" {
            "proxy_connect"
        } else {
            "target_connect"
        };
        return (
            stage.to_string(),
            "connection_refused".to_string(),
            None,
            "Connection was refused. Verify endpoint host/port and firewall policy.".to_string(),
        );
    }
    if (lower.contains("unexpected eof") && lower.contains("handshake"))
        || (lower.contains("eof") && lower.contains("tls"))
        || (lower.contains("eof") && lower.contains("ssl"))
    {
        let (stage, reason_code, message) = if proxy_mode == "custom" {
            (
                "proxy_tunnel",
                "proxy_tls_eof",
                "Proxy tunnel closed during TLS handshake (unexpected EOF). Verify proxy endpoint/auth and outbound access to the notification API.",
            )
        } else {
            (
                "target_tls",
                "tls_eof_handshake",
                "TLS handshake terminated unexpectedly (EOF). Verify endpoint reachability, certificates, and network middleboxes.",
            )
        };
        return (
            stage.to_string(),
            reason_code.to_string(),
            None,
            message.to_string(),
        );
    }
    if lower.contains("proxy") || lower.contains("tunnel") {
        return (
            "proxy_connect".to_string(),
            "proxy_connect_failed".to_string(),
            None,
            "Failed to establish proxy tunnel. Verify proxy endpoint and credentials.".to_string(),
        );
    }
    if lower.contains("certificate") || lower.contains("tls") || lower.contains("ssl") {
        return (
            "target_tls".to_string(),
            "tls_handshake_failed".to_string(),
            None,
            "TLS handshake failed. Verify certificate chain, hostname, and TLS interception settings."
                .to_string(),
        );
    }
    if lower.contains("expected value")
        || lower.contains("eof while parsing")
        || lower.contains("invalid type")
    {
        return (
            "config_validate".to_string(),
            "config_parse_error".to_string(),
            None,
            "Channel configuration format is invalid. Reopen the channel and save again."
                .to_string(),
        );
    }
    if raw.chars().any(|c| !c.is_ascii()) {
        return (
            "target_response".to_string(),
            "non_english_error".to_string(),
            None,
            "Remote service returned a non-English error response. Verify credentials and endpoint settings."
                .to_string(),
        );
    }

    (
        "target_send".to_string(),
        "send_failed".to_string(),
        None,
        format!("Notification send failed: {}", raw),
    )
}

async fn record_notification_test_audit(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    channel: &NotificationChannel,
    diag: &NotificationTestDiagnostics,
) {
    let proxy_display = diag
        .proxy_url_masked
        .as_deref()
        .filter(|v| !v.is_empty())
        .unwrap_or("-");
    let http_display = diag
        .http_status
        .map(|code| code.to_string())
        .unwrap_or_else(|| "-".to_string());
    let outcome = if diag.ok { "success" } else { "failed" };
    let trace_summary = if diag.trace.is_empty() {
        "-".to_string()
    } else {
        diag.trace.join(" || ")
    };
    let details = format!(
        "Notification test {}: app_version={} method={} channel_name=\"{}\" proxy_mode={} proxy_profile={} proxy_scheme={} proxy={} stage={} reason={} http_status={} latency_ms={} message=\"{}\" trace=\"{}\"",
        outcome,
        diag.app_version,
        diag.channel_method,
        channel.name,
        diag.proxy_mode,
        diag.proxy_profile_id.as_deref().unwrap_or("-"),
        diag.proxy_scheme.as_deref().unwrap_or("-"),
        proxy_display,
        diag.stage,
        diag.reason_code,
        http_display,
        diag.duration_ms,
        diag.message,
        trace_summary,
    );
    log_startup_event(&details);
    if !diag.trace.is_empty() {
        for line in &diag.trace {
            log_startup_event(&format!(
                "Notification test trace: channel_name=\"{}\" {}",
                channel.name, line
            ));
        }
    }
    let _ = db::record_audit_log(pool, "NOTIFICATION_TEST", &channel.id, &details).await;
}

fn has_valid_bearer_token(headers: &axum::http::HeaderMap, expected_token: &str) -> bool {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| token.trim() == expected_token)
        .unwrap_or(false)
}

async fn enforce_api_access(
    State(state): State<LocalApiState>,
    ConnectInfo(peer_addr): ConnectInfo<std::net::SocketAddr>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.method() == Method::OPTIONS || request.uri().path() == "/status" {
        return next.run(request).await;
    }

    let has_valid_token = has_valid_bearer_token(request.headers(), &state.api_access_token);
    let auth_bucket = format!("auth_fail:{}", peer_addr.ip());

    if !has_valid_token {
        let allowed = allow_local_rate_limit(
            &state.auth_rate_buckets,
            &auth_bucket,
            60,
            API_MAX_AUTH_FAILURES_PER_MIN,
        )
        .await;
        if !allowed {
            return api_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Too many authentication failures. Retry in about one minute.",
            )
            .into_response();
        }
        return api_error(
            StatusCode::UNAUTHORIZED,
            "Missing or invalid bearer token. Use `Authorization: Bearer <api_access_token>`.",
        )
        .into_response();
    }

    {
        let mut buckets = state.auth_rate_buckets.write().await;
        let _ = buckets.remove(&auth_bucket);
    }

    next.run(request).await
}

// API Server Handlers
async fn handle_api_status(State(state): State<LocalApiState>) -> Json<serde_json::Value> {
    let scheme = if state.tls_enabled { "https" } else { "http" };
    let endpoint = format!("{}://{}:{}", scheme, state.bind_host, state.port);
    Json(serde_json::json!({
        "status": "running",
        "version": state.app_version,
        "api_version": "v1",
        "listen_host": state.bind_host,
        "listen_port": state.port,
        "transport": scheme,
        "tls_enabled": state.tls_enabled,
        "self_signed_tls": state.tls_enabled,
        "base_url": endpoint,
        "api_enabled": state.api_enabled,
        "remote_auth": "bearer_token_required",
        "loopback_without_token": false
    }))
}

async fn handle_api_capabilities(State(state): State<LocalApiState>) -> Json<serde_json::Value> {
    let schedules_enabled = local_api_schedule_entitled(&state).await;
    let mut groups = vec![
        ApiCapabilityGroup {
            name: "meta".to_string(),
            status: "active".to_string(),
            routes: vec![
                "GET /status".to_string(),
                "GET /v1/meta/capabilities".to_string(),
                "GET /v1/meta/error-model".to_string(),
            ],
        },
        ApiCapabilityGroup {
            name: "scan".to_string(),
            status: "active".to_string(),
            routes: vec![
                "GET /v1/scans".to_string(),
                "POST /v1/scans".to_string(),
                "GET /v1/scans/:scan_id".to_string(),
                "GET /v1/scans/:scan_id/progress".to_string(),
                "POST /v1/scans/:scan_id/cancel".to_string(),
            ],
        },
        ApiCapabilityGroup {
            name: "findings".to_string(),
            status: "active".to_string(),
            routes: vec![
                "GET /v1/findings".to_string(),
                "GET /v1/findings/handled".to_string(),
                "POST /v1/findings/:resource_id/handled".to_string(),
                "GET /v1/scan-history".to_string(),
                "GET /v1/scan-history/:history_id".to_string(),
                "DELETE /v1/scan-history/:history_id".to_string(),
            ],
        },
        ApiCapabilityGroup {
            name: "reports".to_string(),
            status: "active".to_string(),
            routes: vec![
                "POST /v1/reports/generate".to_string(),
                "GET /v1/reports".to_string(),
                "GET /v1/reports/overview".to_string(),
                "GET /v1/reports/trend".to_string(),
                "GET /v1/reports/error-taxonomy".to_string(),
                "GET /v1/reports/:report_id".to_string(),
                "GET /v1/reports/:report_id/download".to_string(),
            ],
        },
        ApiCapabilityGroup {
            name: "events".to_string(),
            status: "beta".to_string(),
            routes: vec![
                "GET /v1/events".to_string(),
                "GET /v1/events/types".to_string(),
            ],
        },
        ApiCapabilityGroup {
            name: "webhooks".to_string(),
            status: "beta".to_string(),
            routes: vec![
                "GET /v1/webhooks".to_string(),
                "POST /v1/webhooks".to_string(),
                "DELETE /v1/webhooks/:webhook_id".to_string(),
                "POST /v1/webhooks/:webhook_id/test".to_string(),
            ],
        },
        ApiCapabilityGroup {
            name: "mcp".to_string(),
            status: "beta".to_string(),
            routes: vec![
                "GET /v1/mcp/capabilities".to_string(),
                "POST /v1/mcp/tools/run-scan".to_string(),
                "GET /v1/mcp/tools/get-scan/:scan_id".to_string(),
            ],
        },
        ApiCapabilityGroup {
            name: "accounts".to_string(),
            status: "active".to_string(),
            routes: vec!["GET /v1/accounts".to_string()],
        },
        ApiCapabilityGroup {
            name: "cloud_accounts".to_string(),
            status: "active".to_string(),
            routes: vec![
                "GET /v1/cloud-accounts".to_string(),
                "POST /v1/cloud-accounts".to_string(),
                "GET /v1/cloud-accounts/:account_id".to_string(),
                "PATCH /v1/cloud-accounts/:account_id".to_string(),
                "DELETE /v1/cloud-accounts/:account_id".to_string(),
                "POST /v1/cloud-accounts/test".to_string(),
            ],
        },
        ApiCapabilityGroup {
            name: "proxies".to_string(),
            status: "active".to_string(),
            routes: vec![
                "GET /v1/proxies".to_string(),
                "POST /v1/proxies".to_string(),
                "GET /v1/proxies/:proxy_id".to_string(),
                "PATCH /v1/proxies/:proxy_id".to_string(),
                "DELETE /v1/proxies/:proxy_id".to_string(),
                "POST /v1/proxies/test".to_string(),
            ],
        },
        ApiCapabilityGroup {
            name: "notifications".to_string(),
            status: "active".to_string(),
            routes: vec![
                "GET /v1/notifications/channels".to_string(),
                "POST /v1/notifications/channels".to_string(),
                "PATCH /v1/notifications/channels/:channel_id".to_string(),
                "DELETE /v1/notifications/channels/:channel_id".to_string(),
                "POST /v1/notifications/channels/test".to_string(),
                "GET /v1/notifications/policy".to_string(),
                "PATCH /v1/notifications/policy".to_string(),
            ],
        },
        ApiCapabilityGroup {
            name: "settings".to_string(),
            status: "active".to_string(),
            routes: vec![
                "GET /v1/settings/general".to_string(),
                "PATCH /v1/settings/general".to_string(),
                "GET /v1/settings/network".to_string(),
                "PATCH /v1/settings/network".to_string(),
                "GET /v1/settings/scan-policy".to_string(),
                "PATCH /v1/settings/scan-policy".to_string(),
                "GET /v1/settings/license".to_string(),
            ],
        },
    ];
    groups.push(ApiCapabilityGroup {
        name: "schedules".to_string(),
        status: if schedules_enabled {
            "active".to_string()
        } else {
            "upgrade_required".to_string()
        },
        routes: if schedules_enabled {
            vec![
                "GET /v1/schedules".to_string(),
                "POST /v1/schedules".to_string(),
                "GET /v1/schedules/:schedule_id".to_string(),
                "DELETE /v1/schedules/:schedule_id".to_string(),
                "POST /v1/schedules/:schedule_id/run-now".to_string(),
            ]
        } else {
            Vec::new()
        },
    });

    Json(serde_json::json!({
        "api_version": "v1",
        "app_version": state.app_version,
        "generated_at": now_unix_ts(),
        "groups": groups
    }))
}

fn schedule_gate_error() -> ApiError {
    api_error(
        StatusCode::FORBIDDEN,
        "Scheduled audits are available on Team and Enterprise editions.",
    )
}

fn entitlements_for_runtime_plan(plan: Option<&str>) -> runtime_helpers::RuntimeEntitlements {
    let normalized = plan
        .map(runtime_helpers::normalize_runtime_plan_type)
        .unwrap_or_else(|| "community".to_string());
    let is_trial = normalized.eq_ignore_ascii_case("trial");
    build_runtime_entitlements(&normalized, is_trial)
}

fn schedule_entitled_for_runtime_plan(plan: Option<&str>) -> bool {
    entitlements_for_runtime_plan(plan).scheduled_audits
}

fn audit_log_entitled_for_runtime_plan(plan: Option<&str>) -> bool {
    entitlements_for_runtime_plan(plan).audit_log
}

async fn local_api_schedule_entitled(state: &LocalApiState) -> bool {
    let app_state = state.app_handle.state::<AppState>();
    let runtime_plan = read_runtime_plan_type(&app_state.db_path).await;
    schedule_entitled_for_runtime_plan(runtime_plan.as_deref())
}

async fn handle_api_error_model() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "error_envelope": {
            "error": "string"
        },
        "notes": [
            "Current v1 compatibility keeps legacy {\"error\":\"...\"} responses.",
            "Future versions may add structured fields such as code/stage/reason while preserving backward compatibility.",
            "Governance error taxonomy categories: auth, network, timeout, config, rate_limit, service, unknown."
        ],
        "common_status_codes": [
            {"status": 400, "meaning": "validation_error"},
            {"status": 401, "meaning": "auth_required_or_invalid_token"},
            {"status": 403, "meaning": "license_policy_forbidden"},
            {"status": 404, "meaning": "resource_not_found"},
            {"status": 429, "meaning": "rate_limited"},
            {"status": 500, "meaning": "internal_error"}
        ]
    }))
}

async fn read_api_settings_general(conn: &sqlx::Pool<sqlx::Sqlite>) -> serde_json::Value {
    let currency = db::get_setting(conn, "currency")
        .await
        .unwrap_or_else(|_| "USD".to_string());
    let api_timeout = db::get_setting(conn, "api_timeout")
        .await
        .unwrap_or_else(|_| "10".to_string())
        .parse::<u64>()
        .unwrap_or(10);
    let runtime_plan_type = db::get_setting(conn, "runtime_plan_type")
        .await
        .unwrap_or_default();
    let notification_trigger_mode = db::get_setting(conn, NOTIFICATION_TRIGGER_MODE_SETTING_KEY)
        .await
        .unwrap_or_else(|_| NOTIFICATION_TRIGGER_MODE_WASTE_ONLY.to_string());

    serde_json::json!({
        "currency": if currency.trim().is_empty() { "USD" } else { currency.trim() },
        "api_timeout_seconds": api_timeout,
        "runtime_plan_type": runtime_plan_type.trim(),
        "notification_trigger_mode": notification_trigger_mode,
    })
}

async fn read_api_settings_network(
    conn: &sqlx::Pool<sqlx::Sqlite>,
    state: &LocalApiState,
) -> serde_json::Value {
    let proxy_mode_raw = db::get_setting(conn, "proxy_mode")
        .await
        .unwrap_or_else(|_| "none".to_string());
    let proxy_mode = normalize_proxy_mode(&proxy_mode_raw);
    let proxy_url = db::get_setting(conn, "proxy_url").await.unwrap_or_default();
    let proxy_url_masked = if proxy_mode == "custom" && !proxy_url.trim().is_empty() {
        Some(mask_proxy_url(proxy_url.trim()))
    } else {
        None
    };
    let api_bind_host = db::get_setting(conn, "api_bind_host")
        .await
        .unwrap_or_else(|_| state.bind_host.clone());
    let api_port = db::get_setting(conn, "api_port")
        .await
        .unwrap_or_else(|_| state.port.to_string())
        .parse::<u16>()
        .unwrap_or(state.port);
    let api_tls_enabled_raw = db::get_setting(conn, "api_tls_enabled")
        .await
        .unwrap_or_default();
    let api_tls_enabled = if api_tls_enabled_raw.trim().is_empty() {
        default_api_tls_enabled(&api_bind_host)
    } else {
        parse_bool_setting(&api_tls_enabled_raw)
    };

    serde_json::json!({
        "proxy_mode": proxy_mode,
        "proxy_url_masked": proxy_url_masked,
        "api_bind_host": api_bind_host.trim(),
        "api_port": api_port,
        "api_tls_enabled": api_tls_enabled,
    })
}

async fn read_api_settings_scan_policy(conn: &sqlx::Pool<sqlx::Sqlite>) -> serde_json::Value {
    let cpu_percent = db::get_setting(conn, "policy_cpu_percent")
        .await
        .unwrap_or_else(|_| "2.0".to_string())
        .parse::<f64>()
        .unwrap_or(2.0);
    let network_mb = db::get_setting(conn, "policy_net_mb")
        .await
        .unwrap_or_else(|_| "5.0".to_string())
        .parse::<f64>()
        .unwrap_or(5.0);
    let lookback_days = db::get_setting(conn, "policy_days")
        .await
        .unwrap_or_else(|_| "7".to_string())
        .parse::<i64>()
        .unwrap_or(7);
    let provider_policies_raw = db::get_setting(conn, "provider_policies")
        .await
        .unwrap_or_else(|_| "{}".to_string());
    let provider_policies: serde_json::Value =
        serde_json::from_str(&provider_policies_raw).unwrap_or_else(|_| serde_json::json!({}));

    serde_json::json!({
        "global": {
            "cpu_percent": cpu_percent,
            "network_mb": network_mb,
            "lookback_days": lookback_days
        },
        "provider_policies": provider_policies
    })
}

async fn read_api_settings_license(
    state: &LocalApiState,
    _conn: &sqlx::Pool<sqlx::Sqlite>,
) -> serde_json::Value {
    let key = load_license_file(state.app_handle.clone()).unwrap_or_default();
    serde_json::json!({
        "has_local_license": !key.trim().is_empty(),
        "plan_type": "community",
        "is_trial": false,
        "trial_expires_at": null,
        "quota": null,
        "max_quota": null,
        "api_enabled": true,
        "resource_details_enabled": true,
        "message": "Community mode: local execution without remote license checks."
    })
}

async fn handle_api_settings_general(
    State(state): State<LocalApiState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(read_api_settings_general(&conn).await))
}

async fn handle_api_patch_settings_general(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiSettingsGeneralPatchRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if payload.currency.is_none()
        && payload.api_timeout_seconds.is_none()
        && payload.notification_trigger_mode.is_none()
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "At least one settings field must be provided.",
        ));
    }

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if let Some(currency) = payload.currency {
        let normalized = currency.trim().to_ascii_uppercase();
        if normalized.is_empty() || normalized.len() > 8 {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "currency must be 1-8 characters.",
            ));
        }
        db::save_setting(&conn, "currency", &normalized)
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    if let Some(timeout_secs) = payload.api_timeout_seconds {
        if !(1..=600).contains(&timeout_secs) {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "api_timeout_seconds must be between 1 and 600.",
            ));
        }
        db::save_setting(&conn, "api_timeout", &timeout_secs.to_string())
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    if let Some(trigger_mode_raw) = payload.notification_trigger_mode {
        let trigger_mode = trigger_mode_raw.trim().to_ascii_lowercase();
        if trigger_mode != NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE
            && trigger_mode != NOTIFICATION_TRIGGER_MODE_WASTE_ONLY
        {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "notification_trigger_mode must be scan_complete or waste_only.",
            ));
        }
        db::save_setting(&conn, NOTIFICATION_TRIGGER_MODE_SETTING_KEY, &trigger_mode)
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    Ok(Json(read_api_settings_general(&conn).await))
}

async fn handle_api_settings_network(
    State(state): State<LocalApiState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(read_api_settings_network(&conn, &state).await))
}

async fn handle_api_patch_settings_network(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiSettingsNetworkPatchRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if payload.proxy_mode.is_none()
        && payload.proxy_url.is_none()
        && payload.api_bind_host.is_none()
        && payload.api_port.is_none()
        && payload.api_tls_enabled.is_none()
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "At least one settings field must be provided.",
        ));
    }

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let mut effective_proxy_mode = normalize_proxy_mode(
        &db::get_setting(&conn, "proxy_mode")
            .await
            .unwrap_or_else(|_| "none".to_string()),
    );
    let mut effective_proxy_url = db::get_setting(&conn, "proxy_url")
        .await
        .unwrap_or_default();

    let has_proxy_mode = payload.proxy_mode.is_some();
    let has_proxy_url = payload.proxy_url.is_some();

    if let Some(proxy_mode_raw) = payload.proxy_mode.as_deref() {
        effective_proxy_mode = normalize_proxy_mode(&proxy_mode_raw);
        db::save_setting(&conn, "proxy_mode", &effective_proxy_mode)
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    if let Some(proxy_url_raw) = payload.proxy_url.as_deref() {
        effective_proxy_url = proxy_url_raw.trim().to_string();
    }

    if effective_proxy_mode == "custom" {
        if effective_proxy_url.trim().is_empty() {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "proxy_url is required when proxy_mode=custom.",
            ));
        }
        let normalized_proxy_url = normalize_custom_proxy_url(effective_proxy_url.trim());
        db::save_setting(&conn, "proxy_url", &normalized_proxy_url)
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    } else if has_proxy_mode || has_proxy_url {
        db::save_setting(&conn, "proxy_url", "")
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    if let Some(bind_host_raw) = payload.api_bind_host {
        let bind_host = bind_host_raw.trim().to_string();
        if bind_host.is_empty() {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "api_bind_host cannot be empty.",
            ));
        }
        db::save_setting(&conn, "api_bind_host", &bind_host)
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    if let Some(api_port) = payload.api_port {
        if api_port == 0 {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "api_port must be between 1 and 65535.",
            ));
        }
        db::save_setting(&conn, "api_port", &api_port.to_string())
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    if let Some(api_tls_enabled) = payload.api_tls_enabled {
        db::save_setting(
            &conn,
            "api_tls_enabled",
            if api_tls_enabled { "1" } else { "0" },
        )
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    Ok(Json(read_api_settings_network(&conn, &state).await))
}

async fn handle_api_settings_scan_policy(
    State(state): State<LocalApiState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(read_api_settings_scan_policy(&conn).await))
}

async fn handle_api_patch_settings_scan_policy(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiSettingsScanPolicyPatchRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if payload.global.is_none() && payload.provider_policies.is_none() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "At least one settings field must be provided.",
        ));
    }

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if let Some(global) = payload.global {
        if let Some(cpu_percent) = global.cpu_percent {
            if cpu_percent <= 0.0 {
                return Err(api_error(
                    StatusCode::BAD_REQUEST,
                    "global.cpu_percent must be > 0.",
                ));
            }
            db::save_setting(&conn, "policy_cpu_percent", &cpu_percent.to_string())
                .await
                .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        }

        if let Some(network_mb) = global.network_mb {
            if network_mb < 0.0 {
                return Err(api_error(
                    StatusCode::BAD_REQUEST,
                    "global.network_mb must be >= 0.",
                ));
            }
            db::save_setting(&conn, "policy_net_mb", &network_mb.to_string())
                .await
                .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        }

        if let Some(lookback_days) = global.lookback_days {
            if lookback_days <= 0 {
                return Err(api_error(
                    StatusCode::BAD_REQUEST,
                    "global.lookback_days must be > 0.",
                ));
            }
            db::save_setting(&conn, "policy_days", &lookback_days.to_string())
                .await
                .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        }
    }

    if let Some(provider_policies) = payload.provider_policies {
        if !provider_policies.is_object() {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "provider_policies must be a JSON object.",
            ));
        }
        let serialized = serde_json::to_string(&provider_policies)
            .map_err(|e| api_error(StatusCode::BAD_REQUEST, e.to_string()))?;
        db::save_setting(&conn, "provider_policies", &serialized)
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    Ok(Json(read_api_settings_scan_policy(&conn).await))
}

async fn handle_api_settings_license(
    State(state): State<LocalApiState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(read_api_settings_license(&state, &conn).await))
}

fn mask_secret_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 6 {
        return "***".to_string();
    }
    let prefix: String = chars.iter().take(3).collect();
    let suffix: String = chars
        .iter()
        .rev()
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}***{}", prefix, suffix)
}

fn is_sensitive_credential_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    [
        "secret",
        "token",
        "password",
        "private",
        "credential",
        "access_key",
        "api_key",
        "client_secret",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn mask_credentials_value(key: Option<&str>, value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut masked = serde_json::Map::new();
            for (k, v) in map {
                if is_sensitive_credential_key(k) {
                    if let Some(as_str) = v.as_str() {
                        masked.insert(k.clone(), serde_json::json!(mask_secret_value(as_str)));
                    } else {
                        masked.insert(k.clone(), serde_json::json!("***"));
                    }
                } else {
                    masked.insert(k.clone(), mask_credentials_value(Some(k.as_str()), v));
                }
            }
            serde_json::Value::Object(masked)
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| mask_credentials_value(key, item))
                .collect(),
        ),
        serde_json::Value::String(raw) => {
            if key.map(is_sensitive_credential_key).unwrap_or(false) {
                serde_json::json!(mask_secret_value(raw))
            } else {
                serde_json::json!(raw)
            }
        }
        _ => value.clone(),
    }
}

fn mask_cloud_credentials(credentials: &str) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(credentials) {
        Ok(value) => mask_credentials_value(None, &value),
        Err(_) => serde_json::json!({
            "raw": mask_secret_value(credentials)
        }),
    }
}

fn build_api_cloud_account_record(profile: db::CloudProfile) -> ApiCloudAccountRecord {
    ApiCloudAccountRecord {
        id: profile.id,
        provider: profile.provider,
        name: profile.name,
        timeout_seconds: profile.timeout_seconds,
        policy_custom: profile.policy_custom,
        proxy_profile_id: profile.proxy_profile_id,
        created_at: profile.created_at,
        credentials_masked: mask_cloud_credentials(&profile.credentials),
    }
}

async fn handle_api_accounts(
    State(state): State<LocalApiState>,
) -> Result<Json<Vec<ApiAccountSummary>>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let profiles = db::list_cloud_profiles(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let accounts = profiles
        .into_iter()
        .map(|profile| ApiAccountSummary {
            id: profile.id,
            provider: profile.provider,
            name: profile.name,
        })
        .collect();

    Ok(Json(accounts))
}

async fn handle_api_list_cloud_accounts(
    State(state): State<LocalApiState>,
) -> Result<Json<Vec<ApiCloudAccountRecord>>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let profiles = db::list_cloud_profiles(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(
        profiles
            .into_iter()
            .map(build_api_cloud_account_record)
            .collect(),
    ))
}

async fn handle_api_get_cloud_account(
    State(state): State<LocalApiState>,
    AxumPath(account_id): AxumPath<String>,
) -> Result<Json<ApiCloudAccountRecord>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let profiles = db::list_cloud_profiles(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let record = profiles
        .into_iter()
        .find(|p| p.id == account_id)
        .map(build_api_cloud_account_record)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "cloud account id not found"))?;

    Ok(Json(record))
}

async fn handle_api_create_cloud_account(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiCloudAccountCreateRequest>,
) -> Result<Json<ApiCloudAccountRecord>, ApiError> {
    let provider = payload.provider.trim().to_ascii_lowercase();
    let name = payload.name.trim().to_string();
    let credentials = payload.credentials.trim().to_string();
    if provider.is_empty() || name.is_empty() || credentials.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "provider, name, and credentials are required.",
        ));
    }
    if provider == "aws" {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "AWS accounts must be managed via local AWS profile flows.",
        ));
    }

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let profile_id = db::save_cloud_profile(
        &conn,
        &provider,
        &name,
        &credentials,
        payload.timeout_seconds,
        payload.policy_custom,
        payload.proxy_profile_id,
    )
    .await
    .map_err(|e| api_error(StatusCode::BAD_REQUEST, e))?;

    let profiles = db::list_cloud_profiles(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let record = profiles
        .into_iter()
        .find(|p| p.id == profile_id)
        .map(build_api_cloud_account_record)
        .ok_or_else(|| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "created cloud account could not be reloaded",
            )
        })?;

    Ok(Json(record))
}

async fn handle_api_update_cloud_account(
    State(state): State<LocalApiState>,
    AxumPath(account_id): AxumPath<String>,
    Json(payload): Json<ApiCloudAccountUpdateRequest>,
) -> Result<Json<ApiCloudAccountRecord>, ApiError> {
    if payload.provider.is_none()
        && payload.name.is_none()
        && payload.credentials.is_none()
        && payload.timeout_seconds.is_none()
        && payload.policy_custom.is_none()
        && payload.proxy_profile_id.is_none()
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "At least one field must be provided for update.",
        ));
    }

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let profiles = db::list_cloud_profiles(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let existing = profiles
        .into_iter()
        .find(|p| p.id == account_id)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "cloud account id not found"))?;

    let provider = payload
        .provider
        .as_deref()
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or(existing.provider.clone());
    if provider == "aws" {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "AWS accounts must be managed via local AWS profile flows.",
        ));
    }

    let name = payload
        .name
        .as_deref()
        .map(|v| v.trim().to_string())
        .unwrap_or(existing.name.clone());
    if name.is_empty() {
        return Err(api_error(StatusCode::BAD_REQUEST, "name cannot be empty."));
    }

    let credentials = payload
        .credentials
        .as_deref()
        .map(|v| v.trim().to_string())
        .unwrap_or(existing.credentials.clone());
    if credentials.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "credentials cannot be empty.",
        ));
    }

    db::update_cloud_profile(
        &conn,
        &existing.id,
        &provider,
        &name,
        &credentials,
        payload.timeout_seconds.or(existing.timeout_seconds),
        payload.policy_custom.or(existing.policy_custom),
        payload.proxy_profile_id.or(existing.proxy_profile_id),
    )
    .await
    .map_err(|e| api_error(StatusCode::BAD_REQUEST, e))?;

    let updated = db::list_cloud_profiles(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .into_iter()
        .find(|p| p.id == existing.id)
        .map(build_api_cloud_account_record)
        .ok_or_else(|| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "updated cloud account could not be reloaded",
            )
        })?;

    Ok(Json(updated))
}

async fn handle_api_delete_cloud_account(
    State(state): State<LocalApiState>,
    AxumPath(account_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::delete_cloud_profile(&conn, &account_id)
        .await
        .map_err(|e| api_error(StatusCode::BAD_REQUEST, e))?;

    Ok(Json(serde_json::json!({
        "status": "deleted",
        "id": account_id
    })))
}

async fn handle_api_test_cloud_account(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiCloudAccountTestRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let provider = payload.provider.trim().to_ascii_lowercase();
    let credentials = payload.credentials.trim().to_string();
    if provider.is_empty() || credentials.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "provider and credentials are required.",
        ));
    }

    let result = test_connection(
        state.app_handle.clone(),
        provider.clone(),
        credentials,
        payload.region,
        Some(true),
        payload.proxy_profile_id,
    )
    .await;

    Ok(Json(match result {
        Ok(message) => serde_json::json!({
            "ok": true,
            "provider": provider,
            "message": message
        }),
        Err(message) => serde_json::json!({
            "ok": false,
            "provider": provider,
            "message": message
        }),
    }))
}

fn build_api_proxy_record(profile: db::ProxyProfile) -> ApiProxyRecord {
    ApiProxyRecord {
        id: profile.id,
        name: profile.name,
        protocol: profile.protocol,
        host: profile.host,
        port: profile.port,
        auth_username: profile.auth_username,
        has_auth_password: profile
            .auth_password
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        created_at: profile.created_at,
    }
}

async fn handle_api_list_proxies(
    State(state): State<LocalApiState>,
) -> Result<Json<Vec<ApiProxyRecord>>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let profiles = db::list_proxy_profiles(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(
        profiles.into_iter().map(build_api_proxy_record).collect(),
    ))
}

async fn handle_api_get_proxy(
    State(state): State<LocalApiState>,
    AxumPath(proxy_id): AxumPath<String>,
) -> Result<Json<ApiProxyRecord>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let proxy = db::get_proxy_profile(&conn, &proxy_id)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map(build_api_proxy_record)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "proxy id not found"))?;
    Ok(Json(proxy))
}

async fn handle_api_create_proxy(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiProxyCreateRequest>,
) -> Result<Json<ApiProxyRecord>, ApiError> {
    let proxy_id = save_proxy_profile(
        state.app_handle.clone(),
        None,
        payload.name,
        payload.protocol,
        payload.host,
        payload.port,
        payload.auth_username,
        payload.auth_password,
    )
    .await
    .map_err(|e| api_error(StatusCode::BAD_REQUEST, e))?;

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let created = db::get_proxy_profile(&conn, &proxy_id)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map(build_api_proxy_record)
        .ok_or_else(|| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "created proxy profile could not be reloaded",
            )
        })?;
    Ok(Json(created))
}

async fn handle_api_update_proxy(
    State(state): State<LocalApiState>,
    AxumPath(proxy_id): AxumPath<String>,
    Json(payload): Json<ApiProxyUpdateRequest>,
) -> Result<Json<ApiProxyRecord>, ApiError> {
    if payload.name.is_none()
        && payload.protocol.is_none()
        && payload.host.is_none()
        && payload.port.is_none()
        && payload.auth_username.is_none()
        && payload.auth_password.is_none()
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "At least one field must be provided for update.",
        ));
    }

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let existing = db::get_proxy_profile(&conn, &proxy_id)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "proxy id not found"))?;

    let name = payload.name.unwrap_or(existing.name);
    let protocol = payload.protocol.unwrap_or(existing.protocol);
    let host = payload.host.unwrap_or(existing.host);
    let port = payload.port.unwrap_or(existing.port);
    let auth_username = payload
        .auth_username
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or(existing.auth_username);
    let auth_password = payload
        .auth_password
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or(existing.auth_password);

    save_proxy_profile(
        state.app_handle.clone(),
        Some(proxy_id.clone()),
        name,
        protocol,
        host,
        port,
        auth_username,
        auth_password,
    )
    .await
    .map_err(|e| api_error(StatusCode::BAD_REQUEST, e))?;

    let updated = db::get_proxy_profile(&conn, &proxy_id)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map(build_api_proxy_record)
        .ok_or_else(|| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "updated proxy profile could not be reloaded",
            )
        })?;
    Ok(Json(updated))
}

async fn handle_api_delete_proxy(
    State(state): State<LocalApiState>,
    AxumPath(proxy_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    delete_proxy_profile(state.app_handle.clone(), proxy_id.clone())
        .await
        .map_err(|e| api_error(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(serde_json::json!({
        "status": "deleted",
        "id": proxy_id
    })))
}

async fn handle_api_test_proxy(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiProxyTestRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut mode = payload
        .proxy_mode
        .as_deref()
        .map(normalize_proxy_mode)
        .unwrap_or_else(|| "none".to_string());
    let mut proxy_url = payload.proxy_url.unwrap_or_default().trim().to_string();

    if let Some(proxy_profile_id) = payload.proxy_profile_id.as_deref() {
        let app_state = state.app_handle.state::<AppState>();
        let conn = db::init_db(&app_state.db_path)
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        let profile = db::get_proxy_profile(&conn, proxy_profile_id)
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?
            .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "proxy profile id not found"))?;
        mode = "custom".to_string();
        proxy_url = compose_proxy_url_from_parts(
            &profile.protocol,
            &profile.host,
            profile.port,
            profile.auth_username.as_deref(),
            profile.auth_password.as_deref(),
        );
    }

    if mode == "custom" && proxy_url.trim().is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "proxy_url is required when proxy_mode=custom.",
        ));
    }

    let result = test_proxy_connection(
        mode.clone(),
        if proxy_url.trim().is_empty() {
            None
        } else {
            Some(proxy_url.clone())
        },
    )
    .await;
    let proxy_endpoint = if proxy_url.is_empty() {
        "-".to_string()
    } else {
        mask_proxy_url(&proxy_url)
    };

    Ok(Json(match result {
        Ok(message) => serde_json::json!({
            "ok": true,
            "proxy_mode": mode,
            "proxy_endpoint": proxy_endpoint,
            "message": message
        }),
        Err(message) => serde_json::json!({
            "ok": false,
            "proxy_mode": mode,
            "proxy_endpoint": proxy_endpoint,
            "message": message
        }),
    }))
}

fn normalize_notification_channel_id(id: Option<String>) -> String {
    id.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn build_notification_channel_from_create(
    payload: ApiNotificationChannelCreateRequest,
) -> Result<NotificationChannel, ApiError> {
    let name = payload.name.trim().to_string();
    let method = normalize_notification_method_for_storage(&payload.method);
    let config = payload.config.trim().to_string();
    if name.is_empty() || method.is_empty() || config.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "name, method, and config are required.",
        ));
    }
    if !validate_notification_method(&method) {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "unsupported notification method.",
        ));
    }
    let trigger_mode = normalize_channel_trigger_mode_for_storage(payload.trigger_mode.as_deref())
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, err))?;
    let min_savings = normalize_channel_min_savings_for_storage(payload.min_savings)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, err))?;
    let min_findings = normalize_channel_min_findings_for_storage(payload.min_findings)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, err))?;

    Ok(NotificationChannel {
        id: normalize_notification_channel_id(payload.id),
        name,
        method,
        config,
        is_active: payload.is_active.unwrap_or(true),
        proxy_profile_id: payload.proxy_profile_id,
        trigger_mode,
        min_savings,
        min_findings,
    })
}

fn find_notification_channel(
    channels: &[NotificationChannel],
    channel_id: &str,
) -> Option<NotificationChannel> {
    channels.iter().find(|c| c.id == channel_id).cloned()
}

async fn handle_api_list_notification_channels(
    State(state): State<LocalApiState>,
) -> Result<Json<Vec<NotificationChannel>>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let channels = db::list_notification_channels(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(channels))
}

async fn handle_api_create_notification_channel(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiNotificationChannelCreateRequest>,
) -> Result<Json<NotificationChannel>, ApiError> {
    let channel = build_notification_channel_from_create(payload)?;
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::save_notification_channel(&conn, &channel)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(channel))
}

async fn handle_api_update_notification_channel(
    State(state): State<LocalApiState>,
    AxumPath(channel_id): AxumPath<String>,
    Json(payload): Json<ApiNotificationChannelUpdateRequest>,
) -> Result<Json<NotificationChannel>, ApiError> {
    if payload.name.is_none()
        && payload.method.is_none()
        && payload.config.is_none()
        && payload.is_active.is_none()
        && payload.proxy_profile_id.is_none()
        && payload.trigger_mode.is_none()
        && payload.min_savings.is_none()
        && payload.min_findings.is_none()
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "At least one field must be provided for update.",
        ));
    }

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let channels = db::list_notification_channels(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let existing = find_notification_channel(&channels, &channel_id)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "notification channel id not found"))?;

    let method = payload
        .method
        .as_deref()
        .map(normalize_notification_method_for_storage)
        .unwrap_or_else(|| normalize_notification_method_for_storage(&existing.method));
    if !validate_notification_method(&method) {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "unsupported notification method.",
        ));
    }
    let trigger_mode = if let Some(raw) = payload.trigger_mode.as_deref() {
        normalize_channel_trigger_mode_for_storage(Some(raw))
            .map_err(|err| api_error(StatusCode::BAD_REQUEST, err))?
    } else {
        existing.trigger_mode.clone()
    };
    let min_savings = if let Some(raw) = payload.min_savings {
        normalize_channel_min_savings_for_storage(Some(raw))
            .map_err(|err| api_error(StatusCode::BAD_REQUEST, err))?
    } else {
        existing.min_savings
    };
    let min_findings = if let Some(raw) = payload.min_findings {
        normalize_channel_min_findings_for_storage(Some(raw))
            .map_err(|err| api_error(StatusCode::BAD_REQUEST, err))?
    } else {
        existing.min_findings
    };

    let channel = NotificationChannel {
        id: existing.id,
        name: payload
            .name
            .as_deref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or(existing.name),
        method,
        config: payload
            .config
            .as_deref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or(existing.config),
        is_active: payload.is_active.unwrap_or(existing.is_active),
        proxy_profile_id: payload.proxy_profile_id.or(existing.proxy_profile_id),
        trigger_mode,
        min_savings,
        min_findings,
    };

    db::save_notification_channel(&conn, &channel)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(channel))
}

async fn handle_api_delete_notification_channel(
    State(state): State<LocalApiState>,
    AxumPath(channel_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::delete_notification_channel(&conn, &channel_id)
        .await
        .map_err(|e| api_error(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(serde_json::json!({
        "status": "deleted",
        "id": channel_id
    })))
}

async fn handle_api_test_notification_channel(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiNotificationTestRequest>,
) -> Result<Json<NotificationTestDiagnostics>, ApiError> {
    let channel = if let Some(channel_id) = payload.channel_id.as_deref() {
        let app_state = state.app_handle.state::<AppState>();
        let conn = db::init_db(&app_state.db_path)
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        let channels = db::list_notification_channels(&conn)
            .await
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        find_notification_channel(&channels, channel_id)
            .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "notification channel id not found"))?
    } else if let Some(raw_channel) = payload.channel {
        build_notification_channel_from_create(raw_channel)?
    } else {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "channel_id or channel payload is required.",
        ));
    };

    let diagnostics = test_notification_channel(state.app_handle.clone(), channel).await;
    Ok(Json(diagnostics))
}

async fn handle_api_get_notification_policy(
    State(state): State<LocalApiState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let mode = db::get_setting(&conn, NOTIFICATION_TRIGGER_MODE_SETTING_KEY)
        .await
        .unwrap_or_else(|_| NOTIFICATION_TRIGGER_MODE_WASTE_ONLY.to_string());
    Ok(Json(serde_json::json!({
        "notification_trigger_mode": mode
    })))
}

async fn handle_api_patch_notification_policy(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiNotificationPolicyPatchRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mode = payload
        .notification_trigger_mode
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .ok_or_else(|| {
            api_error(
                StatusCode::BAD_REQUEST,
                "notification_trigger_mode is required.",
            )
        })?;

    if mode != NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE
        && mode != NOTIFICATION_TRIGGER_MODE_WASTE_ONLY
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "notification_trigger_mode must be scan_complete or waste_only.",
        ));
    }

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::save_setting(&conn, NOTIFICATION_TRIGGER_MODE_SETTING_KEY, &mode)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({
        "notification_trigger_mode": mode
    })))
}

async fn handle_api_get_scan(
    State(state): State<LocalApiState>,
    AxumPath(scan_id): AxumPath<String>,
) -> Result<Json<ApiScanJob>, ApiError> {
    let jobs = state.jobs.read().await;
    match jobs.get(&scan_id) {
        Some(job) => Ok(Json(job.clone())),
        None => Err(api_error(StatusCode::NOT_FOUND, "scan_id not found")),
    }
}

async fn handle_api_list_scans(
    State(state): State<LocalApiState>,
    Query(query): Query<ApiScanListQuery>,
) -> Result<Json<Vec<ApiScanJob>>, ApiError> {
    let status_filter = query
        .status
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let limit = query.limit.unwrap_or(50).clamp(1, 250);

    let jobs = state.jobs.read().await;
    let mut items: Vec<ApiScanJob> = jobs.values().cloned().collect();
    if let Some(status) = status_filter {
        items.retain(|job| job.status == status);
    }
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    items.truncate(limit);
    Ok(Json(items))
}

fn build_scan_progress_snapshot(job: &ApiScanJob) -> ApiScanProgressSnapshot {
    match job.status.as_str() {
        "queued" => ApiScanProgressSnapshot {
            scan_id: job.id.clone(),
            status: job.status.clone(),
            phase: "queued".to_string(),
            progress_percent: 0.0,
            indeterminate: false,
            message: "Scan is queued and waiting for execution.".to_string(),
            started_at: job.started_at,
            finished_at: job.finished_at,
        },
        "running" => ApiScanProgressSnapshot {
            scan_id: job.id.clone(),
            status: job.status.clone(),
            phase: "running".to_string(),
            progress_percent: 10.0,
            indeterminate: true,
            message: "Scan is running. Track final outcome via GET /v1/scans/:scan_id.".to_string(),
            started_at: job.started_at,
            finished_at: job.finished_at,
        },
        "completed" => ApiScanProgressSnapshot {
            scan_id: job.id.clone(),
            status: job.status.clone(),
            phase: "completed".to_string(),
            progress_percent: 100.0,
            indeterminate: false,
            message: "Scan completed.".to_string(),
            started_at: job.started_at,
            finished_at: job.finished_at,
        },
        "failed" => ApiScanProgressSnapshot {
            scan_id: job.id.clone(),
            status: job.status.clone(),
            phase: "failed".to_string(),
            progress_percent: 100.0,
            indeterminate: false,
            message: job
                .error
                .clone()
                .unwrap_or_else(|| "Scan failed.".to_string()),
            started_at: job.started_at,
            finished_at: job.finished_at,
        },
        "canceled" => ApiScanProgressSnapshot {
            scan_id: job.id.clone(),
            status: job.status.clone(),
            phase: "canceled".to_string(),
            progress_percent: 100.0,
            indeterminate: false,
            message: job
                .error
                .clone()
                .unwrap_or_else(|| "Scan canceled.".to_string()),
            started_at: job.started_at,
            finished_at: job.finished_at,
        },
        _ => ApiScanProgressSnapshot {
            scan_id: job.id.clone(),
            status: job.status.clone(),
            phase: "unknown".to_string(),
            progress_percent: 0.0,
            indeterminate: true,
            message: "Unknown scan status.".to_string(),
            started_at: job.started_at,
            finished_at: job.finished_at,
        },
    }
}

async fn handle_api_scan_progress(
    State(state): State<LocalApiState>,
    AxumPath(scan_id): AxumPath<String>,
) -> Result<Json<ApiScanProgressSnapshot>, ApiError> {
    let jobs = state.jobs.read().await;
    let job = jobs
        .get(&scan_id)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "scan_id not found"))?;
    Ok(Json(build_scan_progress_snapshot(job)))
}

async fn handle_api_cancel_scan(
    State(state): State<LocalApiState>,
    AxumPath(scan_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut jobs = state.jobs.write().await;
    let job = jobs
        .get_mut(&scan_id)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "scan_id not found"))?;

    match job.status.as_str() {
        "queued" => {
            job.status = "canceled".to_string();
            job.finished_at = Some(now_unix_ts());
            job.error = Some("Canceled via API.".to_string());
            Ok(Json(serde_json::json!({
                "scan_id": scan_id,
                "status": "canceled",
                "message": "Queued scan was canceled."
            })))
        }
        "running" => Err(api_error(
            StatusCode::CONFLICT,
            "Running scans cannot be canceled in current runtime.",
        )),
        _ => Ok(Json(serde_json::json!({
            "scan_id": scan_id,
            "status": job.status,
            "message": "Scan is already in terminal state."
        }))),
    }
}

fn parse_json_value(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or(serde_json::Value::Null)
}

fn map_scan_history_summary(item: db::ScanHistoryItem) -> ApiScanHistorySummary {
    let results = parse_json_value(&item.results_json);
    let results_count = results.as_array().map(|v| v.len()).unwrap_or(0);
    let scan_meta = item
        .scan_meta
        .as_deref()
        .map(parse_json_value)
        .filter(|value| !value.is_null());
    ApiScanHistorySummary {
        id: item.id,
        scanned_at: item.scanned_at,
        total_waste: item.total_waste,
        resource_count: item.resource_count,
        status: item.status,
        results_count,
        scan_meta,
    }
}

fn map_scan_history_detail(item: db::ScanHistoryItem) -> ApiScanHistoryDetail {
    let scan_meta = item
        .scan_meta
        .as_deref()
        .map(parse_json_value)
        .filter(|value| !value.is_null());
    ApiScanHistoryDetail {
        id: item.id,
        scanned_at: item.scanned_at,
        total_waste: item.total_waste,
        resource_count: item.resource_count,
        status: item.status,
        results: parse_json_value(&item.results_json),
        scan_meta,
    }
}

async fn handle_api_list_findings(
    State(state): State<LocalApiState>,
) -> Result<Json<Vec<WastedResource>>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let findings = db::get_scan_results(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(findings))
}

async fn handle_api_list_handled_findings(
    State(state): State<LocalApiState>,
) -> Result<Json<Vec<String>>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let handled = db::get_handled_resources(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(handled))
}

async fn handle_api_mark_finding_handled(
    State(state): State<LocalApiState>,
    AxumPath(resource_id): AxumPath<String>,
    Json(payload): Json<ApiMarkHandledRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if resource_id.trim().is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "resource_id cannot be empty.",
        ));
    }
    let provider = payload
        .provider
        .as_deref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::mark_resource_handled(&conn, &resource_id, &provider, payload.note)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::json!({
        "status": "handled",
        "resource_id": resource_id,
        "provider": provider
    })))
}

async fn handle_api_list_scan_history(
    State(state): State<LocalApiState>,
    Query(query): Query<ApiHistoryQuery>,
) -> Result<Json<Vec<ApiScanHistorySummary>>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let mut history = db::get_scan_history(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if let Some(status) = query
        .status
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    {
        history.retain(|item| item.status.to_ascii_lowercase() == status);
    }

    history.sort_by(|a, b| b.scanned_at.cmp(&a.scanned_at));
    history.truncate(query.limit.unwrap_or(50).clamp(1, 200));

    Ok(Json(
        history
            .into_iter()
            .map(map_scan_history_summary)
            .collect::<Vec<_>>(),
    ))
}

async fn handle_api_get_scan_history_item(
    State(state): State<LocalApiState>,
    AxumPath(history_id): AxumPath<i64>,
) -> Result<Json<ApiScanHistoryDetail>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let history = db::get_scan_history(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let item = history
        .into_iter()
        .find(|record| record.id == history_id)
        .map(map_scan_history_detail)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "scan history id not found"))?;
    Ok(Json(item))
}

async fn handle_api_delete_scan_history_item(
    State(state): State<LocalApiState>,
    AxumPath(history_id): AxumPath<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    db::delete_scan_history(&conn, history_id)
        .await
        .map_err(|e| api_error(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(serde_json::json!({
        "status": "deleted",
        "id": history_id
    })))
}

fn normalize_report_format(format: Option<&str>) -> Result<String, ApiError> {
    let normalized = format.unwrap_or("json").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "json" | "csv" | "pdf" => Ok(normalized),
        _ => Err(api_error(
            StatusCode::BAD_REQUEST,
            "format must be one of: json, csv, pdf.",
        )),
    }
}

fn parse_f64_field(value: &serde_json::Value, key: &str) -> f64 {
    value
        .get(key)
        .and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
        })
        .unwrap_or(0.0)
}

fn parse_string_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn sanitize_ascii(input: &str) -> String {
    input
        .chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect()
}

fn pdf_escape_text(input: &str) -> String {
    sanitize_ascii(input)
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn push_pdf_obj(buffer: &mut Vec<u8>, offsets: &mut Vec<usize>, id: usize, body: &str) {
    offsets.push(buffer.len());
    buffer.extend(format!("{} 0 obj\n{}\nendobj\n", id, body).as_bytes());
}

fn build_simple_pdf_report(lines: &[String]) -> Vec<u8> {
    let mut content = String::new();
    content.push_str("BT\n/F1 11 Tf\n50 780 Td\n14 TL\n");
    for line in lines.iter().take(45) {
        content.push_str(&format!("({}) Tj\nT*\n", pdf_escape_text(line)));
    }
    content.push_str("ET\n");

    let mut buffer = Vec::new();
    buffer.extend(b"%PDF-1.4\n");
    let mut offsets = Vec::new();

    push_pdf_obj(
        &mut buffer,
        &mut offsets,
        1,
        "<< /Type /Catalog /Pages 2 0 R >>",
    );
    push_pdf_obj(
        &mut buffer,
        &mut offsets,
        2,
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    );
    push_pdf_obj(
        &mut buffer,
        &mut offsets,
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
    );
    push_pdf_obj(
        &mut buffer,
        &mut offsets,
        4,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    );
    let stream_obj = format!(
        "<< /Length {} >>\nstream\n{}endstream",
        content.as_bytes().len(),
        content
    );
    push_pdf_obj(&mut buffer, &mut offsets, 5, &stream_obj);

    let xref_pos = buffer.len();
    buffer.extend(format!("xref\n0 {}\n", offsets.len() + 1).as_bytes());
    buffer.extend(b"0000000000 65535 f \n");
    for offset in offsets {
        buffer.extend(format!("{:010} 00000 n \n", offset).as_bytes());
    }
    buffer.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            6, xref_pos
        )
        .as_bytes(),
    );
    buffer
}

fn build_report_csv(rows: &[serde_json::Value], include_esg: bool, total_waste: f64) -> Vec<u8> {
    let mut csv = String::from(
        "provider,resource_id,name,resource_type,region,estimated_monthly_cost,currency\n",
    );
    for row in rows {
        let provider = parse_string_field(row, "provider");
        let resource_id = parse_string_field(row, "id");
        let name = parse_string_field(row, "name");
        let resource_type = parse_string_field(row, "resource_type");
        let region = parse_string_field(row, "region");
        let cost = parse_f64_field(row, "estimated_monthly_cost");
        csv.push_str(&format!(
            "{},{},{},{},{},{:.2},USD\n",
            csv_escape(&provider),
            csv_escape(&resource_id),
            csv_escape(&name),
            csv_escape(&resource_type),
            csv_escape(&region),
            cost
        ));
    }
    if include_esg {
        let co2e = total_waste * 0.42;
        csv.push_str(&format!(
            "\nsummary_metric,value\nestimated_co2e_kg_monthly,{:.2}\n",
            co2e
        ));
    }
    csv.into_bytes()
}

async fn collect_report_source(
    conn: &sqlx::Pool<sqlx::Sqlite>,
    requested_history_id: Option<i64>,
) -> Result<(Option<i64>, i64, f64, String, Vec<serde_json::Value>), ApiError> {
    let mut history = db::get_scan_history(conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    history.sort_by(|a, b| b.scanned_at.cmp(&a.scanned_at));

    if let Some(history_id) = requested_history_id {
        let item = history
            .into_iter()
            .find(|entry| entry.id == history_id)
            .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "scan history id not found"))?;
        let results = parse_json_value(&item.results_json)
            .as_array()
            .cloned()
            .unwrap_or_default();
        return Ok((
            Some(item.id),
            item.scanned_at,
            item.total_waste,
            item.status,
            results,
        ));
    }

    if let Some(item) = history.into_iter().next() {
        let results = parse_json_value(&item.results_json)
            .as_array()
            .cloned()
            .unwrap_or_default();
        return Ok((
            Some(item.id),
            item.scanned_at,
            item.total_waste,
            item.status,
            results,
        ));
    }

    let findings = db::get_scan_results(conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let total_waste: f64 = findings.iter().map(|row| row.estimated_monthly_cost).sum();
    let rows = findings
        .into_iter()
        .map(|row| serde_json::to_value(row).unwrap_or(serde_json::Value::Null))
        .collect::<Vec<_>>();
    Ok((
        None,
        now_unix_ts(),
        total_waste,
        "current".to_string(),
        rows,
    ))
}

async fn handle_api_reports_overview(
    State(state): State<LocalApiState>,
    Query(query): Query<ApiGovernanceQuery>,
) -> Result<Json<GovernanceStatsResponse>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let history = db::get_scan_history(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(compute_governance_stats(history, query.window_days)))
}

async fn handle_api_reports_trend(
    State(state): State<LocalApiState>,
    Query(query): Query<ApiGovernanceQuery>,
) -> Result<Json<GovernanceTrendResponse>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let history = db::get_scan_history(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let stats = compute_governance_stats(history, query.window_days);
    Ok(Json(GovernanceTrendResponse {
        generated_at: stats.generated_at,
        window_days: stats.window_days,
        window_start_ts: stats.window_start_ts,
        window_end_ts: stats.window_end_ts,
        daily: stats.daily,
    }))
}

async fn handle_api_reports_error_taxonomy(
    State(state): State<LocalApiState>,
    Query(query): Query<ApiGovernanceQuery>,
) -> Result<Json<GovernanceErrorTaxonomyResponse>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let history = db::get_scan_history(&conn)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let stats = compute_governance_stats(history, query.window_days);
    Ok(Json(GovernanceErrorTaxonomyResponse {
        generated_at: stats.generated_at,
        window_days: stats.window_days,
        window_start_ts: stats.window_start_ts,
        window_end_ts: stats.window_end_ts,
        error_taxonomy: stats.error_taxonomy,
    }))
}

async fn handle_api_generate_report(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiGenerateReportRequest>,
) -> Result<Json<ApiReportArtifact>, ApiError> {
    let format = normalize_report_format(payload.format.as_deref())?;
    let include_esg = payload.include_esg.unwrap_or(true);

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let (scan_history_id, scanned_at, total_waste, status, rows) =
        collect_report_source(&conn, payload.scan_history_id).await?;

    let report_id = Uuid::new_v4().to_string();
    let short_id = report_id.chars().take(8).collect::<String>();
    let extension = format.as_str();
    let filename = format!("cws-report-{}-{}.{}", scanned_at, short_id, extension);
    let path = state.report_dir.join(&filename);

    let bytes = match format.as_str() {
        "json" => serde_json::to_vec_pretty(&serde_json::json!({
            "report_id": report_id,
            "generated_at": now_unix_ts(),
            "scan_history_id": scan_history_id,
            "scan_status": status,
            "total_waste": total_waste,
            "estimated_co2e_kg_monthly": if include_esg { serde_json::json!(total_waste * 0.42) } else { serde_json::Value::Null },
            "findings": rows
        }))
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
        "csv" => build_report_csv(&rows, include_esg, total_waste),
        "pdf" => {
            let mut lines = vec![
                "Cloud Waste Scanner Report".to_string(),
                format!("Generated at (unix): {}", now_unix_ts()),
                format!("Scan history ID: {}", scan_history_id.map(|v| v.to_string()).unwrap_or_else(|| "none".to_string())),
                format!("Status: {}", status),
                format!("Potential Monthly Savings: ${:.2}", total_waste),
            ];
            if include_esg {
                lines.push(format!(
                    "Estimated CO2e Reduction: {:.2} kg/month",
                    total_waste * 0.42
                ));
            }
            lines.push(String::new());
            lines.push("Top Findings:".to_string());
            for row in rows.iter().take(30) {
                lines.push(format!(
                    "- {} | {} | ${:.2}",
                    parse_string_field(row, "provider"),
                    parse_string_field(row, "resource_type"),
                    parse_f64_field(row, "estimated_monthly_cost")
                ));
            }
            build_simple_pdf_report(&lines)
        }
        _ => unreachable!(),
    };

    std::fs::write(&path, &bytes)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let content_type = match format.as_str() {
        "json" => "application/json",
        "csv" => "text/csv",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
    .to_string();

    let meta = ApiReportArtifact {
        report_id: report_id.clone(),
        format: format.clone(),
        content_type,
        filename: filename.clone(),
        size_bytes: bytes.len() as u64,
        created_at: now_unix_ts(),
        scan_history_id,
        include_esg,
    };

    {
        let mut reports = state.reports.write().await;
        reports.insert(
            report_id.clone(),
            ApiReportArtifactStored {
                meta: meta.clone(),
                file_path: path,
            },
        );
    }

    Ok(Json(meta))
}

async fn handle_api_list_reports(
    State(state): State<LocalApiState>,
    Query(query): Query<ApiReportListQuery>,
) -> Result<Json<Vec<ApiReportArtifact>>, ApiError> {
    let format_filter = query
        .format
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let limit = query.limit.unwrap_or(50).clamp(1, 200);

    let reports = state.reports.read().await;
    let mut items = reports
        .values()
        .map(|stored| stored.meta.clone())
        .collect::<Vec<_>>();
    if let Some(format) = format_filter {
        items.retain(|item| item.format == format);
    }
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    items.truncate(limit);
    Ok(Json(items))
}

async fn handle_api_get_report(
    State(state): State<LocalApiState>,
    AxumPath(report_id): AxumPath<String>,
) -> Result<Json<ApiReportArtifact>, ApiError> {
    let reports = state.reports.read().await;
    let item = reports
        .get(&report_id)
        .map(|stored| stored.meta.clone())
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "report id not found"))?;
    Ok(Json(item))
}

async fn handle_api_download_report(
    State(state): State<LocalApiState>,
    AxumPath(report_id): AxumPath<String>,
) -> Result<Response, ApiError> {
    let stored = {
        let reports = state.reports.read().await;
        reports
            .get(&report_id)
            .cloned()
            .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "report id not found"))?
    };

    let bytes = std::fs::read(&stored.file_path)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut response = Response::new(Body::from(bytes));
    if let Ok(value) = HeaderValue::from_str(&stored.meta.content_type) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    if let Ok(value) = HeaderValue::from_str(&format!(
        "attachment; filename=\"{}\"",
        stored.meta.filename
    )) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    Ok(response)
}

async fn load_api_webhooks(conn: &sqlx::Pool<sqlx::Sqlite>) -> Vec<ApiWebhookConfig> {
    let raw = db::get_setting(conn, API_WEBHOOKS_SETTING_KEY)
        .await
        .unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str::<Vec<ApiWebhookConfig>>(&raw).unwrap_or_default()
}

async fn save_api_webhooks(
    conn: &sqlx::Pool<sqlx::Sqlite>,
    webhooks: &[ApiWebhookConfig],
) -> Result<(), ApiError> {
    let payload = serde_json::to_string(webhooks)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    db::save_setting(conn, API_WEBHOOKS_SETTING_KEY, &payload)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))
}

fn normalize_webhook_events(events: Option<Vec<String>>) -> Vec<String> {
    let mut normalized = events
        .unwrap_or_else(|| vec!["scan.completed".to_string(), "scan.failed".to_string()])
        .into_iter()
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

async fn dispatch_scan_webhooks(
    state: &LocalApiState,
    scan_id: &str,
    status: &str,
    resources_found: Option<usize>,
    estimated_monthly_savings: Option<f64>,
    error: Option<String>,
) {
    let app_state = state.app_handle.state::<AppState>();
    let conn = match db::init_db(&app_state.db_path).await {
        Ok(pool) => pool,
        Err(_) => return,
    };
    let webhooks = load_api_webhooks(&conn).await;
    if webhooks.is_empty() {
        return;
    }

    let event = match status {
        "completed" => "scan.completed",
        "failed" => "scan.failed",
        _ => return,
    };

    let payload = serde_json::json!({
        "event": event,
        "scan_id": scan_id,
        "status": status,
        "resources_found": resources_found,
        "estimated_monthly_savings": estimated_monthly_savings,
        "error": error,
        "timestamp": now_unix_ts()
    });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(client) => client,
        Err(_) => return,
    };

    for hook in webhooks {
        if !hook.is_active {
            continue;
        }
        if !hook.events.is_empty()
            && !hook
                .events
                .iter()
                .any(|registered| registered.eq_ignore_ascii_case(event))
        {
            continue;
        }
        let _ = client.post(&hook.url).json(&payload).send().await;
    }
}

async fn handle_api_list_events(
    State(state): State<LocalApiState>,
    Query(query): Query<ApiEventQuery>,
) -> Result<Json<Vec<db::AuditLog>>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(50).clamp(1, 200) as i64;
    let offset = (page - 1) * limit;
    let logs = db::get_audit_logs(&conn, None, None, limit, offset)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(logs))
}

async fn handle_api_event_types() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "events": [
            "scan.completed",
            "scan.failed",
            "scan.started",
            "notification.tested"
        ]
    }))
}

async fn handle_api_list_webhooks(
    State(state): State<LocalApiState>,
) -> Result<Json<Vec<ApiWebhookConfig>>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(load_api_webhooks(&conn).await))
}

async fn handle_api_create_webhook(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiWebhookCreateRequest>,
) -> Result<Json<ApiWebhookConfig>, ApiError> {
    let url = payload.url.trim().to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "webhook url must start with http:// or https://",
        ));
    }
    let name = payload
        .name
        .as_deref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "webhook".to_string());
    let hook = ApiWebhookConfig {
        id: Uuid::new_v4().to_string(),
        name,
        url,
        events: normalize_webhook_events(payload.events),
        is_active: payload.is_active.unwrap_or(true),
        created_at: now_unix_ts(),
    };

    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let mut hooks = load_api_webhooks(&conn).await;
    hooks.push(hook.clone());
    save_api_webhooks(&conn, &hooks).await?;
    Ok(Json(hook))
}

async fn handle_api_delete_webhook(
    State(state): State<LocalApiState>,
    AxumPath(webhook_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let hooks = load_api_webhooks(&conn).await;
    let filtered = hooks
        .into_iter()
        .filter(|hook| hook.id != webhook_id)
        .collect::<Vec<_>>();
    save_api_webhooks(&conn, &filtered).await?;
    Ok(Json(serde_json::json!({
        "status": "deleted",
        "id": webhook_id
    })))
}

async fn handle_api_test_webhook(
    State(state): State<LocalApiState>,
    AxumPath(webhook_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app_state = state.app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let hooks = load_api_webhooks(&conn).await;
    let hook = hooks
        .into_iter()
        .find(|item| item.id == webhook_id)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "webhook id not found"))?;

    let payload = serde_json::json!({
        "event": "webhook.test",
        "timestamp": now_unix_ts(),
        "message": "Cloud Waste Scanner webhook test"
    });

    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .post(&hook.url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| api_error(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "ok": response.status().is_success(),
        "status": response.status().as_u16()
    })))
}

async fn handle_api_mcp_capabilities() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "mcp_beta": true,
        "tools": [
            {"name": "run_scan", "route": "POST /v1/mcp/tools/run-scan"},
            {"name": "get_scan", "route": "GET /v1/mcp/tools/get-scan/:scan_id"}
        ]
    }))
}

async fn handle_api_mcp_run_scan(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiScanRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let accepted = enqueue_scan_job(&state, payload, Some("api:mcp".to_string()))
        .await
        .map_err(map_scan_enqueue_error)?;
    Ok(Json(serde_json::json!({
        "tool": "run_scan",
        "result": accepted
    })))
}

async fn handle_api_mcp_get_scan(
    State(state): State<LocalApiState>,
    AxumPath(scan_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let jobs = state.jobs.read().await;
    let job = jobs
        .get(&scan_id)
        .cloned()
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "scan_id not found"))?;
    Ok(Json(serde_json::json!({
        "tool": "get_scan",
        "result": job
    })))
}

async fn handle_api_scan(
    State(state): State<LocalApiState>,
    maybe_payload: Option<Json<ApiScanRequest>>,
) -> Result<Json<ApiScanAccepted>, ApiError> {
    let payload = maybe_payload.map(|Json(p)| p).unwrap_or_default();

    enqueue_scan_job(&state, payload, Some("api:manual".to_string()))
        .await
        .map(Json)
        .map_err(map_scan_enqueue_error)
}

async fn handle_api_create_schedule(
    State(state): State<LocalApiState>,
    Json(payload): Json<ApiScheduleRequest>,
) -> Result<Json<ApiSchedule>, ApiError> {
    if !local_api_schedule_entitled(&state).await {
        return Err(schedule_gate_error());
    }
    validate_scan_request(&payload.scan).map_err(|e| api_error(StatusCode::BAD_REQUEST, e))?;

    let now = now_unix_ts();
    let next_run = calculate_initial_next_run(payload.run_at, payload.interval_minutes, now)
        .map_err(|e| api_error(StatusCode::BAD_REQUEST, e))?;

    let schedule = ApiSchedule {
        id: Uuid::new_v4().to_string(),
        name: payload.name.unwrap_or_else(|| format!("schedule-{}", now)),
        enabled: payload.enabled.unwrap_or(true),
        run_at: payload.run_at.unwrap_or(next_run),
        interval_minutes: payload.interval_minutes,
        timezone: payload.timezone.or(Some("UTC".to_string())),
        next_run_at: if payload.enabled.unwrap_or(true) {
            Some(next_run)
        } else {
            None
        },
        last_run_at: None,
        last_scan_id: None,
        last_error: None,
        created_at: now,
        updated_at: now,
        scan: payload.scan,
    };

    {
        let mut schedules = state.schedules.write().await;
        schedules.insert(schedule.id.clone(), schedule.clone());
    }

    persist_schedules_to_db(&state)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(schedule))
}

async fn handle_api_list_schedules(
    State(state): State<LocalApiState>,
) -> Result<Json<Vec<ApiSchedule>>, ApiError> {
    if !local_api_schedule_entitled(&state).await {
        return Err(schedule_gate_error());
    }
    let list = {
        let schedules = state.schedules.read().await;
        list_schedules_sorted(&schedules)
    };
    Ok(Json(list))
}

async fn handle_api_get_schedule(
    State(state): State<LocalApiState>,
    AxumPath(schedule_id): AxumPath<String>,
) -> Result<Json<ApiSchedule>, ApiError> {
    if !local_api_schedule_entitled(&state).await {
        return Err(schedule_gate_error());
    }
    let schedules = state.schedules.read().await;
    match schedules.get(&schedule_id) {
        Some(schedule) => Ok(Json(schedule.clone())),
        None => Err(api_error(StatusCode::NOT_FOUND, "schedule_id not found")),
    }
}

async fn handle_api_delete_schedule(
    State(state): State<LocalApiState>,
    AxumPath(schedule_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !local_api_schedule_entitled(&state).await {
        return Err(schedule_gate_error());
    }
    let removed = {
        let mut schedules = state.schedules.write().await;
        remove_schedule(&mut schedules, &schedule_id)
    };

    if !removed {
        return Err(api_error(StatusCode::NOT_FOUND, "schedule_id not found"));
    }

    persist_schedules_to_db(&state)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(
        serde_json::json!({ "status": "deleted", "schedule_id": schedule_id }),
    ))
}

async fn handle_api_run_schedule_now(
    State(state): State<LocalApiState>,
    AxumPath(schedule_id): AxumPath<String>,
) -> Result<Json<ApiScanAccepted>, ApiError> {
    if !local_api_schedule_entitled(&state).await {
        return Err(schedule_gate_error());
    }
    let schedule = {
        let mut schedules = state.schedules.write().await;
        let Some(schedule) = schedules.get_mut(&schedule_id) else {
            return Err(api_error(StatusCode::NOT_FOUND, "schedule_id not found"));
        };

        schedule.last_run_at = Some(now_unix_ts());
        schedule.updated_at = now_unix_ts();
        schedule.last_error = None;
        schedule.clone()
    };

    let source = Some(format!("schedule:{}:run-now", schedule_id));
    let accepted = enqueue_scan_job(&state, schedule.scan.clone(), source)
        .await
        .map_err(map_scan_enqueue_error)?;

    {
        let mut schedules = state.schedules.write().await;
        if let Some(item) = schedules.get_mut(&schedule_id) {
            item.last_scan_id = Some(accepted.scan_id.clone());
            item.updated_at = now_unix_ts();
        }
    }

    persist_schedules_to_db(&state)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(accepted))
}

fn parse_bool_setting(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "enabled"
    )
}

fn is_loopback_bind_host(bind_host: &str) -> bool {
    let normalized = bind_host.trim().to_ascii_lowercase();
    normalized == "127.0.0.1" || normalized == "localhost" || normalized == "::1"
}

fn default_api_tls_enabled(bind_host: &str) -> bool {
    !is_loopback_bind_host(bind_host)
}

fn detect_primary_local_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip())
}

fn build_self_signed_san_entries(bind_host: &str) -> Vec<String> {
    let mut entries = HashSet::new();
    entries.insert("localhost".to_string());
    entries.insert("127.0.0.1".to_string());
    entries.insert("::1".to_string());

    let host = bind_host.trim();
    if !host.is_empty() && host != "0.0.0.0" && host != "::" {
        entries.insert(host.to_string());
    }

    if let Some(primary_ip) = detect_primary_local_ip() {
        entries.insert(primary_ip.to_string());
    }

    let mut sorted = entries.into_iter().collect::<Vec<_>>();
    sorted.sort();
    sorted
}

fn ensure_local_api_self_signed_cert(
    app_data_dir: &std::path::Path,
    bind_host: &str,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    std::fs::create_dir_all(app_data_dir).map_err(|e| {
        format!(
            "failed to create app data dir for local api certs ({}): {}",
            app_data_dir.display(),
            e
        )
    })?;

    let cert_path = app_data_dir.join("local-api-cert.pem");
    let key_path = app_data_dir.join("local-api-key.pem");

    let cert_exists = std::fs::metadata(&cert_path)
        .map(|meta| meta.len() > 0)
        .unwrap_or(false);
    let key_exists = std::fs::metadata(&key_path)
        .map(|meta| meta.len() > 0)
        .unwrap_or(false);
    if cert_exists && key_exists {
        return Ok((cert_path, key_path));
    }

    let san_entries = build_self_signed_san_entries(bind_host);
    let generated = generate_simple_self_signed(san_entries.clone())
        .map_err(|e| format!("failed to generate self-signed cert: {}", e))?;
    let cert_pem = generated.cert.pem();
    let key_pem = generated.key_pair.serialize_pem();

    std::fs::write(&cert_path, cert_pem)
        .map_err(|e| format!("failed to write {}: {}", cert_path.display(), e))?;
    std::fs::write(&key_path, key_pem)
        .map_err(|e| format!("failed to write {}: {}", key_path.display(), e))?;

    log_startup_event(&format!(
        "generated local api self-signed cert at {} (san_count={})",
        cert_path.display(),
        san_entries.len()
    ));

    Ok((cert_path, key_path))
}

async fn resolve_api_socket_addr(
    bind_host: &str,
    port: u16,
) -> Result<std::net::SocketAddr, String> {
    let addr = format!("{}:{}", bind_host.trim(), port);
    let mut resolved = tokio::net::lookup_host(&addr)
        .await
        .map_err(|e| format!("failed to resolve local api address {}: {}", addr, e))?;
    resolved
        .next()
        .ok_or_else(|| format!("no socket addresses resolved for {}", addr))
}

async fn start_api_server(
    app_handle: tauri::AppHandle,
    bind_host: String,
    port: u16,
    api_access_token: String,
    api_enabled: bool,
    tls_enabled: bool,
    app_data_dir: std::path::PathBuf,
) {
    let report_dir = app_data_dir.join("api-reports");
    let _ = std::fs::create_dir_all(&report_dir);

    let state = LocalApiState {
        app_version: app_handle.package_info().version.to_string(),
        bind_host: bind_host.clone(),
        port,
        api_access_token,
        api_enabled,
        tls_enabled,
        app_handle,
        jobs: Arc::new(RwLock::new(HashMap::new())),
        schedules: Arc::new(RwLock::new(HashMap::new())),
        auth_rate_buckets: Arc::new(RwLock::new(HashMap::new())),
        reports: Arc::new(RwLock::new(HashMap::new())),
        report_dir,
    };

    if let Ok(initial_schedules) = load_schedules_from_db(&state).await {
        let mut schedules = state.schedules.write().await;
        *schedules = initial_schedules;
    }

    {
        let scheduler_state = state.clone();
        tauri::async_runtime::spawn(async move {
            scheduler_loop(scheduler_state).await;
        });
    }

    let app = Router::new()
        .route("/status", get(handle_api_status))
        .route("/v1/meta/capabilities", get(handle_api_capabilities))
        .route("/v1/meta/error-model", get(handle_api_error_model))
        .route("/v1/settings/general", get(handle_api_settings_general))
        .route(
            "/v1/settings/general",
            patch(handle_api_patch_settings_general),
        )
        .route("/v1/settings/network", get(handle_api_settings_network))
        .route(
            "/v1/settings/network",
            patch(handle_api_patch_settings_network),
        )
        .route(
            "/v1/settings/scan-policy",
            get(handle_api_settings_scan_policy),
        )
        .route(
            "/v1/settings/scan-policy",
            patch(handle_api_patch_settings_scan_policy),
        )
        .route("/v1/settings/license", get(handle_api_settings_license))
        .route("/scan", post(handle_api_scan))
        .route("/api/scan", post(handle_api_scan))
        .route("/v1/scans", get(handle_api_list_scans))
        .route("/v1/scans", post(handle_api_scan))
        .route("/v1/scans/:scan_id", get(handle_api_get_scan))
        .route("/v1/scans/:scan_id/progress", get(handle_api_scan_progress))
        .route("/v1/scans/:scan_id/cancel", post(handle_api_cancel_scan))
        .route("/v1/findings", get(handle_api_list_findings))
        .route(
            "/v1/findings/handled",
            get(handle_api_list_handled_findings),
        )
        .route(
            "/v1/findings/:resource_id/handled",
            post(handle_api_mark_finding_handled),
        )
        .route("/v1/scan-history", get(handle_api_list_scan_history))
        .route(
            "/v1/scan-history/:history_id",
            get(handle_api_get_scan_history_item),
        )
        .route(
            "/v1/scan-history/:history_id",
            delete(handle_api_delete_scan_history_item),
        )
        .route("/v1/reports/generate", post(handle_api_generate_report))
        .route("/v1/reports", get(handle_api_list_reports))
        .route("/v1/reports/overview", get(handle_api_reports_overview))
        .route("/v1/reports/trend", get(handle_api_reports_trend))
        .route(
            "/v1/reports/error-taxonomy",
            get(handle_api_reports_error_taxonomy),
        )
        .route("/v1/reports/:report_id", get(handle_api_get_report))
        .route(
            "/v1/reports/:report_id/download",
            get(handle_api_download_report),
        )
        .route("/v1/events", get(handle_api_list_events))
        .route("/v1/events/types", get(handle_api_event_types))
        .route("/v1/webhooks", get(handle_api_list_webhooks))
        .route("/v1/webhooks", post(handle_api_create_webhook))
        .route(
            "/v1/webhooks/:webhook_id",
            delete(handle_api_delete_webhook),
        )
        .route(
            "/v1/webhooks/:webhook_id/test",
            post(handle_api_test_webhook),
        )
        .route("/v1/mcp/capabilities", get(handle_api_mcp_capabilities))
        .route("/v1/mcp/tools/run-scan", post(handle_api_mcp_run_scan))
        .route(
            "/v1/mcp/tools/get-scan/:scan_id",
            get(handle_api_mcp_get_scan),
        )
        .route("/v1/accounts", get(handle_api_accounts))
        .route("/v1/cloud-accounts", get(handle_api_list_cloud_accounts))
        .route("/v1/cloud-accounts", post(handle_api_create_cloud_account))
        .route(
            "/v1/cloud-accounts/:account_id",
            get(handle_api_get_cloud_account),
        )
        .route(
            "/v1/cloud-accounts/:account_id",
            patch(handle_api_update_cloud_account),
        )
        .route(
            "/v1/cloud-accounts/:account_id",
            delete(handle_api_delete_cloud_account),
        )
        .route(
            "/v1/cloud-accounts/test",
            post(handle_api_test_cloud_account),
        )
        .route("/v1/proxies", get(handle_api_list_proxies))
        .route("/v1/proxies", post(handle_api_create_proxy))
        .route("/v1/proxies/:proxy_id", get(handle_api_get_proxy))
        .route("/v1/proxies/:proxy_id", patch(handle_api_update_proxy))
        .route("/v1/proxies/:proxy_id", delete(handle_api_delete_proxy))
        .route("/v1/proxies/test", post(handle_api_test_proxy))
        .route(
            "/v1/notifications/channels",
            get(handle_api_list_notification_channels),
        )
        .route(
            "/v1/notifications/channels",
            post(handle_api_create_notification_channel),
        )
        .route(
            "/v1/notifications/channels/:channel_id",
            patch(handle_api_update_notification_channel),
        )
        .route(
            "/v1/notifications/channels/:channel_id",
            delete(handle_api_delete_notification_channel),
        )
        .route(
            "/v1/notifications/channels/test",
            post(handle_api_test_notification_channel),
        )
        .route(
            "/v1/notifications/policy",
            get(handle_api_get_notification_policy),
        )
        .route(
            "/v1/notifications/policy",
            patch(handle_api_patch_notification_policy),
        )
        .route("/v1/schedules", get(handle_api_list_schedules))
        .route("/v1/schedules", post(handle_api_create_schedule))
        .route("/v1/schedules/:schedule_id", get(handle_api_get_schedule))
        .route(
            "/v1/schedules/:schedule_id",
            delete(handle_api_delete_schedule),
        )
        .route(
            "/v1/schedules/:schedule_id/run-now",
            post(handle_api_run_schedule_now),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            enforce_api_access,
        ))
        .layer(DefaultBodyLimit::max(API_MAX_REQUEST_BODY_BYTES))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let socket_addr = match resolve_api_socket_addr(&bind_host, port).await {
        Ok(addr) => addr,
        Err(e) => {
            log_startup_event(&format!(
                "failed to resolve local api socket address: {}",
                e
            ));
            eprintln!("❌ {}", e);
            return;
        }
    };

    let scheme = if tls_enabled { "https" } else { "http" };
    println!(
        "🚀 Local API Server attempting to bind to {}://{}",
        scheme, socket_addr
    );
    log_startup_event(&format!(
        "local api attempting bind {}://{} (tls_enabled={})",
        scheme, socket_addr, tls_enabled
    ));

    if tls_enabled {
        let (cert_path, key_path) =
            match ensure_local_api_self_signed_cert(&app_data_dir, &bind_host) {
                Ok(paths) => paths,
                Err(e) => {
                    log_startup_event(&format!(
                        "failed to prepare local api self-signed cert: {}",
                        e
                    ));
                    eprintln!("❌ Failed to prepare local api self-signed cert: {}", e);
                    return;
                }
            };

        let rustls_config =
            match RustlsConfig::from_pem_file(cert_path.clone(), key_path.clone()).await {
                Ok(cfg) => cfg,
                Err(e) => {
                    log_startup_event(&format!("failed to load local api tls config: {}", e));
                    eprintln!("❌ Failed to load local api tls config: {}", e);
                    return;
                }
            };

        println!("🚀 Local API Server ready at https://{}", socket_addr);
        log_startup_event(&format!(
            "local api ready at https://{} with cert={}",
            socket_addr,
            cert_path.display()
        ));

        if let Err(e) = axum_server::bind_rustls(socket_addr, rustls_config)
            .serve(app.into_make_service_with_connect_info::<std::net::SocketAddr>())
            .await
        {
            log_startup_event(&format!("local api tls server exited with error: {}", e));
            eprintln!("❌ Local API TLS server failed: {}", e);
        }
        return;
    }

    if let Ok(listener) = tokio::net::TcpListener::bind(socket_addr).await {
        println!("🚀 Local API Server ready at http://{}", socket_addr);
        log_startup_event(&format!("local api ready at http://{}", socket_addr));
        if let Err(e) = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        {
            log_startup_event(&format!("local api http server exited with error: {}", e));
            eprintln!("❌ Local API HTTP server failed: {}", e);
        }
    } else {
        let msg = format!("failed to bind api server to {}", socket_addr);
        log_startup_event(&msg);
        eprintln!("❌ {}", msg);
    }
}

#[tauri::command]
async fn track_event(
    app_handle: tauri::AppHandle,
    event: String,
    meta: serde_json::Value,
) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let pool = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    report_telemetry(&pool, &event, meta).await;
    Ok(())
}

#[tauri::command]
async fn apply_proxy_settings(app_handle: tauri::AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;

    let mode = db::get_setting(&conn, "proxy_mode")
        .await
        .unwrap_or("none".into());
    let url = db::get_setting(&conn, "proxy_url")
        .await
        .unwrap_or_default();

    configure_proxy_env(&mode, &url);
    Ok(())
}

#[tauri::command]
async fn test_proxy_connection(
    proxy_mode: String,
    proxy_url: Option<String>,
) -> Result<String, String> {
    let mode = normalize_proxy_mode(&proxy_mode);
    let url = proxy_url.unwrap_or_default().trim().to_string();
    // Keep proxy test lightweight and deterministic so users get fast feedback.
    // Provider-specific reachability is validated in account-level "Test Connection".
    let probe_target = "https://example.com/";
    log_startup_event(&format!(
        "proxy test started: mode={} endpoint={} target={}",
        mode,
        proxy_endpoint_display(&mode, &url),
        probe_target
    ));

    if mode == "custom" {
        if url.is_empty() {
            let err = "Custom proxy mode requires a proxy URL.".to_string();
            log_startup_event(&format!(
                "proxy test failed: mode={} endpoint={} error=\"{}\"",
                mode,
                proxy_endpoint_display(&mode, &url),
                summarize_error_text(&err, 220)
            ));
            return Err(err);
        }
        if let Err(proxy_err) = precheck_proxy_connectivity(&url).await {
            let err = format_connection_failure_message(
                "Proxy test",
                "proxy_connect",
                "proxy_unreachable",
                &mode,
                &url,
                &proxy_err,
            );
            log_startup_event(&format!(
                "proxy test failed: mode={} endpoint={} error=\"{}\"",
                mode,
                proxy_endpoint_display(&mode, &url),
                summarize_error_text(&err, 280)
            ));
            return Err(err);
        }
    }

    let started = std::time::Instant::now();
    let mut client_builder = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(6));

    if mode == "custom" {
        let normalized_url = normalize_custom_proxy_url(&url);
        let proxy = reqwest::Proxy::all(&normalized_url)
            .map_err(|e| format!("Invalid proxy configuration: {}", e))?;
        client_builder = client_builder.proxy(proxy);
    } else if mode == "none" {
        client_builder = client_builder.no_proxy();
    }

    let client = client_builder
        .build()
        .map_err(|e| format!("Failed to initialize proxy test client: {}", e))?;

    let response = client.head(probe_target).send().await.map_err(|err| {
        let raw = err.to_string();
        let (stage, reason_code, message) = classify_cloud_connectivity_failure(&raw, &mode);
        let formatted = format_connection_failure_message(
            "Proxy test",
            &stage,
            &reason_code,
            &mode,
            &url,
            &message,
        );
        log_startup_event(&format!(
            "proxy test failed: mode={} endpoint={} stage={} reason={} raw=\"{}\"",
            mode,
            proxy_endpoint_display(&mode, &url),
            stage,
            reason_code,
            summarize_error_text(&raw, 280)
        ));
        formatted
    })?;

    let success = format!(
        "Proxy test passed. mode={} endpoint={} target={} status={} latency={} ms",
        mode,
        proxy_endpoint_display(&mode, &url),
        probe_target,
        response.status().as_u16(),
        started.elapsed().as_millis()
    );
    log_startup_event(&format!(
        "proxy test success: mode={} endpoint={} status={} latency_ms={}",
        mode,
        proxy_endpoint_display(&mode, &url),
        response.status().as_u16(),
        started.elapsed().as_millis()
    ));
    Ok(success)
}

#[tauri::command]
fn validate_license_key(key: String) -> Result<license::LicensePayload, String> {
    let started = std::time::Instant::now();
    let result = license::verify_license(&key);
    let elapsed_ms = started.elapsed().as_millis();
    match &result {
        Ok(payload) => {
            log_startup_event(&format!(
                "validate_license_key success: type={:?} expires_at={:?} elapsed_ms={}",
                payload.l_type, payload.expires_at, elapsed_ms
            ));
        }
        Err(err) => {
            log_startup_event(&format!(
                "validate_license_key failed: elapsed_ms={} reason=\"{}\"",
                elapsed_ms,
                summarize_error_text(err, 200)
            ));
        }
    }
    result
}

#[tauri::command]
async fn check_license_status(
    app_handle: tauri::AppHandle,
    _key: String,
) -> Result<license::CheckResponse, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    let status = license::CheckResponse {
        valid: true,
        latest_version: env!("CARGO_PKG_VERSION").to_string(),
        download_url: None,
        download_urls: None,
        message: Some("Community edition runs fully local. Remote license checks are disabled.".to_string()),
        quota: None,
        max_quota: None,
        plan_type: Some("community".to_string()),
        is_trial: Some(false),
        trial_expires_at: None,
        api_enabled: Some(true),
        resource_details_enabled: Some(true),
        customer_email: None,
        license_started_at: None,
        first_purchase_at: None,
        latest_purchase_at: None,
        purchase_count: None,
        latest_order_ref: None,
        latest_order_amount: None,
        latest_order_plan: None,
        latest_order_status: None,
        order_history: None,
    };
    persist_runtime_plan_type_from_status(&conn, &status).await;
    Ok(status)
}

#[tauri::command]
async fn start_trial_license(
    app_handle: tauri::AppHandle,
    _email: Option<String>,
) -> Result<TrialStartResult, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    let local_key = format!("community-local-{}", Uuid::new_v4().to_string().replace('-', ""));
    save_license_file(app_handle.clone(), local_key)?;
    let status = license::CheckResponse {
        valid: true,
        latest_version: env!("CARGO_PKG_VERSION").to_string(),
        download_url: None,
        download_urls: None,
        message: Some("Community mode is active.".to_string()),
        quota: None,
        max_quota: None,
        plan_type: Some("community".to_string()),
        is_trial: Some(false),
        trial_expires_at: None,
        api_enabled: Some(true),
        resource_details_enabled: Some(true),
        customer_email: None,
        license_started_at: None,
        first_purchase_at: None,
        latest_purchase_at: None,
        purchase_count: None,
        latest_order_ref: None,
        latest_order_amount: None,
        latest_order_plan: None,
        latest_order_status: None,
        order_history: None,
    };
    persist_runtime_plan_type_from_status(&conn, &status).await;

    Ok(TrialStartResult {
        status: "community".to_string(),
        plan_type: "community".to_string(),
        trial_expires_at: None,
        quota: None,
        max_quota: None,
        message: Some("Community mode is active.".to_string()),
    })
}

#[tauri::command]
fn list_aws_profiles() -> Result<Vec<aws_utils::AwsProfile>, String> {
    aws_utils::list_profiles()
}

#[tauri::command]
fn discover_importable_cloud_accounts() -> Result<Vec<CloudImportCandidate>, String> {
    let mut candidates: Vec<CloudImportCandidate> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut existing_aws_names: HashSet<String> = HashSet::new();

    if let Ok(aws_profiles) = aws_utils::list_profiles() {
        for profile in aws_profiles {
            existing_aws_names.insert(profile.name.to_lowercase());
            let key = profile.key.trim();
            let secret = profile.secret.trim();
            let region = if profile.region.trim().is_empty() {
                "us-east-1".to_string()
            } else {
                profile.region.trim().to_string()
            };
            let auth_type = if profile.auth_type.trim().is_empty() {
                "access_key".to_string()
            } else {
                profile.auth_type.trim().to_lowercase()
            };
            let credentials = if !key.is_empty() && !secret.is_empty() {
                serde_json::json!({
                    "key": key,
                    "secret": secret,
                    "region": region,
                    "auth_type": "access_key",
                })
                .to_string()
            } else if auth_type == "sso" {
                serde_json::json!({
                    "profile": profile.name,
                    "region": region,
                    "auth_type": "sso",
                })
                .to_string()
            } else {
                continue;
            };
            push_import_candidate(
                &mut candidates,
                &mut seen,
                "aws",
                profile.name,
                credentials,
                Some(region),
                if auth_type == "sso" {
                    "file:~/.aws/config (sso)".to_string()
                } else {
                    "file:~/.aws/credentials".to_string()
                },
                "aws_local",
            );
        }
    }

    if let Ok(sso_candidates) = aws_utils::list_sso_profile_candidates() {
        for candidate in sso_candidates {
            if existing_aws_names.contains(&candidate.name.to_lowercase()) {
                continue;
            }
            let region = if candidate.region.trim().is_empty() {
                "us-east-1".to_string()
            } else {
                candidate.region.trim().to_string()
            };
            let credentials = serde_json::json!({
                "profile": candidate.name,
                "region": region,
                "auth_type": "sso",
            })
            .to_string();
            push_import_candidate(
                &mut candidates,
                &mut seen,
                "aws",
                candidate.name,
                credentials,
                Some(region),
                "file:~/.aws/config (sso)".to_string(),
                "aws_local",
            );
        }
    }

    if let (Some(key), Some(secret)) = (
        read_env_trimmed(&["AWS_ACCESS_KEY_ID"]),
        read_env_trimmed(&["AWS_SECRET_ACCESS_KEY"]),
    ) {
        let region = read_env_trimmed(&["AWS_REGION", "AWS_DEFAULT_REGION"])
            .unwrap_or_else(|| "us-east-1".to_string());
        let name = read_env_trimmed(&["AWS_PROFILE", "CWS_AWS_PROFILE"])
            .unwrap_or_else(|| "aws-env".to_string());
        let credentials = serde_json::json!({
            "key": key,
            "secret": secret,
            "region": region,
            "auth_type": "access_key",
        })
        .to_string();
        push_import_candidate(
            &mut candidates,
            &mut seen,
            "aws",
            name,
            credentials,
            Some(region),
            "env:AWS_ACCESS_KEY_ID".to_string(),
            "aws_local",
        );
    }

    if let (Some(subscription_id), Some(tenant_id), Some(client_id), Some(client_secret)) = (
        read_env_trimmed(&["AZURE_SUBSCRIPTION_ID"]),
        read_env_trimmed(&["AZURE_TENANT_ID"]),
        read_env_trimmed(&["AZURE_CLIENT_ID"]),
        read_env_trimmed(&["AZURE_CLIENT_SECRET"]),
    ) {
        let name =
            read_env_trimmed(&["CWS_AZURE_PROFILE"]).unwrap_or_else(|| "azure-env".to_string());
        let credentials = serde_json::json!({
            "subscription_id": subscription_id,
            "tenant_id": tenant_id,
            "client_id": client_id,
            "client_secret": client_secret
        })
        .to_string();
        push_import_candidate(
            &mut candidates,
            &mut seen,
            "azure",
            name,
            credentials,
            None,
            "env:AZURE_*".to_string(),
            "cloud_profile",
        );
    }

    for (source, raw_json) in load_gcp_adc_json_candidates() {
        if serde_json::from_str::<serde_json::Value>(&raw_json)
            .ok()
            .filter(|v| v.is_object())
            .is_none()
        {
            continue;
        }
        let name = read_env_trimmed(&["CWS_GCP_PROFILE"]).unwrap_or_else(|| "gcp-adc".to_string());
        push_import_candidate(
            &mut candidates,
            &mut seen,
            "gcp",
            name,
            raw_json,
            None,
            source,
            "cloud_profile",
        );
    }

    if let (Some(access_key_id), Some(access_key_secret)) = (
        read_env_trimmed(&["ALIBABA_CLOUD_ACCESS_KEY_ID", "ALICLOUD_ACCESS_KEY_ID"]),
        read_env_trimmed(&[
            "ALIBABA_CLOUD_ACCESS_KEY_SECRET",
            "ALICLOUD_ACCESS_KEY_SECRET",
        ]),
    ) {
        let region = read_env_trimmed(&["ALIBABA_CLOUD_REGION_ID", "ALICLOUD_REGION_ID"])
            .unwrap_or_else(|| "cn-hangzhou".to_string());
        let name =
            read_env_trimmed(&["CWS_ALIBABA_PROFILE"]).unwrap_or_else(|| "alibaba-env".to_string());
        let credentials = serde_json::json!({
            "access_key_id": access_key_id,
            "access_key_secret": access_key_secret,
            "region_id": region
        })
        .to_string();
        push_import_candidate(
            &mut candidates,
            &mut seen,
            "alibaba",
            name,
            credentials,
            Some(region),
            "env:ALIBABA_CLOUD_*".to_string(),
            "cloud_profile",
        );
    }

    if let (Some(secret_id), Some(secret_key)) = (
        read_env_trimmed(&["TENCENTCLOUD_SECRET_ID", "TENCENT_SECRET_ID"]),
        read_env_trimmed(&["TENCENTCLOUD_SECRET_KEY", "TENCENT_SECRET_KEY"]),
    ) {
        let region = read_env_trimmed(&["TENCENTCLOUD_REGION"])
            .unwrap_or_else(|| "ap-guangzhou".to_string());
        let name =
            read_env_trimmed(&["CWS_TENCENT_PROFILE"]).unwrap_or_else(|| "tencent-env".to_string());
        let credentials = serde_json::json!({
            "secret_id": secret_id,
            "secret_key": secret_key,
            "region": region
        })
        .to_string();
        push_import_candidate(
            &mut candidates,
            &mut seen,
            "tencent",
            name,
            credentials,
            Some(region),
            "env:TENCENTCLOUD_*".to_string(),
            "cloud_profile",
        );
    }

    if let (Some(access_key), Some(secret_key)) = (
        read_env_trimmed(&["HUAWEICLOUD_ACCESS_KEY", "HWCLOUD_ACCESS_KEY"]),
        read_env_trimmed(&["HUAWEICLOUD_SECRET_KEY", "HWCLOUD_SECRET_KEY"]),
    ) {
        let region = read_env_trimmed(&["HUAWEICLOUD_REGION", "HWCLOUD_REGION"])
            .unwrap_or_else(|| "cn-north-4".to_string());
        let project_id =
            read_env_trimmed(&["HUAWEICLOUD_PROJECT_ID", "HWCLOUD_PROJECT_ID"]).unwrap_or_default();
        let name =
            read_env_trimmed(&["CWS_HUAWEI_PROFILE"]).unwrap_or_else(|| "huawei-env".to_string());
        let credentials = serde_json::json!({
            "access_key": access_key,
            "secret_key": secret_key,
            "region": region,
            "project_id": project_id
        })
        .to_string();
        push_import_candidate(
            &mut candidates,
            &mut seen,
            "huawei",
            name,
            credentials,
            Some(region),
            "env:HUAWEICLOUD_*".to_string(),
            "cloud_profile",
        );
    }

    if let Some(token) = read_env_trimmed(&["DIGITALOCEAN_TOKEN"]) {
        let name =
            read_env_trimmed(&["CWS_DIGITALOCEAN_PROFILE"]).unwrap_or_else(|| "do-env".to_string());
        push_import_candidate(
            &mut candidates,
            &mut seen,
            "digitalocean",
            name,
            token,
            None,
            "env:DIGITALOCEAN_TOKEN".to_string(),
            "cloud_profile",
        );
    }

    if let (Some(token), Some(account_id)) = (
        read_env_trimmed(&["CLOUDFLARE_API_TOKEN"]),
        read_env_trimmed(&["CLOUDFLARE_ACCOUNT_ID"]),
    ) {
        let name = read_env_trimmed(&["CWS_CLOUDFLARE_PROFILE"])
            .unwrap_or_else(|| "cloudflare-env".to_string());
        let credentials = serde_json::json!({
            "token": token,
            "account_id": account_id,
        })
        .to_string();
        push_import_candidate(
            &mut candidates,
            &mut seen,
            "cloudflare",
            name,
            credentials,
            None,
            "env:CLOUDFLARE_*".to_string(),
            "cloud_profile",
        );
    }

    Ok(candidates)
}

#[tauri::command]
fn save_aws_profile(
    name: String,
    key: String,
    secret: String,
    region: Option<String>,
) -> Result<(), String> {
    aws_utils::save_profile(&name, &key, &secret, region)
}

#[tauri::command]
fn save_aws_profile_reference(name: String, region: Option<String>) -> Result<(), String> {
    aws_utils::save_profile_reference(&name, region)
}

#[tauri::command]
fn delete_aws_profile(name: String) -> Result<(), String> {
    aws_utils::delete_profile(&name)
}

#[tauri::command]
async fn save_cloud_profile(
    app_handle: tauri::AppHandle,
    provider: String,
    name: String,
    credentials: String,
    timeout: Option<i64>,
    policy: Option<String>,
    proxy_profile_id: Option<String>,
) -> Result<String, String> {
    if provider.trim().eq_ignore_ascii_case("aws") {
        return Err(
            "AWS accounts must be saved as local AWS profiles. Use the AWS-specific add/import flow."
                .to_string(),
        );
    }
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    let profile_id = db::save_cloud_profile(
        &conn,
        &provider,
        &name,
        &credentials,
        timeout,
        policy,
        proxy_profile_id,
    )
    .await?;
    {
        let _ = db::record_audit_log(
            &conn,
            "CONNECT",
            &provider,
            &format!("Added account: {}", name),
        )
        .await;
    }
    Ok(profile_id)
}

#[tauri::command]
async fn update_cloud_profile(
    app_handle: tauri::AppHandle,
    id: String,
    provider: String,
    name: String,
    credentials: String,
    timeout: Option<i64>,
    policy: Option<String>,
    proxy_profile_id: Option<String>,
) -> Result<(), String> {
    if provider.trim().eq_ignore_ascii_case("aws") {
        return Err(
            "AWS accounts must be saved as local AWS profiles. Use the AWS-specific add/import flow."
                .to_string(),
        );
    }
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    let res = db::update_cloud_profile(
        &conn,
        &id,
        &provider,
        &name,
        &credentials,
        timeout,
        policy,
        proxy_profile_id,
    )
    .await;
    if res.is_ok() {
        let _ = db::record_audit_log(
            &conn,
            "UPDATE",
            &provider,
            &format!("Updated account: {}", name),
        )
        .await;
    }
    res
}

#[tauri::command]
async fn list_cloud_profiles(
    app_handle: tauri::AppHandle,
) -> Result<Vec<db::CloudProfile>, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::list_cloud_profiles(&conn).await
}

#[tauri::command]
async fn list_proxy_profiles(
    app_handle: tauri::AppHandle,
) -> Result<Vec<db::ProxyProfile>, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::list_proxy_profiles(&conn).await
}

#[tauri::command]
async fn save_proxy_profile(
    app_handle: tauri::AppHandle,
    id: Option<String>,
    name: String,
    protocol: String,
    host: String,
    port: i64,
    auth_username: Option<String>,
    auth_password: Option<String>,
) -> Result<String, String> {
    let normalized_protocol = protocol.trim().to_lowercase();
    if !["socks5h", "socks5", "http", "https"].contains(&normalized_protocol.as_str()) {
        return Err("Unsupported proxy protocol.".to_string());
    }
    let normalized_host = host.trim().to_string();
    if normalized_host.is_empty() {
        return Err("Proxy host cannot be empty.".to_string());
    }
    if !(1..=65535).contains(&port) {
        return Err("Proxy port must be between 1 and 65535.".to_string());
    }
    let normalized_auth_username = auth_username
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let normalized_auth_password = auth_password
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if normalized_auth_password.is_some() && normalized_auth_username.is_none() {
        return Err("Proxy username is required when password is provided.".to_string());
    }

    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    let profile_id = db::save_proxy_profile(
        &conn,
        id,
        name.trim(),
        &normalized_protocol,
        &normalized_host,
        port,
        normalized_auth_username.as_deref(),
        normalized_auth_password.as_deref(),
    )
    .await?;
    Ok(profile_id)
}

#[tauri::command]
async fn delete_proxy_profile(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::delete_proxy_profile(&conn, &id).await
}

#[tauri::command]
async fn delete_cloud_profile(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    match db::delete_cloud_profile(&conn, &id).await {
        Ok(name) => {
            let _ = db::record_audit_log(
                &conn,
                "DISCONNECT",
                "System",
                &format!("Removed account: {}", name),
            )
            .await;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
async fn get_scan_results(app_handle: tauri::AppHandle) -> Result<Vec<WastedResource>, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::get_scan_results(&conn).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_enriched_scan_results(
    app_handle: tauri::AppHandle,
) -> Result<Vec<EnrichedScanResult>, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    current_enriched_scan_results(&conn).await
}

#[tauri::command]
async fn replace_scan_results(
    app_handle: tauri::AppHandle,
    resources: Vec<WastedResource>,
) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::save_scan_results(&conn, &resources)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_scan_results(app_handle: tauri::AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::clear_scan_results(&conn)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_audit_logs(
    app_handle: tauri::AppHandle,
    date_from: Option<i64>,
    date_to: Option<i64>,
    page: i64,
) -> Result<Vec<db::AuditLog>, String> {
    let app_state = app_handle.state::<AppState>();
    let runtime_plan = read_runtime_plan_type(&app_state.db_path).await;
    if !audit_log_entitled_for_runtime_plan(runtime_plan.as_deref()) {
        return Err("Audit Log is available on Enterprise edition.".to_string());
    }

    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;

    let limit = 50;
    let offset = (page - 1) * limit;

    db::get_audit_logs(&conn, date_from, date_to, limit, offset)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_audit_logs(app_handle: tauri::AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let runtime_plan = read_runtime_plan_type(&app_state.db_path).await;
    if !audit_log_entitled_for_runtime_plan(runtime_plan.as_deref()) {
        return Err("Audit Log is available on Enterprise edition.".to_string());
    }

    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::clear_audit_logs(&conn).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_dashboard_stats(
    app_handle: tauri::AppHandle,
    demo_mode: bool,
) -> Result<db::Stats, String> {
    if demo_mode {
        return Ok(demo_data::generate_demo_stats());
    }
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::get_stats(&conn).await.map_err(|e| e.to_string())
}

fn compute_governance_stats(
    history: Vec<db::ScanHistoryItem>,
    window_days: Option<i64>,
) -> GovernanceStatsResponse {
    let now_ts = now_unix_ts();
    let days = normalize_governance_window_days(window_days);
    let day_seconds = 86_400i64;
    let window_end_ts = now_ts.div_euclid(day_seconds) * day_seconds;
    let window_start_ts = window_end_ts - ((days - 1) * day_seconds);

    let mut daily_map: BTreeMap<i64, GovernanceDailyAgg> = BTreeMap::new();
    for offset in 0..days {
        let day_ts = window_start_ts + offset * day_seconds;
        daily_map.insert(day_ts, GovernanceDailyAgg::default());
    }

    let mut total_scan_runs: usize = 0;
    let mut total_findings: usize = 0;
    let mut positive_scan_runs: usize = 0;
    let mut identified_savings: f64 = 0.0;
    let mut scan_checks_attempted_total: i64 = 0;
    let mut scan_checks_succeeded_total: i64 = 0;
    let mut scan_checks_failed_total: i64 = 0;
    let mut last_scan_at: Option<i64> = None;

    let mut provider_acc: HashMap<String, GovernanceProviderAgg> = HashMap::new();
    let mut account_acc: HashMap<String, usize> = HashMap::new();
    let mut error_bucket_totals: HashMap<String, i64> = HashMap::new();
    for category in governance_error_category_catalog() {
        error_bucket_totals.insert((*category).to_string(), 0);
    }

    for item in history {
        let day_ts = item.scanned_at.div_euclid(day_seconds) * day_seconds;
        if day_ts < window_start_ts || day_ts > window_end_ts {
            continue;
        }

        let resources: Vec<WastedResource> =
            serde_json::from_str(&item.results_json).unwrap_or_default();
        let findings = resources.len();
        let mut scan_savings = 0.0f64;
        let mut provider_findings: HashMap<String, usize> = HashMap::new();
        let mut provider_savings: HashMap<String, f64> = HashMap::new();

        for resource in &resources {
            let provider = normalize_governance_provider(&resource.provider);
            let cost = if resource.estimated_monthly_cost.is_finite() {
                resource.estimated_monthly_cost.max(0.0)
            } else {
                0.0
            };
            scan_savings += cost;
            *provider_findings.entry(provider.clone()).or_insert(0) += 1;
            *provider_savings.entry(provider).or_insert(0.0) += cost;
        }

        let mut scanned_accounts: HashSet<String> = HashSet::new();
        let mut checks_attempted = 0i64;
        let mut checks_succeeded = 0i64;
        let mut checks_failed = 0i64;
        let mut scan_error_buckets: HashMap<String, i64> = HashMap::new();
        if let Some(meta_raw) = item.scan_meta.as_deref() {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(meta_raw) {
                checks_attempted = parse_meta_i64(meta.get("scan_checks_attempted"));
                checks_succeeded = parse_meta_i64(meta.get("scan_checks_succeeded"));
                checks_failed = parse_meta_i64(meta.get("scan_checks_failed"));
                scan_error_buckets = parse_governance_error_bucket_counts(&meta);
                if let Some(accounts) = meta
                    .get("scanned_accounts")
                    .and_then(|value| value.as_array())
                {
                    for account in accounts {
                        if let Some(raw_name) = account.as_str() {
                            let normalized = normalize_governance_account_label(raw_name);
                            if !normalized.is_empty() {
                                scanned_accounts.insert(normalized);
                            }
                        }
                    }
                }
            }
        }

        let checks_failed_non_negative = checks_failed.max(0);
        let classified_failures: i64 = scan_error_buckets.values().copied().sum();
        if checks_failed_non_negative > classified_failures {
            *scan_error_buckets.entry("unknown".to_string()).or_insert(0) +=
                checks_failed_non_negative - classified_failures;
        }
        for (category, count) in scan_error_buckets {
            let normalized = normalize_governance_error_category_key(&category);
            *error_bucket_totals.entry(normalized).or_insert(0) += count.max(0);
        }

        total_scan_runs += 1;
        total_findings += findings;
        identified_savings += scan_savings;
        if scan_savings > 0.0 {
            positive_scan_runs += 1;
        }
        scan_checks_attempted_total += checks_attempted.max(0);
        scan_checks_succeeded_total += checks_succeeded.max(0);
        scan_checks_failed_total += checks_failed_non_negative;
        last_scan_at = Some(last_scan_at.map_or(item.scanned_at, |prev| prev.max(item.scanned_at)));

        for account in scanned_accounts {
            *account_acc.entry(account).or_insert(0) += 1;
        }

        for (provider, provider_findings_count) in provider_findings {
            let provider_savings_value = provider_savings.get(&provider).copied().unwrap_or(0.0);
            let entry = provider_acc.entry(provider).or_default();
            entry.scan_runs += 1;
            entry.findings += provider_findings_count;
            entry.savings += provider_savings_value;
            if provider_savings_value > 0.0 {
                entry.positive_scan_runs += 1;
            }
        }

        if let Some(day_entry) = daily_map.get_mut(&day_ts) {
            day_entry.scan_runs += 1;
            day_entry.findings += findings;
            day_entry.savings += scan_savings;
            if scan_savings > 0.0 {
                day_entry.positive_scan_runs += 1;
            }
            day_entry.scan_checks_attempted += checks_attempted.max(0);
            day_entry.scan_checks_succeeded += checks_succeeded.max(0);
            day_entry.scan_checks_failed += checks_failed_non_negative;
        }
    }

    let positive_scan_rate_pct = if total_scan_runs > 0 {
        round_one((positive_scan_runs as f64 / total_scan_runs as f64) * 100.0)
    } else {
        0.0
    };
    let scan_check_success_rate_pct = if scan_checks_attempted_total > 0 {
        round_one((scan_checks_succeeded_total as f64 / scan_checks_attempted_total as f64) * 100.0)
    } else {
        0.0
    };

    let avg_savings_per_scan = if total_scan_runs > 0 {
        round_two(identified_savings / total_scan_runs as f64)
    } else {
        0.0
    };
    let avg_findings_per_scan = if total_scan_runs > 0 {
        round_two(total_findings as f64 / total_scan_runs as f64)
    } else {
        0.0
    };
    let estimated_co2e_kg_monthly = round_two(identified_savings.max(0.0) * 0.42);

    let mut providers: Vec<GovernanceProviderRow> = provider_acc
        .into_iter()
        .map(|(provider, acc)| GovernanceProviderRow {
            provider,
            scan_runs: acc.scan_runs,
            findings: acc.findings,
            savings: round_two(acc.savings),
            estimated_co2e_kg_monthly: round_two(acc.savings.max(0.0) * 0.42),
            positive_scan_runs: acc.positive_scan_runs,
        })
        .collect();
    providers.sort_by(|a, b| {
        b.savings
            .partial_cmp(&a.savings)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.findings.cmp(&a.findings))
            .then_with(|| a.provider.cmp(&b.provider))
    });

    let mut accounts: Vec<GovernanceAccountRow> = account_acc
        .into_iter()
        .map(|(account, scan_runs)| GovernanceAccountRow {
            account,
            scan_runs,
            coverage_pct: if total_scan_runs > 0 {
                round_one((scan_runs as f64 / total_scan_runs as f64) * 100.0)
            } else {
                0.0
            },
        })
        .collect();
    accounts.sort_by(|a, b| {
        b.scan_runs
            .cmp(&a.scan_runs)
            .then_with(|| a.account.cmp(&b.account))
    });

    let daily = daily_map
        .into_iter()
        .map(|(day_ts, agg)| {
            let (day_label, day_date) = if let Some(dt) = Utc.timestamp_opt(day_ts, 0).single() {
                (
                    dt.format("%m-%d").to_string(),
                    dt.format("%Y-%m-%d").to_string(),
                )
            } else {
                (day_ts.to_string(), day_ts.to_string())
            };
            GovernanceDailyPoint {
                day_ts,
                day_label,
                day_date,
                scan_runs: agg.scan_runs,
                positive_scan_runs: agg.positive_scan_runs,
                findings: agg.findings,
                savings: round_two(agg.savings),
                estimated_co2e_kg_monthly: round_two(agg.savings.max(0.0) * 0.42),
                scan_checks_attempted: agg.scan_checks_attempted,
                scan_checks_succeeded: agg.scan_checks_succeeded,
                scan_checks_failed: agg.scan_checks_failed,
                check_success_rate_pct: if agg.scan_checks_attempted > 0 {
                    round_one(
                        (agg.scan_checks_succeeded as f64 / agg.scan_checks_attempted as f64)
                            * 100.0,
                    )
                } else {
                    0.0
                },
            }
        })
        .collect::<Vec<_>>();

    let total_failed_checks = scan_checks_failed_total.max(0);
    let categories = governance_error_category_catalog()
        .iter()
        .map(|category| {
            let key = (*category).to_string();
            let count = error_bucket_totals.get(&key).copied().unwrap_or(0).max(0);
            GovernanceErrorCategoryRow {
                category: key.clone(),
                label: governance_error_category_label(&key).to_string(),
                count,
                ratio_pct: if total_failed_checks > 0 {
                    round_one((count as f64 / total_failed_checks as f64) * 100.0)
                } else {
                    0.0
                },
            }
        })
        .collect::<Vec<_>>();

    GovernanceStatsResponse {
        generated_at: now_ts,
        window_days: days,
        window_start_ts,
        window_end_ts,
        scorecard: GovernanceScorecard {
            scan_runs: total_scan_runs,
            findings: total_findings,
            positive_scan_runs,
            positive_scan_rate_pct,
            identified_savings: round_two(identified_savings),
            estimated_co2e_kg_monthly,
            avg_savings_per_scan,
            avg_findings_per_scan,
            active_accounts: accounts.len(),
            active_providers: providers.len(),
            scan_checks_attempted: scan_checks_attempted_total,
            scan_checks_succeeded: scan_checks_succeeded_total,
            scan_checks_failed: total_failed_checks,
            scan_check_success_rate_pct,
            last_scan_at,
        },
        daily,
        providers,
        accounts,
        error_taxonomy: GovernanceErrorTaxonomy {
            taxonomy_version: "v1".to_string(),
            total_failed_checks,
            categories,
        },
    }
}

#[tauri::command]
async fn get_governance_stats(
    app_handle: tauri::AppHandle,
    window_days: Option<i64>,
    demo_mode: Option<bool>,
) -> Result<GovernanceStatsResponse, String> {
    if demo_mode.unwrap_or(false) {
        let demo_history = generate_demo_governance_history(window_days);
        return Ok(compute_governance_stats(demo_history, window_days));
    }

    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    let history = db::get_scan_history(&conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(compute_governance_stats(history, window_days))
}

fn parse_update_filename(url: &str) -> String {
    let raw_name = url
        .split('/')
        .last()
        .unwrap_or("update.exe")
        .split('?')
        .next()
        .unwrap_or("update.exe");

    if raw_name.trim().is_empty() {
        "update.exe".to_string()
    } else {
        raw_name.to_string()
    }
}

fn normalize_ai_bucket_key(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn extract_scan_meta_accounts(scan_meta_raw: Option<&str>) -> Vec<String> {
    let mut accounts = Vec::new();
    if let Some(raw) = scan_meta_raw {
        if let Ok(meta) = serde_json::from_str::<serde_json::Value>(raw) {
            if let Some(items) = meta
                .get("scanned_accounts")
                .and_then(|value| value.as_array())
            {
                for item in items {
                    if let Some(account) = item.as_str() {
                        let trimmed = account.trim();
                        if !trimmed.is_empty()
                            && !accounts.iter().any(|existing| existing == trimmed)
                        {
                            accounts.push(trimmed.to_string());
                        }
                    }
                }
            }
        }
    }
    accounts
}

fn resource_attribution_key(resource: &WastedResource) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        resource.provider.trim(),
        resource.region.trim(),
        resource.resource_type.trim(),
        resource.id.trim(),
        resource.action_type.trim()
    )
}

fn attribute_results_for_account(
    results: &[WastedResource],
    start_index: usize,
    account_id: &str,
    account_name: &str,
    attribution: &mut HashMap<String, ScanFindingAttribution>,
) {
    for resource in results.iter().skip(start_index) {
        attribution.insert(
            resource_attribution_key(resource),
            ScanFindingAttribution {
                account_id: account_id.to_string(),
                account_name: account_name.to_string(),
            },
        );
    }
}

fn extract_scan_meta_attribution(
    scan_meta_raw: Option<&str>,
) -> HashMap<String, ScanFindingAttribution> {
    let mut attribution = HashMap::new();
    if let Some(raw) = scan_meta_raw {
        if let Ok(meta) = serde_json::from_str::<serde_json::Value>(raw) {
            if let Some(obj) = meta
                .get("resource_attribution")
                .and_then(|value| value.as_object())
            {
                for (key, value) in obj {
                    let account_id = value
                        .get("account_id")
                        .and_then(|field| field.as_str())
                        .unwrap_or("")
                        .trim();
                    let account_name = value
                        .get("account_name")
                        .and_then(|field| field.as_str())
                        .unwrap_or("")
                        .trim();
                    if !account_id.is_empty() || !account_name.is_empty() {
                        attribution.insert(
                            key.clone(),
                            ScanFindingAttribution {
                                account_id: account_id.to_string(),
                                account_name: account_name.to_string(),
                            },
                        );
                    }
                }
            }
        }
    }
    attribution
}

fn build_ai_breakdown_rows(
    groups: HashMap<String, (String, f64, i64)>,
    previous_groups: Option<&HashMap<String, (String, f64, i64)>>,
    total_monthly_waste: f64,
) -> Vec<AiAnalystBreakdownRow> {
    let mut rows: Vec<AiAnalystBreakdownRow> = groups
        .into_iter()
        .map(|(key, (label, estimated_monthly_waste, findings))| {
            let previous = previous_groups.and_then(|map| map.get(&key));
            AiAnalystBreakdownRow {
                key,
                label,
                estimated_monthly_waste,
                findings,
                share_pct: if total_monthly_waste > 0.0 {
                    (estimated_monthly_waste / total_monthly_waste) * 100.0
                } else {
                    0.0
                },
                delta_monthly_waste: previous.map(|item| estimated_monthly_waste - item.1),
                delta_findings: previous.map(|item| findings - item.2),
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        b.estimated_monthly_waste
            .partial_cmp(&a.estimated_monthly_waste)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.findings.cmp(&a.findings))
            .then_with(|| a.label.cmp(&b.label))
    });
    rows
}

fn compute_ai_analyst_summary(
    window_days: i64,
    basis: &str,
    latest_scan_id: Option<i64>,
    latest_scan_at: Option<i64>,
    previous_scan_id: Option<i64>,
    previous_scan_at: Option<i64>,
    previous_resources: Option<&[WastedResource]>,
    scan_count_in_window: usize,
    scanned_accounts: Vec<String>,
    resources: Vec<WastedResource>,
    result_attribution: HashMap<String, ScanFindingAttribution>,
    previous_result_attribution: Option<HashMap<String, ScanFindingAttribution>>,
    mut notes: Vec<String>,
) -> AiAnalystSummary {
    let total_monthly_waste = resources
        .iter()
        .map(|resource| resource.estimated_monthly_cost.max(0.0))
        .sum::<f64>();
    let total_findings = resources.len() as i64;
    let previous_total_monthly_waste = previous_resources.map(|rows| {
        rows.iter()
            .map(|resource| resource.estimated_monthly_cost.max(0.0))
            .sum::<f64>()
    });
    let previous_total_findings = previous_resources.map(|rows| rows.len() as i64);

    let mut accounts: HashMap<String, (String, f64, i64)> = HashMap::new();
    let mut previous_accounts: HashMap<String, (String, f64, i64)> = HashMap::new();
    let mut providers: HashMap<String, (String, f64, i64)> = HashMap::new();
    let mut previous_providers: HashMap<String, (String, f64, i64)> = HashMap::new();
    let mut resource_types: HashMap<String, (String, f64, i64)> = HashMap::new();
    let mut previous_resource_types: HashMap<String, (String, f64, i64)> = HashMap::new();

    if let Some(previous_rows) = previous_resources {
        for resource in previous_rows {
            let provider_label = resource.provider.trim();
            if !provider_label.is_empty() {
                let key = normalize_ai_bucket_key(provider_label);
                let entry = previous_providers
                    .entry(if key.is_empty() {
                        "unknown".to_string()
                    } else {
                        key
                    })
                    .or_insert_with(|| (provider_label.to_string(), 0.0, 0));
                entry.1 += resource.estimated_monthly_cost.max(0.0);
                entry.2 += 1;
            }

            let resource_type_label = resource.resource_type.trim();
            if !resource_type_label.is_empty() {
                let key = normalize_ai_bucket_key(resource_type_label);
                let entry = previous_resource_types
                    .entry(if key.is_empty() {
                        "unknown".to_string()
                    } else {
                        key
                    })
                    .or_insert_with(|| (resource_type_label.to_string(), 0.0, 0));
                entry.1 += resource.estimated_monthly_cost.max(0.0);
                entry.2 += 1;
            }
        }
    }

    for resource in resources {
        if let Some(attribution) = result_attribution.get(&resource_attribution_key(&resource)) {
            let account_label = if !attribution.account_name.trim().is_empty() {
                attribution.account_name.trim()
            } else {
                attribution.account_id.trim()
            };
            if !account_label.is_empty() {
                let key = if !attribution.account_id.trim().is_empty() {
                    attribution.account_id.trim().to_string()
                } else {
                    normalize_ai_bucket_key(account_label)
                };
                let entry = accounts
                    .entry(if key.is_empty() {
                        "unknown".to_string()
                    } else {
                        key
                    })
                    .or_insert_with(|| (account_label.to_string(), 0.0, 0));
                entry.1 += resource.estimated_monthly_cost.max(0.0);
                entry.2 += 1;
            }
        }

        let provider_label = resource.provider.trim();
        if !provider_label.is_empty() {
            let key = normalize_ai_bucket_key(provider_label);
            let entry = providers
                .entry(if key.is_empty() {
                    "unknown".to_string()
                } else {
                    key
                })
                .or_insert_with(|| (provider_label.to_string(), 0.0, 0));
            entry.1 += resource.estimated_monthly_cost.max(0.0);
            entry.2 += 1;
        }

        let resource_type_label = resource.resource_type.trim();
        if !resource_type_label.is_empty() {
            let key = normalize_ai_bucket_key(resource_type_label);
            let entry = resource_types
                .entry(if key.is_empty() {
                    "unknown".to_string()
                } else {
                    key
                })
                .or_insert_with(|| (resource_type_label.to_string(), 0.0, 0));
            entry.1 += resource.estimated_monthly_cost.max(0.0);
            entry.2 += 1;
        }
    }

    if let (Some(previous_rows), Some(previous_attribution)) =
        (previous_resources, previous_result_attribution.as_ref())
    {
        for resource in previous_rows {
            if let Some(attribution) = previous_attribution.get(&resource_attribution_key(resource))
            {
                let account_label = if !attribution.account_name.trim().is_empty() {
                    attribution.account_name.trim()
                } else {
                    attribution.account_id.trim()
                };
                if !account_label.is_empty() {
                    let key = if !attribution.account_id.trim().is_empty() {
                        attribution.account_id.trim().to_string()
                    } else {
                        normalize_ai_bucket_key(account_label)
                    };
                    let entry = previous_accounts
                        .entry(if key.is_empty() {
                            "unknown".to_string()
                        } else {
                            key
                        })
                        .or_insert_with(|| (account_label.to_string(), 0.0, 0));
                    entry.1 += resource.estimated_monthly_cost.max(0.0);
                    entry.2 += 1;
                }
            }
        }
    }

    if total_findings == 0 {
        notes.push("No findings are available in the selected analyst window yet.".to_string());
    }

    AiAnalystSummary {
        window_days,
        basis: basis.to_string(),
        latest_scan_id,
        latest_scan_at,
        previous_scan_id,
        previous_scan_at,
        previous_total_monthly_waste,
        previous_total_findings,
        delta_monthly_waste: previous_total_monthly_waste
            .map(|previous| total_monthly_waste - previous),
        delta_findings: previous_total_findings.map(|previous| total_findings - previous),
        scan_count_in_window,
        total_monthly_waste,
        total_findings,
        scanned_accounts,
        accounts: build_ai_breakdown_rows(
            accounts,
            if previous_accounts.is_empty() {
                None
            } else {
                Some(&previous_accounts)
            },
            total_monthly_waste,
        ),
        providers: build_ai_breakdown_rows(
            providers,
            if previous_providers.is_empty() {
                None
            } else {
                Some(&previous_providers)
            },
            total_monthly_waste,
        ),
        resource_types: build_ai_breakdown_rows(
            resource_types,
            if previous_resource_types.is_empty() {
                None
            } else {
                Some(&previous_resource_types)
            },
            total_monthly_waste,
        ),
        notes,
    }
}

fn latest_ai_source_for_window(
    history: Vec<db::ScanHistoryItem>,
    window_days: i64,
) -> (
    String,
    Option<i64>,
    Option<i64>,
    Vec<WastedResource>,
    HashMap<String, ScanFindingAttribution>,
    Vec<String>,
) {
    let since_ts = Utc::now().timestamp() - (window_days * 86_400);
    let scans_in_window: Vec<db::ScanHistoryItem> = history
        .into_iter()
        .filter(|item| item.status.eq_ignore_ascii_case("completed") && item.scanned_at >= since_ts)
        .collect();

    if let Some(latest_scan) = scans_in_window.first() {
        let mut notes = Vec::new();
        let resources = match serde_json::from_str::<Vec<WastedResource>>(&latest_scan.results_json)
        {
            Ok(parsed) => parsed,
            Err(err) => {
                notes.push(format!(
                    "Stored scan payload could not be fully parsed: {}",
                    err
                ));
                Vec::new()
            }
        };
        let attribution = extract_scan_meta_attribution(latest_scan.scan_meta.as_deref());
        return (
            "latest_scan_in_window".to_string(),
            Some(latest_scan.id),
            Some(latest_scan.scanned_at),
            resources,
            attribution,
            notes,
        );
    }

    (
        "empty".to_string(),
        None,
        None,
        Vec::new(),
        HashMap::new(),
        Vec::new(),
    )
}

async fn current_enriched_scan_results(
    conn: &Pool<Sqlite>,
) -> Result<Vec<EnrichedScanResult>, String> {
    let current_results = db::get_scan_results(conn)
        .await
        .map_err(|e| e.to_string())?;
    let history = db::get_scan_history(conn)
        .await
        .map_err(|e| e.to_string())?;
    let latest_meta = history
        .iter()
        .find(|item| item.status.eq_ignore_ascii_case("completed"))
        .and_then(|item| item.scan_meta.as_deref());
    let attribution = extract_scan_meta_attribution(latest_meta);

    Ok(current_results
        .into_iter()
        .map(|resource| {
            let mapped = attribution.get(&resource_attribution_key(&resource));
            EnrichedScanResult {
                id: resource.id,
                provider: resource.provider,
                region: resource.region,
                resource_type: resource.resource_type,
                details: resource.details,
                estimated_monthly_cost: resource.estimated_monthly_cost,
                action_type: resource.action_type,
                account_id: mapped
                    .map(|value| value.account_id.clone())
                    .filter(|value| !value.is_empty()),
                account_name: mapped
                    .map(|value| value.account_name.clone())
                    .filter(|value| !value.is_empty()),
            }
        })
        .collect())
}

fn make_update_file_path(url: &str) -> Result<std::path::PathBuf, String> {
    let download_dir = dirs::download_dir().ok_or("Failed to get Downloads dir")?;
    let mut file_name = parse_update_filename(url);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if let Some(dot) = file_name.rfind('.') {
        let (stem, ext) = file_name.split_at(dot);
        file_name = format!("{}-{}{}", stem, ts, ext);
    } else {
        file_name = format!("{}-{}", file_name, ts);
    }

    Ok(download_dir.join(file_name))
}

fn normalize_update_candidates(
    primary_url: String,
    candidate_urls: Option<Vec<String>>,
) -> Vec<String> {
    let mut urls = Vec::new();
    let primary_trimmed = primary_url.trim();
    if !primary_trimmed.is_empty() {
        urls.push(primary_trimmed.to_string());
    }

    for candidate in candidate_urls.unwrap_or_default() {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !urls.iter().any(|item| item == trimmed) {
            urls.push(trimmed.to_string());
        }
    }

    urls
}

fn is_update_cancel_requested() -> bool {
    UPDATE_DOWNLOAD_CANCELED.load(Ordering::SeqCst)
}

fn make_update_canceled_error() -> String {
    UPDATE_CANCEL_REASON.to_string()
}

fn is_update_canceled_error(err: &str) -> bool {
    err.contains(UPDATE_CANCEL_REASON)
}

fn compute_update_progress(downloaded: u64, total_size: u64) -> f64 {
    if total_size > 0 {
        ((downloaded as f64 / total_size as f64) * 100.0).clamp(0.0, 100.0)
    } else if downloaded == 0 {
        0.0
    } else {
        let ratio = downloaded.min(UPDATE_UNKNOWN_TOTAL_FALLBACK_BYTES) as f64
            / UPDATE_UNKNOWN_TOTAL_FALLBACK_BYTES as f64;
        (ratio * 95.0).clamp(1.0, 95.0)
    }
}

fn emit_update_progress_detail(
    app_handle: &tauri::AppHandle,
    stage: &str,
    progress: f64,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    url: Option<&str>,
    message: &str,
) {
    let payload = serde_json::json!({
        "stage": stage,
        "progress": progress,
        "downloaded_bytes": downloaded_bytes,
        "total_bytes": total_bytes,
        "url": url.unwrap_or(""),
        "message": message,
    });
    let _ = app_handle.emit("update-progress-detail", payload);
}

async fn probe_update_candidate(client: &reqwest::Client, url: &str) -> Result<f64, String> {
    let started = std::time::Instant::now();
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(UPDATE_PROBE_TIMEOUT_SECS),
        client
            .get(url)
            .header(reqwest::header::RANGE, UPDATE_PROBE_RANGE)
            .send(),
    )
    .await
    .map_err(|_| "probe timeout while connecting".to_string())?
    .map_err(|e| format!("probe request failed: {}", e))?;

    if !response.status().is_success() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
    {
        return Err(format!("probe returned HTTP {}", response.status()));
    }

    let mut stream = response.bytes_stream();
    let mut sampled: usize = 0;
    let probe_deadline = started + std::time::Duration::from_secs(UPDATE_PROBE_TOTAL_TIMEOUT_SECS);
    while sampled < UPDATE_PROBE_TARGET_SAMPLE_BYTES {
        let now = std::time::Instant::now();
        if now >= probe_deadline {
            break;
        }
        let remaining = probe_deadline.saturating_duration_since(now);
        let wait_for = std::cmp::min(
            std::time::Duration::from_secs(UPDATE_PROBE_TIMEOUT_SECS),
            remaining,
        );
        if wait_for.is_zero() {
            break;
        }
        let next_chunk = tokio::time::timeout(wait_for, stream.next())
            .await
            .map_err(|_| "probe timeout while reading".to_string())?;

        match next_chunk {
            Some(Ok(chunk)) => {
                sampled += chunk.len();
            }
            Some(Err(err)) => {
                return Err(format!("probe stream read failed: {}", err));
            }
            None => break,
        }
    }

    let bytes_len = sampled as f64;
    if bytes_len <= 0.0 {
        return Err("probe returned empty body".to_string());
    }

    let elapsed = started.elapsed().as_secs_f64();
    if elapsed <= f64::EPSILON {
        return Ok(bytes_len);
    }
    Ok(bytes_len / elapsed)
}

async fn select_update_download_order(
    client: &reqwest::Client,
    candidates: &[String],
) -> Vec<String> {
    if candidates.len() <= 1 {
        return candidates.to_vec();
    }

    let mut probe_ok: Vec<(String, f64)> = Vec::new();
    let mut probe_failed: Vec<String> = Vec::new();

    for url in candidates {
        match probe_update_candidate(client, url).await {
            Ok(speed_bps) => {
                println!("Update probe success: {} speed={:.0}B/s", url, speed_bps);
                probe_ok.push((url.clone(), speed_bps));
            }
            Err(err) => {
                println!("Update probe failed: {} error={}", url, err);
                probe_failed.push(url.clone());
            }
        }
    }

    probe_ok.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut ordered: Vec<String> = probe_ok.into_iter().map(|(url, _)| url).collect();
    for url in probe_failed {
        if !ordered.iter().any(|item| item == &url) {
            ordered.push(url);
        }
    }

    if ordered.is_empty() {
        candidates.to_vec()
    } else {
        ordered
    }
}

async fn download_update_from_url(
    app_handle: &tauri::AppHandle,
    client: &reqwest::Client,
    url: &str,
    allow_slow_failover: bool,
) -> Result<std::path::PathBuf, String> {
    log_startup_event(&format!("update download start: endpoint={}", url));
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        log_startup_event(&format!(
            "update download http failure: endpoint={} status={}",
            url,
            response.status()
        ));
        return Err(format!("Download failed with HTTP {}", response.status()));
    }

    let mut total_size = response.content_length().unwrap_or(0);
    if total_size == 0 {
        if let Ok(head_response) = client.head(url).send().await {
            total_size = head_response.content_length().unwrap_or(0);
        }
    }

    emit_update_progress_detail(
        app_handle,
        "downloading_start",
        0.0,
        0,
        (total_size > 0).then_some(total_size),
        Some(url),
        if total_size > 0 {
            "Downloading installer..."
        } else {
            "Downloading installer (total size not provided by server)..."
        },
    );

    let mut stream = response.bytes_stream();
    let file_path = make_update_file_path(url)?;
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut last_chunk_at = std::time::Instant::now();
    let poll_timeout = std::time::Duration::from_secs(UPDATE_STREAM_POLL_TIMEOUT_SECS);

    loop {
        if is_update_cancel_requested() {
            let _ = std::fs::remove_file(&file_path);
            emit_update_progress_detail(
                app_handle,
                "canceled",
                compute_update_progress(downloaded, total_size),
                downloaded,
                (total_size > 0).then_some(total_size),
                Some(url),
                "Download canceled.",
            );
            return Err(make_update_canceled_error());
        }

        let next_item = match tokio::time::timeout(poll_timeout, stream.next()).await {
            Ok(item) => item,
            Err(_) => {
                let stalled_for_secs = last_chunk_at.elapsed().as_secs();
                if stalled_for_secs < UPDATE_STREAM_STALL_FAILOVER_SECS {
                    continue;
                }

                let progress = compute_update_progress(downloaded, total_size);
                let should_failover = allow_slow_failover;
                let should_abort =
                    !allow_slow_failover && stalled_for_secs >= UPDATE_STREAM_STALL_ABORT_SECS;

                let status_msg = if should_failover {
                    format!(
                        "Download stalled for {}s. Switching endpoint...",
                        stalled_for_secs
                    )
                } else if should_abort {
                    format!(
                        "Download stalled for {}s on the final endpoint.",
                        stalled_for_secs
                    )
                } else {
                    format!(
                        "No data for {}s. Waiting for endpoint response...",
                        stalled_for_secs
                    )
                };

                emit_update_progress_detail(
                    app_handle,
                    "downloading_stalled",
                    progress,
                    downloaded,
                    (total_size > 0).then_some(total_size),
                    Some(url),
                    &status_msg,
                );

                if should_failover || should_abort {
                    let _ = std::fs::remove_file(&file_path);
                    if should_failover {
                        log_startup_event(&format!(
                            "update download stalled: endpoint={} action=failover stalled_for={}s",
                            url, stalled_for_secs
                        ));
                        return Err(format!(
                            "download stalled for {}s (switching endpoint)",
                            stalled_for_secs
                        ));
                    }
                    log_startup_event(&format!(
                        "update download stalled: endpoint={} action=abort stalled_for={}s",
                        url, stalled_for_secs
                    ));
                    return Err(format!(
                        "download stalled for {}s on final endpoint",
                        stalled_for_secs
                    ));
                }

                continue;
            }
        };

        let item = match next_item {
            Some(value) => value,
            None => break,
        };

        let chunk = match item {
            Ok(value) => {
                last_chunk_at = std::time::Instant::now();
                value
            }
            Err(err) => {
                let _ = std::fs::remove_file(&file_path);
                log_startup_event(&format!(
                    "update download stream error: endpoint={} error={}",
                    url, err
                ));
                return Err(err.to_string());
            }
        };
        if let Err(err) = file.write_all(&chunk) {
            let _ = std::fs::remove_file(&file_path);
            log_startup_event(&format!(
                "update download write error: endpoint={} error={}",
                url, err
            ));
            return Err(err.to_string());
        }
        downloaded += chunk.len() as u64;

        let progress = compute_update_progress(downloaded, total_size);
        let _ = app_handle.emit("update-progress", progress);
        emit_update_progress_detail(
            app_handle,
            "downloading",
            progress,
            downloaded,
            (total_size > 0).then_some(total_size),
            Some(url),
            "Downloading installer...",
        );
    }

    if downloaded == 0 {
        let _ = std::fs::remove_file(&file_path);
        log_startup_event(&format!(
            "update download failed: endpoint={} downloaded=0",
            url
        ));
        return Err("Downloaded 0 bytes".to_string());
    }

    let _ = file.sync_all();
    drop(file);

    emit_update_progress_detail(
        app_handle,
        "downloaded",
        100.0,
        downloaded,
        (total_size > 0).then_some(total_size),
        Some(url),
        "Download complete. Launching installer...",
    );

    log_startup_event(&format!(
        "update download completed: endpoint={} bytes={}",
        url, downloaded
    ));

    Ok(file_path)
}

#[tauri::command]
async fn download_and_install_update(
    app_handle: tauri::AppHandle,
    url: String,
    candidate_urls: Option<Vec<String>>,
    proxy_choice: Option<String>,
) -> Result<(), String> {
    UPDATE_DOWNLOAD_CANCELED.store(false, Ordering::SeqCst);

    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    let (selected_proxy_mode, selected_proxy_url) =
        resolve_proxy_runtime(&conn, proxy_choice.as_deref()).await;
    let selected_route_label =
        if selected_proxy_mode == "custom" && !selected_proxy_url.trim().is_empty() {
            format!("custom / {}", mask_proxy_url(&selected_proxy_url))
        } else if selected_proxy_mode == "none" {
            "direct".to_string()
        } else {
            "system".to_string()
        };

    let mut proxy_routes: Vec<(String, String, String)> = vec![(
        selected_proxy_mode.clone(),
        selected_proxy_url.clone(),
        selected_route_label.clone(),
    )];
    if selected_proxy_mode != "none" {
        proxy_routes.push((
            "none".to_string(),
            String::new(),
            "direct fallback".to_string(),
        ));
    }

    let candidates = normalize_update_candidates(url, candidate_urls);
    if candidates.is_empty() {
        log_startup_event("update download aborted: no candidate URL provided");
        return Err("No update download URL provided".to_string());
    }
    log_startup_event(&format!(
        "update install command started: candidates={} selected_route={}",
        candidates.len(),
        selected_route_label
    ));

    let _ = app_handle.emit("update-progress", 0.0_f64);
    emit_update_progress_detail(
        &app_handle,
        "selecting_route",
        0.0,
        0,
        None,
        None,
        "Selecting the fastest download route...",
    );

    let mut file_path_opt = None;
    let mut route_errors = Vec::new();

    'route_attempt: for (route_index, (proxy_mode, proxy_url, route_label)) in
        proxy_routes.iter().enumerate()
    {
        log_startup_event(&format!("update route start: {}", route_label));
        let _proxy_guard = apply_proxy_env_with_guard(proxy_mode, proxy_url).await;

        emit_update_progress_detail(
            &app_handle,
            "route_policy_start",
            0.0,
            0,
            None,
            None,
            &format!("Using network route: {}", route_label),
        );

        let client = match reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(UPDATE_CONNECT_TIMEOUT_SECS))
            .timeout(std::time::Duration::from_secs(UPDATE_DOWNLOAD_TIMEOUT_SECS))
            .build()
        {
            Ok(c) => c,
            Err(err) => {
                log_startup_event(&format!(
                    "update route client init failed: route={} error={}",
                    route_label, err
                ));
                route_errors.push(format!(
                    "{} => failed to initialize client: {}",
                    route_label, err
                ));
                continue;
            }
        };

        let ordered_candidates = select_update_download_order(&client, &candidates).await;
        log_startup_event(&format!(
            "update route candidate order [{}]: {:?}",
            route_label, ordered_candidates
        ));

        let mut endpoint_errors = Vec::new();
        for (index, candidate) in ordered_candidates.iter().enumerate() {
            let _ = app_handle.emit("update-progress", 0.0_f64);
            emit_update_progress_detail(
                &app_handle,
                "candidate_start",
                0.0,
                0,
                None,
                Some(candidate),
                &format!(
                    "Trying download endpoint ({}/{})...",
                    index + 1,
                    ordered_candidates.len()
                ),
            );
            match download_update_from_url(
                &app_handle,
                &client,
                candidate,
                index + 1 < ordered_candidates.len(),
            )
            .await
            {
                Ok(path) => {
                    log_startup_event(&format!(
                        "update candidate succeeded: route={} endpoint={}",
                        route_label, candidate
                    ));
                    file_path_opt = Some(path);
                    break 'route_attempt;
                }
                Err(err) => {
                    if is_update_canceled_error(&err) || is_update_cancel_requested() {
                        log_startup_event(&format!(
                            "update download canceled: route={} endpoint={}",
                            route_label, candidate
                        ));
                        emit_update_progress_detail(
                            &app_handle,
                            "canceled",
                            0.0,
                            0,
                            None,
                            Some(candidate),
                            "Download canceled by user.",
                        );
                        return Err("Update download canceled by user.".to_string());
                    }
                    log_startup_event(&format!(
                        "update candidate failed: route={} endpoint={} error={}",
                        route_label, candidate, err
                    ));
                    emit_update_progress_detail(
                        &app_handle,
                        "candidate_failed",
                        0.0,
                        0,
                        None,
                        Some(candidate),
                        &format!("Endpoint failed: {}", err),
                    );
                    endpoint_errors.push(format!("{} => {}", candidate, err));
                }
            }
        }

        if !endpoint_errors.is_empty() {
            route_errors.push(format!(
                "{} => {}",
                route_label,
                endpoint_errors.join(" | ")
            ));
        }

        if route_index + 1 < proxy_routes.len() {
            let next_route_label = &proxy_routes[route_index + 1].2;
            emit_update_progress_detail(
                &app_handle,
                "route_policy_retry",
                0.0,
                0,
                None,
                None,
                &format!(
                    "Download failed via {}. Retrying with {}...",
                    route_label, next_route_label
                ),
            );
        }
    }

    let file_path = match file_path_opt {
        Some(path) => {
            let _ = app_handle.emit("update-progress", 100.0_f64);
            emit_update_progress_detail(
                &app_handle,
                "download_complete",
                100.0,
                0,
                None,
                None,
                "Download complete. Launching installer...",
            );
            path
        }
        None => {
            if is_update_cancel_requested() {
                log_startup_event("update download canceled after route attempts");
                return Err("Update download canceled by user.".to_string());
            }
            log_startup_event(&format!(
                "update download failed on all endpoints: {}",
                route_errors.join(" || ")
            ));
            return Err(format!(
                "Update download failed on all endpoints: {}",
                route_errors.join(" || ")
            ));
        }
    };

    #[cfg(target_os = "windows")]
    {
        let file_path_str = file_path.to_string_lossy().to_string();
        log_startup_event(&format!("launching installer: {}", file_path_str));

        let launch_result = if file_path_str.to_ascii_lowercase().ends_with(".msi") {
            Command::new("msiexec")
                .args(["/i", &file_path_str])
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("msiexec launch failed: {}", e))
        } else {
            Command::new(&file_path)
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("direct launch failed: {}", e))
        };

        if let Err(primary_err) = launch_result {
            log_startup_event(&format!(
                "installer primary launch failed, attempting explorer fallback: {}",
                primary_err
            ));
            Command::new("explorer")
                .arg(&file_path)
                .spawn()
                .map(|_| ())
                .map_err(|fallback_err| {
                    format!(
                        "Failed to launch installer. {}; fallback shell open failed: {}",
                        primary_err, fallback_err
                    )
                })?;
        }

        app_handle.exit(0);
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&file_path)
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).map_err(|e| e.to_string())?;
        Command::new("xdg-open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn cancel_update_download(app_handle: tauri::AppHandle) -> Result<(), String> {
    UPDATE_DOWNLOAD_CANCELED.store(true, Ordering::SeqCst);
    log_startup_event("update cancel requested by user");
    emit_update_progress_detail(
        &app_handle,
        "cancel_requested",
        0.0,
        0,
        None,
        None,
        "Cancel requested. Stopping download...",
    );
    Ok(())
}
#[tauri::command]
fn save_license_file(app_handle: tauri::AppHandle, key: String) -> Result<(), String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;
    if !app_dir.exists() {
        std::fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
    }
    let lic_path = app_dir.join("license.key");
    std::fs::write(lic_path, key).map_err(|e| e.to_string())?;
    Ok(())
}

fn sanitize_export_filename(filename: &str) -> String {
    let cleaned: String = filename
        .trim()
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(*ch, '.' | '-' | '_' | ' '))
        .collect();

    let normalized = cleaned.trim().trim_start_matches('.').to_string();
    if normalized.is_empty() {
        "export.bin".to_string()
    } else {
        normalized
    }
}

fn unique_export_path(dir: &std::path::Path, filename: &str) -> std::path::PathBuf {
    let base = dir.join(filename);
    if !base.exists() {
        return base;
    }

    let path = std::path::Path::new(filename);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("export");
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");

    for idx in 1..10000 {
        let candidate_name = if ext.is_empty() {
            format!("{} ({})", stem, idx)
        } else {
            format!("{} ({}).{}", stem, idx, ext)
        };
        let candidate = dir.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    dir.join(format!("{}_{}.bin", stem, now_unix_ts()))
}

fn open_path_with_default_app(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

fn normalize_existing_path(path: &std::path::Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn reveal_path_in_file_manager(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // Use separate args for Explorer selection to avoid command-line parse ambiguity.
        let canonical = normalize_existing_path(path);
        let mut windows_path = canonical.to_string_lossy().replace('/', "\\");
        if windows_path.starts_with(r"\\?\") {
            windows_path = windows_path.trim_start_matches(r"\\?\").to_string();
        }
        Command::new("explorer")
            .arg("/select,")
            .arg(windows_path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[cfg(target_os = "macos")]
    {
        let canonical = normalize_existing_path(path);
        Command::new("open")
            .arg("-R")
            .arg(canonical)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[cfg(target_os = "linux")]
    {
        // Linux file manager selection is not standardized; open parent directory as best effort.
        let canonical = normalize_existing_path(path);
        let target_dir = path
            .parent()
            .map(|value| value.to_path_buf())
            .or_else(|| canonical.parent().map(|value| value.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        if Command::new("xdg-open").arg(&target_dir).spawn().is_ok() {
            return Ok(());
        }

        Command::new("gio")
            .arg("open")
            .arg(target_dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn save_export_file(
    app_handle: tauri::AppHandle,
    filename: String,
    base64_data: String,
    open_after_save: Option<bool>,
) -> Result<String, String> {
    log_startup_event(&format!(
        "export requested: filename={} open_after_save={}",
        filename,
        open_after_save.unwrap_or(false)
    ));
    if base64_data.trim().is_empty() {
        log_startup_event("export failed: empty content");
        return Err("Export content is empty.".to_string());
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_data.trim())
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(base64_data.trim()))
        .map_err(|e| format!("Failed to decode export content: {}", e))?;

    if bytes.is_empty() {
        log_startup_event("export failed: decoded content empty");
        return Err("Export content is empty.".to_string());
    }

    let target_dir = dirs::download_dir()
        .or_else(|| app_handle.path().document_dir().ok())
        .or_else(|| app_handle.path().app_data_dir().ok())
        .ok_or_else(|| "Unable to resolve export directory.".to_string())?;

    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to prepare export directory: {}", e))?;

    let safe_filename = sanitize_export_filename(&filename);
    let target_path = unique_export_path(&target_dir, &safe_filename);
    std::fs::write(&target_path, &bytes)
        .map_err(|e| format!("Failed to save export file: {}", e))?;

    if open_after_save.unwrap_or(false) {
        if let Err(err) = open_path_with_default_app(&target_path) {
            eprintln!(
                "WARN: export file saved but failed to open automatically: {}",
                err
            );
            log_startup_event(&format!(
                "export saved but open_after_save failed: path={} error={}",
                target_path.to_string_lossy(),
                err
            ));
        }
    }

    log_startup_event(&format!(
        "export saved: path={} bytes={}",
        target_path.to_string_lossy(),
        bytes.len()
    ));
    Ok(target_path.to_string_lossy().to_string())
}

#[tauri::command]
fn reveal_export_file(path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        log_startup_event("reveal export failed: empty path");
        return Err("Export path is empty.".to_string());
    }

    let target_path = std::path::PathBuf::from(trimmed);
    if !target_path.exists() {
        log_startup_event(&format!(
            "reveal export failed: file not found path={}",
            trimmed
        ));
        return Err("Export file no longer exists.".to_string());
    }

    log_startup_event(&format!(
        "reveal export path requested: {}",
        target_path.to_string_lossy()
    ));
    reveal_path_in_file_manager(&target_path)
}

#[tauri::command]
fn load_license_file(app_handle: tauri::AppHandle) -> Result<String, String> {
    let started = std::time::Instant::now();
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;
    let lic_path = app_dir.join("license.key");
    let result: Result<String, String> = if lic_path.exists() {
        let key = std::fs::read_to_string(lic_path).map_err(|e| e.to_string())?;
        Ok(key.trim().to_string())
    } else {
        Ok("".to_string())
    };
    let elapsed_ms = started.elapsed().as_millis();
    match &result {
        Ok(value) => {
            log_startup_event(&format!(
                "load_license_file success: exists={} key_len={} elapsed_ms={}",
                !value.is_empty(),
                value.len(),
                elapsed_ms
            ));
        }
        Err(err) => {
            log_startup_event(&format!(
                "load_license_file failed: elapsed_ms={} reason=\"{}\"",
                elapsed_ms,
                summarize_error_text(err, 200)
            ));
        }
    }
    result
}

#[tauri::command]
async fn save_setting(
    app_handle: tauri::AppHandle,
    key: String,
    value: String,
) -> Result<(), String> {
    let normalized_key = key.trim().to_string();
    let mut normalized_value = value.trim().to_string();

    if normalized_key == "api_port" {
        let parsed_port = normalized_value
            .parse::<u16>()
            .map_err(|_| "api_port must be between 1 and 65535.".to_string())?;
        if parsed_port == 0 {
            return Err("api_port must be between 1 and 65535.".to_string());
        }
        normalized_value = parsed_port.to_string();
    }

    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;

    db::save_setting(&conn, &normalized_key, &normalized_value)
        .await
        .map_err(|e| e.to_string())?;

    // Audit Log for critical settings
    if normalized_key.starts_with("proxy")
        || normalized_key == "api_port"
        || normalized_key == "api_bind_host"
        || normalized_key == "api_access_token"
        || normalized_key == "api_tls_enabled"
        || normalized_key == "slack_webhook"
        || normalized_key.starts_with("policy")
    {
        let lower_key = normalized_key.to_lowercase();
        let should_mask = lower_key.contains("webhook")
            || lower_key.contains("key")
            || lower_key.contains("token")
            || lower_key.contains("secret");
        let val_display = if should_mask {
            "***"
        } else {
            normalized_value.as_str()
        };
        let _ = db::record_audit_log(
            &conn,
            "CONFIG",
            "System",
            &format!("Updated {}: {}", normalized_key, val_display),
        )
        .await;
    }

    Ok(())
}

#[tauri::command]
async fn get_setting(app_handle: tauri::AppHandle, key: String) -> Result<String, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::get_setting(&conn, &key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_policies(app_handle: tauri::AppHandle) -> Result<Vec<Policy>, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::list_policies(&conn).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_policy_cmd(app_handle: tauri::AppHandle, policy: Policy) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::save_policy(&conn, &policy)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_policy_cmd(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::delete_policy(&conn, &id)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Clone, serde::Serialize)]
struct ScanProgress {
    current: usize,
    total: usize,
    message: String,
}

#[derive(Clone)]
struct ScanProgressEmitter {
    app_handle: tauri::AppHandle,
    total_hint: usize,
    emitted_steps: Arc<AtomicUsize>,
}

impl ScanProgressEmitter {
    fn new(app_handle: tauri::AppHandle, total_hint: usize) -> Self {
        Self {
            app_handle,
            total_hint: total_hint.max(1),
            emitted_steps: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn emit(&self, event: &str, payload: ScanProgress) -> Result<(), tauri::Error> {
        if event == "scan-progress" {
            let step = self.emitted_steps.fetch_add(1, Ordering::SeqCst) + 1;
            let total = if step >= self.total_hint {
                step + 1
            } else {
                self.total_hint
            };
            return self.app_handle.emit(
                event,
                ScanProgress {
                    current: step,
                    total,
                    message: payload.message,
                },
            );
        }
        self.app_handle.emit(event, payload)
    }

    fn emit_complete(&self, message: impl Into<String>) {
        let observed = self.emitted_steps.load(Ordering::SeqCst);
        let total = self.total_hint.max(observed).max(1);
        self.emitted_steps.store(total, Ordering::SeqCst);
        let _ = self.app_handle.emit(
            "scan-progress",
            ScanProgress {
                current: total,
                total,
                message: message.into(),
            },
        );
    }
}

async fn load_account_enabled_rules(
    conn: &sqlx::Pool<sqlx::Sqlite>,
    account_id: &str,
) -> Option<HashSet<String>> {
    match db::get_account_rules_config(conn, account_id).await {
        Ok(rules) => {
            if rules.is_empty() {
                return None;
            }

            let mut enabled_rule_ids = HashSet::new();
            for rule in rules {
                let is_enabled = rule
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                if is_enabled {
                    if let Some(rule_id) = rule.get("id").and_then(|v| v.as_str()) {
                        enabled_rule_ids.insert(rule_id.to_string());
                    }
                }
            }
            Some(enabled_rule_ids)
        }
        Err(_) => None,
    }
}

fn rule_enabled(rule_set: &Option<HashSet<String>>, rule_id: &str) -> bool {
    match rule_set {
        Some(enabled_rules) => enabled_rules.contains(rule_id),
        None => true,
    }
}

fn estimate_rule_step_count(provider: &str, enabled_rules: Option<&HashSet<String>>) -> usize {
    let provider_key = provider.to_ascii_lowercase();
    let default_count = match provider_key.as_str() {
        "aws" => 13,
        "azure" => 9,
        "gcp" => 6,
        _ => 6,
    };

    let mut count = enabled_rules.map(|set| set.len()).unwrap_or(default_count);
    if provider_key == "aws" {
        // aws_rds_idle triggers two emitted scan stages (stopped + idle RDS).
        if enabled_rules
            .map(|set| set.contains("aws_rds_idle"))
            .unwrap_or(true)
        {
            count += 1;
        }
    }

    count.max(1)
}

fn collect_scan_result<E: std::fmt::Display>(
    result: Result<Vec<WastedResource>, E>,
    all_results: &mut Vec<WastedResource>,
    attempted_scan_checks: &mut usize,
    successful_scan_checks: &mut usize,
    failed_scan_checks: &mut usize,
) {
    *attempted_scan_checks += 1;
    match result {
        Ok(mut res) => {
            *successful_scan_checks += 1;
            all_results.append(&mut res);
        }
        Err(err) => {
            *failed_scan_checks += 1;
            let compact = summarize_error_text(&err.to_string(), 420);
            eprintln!("Scan check failed: {}", compact);
            log_startup_event(&format!("scan check failed: {}", compact));
        }
    }
}

fn record_scan_rule_failure<E: std::fmt::Display>(
    err: E,
    provider: &str,
    account: &str,
    res_type: &str,
    api: &str,
    proxy_mode: &str,
    proxy_url: &str,
    conn: &sqlx::Pool<sqlx::Sqlite>,
) {
    let raw_error = summarize_error_text(&err.to_string(), 1000);
    let (stage, reason_code, detail) = classify_cloud_connectivity_failure(&raw_error, proxy_mode);
    let provider_name = provider_label(provider);
    let structured_error = format_connection_failure_message(
        &provider_name,
        &stage,
        &reason_code,
        proxy_mode,
        proxy_url,
        &detail,
    );
    let proxy_endpoint = proxy_endpoint_display(proxy_mode, proxy_url);

    log_startup_event(&format!(
        "scan rule failed: provider={} account={} res_type={} api={} stage={} reason={} proxy_mode={} proxy={} detail=\"{}\" raw=\"{}\"",
        provider,
        account,
        res_type,
        api,
        stage,
        reason_code,
        proxy_mode,
        proxy_endpoint,
        summarize_error_text(&detail, 220),
        summarize_error_text(&raw_error, 420)
    ));

    let pool = conn.clone();
    let provider_owned = provider.to_string();
    let account_owned = account.to_string();
    let res_type_owned = res_type.to_string();
    let api_owned = api.to_string();
    let stage_owned = stage;
    let reason_code_owned = reason_code;
    let proxy_mode_owned = proxy_mode.to_string();
    let proxy_endpoint_owned = proxy_endpoint;
    let detail_owned = detail;
    let raw_error_owned = raw_error;
    let structured_error_owned = structured_error;

    tauri::async_runtime::spawn(async move {
        report_telemetry(
            &pool,
            "app_scan_error",
            serde_json::json!({
                "provider": provider_owned,
                "account": account_owned,
                "res_type": res_type_owned,
                "api": api_owned,
                "stage": stage_owned,
                "reason_code": reason_code_owned,
                "proxy_mode": proxy_mode_owned,
                "proxy_endpoint": proxy_endpoint_owned,
                "detail": detail_owned,
                "raw_error": raw_error_owned,
                "error": structured_error_owned
            }),
        )
        .await;
    });
}

fn collect_scan_result_detailed<E: std::fmt::Display>(
    result: Result<Vec<WastedResource>, E>,
    all_results: &mut Vec<WastedResource>,
    attempted_scan_checks: &mut usize,
    successful_scan_checks: &mut usize,
    failed_scan_checks: &mut usize,
    provider: &str,
    account: &str,
    res_type: &str,
    api: &str,
    proxy_mode: &str,
    proxy_url: &str,
    conn: &sqlx::Pool<sqlx::Sqlite>,
) {
    *attempted_scan_checks += 1;
    match result {
        Ok(mut res) => {
            *successful_scan_checks += 1;
            all_results.append(&mut res);
        }
        Err(err) => {
            *failed_scan_checks += 1;
            record_scan_rule_failure(
                err, provider, account, res_type, api, proxy_mode, proxy_url, conn,
            );
        }
    }
}

#[tauri::command]
async fn run_scan(
    app_handle: tauri::AppHandle,
    license_key: Option<String>,
    aws_profile: Option<String>,
    aws_region: Option<String>,
    selected_accounts: Option<Vec<String>>,
    demo_mode: bool,
) -> Result<Vec<WastedResource>, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;

    if demo_mode {
        let demo_results = demo_data::generate_demo_data();
        db::save_scan_results(&conn, &demo_results)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(demo_results);
    }

    let start_instant = std::time::Instant::now(); // Start performance timer

    let _ = license_key;
    let key_str = "community-local".to_string();

    let cpu_threshold = db::get_setting(&conn, "policy_cpu_percent")
        .await
        .unwrap_or("2.0".into())
        .parse()
        .unwrap_or(2.0);
    let net_threshold = db::get_setting(&conn, "policy_net_mb")
        .await
        .unwrap_or("5.0".into())
        .parse()
        .unwrap_or(5.0);
    let days = db::get_setting(&conn, "policy_days")
        .await
        .unwrap_or("7".into())
        .parse()
        .unwrap_or(7);
    let global_timeout = db::get_setting(&conn, "api_timeout")
        .await
        .unwrap_or("10".into())
        .parse::<u64>()
        .unwrap_or(10);

    let global_policy = ScanPolicy {
        cpu_percent: cpu_threshold,
        network_mb: net_threshold,
        lookback_days: days,
    };

    let provider_policies_json = db::get_setting(&conn, "provider_policies")
        .await
        .unwrap_or("{}".into());
    let provider_policies: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_str(&provider_policies_json).unwrap_or_default();

    // Helper to resolve policy: Profile > Provider-Specific > Global
    let resolve_policy = |provider: &str, profile_policy_json: Option<&String>| -> ScanPolicy {
        if let Some(json) = profile_policy_json {
            if let Ok(custom) = serde_json::from_str::<ScanPolicy>(json) {
                return custom;
            }
        }

        if let Some(p) = provider_policies.get(provider) {
            let cpu = p["cpu"]
                .as_str()
                .unwrap_or("2.0")
                .parse()
                .unwrap_or(cpu_threshold);
            let net = p["net"]
                .as_str()
                .unwrap_or("5.0")
                .parse()
                .unwrap_or(net_threshold);
            let d = p["days"].as_str().unwrap_or("7").parse().unwrap_or(days);
            ScanPolicy {
                cpu_percent: cpu,
                network_mb: net,
                lookback_days: d,
            }
        } else {
            global_policy.clone()
        }
    };

    let active_policies = db::list_policies(&conn)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.is_active)
        .collect::<Vec<_>>();

    let mut all_results = Vec::new();
    let mut result_attribution: HashMap<String, ScanFindingAttribution> = HashMap::new();
    let mut attempted_scan_checks: usize = 0;
    let mut successful_scan_checks: usize = 0;
    let mut failed_scan_checks: usize = 0;
    let mut credential_precheck_failures: Vec<String> = Vec::new();

    // Determine max timeout for this scan session based on selected profiles
    let mut session_timeout = global_timeout;

    let aws_profiles_to_scan =
        resolve_aws_profiles_to_scan(selected_accounts.as_ref(), aws_profile.as_deref());
    let aws_local_profile_map =
        build_aws_local_profile_map(aws_utils::list_profiles().unwrap_or_default());

    let all_profiles = db::list_cloud_profiles(&conn).await.unwrap_or_default();

    // Filter profiles if specific accounts selected
    let profiles = filter_cloud_profiles_by_selection(all_profiles, selected_accounts.as_ref());
    let account_proxy_assignments = load_account_proxy_assignments(&conn).await;
    let mut enabled_rules_by_account: HashMap<String, Option<HashSet<String>>> = HashMap::new();
    for p in &aws_profiles_to_scan {
        let account_id = format!("aws_local:{}", p);
        let enabled_rules = load_account_enabled_rules(&conn, &account_id).await;
        enabled_rules_by_account.insert(account_id, enabled_rules);
    }
    for p in &profiles {
        let enabled_rules = load_account_enabled_rules(&conn, &p.id).await;
        enabled_rules_by_account.insert(p.id.clone(), enabled_rules);
    }

    // Update session timeout if any selected profile has a custom timeout > global
    for p in &profiles {
        if let Some(t) = p.timeout_seconds {
            if t as u64 > session_timeout {
                session_timeout = t as u64;
            }
        }
    }

    let scan_accounts_total = aws_profiles_to_scan.len() + profiles.len();
    let mut estimated_steps = 0usize;
    for p in &aws_profiles_to_scan {
        let account_id = format!("aws_local:{}", p);
        let enabled_rules = enabled_rules_by_account
            .get(&account_id)
            .and_then(|rules| rules.as_ref());
        estimated_steps += 2 + estimate_rule_step_count("aws", enabled_rules);
    }
    for p in &profiles {
        let enabled_rules = enabled_rules_by_account
            .get(&p.id)
            .and_then(|rules| rules.as_ref());
        estimated_steps += 2 + estimate_rule_step_count(&p.provider, enabled_rules);
    }
    let total_steps = estimated_steps.max((scan_accounts_total.max(1)) * 2);
    let scan_progress = ScanProgressEmitter::new(app_handle.clone(), total_steps);
    let mut current_step = 0;
    log_startup_event(&format!(
        "scan started: accounts_total={} aws_local={} cloud_profiles={} estimated_steps={} timeout_secs={}",
        scan_accounts_total,
        aws_profiles_to_scan.len(),
        profiles.len(),
        total_steps,
        session_timeout
    ));

    for p in &aws_profiles_to_scan {
        let account_result_start = all_results.len();
        current_step += 1;
        let aws_account_id = format!("aws_local:{}", p);
        let aws_account_name = format!("AWS ({})", p);
        let proxy_choice = normalize_account_proxy_choice(
            account_proxy_assignments
                .get(&aws_account_id)
                .map(|value| value.as_str()),
        );
        let (aws_proxy_mode, aws_proxy_url) =
            resolve_proxy_runtime(&conn, Some(proxy_choice.as_str())).await;
        let _proxy_guard = apply_proxy_env_with_guard(&aws_proxy_mode, &aws_proxy_url).await;
        let _ = scan_progress.emit(
            "scan-progress",
            ScanProgress {
                current: current_step,
                total: total_steps,
                message: format!("Scanning AWS profile '{}'...", p),
            },
        );

        let mut effective_region = aws_region
            .clone()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let mut access_key_id: Option<&str> = None;
        let mut secret_access_key: Option<&str> = None;

        if let Some((profile_key, profile_secret, profile_region)) = aws_local_profile_map.get(p) {
            let key_trimmed = profile_key.trim();
            if !key_trimmed.is_empty() {
                access_key_id = Some(key_trimmed);
            }

            let secret_trimmed = profile_secret.trim();
            if !secret_trimmed.is_empty() {
                secret_access_key = Some(secret_trimmed);
            }

            if effective_region.is_none() {
                let region_trimmed = profile_region.trim();
                if !region_trimmed.is_empty() {
                    effective_region = Some(region_trimmed.to_string());
                }
            }
        }

        let profile_for_runtime = if access_key_id.is_some() && secret_access_key.is_some() {
            None
        } else {
            Some(p.as_str())
        };
        let resolved_region = effective_region
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "us-east-1".to_string());
        let _aws_env_guard = apply_aws_env_with_guard(
            profile_for_runtime,
            access_key_id,
            secret_access_key,
            Some(resolved_region.as_str()),
        )
        .await;

        let config = aws_config::load_from_env().await;
        if config.region().is_some() || !resolved_region.is_empty() {
            let ec2_client = Ec2Client::new(&config);
            let elb_client = ElbClient::new(&config);
            let cw_client = CwClient::new(&config);
            let rds_client = RdsClient::new(&config);
            let s3_client = S3Client::new(&config);
            let region_name = config
                .region()
                .map(|region| region.to_string())
                .unwrap_or_else(|| resolved_region.clone());

            // AWS Local Profile Policy (No DB record for local AWS yet, use Global/Provider)
            let policy = resolve_policy("aws", None);
            let enabled_rules = enabled_rules_by_account
                .get(&aws_account_id)
                .cloned()
                .unwrap_or(None);

            let scanner = Scanner::new(
                ec2_client,
                elb_client,
                cw_client,
                rds_client,
                s3_client,
                region_name,
                Some(policy),
                Some(active_policies.clone()),
            );

            let _ = scan_progress.emit(
                "scan-progress",
                ScanProgress {
                    current: current_step,
                    total: total_steps,
                    message: format!("AWS {}: Validating credentials...", p),
                },
            );

            attempted_scan_checks += 1;
            if let Err(err) = Ec2Client::new(&config).describe_regions().send().await {
                failed_scan_checks += 1;
                let raw = err.to_string();
                let (stage, reason_code, detail) =
                    classify_cloud_connectivity_failure(&raw, &aws_proxy_mode);
                let reason = format_connection_failure_message(
                    "AWS",
                    &stage,
                    &reason_code,
                    &aws_proxy_mode,
                    &aws_proxy_url,
                    &detail,
                );
                push_credential_precheck_failure(
                    &mut credential_precheck_failures,
                    format!("AWS ({})", p),
                    reason.clone(),
                );
                let _ = report_telemetry(
                    &conn,
                    "app_scan_auth_error",
                    serde_json::json!({
                        "provider": "aws",
                        "account": p,
                        "error": reason,
                    }),
                )
                .await;
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!(
                            "AWS {}: Credential validation failed. Skipping account.",
                            p
                        ),
                    },
                );
                continue;
            }
            successful_scan_checks += 1;

            // AWS scan with explicit per-rule diagnostics.
            macro_rules! track_aws {
                ($call:expr, $type:expr, $api:expr) => {
                    attempted_scan_checks += 1;
                    match $call.await {
                        Ok(mut r) => {
                            successful_scan_checks += 1;
                            all_results.append(&mut r);
                        }
                        Err(e) => {
                            failed_scan_checks += 1;
                            let raw_error = format!("{:#}", e);
                            let fallback_error = e.to_string();
                            let merged_error = if raw_error.trim().is_empty() {
                                fallback_error
                            } else {
                                raw_error
                            };
                            let compact_raw = summarize_error_text(&merged_error, 1000);
                            let (stage, reason_code, detail) = classify_cloud_connectivity_failure(
                                &compact_raw,
                                &aws_proxy_mode,
                            );
                            let structured_error = format_connection_failure_message(
                                "AWS",
                                &stage,
                                &reason_code,
                                &aws_proxy_mode,
                                &aws_proxy_url,
                                &detail,
                            );
                            log_startup_event(&format!(
                                "scan rule failed: provider=aws account={} res_type={} api={} stage={} reason={} proxy_mode={} proxy={} detail=\"{}\" raw=\"{}\"",
                                p,
                                $type,
                                $api,
                                stage,
                                reason_code,
                                aws_proxy_mode,
                                proxy_endpoint_display(&aws_proxy_mode, &aws_proxy_url),
                                summarize_error_text(&detail, 220),
                                summarize_error_text(&compact_raw, 420)
                            ));
                            let _ = report_telemetry(
                                &conn,
                                "app_scan_error",
                                serde_json::json!({
                                    "provider": "aws",
                                    "account": p,
                                    "res_type": $type,
                                    "api": $api,
                                    "stage": stage,
                                    "reason_code": reason_code,
                                    "proxy_mode": aws_proxy_mode.as_str(),
                                    "proxy_endpoint": proxy_endpoint_display(&aws_proxy_mode, &aws_proxy_url),
                                    "detail": detail,
                                    "raw_error": compact_raw,
                                    "error": structured_error
                                }),
                            )
                            .await;
                        }
                    }
                };
            }

            if rule_enabled(&enabled_rules, "aws_ebs_unattached") {
                track_aws!(scanner.scan_ebs_volumes(), "ebs", "ec2:DescribeVolumes");
            }
            if rule_enabled(&enabled_rules, "aws_eip_unused") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Analyzing Elastic IPs...", p),
                    },
                );
                track_aws!(scanner.scan_elastic_ips(), "eip", "ec2:DescribeAddresses");
            }
            if rule_enabled(&enabled_rules, "aws_snapshot_old") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Checking Snapshots...", p),
                    },
                );
                track_aws!(
                    scanner.scan_snapshots(days),
                    "snapshot",
                    "ec2:DescribeSnapshots"
                );
            }
            if rule_enabled(&enabled_rules, "aws_elb_idle") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Inspecting Load Balancers...", p),
                    },
                );
                track_aws!(
                    scanner.scan_load_balancers(),
                    "elb",
                    "elasticloadbalancing:DescribeLoadBalancers"
                );
            }
            if rule_enabled(&enabled_rules, "aws_rds_idle") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Finding Stopped RDS...", p),
                    },
                );
                track_aws!(
                    scanner.scan_stopped_rds_instances(),
                    "rds_stop",
                    "rds:DescribeDBInstances"
                );
            }
            if rule_enabled(&enabled_rules, "aws_ec2_idle") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Identifying Idle EC2...", p),
                    },
                );
                track_aws!(
                    scanner.scan_idle_instances(),
                    "ec2_idle",
                    "ec2:DescribeInstances+cloudwatch:GetMetricStatistics"
                );
            }
            if rule_enabled(&enabled_rules, "aws_rds_idle") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Checking Idle RDS...", p),
                    },
                );
                track_aws!(
                    scanner.scan_idle_rds(),
                    "rds_idle",
                    "rds:DescribeDBInstances+cloudwatch:GetMetricStatistics"
                );
            }
            if rule_enabled(&enabled_rules, "aws_nat_idle") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Scanning NAT Gateways...", p),
                    },
                );
                track_aws!(
                    scanner.scan_idle_nat_gateways(),
                    "nat",
                    "ec2:DescribeNatGateways"
                );
            }
            if rule_enabled(&enabled_rules, "aws_ami_old") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Cleaning up AMIs...", p),
                    },
                );
                track_aws!(scanner.scan_old_amis(), "ami", "ec2:DescribeImages");
            }
            if rule_enabled(&enabled_rules, "aws_ebs_underutilized") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Checking Volume IOPS...", p),
                    },
                );
                track_aws!(
                    scanner.scan_underutilized_ebs(),
                    "ebs_low_iops",
                    "ec2:DescribeVolumes+cloudwatch:GetMetricStatistics"
                );
            }
            if rule_enabled(&enabled_rules, "aws_ec2_oversized") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Finding Oversized EC2...", p),
                    },
                );
                track_aws!(
                    scanner.scan_oversized_instances(),
                    "ec2_oversized",
                    "ec2:DescribeInstances+cloudwatch:GetMetricStatistics"
                );
            }
            if rule_enabled(&enabled_rules, "aws_s3_no_lifecycle") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Checking S3 Lifecycle...", p),
                    },
                );
                track_aws!(
                    scanner.scan_s3_buckets(),
                    "s3_lifecycle",
                    "s3:ListBuckets+s3:GetBucketLifecycleConfiguration"
                );
            }
            if rule_enabled(&enabled_rules, "aws_log_retention") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Analyzing CloudWatch Logs...", p),
                    },
                );
                track_aws!(
                    scanner.scan_cloudwatch_logs(),
                    "cw_logs",
                    "logs:DescribeLogGroups"
                );
            }
            if rule_enabled(&enabled_rules, "aws_eks_node_idle") {
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!("AWS {}: Reviewing EKS Node Baseline...", p),
                    },
                );
                track_aws!(
                    scanner.scan_eks_idle_nodes(),
                    "eks_node",
                    "ec2:DescribeInstances+cloudwatch:GetMetricStatistics"
                );
            }
        } else {
            attempted_scan_checks += 1;
            failed_scan_checks += 1;
            let reason = "AWS region resolution failed for this profile".to_string();
            push_credential_precheck_failure(
                &mut credential_precheck_failures,
                format!("AWS ({})", p),
                reason.clone(),
            );
            let _ = report_telemetry(
                &conn,
                "app_scan_auth_error",
                serde_json::json!({
                    "provider": "aws",
                    "account": p,
                    "error": reason,
                }),
            )
            .await;
            let _ = scan_progress.emit(
                "scan-progress",
                ScanProgress {
                    current: current_step,
                    total: total_steps,
                    message: format!(
                        "AWS {}: Unable to resolve region/credentials. Skipping account.",
                        p
                    ),
                },
            );
        }
        attribute_results_for_account(
            &all_results,
            account_result_start,
            &aws_account_id,
            &aws_account_name,
            &mut result_attribution,
        );
    }

    for p in &profiles {
        let account_result_start = all_results.len();
        current_step += 1;
        let prov = &p.provider;
        let account_display_name = format!("{} ({})", provider_label(prov), p.name);
        let _ = scan_progress.emit(
            "scan-progress",
            ScanProgress {
                current: current_step,
                total: total_steps,
                message: format!("Scanning {} account '{}'...", prov, p.name),
            },
        );

        let policy = Some(resolve_policy(prov, p.policy_custom.as_ref()));
        let enabled_rules = enabled_rules_by_account.get(&p.id).cloned().unwrap_or(None);
        let provider_name = provider_label(prov);

        let _ = scan_progress.emit(
            "scan-progress",
            ScanProgress {
                current: current_step,
                total: total_steps,
                message: format!("{} {}: Validating credentials...", provider_name, p.name),
            },
        );

        attempted_scan_checks += 1;
        let precheck_timeout_secs =
            p.timeout_seconds.unwrap_or(global_timeout as i64).max(5) as u64;
        let precheck_timeout_secs = precheck_timeout_secs.min(45);

        let precheck_region = if prov == "aws" {
            extract_aws_region_hint(&p.credentials)
        } else {
            None
        };

        match tokio::time::timeout(
            std::time::Duration::from_secs(precheck_timeout_secs),
            test_connection(
                app_handle.clone(),
                prov.to_string(),
                p.credentials.clone(),
                precheck_region,
                Some(true),
                p.proxy_profile_id.clone(),
            ),
        )
        .await
        {
            Ok(Ok(_)) => {
                successful_scan_checks += 1;
            }
            Ok(Err(err)) => {
                failed_scan_checks += 1;
                push_credential_precheck_failure(
                    &mut credential_precheck_failures,
                    format!("{} ({})", provider_name, p.name),
                    err.clone(),
                );
                let _ = report_telemetry(
                    &conn,
                    "app_scan_auth_error",
                    serde_json::json!({
                        "provider": prov,
                        "account": p.name,
                        "error": compact_scan_error(&err),
                    }),
                )
                .await;
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!(
                            "{} {}: Credential validation failed. Skipping account.",
                            provider_name, p.name
                        ),
                    },
                );
                continue;
            }
            Err(_) => {
                failed_scan_checks += 1;
                let timeout_reason = format!(
                    "Credential validation timed out after {}s",
                    precheck_timeout_secs
                );
                push_credential_precheck_failure(
                    &mut credential_precheck_failures,
                    format!("{} ({})", provider_name, p.name),
                    timeout_reason.clone(),
                );
                let _ = report_telemetry(
                    &conn,
                    "app_scan_auth_error",
                    serde_json::json!({
                        "provider": prov,
                        "account": p.name,
                        "error": timeout_reason,
                    }),
                )
                .await;
                let _ = scan_progress.emit(
                    "scan-progress",
                    ScanProgress {
                        current: current_step,
                        total: total_steps,
                        message: format!(
                            "{} {}: Credential validation timed out. Skipping account.",
                            provider_name, p.name
                        ),
                    },
                );
                continue;
            }
        }

        let (profile_proxy_mode_raw, profile_proxy_url_raw) =
            resolve_proxy_runtime(&conn, p.proxy_profile_id.as_deref()).await;
        let profile_proxy_mode = profile_proxy_mode_raw;
        let profile_proxy_url = if profile_proxy_mode == "custom" {
            normalize_custom_proxy_url(&profile_proxy_url_raw)
        } else {
            profile_proxy_url_raw
        };
        let _proxy_guard =
            apply_proxy_env_with_guard(&profile_proxy_mode, &profile_proxy_url).await;
        match prov.as_str() {
            "azure" => {
                if let Ok(creds) = serde_json::from_str::<AzureCreds>(&p.credentials) {
                    let scanner = AzureScanner::new(
                        creds.subscription_id,
                        creds.tenant_id,
                        creds.client_id,
                        creds.client_secret,
                        policy,
                    );
                    if rule_enabled(&enabled_rules, "azure_disk_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Azure {}: Scanning Disks...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_disks().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "disk",
                            "compute/disks:list",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "azure_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Azure {}: Scanning Public IPs...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_public_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "ip",
                            "network/publicIPAddresses:list",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "azure_plan_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Azure {}: Analyzing Service Plans...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_app_service_plans().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "asp",
                            "web/serverfarms:list",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "azure_nic_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Azure {}: Checking NICs...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_nics().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "nic",
                            "network/networkInterfaces:list",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "azure_vm_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Azure {}: Finding Idle VMs...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_idle_vms().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "vm_idle",
                            "compute/virtualMachines:list+metrics",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "azure_sql_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Azure {}: Checking SQL Databases...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_idle_sql().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "sql_idle",
                            "sql/servers/databases:list+metrics",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "azure_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Azure {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "snapshot_old",
                            "compute/snapshots:list",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "azure_blob_no_lifecycle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Azure {}: Checking Blob Lifecycle...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_storage_containers().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "blob_lifecycle",
                            "storage/storageAccounts:list+blobServices",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "azure_aks_nodepool_review") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Azure {}: Reviewing AKS Node Pools...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_aks_node_pools().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "aks_nodepool",
                            "containerservice/managedClusters:list",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                }
            }
            "gcp" => {
                if let Ok(scanner) = GcpScanner::new(&p.credentials) {
                    if rule_enabled(&enabled_rules, "gcp_disk_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("GCP {}: Scanning Disks...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_disks().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "disk",
                            "compute/disks:list",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "gcp_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("GCP {}: Scanning External IPs...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_addresses().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "ip",
                            "compute/addresses:list",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "gcp_vm_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("GCP {}: Analyzing Idle VMs...", p.name),
                            },
                        );
                        attempted_scan_checks += 1;
                        match scanner.list_idle_vm_recommendation_regions().await {
                            Ok(regions) => {
                                let mut region_failures: Vec<String> = Vec::new();
                                for region in regions {
                                    match scanner.scan_idle_vm_recommendations(&region).await {
                                        Ok(mut vms) => all_results.append(&mut vms),
                                        Err(err) => {
                                            region_failures.push(format!("{}: {}", region, err));
                                        }
                                    }
                                }
                                if region_failures.is_empty() {
                                    successful_scan_checks += 1;
                                } else {
                                    failed_scan_checks += 1;
                                    record_scan_rule_failure(
                                        format!(
                                            "idle VM recommendations failed in {} region(s): {}",
                                            region_failures.len(),
                                            summarize_error_text(&region_failures.join(" | "), 700)
                                        ),
                                        prov,
                                        &p.name,
                                        "vm_idle",
                                        "recommender/idleVmRecommendations:list",
                                        &profile_proxy_mode,
                                        &profile_proxy_url,
                                        &conn,
                                    );
                                }
                            }
                            Err(err) => {
                                failed_scan_checks += 1;
                                record_scan_rule_failure(
                                    err,
                                    prov,
                                    &p.name,
                                    "vm_idle_regions",
                                    "recommender/locations:list",
                                    &profile_proxy_mode,
                                    &profile_proxy_url,
                                    &conn,
                                );
                            }
                        }
                    }
                    if rule_enabled(&enabled_rules, "gcp_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("GCP {}: Checking Snapshots...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "snapshot_old",
                            "compute/snapshots:list",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "gcp_storage_no_lifecycle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("GCP {}: Checking Storage Lifecycle...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_storage_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "storage_lifecycle",
                            "storage/buckets:list",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                    if rule_enabled(&enabled_rules, "gcp_gke_nodepool_review") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("GCP {}: Reviewing GKE Node Pools...", p.name),
                            },
                        );
                        collect_scan_result_detailed(
                            scanner.scan_gke_node_pools().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                            prov,
                            &p.name,
                            "gke_nodepool",
                            "container/clusters:list",
                            &profile_proxy_mode,
                            &profile_proxy_url,
                            &conn,
                        );
                    }
                }
            }
            _ => {
                if prov == "alibaba" {
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials) {
                        let key = creds["access_key_id"].as_str().unwrap_or("");
                        let secret = creds["access_key_secret"].as_str().unwrap_or("");
                        let region = creds["region_id"].as_str().unwrap_or("cn-hangzhou");
                        let scanner = AlibabaScanner::new(key, secret, region);

                        if rule_enabled(&enabled_rules, "ali_disk_orphan") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Alibaba {}: Scanning Disks...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_disks().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "ecs_disk_orphan",
                                "ecs:DescribeDisks",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }

                        if rule_enabled(&enabled_rules, "ali_eip_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Alibaba {}: Scanning EIPs...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_eips().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "eip_unused",
                                "vpc:DescribeEipAddresses",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }

                        if rule_enabled(&enabled_rules, "ali_oss_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Alibaba {}: Checking OSS...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_oss_buckets().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "oss_unused",
                                "oss:ListBuckets+GetBucketLifecycle",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }

                        if rule_enabled(&enabled_rules, "ali_snapshot_old") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Alibaba {}: Scanning Snapshots...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_snapshots().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "snapshot_old",
                                "ecs:DescribeSnapshots",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }

                        if rule_enabled(&enabled_rules, "ali_slb_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Alibaba {}: Checking SLB...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_slb().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "slb_idle",
                                "slb:DescribeLoadBalancers",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }

                        if rule_enabled(&enabled_rules, "ali_rds_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Alibaba {}: Checking RDS...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_rds().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "rds_idle",
                                "rds:DescribeDBInstances+cms:QueryMetric",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                    }
                } else if prov == "tencent" {
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials) {
                        let sid = creds["secret_id"].as_str().unwrap_or("");
                        let skey = creds["secret_key"].as_str().unwrap_or("");
                        let region = creds["region"].as_str().unwrap_or("ap-guangzhou");
                        let scanner = tencent::TencentScanner::new(sid, skey, region);

                        if rule_enabled(&enabled_rules, "tc_cvm_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Tencent {}: Scanning CVM...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_cvm().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "cvm_idle",
                                "cvm:DescribeInstances",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "tc_cbs_orphan") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Tencent {}: Scanning CBS Disks...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_cbs().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "cbs_orphan",
                                "cbs:DescribeDisks",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "tc_eip_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Tencent {}: Checking EIPs...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_eips().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "eip_unused",
                                "vpc:DescribeAddresses",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "tc_clb_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Tencent {}: Checking CLB...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_clb().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "clb_idle",
                                "clb:DescribeLoadBalancers",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "tc_cdb_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Tencent {}: Checking CDB...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_cdb().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "cdb_idle",
                                "cdb:DescribeDBInstances",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "tc_cos_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Tencent {}: Checking COS Buckets...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_cos().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "cos_unused",
                                "cos:ListBuckets+GetBucketLifecycle",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                    }
                } else if prov == "baidu" {
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials) {
                        let ak = creds["access_key"].as_str().unwrap_or("");
                        let sk = creds["secret_key"].as_str().unwrap_or("");
                        let region = creds["region"].as_str().unwrap_or("bj");
                        let scanner = baidu::BaiduScanner::new(ak, sk, region);

                        if rule_enabled(&enabled_rules, "bd_bcc_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Baidu {}: Scanning BCC Instances...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_bcc().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "bcc_idle",
                                "bcc:ListInstances",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "bd_cds_orphan") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Baidu {}: Scanning CDS Disks...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_cds().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "cds_orphan",
                                "bcc:ListVolumes",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "bd_eip_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Baidu {}: Checking EIPs...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_eips().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "eip_unused",
                                "bcc:ListEip",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "bd_blb_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Baidu {}: Checking BLB...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_blb().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "blb_idle",
                                "blb:ListLoadBalancer",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "bd_bos_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!(
                                        "Baidu {}: Analyzing Storage (BOS)...",
                                        p.name
                                    ),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_bos().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "bos_unused",
                                "bos:ListBuckets+GetBucketLifecycle",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                    }
                } else if prov == "huawei" {
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials) {
                        let ak = creds["access_key"].as_str().unwrap_or("");
                        let sk = creds["secret_key"].as_str().unwrap_or("");
                        let region = creds["region"].as_str().unwrap_or("cn-north-4");
                        let pid = creds["project_id"].as_str().unwrap_or("");
                        let scanner = huawei::HuaweiScanner::new(ak, sk, region, pid);

                        if rule_enabled(&enabled_rules, "hw_ecs_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Huawei {}: Scanning ECS...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_ecs().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "ecs_idle",
                                "ecs:ListServersDetails",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "hw_evs_orphan") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Huawei {}: Scanning EVS Disks...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_evs().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "evs_orphan",
                                "evs:ListVolumes",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "hw_eip_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Huawei {}: Checking EIPs...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_eips().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "eip_unused",
                                "vpc:ListPublicips",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "hw_rds_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Huawei {}: Checking RDS...", p.name),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_rds().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "rds_idle",
                                "rds:ListInstances",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "hw_elb_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!(
                                        "Huawei {}: Checking Load Balancers...",
                                        p.name
                                    ),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_load_balancers().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "elb_idle",
                                "elb:ListLoadBalancers",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                        if rule_enabled(&enabled_rules, "hw_obs_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!(
                                        "Huawei {}: Checking OBS Lifecycle...",
                                        p.name
                                    ),
                                },
                            );
                            collect_scan_result_detailed(
                                scanner.scan_obs().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                                prov,
                                &p.name,
                                "obs_unused",
                                "obs:ListBuckets+GetBucketLifecycle",
                                &profile_proxy_mode,
                                &profile_proxy_url,
                                &conn,
                            );
                        }
                    }
                } else if prov == "volcengine" {
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials) {
                        let ak = creds["access_key"].as_str().unwrap_or("");
                        let sk = creds["secret_key"].as_str().unwrap_or("");
                        let region = creds["region"].as_str().unwrap_or("cn-beijing");
                        let scanner = volcengine::VolcengineScanner::new(ak, sk, region);

                        if rule_enabled(&enabled_rules, "volc_ecs_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Volcengine {}: Scanning ECS...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_instances().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "volc_ebs_orphan") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Volcengine {}: Scanning EBS...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_ebs().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "volc_eip_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Volcengine {}: Checking EIPs...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_eips().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "volc_clb_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Volcengine {}: Checking CLB...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_clb().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "volc_redis_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Volcengine {}: Checking Redis...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_redis().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "volc_tos_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Volcengine {}: Checking TOS...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_tos().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                    }
                } else if prov == "digitalocean" {
                    let scanner = cloud_waste_scanner_core::digitalocean::DigitalOceanScanner::new(
                        &p.credentials,
                    );
                    if rule_enabled(&enabled_rules, "dig_droplet_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("DigitalOcean {}: Scanning Droplets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_droplets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "dig_vol_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("DigitalOcean {}: Checking Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "dig_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "DigitalOcean {}: Checking Floating IPs...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "dig_lb_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "DigitalOcean {}: Checking Load Balancers...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_load_balancers().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "dig_snap_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("DigitalOcean {}: Checking Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "dig_spaces_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "DigitalOcean {}: Analyzing Spaces (CDN)...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_spaces().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "linode" {
                    let scanner = LinodeScanner::new(&p.credentials);
                    if rule_enabled(&enabled_rules, "lin_linode_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Linode {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lin_vol_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Linode {}: Checking Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lin_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Linode {}: Checking Reserved IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lin_nodebal_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Linode {}: Checking NodeBalancers...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_nodebalancers().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lin_snap_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Linode {}: Checking Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lin_oversized") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Linode {}: Finding Oversized Instances...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_oversized_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lin_obj_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Linode {}: Checking Object Storage...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "akamai" {
                    let scanner = AkamaiScanner::new(&p.credentials);
                    if rule_enabled(&enabled_rules, "akamai_instance_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Akamai {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "akamai_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Akamai {}: Checking Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "akamai_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Akamai {}: Checking Reserved IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "akamai_lb_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Akamai {}: Checking NodeBalancers...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_nodebalancers().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "akamai_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Akamai {}: Checking Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "akamai_instance_oversized") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Akamai {}: Finding Oversized Instances...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_oversized_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "akamai_obj_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Akamai {}: Checking Object Storage...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "vultr" {
                    let scanner = VultrScanner::new(&p.credentials);
                    if rule_enabled(&enabled_rules, "vultr_vps_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Vultr {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "vultr_blk_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Vultr {}: Checking Block Storage...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_blocks().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "vultr_snap_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Vultr {}: Checking Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "vultr_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Vultr {}: Checking Reserved IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_reserved_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "vultr_lb_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Vultr {}: Checking Load Balancers...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_load_balancers().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "vultr_obj_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Vultr {}: Analyzing Object Storage...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_object_storage().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "cloudflare" {
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials) {
                        let token = creds["token"].as_str().unwrap_or("");
                        let acc_id = creds["account_id"].as_str().unwrap_or("");
                        let scanner = CloudflareScanner::new(token, acc_id);
                        if rule_enabled(&enabled_rules, "cloudflare_dns_exposed") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Cloudflare {}: Checking DNS...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_dns().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "cloudflare_r2_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!(
                                        "Cloudflare {}: Checking R2 Buckets...",
                                        p.name
                                    ),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_r2().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "cloudflare_tunnel_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Cloudflare {}: Checking Tunnels...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_tunnels().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "cloudflare_worker_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Cloudflare {}: Checking Workers...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_workers().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "cloudflare_pages_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Cloudflare {}: Checking Pages...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_pages().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                    }
                } else if prov == "oracle" {
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials) {
                        let tenant = creds["tenancy_id"].as_str().unwrap_or("");
                        let user = creds["user_id"].as_str().unwrap_or("");
                        let fp = creds["fingerprint"].as_str().unwrap_or("");
                        let pk = creds["private_key"].as_str().unwrap_or("");
                        let region = creds["region"].as_str().unwrap_or("us-ashburn-1");
                        let scanner = OracleScanner::new(tenant, user, fp, pk, region);
                        if rule_enabled(&enabled_rules, "oracle_compute_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Oracle {}: Scanning Compute...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_instances().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "oracle_boot_orphan") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Oracle {}: Checking Boot Volumes...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_boot_volumes().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "oracle_block_orphan") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!(
                                        "Oracle {}: Checking Block Volumes...",
                                        p.name
                                    ),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_block_volumes().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "oracle_lb_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!(
                                        "Oracle {}: Checking Load Balancers...",
                                        p.name
                                    ),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_load_balancers().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "oracle_ip_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Oracle {}: Checking Reserved IPs...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_reserved_ips().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "oracle_obj_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!(
                                        "Oracle {}: Scanning Object Storage...",
                                        p.name
                                    ),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_object_storage().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                    }
                } else if prov == "ibm" {
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials) {
                        let key = creds["api_key"].as_str().unwrap_or("");
                        let region = creds["region"].as_str().unwrap_or("us-south");
                        let cos_endpoint = creds["cos_endpoint"].as_str().unwrap_or("");
                        let cos_service_instance_id =
                            creds["cos_service_instance_id"].as_str().unwrap_or("");
                        let scanner = ibm::IbmScanner::new(
                            key,
                            region,
                            Some(cos_endpoint),
                            Some(cos_service_instance_id),
                        );
                        if rule_enabled(&enabled_rules, "ibm_vpc_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("IBM {}: Scanning VPC Instances...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_instances().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "ibm_fip_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("IBM {}: Checking Floating IPs...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_floating_ips().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "ibm_block_orphan") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("IBM {}: Checking Block Storage...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_block_storage().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "ibm_lb_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("IBM {}: Checking Load Balancers...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_load_balancers().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "ibm_snap_old") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("IBM {}: Checking Snapshots...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_snapshots().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "ibm_cos_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("IBM {}: Checking COS Buckets...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_cos().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                    }
                } else if prov == "tianyi" {
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials) {
                        let ak = creds["access_key"].as_str().unwrap_or("");
                        let sk = creds["secret_key"].as_str().unwrap_or("");
                        let region = creds["region"].as_str().unwrap_or("cn-east-1");
                        let scanner = tianyi::TianyiScanner::new(ak, sk, region);
                        if rule_enabled(&enabled_rules, "tianyi_host_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Tianyi {}: Scanning Cloud Hosts...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_host().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "tianyi_disk_orphan") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Tianyi {}: Scanning Hard Disks...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_disk().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "tianyi_eip_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Tianyi {}: Checking EIPs...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_eips().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "tianyi_lb_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!(
                                        "Tianyi {}: Checking Load Balancers...",
                                        p.name
                                    ),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_load_balancers().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "tianyi_oos_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Tianyi {}: Scanning OOS Buckets...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_oos().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                    }
                } else if prov == "ovh" {
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials) {
                        let app_key = creds["application_key"].as_str().unwrap_or("");
                        let app_secret = creds["application_secret"].as_str().unwrap_or("");
                        let consumer_key = creds["consumer_key"].as_str().unwrap_or("");
                        let endpoint = creds["endpoint"].as_str().unwrap_or("eu");
                        let project_id = creds["project_id"].as_str().unwrap_or("");
                        let scanner = ovh::OvhScanner::new(
                            app_key,
                            app_secret,
                            consumer_key,
                            endpoint,
                            project_id,
                        );

                        if rule_enabled(&enabled_rules, "ovh_instance_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("OVHcloud {}: Scanning Instances...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_instances().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "ovh_volume_orphan") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("OVHcloud {}: Scanning Volumes...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_volumes().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "ovh_ip_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("OVHcloud {}: Checking Public IPs...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_ips().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "ovh_snapshot_old") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("OVHcloud {}: Scanning Snapshots...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_snapshots().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                    }
                } else if prov == "hetzner" {
                    let token = p.credentials.trim();
                    let scanner = hetzner::HetznerScanner::new(token);

                    if rule_enabled(&enabled_rules, "hetzner_server_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Hetzner {}: Scanning Servers...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_servers().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "hetzner_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Hetzner {}: Scanning Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "hetzner_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Hetzner {}: Checking Floating IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_floating_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "hetzner_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Hetzner {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "scaleway" {
                    let (token, zones) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["token"].as_str().unwrap_or("").to_string(),
                            creds["zones"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (p.credentials.trim().to_string(), "".to_string())
                    };

                    let scanner = scaleway::ScalewayScanner::new(&token, &zones);

                    if rule_enabled(&enabled_rules, "scw_server_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Scaleway {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_servers().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "scw_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Scaleway {}: Scanning Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "scw_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Scaleway {}: Checking Public IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "scw_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Scaleway {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "civo" {
                    let (token, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["token"].as_str().unwrap_or("").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (p.credentials.trim().to_string(), "".to_string())
                    };

                    let scanner = civo::CivoScanner::new(&token, &endpoint);

                    if rule_enabled(&enabled_rules, "civo_instance_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Civo {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "civo_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Civo {}: Scanning Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "civo_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Civo {}: Checking Public IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "civo_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Civo {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "equinix" {
                    let (token, endpoint, project_id) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["token"].as_str().unwrap_or("").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                            creds["project_id"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            p.credentials.trim().to_string(),
                            "".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner = equinix::EquinixScanner::new(&token, &endpoint, &project_id);

                    if rule_enabled(&enabled_rules, "equinix_instance_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Equinix {}: Scanning Devices...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "equinix_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Equinix {}: Scanning Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "equinix_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Equinix {}: Checking Reserved IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "equinix_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Equinix {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "rackspace" {
                    let (token, endpoint, project_id) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["token"].as_str().unwrap_or("").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                            creds["project_id"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            p.credentials.trim().to_string(),
                            "".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner = rackspace::RackspaceScanner::new(&token, &endpoint, &project_id);

                    if rule_enabled(&enabled_rules, "rackspace_instance_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Rackspace {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "rackspace_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Rackspace {}: Scanning Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "rackspace_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Rackspace {}: Checking Public IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "rackspace_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Rackspace {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "openstack" {
                    let (token, endpoint, project_id) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["token"].as_str().unwrap_or("").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                            creds["project_id"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            p.credentials.trim().to_string(),
                            "".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner = openstack::OpenstackScanner::new(&token, &endpoint, &project_id);

                    if rule_enabled(&enabled_rules, "openstack_instance_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("OpenStack {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "openstack_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("OpenStack {}: Scanning Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "openstack_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("OpenStack {}: Checking Public IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "openstack_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("OpenStack {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "wasabi" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner =
                        wasabi::WasabiScanner::new(&access_key, &secret_key, &region, &endpoint);

                    if rule_enabled(&enabled_rules, "wasabi_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Wasabi {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "wasabi_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Wasabi {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "wasabi_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Wasabi {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "wasabi_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Wasabi {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "backblaze" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"]
                                .as_str()
                                .unwrap_or("us-west-004")
                                .to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-west-004".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner = backblaze::BackblazeScanner::new(
                        &access_key,
                        &secret_key,
                        &region,
                        &endpoint,
                    );

                    if rule_enabled(&enabled_rules, "backblaze_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Backblaze {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "backblaze_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Backblaze {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "backblaze_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Backblaze {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "backblaze_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Backblaze {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "idrive" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner =
                        idrive::IdriveScanner::new(&access_key, &secret_key, &region, &endpoint);

                    if rule_enabled(&enabled_rules, "idrive_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("IDrive e2 {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "idrive_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "IDrive e2 {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "idrive_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "IDrive e2 {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "idrive_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "IDrive e2 {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "storj" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner =
                        storj::StorjScanner::new(&access_key, &secret_key, &region, &endpoint);

                    if rule_enabled(&enabled_rules, "storj_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Storj DCS {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "storj_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Storj DCS {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "storj_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Storj DCS {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "storj_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Storj DCS {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "dreamhost" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner = dreamhost::DreamhostScanner::new(
                        &access_key,
                        &secret_key,
                        &region,
                        &endpoint,
                    );

                    if rule_enabled(&enabled_rules, "dreamhost_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("DreamHost {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "dreamhost_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "DreamHost {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "dreamhost_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "DreamHost {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "dreamhost_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "DreamHost {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "cloudian" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner = cloudian::CloudianScanner::new(
                        &access_key,
                        &secret_key,
                        &region,
                        &endpoint,
                    );

                    if rule_enabled(&enabled_rules, "cloudian_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Cloudian {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "cloudian_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Cloudian {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "cloudian_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Cloudian {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "cloudian_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Cloudian {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "s3compatible" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner = generic_s3::GenericS3Scanner::new(
                        &access_key,
                        &secret_key,
                        &region,
                        &endpoint,
                    );

                    if rule_enabled(&enabled_rules, "s3compatible_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Generic S3 {}: Checking empty buckets...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "s3compatible_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Generic S3 {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "s3compatible_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Generic S3 {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "s3compatible_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Generic S3 {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "minio" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner =
                        minio::MinioScanner::new(&access_key, &secret_key, &region, &endpoint);

                    if rule_enabled(&enabled_rules, "minio_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("MinIO {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "minio_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "MinIO {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "minio_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("MinIO {}: Checking multipart uploads...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "minio_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "MinIO {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "ceph" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner =
                        ceph::CephScanner::new(&access_key, &secret_key, &region, &endpoint);

                    if rule_enabled(&enabled_rules, "ceph_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Ceph RGW {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "ceph_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Ceph RGW {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "ceph_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Ceph RGW {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "ceph_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Ceph RGW {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "lyve" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-west-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-west-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner =
                        lyve::LyveScanner::new(&access_key, &secret_key, &region, &endpoint);

                    if rule_enabled(&enabled_rules, "lyve_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Lyve Cloud {}: Checking empty buckets...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lyve_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Lyve Cloud {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lyve_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Lyve Cloud {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lyve_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Lyve Cloud {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "dell" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner =
                        dell::DellScanner::new(&access_key, &secret_key, &region, &endpoint);

                    if rule_enabled(&enabled_rules, "dell_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Dell ECS {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "dell_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Dell ECS {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "dell_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Dell ECS {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "dell_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Dell ECS {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "storagegrid" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner = storagegrid::StoragegridScanner::new(
                        &access_key,
                        &secret_key,
                        &region,
                        &endpoint,
                    );

                    if rule_enabled(&enabled_rules, "storagegrid_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "StorageGRID {}: Checking empty buckets...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "storagegrid_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "StorageGRID {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "storagegrid_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "StorageGRID {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "storagegrid_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "StorageGRID {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "scality" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner =
                        scality::ScalityScanner::new(&access_key, &secret_key, &region, &endpoint);

                    if rule_enabled(&enabled_rules, "scality_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Scality {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "scality_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Scality {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "scality_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Scality {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "scality_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Scality {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "hcp" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner =
                        hcp::HcpScanner::new(&access_key, &secret_key, &region, &endpoint);

                    if rule_enabled(&enabled_rules, "hcp_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("HCP {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "hcp_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("HCP {}: Checking lifecycle policies...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "hcp_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("HCP {}: Checking multipart uploads...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "hcp_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("HCP {}: Checking old object versions...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "qumulo" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner =
                        qumulo::QumuloScanner::new(&access_key, &secret_key, &region, &endpoint);

                    if rule_enabled(&enabled_rules, "qumulo_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Qumulo {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "qumulo_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Qumulo {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "qumulo_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Qumulo {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "qumulo_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Qumulo {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "nutanix" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner =
                        nutanix::NutanixScanner::new(&access_key, &secret_key, &region, &endpoint);

                    if rule_enabled(&enabled_rules, "nutanix_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Nutanix {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "nutanix_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Nutanix {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "nutanix_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Nutanix {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "nutanix_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "Nutanix {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "flashblade" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner = flashblade::FlashbladeScanner::new(
                        &access_key,
                        &secret_key,
                        &region,
                        &endpoint,
                    );

                    if rule_enabled(&enabled_rules, "flashblade_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "FlashBlade {}: Checking empty buckets...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "flashblade_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "FlashBlade {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "flashblade_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "FlashBlade {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "flashblade_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "FlashBlade {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "greenlake" {
                    let (access_key, secret_key, region, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["access_key"].as_str().unwrap_or("").to_string(),
                            creds["secret_key"].as_str().unwrap_or("").to_string(),
                            creds["region"].as_str().unwrap_or("us-east-1").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            "".to_string(),
                            "".to_string(),
                            "us-east-1".to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner = greenlake::GreenlakeScanner::new(
                        &access_key,
                        &secret_key,
                        &region,
                        &endpoint,
                    );

                    if rule_enabled(&enabled_rules, "greenlake_bucket_empty") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("GreenLake {}: Checking empty buckets...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_empty_buckets().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "greenlake_lifecycle_missing") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "GreenLake {}: Checking lifecycle policies...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_missing_lifecycle().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "greenlake_multipart_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "GreenLake {}: Checking multipart uploads...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_orphan_multipart_uploads().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "greenlake_old_versions") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!(
                                    "GreenLake {}: Checking old object versions...",
                                    p.name
                                ),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_old_versions().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "contabo" {
                    let (token, client_id, client_secret, username, password, endpoint) =
                        if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials)
                        {
                            (
                                creds["token"].as_str().unwrap_or("").to_string(),
                                creds["client_id"].as_str().unwrap_or("").to_string(),
                                creds["client_secret"].as_str().unwrap_or("").to_string(),
                                creds["username"].as_str().unwrap_or("").to_string(),
                                creds["password"].as_str().unwrap_or("").to_string(),
                                creds["endpoint"].as_str().unwrap_or("").to_string(),
                            )
                        } else {
                            (
                                p.credentials.trim().to_string(),
                                "".to_string(),
                                "".to_string(),
                                "".to_string(),
                                "".to_string(),
                                "".to_string(),
                            )
                        };

                    let scanner = contabo::ContaboScanner::new(
                        &token,
                        &client_id,
                        &client_secret,
                        &username,
                        &password,
                        &endpoint,
                    );

                    if rule_enabled(&enabled_rules, "contabo_instance_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Contabo {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "contabo_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Contabo {}: Scanning Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "contabo_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Contabo {}: Checking Public IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "contabo_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Contabo {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "gcore" {
                    let (token, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["token"].as_str().unwrap_or("").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (p.credentials.trim().to_string(), "".to_string())
                    };

                    let scanner = gcore::GcoreScanner::new(&token, &endpoint);

                    if rule_enabled(&enabled_rules, "gcore_instance_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Gcore {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "gcore_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Gcore {}: Scanning Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "gcore_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Gcore {}: Checking Public IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "gcore_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Gcore {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "upcloud" {
                    let (username, password, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["username"].as_str().unwrap_or("").to_string(),
                            creds["password"].as_str().unwrap_or("").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        let raw = p.credentials.trim();
                        let mut parts = raw.splitn(2, ':');
                        (
                            parts.next().unwrap_or("").to_string(),
                            parts.next().unwrap_or("").to_string(),
                            "".to_string(),
                        )
                    };

                    let scanner = upcloud::UpcloudScanner::new(&username, &password, &endpoint);

                    if rule_enabled(&enabled_rules, "upc_instance_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("UpCloud {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "upc_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("UpCloud {}: Scanning Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "upc_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("UpCloud {}: Checking Public IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "upc_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("UpCloud {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "leaseweb" {
                    let (token, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["token"].as_str().unwrap_or("").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (p.credentials.trim().to_string(), "".to_string())
                    };

                    let scanner = leaseweb::LeasewebScanner::new(&token, &endpoint);

                    if rule_enabled(&enabled_rules, "lw_instance_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Leaseweb {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_instances().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lw_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Leaseweb {}: Scanning Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lw_ip_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Leaseweb {}: Checking Public IPs...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_public_ips().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "lw_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("Leaseweb {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                } else if prov == "exoscale" {
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&p.credentials) {
                        let api_key = creds["api_key"].as_str().unwrap_or("").to_string();
                        let secret_key = creds["secret_key"].as_str().unwrap_or("").to_string();
                        let endpoint = creds["endpoint"].as_str().unwrap_or("").to_string();
                        let scanner =
                            exoscale::ExoscaleScanner::new(&api_key, &secret_key, &endpoint);

                        if rule_enabled(&enabled_rules, "exo_instance_idle") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Exoscale {}: Scanning Instances...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_instances().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "exo_volume_orphan") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Exoscale {}: Scanning Volumes...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_volumes().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "exo_ip_unused") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Exoscale {}: Checking Public IPs...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_public_ips().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                        if rule_enabled(&enabled_rules, "exo_snapshot_old") {
                            let _ = scan_progress.emit(
                                "scan-progress",
                                ScanProgress {
                                    current: current_step,
                                    total: total_steps,
                                    message: format!("Exoscale {}: Scanning Snapshots...", p.name),
                                },
                            );
                            collect_scan_result(
                                scanner.scan_snapshots().await,
                                &mut all_results,
                                &mut attempted_scan_checks,
                                &mut successful_scan_checks,
                                &mut failed_scan_checks,
                            );
                        }
                    }
                } else if prov == "ionos" {
                    let (token, endpoint) = if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&p.credentials)
                    {
                        (
                            creds["token"].as_str().unwrap_or("").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (p.credentials.trim().to_string(), "".to_string())
                    };

                    let scanner = ionos::IonosScanner::new(&token, &endpoint);

                    if rule_enabled(&enabled_rules, "ionos_instance_idle") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("IONOS {}: Scanning Instances...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_servers().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "ionos_volume_orphan") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("IONOS {}: Scanning Volumes...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_volumes().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "ionos_ipblock_unused") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("IONOS {}: Checking IP Blocks...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_ipblocks().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                    if rule_enabled(&enabled_rules, "ionos_snapshot_old") {
                        let _ = scan_progress.emit(
                            "scan-progress",
                            ScanProgress {
                                current: current_step,
                                total: total_steps,
                                message: format!("IONOS {}: Scanning Snapshots...", p.name),
                            },
                        );
                        collect_scan_result(
                            scanner.scan_snapshots().await,
                            &mut all_results,
                            &mut attempted_scan_checks,
                            &mut successful_scan_checks,
                            &mut failed_scan_checks,
                        );
                    }
                }
            }
        }
        attribute_results_for_account(
            &all_results,
            account_result_start,
            &p.id,
            &account_display_name,
            &mut result_attribution,
        );
    }

    // Filter out manually handled resources
    let handled_ids = db::get_handled_resources(&conn).await.unwrap_or_default();
    if !handled_ids.is_empty() {
        all_results.retain(|r| !handled_ids.contains(&r.id));
    }

    let mut scanned_accounts_meta: Vec<String> = aws_profiles_to_scan
        .iter()
        .map(|p| format!("AWS ({})", p))
        .collect();
    scanned_accounts_meta.extend(
        profiles
            .iter()
            .map(|p| format!("{} ({})", p.provider.to_uppercase(), p.name)),
    );
    let scanned_accounts_label = if scanned_accounts_meta.is_empty() {
        "none".to_string()
    } else {
        scanned_accounts_meta.join(", ")
    };
    let credential_precheck_summary =
        summarize_credential_precheck_failures(&credential_precheck_failures);

    if attempted_scan_checks == 0 {
        log_startup_event(&format!(
            "scan aborted: attempted=0 accounts=[{}]",
            scanned_accounts_label
        ));
        let _ = db::record_audit_log(
            &conn,
            "SCAN",
            "System",
            &format!(
                "Scan failed before execution. attempted=0 succeeded=0 failed=0 accounts=[{}]",
                scanned_accounts_label
            ),
        )
        .await;
        return Err("No active checks were executed for the selected accounts. Please enable rules and verify account configuration. This attempt was not counted toward your scan quota.".into());
    }

    if successful_scan_checks == 0 {
        log_startup_event(&format!(
            "scan failed: attempted={} failed={} credential_failures=[{}] accounts=[{}]",
            attempted_scan_checks,
            failed_scan_checks,
            credential_precheck_summary.as_deref().unwrap_or("none"),
            scanned_accounts_label
        ));
        let credential_failures_log = credential_precheck_summary.as_deref().unwrap_or("none");
        let _ = db::record_audit_log(
            &conn,
            "SCAN",
            "System",
            &format!(
                "Scan failed: zero successful checks. attempted={} succeeded=0 failed={} credential_failures=[{}] accounts=[{}]",
                attempted_scan_checks,
                failed_scan_checks,
                credential_failures_log,
                scanned_accounts_label
            ),
        )
        .await;
        let credential_failure_message = credential_precheck_summary
            .map(|summary| format!(" Credential validation failures: {}.", summary))
            .unwrap_or_default();
        return Err(format!(
            "No cloud data was collected due to connectivity or account configuration issues. Please verify credentials/network and try again. Attempted checks: {}, failed checks: {}.{} This attempt was not counted toward your scan quota.",
            attempted_scan_checks,
            failed_scan_checks,
            credential_failure_message
        ));
    }

    let mut scan_error_buckets: HashMap<String, i64> = HashMap::new();
    for failure in &credential_precheck_failures {
        let reason_text = failure
            .split_once(':')
            .map(|(_, right)| right.trim())
            .unwrap_or(failure.as_str());
        let category = classify_error_text_category(reason_text);
        *scan_error_buckets.entry(category).or_insert(0) += 1;
    }
    let classified_failure_total: i64 = scan_error_buckets.values().copied().sum();
    if (failed_scan_checks as i64) > classified_failure_total {
        *scan_error_buckets.entry("unknown".to_string()).or_insert(0) +=
            failed_scan_checks as i64 - classified_failure_total;
    }

    let meta = serde_json::json!({
        "scanned_accounts": scanned_accounts_meta,
        "resource_attribution": result_attribution,
        "duration_ms": start_instant.elapsed().as_millis() as u64,
        "scan_checks_attempted": attempted_scan_checks,
        "scan_checks_succeeded": successful_scan_checks,
        "scan_checks_failed": failed_scan_checks,
        "scan_error_taxonomy_version": "v1",
        "scan_error_buckets": scan_error_buckets
    });

    let visible_results = all_results.clone();

    let total_savings_calc: f64 = visible_results
        .iter()
        .map(|r| r.estimated_monthly_cost)
        .sum();
    let scan_history_id = db::save_scan_history(
        &conn,
        total_savings_calc,
        visible_results.len() as i64,
        &visible_results,
        &meta,
    )
    .await
    .ok();
    let scan_ref = scan_history_id
        .map(|id| format!("scan-history-{}", id))
        .unwrap_or_else(|| format!("scan-{}", Utc::now().timestamp()));

    db::save_scan_results(&conn, &visible_results)
        .await
        .map_err(|e| e.to_string())?;

    let _ = session_timeout;

    let total_savings: f64 = visible_results
        .iter()
        .map(|r| r.estimated_monthly_cost)
        .sum();
    let _ = db::record_audit_log(
        &conn,
        "SCAN",
        "System",
        &format!(
            "Scan completed. Found {} items, ${:.2} potential savings.",
            visible_results.len(),
            total_savings
        ),
    )
    .await;
    log_startup_event(&format!(
        "scan completed: findings={} total_savings={:.2} attempted_checks={} succeeded_checks={} failed_checks={}",
        visible_results.len(),
        total_savings,
        attempted_scan_checks,
        successful_scan_checks,
        failed_scan_checks
    ));

    let notification_started = std::time::Instant::now();
    let mut notification_trace: Vec<String> = Vec::new();
    let mut notify_attempted_channels = 0usize;
    let mut notify_success_channels = 0usize;
    let mut notify_failed_channels = 0usize;
    let mut notify_skipped_channels = 0usize;
    push_notification_trace(
        &mut notification_trace,
        &notification_started,
        format!(
            "scan notification evaluation started: channel_policies=true total_savings={:.2} findings={}",
            total_savings,
            visible_results.len()
        ),
    );

    use cloud_waste_scanner_core::notify;
    let currency = db::get_setting(&conn, "currency")
        .await
        .unwrap_or("USD".into());
    let symbol = match currency.as_str() {
        "EUR" => "€",
        "GBP" => "£",
        "CNY" => "¥",
        "JPY" => "¥",
        _ => "$",
    };
    let rate = match currency.as_str() {
        "EUR" => 0.92,
        "GBP" => 0.79,
        "CNY" => 7.20,
        "JPY" => 150.0,
        _ => 1.0,
    };

    let message = if total_savings > 0.0 {
        format!(
            "🚨 *Cloud Waste Found!* \n\nCloud Waste Scanner detected *{}* idle resources with a potential savings of *{}{:.2}/mo*.\n\nPlease open the app to review and cleanup.",
            all_results.len(),
            symbol,
            total_savings * rate
        )
    } else {
        format!(
            "✅ *Scan Completed* \n\nCloud Waste Scanner finished the scan and found *no actionable waste* in this run.\n\nChecks: *{} attempted / {} succeeded / {} failed*.",
            attempted_scan_checks,
            successful_scan_checks,
            failed_scan_checks
        )
    };
    push_notification_trace(
        &mut notification_trace,
        &notification_started,
        format!(
            "notification payload prepared: channel_policies=true currency={} normalized_savings={:.2}",
            currency,
            total_savings * rate
        ),
    );
    let findings_count = visible_results.len() as i64;
    let scanned_account_ids: Vec<String> = aws_profiles_to_scan
        .iter()
        .map(|name| format!("aws_local:{}", name))
        .chain(profiles.iter().map(|profile| profile.id.clone()))
        .collect();
    let account_notification_assignments = load_account_notification_assignments(&conn).await;
    let routing_plan =
        build_channel_routing_plan(&scanned_account_ids, &account_notification_assignments);
    push_notification_trace(
        &mut notification_trace,
        &notification_started,
        format!(
            "account notification routing resolved: scanned_accounts={} strict_routing={} routed_channel_count={}",
            scanned_account_ids.len(),
            routing_plan.strict_channel_routing,
            routing_plan.routed_channel_ids.len()
        ),
    );

    match db::list_notification_channels(&conn).await {
        Ok(channels) => {
            push_notification_trace(
                &mut notification_trace,
                &notification_started,
                format!("loaded notification channels: {}", channels.len()),
            );
            for channel in channels {
                let channel_label = format!("{} ({})", channel.name, channel.method);
                let effective_trigger_mode = match evaluate_channel_dispatch(
                    &channel,
                    &routing_plan,
                    total_savings,
                    findings_count,
                ) {
                    Ok(mode) => mode,
                    Err(ChannelSkipReason::Inactive) => {
                        notify_skipped_channels += 1;
                        push_notification_trace(
                            &mut notification_trace,
                            &notification_started,
                            format!("channel skipped (inactive): {}", channel_label),
                        );
                        continue;
                    }
                    Err(ChannelSkipReason::AccountRouting) => {
                        notify_skipped_channels += 1;
                        push_notification_trace(
                            &mut notification_trace,
                            &notification_started,
                            format!(
                                "channel skipped (account routing): {} scanned_accounts={} routed_channel_count={}",
                                channel_label,
                                scanned_account_ids.len(),
                                routing_plan.routed_channel_ids.len()
                            ),
                        );
                        continue;
                    }
                    Err(ChannelSkipReason::TriggerPolicy) => {
                        notify_skipped_channels += 1;
                        let channel_mode_display = channel
                            .trigger_mode
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .unwrap_or(NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE);
                        push_notification_trace(
                            &mut notification_trace,
                            &notification_started,
                            format!(
                                "channel skipped (trigger policy): {} channel_mode={} effective_mode={} total_savings={:.2}",
                                channel_label,
                                channel_mode_display,
                                resolve_effective_notification_trigger_mode(channel.trigger_mode.as_deref()),
                                total_savings
                            ),
                        );
                        continue;
                    }
                    Err(ChannelSkipReason::Threshold) => {
                        notify_skipped_channels += 1;
                        push_notification_trace(
                            &mut notification_trace,
                            &notification_started,
                            format!(
                                "channel skipped (threshold): {} total_savings={:.2} findings={} min_savings={:.2} min_findings={}",
                                channel_label,
                                total_savings,
                                findings_count,
                                channel.min_savings.unwrap_or(0.0),
                                channel.min_findings.unwrap_or(0)
                            ),
                        );
                        continue;
                    }
                };
                let min_savings = channel.min_savings.unwrap_or(0.0);
                let min_findings = channel.min_findings.unwrap_or(0);
                if effective_trigger_mode != NOTIFICATION_TRIGGER_MODE_WASTE_ONLY
                    && (min_savings > f64::EPSILON || min_findings > 0)
                {
                    push_notification_trace(
                        &mut notification_trace,
                        &notification_started,
                        format!(
                            "channel threshold bypassed (mode={}): {} min_savings={:.2} min_findings={}",
                            effective_trigger_mode, channel_label, min_savings, min_findings
                        ),
                    );
                }

                notify_attempted_channels += 1;
                let (proxy_mode, resolved_proxy_url) =
                    resolve_proxy_runtime(&conn, channel.proxy_profile_id.as_deref()).await;
                let proxy_url = if proxy_mode == "custom" {
                    normalize_custom_proxy_url(&resolved_proxy_url)
                } else {
                    resolved_proxy_url
                };
                let proxy_display = if proxy_mode == "custom" && !proxy_url.trim().is_empty() {
                    mask_proxy_url(&proxy_url)
                } else {
                    "-".to_string()
                };
                push_notification_trace(
                    &mut notification_trace,
                    &notification_started,
                    format!(
                        "channel dispatch start: {} mode={} min_savings={:.2} min_findings={} proxy_mode={} proxy={}",
                        channel_label, effective_trigger_mode, min_savings, min_findings, proxy_mode, proxy_display
                    ),
                );
                let _proxy_guard = apply_proxy_env_with_guard(&proxy_mode, &proxy_url).await;
                let explicit_proxy_url = if proxy_mode == "custom" && !proxy_url.trim().is_empty() {
                    Some(proxy_url.trim())
                } else {
                    None
                };
                if channel.method.eq_ignore_ascii_case("email") {
                    let recipients = parse_notification_channel_email_recipients(&channel.config);
                    if recipients.is_empty() {
                        notify_skipped_channels += 1;
                        push_notification_trace(
                            &mut notification_trace,
                            &notification_started,
                            format!(
                                "channel skipped (email recipients missing): {}",
                                channel_label
                            ),
                        );
                        continue;
                    }
                    push_notification_trace(
                        &mut notification_trace,
                        &notification_started,
                        format!(
                            "channel dispatch via email report API: {} recipients={}",
                            channel_label,
                            recipients.len()
                        ),
                    );
                    match send_scan_report_email(&key_str, &recipients, &scan_ref, &visible_results)
                        .await
                    {
                        Ok(_) => {
                            notify_success_channels += 1;
                            push_notification_trace(
                                &mut notification_trace,
                                &notification_started,
                                format!(
                                    "channel delivered via email report API: {} recipients={}",
                                    channel_label,
                                    recipients.len()
                                ),
                            );
                        }
                        Err(err) => {
                            notify_failed_channels += 1;
                            let raw = err.to_string();
                            let (stage, reason_code, http_status, _) =
                                classify_notification_test_failure(&raw, &proxy_mode);
                            push_notification_trace(
                                &mut notification_trace,
                                &notification_started,
                                format!(
                                    "channel failed: {} stage={} reason={} http={} raw={}",
                                    channel_label,
                                    stage,
                                    reason_code,
                                    http_status
                                        .map(|value| value.to_string())
                                        .unwrap_or_else(|| "-".to_string()),
                                    compact_scan_error(&raw),
                                ),
                            );
                        }
                    }
                    continue;
                }

                let mut dispatch_channel = channel.clone();
                if dispatch_channel.method.eq_ignore_ascii_case("custom") {
                    dispatch_channel.method = "webhook".to_string();
                    push_notification_trace(
                        &mut notification_trace,
                        &notification_started,
                        format!(
                            "channel method alias normalized: {} custom->webhook",
                            channel_label
                        ),
                    );
                }

                match notify::send_notification_with_proxy(
                    &dispatch_channel,
                    &message,
                    explicit_proxy_url,
                )
                .await
                {
                    Ok(status) => {
                        notify_success_channels += 1;
                        push_notification_trace(
                            &mut notification_trace,
                            &notification_started,
                            format!("channel delivered: {} http={}", channel_label, status),
                        );
                    }
                    Err(err) => {
                        notify_failed_channels += 1;
                        let raw = err.to_string();
                        let (stage, reason_code, http_status, _) =
                            classify_notification_test_failure(&raw, &proxy_mode);
                        push_notification_trace(
                            &mut notification_trace,
                            &notification_started,
                            format!(
                                "channel failed: {} stage={} reason={} http={} raw={}",
                                channel_label,
                                stage,
                                reason_code,
                                http_status
                                    .map(|value| value.to_string())
                                    .unwrap_or_else(|| "-".to_string()),
                                compact_scan_error(&raw),
                            ),
                        );
                    }
                }
            }
        }
        Err(err) => {
            notify_failed_channels += 1;
            push_notification_trace(
                &mut notification_trace,
                &notification_started,
                format!(
                    "failed to load notification channels: {}",
                    compact_scan_error(&err.to_string())
                ),
            );
        }
    }

    // Legacy fallback keeps historical behavior (only send when waste is detected),
    // and still follows global policy to avoid unexpected behavior changes.
    if total_savings > 0.0 {
        match db::get_setting(&conn, "slack_webhook").await {
            Ok(webhook) if !webhook.is_empty() => {
                push_notification_trace(
                    &mut notification_trace,
                    &notification_started,
                    "legacy slack webhook dispatch start",
                );
                match notify::send_slack_notification(
                    &webhook,
                    total_savings * rate,
                    all_results.len(),
                    symbol,
                )
                .await
                {
                    Ok(_) => {
                        push_notification_trace(
                            &mut notification_trace,
                            &notification_started,
                            "legacy slack webhook delivered",
                        );
                    }
                    Err(err) => {
                        push_notification_trace(
                            &mut notification_trace,
                            &notification_started,
                            format!(
                                "legacy slack webhook failed: {}",
                                compact_scan_error(&err.to_string())
                            ),
                        );
                    }
                }
            }
            Ok(_) => {
                push_notification_trace(
                    &mut notification_trace,
                    &notification_started,
                    "legacy slack webhook skipped: empty webhook",
                );
            }
            Err(err) => {
                push_notification_trace(
                    &mut notification_trace,
                    &notification_started,
                    format!(
                        "legacy slack webhook lookup failed: {}",
                        compact_scan_error(&err.to_string())
                    ),
                );
            }
        }
    } else {
        push_notification_trace(
            &mut notification_trace,
            &notification_started,
            "legacy slack webhook skipped: zero-waste summary uses channel-based notifications only",
        );
    }

    push_notification_trace(
        &mut notification_trace,
        &notification_started,
        format!(
            "scan notification summary: attempted={} delivered={} failed={} skipped={}",
            notify_attempted_channels,
            notify_success_channels,
            notify_failed_channels,
            notify_skipped_channels,
        ),
    );
    let notification_trace_summary = summarize_trace_entries(&notification_trace, 48);
    let _ = db::record_audit_log(
        &conn,
        "SCAN_NOTIFICATION",
        "System",
        &format!(
            "Scan notification diagnostics: attempted={} delivered={} failed={} skipped={} trace=\"{}\"",
            notify_attempted_channels,
            notify_success_channels,
            notify_failed_channels,
            notify_skipped_channels,
            notification_trace_summary,
        ),
    )
    .await;
    log_startup_event(&format!(
        "scan notification diagnostics: attempted={} delivered={} failed={} skipped={} trace=\"{}\"",
        notify_attempted_channels,
        notify_success_channels,
        notify_failed_channels,
        notify_skipped_channels,
        notification_trace_summary,
    ));

    // Keep monitor data in sync with the latest completed scan without requiring manual refresh.
    // Trigger after channel notifications to avoid temporary proxy env contention.
    let monitor_sync_handle = app_handle.clone();
    tokio::spawn(async move {
        if let Err(err) = collect_metrics(monitor_sync_handle, None, None).await {
            eprintln!("Post-scan monitor collection failed: {}", err);
        }
    });

    // Telemetry: Product Value
    let providers: Vec<String> = all_results
        .iter()
        .map(|r| r.provider.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let meta = serde_json::json!({
        "providers": providers,
        "savings": total_savings,
        "count": all_results.len()
    });
    report_telemetry(&conn, "app_scan_complete", meta).await;

    // Telemetry: Performance (Phase 3)
    let duration = start_instant.elapsed().as_millis() as u64;
    let perf_meta = serde_json::json!({
        "duration_ms": duration,
        "provider_count": profiles.len() + (if aws_profile.is_some() { 1 } else { 0 }),
        "resource_count": all_results.len(),
        "selected_subset": selected_accounts.is_some()
    });
    report_telemetry(&conn, "app_scan_performance", perf_meta).await;

    scan_progress.emit_complete("Scan complete. Preparing results...");
    Ok(visible_results)
}

fn normalize_provider_key(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn title_case_provider(raw: &str) -> String {
    raw.split(|c: char| c == '_' || c == '-' || c == ' ')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                format!(
                    "{}{}",
                    first.to_uppercase(),
                    chars.as_str().to_ascii_lowercase()
                )
            } else {
                String::new()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn provider_label(provider: &str) -> String {
    match normalize_provider_key(provider).as_str() {
        "aws" => "AWS".to_string(),
        "azure" => "Azure".to_string(),
        "gcp" | "googlecloudgcp" => "Google Cloud (GCP)".to_string(),
        "alibaba" => "Alibaba Cloud".to_string(),
        "tencent" => "Tencent Cloud".to_string(),
        "huawei" => "Huawei Cloud".to_string(),
        "digitalocean" => "DigitalOcean".to_string(),
        "linode" => "Linode".to_string(),
        "akamai" => "Akamai Connected Cloud".to_string(),
        "oracle" => "Oracle Cloud".to_string(),
        "ibm" => "IBM Cloud".to_string(),
        "volcengine" => "Volcengine".to_string(),
        "baidu" => "Baidu AI Cloud".to_string(),
        "tianyi" => "Tianyi Cloud".to_string(),
        "cloudflare" => "Cloudflare".to_string(),
        "hetzner" => "Hetzner".to_string(),
        "scaleway" => "Scaleway".to_string(),
        "exoscale" => "Exoscale".to_string(),
        "leaseweb" => "Leaseweb".to_string(),
        "upcloud" => "UpCloud".to_string(),
        "gcore" => "Gcore".to_string(),
        "contabo" => "Contabo".to_string(),
        "civo" => "Civo".to_string(),
        "equinix" => "Equinix Metal".to_string(),
        "rackspace" => "Rackspace".to_string(),
        "openstack" => "OpenStack".to_string(),
        "wasabi" => "Wasabi".to_string(),
        "backblaze" => "Backblaze B2".to_string(),
        "idrive" => "IDrive e2".to_string(),
        "storj" => "Storj DCS".to_string(),
        "dreamhost" => "DreamHost DreamObjects".to_string(),
        "cloudian" => "Cloudian HyperStore (S3)".to_string(),
        "s3compatible" => "Generic S3-Compatible".to_string(),
        "minio" => "MinIO (S3-Compatible)".to_string(),
        "ceph" => "Ceph RGW (S3-Compatible)".to_string(),
        "lyve" => "Seagate Lyve Cloud".to_string(),
        "dell" => "Dell EMC ECS (S3)".to_string(),
        "storagegrid" => "NetApp StorageGRID (S3)".to_string(),
        "scality" => "Scality (S3)".to_string(),
        "hcp" => "Hitachi HCP (S3)".to_string(),
        "qumulo" => "Qumulo (S3)".to_string(),
        "nutanix" => "Nutanix Objects (S3)".to_string(),
        "flashblade" => "Pure Storage FlashBlade (S3)".to_string(),
        "greenlake" => "HPE GreenLake (S3)".to_string(),
        "ionos" => "IONOS Cloud".to_string(),
        "ovh" => "OVHcloud".to_string(),
        _ => {
            if provider.chars().any(|c| c.is_whitespace())
                || provider.chars().any(|c| c.is_uppercase())
            {
                provider.to_string()
            } else {
                title_case_provider(provider)
            }
        }
    }
}

fn monitor_status_from_action(action_type: &str) -> String {
    match action_type.to_ascii_uppercase().as_str() {
        "RIGHTSIZE" => "rightsizing_candidate".to_string(),
        "ARCHIVE" => "archive_candidate".to_string(),
        "DELETE" => "waste_candidate".to_string(),
        _ => "finding".to_string(),
    }
}

fn compact_monitor_name(details: &str) -> Option<String> {
    let normalized = details
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.chars().take(120).collect::<String>())
    }
}

fn push_finding_metrics(
    findings: Vec<WastedResource>,
    profile: &db::CloudProfile,
    collected_at: i64,
    source: &str,
    seen_metric_keys: &mut HashSet<String>,
    target: &mut Vec<db::MonitorMetric>,
) {
    for finding in findings {
        let metric = db::MonitorMetric {
            id: finding.id,
            provider: provider_label(&profile.provider),
            region: if finding.region.trim().is_empty() {
                "-".to_string()
            } else {
                finding.region
            },
            resource_type: finding.resource_type,
            name: compact_monitor_name(&finding.details),
            status: monitor_status_from_action(&finding.action_type),
            cpu_utilization: None,
            network_in_mb: None,
            connections: None,
            updated_at: collected_at,
            source: Some(source.to_string()),
            account_id: Some(profile.id.clone()),
        };
        push_monitor_metric(metric, seen_metric_keys, target);
    }
}

fn push_monitor_metric(
    metric: db::MonitorMetric,
    seen: &mut HashSet<String>,
    target: &mut Vec<db::MonitorMetric>,
) {
    let mut item = metric;
    let provider_key = normalize_provider_key(&item.provider);
    let account_key = item.account_id.clone().unwrap_or_default();
    let base_id = item.id.clone();

    let mut candidate = base_id.clone();
    let mut idx = 1;
    loop {
        let uniq = format!("{}::{}::{}", provider_key, account_key, candidate);
        if seen.insert(uniq) {
            item.id = candidate;
            target.push(item);
            break;
        }
        idx += 1;
        candidate = format!("{}#{}", base_id, idx);
    }
}

#[tauri::command]
async fn collect_metrics(
    app_handle: tauri::AppHandle,
    aws_profile: Option<String>,
    aws_region: Option<String>,
) -> Result<Vec<db::MonitorMetric>, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;

    let cpu_threshold = db::get_setting(&conn, "policy_cpu_percent")
        .await
        .unwrap_or("2.0".into())
        .parse()
        .unwrap_or(2.0);
    let net_threshold = db::get_setting(&conn, "policy_net_mb")
        .await
        .unwrap_or("5.0".into())
        .parse()
        .unwrap_or(5.0);
    let days = db::get_setting(&conn, "policy_days")
        .await
        .unwrap_or("7".into())
        .parse()
        .unwrap_or(7);

    let policy = Some(ScanPolicy {
        cpu_percent: cpu_threshold,
        network_mb: net_threshold,
        lookback_days: days,
    });

    let collected_at = now_unix_ts();
    let mut all_metrics: Vec<db::MonitorMetric> = Vec::new();
    let mut seen_metric_keys: HashSet<String> = HashSet::new();
    let account_proxy_assignments = load_account_proxy_assignments(&conn).await;
    let aws_local_profile_map: HashMap<String, (String, String, String)> =
        aws_utils::list_profiles()
            .unwrap_or_default()
            .into_iter()
            .map(|profile| (profile.name, (profile.key, profile.secret, profile.region)))
            .collect();

    let available_aws_profiles = aws_utils::list_profiles().unwrap_or_default();
    let aws_targets: Vec<(String, String)> = if let Some(target_profile) = aws_profile {
        let inferred_region = available_aws_profiles
            .iter()
            .find(|p| p.name == target_profile)
            .map(|p| p.region.clone())
            .unwrap_or_else(|| "us-east-1".to_string());
        vec![(
            target_profile,
            aws_region.clone().unwrap_or(inferred_region),
        )]
    } else {
        available_aws_profiles
            .into_iter()
            .map(|p| {
                let region = if let Some(forced_region) = &aws_region {
                    forced_region.clone()
                } else if p.region.trim().is_empty() {
                    "us-east-1".to_string()
                } else {
                    p.region.clone()
                };
                (p.name, region)
            })
            .collect()
    };

    for (profile_name, profile_region) in aws_targets {
        let aws_account_id = format!("aws_local:{}", profile_name);
        let proxy_choice = normalize_account_proxy_choice(
            account_proxy_assignments
                .get(&aws_account_id)
                .map(|value| value.as_str()),
        );
        let _proxy_guard = apply_proxy_choice_with_guard(&conn, Some(proxy_choice.as_str())).await;
        let mut access_key_id: Option<&str> = None;
        let mut secret_access_key: Option<&str> = None;
        if let Some((profile_key, profile_secret, _profile_region)) =
            aws_local_profile_map.get(&profile_name)
        {
            let key_trimmed = profile_key.trim();
            if !key_trimmed.is_empty() {
                access_key_id = Some(key_trimmed);
            }
            let secret_trimmed = profile_secret.trim();
            if !secret_trimmed.is_empty() {
                secret_access_key = Some(secret_trimmed);
            }
        }
        let _aws_env_guard = apply_aws_env_with_guard(
            Some(profile_name.as_str()),
            access_key_id,
            secret_access_key,
            Some(profile_region.as_str()),
        )
        .await;

        let region_provider =
            RegionProviderChain::first_try(Some(Region::new(profile_region.clone())))
                .or_default_provider()
                .or_else(Region::new("us-east-1"));
        let config = aws_config::from_env().region(region_provider).load().await;

        if let Some(active_region) = config.region().map(|r| r.to_string()) {
            let ec2 = Ec2Client::new(&config);
            let elb = ElbClient::new(&config);
            let cw = CwClient::new(&config);
            let rds = RdsClient::new(&config);
            let s3 = S3Client::new(&config);
            let scanner = Scanner::new(
                ec2,
                elb,
                cw,
                rds,
                s3,
                active_region.clone(),
                policy.clone(),
                None,
            );

            match scanner.collect_ec2_metrics().await {
                Ok(metrics) => {
                    for metric in metrics {
                        let monitor_metric = db::MonitorMetric {
                            id: metric.id,
                            provider: provider_label(&metric.provider),
                            region: metric.region,
                            resource_type: metric.resource_type,
                            name: metric.name,
                            status: metric.status,
                            cpu_utilization: metric.cpu_utilization,
                            network_in_mb: metric.network_in_mb,
                            connections: metric.connections,
                            updated_at: collected_at,
                            source: Some("aws_cloudwatch".to_string()),
                            account_id: Some(format!("aws_local:{}", profile_name)),
                        };
                        push_monitor_metric(
                            monitor_metric,
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
                Err(err) => {
                    eprintln!(
                        "Monitor AWS metric collection failed for profile '{}': {}",
                        profile_name, err
                    );
                }
            }
        }
    }

    let profiles = db::list_cloud_profiles(&conn).await.unwrap_or_default();
    for profile in profiles {
        let status = if profile.credentials.trim().is_empty() {
            "credentials_missing".to_string()
        } else {
            let timeout_seconds = profile.timeout_seconds.unwrap_or(8).clamp(5, 30) as u64;
            match tokio::time::timeout(
                std::time::Duration::from_secs(timeout_seconds),
                test_connection(
                    app_handle.clone(),
                    profile.provider.clone(),
                    profile.credentials.clone(),
                    None,
                    Some(true),
                    profile.proxy_profile_id.clone(),
                ),
            )
            .await
            {
                Ok(Ok(_)) => "connected".to_string(),
                Ok(Err(_)) => "auth_error".to_string(),
                Err(_) => "auth_timeout_error".to_string(),
            }
        };

        let is_connected = status == "connected";
        let source = if status == "credentials_missing" {
            "profile_config".to_string()
        } else {
            "profile_probe".to_string()
        };

        let profile_id = profile.id.clone();
        let profile_name = profile.name.clone();
        let profile_metric = db::MonitorMetric {
            id: format!("profile:{}", profile_id),
            provider: provider_label(&profile.provider),
            region: "-".to_string(),
            resource_type: "Connected Account".to_string(),
            name: Some(profile_name),
            status,
            cpu_utilization: None,
            network_in_mb: None,
            connections: None,
            updated_at: collected_at,
            source: Some(source),
            account_id: Some(profile_id),
        };

        push_monitor_metric(profile_metric, &mut seen_metric_keys, &mut all_metrics);

        if !is_connected {
            continue;
        }

        match profile.provider.as_str() {
            "azure" => {
                if let Ok(creds) = serde_json::from_str::<AzureCreds>(&profile.credentials) {
                    let scanner = AzureScanner::new(
                        creds.subscription_id,
                        creds.tenant_id,
                        creds.client_id,
                        creds.client_secret,
                        policy.clone(),
                    );

                    if let Ok(found) = scanner.scan_idle_vms().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "azure_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_disks().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "azure_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "gcp" => {
                if let Ok(scanner) = GcpScanner::new(&profile.credentials) {
                    if let Ok(found) = scanner.scan_disks().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "gcp_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(regions) = scanner.list_idle_vm_recommendation_regions().await {
                        for region in regions {
                            if let Ok(found) = scanner.scan_idle_vm_recommendations(&region).await {
                                push_finding_metrics(
                                    found,
                                    &profile,
                                    collected_at,
                                    "gcp_live",
                                    &mut seen_metric_keys,
                                    &mut all_metrics,
                                );
                            }
                        }
                    }
                }
            }
            "alibaba" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let key = creds["access_key_id"].as_str().unwrap_or("");
                    let secret = creds["access_key_secret"].as_str().unwrap_or("");
                    let region = creds["region_id"].as_str().unwrap_or("cn-hangzhou");
                    let scanner = AlibabaScanner::new(key, secret, region);

                    if let Ok(found) = scanner.scan_disks().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "alibaba_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_eips().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "alibaba_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "tencent" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let sid = creds["secret_id"].as_str().unwrap_or("");
                    let skey = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("ap-guangzhou");
                    let scanner = tencent::TencentScanner::new(sid, skey, region);

                    if let Ok(found) = scanner.scan_cvm().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "tencent_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_cbs().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "tencent_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "huawei" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let ak = creds["access_key"].as_str().unwrap_or("");
                    let sk = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("cn-north-4");
                    let pid = creds["project_id"].as_str().unwrap_or("");
                    let scanner = huawei::HuaweiScanner::new(ak, sk, region, pid);

                    if let Ok(found) = scanner.scan_ecs().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "huawei_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_evs().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "huawei_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "oracle" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let tenant = creds["tenancy_id"].as_str().unwrap_or("");
                    let user = creds["user_ocid"].as_str().unwrap_or("");
                    let fp = creds["fingerprint"].as_str().unwrap_or("");
                    let pk = creds["private_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-ashburn-1");
                    let scanner = OracleScanner::new(tenant, user, fp, pk, region);

                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "oracle_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_block_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "oracle_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "digitalocean" => {
                let token = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    creds["token"].as_str().unwrap_or("").to_string()
                } else {
                    profile.credentials.trim().to_string()
                };

                if !token.is_empty() {
                    let scanner =
                        cloud_waste_scanner_core::digitalocean::DigitalOceanScanner::new(&token);
                    if let Ok(found) = scanner.scan_droplets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "digitalocean_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "digitalocean_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "linode" => {
                let token = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    creds["token"].as_str().unwrap_or("").to_string()
                } else {
                    profile.credentials.trim().to_string()
                };

                if !token.is_empty() {
                    let scanner = LinodeScanner::new(&token);
                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "linode_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "linode_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "vultr" => {
                let api_key = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    creds["api_key"].as_str().unwrap_or("").to_string()
                } else {
                    profile.credentials.trim().to_string()
                };

                if !api_key.is_empty() {
                    let scanner = VultrScanner::new(&api_key);
                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "vultr_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_blocks().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "vultr_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "ibm" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let key = creds["api_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-south");
                    let cos_endpoint = creds["cos_endpoint"].as_str().unwrap_or("");
                    let cos_service_instance_id =
                        creds["cos_service_instance_id"].as_str().unwrap_or("");
                    let scanner = ibm::IbmScanner::new(
                        key,
                        region,
                        Some(cos_endpoint),
                        Some(cos_service_instance_id),
                    );

                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "ibm_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_block_storage().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "ibm_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "ovh" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let app_key = creds["application_key"].as_str().unwrap_or("");
                    let app_secret = creds["application_secret"].as_str().unwrap_or("");
                    let consumer_key = creds["consumer_key"].as_str().unwrap_or("");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("eu");
                    let project_id = creds["project_id"].as_str().unwrap_or("");
                    let scanner = ovh::OvhScanner::new(
                        app_key,
                        app_secret,
                        consumer_key,
                        endpoint,
                        project_id,
                    );

                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "ovh_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "ovh_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "wasabi" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        wasabi::WasabiScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "wasabi_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "wasabi_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "backblaze" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-west-004");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        backblaze::BackblazeScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "backblaze_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "backblaze_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "idrive" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        idrive::IdriveScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "idrive_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "idrive_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "storj" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        storj::StorjScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "storj_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "storj_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "dreamhost" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        dreamhost::DreamhostScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "dreamhost_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "dreamhost_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "cloudian" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        cloudian::CloudianScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "cloudian_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "cloudian_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "s3compatible" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        generic_s3::GenericS3Scanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "s3compatible_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "s3compatible_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "minio" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        minio::MinioScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "minio_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "minio_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "ceph" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner = ceph::CephScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "ceph_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "ceph_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "lyve" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-west-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner = lyve::LyveScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "lyve_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "lyve_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "dell" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner = dell::DellScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "dell_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "dell_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "storagegrid" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner = storagegrid::StoragegridScanner::new(
                        access_key, secret_key, region, endpoint,
                    );

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "storagegrid_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "storagegrid_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "scality" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        scality::ScalityScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "scality_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "scality_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "hcp" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner = hcp::HcpScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "hcp_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "hcp_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "qumulo" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        qumulo::QumuloScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "qumulo_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "qumulo_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "nutanix" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        nutanix::NutanixScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "nutanix_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "nutanix_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "flashblade" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner = flashblade::FlashbladeScanner::new(
                        access_key, secret_key, region, endpoint,
                    );

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "flashblade_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "flashblade_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "greenlake" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("us-east-1");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner =
                        greenlake::GreenlakeScanner::new(access_key, secret_key, region, endpoint);

                    if let Ok(found) = scanner.scan_empty_buckets().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "greenlake_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_missing_lifecycle().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "greenlake_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "volcengine" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("cn-beijing");
                    let scanner =
                        volcengine::VolcengineScanner::new(access_key, secret_key, region);

                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "volcengine_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_ebs().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "volcengine_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "baidu" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("bj");
                    let scanner = baidu::BaiduScanner::new(access_key, secret_key, region);

                    if let Ok(found) = scanner.scan_bcc().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "baidu_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_cds().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "baidu_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "tianyi" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let access_key = creds["access_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let region = creds["region"].as_str().unwrap_or("cn-east-1");
                    let scanner = tianyi::TianyiScanner::new(access_key, secret_key, region);

                    if let Ok(found) = scanner.scan_host().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "tianyi_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_disk().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "tianyi_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "cloudflare" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let token = creds["token"].as_str().unwrap_or("");
                    let account_id = creds["account_id"].as_str().unwrap_or("");
                    let scanner = CloudflareScanner::new(token, account_id);

                    if let Ok(found) = scanner.scan_dns().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "cloudflare_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_r2().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "cloudflare_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "akamai" => {
                let token = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    creds["token"].as_str().unwrap_or("").to_string()
                } else {
                    profile.credentials.trim().to_string()
                };

                if !token.is_empty() {
                    let scanner = AkamaiScanner::new(&token);
                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "akamai_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "akamai_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "equinix" => {
                let (token, endpoint, project_id) = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    (
                        creds["token"].as_str().unwrap_or("").to_string(),
                        creds["endpoint"].as_str().unwrap_or("").to_string(),
                        creds["project_id"].as_str().unwrap_or("").to_string(),
                    )
                } else {
                    (
                        profile.credentials.trim().to_string(),
                        "".to_string(),
                        "".to_string(),
                    )
                };

                if !token.is_empty() {
                    let scanner = equinix::EquinixScanner::new(&token, &endpoint, &project_id);
                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "equinix_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "equinix_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "rackspace" => {
                let (token, endpoint, project_id) = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    (
                        creds["token"].as_str().unwrap_or("").to_string(),
                        creds["endpoint"].as_str().unwrap_or("").to_string(),
                        creds["project_id"].as_str().unwrap_or("").to_string(),
                    )
                } else {
                    (
                        profile.credentials.trim().to_string(),
                        "".to_string(),
                        "".to_string(),
                    )
                };

                if !token.is_empty() {
                    let scanner = rackspace::RackspaceScanner::new(&token, &endpoint, &project_id);
                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "rackspace_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "rackspace_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "openstack" => {
                let (token, endpoint, project_id) = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    (
                        creds["token"].as_str().unwrap_or("").to_string(),
                        creds["endpoint"].as_str().unwrap_or("").to_string(),
                        creds["project_id"].as_str().unwrap_or("").to_string(),
                    )
                } else {
                    (
                        profile.credentials.trim().to_string(),
                        "".to_string(),
                        "".to_string(),
                    )
                };

                if !token.is_empty() {
                    let scanner = openstack::OpenstackScanner::new(&token, &endpoint, &project_id);
                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "openstack_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "openstack_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "hetzner" => {
                let token = profile.credentials.trim().to_string();
                if !token.is_empty() {
                    let scanner = hetzner::HetznerScanner::new(&token);
                    if let Ok(found) = scanner.scan_servers().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "hetzner_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "hetzner_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "scaleway" => {
                let (token, zones) = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    (
                        creds["token"].as_str().unwrap_or("").to_string(),
                        creds["zones"].as_str().unwrap_or("").to_string(),
                    )
                } else {
                    (profile.credentials.trim().to_string(), "".to_string())
                };

                if !token.is_empty() {
                    let scanner = scaleway::ScalewayScanner::new(&token, &zones);
                    if let Ok(found) = scanner.scan_servers().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "scaleway_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "scaleway_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "civo" => {
                let (token, endpoint) = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    (
                        creds["token"].as_str().unwrap_or("").to_string(),
                        creds["endpoint"].as_str().unwrap_or("").to_string(),
                    )
                } else {
                    (profile.credentials.trim().to_string(), "".to_string())
                };

                if !token.is_empty() {
                    let scanner = civo::CivoScanner::new(&token, &endpoint);
                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "civo_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "civo_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "contabo" => {
                let (token, client_id, client_secret, username, password, endpoint) =
                    if let Ok(creds) =
                        serde_json::from_str::<serde_json::Value>(&profile.credentials)
                    {
                        (
                            creds["token"].as_str().unwrap_or("").to_string(),
                            creds["client_id"].as_str().unwrap_or("").to_string(),
                            creds["client_secret"].as_str().unwrap_or("").to_string(),
                            creds["username"].as_str().unwrap_or("").to_string(),
                            creds["password"].as_str().unwrap_or("").to_string(),
                            creds["endpoint"].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            profile.credentials.trim().to_string(),
                            "".to_string(),
                            "".to_string(),
                            "".to_string(),
                            "".to_string(),
                            "".to_string(),
                        )
                    };

                if !token.is_empty() {
                    let scanner = contabo::ContaboScanner::new(
                        &token,
                        &client_id,
                        &client_secret,
                        &username,
                        &password,
                        &endpoint,
                    );
                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "contabo_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "contabo_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "gcore" => {
                let (token, endpoint) = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    (
                        creds["token"].as_str().unwrap_or("").to_string(),
                        creds["endpoint"].as_str().unwrap_or("").to_string(),
                    )
                } else {
                    (profile.credentials.trim().to_string(), "".to_string())
                };

                if !token.is_empty() {
                    let scanner = gcore::GcoreScanner::new(&token, &endpoint);
                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "gcore_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "gcore_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "upcloud" => {
                let (username, password, endpoint) = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    (
                        creds["username"].as_str().unwrap_or("").to_string(),
                        creds["password"].as_str().unwrap_or("").to_string(),
                        creds["endpoint"].as_str().unwrap_or("").to_string(),
                    )
                } else {
                    let raw = profile.credentials.trim();
                    let mut parts = raw.splitn(2, ':');
                    (
                        parts.next().unwrap_or("").to_string(),
                        parts.next().unwrap_or("").to_string(),
                        "".to_string(),
                    )
                };

                if !username.is_empty() {
                    let scanner = upcloud::UpcloudScanner::new(&username, &password, &endpoint);
                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "upcloud_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "upcloud_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "leaseweb" => {
                let (token, endpoint) = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    (
                        creds["token"].as_str().unwrap_or("").to_string(),
                        creds["endpoint"].as_str().unwrap_or("").to_string(),
                    )
                } else {
                    (profile.credentials.trim().to_string(), "".to_string())
                };

                if !token.is_empty() {
                    let scanner = leaseweb::LeasewebScanner::new(&token, &endpoint);
                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "leaseweb_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "leaseweb_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "exoscale" => {
                if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&profile.credentials) {
                    let api_key = creds["api_key"].as_str().unwrap_or("");
                    let secret_key = creds["secret_key"].as_str().unwrap_or("");
                    let endpoint = creds["endpoint"].as_str().unwrap_or("");
                    let scanner = exoscale::ExoscaleScanner::new(api_key, secret_key, endpoint);

                    if let Ok(found) = scanner.scan_instances().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "exoscale_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "exoscale_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            "ionos" => {
                let (token, endpoint) = if let Ok(creds) =
                    serde_json::from_str::<serde_json::Value>(&profile.credentials)
                {
                    (
                        creds["token"].as_str().unwrap_or("").to_string(),
                        creds["endpoint"].as_str().unwrap_or("").to_string(),
                    )
                } else {
                    (profile.credentials.trim().to_string(), "".to_string())
                };

                if !token.is_empty() {
                    let scanner = ionos::IonosScanner::new(&token, &endpoint);
                    if let Ok(found) = scanner.scan_servers().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "ionos_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                    if let Ok(found) = scanner.scan_volumes().await {
                        push_finding_metrics(
                            found,
                            &profile,
                            collected_at,
                            "ionos_live",
                            &mut seen_metric_keys,
                            &mut all_metrics,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    let scan_results = db::get_scan_results(&conn).await.unwrap_or_default();
    for finding in scan_results {
        let compact_name = if finding.details.trim().is_empty() {
            None
        } else {
            Some(finding.details.chars().take(120).collect::<String>())
        };

        let finding_metric = db::MonitorMetric {
            id: finding.id,
            provider: provider_label(&finding.provider),
            region: finding.region,
            resource_type: finding.resource_type,
            name: compact_name,
            status: monitor_status_from_action(&finding.action_type),
            cpu_utilization: None,
            network_in_mb: None,
            connections: None,
            updated_at: collected_at,
            source: Some("latest_scan".to_string()),
            account_id: None,
        };

        push_monitor_metric(finding_metric, &mut seen_metric_keys, &mut all_metrics);
    }

    all_metrics.sort_by(|a, b| {
        a.provider
            .cmp(&b.provider)
            .then(a.resource_type.cmp(&b.resource_type))
            .then(a.id.cmp(&b.id))
    });

    db::save_resource_metrics(&conn, &all_metrics)
        .await
        .map_err(|e| e.to_string())?;

    Ok(all_metrics)
}

#[tauri::command]
async fn get_resource_metrics(
    app_handle: tauri::AppHandle,
    demo_mode: bool,
) -> Result<Vec<db::MonitorMetric>, String> {
    if demo_mode {
        let now = now_unix_ts();
        let demo_metrics = demo_data::generate_demo_metrics()
            .into_iter()
            .map(|metric| db::MonitorMetric {
                id: metric.id,
                provider: metric.provider,
                region: metric.region,
                resource_type: metric.resource_type,
                name: metric.name,
                status: metric.status,
                cpu_utilization: metric.cpu_utilization,
                network_in_mb: metric.network_in_mb,
                connections: metric.connections,
                updated_at: now,
                source: Some("demo".to_string()),
                account_id: None,
            })
            .collect::<Vec<_>>();
        return Ok(demo_metrics);
    }

    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::get_all_metrics(&conn).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_monitor_snapshots(
    app_handle: tauri::AppHandle,
    demo_mode: bool,
    window_days: Option<i64>,
) -> Result<Vec<db::MonitorSnapshot>, String> {
    if demo_mode {
        if let Some(days_raw) = window_days {
            let now = now_unix_ts();
            let days = days_raw.clamp(1, 90);
            let bucket_seconds = if days <= 7 {
                3 * 3600
            } else if days <= 30 {
                12 * 3600
            } else {
                24 * 3600
            };
            let total_points: usize = if days <= 7 {
                56
            } else if days <= 30 {
                72
            } else {
                90
            };
            let mut snapshots = Vec::with_capacity(total_points);
            for step in (0..total_points).rev() {
                let step_i64 = step as i64;
                let collected_at = now - step_i64 * bucket_seconds;
                let wave = (step_i64 % 12) - 6;
                let total_resources = (18 + ((step_i64 % 9) as i64) + wave.abs() / 3) as i64;
                let idle_resources = (2 + (step_i64 % 5)) as i64;
                let high_load_resources = if step_i64 % 11 == 0 {
                    4
                } else if step_i64 % 5 == 0 {
                    2
                } else {
                    1
                };
                snapshots.push(db::MonitorSnapshot {
                    collected_at,
                    total_resources,
                    idle_resources,
                    high_load_resources,
                });
            }
            return Ok(snapshots);
        }

        let now = now_unix_ts();
        let mut snapshots = Vec::new();
        for step in (0..24).rev() {
            let step_i64 = step as i64;
            let collected_at = now - step_i64 * 300;
            let total_resources = 11 + (step_i64 % 4);
            let idle_resources = 2 + (step_i64 % 3);
            let high_load_resources = if step_i64 % 6 == 0 {
                2
            } else if step_i64 % 4 == 0 {
                1
            } else {
                0
            };
            snapshots.push(db::MonitorSnapshot {
                collected_at,
                total_resources,
                idle_resources,
                high_load_resources,
            });
        }
        return Ok(snapshots);
    }

    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(days_raw) = window_days {
        db::get_monitor_snapshots_window(&conn, days_raw.clamp(1, 90))
            .await
            .map_err(|e| e.to_string())
    } else {
        db::get_monitor_snapshots(&conn, 24)
            .await
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn confirm_cleanup(
    app_handle: tauri::AppHandle,
    resources: Vec<WastedResource>,
    demo_mode: bool,
) -> Result<db::Stats, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;

    if demo_mode {
        let removed_ids = resources
            .iter()
            .map(|item| item.id.as_str())
            .collect::<HashSet<_>>();
        let remaining = demo_data::generate_demo_data()
            .into_iter()
            .filter(|item| !removed_ids.contains(item.id.as_str()))
            .collect::<Vec<_>>();
        let total_savings = round_two(
            remaining
                .iter()
                .map(|item| item.estimated_monthly_cost)
                .sum::<f64>(),
        );
        let mut stats = demo_data::generate_demo_stats();
        stats.total_savings = total_savings;
        stats.wasted_resource_count = remaining.len() as i64;
        stats.cleanup_count += resources.len() as i64;
        return Ok(stats);
    }

    if matches!(
        read_runtime_plan_type(&app_handle.state::<AppState>().db_path)
            .await
            .as_deref(),
        Some("trial")
    ) {
        return Err(
            "Trial mode cannot execute cleanup actions. Upgrade to Pro to run resource remediation."
                .to_string(),
        );
    }

    let profiles = db::list_cloud_profiles(&conn)
        .await
        .map_err(|e| e.to_string())?;

    for r in resources {
        let mut deleted = false;

        match r.provider.as_str() {
            "AWS" => {
                let region = Region::new(r.region.clone());
                let config = aws_config::from_env().region(region).load().await;

                let ec2 = Ec2Client::new(&config);
                let elb = ElbClient::new(&config);
                let cw = CwClient::new(&config);
                let rds = RdsClient::new(&config);
                let s3 = S3Client::new(&config);
                let scanner = Scanner::new(ec2, elb, cw, rds, s3, r.region.clone(), None, None);

                match r.resource_type.as_str() {
                    "EBS Volume" => {
                        to_str_err(scanner.delete_volume(&r.id).await)?;
                    }
                    "Elastic IP" => {
                        to_str_err(scanner.release_eip(&r.id).await)?;
                    }
                    "EBS Snapshot" => {
                        to_str_err(scanner.delete_snapshot(&r.id).await)?;
                    }
                    "Load Balancer" => {
                        to_str_err(scanner.delete_load_balancer(&r.id).await)?;
                    }
                    "RDS Instance" => {
                        to_str_err(scanner.terminate_rds_instance(&r.id).await)?;
                    }
                    "NAT Gateway" => {
                        to_str_err(scanner.delete_nat_gateway(&r.id).await)?;
                    }
                    "Old AMI" => {
                        to_str_err(scanner.deregister_image(&r.id).await)?;
                    }
                    _ => return Err(format!("Cleanup not supported for {}", r.resource_type)),
                };
                deleted = true;
            }
            "Azure" => {
                if let Some(p) = profiles.iter().find(|p| p.provider == "azure") {
                    if let Ok(creds) = serde_json::from_str::<AzureCreds>(&p.credentials) {
                        let scanner = AzureScanner::new(
                            creds.subscription_id,
                            creds.tenant_id,
                            creds.client_id,
                            creds.client_secret,
                            None,
                        );
                        to_str_err(scanner.delete_resource(&r.id).await)?;
                        deleted = true;
                    }
                }
            }
            "GCP" => {
                if let Some(p) = profiles.iter().find(|p| p.provider == "gcp") {
                    if let Ok(scanner) = GcpScanner::new(&p.credentials) {
                        if r.resource_type == "Persistent Disk" {
                            to_str_err(scanner.delete_disk(&r.region, &r.id).await)?;
                            deleted = true;
                        } else if r.resource_type == "External IP" {
                            to_str_err(scanner.release_address(&r.region, &r.id).await)?;
                            deleted = true;
                        }
                    }
                }
            }
            _ => {}
        }

        if deleted || r.provider == "System" {
            db::record_cleanup(
                &conn,
                &r.id,
                &r.resource_type.to_string(),
                r.estimated_monthly_cost,
            )
            .await
            .map_err(|e| e.to_string())?;

            // Telemetry: Realized Savings
            let meta = serde_json::json!({
                "provider": r.provider,
                "type": r.resource_type,
                "saved": r.estimated_monthly_cost
            });
            report_telemetry(&conn, "app_cleanup", meta).await;
        }
    }
    db::get_stats(&conn).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_connection(
    app_handle: tauri::AppHandle,
    provider: String,
    credentials: String,
    region: Option<String>,
    silent: Option<bool>,
    proxy_profile_id: Option<String>,
) -> Result<String, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    let is_silent = silent.unwrap_or(false);

    let provider_name = provider_label(&provider);
    let (proxy_mode, proxy_url) = resolve_proxy_runtime(&conn, proxy_profile_id.as_deref()).await;
    let _proxy_guard = apply_proxy_env_with_guard(&proxy_mode, &proxy_url).await;
    log_startup_event(&format!(
        "cloud connection test started: provider={} silent={} proxy_mode={} proxy={}",
        provider_name,
        is_silent,
        proxy_mode,
        proxy_endpoint_display(&proxy_mode, &proxy_url)
    ));

    if proxy_mode == "custom" && !proxy_url.trim().is_empty() {
        if let Err(proxy_err) = precheck_proxy_connectivity(proxy_url.trim()).await {
            let err = format_connection_failure_message(
                &provider_name,
                "proxy_connect",
                "proxy_unreachable",
                &proxy_mode,
                &proxy_url,
                &proxy_err,
            );
            log_startup_event(&format!(
                "cloud connection test failed: provider={} silent={} proxy_mode={} proxy={} error=\"{}\"",
                provider_name,
                is_silent,
                proxy_mode,
                proxy_endpoint_display(&proxy_mode, &proxy_url),
                summarize_error_text(&err, 320)
            ));
            return Err(err);
        }
    }

    println!("Testing connection for {}", provider);

    let result = match provider.as_str() {
        "aws" => {
            let creds: serde_json::Value =
                serde_json::from_str(&credentials).map_err(|e| e.to_string())?;
            let key = creds
                .get("key")
                .and_then(|value| value.as_str())
                .or_else(|| creds.get("access_key_id").and_then(|value| value.as_str()))
                .unwrap_or("");
            let secret = creds
                .get("secret")
                .and_then(|value| value.as_str())
                .or_else(|| {
                    creds
                        .get("secret_access_key")
                        .and_then(|value| value.as_str())
                })
                .or_else(|| {
                    creds
                        .get("access_key_secret")
                        .and_then(|value| value.as_str())
                })
                .unwrap_or("");
            let profile_name = creds
                .get("profile")
                .and_then(|value| value.as_str())
                .or_else(|| creds.get("name").and_then(|value| value.as_str()))
                .unwrap_or("")
                .trim()
                .to_string();
            let reg = region
                .or_else(|| extract_aws_region_hint(&credentials))
                .unwrap_or("us-east-1".to_string());

            if key.is_empty() || secret.is_empty() {
                if profile_name.is_empty() {
                    return Err(
                        "Missing Access Key/Secret, and no AWS profile name provided.".into(),
                    );
                }

                let _aws_env_guard = apply_aws_env_with_guard(
                    Some(profile_name.as_str()),
                    None,
                    None,
                    Some(reg.as_str()),
                )
                .await;

                let config = aws_config::load_from_env().await;
                let ec2 = Ec2Client::new(&config);

                match ec2.describe_regions().send().await {
                    Ok(_) => Ok(format!(
                        "AWS Connection Successful! (Profile: {} / Listed Regions)",
                        profile_name
                    )),
                    Err(e) => {
                        let raw = e.to_string();
                        let raw_debug = format!("{:?}", e);
                        let combined = if raw_debug.trim().is_empty() {
                            raw.clone()
                        } else if raw_debug.contains(&raw) {
                            raw_debug.clone()
                        } else {
                            format!("{} | debug={}", raw, raw_debug)
                        };
                        log_startup_event(&format!(
                            "cloud connection aws raw error: profile={} region={} detail=\"{}\"",
                            if profile_name.is_empty() {
                                "-"
                            } else {
                                profile_name.as_str()
                            },
                            reg,
                            summarize_error_text(&combined, 640)
                        ));
                        let (stage, reason_code, detail) =
                            classify_cloud_connectivity_failure(&combined, &proxy_mode);
                        Err(format_connection_failure_message(
                            "AWS",
                            &stage,
                            &reason_code,
                            &proxy_mode,
                            &proxy_url,
                            &detail,
                        ))
                    }
                }
            } else {
                let _aws_env_guard =
                    apply_aws_env_with_guard(None, Some(key), Some(secret), Some(reg.as_str()))
                        .await;

                let config = aws_config::load_from_env().await;
                let ec2 = Ec2Client::new(&config);

                match ec2.describe_regions().send().await {
                    Ok(_) => Ok("AWS Connection Successful! (Listed Regions)".into()),
                    Err(e) => {
                        let raw = e.to_string();
                        let raw_debug = format!("{:?}", e);
                        let combined = if raw_debug.trim().is_empty() {
                            raw.clone()
                        } else if raw_debug.contains(&raw) {
                            raw_debug.clone()
                        } else {
                            format!("{} | debug={}", raw, raw_debug)
                        };
                        log_startup_event(&format!(
                            "cloud connection aws raw error: profile={} region={} detail=\"{}\"",
                            if profile_name.is_empty() {
                                "-"
                            } else {
                                profile_name.as_str()
                            },
                            reg,
                            summarize_error_text(&combined, 640)
                        ));
                        let (stage, reason_code, detail) =
                            classify_cloud_connectivity_failure(&combined, &proxy_mode);
                        Err(format_connection_failure_message(
                            "AWS",
                            &stage,
                            &reason_code,
                            &proxy_mode,
                            &proxy_url,
                            &detail,
                        ))
                    }
                }
            }
        }
        "azure" => {
            if let Ok(c) = serde_json::from_str::<AzureCreds>(&credentials) {
                let scanner = AzureScanner::new(
                    c.subscription_id,
                    c.tenant_id,
                    c.client_id,
                    c.client_secret,
                    None,
                );
                match scanner.scan_public_ips().await {
                    Ok(_) => Ok("Azure Connection Successful!".into()),
                    Err(e) => Err(format!("Azure Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Azure Credentials Format".into())
            }
        }
        "gcp" => {
            if let Ok(scanner) = GcpScanner::new(&credentials) {
                match scanner.scan_addresses().await {
                    Ok(_) => Ok("GCP Connection Successful!".into()),
                    Err(e) => Err(format!("GCP Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid GCP JSON Key".into())
            }
        }
        "alibaba" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let key = c["access_key_id"].as_str().unwrap_or("");
                let secret = c["access_key_secret"].as_str().unwrap_or("");
                let reg = c["region_id"].as_str().unwrap_or("cn-hangzhou");
                let scanner = AlibabaScanner::new(key, secret, reg);
                match scanner.scan_eips().await {
                    Ok(_) => Ok("Alibaba Cloud Connection Successful!".into()),
                    Err(e) => Err(format!("Alibaba Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Alibaba Credentials".into())
            }
        }
        "ibm" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let api_key = c["api_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-south");
                let cos_endpoint = c["cos_endpoint"].as_str().unwrap_or("");
                let cos_service_instance_id = c["cos_service_instance_id"].as_str().unwrap_or("");
                let scanner = ibm::IbmScanner::new(
                    api_key,
                    region,
                    Some(cos_endpoint),
                    Some(cos_service_instance_id),
                );
                match scanner.check_cos_access_summary().await {
                    Ok(summary) => Ok(format!("IBM Cloud Connection Successful! {}", summary)),
                    Err(e) => Err(format!("IBM Cloud Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid IBM Cloud Credentials".into())
            }
        }
        "oracle" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let tenancy_id = c["tenancy_id"].as_str().unwrap_or("");
                let user_id = c["user_id"].as_str().unwrap_or("");
                let fingerprint = c["fingerprint"].as_str().unwrap_or("");
                let private_key = c["private_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-ashburn-1");
                let scanner =
                    OracleScanner::new(tenancy_id, user_id, fingerprint, private_key, region);
                match scanner.scan_instances().await {
                    Ok(_) => Ok("Oracle Cloud Connection Successful!".into()),
                    Err(e) => Err(format!("Oracle Cloud Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Oracle Credentials".into())
            }
        }
        "huawei" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("cn-north-4");
                let project_id = c["project_id"].as_str().unwrap_or("");
                let scanner =
                    huawei::HuaweiScanner::new(access_key, secret_key, region, project_id);
                match scanner.scan_eips().await {
                    Ok(_) => Ok("Huawei Cloud Connection Successful!".into()),
                    Err(e) => Err(format!("Huawei Cloud Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Huawei Credentials".into())
            }
        }
        "tencent" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let secret_id = c["secret_id"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("ap-guangzhou");
                let scanner = tencent::TencentScanner::new(secret_id, secret_key, region);
                match scanner.scan_eips().await {
                    Ok(_) => Ok("Tencent Cloud Connection Successful!".into()),
                    Err(e) => Err(format!("Tencent Cloud Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Tencent Credentials".into())
            }
        }
        "volcengine" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("cn-beijing");
                let scanner = volcengine::VolcengineScanner::new(access_key, secret_key, region);
                match scanner.scan_eips().await {
                    Ok(_) => Ok("Volcengine Connection Successful!".into()),
                    Err(e) => Err(format!("Volcengine Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Volcengine Credentials".into())
            }
        }
        "baidu" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("bj");
                let scanner = baidu::BaiduScanner::new(access_key, secret_key, region);
                match scanner.scan_eips().await {
                    Ok(_) => Ok("Baidu AI Cloud Connection Successful!".into()),
                    Err(e) => Err(format!("Baidu AI Cloud Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Baidu Credentials".into())
            }
        }
        "tianyi" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("cn-east-1");
                let scanner = tianyi::TianyiScanner::new(access_key, secret_key, region);
                match scanner.scan_host().await {
                    Ok(_) => Ok("Tianyi Cloud Connection Successful!".into()),
                    Err(e) => Err(format!("Tianyi Cloud Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Tianyi Credentials".into())
            }
        }
        "cloudflare" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let token = c["token"].as_str().unwrap_or("");
                let account_id = c["account_id"].as_str().unwrap_or("");
                let scanner = cloudflare::CloudflareScanner::new(token, account_id);
                match scanner.scan_dns().await {
                    Ok(_) => Ok("Cloudflare Connection Successful!".into()),
                    Err(e) => Err(format!("Cloudflare Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Cloudflare Credentials".into())
            }
        }
        "linode" => {
            let token = if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                c["token"].as_str().unwrap_or("").to_string()
            } else {
                credentials.trim().to_string()
            };
            let scanner = linode::LinodeScanner::new(&token);
            match scanner.scan_instances().await {
                Ok(_) => Ok("Linode Connection Successful!".into()),
                Err(e) => Err(format!("Linode Auth Failed: {}", e)),
            }
        }
        "vultr" => {
            let api_key = if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                c["api_key"].as_str().unwrap_or("").to_string()
            } else {
                credentials.trim().to_string()
            };
            let scanner = VultrScanner::new(&api_key);
            match scanner.scan_instances().await {
                Ok(_) => Ok("Vultr Connection Successful!".into()),
                Err(e) => Err(format!("Vultr Auth Failed: {}", e)),
            }
        }
        "digitalocean" => {
            let token = credentials;
            if cloud_waste_scanner_core::digitalocean::check_auth(&token).await {
                Ok("DigitalOcean Connection Successful!".into())
            } else {
                Err("DigitalOcean Auth Failed".into())
            }
        }
        "akamai" => {
            let token = if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                c["token"].as_str().unwrap_or("").to_string()
            } else {
                credentials.trim().to_string()
            };
            let scanner = akamai::AkamaiScanner::new(&token);
            match scanner.check_auth().await {
                Ok(_) => Ok("Akamai Connection Successful!".into()),
                Err(e) => Err(format!("Akamai Auth Failed: {}", e)),
            }
        }
        "equinix" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let token = c["token"].as_str().unwrap_or("");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let project_id = c["project_id"].as_str().unwrap_or("");
                let scanner = equinix::EquinixScanner::new(token, endpoint, project_id);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Equinix Metal Connection Successful!".into()),
                    Err(e) => Err(format!("Equinix Metal Auth Failed: {}", e)),
                }
            } else {
                let scanner = equinix::EquinixScanner::new(&credentials, "", "");
                match scanner.check_auth().await {
                    Ok(_) => Ok("Equinix Metal Connection Successful!".into()),
                    Err(e) => Err(format!("Equinix Metal Auth Failed: {}", e)),
                }
            }
        }
        "rackspace" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let token = c["token"].as_str().unwrap_or("");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let project_id = c["project_id"].as_str().unwrap_or("");
                let scanner = rackspace::RackspaceScanner::new(token, endpoint, project_id);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Rackspace Connection Successful!".into()),
                    Err(e) => Err(format!("Rackspace Auth Failed: {}", e)),
                }
            } else {
                let scanner = rackspace::RackspaceScanner::new(&credentials, "", "");
                match scanner.check_auth().await {
                    Ok(_) => Ok("Rackspace Connection Successful!".into()),
                    Err(e) => Err(format!("Rackspace Auth Failed: {}", e)),
                }
            }
        }
        "openstack" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let token = c["token"].as_str().unwrap_or("");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let project_id = c["project_id"].as_str().unwrap_or("");
                let scanner = openstack::OpenstackScanner::new(token, endpoint, project_id);
                match scanner.check_auth().await {
                    Ok(_) => Ok("OpenStack Connection Successful!".into()),
                    Err(e) => Err(format!("OpenStack Auth Failed: {}", e)),
                }
            } else {
                let scanner = openstack::OpenstackScanner::new(&credentials, "", "");
                match scanner.check_auth().await {
                    Ok(_) => Ok("OpenStack Connection Successful!".into()),
                    Err(e) => Err(format!("OpenStack Auth Failed: {}", e)),
                }
            }
        }
        "wasabi" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = wasabi::WasabiScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Wasabi Connection Successful!".into()),
                    Err(e) => Err(format!("Wasabi Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Wasabi Credentials".into())
            }
        }
        "backblaze" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-west-004");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner =
                    backblaze::BackblazeScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Backblaze B2 Connection Successful!".into()),
                    Err(e) => Err(format!("Backblaze B2 Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Backblaze Credentials".into())
            }
        }
        "idrive" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = idrive::IdriveScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("IDrive e2 Connection Successful!".into()),
                    Err(e) => Err(format!("IDrive e2 Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid IDrive e2 Credentials".into())
            }
        }
        "storj" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = storj::StorjScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Storj DCS Connection Successful!".into()),
                    Err(e) => Err(format!("Storj DCS Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Storj DCS Credentials".into())
            }
        }
        "dreamhost" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner =
                    dreamhost::DreamhostScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("DreamHost Connection Successful!".into()),
                    Err(e) => Err(format!("DreamHost Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid DreamHost Credentials".into())
            }
        }
        "cloudian" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner =
                    cloudian::CloudianScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Cloudian Connection Successful!".into()),
                    Err(e) => Err(format!("Cloudian Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Cloudian Credentials".into())
            }
        }
        "s3compatible" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner =
                    generic_s3::GenericS3Scanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Generic S3-Compatible Connection Successful!".into()),
                    Err(e) => Err(format!("Generic S3-Compatible Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Generic S3-Compatible Credentials".into())
            }
        }
        "minio" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = minio::MinioScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("MinIO Connection Successful!".into()),
                    Err(e) => Err(format!("MinIO Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid MinIO Credentials".into())
            }
        }
        "ceph" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = ceph::CephScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Ceph RGW Connection Successful!".into()),
                    Err(e) => Err(format!("Ceph RGW Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Ceph RGW Credentials".into())
            }
        }
        "lyve" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-west-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = lyve::LyveScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Lyve Cloud Connection Successful!".into()),
                    Err(e) => Err(format!("Lyve Cloud Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Lyve Cloud Credentials".into())
            }
        }
        "dell" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = dell::DellScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Dell ECS Connection Successful!".into()),
                    Err(e) => Err(format!("Dell ECS Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Dell ECS Credentials".into())
            }
        }
        "storagegrid" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner =
                    storagegrid::StoragegridScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("StorageGRID Connection Successful!".into()),
                    Err(e) => Err(format!("StorageGRID Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid StorageGRID Credentials".into())
            }
        }
        "scality" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner =
                    scality::ScalityScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Scality Connection Successful!".into()),
                    Err(e) => Err(format!("Scality Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Scality Credentials".into())
            }
        }
        "hcp" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = hcp::HcpScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Hitachi HCP Connection Successful!".into()),
                    Err(e) => Err(format!("Hitachi HCP Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Hitachi HCP Credentials".into())
            }
        }
        "qumulo" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = qumulo::QumuloScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Qumulo Connection Successful!".into()),
                    Err(e) => Err(format!("Qumulo Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Qumulo Credentials".into())
            }
        }
        "nutanix" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner =
                    nutanix::NutanixScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Nutanix Objects Connection Successful!".into()),
                    Err(e) => Err(format!("Nutanix Objects Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Nutanix Objects Credentials".into())
            }
        }
        "flashblade" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner =
                    flashblade::FlashbladeScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Pure Storage FlashBlade Connection Successful!".into()),
                    Err(e) => Err(format!("Pure Storage FlashBlade Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid FlashBlade Credentials".into())
            }
        }

        "greenlake" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let access_key = c["access_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let region = c["region"].as_str().unwrap_or("us-east-1");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner =
                    greenlake::GreenlakeScanner::new(access_key, secret_key, region, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("HPE GreenLake Connection Successful!".into()),
                    Err(e) => Err(format!("HPE GreenLake Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid HPE GreenLake Credentials".into())
            }
        }
        "ovh" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let app_key = c["application_key"].as_str().unwrap_or("");
                let app_secret = c["application_secret"].as_str().unwrap_or("");
                let consumer_key = c["consumer_key"].as_str().unwrap_or("");
                let endpoint = c["endpoint"].as_str().unwrap_or("eu");
                let project_id = c["project_id"].as_str().unwrap_or("");
                let scanner =
                    ovh::OvhScanner::new(app_key, app_secret, consumer_key, endpoint, project_id);
                match scanner.check_connection().await {
                    Ok(found_project) => Ok(format!(
                        "OVHcloud Connection Successful! (project: {})",
                        found_project
                    )),
                    Err(e) => Err(format!("OVHcloud Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid OVHcloud Credentials".into())
            }
        }
        "hetzner" => {
            let token = credentials;
            let scanner = hetzner::HetznerScanner::new(&token);
            match scanner.check_auth().await {
                Ok(_) => Ok("Hetzner Connection Successful!".into()),
                Err(e) => Err(format!("Hetzner Auth Failed: {}", e)),
            }
        }
        "scaleway" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let token = c["token"].as_str().unwrap_or("");
                let zones = c["zones"].as_str().unwrap_or("");
                let scanner = scaleway::ScalewayScanner::new(token, zones);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Scaleway Connection Successful!".into()),
                    Err(e) => Err(format!("Scaleway Auth Failed: {}", e)),
                }
            } else {
                let scanner = scaleway::ScalewayScanner::new(&credentials, "");
                match scanner.check_auth().await {
                    Ok(_) => Ok("Scaleway Connection Successful!".into()),
                    Err(e) => Err(format!("Scaleway Auth Failed: {}", e)),
                }
            }
        }
        "civo" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let token = c["token"].as_str().unwrap_or("");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = civo::CivoScanner::new(token, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Civo Connection Successful!".into()),
                    Err(e) => Err(format!("Civo Auth Failed: {}", e)),
                }
            } else {
                let scanner = civo::CivoScanner::new(&credentials, "");
                match scanner.check_auth().await {
                    Ok(_) => Ok("Civo Connection Successful!".into()),
                    Err(e) => Err(format!("Civo Auth Failed: {}", e)),
                }
            }
        }
        "contabo" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let token = c["token"].as_str().unwrap_or("");
                let client_id = c["client_id"].as_str().unwrap_or("");
                let client_secret = c["client_secret"].as_str().unwrap_or("");
                let username = c["username"].as_str().unwrap_or("");
                let password = c["password"].as_str().unwrap_or("");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = contabo::ContaboScanner::new(
                    token,
                    client_id,
                    client_secret,
                    username,
                    password,
                    endpoint,
                );
                match scanner.check_auth().await {
                    Ok(_) => Ok("Contabo Connection Successful!".into()),
                    Err(e) => Err(format!("Contabo Auth Failed: {}", e)),
                }
            } else {
                let scanner = contabo::ContaboScanner::new(&credentials, "", "", "", "", "");
                match scanner.check_auth().await {
                    Ok(_) => Ok("Contabo Connection Successful!".into()),
                    Err(e) => Err(format!("Contabo Auth Failed: {}", e)),
                }
            }
        }
        "gcore" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let token = c["token"].as_str().unwrap_or("");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = gcore::GcoreScanner::new(token, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Gcore Connection Successful!".into()),
                    Err(e) => Err(format!("Gcore Auth Failed: {}", e)),
                }
            } else {
                let scanner = gcore::GcoreScanner::new(&credentials, "");
                match scanner.check_auth().await {
                    Ok(_) => Ok("Gcore Connection Successful!".into()),
                    Err(e) => Err(format!("Gcore Auth Failed: {}", e)),
                }
            }
        }
        "upcloud" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let username = c["username"].as_str().unwrap_or("");
                let password = c["password"].as_str().unwrap_or("");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = upcloud::UpcloudScanner::new(username, password, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("UpCloud Connection Successful!".into()),
                    Err(e) => Err(format!("UpCloud Auth Failed: {}", e)),
                }
            } else {
                let mut parts = credentials.splitn(2, ':');
                let username = parts.next().unwrap_or("");
                let password = parts.next().unwrap_or("");
                let scanner = upcloud::UpcloudScanner::new(username, password, "");
                match scanner.check_auth().await {
                    Ok(_) => Ok("UpCloud Connection Successful!".into()),
                    Err(e) => Err(format!("UpCloud Auth Failed: {}", e)),
                }
            }
        }
        "leaseweb" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let token = c["token"].as_str().unwrap_or("");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = leaseweb::LeasewebScanner::new(token, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Leaseweb Connection Successful!".into()),
                    Err(e) => Err(format!("Leaseweb Auth Failed: {}", e)),
                }
            } else {
                let scanner = leaseweb::LeasewebScanner::new(&credentials, "");
                match scanner.check_auth().await {
                    Ok(_) => Ok("Leaseweb Connection Successful!".into()),
                    Err(e) => Err(format!("Leaseweb Auth Failed: {}", e)),
                }
            }
        }
        "exoscale" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let api_key = c["api_key"].as_str().unwrap_or("");
                let secret_key = c["secret_key"].as_str().unwrap_or("");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = exoscale::ExoscaleScanner::new(api_key, secret_key, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("Exoscale Connection Successful!".into()),
                    Err(e) => Err(format!("Exoscale Auth Failed: {}", e)),
                }
            } else {
                Err("Invalid Exoscale Credentials".into())
            }
        }
        "ionos" => {
            if let Ok(c) = serde_json::from_str::<serde_json::Value>(&credentials) {
                let token = c["token"].as_str().unwrap_or("");
                let endpoint = c["endpoint"].as_str().unwrap_or("");
                let scanner = ionos::IonosScanner::new(token, endpoint);
                match scanner.check_auth().await {
                    Ok(_) => Ok("IONOS Connection Successful!".into()),
                    Err(e) => Err(format!("IONOS Auth Failed: {}", e)),
                }
            } else {
                let scanner = ionos::IonosScanner::new(&credentials, "");
                match scanner.check_auth().await {
                    Ok(_) => Ok("IONOS Connection Successful!".into()),
                    Err(e) => Err(format!("IONOS Auth Failed: {}", e)),
                }
            }
        }
        _ => {
            if credentials.len() < 10 {
                Err("Credentials seem too short".into())
            } else {
                Ok(format!(
                    "{} format looks valid (Deep verify not implemented yet)",
                    provider
                ))
            }
        }
    };

    match &result {
        Ok(message) => {
            log_startup_event(&format!(
                "cloud connection test success: provider={} silent={} proxy_mode={} proxy={} message=\"{}\"",
                provider_name,
                is_silent,
                proxy_mode,
                proxy_endpoint_display(&proxy_mode, &proxy_url),
                summarize_error_text(message, 220)
            ));
        }
        Err(err) => {
            log_startup_event(&format!(
                "cloud connection test failed: provider={} silent={} proxy_mode={} proxy={} error=\"{}\"",
                provider_name,
                is_silent,
                proxy_mode,
                proxy_endpoint_display(&proxy_mode, &proxy_url),
                summarize_error_text(err, 320)
            ));
        }
    }

    if !is_silent {
        if let Err(ref e) = result {
            report_telemetry(
                &conn,
                "app_test_connection_error",
                serde_json::json!({
                    "provider": provider,
                    "error": e
                }),
            )
            .await;
        }
    }

    result
}

async fn load_pending_consume_events(pool: &sqlx::Pool<sqlx::Sqlite>) -> Vec<PendingConsumeEvent> {
    let raw = db::get_setting(pool, PENDING_CONSUME_SETTING_KEY)
        .await
        .unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str(&raw).unwrap_or_default()
}

async fn persist_pending_consume_events(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    events: &[PendingConsumeEvent],
) -> Result<(), String> {
    let payload = serde_json::to_string(events).map_err(|e| e.to_string())?;
    db::save_setting(pool, PENDING_CONSUME_SETTING_KEY, &payload).await
}

async fn try_consume_license_on_server(
    _key: &str,
    _machine_id: &str,
    _timeout_secs: u64,
) -> Result<(), String> {
    Ok(())
}

async fn enqueue_pending_consume_event(
    app_state: &AppState,
    pool: &sqlx::Pool<sqlx::Sqlite>,
    key: &str,
    machine_id: &str,
) -> Result<(), String> {
    let _guard = app_state.pending_consume_lock.lock().await;
    let mut events = load_pending_consume_events(pool).await;

    if let Some(existing) = events
        .iter_mut()
        .find(|evt| evt.license_key == key && evt.machine_id == machine_id)
    {
        existing.pending_scans = existing.pending_scans.saturating_add(1);
    } else {
        events.push(PendingConsumeEvent {
            license_key: key.to_string(),
            machine_id: machine_id.to_string(),
            created_at: now_unix_ts(),
            pending_scans: 1,
            attempts: 0,
        });
    }

    events.sort_by_key(|evt| evt.created_at);
    while events.len() > MAX_PENDING_CONSUME_EVENTS {
        let _ = events.remove(0);
    }

    persist_pending_consume_events(pool, &events).await
}

async fn flush_pending_consume_events(
    app_state: &AppState,
    pool: &sqlx::Pool<sqlx::Sqlite>,
    timeout_secs: u64,
) {
    let _guard = app_state.pending_consume_lock.lock().await;
    let events = load_pending_consume_events(pool).await;
    if events.is_empty() {
        return;
    }

    let mut remaining: Vec<PendingConsumeEvent> = Vec::new();
    let mut flushed_scans = 0u32;
    let mut consume_attempts = 0u32;

    for mut event in events {
        let mut outstanding = event.pending_scans.max(1);
        while outstanding > 0 {
            if consume_attempts >= MAX_PENDING_CONSUME_FLUSH_PER_PASS {
                break;
            }
            consume_attempts = consume_attempts.saturating_add(1);

            match try_consume_license_on_server(&event.license_key, &event.machine_id, timeout_secs)
                .await
            {
                Ok(_) => {
                    flushed_scans = flushed_scans.saturating_add(1);
                    outstanding = outstanding.saturating_sub(1);
                }
                Err(err) => {
                    event.attempts = event.attempts.saturating_add(1);
                    eprintln!(
                        "Pending consume retry failed (attempts={} key={}): {}",
                        event.attempts,
                        event.license_key.chars().take(8).collect::<String>(),
                        summarize_error_text(&err, 220)
                    );
                    break;
                }
            }
        }

        if outstanding > 0 {
            event.pending_scans = outstanding;
            remaining.push(event);
        }
    }

    let _ = persist_pending_consume_events(pool, &remaining).await;
    if flushed_scans > 0 {
        println!(
            "DEBUG: Flushed {} pending license consumption scan(s) across {} attempt(s).",
            flushed_scans, consume_attempts
        );
    }
}

async fn consume_license_on_server(
    _key: &str,
    _machine_id: &str,
    _timeout_secs: u64,
) -> Result<(), String> {
    Ok(())
}

async fn report_telemetry(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    event: &str,
    mut meta: serde_json::Value,
) {
    if let Ok(mid) = db::get_or_create_machine_id(pool).await {
        if let Some(obj) = meta.as_object_mut() {
            obj.insert("machine_id".to_string(), serde_json::json!(mid));
            obj.insert("os".to_string(), serde_json::json!(std::env::consts::OS));
            obj.insert(
                "version".to_string(),
                serde_json::json!(env!("CARGO_PKG_VERSION")),
            );
        }
    }
    let compact = summarize_error_text(&meta.to_string(), 240);
    let _ = db::record_audit_log(pool, "TELEMETRY", event, &compact).await;
}

#[tauri::command]
async fn list_notification_channels(
    app_handle: tauri::AppHandle,
) -> Result<Vec<NotificationChannel>, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::list_notification_channels(&conn)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_notification_channel(
    app_handle: tauri::AppHandle,
    channel: NotificationChannel,
) -> Result<(), String> {
    let channel = normalize_channel_for_save(channel)?;
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::save_notification_channel(&conn, &channel)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_notification_channel(
    app_handle: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::delete_notification_channel(&conn, &id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_notification_channel(
    app_handle: tauri::AppHandle,
    channel: NotificationChannel,
) -> NotificationTestDiagnostics {
    use cloud_waste_scanner_core::notify;

    let started = std::time::Instant::now();
    let tested_at = now_unix_ts();
    let app_version = app_handle.package_info().version.to_string();
    let channel_name = channel.name.clone();
    let channel_method = channel.method.clone();
    let mut trace: Vec<String> = Vec::new();
    push_notification_trace(
        &mut trace,
        &started,
        format!(
            "notification test started: method={} channel={} app_version={} diag_build=notif_trace_v1",
            channel_method, channel_name, app_version
        ),
    );

    let app_state = app_handle.state::<AppState>();
    let conn = match db::init_db(&app_state.db_path).await {
        Ok(pool) => pool,
        Err(err) => {
            let diag = NotificationTestDiagnostics {
                ok: false,
                channel_name: channel_name.clone(),
                channel_method: channel_method.clone(),
                app_version: app_version.clone(),
                proxy_mode: "none".to_string(),
                proxy_profile_id: None,
                proxy_scheme: None,
                proxy_url_masked: None,
                stage: "internal".to_string(),
                reason_code: "db_init_failed".to_string(),
                message: format!("Failed to initialize local database: {}", err),
                http_status: None,
                duration_ms: started.elapsed().as_millis() as u64,
                tested_at,
                trace,
            };
            log_startup_event(&format!(
                "Notification test failed before DB init: channel_name=\"{}\" method={} stage={} reason={} message=\"{}\"",
                diag.channel_name, diag.channel_method, diag.stage, diag.reason_code, diag.message
            ));
            for line in &diag.trace {
                log_startup_event(&format!(
                    "Notification test trace: channel_name=\"{}\" {}",
                    diag.channel_name, line
                ));
            }
            return diag;
        }
    };
    push_notification_trace(&mut trace, &started, "local database initialized");

    let (proxy_mode, resolved_proxy_url) =
        resolve_proxy_runtime(&conn, channel.proxy_profile_id.as_deref()).await;
    let proxy_url = if proxy_mode == "custom" {
        normalize_custom_proxy_url(&resolved_proxy_url)
    } else {
        resolved_proxy_url
    };
    let proxy_scheme = proxy_scheme_from_url(&proxy_url);
    push_notification_trace(
        &mut trace,
        &started,
        format!(
            "proxy resolved: mode={} profile={} endpoint={} scheme={}",
            proxy_mode,
            channel
                .proxy_profile_id
                .as_deref()
                .unwrap_or(PROXY_CHOICE_GLOBAL),
            if proxy_url.trim().is_empty() {
                "-".to_string()
            } else {
                mask_proxy_url(&proxy_url)
            },
            proxy_scheme
                .as_deref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "-".to_string()),
        ),
    );

    let _proxy_guard = apply_proxy_env_with_guard(&proxy_mode, &proxy_url).await;
    push_notification_trace(&mut trace, &started, "proxy environment guard applied");

    let proxy_url_masked = if proxy_mode == "custom" && !proxy_url.trim().is_empty() {
        Some(mask_proxy_url(&proxy_url))
    } else {
        None
    };

    if proxy_mode == "custom" && !proxy_url.trim().is_empty() {
        push_notification_trace(&mut trace, &started, "prechecking proxy TCP reachability");
        if let Err(proxy_err) = precheck_proxy_connectivity(proxy_url.trim()).await {
            push_notification_trace(
                &mut trace,
                &started,
                format!("proxy precheck failed: {}", proxy_err),
            );
            let diag = NotificationTestDiagnostics {
                ok: false,
                channel_name,
                channel_method,
                app_version,
                proxy_mode,
                proxy_profile_id: channel.proxy_profile_id.clone(),
                proxy_scheme,
                proxy_url_masked,
                stage: "proxy_connect".to_string(),
                reason_code: "proxy_unreachable".to_string(),
                message: proxy_err,
                http_status: None,
                duration_ms: started.elapsed().as_millis() as u64,
                tested_at,
                trace,
            };
            record_notification_test_audit(&conn, &channel, &diag).await;
            return diag;
        }
        push_notification_trace(&mut trace, &started, "proxy precheck passed");
    }

    let explicit_proxy_url = if proxy_mode == "custom" && !proxy_url.trim().is_empty() {
        Some(proxy_url.trim())
    } else {
        None
    };
    push_notification_trace(
        &mut trace,
        &started,
        format!(
            "effective proxy for request: {}",
            explicit_proxy_url
                .map(mask_proxy_url)
                .unwrap_or_else(|| "-".to_string())
        ),
    );

    if let Some(probe_url) = extract_notification_probe_url(&channel) {
        let probe_origin = render_probe_origin(&probe_url);
        let probe_target = extract_probe_host_port(&probe_url);
        if let (Some((target_host, target_port)), Some(scheme)) =
            (probe_target.as_ref(), proxy_scheme.as_deref())
        {
            if proxy_mode == "custom" && (scheme == "socks5" || scheme == "socks5h") {
                push_notification_trace(
                    &mut trace,
                    &started,
                    format!(
                        "running SOCKS tunnel probe: scheme={} target={}:{}",
                        scheme, target_host, target_port
                    ),
                );
                match socks5_tunnel_probe(
                    explicit_proxy_url.unwrap_or_default(),
                    target_host,
                    *target_port,
                )
                .await
                {
                    Ok(_) => {
                        push_notification_trace(
                            &mut trace,
                            &started,
                            "SOCKS tunnel probe succeeded",
                        );
                    }
                    Err(err) => {
                        push_notification_trace(
                            &mut trace,
                            &started,
                            format!("SOCKS tunnel probe failed: {}", err),
                        );
                    }
                }
            }
        }
        push_notification_trace(
            &mut trace,
            &started,
            format!("probing target origin reachability: {}", probe_origin),
        );
        match probe_notification_target(
            &proxy_mode,
            explicit_proxy_url.unwrap_or_default(),
            &probe_url,
        )
        .await
        {
            Ok(code) => {
                push_notification_trace(
                    &mut trace,
                    &started,
                    format!("target origin probe responded with HTTP {}", code),
                );
            }
            Err(err) => {
                push_notification_trace(
                    &mut trace,
                    &started,
                    format!("target origin probe failed: {}", err),
                );
            }
        }
    }

    push_notification_trace(
        &mut trace,
        &started,
        "sending notification payload with configured proxy route",
    );

    let diag = if channel.method.eq_ignore_ascii_case("email") {
        let recipients = parse_notification_channel_email_recipients(&channel.config);
        if recipients.is_empty() {
            push_notification_trace(
                &mut trace,
                &started,
                "email notification test aborted: no valid recipients parsed from config",
            );
            NotificationTestDiagnostics {
                ok: false,
                channel_name,
                channel_method,
                app_version,
                proxy_mode,
                proxy_profile_id: channel.proxy_profile_id.clone(),
                proxy_scheme,
                proxy_url_masked,
                stage: "validation".to_string(),
                reason_code: "missing_email_recipients".to_string(),
                message: "Email channel requires at least one valid recipient address.".to_string(),
                http_status: None,
                duration_ms: started.elapsed().as_millis() as u64,
                tested_at,
                trace,
            }
        } else {
            let effective_license_key = match resolve_effective_license_key(&app_handle) {
                Ok(value) => value,
                Err(err) => {
                    push_notification_trace(
                        &mut trace,
                        &started,
                        format!(
                            "email notification test aborted: failed to resolve license key ({})",
                            compact_scan_error(&err)
                        ),
                    );
                    let diag = NotificationTestDiagnostics {
                        ok: false,
                        channel_name,
                        channel_method,
                        app_version,
                        proxy_mode,
                        proxy_profile_id: channel.proxy_profile_id.clone(),
                        proxy_scheme,
                        proxy_url_masked,
                        stage: "license".to_string(),
                        reason_code: "license_unavailable".to_string(),
                        message: format!("Failed to resolve active license key: {}", err),
                        http_status: None,
                        duration_ms: started.elapsed().as_millis() as u64,
                        tested_at,
                        trace,
                    };
                    record_notification_test_audit(&conn, &channel, &diag).await;
                    return diag;
                }
            };
            let test_scan_ref = format!("notification-test-{}", tested_at);
            let test_results: Vec<WastedResource> = Vec::new();
            push_notification_trace(
                &mut trace,
                &started,
                format!(
                    "email notification test dispatch start: recipients={}",
                    recipients.len()
                ),
            );
            match send_scan_report_email(
                &effective_license_key,
                &recipients,
                &test_scan_ref,
                &test_results,
            )
            .await
            {
                Ok(_) => {
                    push_notification_trace(
                        &mut trace,
                        &started,
                        "email notification test dispatch accepted",
                    );
                    NotificationTestDiagnostics {
                        ok: true,
                        channel_name,
                        channel_method,
                        app_version,
                        proxy_mode,
                        proxy_profile_id: channel.proxy_profile_id.clone(),
                        proxy_scheme,
                        proxy_url_masked,
                        stage: "delivered".to_string(),
                        reason_code: "ok".to_string(),
                        message: format!(
                            "Email notification request accepted for {} recipient(s).",
                            recipients.len()
                        ),
                        http_status: None,
                        duration_ms: started.elapsed().as_millis() as u64,
                        tested_at,
                        trace,
                    }
                }
                Err(err) => {
                    let raw = err.to_string();
                    let (stage, reason_code, http_status, message) =
                        classify_notification_test_failure(&raw, &proxy_mode);
                    push_notification_trace(
                        &mut trace,
                        &started,
                        format!(
                            "email notification test failed: stage={} reason={} raw={}",
                            stage,
                            reason_code,
                            compact_scan_error(&raw)
                        ),
                    );
                    NotificationTestDiagnostics {
                        ok: false,
                        channel_name,
                        channel_method,
                        app_version,
                        proxy_mode,
                        proxy_profile_id: channel.proxy_profile_id.clone(),
                        proxy_scheme,
                        proxy_url_masked,
                        stage,
                        reason_code,
                        message,
                        http_status,
                        duration_ms: started.elapsed().as_millis() as u64,
                        tested_at,
                        trace,
                    }
                }
            }
        }
    } else {
        let mut dispatch_channel = channel.clone();
        if dispatch_channel.method.eq_ignore_ascii_case("custom") {
            dispatch_channel.method = "webhook".to_string();
            push_notification_trace(
                &mut trace,
                &started,
                "notification test method alias normalized: custom->webhook",
            );
        }
        match notify::send_notification_with_proxy(
            &dispatch_channel,
            "🔔 Cloud Waste Scanner: Test Notification Successful!",
            explicit_proxy_url,
        )
        .await
        {
            Ok(status_code) => {
                push_notification_trace(
                    &mut trace,
                    &started,
                    format!("delivery completed with HTTP {}", status_code),
                );
                NotificationTestDiagnostics {
                    ok: true,
                    channel_name,
                    channel_method,
                    app_version,
                    proxy_mode,
                    proxy_profile_id: channel.proxy_profile_id.clone(),
                    proxy_scheme,
                    proxy_url_masked,
                    stage: "delivered".to_string(),
                    reason_code: "ok".to_string(),
                    message: format!(
                        "Notification accepted by remote endpoint (HTTP {}).",
                        status_code
                    ),
                    http_status: Some(status_code),
                    duration_ms: started.elapsed().as_millis() as u64,
                    tested_at,
                    trace,
                }
            }
            Err(err) => {
                let raw = err.to_string();
                let (stage, reason_code, http_status, message) =
                    classify_notification_test_failure(&raw, &proxy_mode);
                push_notification_trace(
                    &mut trace,
                    &started,
                    format!(
                        "delivery failed: stage={} reason={} raw={}",
                        stage, reason_code, raw
                    ),
                );
                NotificationTestDiagnostics {
                    ok: false,
                    channel_name,
                    channel_method,
                    app_version,
                    proxy_mode,
                    proxy_profile_id: channel.proxy_profile_id.clone(),
                    proxy_scheme,
                    proxy_url_masked,
                    stage,
                    reason_code,
                    message,
                    http_status,
                    duration_ms: started.elapsed().as_millis() as u64,
                    tested_at,
                    trace,
                }
            }
        }
    };

    record_notification_test_audit(&conn, &channel, &diag).await;
    diag
}

#[tauri::command]
async fn get_scan_history(
    app_handle: tauri::AppHandle,
) -> Result<Vec<db::ScanHistoryItem>, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;

    if matches!(
        read_runtime_plan_type(&app_handle.state::<AppState>().db_path)
            .await
            .as_deref(),
        Some("trial")
    ) {
        // Self-heal stale local plan cache after subscription upgrades.
        let key = load_license_file(app_handle.clone()).unwrap_or_default();
        if !key.trim().is_empty() {
            let machine_id = db::get_or_create_machine_id(&conn).await.ok();
            if let Ok(status) =
                license::check_online_status(key.trim(), machine_id.as_deref()).await
            {
                persist_runtime_plan_type_from_status(&conn, &status).await;
                let still_trial = status.valid
                    && status
                        .is_trial
                        .unwrap_or_else(|| matches!(status.plan_type.as_deref(), Some("trial")));
                if !still_trial {
                    return db::get_scan_history(&conn).await.map_err(|e| e.to_string());
                }
            }
        }

        return Err(
            "Trial mode does not include historical detailed findings. Upgrade to Pro to unlock history details."
                .to_string(),
        );
    }

    db::get_scan_history(&conn).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_ai_analyst_summary(
    app_handle: tauri::AppHandle,
    window_days: Option<i64>,
) -> Result<AiAnalystSummary, String> {
    let window_days = window_days.unwrap_or(30).clamp(1, 365);
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;

    let history = db::get_scan_history(&conn)
        .await
        .map_err(|e| e.to_string())?;
    let completed_in_window: Vec<&db::ScanHistoryItem> = history
        .iter()
        .filter(|item| {
            item.status.eq_ignore_ascii_case("completed")
                && item.scanned_at >= Utc::now().timestamp() - (window_days * 86_400)
        })
        .collect();
    let previous_scan_id = completed_in_window.get(1).map(|item| item.id);
    let previous_scan_at = completed_in_window.get(1).map(|item| item.scanned_at);
    let previous_attribution = completed_in_window
        .get(1)
        .map(|item| extract_scan_meta_attribution(item.scan_meta.as_deref()));
    let previous_resources = completed_in_window
        .get(1)
        .and_then(|item| serde_json::from_str::<Vec<WastedResource>>(&item.results_json).ok());
    let scan_count_in_window = history
        .iter()
        .filter(|item| {
            item.status.eq_ignore_ascii_case("completed")
                && item.scanned_at >= Utc::now().timestamp() - (window_days * 86_400)
        })
        .count();
    let (basis, latest_scan_id, latest_scan_at, resources, result_attribution, notes) =
        latest_ai_source_for_window(history, window_days);

    if latest_scan_id.is_some() {
        let scanned_accounts = if let Some(scan_id) = latest_scan_id {
            let history_rows = db::get_scan_history(&conn)
                .await
                .map_err(|e| e.to_string())?;
            history_rows
                .iter()
                .find(|item| item.id == scan_id)
                .map(|item| extract_scan_meta_accounts(item.scan_meta.as_deref()))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        return Ok(compute_ai_analyst_summary(
            window_days,
            &basis,
            latest_scan_id,
            latest_scan_at,
            previous_scan_id,
            previous_scan_at,
            previous_resources.as_deref(),
            scan_count_in_window,
            scanned_accounts,
            resources,
            result_attribution,
            previous_attribution,
            notes,
        ));
    }

    let current_results = db::get_scan_results(&conn)
        .await
        .map_err(|e| e.to_string())?;
    if !current_results.is_empty() {
        return Ok(compute_ai_analyst_summary(
            window_days,
            "current_findings_fallback",
            None,
            None,
            None,
            None,
            None,
            0,
            Vec::new(),
            current_results,
            HashMap::new(),
            None,
            vec![
                "No completed historical scan was found in the selected window. Showing the current findings table instead."
                    .to_string(),
            ],
        ));
    }

    Ok(compute_ai_analyst_summary(
        window_days,
        "empty",
        None,
        None,
        None,
        None,
        None,
        0,
        Vec::new(),
        Vec::new(),
        HashMap::new(),
        None,
        vec!["Run a scan to populate AI Analyst with local findings.".to_string()],
    ))
}

#[tauri::command]
async fn get_ai_analyst_drilldown(
    app_handle: tauri::AppHandle,
    window_days: Option<i64>,
    dimension: String,
    key: String,
) -> Result<AiAnalystDrilldownResponse, String> {
    let window_days = window_days.unwrap_or(30).clamp(1, 365);
    let dimension = dimension.trim().to_lowercase();
    let selected_key = key.trim().to_string();

    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    let history = db::get_scan_history(&conn)
        .await
        .map_err(|e| e.to_string())?;

    let (mut basis, latest_scan_id, latest_scan_at, mut resources, attribution, mut notes) =
        latest_ai_source_for_window(history, window_days);

    if latest_scan_id.is_none() {
        let current_results = db::get_scan_results(&conn)
            .await
            .map_err(|e| e.to_string())?;
        if !current_results.is_empty() {
            basis = "current_findings_fallback".to_string();
            resources = current_results;
            notes.push(
                "No completed historical scan was found in the selected window. Showing current findings.".to_string(),
            );
        }
    }

    let mut rows = Vec::new();
    for resource in resources {
        let resource_key = resource_attribution_key(&resource);
        let row_attribution = attribution.get(&resource_key);
        let account_id = row_attribution
            .map(|value| value.account_id.clone())
            .filter(|value| !value.is_empty());
        let account_name = row_attribution
            .map(|value| value.account_name.clone())
            .filter(|value| !value.is_empty());

        let matched = match dimension.as_str() {
            "account" => account_id.as_deref() == Some(selected_key.as_str()),
            "provider" => normalize_ai_bucket_key(&resource.provider) == selected_key,
            "resource_type" => normalize_ai_bucket_key(&resource.resource_type) == selected_key,
            _ => false,
        };
        if !matched {
            continue;
        }

        rows.push(AiAnalystDrilldownRow {
            account_id,
            account_name,
            provider: resource.provider,
            region: resource.region,
            resource_type: resource.resource_type,
            resource_id: resource.id,
            details: resource.details,
            action_type: resource.action_type,
            estimated_monthly_waste: resource.estimated_monthly_cost,
        });
    }

    rows.sort_by(|a, b| {
        b.estimated_monthly_waste
            .partial_cmp(&a.estimated_monthly_waste)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.provider.cmp(&b.provider))
            .then_with(|| a.resource_id.cmp(&b.resource_id))
    });

    let selected_label = rows
        .first()
        .map(|row| match dimension.as_str() {
            "account" => row
                .account_name
                .clone()
                .or_else(|| row.account_id.clone())
                .unwrap_or_else(|| selected_key.clone()),
            "provider" => row.provider.clone(),
            "resource_type" => row.resource_type.clone(),
            _ => selected_key.clone(),
        })
        .unwrap_or_else(|| selected_key.clone());

    let total_monthly_waste = rows
        .iter()
        .map(|row| row.estimated_monthly_waste.max(0.0))
        .sum();
    let total_findings = rows.len() as i64;

    Ok(AiAnalystDrilldownResponse {
        window_days,
        basis,
        latest_scan_id,
        latest_scan_at,
        dimension,
        selected_key,
        selected_label,
        total_monthly_waste,
        total_findings,
        rows,
        notes,
    })
}

#[tauri::command]
async fn ask_ai_analyst_local(
    app_handle: tauri::AppHandle,
    question: String,
    window_days: Option<i64>,
) -> Result<AiAnalystLocalAnswer, String> {
    let window_days = window_days.unwrap_or(30).clamp(1, 365);
    let summary = get_ai_analyst_summary(app_handle, Some(window_days)).await?;
    Ok(answer_local_question(&question, window_days, &summary))
}

#[tauri::command]
async fn delete_scan_history(app_handle: tauri::AppHandle, id: i64) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::delete_scan_history(&conn, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn mark_resource_handled(
    app_handle: tauri::AppHandle,
    id: String,
    provider: String,
    note: Option<String>,
) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::mark_resource_handled(&conn, &id, &provider, note)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_account_rules_config(
    app_handle: tauri::AppHandle,
    account_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::get_account_rules_config(&conn, &account_id).await
}

#[tauri::command]
async fn get_provider_rules_config(
    app_handle: tauri::AppHandle,
    provider: String,
) -> Result<Vec<serde_json::Value>, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::get_provider_rules_config(&conn, &provider).await
}

#[tauri::command]
async fn update_account_rule_config(
    app_handle: tauri::AppHandle,
    account_id: String,
    rule_id: String,
    enabled: bool,
    params: Option<String>,
) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path)
        .await
        .map_err(|e| e.to_string())?;
    db::update_account_rule_config(&conn, &account_id, &rule_id, enabled, params).await
}

fn resolve_log_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();

    if let Ok(explicit_log_path) = std::env::var("CWS_LOG_PATH") {
        let trimmed = explicit_log_path.trim();
        if !trimmed.is_empty() {
            paths.push(std::path::PathBuf::from(trimmed));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            paths.push(
                std::path::PathBuf::from(local_app_data)
                    .join("CloudWasteScanner")
                    .join("cws.log"),
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(xdg_data_home) = std::env::var_os("XDG_DATA_HOME") {
            paths.push(
                std::path::PathBuf::from(xdg_data_home)
                    .join("CloudWasteScanner")
                    .join("cws.log"),
            );
        }

        if let Some(home_dir) = std::env::var_os("HOME") {
            paths.push(
                std::path::PathBuf::from(home_dir)
                    .join(".local")
                    .join("share")
                    .join("CloudWasteScanner")
                    .join("cws.log"),
            );
        }
    }

    // Always keep temp fallback for early-startup and permission-edge cases.
    paths.push(std::env::temp_dir().join("cws.log"));

    let mut dedup = HashSet::new();
    paths
        .into_iter()
        .filter(|path| dedup.insert(path.clone()))
        .collect()
}

#[derive(Debug, Clone, Serialize)]
struct SystemLogOverview {
    path: String,
    exists: bool,
    size_bytes: u64,
    updated_at: Option<i64>,
    total_lines: usize,
    error_lines: usize,
}

#[derive(Debug, Clone, Serialize)]
struct SystemLogRecord {
    line_number: usize,
    timestamp: Option<String>,
    level: String,
    area: String,
    event: String,
    message: String,
    raw: String,
}

#[derive(Debug, Clone, Serialize)]
struct SystemLogResponse {
    overview: SystemLogOverview,
    records: Vec<SystemLogRecord>,
}

#[derive(Debug, Clone, Serialize)]
struct SupportSnapshot {
    generated_at: i64,
    app_version: String,
    runtime_plan_type: Option<String>,
    license_present: bool,
    system_log: SystemLogOverview,
    recent_log_records: Vec<SystemLogRecord>,
    audit_rows: usize,
    feedback_records: usize,
    settings: serde_json::Value,
}

fn active_log_path() -> std::path::PathBuf {
    for candidate in resolve_log_paths() {
        if candidate.exists() {
            return candidate;
        }
    }
    resolve_log_paths()
        .into_iter()
        .next()
        .unwrap_or_else(|| std::env::temp_dir().join("cws.log"))
}

fn parse_system_log_line(line_number: usize, line: &str) -> SystemLogRecord {
    let trimmed = line.trim();

    let (timestamp, after_timestamp) = if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            (Some(rest[..end].to_string()), rest[end + 1..].trim())
        } else {
            (None, trimmed)
        }
    } else {
        (None, trimmed)
    };

    let lower = after_timestamp.to_ascii_lowercase();
    let level = if lower.contains("panic") || lower.contains("error") || lower.contains("failed") {
        "error"
    } else if lower.contains("warn") {
        "warn"
    } else if lower.contains("debug") {
        "debug"
    } else {
        "info"
    }
    .to_string();

    let area = if lower.contains("setup") || lower.contains("startup") {
        "startup"
    } else if lower.contains("proxy") {
        "proxy"
    } else if lower.contains("license") {
        "license"
    } else if lower.contains("update") {
        "update"
    } else if lower.contains("scan") {
        "scan"
    } else if lower.contains("api") {
        "local_api"
    } else {
        "general"
    }
    .to_string();

    let mut event = after_timestamp
        .split(':')
        .next()
        .unwrap_or(after_timestamp)
        .trim()
        .to_string();
    if event.len() > 72 {
        event.truncate(72);
    }
    if event.is_empty() {
        event = "log event".to_string();
    }

    SystemLogRecord {
        line_number,
        timestamp,
        level,
        area,
        event,
        message: after_timestamp.to_string(),
        raw: line.to_string(),
    }
}

fn build_system_log_response(log_path: &std::path::Path) -> Result<SystemLogResponse, String> {
    let exists = log_path.exists();
    let content = if exists {
        std::fs::read_to_string(log_path).map_err(|e| e.to_string())?
    } else {
        String::new()
    };

    let metadata = std::fs::metadata(log_path).ok();
    let updated_at = metadata
        .as_ref()
        .and_then(|meta| meta.modified().ok())
        .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64);

    let records: Vec<SystemLogRecord> = content
        .lines()
        .enumerate()
        .map(|(idx, line)| parse_system_log_line(idx + 1, line))
        .collect();
    let error_lines = records
        .iter()
        .filter(|record| record.level == "error")
        .count();

    Ok(SystemLogResponse {
        overview: SystemLogOverview {
            path: log_path.to_string_lossy().to_string(),
            exists,
            size_bytes: metadata.as_ref().map(|meta| meta.len()).unwrap_or(0),
            updated_at,
            total_lines: records.len(),
            error_lines,
        },
        records,
    })
}

#[tauri::command]
async fn get_system_log_overview() -> Result<SystemLogOverview, String> {
    let log_path = active_log_path();
    Ok(build_system_log_response(&log_path)?.overview)
}

#[tauri::command]
async fn read_system_logs() -> Result<SystemLogResponse, String> {
    let log_path = active_log_path();
    build_system_log_response(&log_path)
}

#[tauri::command]
async fn get_support_snapshot(app_handle: tauri::AppHandle) -> Result<SupportSnapshot, String> {
    let app_state = app_handle.state::<AppState>();
    let conn = db::init_db(&app_state.db_path).await?;
    let log_path = active_log_path();
    let log_response = build_system_log_response(&log_path)?;
    let audit_rows = db::get_audit_logs(&conn, None, None, 100, 0)
        .await
        .map_err(|e| e.to_string())?;
    let feedback_raw = db::get_setting(&conn, "feedback_history_json")
        .await
        .unwrap_or_else(|_| "[]".to_string());
    let feedback_records = serde_json::from_str::<Vec<serde_json::Value>>(&feedback_raw)
        .map(|items| items.len())
        .unwrap_or(0);
    let runtime_plan_type = read_runtime_plan_type(&app_state.db_path).await;
    let license_present = !load_license_file(app_handle.clone())
        .unwrap_or_default()
        .trim()
        .is_empty();
    let settings = serde_json::json!({
        "api_bind_host": db::get_setting(&conn, "api_bind_host").await.unwrap_or_default(),
        "api_port": db::get_setting(&conn, "api_port").await.unwrap_or_default(),
        "api_tls_enabled": db::get_setting(&conn, "api_tls_enabled").await.unwrap_or_default(),
        "proxy_mode": db::get_setting(&conn, "proxy_mode").await.unwrap_or_default(),
        "notification_trigger_mode": db::get_setting(&conn, NOTIFICATION_TRIGGER_MODE_SETTING_KEY).await.unwrap_or_default(),
    });

    Ok(SupportSnapshot {
        generated_at: now_unix_ts(),
        app_version: app_handle.package_info().version.to_string(),
        runtime_plan_type,
        license_present,
        system_log: log_response.overview.clone(),
        recent_log_records: log_response.records.into_iter().rev().take(20).collect(),
        audit_rows: audit_rows.len(),
        feedback_records,
        settings,
    })
}

#[tauri::command]
async fn open_system_log_location() -> Result<(), String> {
    let log_path = active_log_path();
    if let Some(parent) = log_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        return open_path_with_default_app(parent);
    }
    Err("System log directory is not available.".to_string())
}

#[tauri::command]
async fn open_system_log_file() -> Result<(), String> {
    let log_path = active_log_path();
    if !log_path.exists() {
        return Err("System log file does not exist yet.".to_string());
    }
    open_path_with_default_app(&log_path)
}

#[tauri::command]
async fn open_external_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("URL is required.".to_string());
    }
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err("Only http/https URLs are allowed.".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", trimmed])
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(trimmed)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(trimmed)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn log_startup_event(message: &str) {
    let ts = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let elapsed_ms = STARTUP_MONO
        .get()
        .map(|started| started.elapsed().as_millis())
        .unwrap_or(0);
    let line = format!("[{}] [+{}ms] {}", ts, elapsed_ms, message);

    let mut wrote_any = false;
    for log_path in resolve_log_paths() {
        if let Some(parent) = log_path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                continue;
            }
        }

        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            if writeln!(f, "{}", line).is_ok() {
                wrote_any = true;
            }
        }
    }

    if !wrote_any {
        eprintln!("{}", line);
    }
}

#[cfg(target_os = "windows")]
fn apply_main_window_icon(app: &tauri::App) {
    let Some(window) = app.get_webview_window("main") else {
        log_startup_event("main window icon skipped: main window handle not found");
        return;
    };

    match tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png")) {
        Ok(icon) => {
            if let Err(err) = window.set_icon(icon) {
                log_startup_event(&format!("failed to apply main window icon: {}", err));
            } else {
                log_startup_event("main window icon applied");
            }
        }
        Err(err) => {
            log_startup_event(&format!("failed to decode app icon bytes: {}", err));
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_main_window_icon(_app: &tauri::App) {}

fn install_panic_logger() {
    std::panic::set_hook(Box::new(|panic_info| {
        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "unknown panic payload"
        };
        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "unknown_location".to_string());
        log_startup_event(&format!("panic at {}: {}", location, payload));
    }));
}

#[cfg(target_os = "windows")]
fn configure_webview2_safety_args() {
    let key = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
    let fallback_args =
        "--disable-gpu --disable-software-rasterizer --disable-features=RendererCodeIntegrity";

    let existing = std::env::var(key).unwrap_or_default();
    if existing.trim().is_empty() {
        std::env::set_var(key, fallback_args);
        log_startup_event(&format!("set {}={}", key, fallback_args));
    } else if !existing.contains("--disable-gpu") {
        let merged = format!("{} {}", existing, fallback_args);
        std::env::set_var(key, merged.trim());
        log_startup_event(&format!("extended {} with fallback args", key));
    }
}

#[cfg(not(target_os = "windows"))]
fn configure_webview2_safety_args() {}

#[cfg(all(windows, target_env = "gnu"))]
fn prepend_path_dir(dir: &std::path::Path) {
    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(std::env::split_paths(&current));
    if let Ok(joined) = std::env::join_paths(paths) {
        std::env::set_var("PATH", joined);
    }
}

#[cfg(all(windows, target_env = "gnu"))]
fn prepare_windows_gnu_webview2_loader() {
    use std::path::PathBuf;

    let dll_name = "WebView2Loader.dll";
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let exe_dir = exe_dir.to_path_buf();
            let _ = std::env::set_current_dir(&exe_dir);
            candidates.push(exe_dir.clone());
            candidates.push(exe_dir.join("resources"));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if !candidates.iter().any(|p| p == &cwd) {
            candidates.push(cwd);
        }
    }

    let mut loader_dir = candidates
        .iter()
        .find(|dir| dir.join(dll_name).exists())
        .cloned();

    if loader_dir.is_none() {
        if let Some(exe_dir) = candidates.first() {
            let target = exe_dir.join(dll_name);
            match std::fs::File::create(&target) {
                Ok(mut file) => {
                    let write_res = file.write_all(include_bytes!("../WebView2Loader.dll"));
                    if write_res.is_ok() {
                        loader_dir = Some(exe_dir.clone());
                    } else if let Err(e) = write_res {
                        log_startup_event(&format!(
                            "failed to write {} to {}: {}",
                            dll_name,
                            target.display(),
                            e
                        ));
                    }
                }
                Err(e) => {
                    log_startup_event(&format!(
                        "failed to create {} in {}: {}",
                        dll_name,
                        exe_dir.display(),
                        e
                    ));
                }
            }
        }
    }

    if let Some(dir) = loader_dir {
        prepend_path_dir(&dir);
    } else {
        log_startup_event("WebView2Loader.dll not found in candidate startup paths");
    }
}

#[cfg(not(all(windows, target_env = "gnu")))]
fn prepare_windows_gnu_webview2_loader() {}

fn push_unique_path(paths: &mut Vec<std::path::PathBuf>, value: std::path::PathBuf) {
    if !paths.iter().any(|existing| existing == &value) {
        paths.push(value);
    }
}

fn existing_db_candidates(paths: &[std::path::PathBuf]) -> Vec<std::path::PathBuf> {
    let db_names = [
        "cws_pro.db",
        "cws.db",
        "cloud-waste-scanner.db",
        "cloud_waste_scanner.db",
    ];
    let mut out = Vec::new();
    for base in paths {
        for name in db_names {
            let candidate = base.join(name);
            if candidate.is_file() {
                out.push(candidate);
            }
        }
    }
    out
}

fn pick_best_db_candidate(
    candidates: Vec<std::path::PathBuf>,
    preferred_db_path: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let mut best: Option<(std::path::PathBuf, std::time::SystemTime, u64)> = None;
    for candidate in candidates {
        if candidate == preferred_db_path {
            continue;
        }
        let meta = match std::fs::metadata(&candidate) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if !meta.is_file() || meta.len() == 0 {
            continue;
        }
        let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let len = meta.len();
        let replace = match &best {
            None => true,
            Some((_, best_modified, best_len)) => {
                modified > *best_modified || (modified == *best_modified && len > *best_len)
            }
        };
        if replace {
            best = Some((candidate, modified, len));
        }
    }
    best.map(|(path, _, _)| path)
}

fn resolve_app_db_path(app: &tauri::App) -> std::path::PathBuf {
    let temp_base = std::env::temp_dir().join("cloud-waste-scanner");
    let preferred_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| temp_base.clone());
    let preferred_db_path = preferred_dir.join("cws_pro.db");

    if preferred_db_path.is_file() {
        log_startup_event(&format!(
            "using preferred app db: {}",
            preferred_db_path.display()
        ));
        return preferred_db_path;
    }

    let mut search_dirs: Vec<std::path::PathBuf> = Vec::new();
    push_unique_path(&mut search_dirs, preferred_dir.clone());
    if let Ok(dir) = app.path().local_data_dir() {
        push_unique_path(&mut search_dirs, dir.clone());
        push_unique_path(&mut search_dirs, dir.join("CloudWasteScanner"));
        push_unique_path(&mut search_dirs, dir.join("cloud-waste-scanner"));
        push_unique_path(&mut search_dirs, dir.join("cloud-waste-scanner-gui"));
        push_unique_path(&mut search_dirs, dir.join("com.cloud-waste-scanner.app"));
    }
    if let Ok(dir) = app.path().config_dir() {
        push_unique_path(&mut search_dirs, dir.clone());
        push_unique_path(&mut search_dirs, dir.join("CloudWasteScanner"));
        push_unique_path(&mut search_dirs, dir.join("cloud-waste-scanner"));
        push_unique_path(&mut search_dirs, dir.join("cloud-waste-scanner-gui"));
        push_unique_path(&mut search_dirs, dir.join("com.cloud-waste-scanner.app"));
    }
    if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from) {
        push_unique_path(&mut search_dirs, local_appdata.clone());
        push_unique_path(&mut search_dirs, local_appdata.join("CloudWasteScanner"));
        push_unique_path(&mut search_dirs, local_appdata.join("cloud-waste-scanner"));
        push_unique_path(
            &mut search_dirs,
            local_appdata.join("cloud-waste-scanner-gui"),
        );
        push_unique_path(
            &mut search_dirs,
            local_appdata.join("com.cloud-waste-scanner.app"),
        );
    }
    if let Some(appdata) = std::env::var_os("APPDATA").map(std::path::PathBuf::from) {
        push_unique_path(&mut search_dirs, appdata.clone());
        push_unique_path(&mut search_dirs, appdata.join("CloudWasteScanner"));
        push_unique_path(&mut search_dirs, appdata.join("cloud-waste-scanner"));
        push_unique_path(&mut search_dirs, appdata.join("cloud-waste-scanner-gui"));
        push_unique_path(
            &mut search_dirs,
            appdata.join("com.cloud-waste-scanner.app"),
        );
    }
    push_unique_path(&mut search_dirs, temp_base.join("app-data-fallback"));

    let legacy_db =
        pick_best_db_candidate(existing_db_candidates(&search_dirs), &preferred_db_path);

    if std::fs::create_dir_all(&preferred_dir).is_ok() {
        if let Some(legacy_path) = legacy_db {
            match std::fs::copy(&legacy_path, &preferred_db_path) {
                Ok(_) => {
                    log_startup_event(&format!(
                        "migrated app db from {} to {}",
                        legacy_path.display(),
                        preferred_db_path.display()
                    ));
                    return preferred_db_path;
                }
                Err(err) => {
                    log_startup_event(&format!(
                        "failed to copy legacy db from {} to {}: {}",
                        legacy_path.display(),
                        preferred_db_path.display(),
                        err
                    ));
                    log_startup_event(&format!(
                        "using legacy app db in place: {}",
                        legacy_path.display()
                    ));
                    return legacy_path;
                }
            }
        }

        log_startup_event(&format!(
            "using preferred app db (new): {}",
            preferred_db_path.display()
        ));
        return preferred_db_path;
    }

    if let Some(legacy_path) = legacy_db {
        log_startup_event(&format!(
            "preferred app data dir unavailable; using legacy db {}",
            legacy_path.display()
        ));
        return legacy_path;
    }

    let fallback_dir = temp_base.join("app-data-fallback");
    if std::fs::create_dir_all(&fallback_dir).is_err() {
        log_startup_event("failed to create both preferred and fallback app data dirs");
    }
    let fallback_db_path = fallback_dir.join("cws_pro.db");
    log_startup_event(&format!(
        "using fallback app db: {}",
        fallback_db_path.display()
    ));
    fallback_db_path
}

fn main() {
    install_panic_logger();
    let _ = STARTUP_MONO.set(std::time::Instant::now());
    log_startup_event("main entered");
    configure_webview2_safety_args();
    prepare_windows_gnu_webview2_loader();

    let app_result = tauri::Builder::default()
        .setup(|app| {
            log_startup_event("setup begin");
            let setup_started = std::time::Instant::now();
            apply_main_window_icon(app);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_min_size(Some(tauri::LogicalSize::new(1280.0, 800.0)));
            }
            let db_path = resolve_app_db_path(app);
            log_startup_event(&format!(
                "setup path resolved: db_path={} elapsed_ms={}",
                db_path.display(),
                setup_started.elapsed().as_millis()
            ));
            // Important startup invariant: register app state before invoking any helper
            // that may access app_handle.state::<AppState>().
            app.manage(AppState {
                db_path: db_path.clone(),
                pending_consume_lock: Arc::new(AsyncMutex::new(())),
            });
            log_startup_event(&format!(
                "setup app state managed elapsed_ms={}",
                setup_started.elapsed().as_millis()
            ));

            let handle = app.handle().clone();
            let db_path_clone = db_path.clone();
            tauri::async_runtime::spawn(async move {
                log_startup_event("startup async init begin");
                let async_started = std::time::Instant::now();
                let db_started = std::time::Instant::now();
                let pool = match db::init_db(&db_path_clone).await {
                    Ok(pool) => pool,
                    Err(err) => {
                        log_startup_event(&format!("startup async init failed: {}", err));
                        return;
                    }
                };
                log_startup_event(&format!(
                    "startup async db ready elapsed_ms={}",
                    db_started.elapsed().as_millis()
                ));

                let proxy_started = std::time::Instant::now();
                let mode_raw = db::get_setting(&pool, "proxy_mode")
                    .await
                    .unwrap_or("none".into());
                let mode = normalize_proxy_mode(&mode_raw);
                let url = db::get_setting(&pool, "proxy_url")
                    .await
                    .unwrap_or_default();
                configure_proxy_env(&mode, &url);
                log_startup_event(&format!(
                    "startup async proxy configured: mode={} has_url={} elapsed_ms={}",
                    mode.trim(),
                    !url.trim().is_empty(),
                    proxy_started.elapsed().as_millis()
                ));

                let api_settings_started = std::time::Instant::now();
                let bind_host_raw = db::get_setting(&pool, "api_bind_host")
                    .await
                    .unwrap_or("0.0.0.0".into());
                let bind_host = if bind_host_raw.trim().is_empty() {
                    "0.0.0.0".to_string()
                } else {
                    bind_host_raw.trim().to_string()
                };
                if bind_host_raw.trim().is_empty() {
                    let _ = db::save_setting(&pool, "api_bind_host", &bind_host).await;
                }

                let port_str = db::get_setting(&pool, "api_port")
                    .await
                    .unwrap_or("9123".into());
                let port = port_str.trim().parse::<u16>().unwrap_or(9123);
                let app_data_dir = db_path_clone
                    .parent()
                    .map(|parent| parent.to_path_buf())
                    .unwrap_or_else(|| {
                        std::env::temp_dir()
                            .join("cloud-waste-scanner")
                            .join("app-data-fallback")
                    });

                let tls_raw = db::get_setting(&pool, "api_tls_enabled")
                    .await
                    .unwrap_or_default();
                let api_tls_enabled = if tls_raw.trim().is_empty() {
                    let default_tls = default_api_tls_enabled(&bind_host);
                    let persisted = if default_tls { "1" } else { "0" };
                    let _ = db::save_setting(&pool, "api_tls_enabled", persisted).await;
                    default_tls
                } else {
                    parse_bool_setting(&tls_raw)
                };

                let token_raw = db::get_setting(&pool, "api_access_token")
                    .await
                    .unwrap_or_default();
                let token_candidate = token_raw.trim();
                let token_is_valid = token_candidate.len() >= API_MIN_ACCESS_TOKEN_LEN
                    && token_candidate.len() <= API_MAX_LICENSE_KEY_LEN
                    && !token_candidate.chars().any(|c| c.is_whitespace());
                let api_access_token = if !token_is_valid {
                    let generated = format!("cws_{}", Uuid::new_v4().to_string().replace('-', ""));
                    let _ = db::save_setting(&pool, "api_access_token", &generated).await;
                    generated
                } else {
                    token_candidate.to_string()
                };
                log_startup_event(&format!(
                    "startup async api settings loaded: bind_host={} port={} tls={} token_len={} elapsed_ms={}",
                    bind_host,
                    port,
                    api_tls_enabled,
                    api_access_token.len(),
                    api_settings_started.elapsed().as_millis()
                ));

                println!("DEBUG: API Bind Host Setting: '{}'", bind_host);
                println!("DEBUG: API Port Setting: '{}'", port_str);
                println!(
                    "DEBUG: API Transport: {}",
                    if api_tls_enabled {
                        "https(self-signed)"
                    } else {
                        "http"
                    }
                );
                println!(
                    "DEBUG: API Access Token loaded ({} chars)",
                    api_access_token.len()
                );

                let api_enabled = true;
                log_startup_event("startup api policy resolved: community mode local api enabled");

                // Startup must not be blocked by telemetry network conditions.
                let telemetry_pool = pool.clone();
                tauri::async_runtime::spawn(async move {
                    let telemetry_started = std::time::Instant::now();
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(2),
                        report_telemetry(&telemetry_pool, "app_launch", serde_json::json!({})),
                    )
                    .await
                    {
                        Ok(_) => {
                            log_startup_event(&format!(
                                "startup telemetry app_launch completed elapsed_ms={}",
                                telemetry_started.elapsed().as_millis()
                            ));
                        }
                        Err(_) => {
                            log_startup_event(
                                "startup telemetry app_launch timeout after 2000ms (ignored)",
                            );
                        }
                    }
                });

                log_startup_event(&format!(
                    "startup async init complete total_elapsed_ms={}",
                    async_started.elapsed().as_millis()
                ));
                tauri::async_runtime::spawn(async move {
                    start_api_server(
                        handle.clone(),
                        bind_host,
                        port,
                        api_access_token,
                        api_enabled,
                        api_tls_enabled,
                        app_data_dir,
                    )
                    .await;
                });
            });

            log_startup_event(&format!(
                "setup complete total_elapsed_ms={}",
                setup_started.elapsed().as_millis()
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_scan,
            get_scan_results,
            get_enriched_scan_results,
            replace_scan_results,
            clear_scan_results,
            get_audit_logs,
            clear_audit_logs,
            validate_license_key,
            check_license_status,
            start_trial_license,
            confirm_cleanup,
            get_dashboard_stats,
            get_governance_stats,
            list_aws_profiles,
            discover_importable_cloud_accounts,
            save_aws_profile,
            save_aws_profile_reference,
            delete_aws_profile,
            save_cloud_profile,
            update_cloud_profile,
            list_cloud_profiles,
            list_proxy_profiles,
            save_proxy_profile,
            delete_proxy_profile,
            delete_cloud_profile,
            download_and_install_update,
            cancel_update_download,
            save_license_file,
            save_export_file,
            reveal_export_file,
            load_license_file,
            save_setting,
            get_setting,
            get_system_log_overview,
            read_system_logs,
            get_support_snapshot,
            open_system_log_location,
            open_system_log_file,
            open_external_url,
            apply_proxy_settings,
            test_proxy_connection,
            collect_metrics,
            get_resource_metrics,
            get_monitor_snapshots,
            test_connection,
            track_event,
            list_policies,
            save_policy_cmd,
            delete_policy_cmd,
            list_notification_channels,
            save_notification_channel,
            delete_notification_channel,
            test_notification_channel,
            get_scan_history,
            get_ai_analyst_summary,
            get_ai_analyst_drilldown,
            ask_ai_analyst_local,
            delete_scan_history,
            mark_resource_handled,
            get_account_rules_config,
            get_provider_rules_config,
            update_account_rule_config
        ])
        .run(tauri::generate_context!());

    if let Err(e) = app_result {
        log_startup_event(&format!("tauri runtime failed: {}", e));
        eprintln!("error while running tauri application: {}", e);
    } else {
        log_startup_event("tauri runtime exited normally");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_import_id_part_normalizes_to_ascii_lower_and_underscores() {
        assert_eq!(
            sanitize_import_id_part(" AWS Prod/Main "),
            "_aws_prod_main_"
        );
    }

    #[test]
    fn enqueue_error_helpers_strip_rate_limit_prefix() {
        let (status, body) = map_scan_enqueue_error("rate_limited: slow down".to_string());
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body.0["error"], "slow down");
    }

    #[test]
    fn runtime_plan_gate_entitlements_match_edition_boundaries() {
        assert!(!schedule_entitled_for_runtime_plan(None));
        assert!(!audit_log_entitled_for_runtime_plan(None));

        assert!(!schedule_entitled_for_runtime_plan(Some("trial")));
        assert!(!audit_log_entitled_for_runtime_plan(Some("trial")));

        assert!(schedule_entitled_for_runtime_plan(Some("monthly")));
        assert!(!audit_log_entitled_for_runtime_plan(Some("monthly")));

        assert!(schedule_entitled_for_runtime_plan(Some("yearly")));
        assert!(!audit_log_entitled_for_runtime_plan(Some("yearly")));

        assert!(schedule_entitled_for_runtime_plan(Some("enterprise")));
        assert!(audit_log_entitled_for_runtime_plan(Some("enterprise")));
    }

    #[test]
    fn schedule_gate_message_mentions_required_editions() {
        let err = schedule_gate_error();
        let message = err.1 .0["error"].as_str().unwrap_or_default();
        assert!(message.contains("Team"));
        assert!(message.contains("Enterprise"));
    }
}
