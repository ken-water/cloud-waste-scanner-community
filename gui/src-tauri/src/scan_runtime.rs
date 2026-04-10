use crate::db;
use std::collections::HashMap;

pub(crate) fn compact_scan_error(raw: &str) -> String {
    let compacted = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let max_chars = 220usize;
    if compacted.chars().count() <= max_chars {
        compacted
    } else {
        format!(
            "{}...",
            compacted.chars().take(max_chars).collect::<String>()
        )
    }
}

pub(crate) fn push_credential_precheck_failure(
    failures: &mut Vec<String>,
    account: String,
    reason: String,
) {
    if failures.len() >= 6 {
        return;
    }
    failures.push(format!("{}: {}", account, compact_scan_error(&reason)));
}

pub(crate) fn summarize_credential_precheck_failures(failures: &[String]) -> Option<String> {
    if failures.is_empty() {
        return None;
    }
    let mut summary = failures
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ");
    if failures.len() > 3 {
        summary.push_str(&format!(" | +{} more", failures.len() - 3));
    }
    Some(summary)
}

pub(crate) fn resolve_aws_profiles_to_scan(
    selected_accounts: Option<&Vec<String>>,
    aws_profile: Option<&str>,
) -> Vec<String> {
    let mut aws_profiles_to_scan = Vec::new();
    if let Some(selected) = selected_accounts {
        for id in selected {
            if id.starts_with("aws_local:") {
                aws_profiles_to_scan.push(id.trim_start_matches("aws_local:").to_string());
            }
        }
    } else if let Some(profile) = aws_profile {
        let trimmed = profile.trim();
        if !trimmed.is_empty() {
            aws_profiles_to_scan.push(trimmed.to_string());
        }
    }
    aws_profiles_to_scan
}

pub(crate) fn filter_cloud_profiles_by_selection(
    all_profiles: Vec<db::CloudProfile>,
    selected_accounts: Option<&Vec<String>>,
) -> Vec<db::CloudProfile> {
    if let Some(selected) = selected_accounts {
        if selected.is_empty() {
            all_profiles
        } else {
            all_profiles
                .into_iter()
                .filter(|profile| selected.contains(&profile.id))
                .collect()
        }
    } else {
        all_profiles
    }
}

pub(crate) fn build_aws_local_profile_map(
    profiles: Vec<crate::aws_utils::AwsProfile>,
) -> HashMap<String, (String, String, String)> {
    profiles
        .into_iter()
        .map(|profile| (profile.name, (profile.key, profile.secret, profile.region)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cloud_profile(id: &str) -> db::CloudProfile {
        db::CloudProfile {
            id: id.to_string(),
            provider: "aws".to_string(),
            name: id.to_string(),
            credentials: "{}".to_string(),
            created_at: 0,
            timeout_seconds: None,
            policy_custom: None,
            proxy_profile_id: None,
        }
    }

    #[test]
    fn compact_scan_error_and_failure_summary_stay_readable() {
        let long = "timeout ".repeat(80);
        let compact = compact_scan_error(&long);
        assert!(compact.len() <= 223);

        let mut failures = Vec::new();
        for idx in 0..8 {
            push_credential_precheck_failure(
                &mut failures,
                format!("acct-{}", idx),
                "network timeout".to_string(),
            );
        }
        assert_eq!(failures.len(), 6);
        let summary = summarize_credential_precheck_failures(&failures).expect("summary");
        assert!(summary.contains("acct-0"));
        assert!(summary.contains("+3 more"));
    }

    #[test]
    fn account_selection_splits_local_aws_and_cloud_profiles() {
        let selected = vec![
            "aws_local:prod".to_string(),
            "azure-finance".to_string(),
            "aws_local:stage".to_string(),
        ];
        assert_eq!(
            resolve_aws_profiles_to_scan(Some(&selected), None),
            vec!["prod".to_string(), "stage".to_string()]
        );
        assert_eq!(
            resolve_aws_profiles_to_scan(None, Some("default")),
            vec!["default".to_string()]
        );

        let filtered = filter_cloud_profiles_by_selection(
            vec![
                cloud_profile("azure-finance"),
                cloud_profile("gcp-dev"),
                cloud_profile("aws-prod"),
            ],
            Some(&selected),
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "azure-finance");
    }

    #[test]
    fn aws_local_profile_map_keeps_credentials_by_profile_name() {
        let profiles = vec![
            crate::aws_utils::AwsProfile {
                name: "prod".to_string(),
                key: "AKIA1".to_string(),
                secret: "secret1".to_string(),
                region: "us-east-1".to_string(),
                auth_type: "access_key".to_string(),
            },
            crate::aws_utils::AwsProfile {
                name: "stage".to_string(),
                key: "AKIA2".to_string(),
                secret: "secret2".to_string(),
                region: "eu-west-1".to_string(),
                auth_type: "access_key".to_string(),
            },
        ];
        let map = build_aws_local_profile_map(profiles);
        assert_eq!(
            map.get("stage"),
            Some(&(
                "AKIA2".to_string(),
                "secret2".to_string(),
                "eu-west-1".to_string()
            ))
        );
    }

    #[test]
    fn summary_limits_examples_and_handles_empty_list() {
        assert_eq!(summarize_credential_precheck_failures(&[]), None);
        let failures = vec![
            "a: one".to_string(),
            "b: two".to_string(),
            "c: three".to_string(),
            "d: four".to_string(),
        ];
        let summary = summarize_credential_precheck_failures(&failures).expect("summary");
        assert!(summary.contains("a: one"));
        assert!(summary.contains("+1 more"));
    }
}
