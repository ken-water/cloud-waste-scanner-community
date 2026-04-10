use crate::models::{Policy, ResourceMetric};

pub fn evaluate(metric: &ResourceMetric, policies: &[Policy]) -> bool {
    // If no policies for this type, return false (not wasted)
    // Or true? Default behavior is conservative: don't flag unless policy says so.
    // BUT we have legacy "hardcoded" checks in the scanners.
    // Ideally, we want: if ANY active policy matches, it IS wasted.

    let relevant_policies: Vec<&Policy> = policies
        .iter()
        .filter(|p| {
            p.is_active
                && (p.target_type == "all"
                    || p.target_type == metric.resource_type.to_lowercase()
                    || map_type(&p.target_type, &metric.resource_type))
        })
        .collect();

    if relevant_policies.is_empty() {
        return false;
    }

    for policy in relevant_policies {
        if matches_policy(metric, policy) {
            return true; // Wasted!
        }
    }

    false
}

fn map_type(policy_type: &str, resource_type: &str) -> bool {
    // Map simplified types (ec2) to actual API types (EC2 Instance)
    match policy_type {
        "ec2" => resource_type == "EC2 Instance" || resource_type == "Virtual Machine",
        "rds" => resource_type == "RDS Instance" || resource_type == "SQL Database",
        "disk" => {
            resource_type == "EBS Volume"
                || resource_type == "Disk"
                || resource_type == "Persistent Disk"
        }
        "eip" => resource_type == "Elastic IP" || resource_type == "Public IP",
        _ => policy_type.eq_ignore_ascii_case(resource_type),
    }
}

fn matches_policy(metric: &ResourceMetric, policy: &Policy) -> bool {
    let matches = policy.logic != "OR";

    for condition in &policy.conditions {
        let val = get_metric_value(metric, &condition.metric);
        let is_match = check_condition(val, &condition.operator, condition.value);

        if policy.logic == "OR" {
            if is_match {
                return true;
            }
        } else {
            // AND
            if !is_match {
                return false;
            }
        }
    }

    matches
}

fn get_metric_value(metric: &ResourceMetric, field: &str) -> f64 {
    match field {
        "cpu" => metric.cpu_utilization.unwrap_or(0.0),
        "network_in" => metric.network_in_mb.unwrap_or(0.0),
        "connections" => metric.connections.unwrap_or(0) as f64,
        "status" => {
            if metric.status == "available" || metric.status == "unused" {
                0.0
            } else {
                1.0
            }
        } // 0 = available (wasted)
        _ => 0.0,
    }
}

fn check_condition(actual: f64, operator: &str, threshold: f64) -> bool {
    match operator {
        "<" => actual < threshold,
        ">" => actual > threshold,
        "=" => (actual - threshold).abs() < 0.001,
        "<=" => actual <= threshold,
        ">=" => actual >= threshold,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Policy, PolicyCondition, ResourceMetric};

    fn sample_metric() -> ResourceMetric {
        ResourceMetric {
            id: "r-1".to_string(),
            provider: "aws".to_string(),
            region: "us-east-1".to_string(),
            resource_type: "EC2 Instance".to_string(),
            name: Some("prod-idle".to_string()),
            status: "running".to_string(),
            cpu_utilization: Some(1.2),
            network_in_mb: Some(2.4),
            connections: Some(0),
        }
    }

    fn condition(metric: &str, operator: &str, value: f64) -> PolicyCondition {
        PolicyCondition {
            metric: metric.to_string(),
            operator: operator.to_string(),
            value,
            unit: None,
        }
    }

    fn policy(target_type: &str, logic: &str, conditions: Vec<PolicyCondition>) -> Policy {
        Policy {
            id: "p-1".to_string(),
            name: "Idle resource".to_string(),
            description: None,
            target_type: target_type.to_string(),
            conditions,
            logic: logic.to_string(),
            is_active: true,
            priority: 1,
        }
    }

    #[test]
    fn evaluate_returns_false_without_relevant_policies() {
        let metric = sample_metric();
        let policies = vec![policy("rds", "AND", vec![condition("cpu", "<", 5.0)])];
        assert!(!evaluate(&metric, &policies));
    }

    #[test]
    fn evaluate_matches_mapped_ec2_type() {
        let metric = sample_metric();
        let policies = vec![policy(
            "ec2",
            "AND",
            vec![
                condition("cpu", "<", 2.0),
                condition("network_in", "<", 5.0),
            ],
        )];
        assert!(evaluate(&metric, &policies));
    }

    #[test]
    fn evaluate_respects_or_logic() {
        let metric = sample_metric();
        let policies = vec![policy(
            "all",
            "OR",
            vec![
                condition("cpu", ">", 90.0),
                condition("connections", "=", 0.0),
            ],
        )];
        assert!(evaluate(&metric, &policies));
    }

    #[test]
    fn evaluate_ignores_inactive_policies() {
        let metric = sample_metric();
        let mut inactive = policy("all", "AND", vec![condition("cpu", "<", 2.0)]);
        inactive.is_active = false;
        assert!(!evaluate(&metric, &[inactive]));
    }

    #[test]
    fn check_condition_supports_expected_operators() {
        assert!(check_condition(1.0, "<", 2.0));
        assert!(check_condition(2.0, ">", 1.0));
        assert!(check_condition(1.0, "=", 1.0));
        assert!(check_condition(2.0, ">=", 2.0));
        assert!(check_condition(2.0, "<=", 2.0));
        assert!(!check_condition(2.0, "contains", 2.0));
    }

    #[test]
    fn map_type_matches_known_aliases_and_case_insensitive_fallback() {
        assert!(map_type("disk", "Persistent Disk"));
        assert!(map_type("eip", "Public IP"));
        assert!(map_type("custom", "Custom"));
    }

    #[test]
    fn get_metric_value_maps_status_available_to_zero() {
        let mut metric = sample_metric();
        metric.status = "available".to_string();
        assert_eq!(get_metric_value(&metric, "status"), 0.0);
        metric.status = "running".to_string();
        assert_eq!(get_metric_value(&metric, "status"), 1.0);
    }

    #[test]
    fn and_logic_with_empty_conditions_matches_by_default() {
        let metric = sample_metric();
        let p = policy("all", "AND", vec![]);
        assert!(matches_policy(&metric, &p));
    }
}
