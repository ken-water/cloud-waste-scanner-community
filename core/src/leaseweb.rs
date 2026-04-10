use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde_json::Value;

use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct LeasewebScanner {
    client: Client,
    token: String,
    base_url: String,
}

impl LeasewebScanner {
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
            return "https://api.leaseweb.com/hosting/v2".to_string();
        }

        let mut value = endpoint.to_string();
        if !value.starts_with("http://") && !value.starts_with("https://") {
            value = format!("https://{}", value);
        }

        if value.contains("/hosting/v") {
            return value.trim_end_matches('/').to_string();
        }

        format!("{}/hosting/v2", value.trim_end_matches('/'))
    }

    async fn request_json(&self, path: &str) -> Result<Value> {
        if self.token.is_empty() {
            return Err(anyhow!("Leaseweb API token is required"));
        }

        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .header("x-lsw-auth", &self.token)
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(anyhow!(
                "Leaseweb API {} failed ({}): {}",
                path,
                status.as_u16(),
                snippet
            ));
        }

        serde_json::from_str(&text).map_err(|e| anyhow!("Invalid Leaseweb JSON: {}", e))
    }

    async fn request_any_json(&self, paths: &[&str]) -> Result<Value> {
        let mut last_err: Option<anyhow::Error> = None;

        for path in paths {
            match self.request_json(path).await {
                Ok(payload) => return Ok(payload),
                Err(err) => {
                    last_err = Some(err);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Leaseweb API request failed")))
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

    fn parse_bool(value: &Value, key: &str) -> Option<bool> {
        if let Some(boolean) = value.get(key).and_then(|v| v.as_bool()) {
            return Some(boolean);
        }

        value.get(key).and_then(|v| v.as_str()).map(|raw| {
            let normalized = raw.trim().to_lowercase();
            normalized == "true" || normalized == "yes" || normalized == "1"
        })
    }

    fn extract_items(payload: &Value, keys: &[&str]) -> Vec<Value> {
        if let Some(array) = payload.as_array() {
            return array.iter().cloned().collect();
        }

        for key in keys {
            if let Some(array) = payload.get(*key).and_then(|v| v.as_array()) {
                return array.iter().cloned().collect();
            }
        }

        if let Some(data) = payload.get("data") {
            if let Some(array) = data.as_array() {
                return array.iter().cloned().collect();
            }

            for key in keys {
                if let Some(array) = data.get(*key).and_then(|v| v.as_array()) {
                    return array.iter().cloned().collect();
                }
            }
        }

        if let Some(items) = payload.get("items").and_then(|v| v.as_array()) {
            return items.iter().cloned().collect();
        }

        Vec::new()
    }

    fn bytes_to_gb(size: f64) -> f64 {
        if size > 1024.0 * 1024.0 * 1024.0 {
            return size / 1024.0 / 1024.0 / 1024.0;
        }

        size
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

    pub async fn check_auth(&self) -> Result<()> {
        self.request_any_json(&[
            "/virtualServers?limit=1",
            "/servers?limit=1",
            "/instances?limit=1",
        ])
        .await?;
        Ok(())
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_any_json(&[
                "/virtualServers?limit=200",
                "/servers?limit=200",
                "/instances?limit=200",
            ])
            .await?;

        let mut wastes = Vec::new();
        for instance in Self::extract_items(&payload, &["virtualServers", "servers", "instances"]) {
            let status =
                Self::str_field(&instance, &["state", "status", "powerState"]).to_lowercase();
            let stopped = status.contains("stop")
                || status.contains("off")
                || status.contains("shut")
                || status == "inactive";
            if !stopped {
                continue;
            }

            let id = Self::str_field(&instance, &["id", "instanceId", "serverId"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::str_field(&instance, &["name", "displayName"]);
            let region = Self::str_field(&instance, &["region", "location", "zone", "datacenter"]);
            let flavor = Self::str_field(&instance, &["flavor", "plan", "machineType"]);

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "Leaseweb".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Instance".to_string(),
                details: format!(
                    "Stopped Leaseweb instance: {} ({})",
                    if name.is_empty() { id } else { name },
                    if flavor.is_empty() {
                        "unknown"
                    } else {
                        &flavor
                    }
                ),
                estimated_monthly_cost: 11.0,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_any_json(&[
                "/volumes?limit=200",
                "/blockStorage/volumes?limit=200",
                "/storage/volumes?limit=200",
            ])
            .await?;

        let mut wastes = Vec::new();
        for volume in Self::extract_items(&payload, &["volumes", "blockStorageVolumes"]) {
            let attached_instance = !Self::str_field(
                &volume,
                &["instanceId", "virtualServerId", "serverId", "attachedTo"],
            )
            .is_empty();
            let attached_list = volume
                .get("attachments")
                .and_then(|v| v.as_array())
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            let attached_state = Self::parse_bool(&volume, "attached").unwrap_or(false);

            if attached_instance || attached_list || attached_state {
                continue;
            }

            let id = Self::str_field(&volume, &["id", "volumeId"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::str_field(&volume, &["name", "displayName"]);
            let region = Self::str_field(&volume, &["region", "location", "zone", "datacenter"]);
            let size_raw =
                Self::parse_f64(&volume, &["sizeGb", "sizeGB", "size", "capacity"]).unwrap_or(0.0);
            let size_gb = Self::bytes_to_gb(size_raw);
            let normalized = if size_gb <= 0.0 { 20.0 } else { size_gb };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "Leaseweb".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Volume".to_string(),
                details: format!(
                    "Unattached Leaseweb volume: {} ({:.0} GB)",
                    if name.is_empty() { id } else { name },
                    normalized
                ),
                estimated_monthly_cost: normalized * 0.07,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_public_ips(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_any_json(&[
                "/ipAddresses?limit=200",
                "/ips?limit=200",
                "/publicIps?limit=200",
            ])
            .await?;

        let mut wastes = Vec::new();
        for ip in Self::extract_items(&payload, &["ipAddresses", "ips", "publicIps"]) {
            let assigned_target = !Self::str_field(
                &ip,
                &[
                    "instanceId",
                    "virtualServerId",
                    "serverId",
                    "assignedTo",
                    "resourceId",
                ],
            )
            .is_empty();
            let in_use = Self::parse_bool(&ip, "inUse")
                .or_else(|| Self::parse_bool(&ip, "assigned"))
                .unwrap_or(false);

            if assigned_target || in_use {
                continue;
            }

            let id = Self::str_field(&ip, &["address", "ip", "id"]);
            if id.is_empty() {
                continue;
            }

            let region = Self::str_field(&ip, &["region", "location", "zone", "datacenter"]);

            wastes.push(WastedResource {
                id,
                provider: "Leaseweb".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Public IP".to_string(),
                details: "Unassigned Leaseweb public IP".to_string(),
                estimated_monthly_cost: 1.8,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_any_json(&[
                "/snapshots?limit=200",
                "/volumeSnapshots?limit=200",
                "/backupSnapshots?limit=200",
            ])
            .await?;

        let mut wastes = Vec::new();
        for snapshot in Self::extract_items(
            &payload,
            &["snapshots", "volumeSnapshots", "backupSnapshots"],
        ) {
            let id = Self::str_field(&snapshot, &["id", "snapshotId"]);
            if id.is_empty() {
                continue;
            }

            let created =
                Self::str_field(&snapshot, &["createdAt", "created", "creationDate", "date"]);
            let is_old = Self::parse_time(&created)
                .map(|dt| dt < Utc::now() - Duration::days(30))
                .unwrap_or(false);
            if !is_old {
                continue;
            }

            let name = Self::str_field(&snapshot, &["name", "displayName"]);
            let region = Self::str_field(&snapshot, &["region", "location", "zone", "datacenter"]);
            let size_raw = Self::parse_f64(&snapshot, &["sizeGb", "sizeGB", "size", "capacity"])
                .unwrap_or(0.0);
            let size_gb = Self::bytes_to_gb(size_raw);
            let normalized = if size_gb <= 0.0 { 20.0 } else { size_gb };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "Leaseweb".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Snapshot".to_string(),
                details: format!(
                    "Old Leaseweb snapshot: {} ({:.0} GB)",
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
impl CloudProvider for LeasewebScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();

        if let Ok(items) = self.scan_instances().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_volumes().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_public_ips().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_snapshots().await {
            results.extend(items);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_endpoint_applies_defaults_and_version_suffix() {
        assert_eq!(
            LeasewebScanner::normalize_endpoint(""),
            "https://api.leaseweb.com/hosting/v2"
        );
        assert_eq!(
            LeasewebScanner::normalize_endpoint("api.leaseweb.com"),
            "https://api.leaseweb.com/hosting/v2"
        );
        assert_eq!(
            LeasewebScanner::normalize_endpoint("https://api.leaseweb.com/hosting/v1/"),
            "https://api.leaseweb.com/hosting/v1"
        );
    }

    #[test]
    fn parser_helpers_cover_string_number_and_nested_arrays() {
        let payload = json!({
            "size": "12.5",
            "enabled": "yes",
            "data": { "items": [ {"id":"a"}, {"id":"b"} ] }
        });
        assert_eq!(LeasewebScanner::parse_f64(&payload, &["size"]), Some(12.5));
        assert_eq!(LeasewebScanner::parse_bool(&payload, "enabled"), Some(true));
        let items = LeasewebScanner::extract_items(&payload, &["items"]);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn parse_time_and_bytes_to_gb_handle_edge_inputs() {
        assert!(LeasewebScanner::parse_time("2026-03-17T12:30:00Z").is_some());
        assert!(LeasewebScanner::parse_time("2026-03-17 12:30:00").is_none());
        assert!(LeasewebScanner::parse_time(" ").is_none());
        assert!(LeasewebScanner::bytes_to_gb(1_073_741_825.0) > 1.0);
        assert_eq!(LeasewebScanner::bytes_to_gb(50.0), 50.0);
    }
}
