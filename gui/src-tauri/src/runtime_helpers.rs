use cloud_waste_scanner_core::models::WastedResource;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

pub(crate) fn trial_gate_message() -> String {
    "Trial mode does not include Local API access. Upgrade to Pro to enable API automation."
        .to_string()
}

pub(crate) fn validate_ascii_input(name: &str, value: &str, max_len: usize) -> Result<(), String> {
    if value.len() > max_len {
        return Err(format!("{} exceeds max length {}", name, max_len));
    }
    if value.chars().any(|c| !c.is_ascii() || c.is_ascii_control()) {
        return Err(format!(
            "{} contains unsupported characters; use plain ASCII text",
            name
        ));
    }
    Ok(())
}

pub(crate) fn looks_like_email(raw: &str) -> bool {
    if raw.is_empty()
        || raw.len() > super::API_MAX_EMAIL_LEN
        || raw.chars().any(|c| c.is_whitespace())
    {
        return false;
    }
    let mut parts = raw.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if parts.next().is_some() || local.is_empty() || domain.is_empty() {
        return false;
    }
    if domain.starts_with('.') || domain.ends_with('.') || !domain.contains('.') {
        return false;
    }
    true
}

pub(crate) fn normalize_enqueue_error_message(err: &str) -> String {
    err.strip_prefix(super::API_RATE_LIMIT_PREFIX)
        .unwrap_or(err)
        .trim()
        .to_string()
}

pub(crate) fn normalize_channel_trigger_mode_for_storage(
    raw: Option<&str>,
) -> Result<Option<String>, String> {
    let normalized = raw
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    match normalized.as_str() {
        "" | "inherit" | super::NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE => Ok(Some(
            super::NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE.to_string(),
        )),
        "waste_found" | super::NOTIFICATION_TRIGGER_MODE_WASTE_ONLY => Ok(Some(
            super::NOTIFICATION_TRIGGER_MODE_WASTE_ONLY.to_string(),
        )),
        _ => Err("channel trigger_mode must be scan_complete or waste_only.".to_string()),
    }
}

pub(crate) fn normalize_channel_trigger_mode_for_eval(raw: Option<&str>) -> Option<&'static str> {
    let normalized = raw
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    match normalized.as_str() {
        super::NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE => {
            Some(super::NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE)
        }
        "waste_found" | super::NOTIFICATION_TRIGGER_MODE_WASTE_ONLY => {
            Some(super::NOTIFICATION_TRIGGER_MODE_WASTE_ONLY)
        }
        _ => None,
    }
}

pub(crate) fn resolve_effective_notification_trigger_mode(
    channel_mode: Option<&str>,
) -> &'static str {
    normalize_channel_trigger_mode_for_eval(channel_mode)
        .unwrap_or(super::NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE)
}

pub(crate) fn should_dispatch_notification_by_mode(trigger_mode: &str, total_savings: f64) -> bool {
    trigger_mode == super::NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE
        || (trigger_mode == super::NOTIFICATION_TRIGGER_MODE_WASTE_ONLY && total_savings > 0.0)
}

pub(crate) fn normalize_channel_min_savings_for_storage(
    raw: Option<f64>,
) -> Result<Option<f64>, String> {
    match raw {
        Some(value) if !value.is_finite() => {
            Err("channel min_savings must be a finite number.".to_string())
        }
        Some(value) if value < 0.0 => Err("channel min_savings must be >= 0.".to_string()),
        Some(value) if value <= f64::EPSILON => Ok(None),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

pub(crate) fn normalize_channel_min_findings_for_storage(
    raw: Option<i64>,
) -> Result<Option<i64>, String> {
    match raw {
        Some(value) if value < 0 => Err("channel min_findings must be >= 0.".to_string()),
        Some(0) => Ok(None),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

pub(crate) fn normalize_account_notification_choices(raw: Value) -> Vec<String> {
    let mut values: Vec<String> = Vec::new();
    match raw {
        Value::Array(items) => {
            for item in items {
                if let Some(text) = item.as_str() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        values.push(trimmed.to_string());
                    }
                }
            }
        }
        Value::String(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                values.push(trimmed.to_string());
            }
        }
        _ => {}
    }
    values.sort();
    values.dedup();
    if values.is_empty()
        || values
            .iter()
            .any(|value| value == super::ACCOUNT_NOTIFICATION_CHOICE_ALL)
    {
        vec![super::ACCOUNT_NOTIFICATION_CHOICE_ALL.to_string()]
    } else {
        values
    }
}

pub(crate) fn parse_notification_channel_email_recipients(config_raw: &str) -> Vec<String> {
    let mut ordered: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut push_candidate = |raw: &str| {
        for token in raw.split(|ch| ch == ',' || ch == ';' || ch == '\n') {
            let trimmed = token.trim();
            if trimmed.is_empty() || !looks_like_email(trimmed) {
                continue;
            }
            let key = trimmed.to_ascii_lowercase();
            if seen.insert(key) {
                ordered.push(trimmed.to_string());
            }
        }
    };

    let parsed = serde_json::from_str::<Value>(config_raw).ok();
    if let Some(value) = parsed.as_ref() {
        for key in ["emails", "recipients"] {
            if let Some(items) = value.get(key).and_then(|item| item.as_array()) {
                for item in items {
                    if let Some(text) = item.as_str() {
                        push_candidate(text);
                    }
                }
            }
        }

        for key in ["email_to", "email", "to"] {
            if let Some(text) = value.get(key).and_then(|item| item.as_str()) {
                push_candidate(text);
            }
        }
    }

    ordered
}

pub(crate) fn normalize_transport_error_detail(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "Unknown network transport error".to_string();
    }

    let lower = trimmed.to_lowercase();
    if lower.contains("connection refused")
        || lower.contains("actively refused")
        || lower.contains("积极拒绝")
        || lower.contains("目标计算机积极拒绝")
    {
        return "Connection refused by target host".to_string();
    }
    if lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("连接超时")
        || lower.contains("连接尝试失败")
    {
        return "Connection timed out".to_string();
    }
    if lower.contains("dns")
        || lower.contains("lookup")
        || lower.contains("could not resolve")
        || lower.contains("name or service not known")
        || lower.contains("找不到主机")
        || lower.contains("无法解析")
        || lower.contains("名称解析")
    {
        return "DNS resolution failed".to_string();
    }
    if lower.contains("forcibly closed")
        || lower.contains("connection reset")
        || lower.contains("强迫关闭")
        || lower.contains("现有的连接")
    {
        return "Connection reset by peer".to_string();
    }
    if lower.contains("network is unreachable")
        || lower.contains("host is unreachable")
        || lower.contains("无法访问")
    {
        return "Network unreachable".to_string();
    }
    trimmed.to_string()
}

pub(crate) fn compact_error_text(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn truncate_error_text(raw: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut out = String::new();
    for ch in raw.chars().take(max_chars) {
        out.push(ch);
    }
    if raw.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

pub(crate) fn summarize_error_text(raw: &str, max_chars: usize) -> String {
    truncate_error_text(&compact_error_text(raw), max_chars)
}

pub(crate) fn normalize_runtime_plan_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "subscription" => "monthly".to_string(),
        "per-use" => "starter".to_string(),
        "starter" => "starter".to_string(),
        "trial" => "trial".to_string(),
        "monthly" => "monthly".to_string(),
        "yearly" => "yearly".to_string(),
        "lifetime" => "lifetime".to_string(),
        other => other.to_string(),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct RuntimeEntitlements {
    pub(crate) local_scan: bool,
    pub(crate) basic_report: bool,
    pub(crate) resource_details: bool,
    pub(crate) local_api: bool,
    pub(crate) team_workspace: bool,
    pub(crate) scheduled_audits: bool,
    pub(crate) audit_log: bool,
    pub(crate) sso: bool,
    pub(crate) scim: bool,
}

pub(crate) fn resolve_runtime_edition(plan_type: &str, is_trial: bool) -> String {
    if is_trial {
        return "community".to_string();
    }
    match normalize_runtime_plan_type(plan_type).as_str() {
        "enterprise" | "advanced" | "site" => "enterprise".to_string(),
        "team" | "monthly" | "yearly" | "lifetime" | "pro" => "team".to_string(),
        "starter" | "trial" | "community" | "oss" | "free" => "community".to_string(),
        _ => "community".to_string(),
    }
}

pub(crate) fn build_runtime_entitlements(plan_type: &str, is_trial: bool) -> RuntimeEntitlements {
    let edition = resolve_runtime_edition(plan_type, is_trial);
    match edition.as_str() {
        "enterprise" => RuntimeEntitlements {
            local_scan: true,
            basic_report: true,
            resource_details: true,
            local_api: true,
            team_workspace: true,
            scheduled_audits: true,
            audit_log: true,
            sso: true,
            scim: true,
        },
        "team" => RuntimeEntitlements {
            local_scan: true,
            basic_report: true,
            resource_details: true,
            local_api: true,
            team_workspace: true,
            scheduled_audits: true,
            audit_log: false,
            sso: false,
            scim: false,
        },
        _ => RuntimeEntitlements {
            local_scan: true,
            basic_report: true,
            resource_details: !is_trial,
            local_api: !is_trial,
            team_workspace: false,
            scheduled_audits: false,
            audit_log: false,
            sso: false,
            scim: false,
        },
    }
}

pub(crate) fn summarize_for_trial(results: &[WastedResource]) -> Vec<WastedResource> {
    let total_savings: f64 = results.iter().map(|r| r.estimated_monthly_cost).sum();

    if total_savings <= 0.0 {
        return vec![WastedResource {
            id: "TRIAL_SUMMARY".to_string(),
            provider: "Summary".to_string(),
            region: "-".to_string(),
            resource_type: "Estimated Waste".to_string(),
            details: "No estimated waste detected in this scan window.".to_string(),
            estimated_monthly_cost: 0.0,
            action_type: "VIEW_ONLY".to_string(),
        }];
    }

    vec![WastedResource {
        id: "TRIAL_SUMMARY".to_string(),
        provider: "Summary".to_string(),
        region: "-".to_string(),
        resource_type: "Estimated Waste".to_string(),
        details: format!(
            "Potential waste found across your selected accounts. Upgrade to Pro to view exact resource IDs and remediation actions ({} findings hidden).",
            results.len()
        ),
        estimated_monthly_cost: total_savings,
        action_type: "UPGRADE_REQUIRED".to_string(),
    }]
}

pub(crate) fn validate_scan_request(payload: &super::ApiScanRequest) -> Result<(), String> {
    if let Some(license_key) = payload.license_key.as_deref() {
        let trimmed = license_key.trim();
        if !trimmed.is_empty() {
            validate_ascii_input("license_key", trimmed, super::API_MAX_LICENSE_KEY_LEN)?;
        }
    }

    if let Some(profile) = payload.aws_profile.as_deref() {
        let trimmed = profile.trim();
        if !trimmed.is_empty() {
            validate_ascii_input("aws_profile", trimmed, super::API_MAX_OPTIONAL_FIELD_LEN)?;
        }
    }

    if let Some(region) = payload.aws_region.as_deref() {
        let trimmed = region.trim();
        if !trimmed.is_empty() {
            validate_ascii_input("aws_region", trimmed, super::API_MAX_OPTIONAL_FIELD_LEN)?;
        }
    }

    if let Some(selected_accounts) = payload.selected_accounts.as_ref() {
        if selected_accounts.len() > super::API_MAX_SELECTED_ACCOUNTS {
            return Err(format!(
                "selected_accounts exceeds limit {}",
                super::API_MAX_SELECTED_ACCOUNTS
            ));
        }
        for account_id in selected_accounts {
            let trimmed = account_id.trim();
            if trimmed.is_empty() {
                return Err("selected_accounts contains an empty id".to_string());
            }
            validate_ascii_input(
                "selected_accounts[*]",
                trimmed,
                super::API_MAX_ACCOUNT_ID_LEN,
            )?;
        }
    }

    if let Some(report_emails) = payload.report_emails.as_ref() {
        if report_emails.len() > super::API_MAX_REPORT_EMAILS {
            return Err(format!(
                "report_emails exceeds limit {}",
                super::API_MAX_REPORT_EMAILS
            ));
        }
        for email in report_emails {
            let trimmed = email.trim();
            if !looks_like_email(trimmed) {
                return Err(format!("invalid report_emails item: {}", trimmed));
            }
        }
    }

    Ok(())
}

pub(crate) fn calculate_initial_next_run(
    run_at: Option<i64>,
    interval_minutes: Option<i64>,
    now: i64,
) -> Result<i64, String> {
    if let Some(interval) = interval_minutes {
        if interval <= 0 {
            return Err("interval_minutes must be > 0".to_string());
        }
    }

    let mut next = run_at.unwrap_or(now);
    if next < now {
        if let Some(interval) = interval_minutes {
            let step = interval.saturating_mul(60);
            let diff = now - next;
            let jumps = (diff / step) + 1;
            next = next.saturating_add(jumps.saturating_mul(step));
        } else {
            next = now;
        }
    }

    Ok(next)
}

pub(crate) fn calculate_follow_up_next_run(
    prev_next: i64,
    interval_minutes: Option<i64>,
    now: i64,
) -> Option<i64> {
    let interval = interval_minutes?;
    if interval <= 0 {
        return None;
    }

    let step = interval.saturating_mul(60);
    let mut next = prev_next.saturating_add(step);
    while next <= now {
        next = next.saturating_add(step);
    }
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn notification_trigger_modes_normalize_and_dispatch_as_expected() {
        assert_eq!(
            normalize_channel_trigger_mode_for_storage(Some("inherit")).unwrap(),
            Some(super::super::NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE.to_string())
        );
        assert_eq!(
            normalize_channel_trigger_mode_for_storage(Some("waste_found")).unwrap(),
            Some(super::super::NOTIFICATION_TRIGGER_MODE_WASTE_ONLY.to_string())
        );
        assert!(normalize_channel_trigger_mode_for_storage(Some("bad")).is_err());

        assert_eq!(
            resolve_effective_notification_trigger_mode(Some("waste_only")),
            super::super::NOTIFICATION_TRIGGER_MODE_WASTE_ONLY
        );
        assert!(should_dispatch_notification_by_mode(
            super::super::NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE,
            0.0
        ));
        assert!(!should_dispatch_notification_by_mode(
            super::super::NOTIFICATION_TRIGGER_MODE_WASTE_ONLY,
            0.0
        ));
        assert!(should_dispatch_notification_by_mode(
            super::super::NOTIFICATION_TRIGGER_MODE_WASTE_ONLY,
            10.0
        ));
    }

    #[test]
    fn notification_thresholds_reject_invalid_values() {
        assert_eq!(
            normalize_channel_min_savings_for_storage(Some(0.0)).unwrap(),
            None
        );
        assert_eq!(
            normalize_channel_min_savings_for_storage(Some(12.5)).unwrap(),
            Some(12.5)
        );
        assert!(normalize_channel_min_savings_for_storage(Some(-1.0)).is_err());

        assert_eq!(
            normalize_channel_min_findings_for_storage(Some(0)).unwrap(),
            None
        );
        assert_eq!(
            normalize_channel_min_findings_for_storage(Some(3)).unwrap(),
            Some(3)
        );
        assert!(normalize_channel_min_findings_for_storage(Some(-1)).is_err());
    }

    #[test]
    fn account_notification_choices_deduplicate_and_fall_back_to_all() {
        assert_eq!(
            normalize_account_notification_choices(json!(["slack", "email", "slack"])),
            vec!["email".to_string(), "slack".to_string()]
        );
        assert_eq!(
            normalize_account_notification_choices(json!([
                super::super::ACCOUNT_NOTIFICATION_CHOICE_ALL,
                "email"
            ])),
            vec![super::super::ACCOUNT_NOTIFICATION_CHOICE_ALL.to_string()]
        );
        assert_eq!(
            normalize_account_notification_choices(json!("  ")),
            vec![super::super::ACCOUNT_NOTIFICATION_CHOICE_ALL.to_string()]
        );
    }

    #[test]
    fn parse_notification_channel_email_recipients_keeps_order_and_deduplicates() {
        let recipients = parse_notification_channel_email_recipients(
            r#"{"emails":["ops@example.com","finops@example.com"],"to":"ops@example.com;lead@example.com"}"#,
        );
        assert_eq!(
            recipients,
            vec![
                "ops@example.com".to_string(),
                "finops@example.com".to_string(),
                "lead@example.com".to_string()
            ]
        );
    }

    #[test]
    fn transport_and_error_text_helpers_produce_operator_friendly_output() {
        assert_eq!(
            normalize_transport_error_detail("dial tcp: lookup api.telegram.org: no such host"),
            "DNS resolution failed"
        );
        assert_eq!(
            summarize_error_text(" stage=config_validate   reason=dispatch_failure ", 22),
            "stage=config_validate ..."
        );
        assert_eq!(truncate_error_text("abcdef", 3), "abc...");
        assert_eq!(compact_error_text("a   b\n c"), "a b c");
    }

    #[test]
    fn input_validation_and_email_shape_are_enforced() {
        assert!(validate_ascii_input("name", "plain-ascii", 32).is_ok());
        assert!(validate_ascii_input("name", "含中文", 32).is_err());
        assert!(validate_ascii_input("name", "too-long", 3).is_err());

        assert!(looks_like_email("ops@example.com"));
        assert!(!looks_like_email("ops @example.com"));
        assert!(!looks_like_email("ops@example"));
    }

    #[test]
    fn enqueue_error_helpers_strip_rate_limit_prefix() {
        assert_eq!(
            normalize_enqueue_error_message("rate_limited: too many scans"),
            "too many scans"
        );
    }

    #[test]
    fn runtime_plan_type_normalization_handles_legacy_values() {
        assert_eq!(normalize_runtime_plan_type("subscription"), "monthly");
        assert_eq!(normalize_runtime_plan_type("per-use"), "starter");
        assert_eq!(normalize_runtime_plan_type(" yearly "), "yearly");
    }

    #[test]
    fn runtime_edition_and_entitlements_follow_plan_boundaries() {
        assert_eq!(resolve_runtime_edition("trial", true), "community");
        assert_eq!(resolve_runtime_edition("monthly", false), "team");
        assert_eq!(resolve_runtime_edition("enterprise", false), "enterprise");

        let community = build_runtime_entitlements("starter", false);
        assert!(community.local_scan);
        assert!(!community.team_workspace);
        assert!(!community.sso);

        let team = build_runtime_entitlements("yearly", false);
        assert!(team.team_workspace);
        assert!(team.scheduled_audits);
        assert!(!team.sso);

        let enterprise = build_runtime_entitlements("enterprise", false);
        assert!(enterprise.sso);
        assert!(enterprise.scim);
        assert!(enterprise.audit_log);
    }

    #[test]
    fn trial_summary_shows_view_only_or_upgrade_required() {
        let empty = summarize_for_trial(&[]);
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0].action_type, "VIEW_ONLY");

        let populated = summarize_for_trial(&[WastedResource {
            id: "r1".to_string(),
            provider: "AWS".to_string(),
            region: "us-east-1".to_string(),
            resource_type: "EC2".to_string(),
            details: "idle".to_string(),
            estimated_monthly_cost: 42.5,
            action_type: "DELETE".to_string(),
        }]);
        assert_eq!(populated.len(), 1);
        assert_eq!(populated[0].action_type, "UPGRADE_REQUIRED");
        assert_eq!(populated[0].estimated_monthly_cost, 42.5);
    }

    #[test]
    fn validate_scan_request_enforces_limits_and_ascii_fields() {
        let ok = super::super::ApiScanRequest {
            license_key: Some("ABC-123".to_string()),
            aws_profile: Some("prod".to_string()),
            aws_region: Some("us-east-1".to_string()),
            selected_accounts: Some(vec!["aws-prod".to_string()]),
            demo_mode: Some(false),
            report_emails: Some(vec!["ops@example.com".to_string()]),
        };
        assert!(validate_scan_request(&ok).is_ok());

        let bad_email = super::super::ApiScanRequest {
            report_emails: Some(vec!["ops @example.com".to_string()]),
            ..Default::default()
        };
        assert!(validate_scan_request(&bad_email).is_err());

        let bad_account = super::super::ApiScanRequest {
            selected_accounts: Some(vec!["".to_string()]),
            ..Default::default()
        };
        assert!(validate_scan_request(&bad_account).is_err());
    }

    #[test]
    fn schedule_helpers_compute_next_runs_consistently() {
        assert_eq!(calculate_initial_next_run(None, None, 100).unwrap(), 100);
        assert_eq!(
            calculate_initial_next_run(Some(40), Some(10), 100).unwrap(),
            640
        );
        assert!(calculate_initial_next_run(Some(40), Some(0), 100).is_err());

        assert_eq!(calculate_follow_up_next_run(100, Some(10), 101), Some(700));
        assert_eq!(calculate_follow_up_next_run(100, None, 101), None);
        assert_eq!(calculate_follow_up_next_run(100, Some(0), 101), None);
    }

    #[test]
    fn trial_gate_message_mentions_local_api_upgrade_requirement() {
        let text = trial_gate_message();
        assert!(text.contains("Local API access"));
        assert!(text.contains("Upgrade to Pro"));
    }

    #[test]
    fn scan_request_validation_rejects_non_ascii_and_too_many_accounts() {
        let mut payload = super::super::ApiScanRequest {
            selected_accounts: Some(
                (0..=super::super::API_MAX_SELECTED_ACCOUNTS)
                    .map(|idx| format!("acct-{idx}"))
                    .collect(),
            ),
            ..Default::default()
        };
        assert!(validate_scan_request(&payload).is_err());

        payload.selected_accounts = Some(vec!["aws-生产".to_string()]);
        assert!(validate_scan_request(&payload).is_err());
    }

    #[test]
    fn error_helpers_handle_zero_max_and_unknown_transport() {
        assert_eq!(truncate_error_text("abcdef", 0), "");
        assert_eq!(
            normalize_transport_error_detail("mystery transport failure"),
            "mystery transport failure"
        );
        assert_eq!(summarize_error_text("a   b   c", 64), "a b c");
    }

    #[test]
    fn trigger_mode_and_eval_normalizers_cover_unknown_and_defaults() {
        assert_eq!(
            normalize_channel_trigger_mode_for_eval(Some("unknown")),
            None
        );
        assert_eq!(
            resolve_effective_notification_trigger_mode(Some(" unknown ")),
            super::super::NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE
        );
        assert_eq!(
            normalize_channel_trigger_mode_for_storage(None).unwrap(),
            Some(super::super::NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE.to_string())
        );
    }

    #[test]
    fn account_choice_and_email_parser_handle_mixed_payloads() {
        assert_eq!(
            normalize_account_notification_choices(json!(null)),
            vec![super::super::ACCOUNT_NOTIFICATION_CHOICE_ALL.to_string()]
        );
        let parsed = parse_notification_channel_email_recipients(
            r#"{"recipients":["ops@example.com","bad"],"email_to":"fin@example.com"}"#,
        );
        assert_eq!(
            parsed,
            vec!["ops@example.com".to_string(), "fin@example.com".to_string()]
        );
    }

    #[test]
    fn transport_detail_classifies_common_network_failure_patterns() {
        assert_eq!(
            normalize_transport_error_detail("connection refused by peer"),
            "Connection refused by target host"
        );
        assert_eq!(
            normalize_transport_error_detail("TLS handshake failure on proxy"),
            "TLS handshake failure on proxy"
        );
        assert_eq!(
            normalize_transport_error_detail("network is unreachable"),
            "Network unreachable"
        );
    }

    #[test]
    fn schedule_calculators_cover_catch_up_paths() {
        assert_eq!(
            calculate_initial_next_run(Some(10), Some(1), 400).unwrap(),
            430
        );
        assert_eq!(calculate_follow_up_next_run(100, Some(2), 800), Some(820));
    }
}
