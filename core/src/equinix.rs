use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde_json::Value;

use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct EquinixScanner {
    client: Client,
    token: String,
    base_url: String,
    project_id: String,
}

impl EquinixScanner {
    pub fn new(token: &str, endpoint: &str, project_id: &str) -> Self {
        Self {
            client: Client::new(),
            token: token.trim().to_string(),
            base_url: Self::normalize_endpoint(endpoint),
            project_id: project_id.trim().to_string(),
        }
    }

    fn normalize_endpoint(raw: &str) -> String {
        let endpoint = raw.trim();
        if endpoint.is_empty() {
            return "https://api.equinix.com/metal/v1".to_string();
        }

        let mut value = endpoint.to_string();
        if !value.starts_with("http://") && !value.starts_with("https://") {
            value = format!("https://{}", value);
        }

        value = value.trim_end_matches('/').to_string();

        if value.contains("/metal/v1") || value.ends_with("/v1") {
            return value;
        }
        if value.ends_with("/metal") {
            return format!("{}/v1", value);
        }

        format!("{}/metal/v1", value)
    }

    fn value_by_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
        let mut current = value;
        for part in path.split('.') {
            match current {
                Value::Object(map) => current = map.get(part)?,
                _ => return None,
            }
        }
        Some(current)
    }

    fn str_field(value: &Value, keys: &[&str]) -> String {
        for key in keys {
            if let Some(node) = Self::value_by_path(value, key) {
                if let Some(text) = node.as_str() {
                    if !text.trim().is_empty() {
                        return text.to_string();
                    }
                } else if let Some(num) = node.as_i64() {
                    return num.to_string();
                } else if let Some(num) = node.as_u64() {
                    return num.to_string();
                }
            }
        }
        String::new()
    }

    fn parse_f64(value: &Value, keys: &[&str]) -> Option<f64> {
        for key in keys {
            if let Some(node) = Self::value_by_path(value, key) {
                if let Some(number) = node.as_f64() {
                    return Some(number);
                }
                if let Some(number) = node.as_i64() {
                    return Some(number as f64);
                }
                if let Some(number) = node.as_u64() {
                    return Some(number as f64);
                }
                if let Some(text) = node.as_str() {
                    if let Ok(parsed) = text.parse::<f64>() {
                        return Some(parsed);
                    }
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

    async fn request_json(&self, path: &str) -> Result<Value> {
        if self.token.is_empty() {
            return Err(anyhow!("Equinix Metal API token is required"));
        }

        let normalized_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{}", path)
        };

        let url = format!("{}{}", self.base_url, normalized_path);
        let response = self
            .client
            .get(&url)
            .header("X-Auth-Token", &self.token)
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(anyhow!(
                "Equinix Metal API {} failed ({}): {}",
                normalized_path,
                status.as_u16(),
                snippet
            ));
        }

        serde_json::from_str(&text).map_err(|e| anyhow!("Invalid Equinix JSON: {}", e))
    }

    async fn request_any_json(&self, paths: &[String]) -> Result<Value> {
        let mut last_err: Option<anyhow::Error> = None;
        for path in paths {
            match self.request_json(path).await {
                Ok(payload) => return Ok(payload),
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Equinix Metal API request failed")))
    }

    async fn request_any_json_strs(&self, paths: &[&str]) -> Result<Value> {
        let owned: Vec<String> = paths.iter().map(|path| path.to_string()).collect();
        self.request_any_json(&owned).await
    }

    fn parse_project_ids(payload: &Value) -> Vec<String> {
        let mut project_ids = Vec::new();
        for project in Self::extract_items(
            payload,
            &["projects", "data", "result"],
            &["projects", "items"],
        ) {
            let id = Self::str_field(&project, &["id", "project_id", "uuid"]);
            if !id.is_empty() {
                project_ids.push(id);
            }
        }
        project_ids
    }

    async fn list_project_ids(&self) -> Result<Vec<String>> {
        if !self.project_id.is_empty() {
            return Ok(vec![self.project_id.clone()]);
        }

        let payload = self
            .request_any_json_strs(&["/projects?per_page=200", "/projects"])
            .await?;

        let project_ids = Self::parse_project_ids(&payload);
        if project_ids.is_empty() {
            return Err(anyhow!("No Equinix Metal projects found for this token"));
        }

        Ok(project_ids)
    }

    pub async fn check_auth(&self) -> Result<()> {
        let _ = self.list_project_ids().await?;
        Ok(())
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let project_ids = self.list_project_ids().await?;
        let mut wastes = Vec::new();

        for project_id in project_ids {
            let paths = vec![
                format!("/projects/{}/devices?per_page=200", project_id),
                format!("/projects/{}/devices", project_id),
            ];
            let payload = match self.request_any_json(&paths).await {
                Ok(p) => p,
                Err(_) => continue,
            };

            for device in Self::extract_items(
                &payload,
                &["devices", "data", "result"],
                &["devices", "items"],
            ) {
                let state = Self::str_field(&device, &["state", "status"]).to_lowercase();
                if state.is_empty() || state.contains("active") || state.contains("running") {
                    continue;
                }

                let id = Self::str_field(&device, &["id", "short_id"]);
                if id.is_empty() {
                    continue;
                }

                let name = Self::str_field(&device, &["hostname", "name"]);
                let region = Self::str_field(
                    &device,
                    &["metro.code", "metro", "facility.code", "facility"],
                );
                let plan = Self::str_field(&device, &["plan.slug", "plan.name", "plan"]);

                wastes.push(WastedResource {
                    id: id.clone(),
                    provider: "Equinix Metal".to_string(),
                    region: if region.is_empty() {
                        "global".to_string()
                    } else {
                        region
                    },
                    resource_type: "Device".to_string(),
                    details: format!(
                        "Inactive device: {} ({})",
                        if name.is_empty() { id } else { name },
                        if plan.is_empty() { "unknown" } else { &plan }
                    ),
                    estimated_monthly_cost: 45.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        let project_ids = self.list_project_ids().await?;
        let mut wastes = Vec::new();

        for project_id in project_ids {
            let paths = vec![
                format!("/projects/{}/storage?per_page=200", project_id),
                format!("/projects/{}/volumes?per_page=200", project_id),
                format!("/projects/{}/storage", project_id),
            ];
            let payload = match self.request_any_json(&paths).await {
                Ok(p) => p,
                Err(_) => continue,
            };

            for volume in Self::extract_items(
                &payload,
                &["volumes", "storage", "data", "result"],
                &["volumes", "storage", "items"],
            ) {
                let has_attachments = Self::value_by_path(&volume, "attachments")
                    .and_then(|v| v.as_array())
                    .map(|v| !v.is_empty())
                    .unwrap_or(false);
                let attached_to = Self::str_field(
                    &volume,
                    &["attached_to", "device.id", "device_id", "instance_id"],
                );
                if has_attachments || !attached_to.is_empty() {
                    continue;
                }

                let id = Self::str_field(&volume, &["id", "volume_id"]);
                if id.is_empty() {
                    continue;
                }

                let name = Self::str_field(&volume, &["name", "description"]);
                let region = Self::str_field(
                    &volume,
                    &["metro.code", "metro", "facility.code", "facility"],
                );
                let size =
                    Self::parse_f64(&volume, &["size", "size_gib", "capacity"]).unwrap_or(100.0);

                wastes.push(WastedResource {
                    id: id.clone(),
                    provider: "Equinix Metal".to_string(),
                    region: if region.is_empty() {
                        "global".to_string()
                    } else {
                        region
                    },
                    resource_type: "Volume".to_string(),
                    details: format!(
                        "Unattached volume: {} ({:.0} GB)",
                        if name.is_empty() { id } else { name },
                        size
                    ),
                    estimated_monthly_cost: size * 0.10,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_ips(&self) -> Result<Vec<WastedResource>> {
        let project_ids = self.list_project_ids().await?;
        let mut wastes = Vec::new();

        for project_id in project_ids {
            let paths = vec![
                format!("/projects/{}/reserved-ips?per_page=200", project_id),
                format!("/projects/{}/ips?per_page=200", project_id),
                format!("/projects/{}/reserved-ips", project_id),
            ];
            let payload = match self.request_any_json(&paths).await {
                Ok(p) => p,
                Err(_) => continue,
            };

            for ip in Self::extract_items(
                &payload,
                &["reserved_ips", "ips", "data", "result"],
                &["reserved_ips", "ips", "items"],
            ) {
                let has_assignments = Self::value_by_path(&ip, "assignments")
                    .and_then(|v| v.as_array())
                    .map(|v| !v.is_empty())
                    .unwrap_or(false);
                let assigned_to =
                    Self::str_field(&ip, &["assigned_to", "instance_id", "device_id"]);
                if has_assignments || !assigned_to.is_empty() {
                    continue;
                }

                let id = Self::str_field(&ip, &["cidr", "address", "id"]);
                if id.is_empty() {
                    continue;
                }

                let region =
                    Self::str_field(&ip, &["metro.code", "metro", "facility.code", "facility"]);

                wastes.push(WastedResource {
                    id,
                    provider: "Equinix Metal".to_string(),
                    region: if region.is_empty() {
                        "global".to_string()
                    } else {
                        region
                    },
                    resource_type: "Reserved IP".to_string(),
                    details: "Unassigned reserved IP block".to_string(),
                    estimated_monthly_cost: 2.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let project_ids = self.list_project_ids().await?;
        let mut wastes = Vec::new();

        for project_id in project_ids {
            let paths = vec![
                format!("/projects/{}/volume-snapshots?per_page=200", project_id),
                format!("/projects/{}/snapshots?per_page=200", project_id),
                format!("/projects/{}/volume-snapshots", project_id),
            ];
            let payload = match self.request_any_json(&paths).await {
                Ok(p) => p,
                Err(_) => continue,
            };

            for snapshot in Self::extract_items(
                &payload,
                &["snapshots", "volume_snapshots", "data", "result"],
                &["snapshots", "volume_snapshots", "items"],
            ) {
                let id = Self::str_field(&snapshot, &["id", "snapshot_id"]);
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

                let name = Self::str_field(&snapshot, &["name", "description"]);
                let region = Self::str_field(
                    &snapshot,
                    &["metro.code", "metro", "facility.code", "facility"],
                );
                let size =
                    Self::parse_f64(&snapshot, &["size", "size_gib", "capacity"]).unwrap_or(50.0);

                wastes.push(WastedResource {
                    id: id.clone(),
                    provider: "Equinix Metal".to_string(),
                    region: if region.is_empty() {
                        "global".to_string()
                    } else {
                        region
                    },
                    resource_type: "Snapshot".to_string(),
                    details: format!(
                        "Old snapshot: {} ({:.0} GB)",
                        if name.is_empty() { id } else { name },
                        size
                    ),
                    estimated_monthly_cost: size * 0.03,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for EquinixScanner {
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
