use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashSet;

use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct IonosScanner {
    client: Client,
    token: String,
    base_url: String,
}

impl IonosScanner {
    pub fn new(token: &str, endpoint: &str) -> Self {
        Self {
            client: Client::new(),
            token: token.to_string(),
            base_url: Self::normalize_endpoint(endpoint),
        }
    }

    fn normalize_endpoint(raw: &str) -> String {
        let endpoint = raw.trim();
        if endpoint.is_empty() {
            return "https://api.ionos.com/cloudapi/v6".to_string();
        }

        let mut value = endpoint.to_string();
        if !value.starts_with("http://") && !value.starts_with("https://") {
            value = format!("https://{}", value);
        }

        if value.contains("/cloudapi/v") {
            return value.trim_end_matches('/').to_string();
        }

        format!("{}/cloudapi/v6", value.trim_end_matches('/'))
    }

    async fn request_json(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(anyhow!(
                "IONOS API {} failed ({}): {}",
                path,
                status.as_u16(),
                snippet
            ));
        }

        serde_json::from_str(&text).map_err(|e| anyhow!("Invalid IONOS JSON: {}", e))
    }

    fn items(value: &Value) -> Vec<Value> {
        value
            .get("items")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().cloned().collect())
            .unwrap_or_default()
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
            if let Some(text) = value.get(*key).and_then(|v| v.as_str()) {
                if let Ok(parsed) = text.parse::<f64>() {
                    return Some(parsed);
                }
            }
        }
        None
    }

    async fn datacenter_ids(&self) -> Result<Vec<String>> {
        let payload = self.request_json("/datacenters?depth=1").await?;
        let mut ids = Vec::new();

        for item in Self::items(&payload) {
            let id = Self::str_field(&item, &["id"]);
            if !id.is_empty() {
                ids.push(id);
            }
        }

        Ok(ids)
    }

    pub async fn check_auth(&self) -> Result<()> {
        let datacenters = self.datacenter_ids().await?;
        if datacenters.is_empty() {
            return Err(anyhow!("Authenticated, but no IONOS datacenter is visible"));
        }
        Ok(())
    }

    pub async fn scan_servers(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();

        for dc_id in self.datacenter_ids().await? {
            let path = format!("/datacenters/{}/servers?depth=1", dc_id);
            let payload = self.request_json(&path).await?;

            for server in Self::items(&payload) {
                let properties = server.get("properties").cloned().unwrap_or(Value::Null);
                let state =
                    Self::str_field(&properties, &["vmState", "status", "state"]).to_lowercase();
                let stopped = state.contains("shut")
                    || state.contains("off")
                    || state.contains("stop")
                    || state.contains("inactive");
                if !stopped {
                    continue;
                }

                let id = Self::str_field(&server, &["id"]);
                if id.is_empty() {
                    continue;
                }

                let name = Self::str_field(&properties, &["name"]);
                let cores = Self::parse_f64(&properties, &["cores"]).unwrap_or(0.0);
                let ram = Self::parse_f64(&properties, &["ram"]).unwrap_or(0.0);

                wastes.push(WastedResource {
                    id: id.clone(),
                    provider: "IONOS".to_string(),
                    region: dc_id.clone(),
                    resource_type: "Instance".to_string(),
                    details: format!(
                        "Stopped IONOS instance: {} ({} cores / {:.0} MB RAM)",
                        if name.is_empty() { id } else { name },
                        cores as i64,
                        ram
                    ),
                    estimated_monthly_cost: 12.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();

        for dc_id in self.datacenter_ids().await? {
            let server_path = format!("/datacenters/{}/servers?depth=1", dc_id);
            let server_payload = self.request_json(&server_path).await?;

            let mut attached_ids = HashSet::new();
            for server in Self::items(&server_payload) {
                let server_id = Self::str_field(&server, &["id"]);
                if server_id.is_empty() {
                    continue;
                }

                let attach_path = format!(
                    "/datacenters/{}/servers/{}/volumes?depth=1",
                    dc_id, server_id
                );
                if let Ok(attach_payload) = self.request_json(&attach_path).await {
                    for volume in Self::items(&attach_payload) {
                        let volume_id = Self::str_field(&volume, &["id"]);
                        if !volume_id.is_empty() {
                            attached_ids.insert(volume_id);
                        }
                    }
                }
            }

            let volume_path = format!("/datacenters/{}/volumes?depth=1", dc_id);
            let volume_payload = self.request_json(&volume_path).await?;
            for volume in Self::items(&volume_payload) {
                let id = Self::str_field(&volume, &["id"]);
                if id.is_empty() || attached_ids.contains(&id) {
                    continue;
                }

                let properties = volume.get("properties").cloned().unwrap_or(Value::Null);
                let name = Self::str_field(&properties, &["name"]);
                let size = Self::parse_f64(&properties, &["size"]).unwrap_or(10.0);
                let normalized = if size <= 0.0 { 10.0 } else { size };

                wastes.push(WastedResource {
                    id: id.clone(),
                    provider: "IONOS".to_string(),
                    region: dc_id.clone(),
                    resource_type: "Volume".to_string(),
                    details: format!(
                        "Unattached IONOS volume: {} ({:.0} GB)",
                        if name.is_empty() { id } else { name },
                        normalized
                    ),
                    estimated_monthly_cost: normalized * 0.06,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_ipblocks(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let payload = self.request_json("/ipblocks?depth=1").await?;

        for block in Self::items(&payload) {
            let id = Self::str_field(&block, &["id"]);
            if id.is_empty() {
                continue;
            }

            let properties = block.get("properties").cloned().unwrap_or(Value::Null);
            let ips_len = properties
                .get("ips")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            if ips_len > 0 {
                continue;
            }

            let name = Self::str_field(&properties, &["name"]);
            let location = Self::str_field(&properties, &["location"]);

            wastes.push(WastedResource {
                id,
                provider: "IONOS".to_string(),
                region: if location.is_empty() {
                    "global".to_string()
                } else {
                    location
                },
                resource_type: "IP Block".to_string(),
                details: format!(
                    "Unused IONOS IP block{}",
                    if name.is_empty() {
                        "".to_string()
                    } else {
                        format!(": {}", name)
                    }
                ),
                estimated_monthly_cost: 2.0,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let payload = self.request_json("/snapshots?depth=1").await?;

        for snapshot in Self::items(&payload) {
            let id = Self::str_field(&snapshot, &["id"]);
            if id.is_empty() {
                continue;
            }

            let properties = snapshot.get("properties").cloned().unwrap_or(Value::Null);
            let metadata = snapshot.get("metadata").cloned().unwrap_or(Value::Null);
            let created_at = Self::str_field(&metadata, &["createdDate"]);
            let is_old = chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&Utc) < Utc::now() - Duration::days(30))
                .unwrap_or(false);
            if !is_old {
                continue;
            }

            let name = Self::str_field(&properties, &["name"]);
            let size = Self::parse_f64(&properties, &["size"]).unwrap_or(20.0);
            let normalized = if size <= 0.0 { 20.0 } else { size };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "IONOS".to_string(),
                region: "global".to_string(),
                resource_type: "Snapshot".to_string(),
                details: format!(
                    "Old IONOS snapshot: {} ({:.0} GB)",
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
impl CloudProvider for IonosScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_servers().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_volumes().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_ipblocks().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_snapshots().await {
            results.extend(r);
        }
        Ok(results)
    }
}
