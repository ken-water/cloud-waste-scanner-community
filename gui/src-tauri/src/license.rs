use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::Utc;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

// TODO: 生产环境请替换为真实的 PUBLIC_KEY
const PUBLIC_KEY_B64: &str = "G1RO_LymQXy9q_BWAiVVYxTa6RK4w_b7FVmrqbU1uvo";
const FIRST_PARTY_API_BASES: [&str; 0] = [];
const API_ROUTE_COOLDOWN_SECS: i64 = 45;
const ONLINE_STATUS_SUCCESS_CACHE_TTL_SECS: i64 = 45;
const ONLINE_STATUS_ERROR_CACHE_TTL_SECS: i64 = 8;

static API_ROUTE_COOLDOWN_UNTIL: OnceLock<Mutex<HashMap<&'static str, i64>>> = OnceLock::new();
static ONLINE_STATUS_CACHE: OnceLock<Mutex<HashMap<String, CachedOnlineStatus>>> = OnceLock::new();

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseType {
    Trial,
    Starter,
    Monthly,
    Yearly,
    Lifetime,
    Subscription, // legacy
    PerUse,       // legacy
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LicensePayload {
    pub id: String,              // 唯一ID (用于防重放)
    pub user: String,            // 用户邮箱
    pub l_type: LicenseType,     // 类型
    pub expires_at: Option<i64>, // 过期时间 (Subscription 用)
    pub max_hosts: Option<i64>,  // 资源数量限制 (50)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OrderHistoryEntry {
    pub order_ref: String,
    pub amount: f64,
    pub plan: String,
    pub status: String,
    pub created_at: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CheckResponse {
    pub valid: bool,
    pub latest_version: String,
    pub download_url: Option<String>,
    pub download_urls: Option<Vec<String>>,
    pub message: Option<String>,
    pub quota: Option<i64>,
    pub max_quota: Option<i64>,
    pub plan_type: Option<String>,
    pub is_trial: Option<bool>,
    pub trial_expires_at: Option<i64>,
    pub api_enabled: Option<bool>,
    pub resource_details_enabled: Option<bool>,
    pub customer_email: Option<String>,
    pub license_started_at: Option<i64>,
    pub first_purchase_at: Option<i64>,
    pub latest_purchase_at: Option<i64>,
    pub purchase_count: Option<i64>,
    pub latest_order_ref: Option<String>,
    pub latest_order_amount: Option<f64>,
    pub latest_order_plan: Option<String>,
    pub latest_order_status: Option<String>,
    pub order_history: Option<Vec<OrderHistoryEntry>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StartTrialResponse {
    pub status: Option<String>,
    pub license_key: String,
    pub trial_expires_at: Option<i64>,
    pub message: Option<String>,
}

#[derive(Clone)]
struct CachedOnlineStatus {
    cached_at: i64,
    ttl_secs: i64,
    result: Result<CheckResponse, String>,
}

fn summarize_text(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "-".to_string();
    }
    let mut iter = trimmed.chars();
    let snippet: String = iter.by_ref().take(max_chars).collect();
    if iter.next().is_some() {
        format!("{}...", snippet)
    } else {
        snippet
    }
}

fn route_cooldown_map() -> &'static Mutex<HashMap<&'static str, i64>> {
    API_ROUTE_COOLDOWN_UNTIL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn status_cache_map() -> &'static Mutex<HashMap<String, CachedOnlineStatus>> {
    ONLINE_STATUS_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn mark_route_failed(base: &'static str) {
    if let Ok(mut map) = route_cooldown_map().lock() {
        map.insert(base, Utc::now().timestamp() + API_ROUTE_COOLDOWN_SECS);
    }
}

fn mark_route_healthy(base: &'static str) {
    if let Ok(mut map) = route_cooldown_map().lock() {
        map.remove(base);
    }
}

fn ordered_api_bases() -> Vec<&'static str> {
    let now = Utc::now().timestamp();
    let mut preferred = Vec::new();
    let mut cooling_down = Vec::new();

    if let Ok(map) = route_cooldown_map().lock() {
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

fn should_retry_status(status: reqwest::StatusCode) -> bool {
    status.is_server_error()
        || status == reqwest::StatusCode::FORBIDDEN
        || status == reqwest::StatusCode::NOT_FOUND
        || status == reqwest::StatusCode::METHOD_NOT_ALLOWED
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
}

fn cache_key_for_status(key: &str, machine_id: Option<&str>) -> String {
    format!("{}::{}", key.trim(), machine_id.unwrap_or("-").trim())
}

fn read_cached_online_status(cache_key: &str) -> Option<Result<CheckResponse, String>> {
    let now = Utc::now().timestamp();
    let mut map = status_cache_map().lock().ok()?;
    let entry = map.get(cache_key)?.clone();
    if entry.cached_at + entry.ttl_secs < now {
        map.remove(cache_key);
        return None;
    }
    Some(entry.result)
}

fn write_cached_online_status(
    cache_key: String,
    result: Result<CheckResponse, String>,
    ttl_secs: i64,
) {
    if let Ok(mut map) = status_cache_map().lock() {
        map.insert(
            cache_key,
            CachedOnlineStatus {
                cached_at: Utc::now().timestamp(),
                ttl_secs,
                result,
            },
        );
    }
}

#[cfg(test)]
fn clear_test_state() {
    if let Ok(mut map) = route_cooldown_map().lock() {
        map.clear();
    }
    if let Ok(mut map) = status_cache_map().lock() {
        map.clear();
    }
}

async fn post_json_with_failover<T: DeserializeOwned>(
    path: &str,
    payload: &serde_json::Value,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<T, String> {
    let mut route_errors: Vec<String> = Vec::new();
    let ordered = ordered_api_bases();
    let total = ordered.len();

    for (idx, base) in ordered.into_iter().enumerate() {
        let url = format!("{}{}", base, path);
        let client = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()
            .map_err(|e| format!("Failed to initialize API client: {}", e))?;

        let response = match client.post(&url).json(payload).send().await {
            Ok(resp) => resp,
            Err(err) => {
                mark_route_failed(base);
                route_errors.push(format!("{} => network error: {}", base, err));
                continue;
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let detail = format!(
                "{} => HTTP {}: {}",
                base,
                status.as_u16(),
                summarize_text(&body, 180)
            );
            if should_retry_status(status) && idx + 1 < total {
                mark_route_failed(base);
                route_errors.push(detail);
                continue;
            }
            mark_route_healthy(base);
            return Err(detail);
        }

        match response.json::<T>().await {
            Ok(parsed) => {
                mark_route_healthy(base);
                return Ok(parsed);
            }
            Err(err) => {
                mark_route_failed(base);
                route_errors.push(format!("{} => decode error: {}", base, err));
            }
        }
    }

    if route_errors.is_empty() {
        Err("All API routes failed without a specific error.".to_string())
    } else {
        Err(format!(
            "All API routes failed: {}",
            route_errors.join(" | ")
        ))
    }
}

async fn check_online_status_impl(
    key: &str,
    machine_id: Option<&str>,
    force_refresh: bool,
) -> Result<CheckResponse, String> {
    let normalized_key = key.trim();
    if normalized_key.is_empty() {
        return Err("License key is required.".to_string());
    }

    let cache_key = cache_key_for_status(normalized_key, machine_id);
    if !force_refresh {
        if let Some(cached) = read_cached_online_status(&cache_key) {
            return cached;
        }
    }

    let result = Ok(CheckResponse {
        valid: true,
        latest_version: env!("CARGO_PKG_VERSION").to_string(),
        download_url: None,
        download_urls: None,
        message: Some("Community mode local check.".to_string()),
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
    });
    let ttl_secs = if result.is_ok() {
        ONLINE_STATUS_SUCCESS_CACHE_TTL_SECS
    } else {
        ONLINE_STATUS_ERROR_CACHE_TTL_SECS
    };
    write_cached_online_status(cache_key, result.clone(), ttl_secs);
    result
}

pub async fn check_online_status(
    key: &str,
    machine_id: Option<&str>,
) -> Result<CheckResponse, String> {
    check_online_status_impl(key, machine_id, false).await
}

pub async fn check_online_status_fresh(
    key: &str,
    machine_id: Option<&str>,
) -> Result<CheckResponse, String> {
    check_online_status_impl(key, machine_id, true).await
}

pub async fn start_trial(
    _email: Option<String>,
    _machine_id: &str,
) -> Result<StartTrialResponse, String> {
    Ok(StartTrialResponse {
        status: Some("community".to_string()),
        license_key: format!("community-local-{}", Utc::now().timestamp()),
        trial_expires_at: None,
        message: Some("Community mode local activation.".to_string()),
    })
}

pub fn verify_license(key: &str) -> Result<LicensePayload, String> {
    let key = key.trim();
    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() != 2 {
        return Err("Invalid license format".into());
    }

    let payload_b64 = parts[0];
    let signature_b64 = parts[1];

    // 1. Decode Key
    let pub_key_bytes = URL_SAFE_NO_PAD
        .decode(PUBLIC_KEY_B64)
        .map_err(|_| "Bad public key config")?;
    let verifying_key = VerifyingKey::from_bytes(
        &pub_key_bytes
            .try_into()
            .map_err(|_| "Invalid public key length")?,
    )
    .map_err(|_| "Invalid public key")?;

    // 2. Decode Data
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| "Invalid payload encoding")?;
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(signature_b64)
        .map_err(|_| "Invalid signature encoding")?;
    let signature = Signature::from_bytes(
        &sig_bytes
            .try_into()
            .map_err(|_| "Invalid signature length")?,
    );

    // 3. Verify
    verifying_key
        .verify(&payload_bytes, &signature)
        .map_err(|_| "License signature mismatch")?;

    // 4. Parse
    let payload: LicensePayload =
        serde_json::from_slice(&payload_bytes).map_err(|_| "Invalid license data")?;

    // 5. Check Expiry (when provided)
    if let Some(exp) = payload.expires_at {
        if exp < Utc::now().timestamp() {
            return Err("License has expired".into());
        }
    }

    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_status(valid: bool) -> CheckResponse {
        CheckResponse {
            valid,
            latest_version: "2.7.0".to_string(),
            download_url: None,
            download_urls: None,
            message: Some("ok".to_string()),
            quota: Some(88),
            max_quota: Some(100),
            plan_type: Some("monthly".to_string()),
            is_trial: Some(false),
            trial_expires_at: None,
            api_enabled: Some(true),
            resource_details_enabled: Some(true),
            customer_email: Some("ops@example.com".to_string()),
            license_started_at: None,
            first_purchase_at: None,
            latest_purchase_at: None,
            purchase_count: None,
            latest_order_ref: None,
            latest_order_amount: None,
            latest_order_plan: None,
            latest_order_status: None,
            order_history: None,
        }
    }

    #[test]
    fn summarize_text_handles_blank_and_truncation() {
        assert_eq!(summarize_text("   ", 10), "-");
        assert_eq!(summarize_text("short", 10), "short");
        assert_eq!(summarize_text("abcdef", 3), "abc...");
    }

    #[test]
    fn retry_policy_marks_expected_statuses_retryable() {
        assert!(should_retry_status(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(should_retry_status(reqwest::StatusCode::FORBIDDEN));
        assert!(should_retry_status(reqwest::StatusCode::NOT_FOUND));
        assert!(should_retry_status(reqwest::StatusCode::METHOD_NOT_ALLOWED));
        assert!(should_retry_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(!should_retry_status(reqwest::StatusCode::BAD_REQUEST));
    }

    #[test]
    fn cache_key_includes_machine_dimension() {
        assert_eq!(
            cache_key_for_status(" abc ", Some(" host-1 ")),
            "abc::host-1"
        );
        assert_eq!(cache_key_for_status("abc", None), "abc::-");
    }

    #[test]
    fn online_status_cache_reads_and_expires_entries() {
        clear_test_state();

        let key = "license::machine".to_string();
        let ok_result = Ok(sample_status(true));
        write_cached_online_status(key.clone(), ok_result.clone(), 60);
        let cached = read_cached_online_status(&key).expect("cached result");
        assert!(cached.as_ref().expect("ok").valid);

        write_cached_online_status(key.clone(), ok_result, -1);
        assert!(read_cached_online_status(&key).is_none());
        clear_test_state();
    }

    #[test]
    fn route_failover_order_prefers_healthy_routes() {
        clear_test_state();
        let Some(first_base) = FIRST_PARTY_API_BASES.first().copied() else {
            clear_test_state();
            return;
        };
        let Some(second_base) = FIRST_PARTY_API_BASES.get(1).copied() else {
            clear_test_state();
            return;
        };

        let default_order = ordered_api_bases();
        assert_eq!(default_order, FIRST_PARTY_API_BASES.to_vec());

        mark_route_failed(first_base);
        let cooled = ordered_api_bases();
        assert_eq!(cooled.first().copied(), Some(second_base));
        assert_eq!(cooled.get(1).copied(), Some(first_base));

        mark_route_healthy(first_base);
        let restored = ordered_api_bases();
        assert_eq!(restored, FIRST_PARTY_API_BASES.to_vec());
        clear_test_state();
    }

    #[test]
    fn verify_license_rejects_bad_shapes_early() {
        assert_eq!(
            verify_license("not-a-license").unwrap_err(),
            "Invalid license format"
        );
        assert_eq!(
            verify_license("bad.payload").unwrap_err(),
            "Invalid payload encoding"
        );
        assert_eq!(
            verify_license("e30.bad-signature").unwrap_err(),
            "Invalid signature encoding"
        );
    }
}
