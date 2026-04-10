use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde_json::Value;

use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct UpcloudScanner {
    client: Client,
    username: String,
    password: String,
    base_url: String,
}

impl UpcloudScanner {
    pub fn new(username: &str, password: &str, endpoint: &str) -> Self {
        Self {
            client: Client::new(),
            username: username.trim().to_string(),
            password: password.trim().to_string(),
            base_url: Self::normalize_endpoint(endpoint),
        }
    }

    fn normalize_endpoint(raw: &str) -> String {
        let endpoint = raw.trim();
        if endpoint.is_empty() {
            return "https://api.upcloud.com/1.3".to_string();
        }

        let mut value = endpoint.to_string();
        if !value.starts_with("http://") && !value.starts_with("https://") {
            value = format!("https://{}", value);
        }

        if value.contains("/1.") {
            return value.trim_end_matches('/').to_string();
        }

        format!("{}/1.3", value.trim_end_matches('/'))
    }

    async fn request_json(&self, path: &str) -> Result<Value> {
        if self.username.is_empty() || self.password.is_empty() {
            return Err(anyhow!("UpCloud username and password are required"));
        }

        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(anyhow!(
                "UpCloud API {} failed ({}): {}",
                path,
                status.as_u16(),
                snippet
            ));
        }

        serde_json::from_str(&text).map_err(|e| anyhow!("Invalid UpCloud JSON: {}", e))
    }

    async fn request_any_json(&self, paths: &[&str]) -> Result<Value> {
        let mut last_err: Option<anyhow::Error> = None;
        for path in paths {
            match self.request_json(path).await {
                Ok(payload) => return Ok(payload),
                Err(err) => last_err = Some(err),
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("UpCloud API request failed")))
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
        self.request_any_json(&["/account", "/server?limit=1", "/server"])
            .await?;
        Ok(())
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let payload = self.request_any_json(&["/server", "/servers"]).await?;
        let mut wastes = Vec::new();

        for server in Self::extract_items(&payload, &["servers", "server"], &["server", "servers"])
        {
            let state = Self::str_field(&server, &["state", "status"]).to_lowercase();
            let stopped = state.contains("stop")
                || state.contains("off")
                || state.contains("halt")
                || state.contains("inactive");
            if !stopped {
                continue;
            }

            let id = Self::str_field(&server, &["uuid", "id"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::str_field(&server, &["hostname", "title", "name"]);
            let region = Self::str_field(&server, &["zone", "location"]);
            let plan = Self::str_field(&server, &["plan", "server_plan"]);

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "UpCloud".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Instance".to_string(),
                details: format!(
                    "Stopped UpCloud instance: {} ({})",
                    if name.is_empty() { id } else { name },
                    if plan.is_empty() { "unknown" } else { &plan }
                ),
                estimated_monthly_cost: 10.0,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        let payload = self.request_any_json(&["/storage", "/storages"]).await?;
        let mut wastes = Vec::new();

        for storage in
            Self::extract_items(&payload, &["storages", "storage"], &["storage", "storages"])
        {
            let storage_type = Self::str_field(&storage, &["type", "storage_type"]).to_lowercase();
            if storage_type.contains("backup") || storage_type.contains("snapshot") {
                continue;
            }

            let attached_to = Self::str_field(&storage, &["server", "server_uuid", "server_id"]);
            if !attached_to.is_empty() {
                continue;
            }

            let id = Self::str_field(&storage, &["uuid", "id"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::str_field(&storage, &["title", "name"]);
            let region = Self::str_field(&storage, &["zone", "location"]);
            let size =
                Self::parse_f64(&storage, &["size", "size_gib", "storage_size"]).unwrap_or(0.0);
            let normalized = if size <= 0.0 { 20.0 } else { size };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "UpCloud".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Volume".to_string(),
                details: format!(
                    "Unattached UpCloud volume: {} ({:.0} GB)",
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
            .request_any_json(&["/ip_address", "/ip_addresses", "/ip-addresses"])
            .await?;
        let mut wastes = Vec::new();

        for ip in Self::extract_items(
            &payload,
            &["ip_addresses", "ip_address", "ipAddresses"],
            &["ip_address", "ip", "address"],
        ) {
            let attached_to =
                Self::str_field(&ip, &["server", "server_uuid", "server_id", "resource"]);
            if !attached_to.is_empty() {
                continue;
            }

            let address = Self::str_field(&ip, &["address", "ip", "id"]);
            if address.is_empty() {
                continue;
            }

            let access = Self::str_field(&ip, &["access"]);
            if !access.is_empty() && access.to_lowercase() != "public" {
                continue;
            }

            let region = Self::str_field(&ip, &["zone", "location"]);

            wastes.push(WastedResource {
                id: address,
                provider: "UpCloud".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Public IP".to_string(),
                details: "Unassigned UpCloud public IP".to_string(),
                estimated_monthly_cost: 1.5,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_any_json(&["/storage", "/storages", "/backup", "/backups"])
            .await?;
        let mut wastes = Vec::new();

        let items = Self::extract_items(
            &payload,
            &["storages", "storage", "backups", "backup"],
            &["storage", "backup", "storages", "backups"],
        );

        for snapshot in items {
            let storage_type = Self::str_field(&snapshot, &["type", "storage_type"]).to_lowercase();
            let looks_like_snapshot = storage_type.contains("backup")
                || storage_type.contains("snapshot")
                || snapshot.get("backup_of").is_some();
            if !looks_like_snapshot {
                continue;
            }

            let id = Self::str_field(&snapshot, &["uuid", "id"]);
            if id.is_empty() {
                continue;
            }

            let created =
                Self::str_field(&snapshot, &["created", "create_time", "timestamp", "time"]);
            let is_old = Self::parse_time(&created)
                .map(|dt| dt < Utc::now() - Duration::days(30))
                .unwrap_or(false);
            if !is_old {
                continue;
            }

            let name = Self::str_field(&snapshot, &["title", "name"]);
            let region = Self::str_field(&snapshot, &["zone", "location"]);
            let size =
                Self::parse_f64(&snapshot, &["size", "size_gib", "storage_size"]).unwrap_or(0.0);
            let normalized = if size <= 0.0 { 20.0 } else { size };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "UpCloud".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Snapshot".to_string(),
                details: format!(
                    "Old UpCloud snapshot: {} ({:.0} GB)",
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
impl CloudProvider for UpcloudScanner {
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
