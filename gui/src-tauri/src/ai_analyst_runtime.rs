use crate::{AiAnalystBreakdownRow, AiAnalystSummary};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AiAnalystIntent {
    TopProvider,
    TopAccount,
    TopResourceType,
    WasteDelta,
    Overview,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct AiAnalystAnswerHighlight {
    pub(crate) dimension: String,
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) estimated_monthly_waste: f64,
    pub(crate) findings: i64,
    pub(crate) share_pct: f64,
    pub(crate) delta_monthly_waste: Option<f64>,
    pub(crate) delta_findings: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct AiAnalystLocalAnswer {
    pub(crate) intent: String,
    pub(crate) window_days: i64,
    pub(crate) headline: String,
    pub(crate) summary: String,
    pub(crate) highlights: Vec<AiAnalystAnswerHighlight>,
    pub(crate) follow_up_suggestions: Vec<String>,
    pub(crate) basis: String,
    pub(crate) notes: Vec<String>,
}

pub(crate) fn parse_ai_analyst_question(question: &str) -> AiAnalystIntent {
    let normalized = question.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return AiAnalystIntent::Overview;
    }

    if contains_any(
        &normalized,
        &[
            "delta",
            "trend",
            "change",
            "compare",
            "compared",
            "last week",
            "last month",
            "increase",
            "decrease",
        ],
    ) {
        return AiAnalystIntent::WasteDelta;
    }

    if contains_any(&normalized, &["provider", "cloud vendor", "vendor"]) {
        return AiAnalystIntent::TopProvider;
    }

    if contains_any(&normalized, &["account", "workspace", "subscription"]) {
        return AiAnalystIntent::TopAccount;
    }

    if contains_any(
        &normalized,
        &[
            "resource type",
            "resource kind",
            "service type",
            "instance type",
            "which type",
        ],
    ) {
        return AiAnalystIntent::TopResourceType;
    }

    if normalized.contains("who")
        || normalized.contains("highest")
        || normalized.contains("largest")
        || normalized.contains("most")
        || normalized.contains("top")
    {
        if normalized.contains("resource") {
            return AiAnalystIntent::TopResourceType;
        }
        if normalized.contains("account") {
            return AiAnalystIntent::TopAccount;
        }
        return AiAnalystIntent::TopProvider;
    }

    AiAnalystIntent::Overview
}

pub(crate) fn answer_local_question(
    question: &str,
    window_days: i64,
    summary: &AiAnalystSummary,
) -> AiAnalystLocalAnswer {
    let intent = parse_ai_analyst_question(question);
    match intent {
        AiAnalystIntent::TopProvider => build_top_dimension_answer(
            "top_provider",
            window_days,
            summary,
            "provider",
            "provider",
            &summary.providers,
            "No provider-level waste is available yet. Run a scan first.",
        ),
        AiAnalystIntent::TopAccount => build_top_dimension_answer(
            "top_account",
            window_days,
            summary,
            "account",
            "account",
            &summary.accounts,
            "No account-level attribution is available yet. Run a scan with account mapping enabled.",
        ),
        AiAnalystIntent::TopResourceType => build_top_dimension_answer(
            "top_resource_type",
            window_days,
            summary,
            "resource type",
            "resource type",
            &summary.resource_types,
            "No resource-type breakdown is available yet. Run a scan first.",
        ),
        AiAnalystIntent::WasteDelta => build_delta_answer(window_days, summary),
        AiAnalystIntent::Overview => build_overview_answer(window_days, summary),
    }
}

fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| text.contains(pattern))
}

fn build_highlights(
    dimension: &str,
    rows: &[AiAnalystBreakdownRow],
) -> Vec<AiAnalystAnswerHighlight> {
    rows.iter()
        .take(3)
        .map(|row| AiAnalystAnswerHighlight {
            dimension: dimension.to_string(),
            key: row.key.clone(),
            label: row.label.clone(),
            estimated_monthly_waste: row.estimated_monthly_waste,
            findings: row.findings,
            share_pct: row.share_pct,
            delta_monthly_waste: row.delta_monthly_waste,
            delta_findings: row.delta_findings,
        })
        .collect()
}

fn build_top_dimension_answer(
    intent: &str,
    window_days: i64,
    summary: &AiAnalystSummary,
    dimension: &str,
    subject_label: &str,
    rows: &[AiAnalystBreakdownRow],
    empty_message: &str,
) -> AiAnalystLocalAnswer {
    let highlights = build_highlights(dimension, rows);
    if let Some(top) = rows.first() {
        let summary_line = format!(
            "In the last {} days, {} is the top {} at ${:.2}/month across {} finding(s), representing {:.1}% of the current waste view.",
            window_days,
            top.label,
            subject_label,
            top.estimated_monthly_waste,
            top.findings,
            top.share_pct
        );
        return AiAnalystLocalAnswer {
            intent: intent.to_string(),
            window_days,
            headline: format!("Top {}: {}", subject_label, top.label),
            summary: summary_line,
            highlights,
            follow_up_suggestions: vec![
                format!("Show the top {}s in the same window.", dimension),
                "Compare this window against the previous scan.".to_string(),
                "Drill into the affected resources.".to_string(),
            ],
            basis: summary.basis.clone(),
            notes: summary.notes.clone(),
        };
    }

    AiAnalystLocalAnswer {
        intent: intent.to_string(),
        window_days,
        headline: format!("No {} data yet", subject_label),
        summary: empty_message.to_string(),
        highlights,
        follow_up_suggestions: vec![
            "Run a fresh scan.".to_string(),
            "Ask for an overall waste summary.".to_string(),
        ],
        basis: summary.basis.clone(),
        notes: summary.notes.clone(),
    }
}

fn build_delta_answer(window_days: i64, summary: &AiAnalystSummary) -> AiAnalystLocalAnswer {
    let delta = summary.delta_monthly_waste.unwrap_or(0.0);
    let delta_findings = summary.delta_findings.unwrap_or(0);
    let trend_word = if delta > 0.0 {
        "increased"
    } else if delta < 0.0 {
        "decreased"
    } else {
        "held steady"
    };
    let comparison_text = if summary.previous_scan_id.is_some() {
        format!(
            "Compared with the previous scan, estimated monthly waste {} by ${:.2} and findings changed by {}.",
            trend_word,
            delta.abs(),
            delta_findings
        )
    } else {
        "There is no earlier completed scan in the selected window, so no delta can be calculated yet.".to_string()
    };

    AiAnalystLocalAnswer {
        intent: "waste_delta".to_string(),
        window_days,
        headline: "Waste delta".to_string(),
        summary: comparison_text,
        highlights: build_highlights("provider", &summary.providers),
        follow_up_suggestions: vec![
            "Show the top provider in this window.".to_string(),
            "Show the top account in this window.".to_string(),
            "Drill into the current highest-cost findings.".to_string(),
        ],
        basis: summary.basis.clone(),
        notes: summary.notes.clone(),
    }
}

fn build_overview_answer(window_days: i64, summary: &AiAnalystSummary) -> AiAnalystLocalAnswer {
    let top_provider = summary.providers.first().map(|row| row.label.clone());
    let summary_line = if summary.total_findings > 0 {
        match top_provider {
            Some(provider) => format!(
                "The current {}-day local view shows ${:.2}/month across {} finding(s). {} is the largest provider bucket right now.",
                window_days, summary.total_monthly_waste, summary.total_findings, provider
            ),
            None => format!(
                "The current {}-day local view shows ${:.2}/month across {} finding(s).",
                window_days, summary.total_monthly_waste, summary.total_findings
            ),
        }
    } else {
        "No local findings are available yet. Run a scan to populate the analyst view.".to_string()
    };

    AiAnalystLocalAnswer {
        intent: "overview".to_string(),
        window_days,
        headline: "Local waste overview".to_string(),
        summary: summary_line,
        highlights: build_highlights("provider", &summary.providers),
        follow_up_suggestions: vec![
            "Which provider has the most waste?".to_string(),
            "Which account has the most waste?".to_string(),
            "How did waste change from the previous scan?".to_string(),
        ],
        basis: summary.basis.clone(),
        notes: summary.notes.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        key: &str,
        label: &str,
        waste: f64,
        findings: i64,
        share_pct: f64,
        delta_monthly_waste: Option<f64>,
    ) -> AiAnalystBreakdownRow {
        AiAnalystBreakdownRow {
            key: key.to_string(),
            label: label.to_string(),
            estimated_monthly_waste: waste,
            findings,
            share_pct,
            delta_monthly_waste,
            delta_findings: Some(1),
        }
    }

    fn summary() -> AiAnalystSummary {
        AiAnalystSummary {
            window_days: 30,
            basis: "latest_scan_in_window".to_string(),
            latest_scan_id: Some(12),
            latest_scan_at: Some(1000),
            scan_count_in_window: 3,
            total_monthly_waste: 2400.0,
            total_findings: 8,
            previous_scan_id: Some(11),
            previous_scan_at: Some(900),
            previous_total_monthly_waste: Some(2100.0),
            previous_total_findings: Some(7),
            delta_monthly_waste: Some(300.0),
            delta_findings: Some(1),
            scanned_accounts: vec!["aws-prod".to_string()],
            accounts: vec![row("aws-prod", "AWS Prod", 1500.0, 5, 62.5, Some(120.0))],
            providers: vec![row("aws", "AWS", 1500.0, 5, 62.5, Some(120.0))],
            resource_types: vec![row("ec2", "EC2 Instance", 900.0, 3, 37.5, Some(50.0))],
            notes: vec!["Local data only.".to_string()],
        }
    }

    #[test]
    fn parser_classifies_supported_intents() {
        assert_eq!(
            parse_ai_analyst_question("Which provider had the most waste last week?"),
            AiAnalystIntent::WasteDelta
        );
        assert_eq!(
            parse_ai_analyst_question("Which account has the highest waste?"),
            AiAnalystIntent::TopAccount
        );
        assert_eq!(
            parse_ai_analyst_question("Top resource type by waste"),
            AiAnalystIntent::TopResourceType
        );
        assert_eq!(
            parse_ai_analyst_question("Give me an overview"),
            AiAnalystIntent::Overview
        );
    }

    #[test]
    fn answer_generation_returns_top_dimension_headline() {
        let answer = answer_local_question("Which provider has the most waste?", 30, &summary());
        assert_eq!(answer.intent, "top_provider");
        assert!(answer.headline.contains("AWS"));
        assert_eq!(answer.highlights.len(), 1);
        assert!(answer.summary.contains("$1500.00/month"));
    }

    #[test]
    fn delta_answer_uses_previous_scan_comparison() {
        let answer = answer_local_question("Compare with the previous scan", 30, &summary());
        assert_eq!(answer.intent, "waste_delta");
        assert!(answer.summary.contains("increased by $300.00"));
    }

    #[test]
    fn overview_answer_contract_serializes_expected_keys() {
        let answer = answer_local_question("overview", 30, &summary());
        let payload = serde_json::to_value(&answer).expect("serialize answer");

        assert_eq!(payload["intent"], "overview");
        assert_eq!(payload["window_days"], 30);
        assert_eq!(payload["basis"], "latest_scan_in_window");
        assert!(payload["headline"]
            .as_str()
            .unwrap_or_default()
            .contains("overview"));
        assert!(payload["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("$2400.00/month"));
        assert!(payload["highlights"].as_array().is_some());
        assert!(payload["follow_up_suggestions"].as_array().is_some());
        assert_eq!(payload["notes"][0], "Local data only.");
    }

    #[test]
    fn empty_state_contract_stays_actionable() {
        let empty = AiAnalystSummary {
            window_days: 30,
            basis: "empty".to_string(),
            latest_scan_id: None,
            latest_scan_at: None,
            scan_count_in_window: 0,
            total_monthly_waste: 0.0,
            total_findings: 0,
            previous_scan_id: None,
            previous_scan_at: None,
            previous_total_monthly_waste: None,
            previous_total_findings: None,
            delta_monthly_waste: None,
            delta_findings: None,
            scanned_accounts: Vec::new(),
            accounts: Vec::new(),
            providers: Vec::new(),
            resource_types: Vec::new(),
            notes: vec!["Run a scan.".to_string()],
        };

        let answer = answer_local_question("Which account has the most waste?", 30, &empty);
        assert_eq!(answer.intent, "top_account");
        assert!(answer.headline.contains("No account data yet"));
        assert!(answer.summary.contains("No account-level attribution"));
        assert!(answer.highlights.is_empty());
        assert_eq!(answer.follow_up_suggestions[0], "Run a fresh scan.");
    }

    #[test]
    fn delta_contract_without_previous_scan_explains_gap() {
        let mut no_previous = summary();
        no_previous.previous_scan_id = None;
        no_previous.previous_scan_at = None;
        no_previous.previous_total_monthly_waste = None;
        no_previous.previous_total_findings = None;
        no_previous.delta_monthly_waste = None;
        no_previous.delta_findings = None;

        let answer = answer_local_question("trend", 30, &no_previous);
        let payload = serde_json::to_value(&answer).expect("serialize answer");
        assert_eq!(payload["intent"], "waste_delta");
        assert!(payload["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("no earlier completed scan"));
    }
}
