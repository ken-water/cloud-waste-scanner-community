use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanPolicy {
    pub cpu_percent: f64,
    pub network_mb: f64,
    pub lookback_days: i64,
}

impl Default for ScanPolicy {
    fn default() -> Self {
        Self {
            cpu_percent: 2.0,
            network_mb: 5.0,
            lookback_days: 7,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WastedResource {
    pub id: String,
    pub provider: String,
    pub region: String,
    pub resource_type: String,
    pub details: String,
    pub estimated_monthly_cost: f64,
    #[serde(default = "default_action")]
    pub action_type: String, // "DELETE", "RIGHTSIZE", "ARCHIVE"
}

fn default_action() -> String {
    "DELETE".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResourceMetric {
    pub id: String,
    pub provider: String,
    pub region: String,
    pub resource_type: String,
    pub name: Option<String>,
    pub status: String,
    pub cpu_utilization: Option<f64>,
    pub network_in_mb: Option<f64>,
    pub connections: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PolicyCondition {
    pub metric: String,   // "cpu", "network_in", "connections", "age"
    pub operator: String, // "<", ">", "=", "contains"
    pub value: f64,
    pub unit: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Policy {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub target_type: String, // "ec2", "rds", "disk", "all"
    pub conditions: Vec<PolicyCondition>,
    pub logic: String, // "AND", "OR"
    pub is_active: bool,
    pub priority: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct NotificationChannel {
    pub id: String,
    pub name: String,
    pub method: String, // "slack", "teams", "discord", "webhook"
    pub config: String, // JSON: { "url": "..." }
    pub is_active: bool,
    #[serde(default)]
    pub proxy_profile_id: Option<String>,
    #[serde(default)]
    pub trigger_mode: Option<String>, // null/"inherit" => follow global mode
    #[serde(default)]
    pub min_savings: Option<f64>, // null => no savings threshold
    #[serde(default)]
    pub min_findings: Option<i64>, // null => no findings threshold
}
