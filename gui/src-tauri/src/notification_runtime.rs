use crate::runtime_helpers::{
    normalize_channel_min_findings_for_storage, normalize_channel_min_savings_for_storage,
    normalize_channel_trigger_mode_for_storage, resolve_effective_notification_trigger_mode,
    should_dispatch_notification_by_mode,
};
use crate::{ACCOUNT_NOTIFICATION_CHOICE_ALL, NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE};
use cloud_waste_scanner_core::NotificationChannel;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChannelSkipReason {
    Inactive,
    AccountRouting,
    TriggerPolicy,
    Threshold,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ChannelRoutingPlan {
    pub(crate) strict_channel_routing: bool,
    pub(crate) allow_all_channels: bool,
    pub(crate) routed_channel_ids: HashSet<String>,
}

pub(crate) fn validate_notification_method(method: &str) -> bool {
    matches!(
        method,
        "telegram" | "slack" | "webhook" | "custom" | "email" | "whatsapp"
    )
}

pub(crate) fn normalize_notification_method_for_storage(raw: &str) -> String {
    let method = raw.trim().to_ascii_lowercase();
    if method == "custom" {
        "webhook".to_string()
    } else {
        method
    }
}

pub(crate) fn build_channel_routing_plan(
    scanned_account_ids: &[String],
    assignments: &HashMap<String, Vec<String>>,
) -> ChannelRoutingPlan {
    let mut routed_channel_ids: HashSet<String> = HashSet::new();
    let mut allow_all_channels = scanned_account_ids.is_empty();
    for account_id in scanned_account_ids {
        if let Some(choices) = assignments.get(account_id) {
            for choice in choices {
                if choice == ACCOUNT_NOTIFICATION_CHOICE_ALL {
                    allow_all_channels = true;
                } else {
                    routed_channel_ids.insert(choice.to_string());
                }
            }
        } else {
            allow_all_channels = true;
        }
    }
    let strict_channel_routing = !allow_all_channels && !routed_channel_ids.is_empty();
    ChannelRoutingPlan {
        strict_channel_routing,
        allow_all_channels,
        routed_channel_ids,
    }
}

pub(crate) fn evaluate_channel_dispatch(
    channel: &NotificationChannel,
    routing_plan: &ChannelRoutingPlan,
    total_savings: f64,
    findings_count: i64,
) -> Result<&'static str, ChannelSkipReason> {
    if !channel.is_active {
        return Err(ChannelSkipReason::Inactive);
    }

    if routing_plan.strict_channel_routing
        && !routing_plan
            .routed_channel_ids
            .contains(channel.id.as_str())
    {
        return Err(ChannelSkipReason::AccountRouting);
    }

    let effective_trigger_mode =
        resolve_effective_notification_trigger_mode(channel.trigger_mode.as_deref());
    if !should_dispatch_notification_by_mode(effective_trigger_mode, total_savings) {
        return Err(ChannelSkipReason::TriggerPolicy);
    }

    let min_savings = channel.min_savings.unwrap_or(0.0);
    let min_findings = channel.min_findings.unwrap_or(0);
    if effective_trigger_mode != NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE {
        let meets_savings_threshold = total_savings + f64::EPSILON >= min_savings;
        let meets_findings_threshold = findings_count >= min_findings;
        if !meets_savings_threshold || !meets_findings_threshold {
            return Err(ChannelSkipReason::Threshold);
        }
    }

    Ok(effective_trigger_mode)
}

pub(crate) fn normalize_channel_for_save(
    mut channel: NotificationChannel,
) -> Result<NotificationChannel, String> {
    channel.method = normalize_notification_method_for_storage(&channel.method);
    if !validate_notification_method(&channel.method) {
        return Err("unsupported notification method.".to_string());
    }
    channel.trigger_mode =
        normalize_channel_trigger_mode_for_storage(channel.trigger_mode.as_deref())?;
    channel.min_savings = normalize_channel_min_savings_for_storage(channel.min_savings)?;
    channel.min_findings = normalize_channel_min_findings_for_storage(channel.min_findings)?;
    Ok(channel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(id: &str) -> NotificationChannel {
        NotificationChannel {
            id: id.to_string(),
            name: format!("Channel {}", id),
            method: "slack".to_string(),
            config: "{}".to_string(),
            is_active: true,
            proxy_profile_id: None,
            trigger_mode: Some("waste_only".to_string()),
            min_savings: Some(100.0),
            min_findings: Some(2),
        }
    }

    #[test]
    fn routing_plan_tracks_all_and_strict_modes() {
        let scanned = vec!["aws-prod".to_string(), "azure-finance".to_string()];
        let assignments = HashMap::from([
            ("aws-prod".to_string(), vec!["slack-ops".to_string()]),
            (
                "azure-finance".to_string(),
                vec![ACCOUNT_NOTIFICATION_CHOICE_ALL.to_string()],
            ),
        ]);
        let plan = build_channel_routing_plan(&scanned, &assignments);
        assert!(plan.allow_all_channels);
        assert!(!plan.strict_channel_routing);
        assert!(plan.routed_channel_ids.contains("slack-ops"));

        let strict_assignments =
            HashMap::from([("aws-prod".to_string(), vec!["email-finance".to_string()])]);
        let strict_plan =
            build_channel_routing_plan(&["aws-prod".to_string()], &strict_assignments);
        assert!(strict_plan.strict_channel_routing);
        assert!(!strict_plan.allow_all_channels);
    }

    #[test]
    fn channel_dispatch_checks_inactive_routing_trigger_and_thresholds() {
        let base = channel("slack-ops");
        let strict_plan = ChannelRoutingPlan {
            strict_channel_routing: true,
            allow_all_channels: false,
            routed_channel_ids: HashSet::from(["email-finance".to_string()]),
        };
        assert_eq!(
            evaluate_channel_dispatch(&base, &strict_plan, 500.0, 5),
            Err(ChannelSkipReason::AccountRouting)
        );

        let mut inactive = base.clone();
        inactive.is_active = false;
        assert_eq!(
            evaluate_channel_dispatch(&inactive, &ChannelRoutingPlan::default(), 500.0, 5),
            Err(ChannelSkipReason::Inactive)
        );

        let mut trigger = base.clone();
        trigger.trigger_mode = Some("waste_only".to_string());
        assert_eq!(
            evaluate_channel_dispatch(&trigger, &ChannelRoutingPlan::default(), 0.0, 5),
            Err(ChannelSkipReason::TriggerPolicy)
        );

        assert_eq!(
            evaluate_channel_dispatch(&base, &ChannelRoutingPlan::default(), 50.0, 1),
            Err(ChannelSkipReason::Threshold)
        );
        assert_eq!(
            evaluate_channel_dispatch(&base, &ChannelRoutingPlan::default(), 250.0, 3),
            Ok("waste_only")
        );
    }

    #[test]
    fn normalize_channel_for_save_cleans_aliases_and_thresholds() {
        let normalized = normalize_channel_for_save(NotificationChannel {
            id: "webhook-1".to_string(),
            name: "Webhook".to_string(),
            method: " custom ".to_string(),
            config: "{}".to_string(),
            is_active: true,
            proxy_profile_id: None,
            trigger_mode: Some("inherit".to_string()),
            min_savings: Some(0.0),
            min_findings: Some(0),
        })
        .expect("normalize channel");

        assert_eq!(normalized.method, "webhook");
        assert_eq!(
            normalized.trigger_mode.as_deref(),
            Some(NOTIFICATION_TRIGGER_MODE_SCAN_COMPLETE)
        );
        assert_eq!(normalized.min_savings, None);
        assert_eq!(normalized.min_findings, None);
        assert!(normalize_channel_for_save(NotificationChannel {
            method: "sms".to_string(),
            ..normalized
        })
        .is_err());
    }

    #[test]
    fn dispatch_matrix_covers_trigger_modes_and_threshold_boundaries() {
        let cases = vec![
            ("scan_complete", Some("scan_complete"), 0.0, 0, true),
            ("waste_only zero", Some("waste_only"), 0.0, 3, false),
            ("waste_only savings hit", Some("waste_only"), 200.0, 3, true),
            (
                "threshold miss findings",
                Some("waste_only"),
                200.0,
                1,
                false,
            ),
            ("legacy alias", Some("waste_found"), 150.0, 2, true),
        ];
        for (name, trigger_mode, savings, findings, should_pass) in cases {
            let mut c = channel("matrix");
            c.trigger_mode = trigger_mode.map(ToString::to_string);
            let result =
                evaluate_channel_dispatch(&c, &ChannelRoutingPlan::default(), savings, findings);
            assert_eq!(result.is_ok(), should_pass, "case={name}");
        }
    }
}
