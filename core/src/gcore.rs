use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde_json::Value;

use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct GcoreScanner {
    client: Client,
    token: String,
    base_url: String,
}

impl GcoreScanner {
    pub fn new(token: &str, endpoint: &str) -> Self {
        Self {
            client: Client::new(),
            token: token.trim().to_string(),
            base_url: Self::normalize_endpoint(endpoint),
        }
    }

    fn normalize_endpoint(raw: &str) -> String {
        let endpoint = raw.trim();
        if endpoint.is_empty() {
            return "https://api.gcore.com/cloud/v1".to_string();
        }

        let mut value = endpoint.to_string();
        if !value.starts_with("http://") && !value.starts_with("https://") {
            value = format!("https://{}", value);
        }

        if value.contains("/cloud/v") {
            return value.trim_end_matches('/').to_string();
        }

        format!("{}/cloud/v1", value.trim_end_matches('/'))
    }

    async fn request_json(&self, path: &str) -> Result<Value> {
        if self.token.is_empty() {
            return Err(anyhow!("Gcore API token is required"));
        }

        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(anyhow!(
                "Gcore API {} failed ({}): {}",
                path,
                status.as_u16(),
                snippet
            ));
        }

        serde_json::from_str(&text).map_err(|e| anyhow!("Invalid Gcore JSON: {}", e))
    }

    async fn request_any_json(&self, paths: &[&str]) -> Result<Value> {
        let mut last_err: Option<anyhow::Error> = None;

        for path in paths {
            match self.request_json(path).await {
                Ok(payload) => return Ok(payload),
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Gcore API request failed")))
    }

    fn str_field(value: &Value, keys: &[&str]) -> String {
        for key in keys {
            if let Some(text) = value.get(*key).and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    return text.to_string();
                }
            }
        }
        String::new()
    }

    fn parse_f64(value: &Value, keys: &[&str]) -> Option<f64> {
        for key in keys {
            if let Some(number) = value.get(*key).and_then(|v| v.as_f64()) {
                return Some(number);
            }
            if let Some(number) = value.get(*key).and_then(|v| v.as_i64()) {
                return Some(number as f64);
            }
            if let Some(number) = value.get(*key).and_then(|v| v.as_u64()) {
                return Some(number as f64);
            }
            if let Some(text) = value.get(*key).and_then(|v| v.as_str()) {
                if let Ok(parsed) = text.parse::<f64>() {
                    return Some(parsed);
                }
            }
        }
        None
    }

    fn parse_time(raw: &str) -> Option<DateTime<Utc>> {
        if raw.trim().is_empty() {
            return None;
        }

        let formats = [
            "%Y-%m-%dT%H:%M:%S%z",
            "%Y-%m-%dT%H:%M:%S%.3f%z",
            "%Y-%m-%dT%H:%M:%S%.f%z",
            "%Y-%m-%dT%H:%M:%SZ",
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d",
        ];

        for fmt in formats {
            if let Ok(dt) = DateTime::parse_from_str(raw, fmt) {
                return Some(dt.with_timezone(&Utc));
            }
        }

        chrono::DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }

    fn extract_items(payload: &Value, root_keys: &[&str], item_keys: &[&str]) -> Vec<Value> {
        let mut containers: Vec<&Value> = vec![payload];

        for root in root_keys {
            if let Some(value) = payload.get(*root) {
                containers.push(value);
            }
        }

        let mut results = Vec::new();
        for container in containers {
            if let Some(array) = container.as_array() {
                results.extend(array.iter().cloned());
            }

            for item_key in item_keys {
                if let Some(value) = container.get(*item_key) {
                    if let Some(array) = value.as_array() {
                        results.extend(array.iter().cloned());
                    } else if value.is_object() {
                        results.push(value.clone());
                    }
                }
            }
        }

        results
    }

    pub async fn check_auth(&self) -> Result<()> {
        self.request_any_json(&["/instances?limit=1", "/servers?limit=1", "/projects"])
            .await?;
        Ok(())
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_any_json(&["/instances?limit=200", "/servers?limit=200"])
            .await?;

        let mut wastes = Vec::new();
        for instance in Self::extract_items(
            &payload,
            &["instances", "servers", "results"],
            &["instances", "servers", "items"],
        ) {
            let state =
                Self::str_field(&instance, &["status", "state", "power_state"]).to_lowercase();
            let stopped = state.contains("stop")
                || state.contains("off")
                || state.contains("shut")
                || state.contains("inactive");
            if !stopped {
                continue;
            }

            let id = Self::str_field(&instance, &["id", "instance_id", "server_id"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::str_field(&instance, &["name", "display_name", "hostname"]);
            let region = Self::str_field(&instance, &["region", "location", "zone"]);
            let flavor = Self::str_field(&instance, &["flavor", "plan", "type"]);

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "Gcore".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Instance".to_string(),
                details: format!(
                    "Stopped Gcore instance: {} ({})",
                    if name.is_empty() { id } else { name },
                    if flavor.is_empty() {
                        "unknown"
                    } else {
                        &flavor
                    }
                ),
                estimated_monthly_cost: 10.0,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_any_json(&["/volumes?limit=200", "/block-storage/volumes?limit=200"])
            .await?;

        let mut wastes = Vec::new();
        for volume in Self::extract_items(&payload, &["volumes", "results"], &["volumes", "items"])
        {
            let attached_to = Self::str_field(
                &volume,
                &["instance_id", "server_id", "attached_to", "resource_id"],
            );
            if !attached_to.is_empty() {
                continue;
            }

            let attachments = volume
                .get("attachments")
                .and_then(|v| v.as_array())
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            if attachments {
                continue;
            }

            let id = Self::str_field(&volume, &["id", "volume_id"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::str_field(&volume, &["name", "display_name"]);
            let region = Self::str_field(&volume, &["region", "location", "zone"]);
            let size =
                Self::parse_f64(&volume, &["size", "size_gb", "sizeGB", "capacity"]).unwrap_or(0.0);
            let normalized = if size <= 0.0 { 20.0 } else { size };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "Gcore".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Volume".to_string(),
                details: format!(
                    "Unattached Gcore volume: {} ({:.0} GB)",
                    if name.is_empty() { id } else { name },
                    normalized
                ),
                estimated_monthly_cost: normalized * 0.08,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_ips(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_any_json(&[
                "/floatingips?limit=200",
                "/ips?limit=200",
                "/public-ips?limit=200",
            ])
            .await?;

        let mut wastes = Vec::new();
        for ip in Self::extract_items(
            &payload,
            &["floatingips", "ips", "results"],
            &["floatingips", "ips", "items"],
        ) {
            let attached_to = Self::str_field(
                &ip,
                &["instance_id", "server_id", "attached_to", "resource_id"],
            );
            if !attached_to.is_empty() {
                continue;
            }

            let id = Self::str_field(&ip, &["address", "ip", "id"]);
            if id.is_empty() {
                continue;
            }

            let region = Self::str_field(&ip, &["region", "location", "zone"]);

            wastes.push(WastedResource {
                id,
                provider: "Gcore".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Public IP".to_string(),
                details: "Unassigned Gcore public IP".to_string(),
                estimated_monthly_cost: 1.5,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_any_json(&[
                "/snapshots?limit=200",
                "/volume-snapshots?limit=200",
                "/backups?limit=200",
            ])
            .await?;

        let mut wastes = Vec::new();
        for snapshot in Self::extract_items(
            &payload,
            &["snapshots", "backups", "results"],
            &["snapshots", "backups", "items"],
        ) {
            let id = Self::str_field(&snapshot, &["id", "snapshot_id", "backup_id"]);
            if id.is_empty() {
                continue;
            }

            let created = Self::str_field(
                &snapshot,
                &["created_at", "created", "creation_date", "timestamp"],
            );
            let is_old = Self::parse_time(&created)
                .map(|dt| dt < Utc::now() - Duration::days(30))
                .unwrap_or(false);
            if !is_old {
                continue;
            }

            let name = Self::str_field(&snapshot, &["name", "display_name"]);
            let region = Self::str_field(&snapshot, &["region", "location", "zone"]);
            let size = Self::parse_f64(&snapshot, &["size", "size_gb", "sizeGB", "capacity"])
                .unwrap_or(0.0);
            let normalized = if size <= 0.0 { 20.0 } else { size };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "Gcore".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Snapshot".to_string(),
                details: format!(
                    "Old Gcore snapshot: {} ({:.0} GB)",
                    if name.is_empty() { id } else { name },
                    normalized
                ),
                estimated_monthly_cost: normalized * 0.02,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for GcoreScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();

        if let Ok(items) = self.scan_instances().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_volumes().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_ips().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_snapshots().await {
            results.extend(items);
        }

        Ok(results)
    }
}
