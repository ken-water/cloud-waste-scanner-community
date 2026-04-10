use crate::db;
use serde_json::Value;
use std::collections::HashMap;

pub(crate) fn parse_meta_i64(raw: Option<&Value>) -> i64 {
    if let Some(value) = raw {
        if let Some(v) = value.as_i64() {
            return v;
        }
        if let Some(v) = value.as_u64() {
            return i64::try_from(v).unwrap_or(i64::MAX);
        }
        if let Some(v) = value.as_f64() {
            return v.round() as i64;
        }
    }
    0
}

pub(crate) fn normalize_governance_window_days(window_days: Option<i64>) -> i64 {
    let mut days = window_days.unwrap_or(30);
    if days < 7 {
        days = 7;
    }
    if days > 90 {
        days = 90;
    }
    days
}

pub(crate) fn governance_error_category_catalog() -> &'static [&'static str] {
    &[
        "auth",
        "network",
        "timeout",
        "config",
        "rate_limit",
        "service",
        "unknown",
    ]
}

pub(crate) fn governance_error_category_label(category: &str) -> &'static str {
    match category {
        "auth" => "Authentication / Authorization",
        "network" => "Network Connectivity",
        "timeout" => "Timeout",
        "config" => "Configuration",
        "rate_limit" => "Rate Limited",
        "service" => "Provider Service",
        _ => "Unknown",
    }
}

pub(crate) fn normalize_governance_error_category_key(raw: &str) -> String {
    let key = raw
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .replace(' ', "_");
    match key.as_str() {
        "auth" | "authentication" | "authorization" | "permission" | "auth_error" => {
            "auth".to_string()
        }
        "network" | "connection" | "dns" | "proxy" => "network".to_string(),
        "timeout" | "timed_out" => "timeout".to_string(),
        "config" | "configuration" | "invalid_input" | "validation" => "config".to_string(),
        "rate_limit" | "ratelimit" | "too_many_requests" | "throttle" => "rate_limit".to_string(),
        "service" | "provider" | "upstream" | "http_error" | "target_http_error" => {
            "service".to_string()
        }
        "unknown" => "unknown".to_string(),
        _ => "unknown".to_string(),
    }
}

pub(crate) fn classify_error_text_category(raw: &str) -> String {
    let text = raw.to_ascii_lowercase();
    if text.contains("timeout") || text.contains("timed out") {
        return "timeout".to_string();
    }
    if text.contains("unauthorized")
        || text.contains("forbidden")
        || text.contains("access denied")
        || text.contains("auth")
        || text.contains("credential")
        || text.contains("permission")
        || text.contains("token")
    {
        return "auth".to_string();
    }
    if text.contains("rate limit") || text.contains("too many requests") || text.contains("throttl")
    {
        return "rate_limit".to_string();
    }
    if text.contains("proxy")
        || text.contains("dns")
        || text.contains("network")
        || text.contains("connection")
        || text.contains("tls")
        || text.contains("ssl")
        || text.contains("handshake")
    {
        return "network".to_string();
    }
    if text.contains("invalid")
        || text.contains("missing")
        || text.contains("config")
        || text.contains("format")
        || text.contains("malformed")
    {
        return "config".to_string();
    }
    if text.contains("http")
        || text.contains("server error")
        || text.contains("service unavailable")
        || text.contains("internal error")
    {
        return "service".to_string();
    }
    "unknown".to_string()
}

pub(crate) fn parse_governance_error_bucket_counts(meta: &Value) -> HashMap<String, i64> {
    let mut out: HashMap<String, i64> = HashMap::new();
    if let Some(obj) = meta
        .get("scan_error_buckets")
        .and_then(|value| value.as_object())
    {
        for (raw_key, raw_count) in obj {
            let normalized_key = normalize_governance_error_category_key(raw_key);
            let count = parse_meta_i64(Some(raw_count)).max(0);
            if count > 0 {
                *out.entry(normalized_key).or_insert(0) += count;
            }
        }
    }
    out
}

pub(crate) fn normalize_governance_account_label(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn title_case_words(raw: &str) -> String {
    raw.split_whitespace()
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let first = chars.next().map(|c| c.to_ascii_uppercase()).unwrap_or(' ');
            let rest = chars.as_str().to_ascii_lowercase();
            format!("{}{}", first, rest)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn normalize_governance_provider(raw: &str) -> String {
    let mut provider = raw.trim();
    if provider.is_empty() {
        return "Unknown".to_string();
    }
    if let Some((prefix, _)) = provider.split_once('(') {
        let prefix_trimmed = prefix.trim();
        if !prefix_trimmed.is_empty() {
            provider = prefix_trimmed;
        }
    }
    let key = provider
        .trim()
        .to_ascii_lowercase()
        .replace('_', "")
        .replace('-', "")
        .replace(' ', "");
    match key.as_str() {
        "aws" => "AWS".to_string(),
        "gcp" | "googlecloud" => "GCP".to_string(),
        "azure" => "Azure".to_string(),
        "alibaba" | "alibabacloud" => "Alibaba Cloud".to_string(),
        "tencent" | "tencentcloud" => "Tencent Cloud".to_string(),
        "huawei" | "huaweicloud" => "Huawei Cloud".to_string(),
        "baidu" | "baiducloud" => "Baidu Cloud".to_string(),
        "volcengine" => "Volcengine".to_string(),
        "cloudflare" => "Cloudflare".to_string(),
        _ => title_case_words(&provider.replace(['_', '-'], " ")),
    }
}

pub(crate) fn generate_demo_governance_history(
    window_days: Option<i64>,
    now_ts: i64,
) -> Vec<db::ScanHistoryItem> {
    let days = normalize_governance_window_days(window_days);
    let day_seconds = 86_400i64;
    let window_end_ts = now_ts.div_euclid(day_seconds) * day_seconds;
    let window_start_ts = window_end_ts - ((days - 1) * day_seconds);
    let account_pool = [
        "aws-prod-main",
        "aws-shared-services",
        "azure-finance",
        "gcp-analytics",
        "do-edge",
    ];

    let mut history = Vec::new();
    let mut next_id = 1_i64;

    for offset in 0..days {
        let day_ts = window_start_ts + offset * day_seconds;
        let runs_for_day = if offset % 11 == 0 {
            2
        } else if offset % 5 == 0 {
            0
        } else {
            1
        };

        for run_idx in 0..runs_for_day {
            let seed = offset * 7 + run_idx as i64;
            let mut resources = crate::demo_data::generate_demo_data();
            for (idx, item) in resources.iter_mut().enumerate() {
                let jitter = ((seed + idx as i64 * 3).rem_euclid(7) as f64 - 3.0) * 0.04;
                let factor = (0.92 + jitter).clamp(0.70, 1.20);
                item.estimated_monthly_cost =
                    crate::round_two((item.estimated_monthly_cost * factor).max(0.0));
            }
            let keep_count = (6 + seed.rem_euclid(6)) as usize;
            resources.truncate(keep_count.min(resources.len()));

            let attempted = 16 + seed.rem_euclid(7);
            let failed = if seed.rem_euclid(9) == 0 {
                3
            } else if seed.rem_euclid(5) == 0 {
                2
            } else if seed.rem_euclid(4) == 0 {
                1
            } else {
                0
            };
            let succeeded = (attempted - failed).max(0);

            let mut scan_error_buckets: HashMap<String, i64> = HashMap::new();
            if failed > 0 {
                *scan_error_buckets.entry("auth".to_string()).or_insert(0) += 1;
            }
            if failed > 1 {
                *scan_error_buckets.entry("network".to_string()).or_insert(0) += 1;
            }
            if failed > 2 {
                *scan_error_buckets.entry("timeout".to_string()).or_insert(0) += failed - 2;
            }

            let account_span = 2 + seed.rem_euclid(3) as usize;
            let mut scanned_accounts = Vec::new();
            for account_offset in 0..account_span {
                let index = (seed as usize + account_offset) % account_pool.len();
                scanned_accounts.push(account_pool[index].to_string());
            }

            let meta = serde_json::json!({
                "scanned_accounts": scanned_accounts,
                "duration_ms": 18_000 + seed.rem_euclid(8) * 1_800,
                "scan_checks_attempted": attempted,
                "scan_checks_succeeded": succeeded,
                "scan_checks_failed": failed,
                "scan_error_taxonomy_version": "v1",
                "scan_error_buckets": scan_error_buckets
            });

            let total_waste: f64 = resources.iter().map(|r| r.estimated_monthly_cost).sum();
            let scanned_at = day_ts + 9 * 3_600 + (run_idx as i64) * 3_600;
            history.push(db::ScanHistoryItem {
                id: next_id,
                scanned_at,
                total_waste: crate::round_two(total_waste),
                resource_count: resources.len() as i64,
                status: "success".to_string(),
                results_json: serde_json::to_string(&resources)
                    .unwrap_or_else(|_| "[]".to_string()),
                scan_meta: Some(meta.to_string()),
            });
            next_id += 1;
        }
    }

    history
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_governance_window_days_clamps_to_supported_range() {
        assert_eq!(normalize_governance_window_days(None), 30);
        assert_eq!(normalize_governance_window_days(Some(3)), 7);
        assert_eq!(normalize_governance_window_days(Some(120)), 90);
    }

    #[test]
    fn normalize_governance_provider_maps_major_clouds_and_titles_fallback() {
        assert_eq!(normalize_governance_provider("aws"), "AWS");
        assert_eq!(
            normalize_governance_provider("alibaba_cloud"),
            "Alibaba Cloud"
        );
        assert_eq!(
            normalize_governance_provider("custom_provider-east"),
            "Custom Provider East"
        );
    }

    #[test]
    fn classify_and_bucket_governance_errors_normalize_consistently() {
        assert_eq!(classify_error_text_category("request timed out"), "timeout");
        assert_eq!(
            classify_error_text_category("credential rejected by provider"),
            "auth"
        );
        assert_eq!(
            classify_error_text_category("proxy dns lookup failed"),
            "network"
        );

        let buckets = parse_governance_error_bucket_counts(&json!({
            "scan_error_buckets": {
                "auth_error": 2,
                "dns": 1,
                "timeout": 3,
                "ignored": 0
            }
        }));
        assert_eq!(buckets.get("auth"), Some(&2));
        assert_eq!(buckets.get("network"), Some(&1));
        assert_eq!(buckets.get("timeout"), Some(&3));
        assert!(!buckets.contains_key("ignored"));
    }

    #[test]
    fn parse_meta_and_label_normalizers_cover_edge_cases() {
        assert_eq!(parse_meta_i64(Some(&json!(1.6))), 2);
        assert_eq!(parse_meta_i64(Some(&json!(u64::MAX))), i64::MAX);
        assert_eq!(parse_meta_i64(None), 0);
        assert_eq!(
            normalize_governance_account_label("  aws-prod   finance  "),
            "aws-prod finance"
        );
        assert_eq!(
            governance_error_category_label("rate_limit"),
            "Rate Limited"
        );
        assert!(governance_error_category_catalog().contains(&"service"));
    }

    #[test]
    fn demo_governance_history_respects_window_and_meta_schema() {
        let now = 1_710_000_000;
        let history = generate_demo_governance_history(Some(7), now);
        assert!(!history.is_empty());
        let min_ts = history
            .iter()
            .map(|item| item.scanned_at)
            .min()
            .unwrap_or(now);
        let max_ts = history
            .iter()
            .map(|item| item.scanned_at)
            .max()
            .unwrap_or(now);
        assert!(max_ts >= min_ts);
        let sample_meta = history
            .iter()
            .find_map(|item| item.scan_meta.clone())
            .expect("scan meta");
        let parsed = serde_json::from_str::<serde_json::Value>(&sample_meta).expect("meta json");
        assert!(parsed.get("scan_checks_attempted").is_some());
        assert!(parsed.get("scan_error_buckets").is_some());
    }
}
