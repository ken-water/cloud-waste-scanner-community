use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use reqwest::{Client, Method};
use serde_json::Value;
use sha1::{Digest, Sha1};

use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct OvhScanner {
    client: Client,
    application_key: String,
    application_secret: String,
    consumer_key: String,
    base_url: String,
    project_id: String,
}

impl OvhScanner {
    pub fn new(
        application_key: &str,
        application_secret: &str,
        consumer_key: &str,
        endpoint: &str,
        project_id: &str,
    ) -> Self {
        Self {
            client: Client::new(),
            application_key: application_key.to_string(),
            application_secret: application_secret.to_string(),
            consumer_key: consumer_key.to_string(),
            base_url: Self::normalize_endpoint(endpoint),
            project_id: project_id.to_string(),
        }
    }

    fn normalize_endpoint(raw: &str) -> String {
        let value = raw.trim();
        if value.is_empty() {
            return "https://eu.api.ovh.com/1.0".to_string();
        }

        let lowered = value.to_lowercase();
        match lowered.as_str() {
            "eu" | "ovh-eu" => "https://eu.api.ovh.com/1.0".to_string(),
            "ca" | "ovh-ca" => "https://ca.api.ovh.com/1.0".to_string(),
            "us" | "ovh-us" => "https://api.us.ovhcloud.com/1.0".to_string(),
            _ => {
                let mut url = value.to_string();
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    url = format!("https://{}", url);
                }
                if !url.ends_with("/1.0") {
                    url = format!("{}/1.0", url.trim_end_matches('/'));
                }
                url
            }
        }
    }

    async fn get_server_timestamp(&self) -> Result<i64> {
        let url = format!("{}/auth/time", self.base_url);
        let response = self.client.get(&url).send().await?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!(
                "OVH time API failed ({}): {}",
                status.as_u16(),
                body
            ));
        }

        body.trim()
            .parse::<i64>()
            .map_err(|e| anyhow!("Invalid OVH time response: {} ({})", body, e))
    }

    fn build_signature(&self, method: &str, full_url: &str, body: &str, timestamp: i64) -> String {
        let payload = format!(
            "{}+{}+{}+{}+{}+{}",
            self.application_secret, self.consumer_key, method, full_url, body, timestamp
        );
        let mut hasher = Sha1::new();
        hasher.update(payload.as_bytes());
        format!("$1${}", hex::encode(hasher.finalize()))
    }

    async fn signed_json_request(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        let method_name = method.as_str().to_string();
        let full_url = format!("{}{}", self.base_url, path);
        let body_string = match body {
            Some(value) => serde_json::to_string(value)?,
            None => String::new(),
        };

        let timestamp = self.get_server_timestamp().await?;
        let signature = self.build_signature(&method_name, &full_url, &body_string, timestamp);

        let mut request = self
            .client
            .request(method, &full_url)
            .header("X-Ovh-Application", &self.application_key)
            .header("X-Ovh-Consumer", &self.consumer_key)
            .header("X-Ovh-Timestamp", timestamp.to_string())
            .header("X-Ovh-Signature", signature)
            .header("Content-Type", "application/json");

        if !body_string.is_empty() {
            request = request.body(body_string.clone());
        }

        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(anyhow!(
                "OVH API {} {} failed ({}): {}",
                method_name,
                path,
                status.as_u16(),
                snippet
            ));
        }

        if text.trim().is_empty() {
            return Ok(Value::Null);
        }

        serde_json::from_str(&text).map_err(|e| anyhow!("Invalid OVH JSON response: {}", e))
    }

    fn as_items(value: Value) -> Vec<Value> {
        if let Some(items) = value.as_array() {
            return items.to_vec();
        }
        if let Some(items) = value.get("items").and_then(|v| v.as_array()) {
            return items.to_vec();
        }
        vec![]
    }

    fn get_string_field(value: &Value, candidates: &[&str]) -> String {
        for key in candidates {
            if let Some(text) = value.get(*key).and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    return text.to_string();
                }
            }
        }
        String::new()
    }

    fn get_f64_field(value: &Value, candidates: &[&str]) -> Option<f64> {
        for key in candidates {
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

    async fn resolve_project_id(&self) -> Result<String> {
        if !self.project_id.trim().is_empty() {
            return Ok(self.project_id.clone());
        }

        let projects = self.list_projects().await?;
        projects
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No OVH Public Cloud project found. Please set project_id."))
    }

    pub async fn list_projects(&self) -> Result<Vec<String>> {
        let response = self
            .signed_json_request(Method::GET, "/cloud/project", None)
            .await?;
        let mut projects = Vec::new();

        for item in Self::as_items(response) {
            if let Some(text) = item.as_str() {
                if !text.trim().is_empty() {
                    projects.push(text.to_string());
                }
                continue;
            }

            let service_name = Self::get_string_field(&item, &["project_id", "serviceName", "id"]);
            if !service_name.is_empty() {
                projects.push(service_name);
            }
        }

        Ok(projects)
    }

    pub async fn check_connection(&self) -> Result<String> {
        let projects = self.list_projects().await?;
        if projects.is_empty() {
            return Err(anyhow!(
                "Authenticated, but no OVH Public Cloud project is visible"
            ));
        }

        if self.project_id.trim().is_empty() {
            return Ok(projects[0].clone());
        }

        if projects.iter().any(|id| id == self.project_id.trim()) {
            return Ok(self.project_id.clone());
        }

        Err(anyhow!(
            "Configured project_id '{}' not found in account",
            self.project_id
        ))
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let project_id = self.resolve_project_id().await?;
        let path = format!("/cloud/project/{}/instance", project_id);
        let response = self.signed_json_request(Method::GET, &path, None).await?;
        let mut wastes = Vec::new();

        for item in Self::as_items(response) {
            let status = Self::get_string_field(&item, &["status", "state"]).to_lowercase();
            let is_idle = status.contains("stop")
                || status.contains("shelv")
                || status.contains("pause")
                || status.contains("error");
            if !is_idle {
                continue;
            }

            let id = Self::get_string_field(&item, &["id", "instanceId"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::get_string_field(&item, &["name"]);
            let region = Self::get_string_field(&item, &["region"]);
            let flavor = Self::get_string_field(&item, &["flavorId", "flavor"]);

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "OVHcloud".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Instance".to_string(),
                details: format!(
                    "Idle OVH instance: {} ({}, flavor {})",
                    if name.is_empty() { id } else { name },
                    status,
                    if flavor.is_empty() { "n/a" } else { &flavor }
                ),
                estimated_monthly_cost: 10.0,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        let project_id = self.resolve_project_id().await?;
        let path = format!("/cloud/project/{}/volume", project_id);
        let response = self.signed_json_request(Method::GET, &path, None).await?;
        let mut wastes = Vec::new();

        for item in Self::as_items(response) {
            let attached_count = item
                .get("attachedTo")
                .and_then(|v| v.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            if attached_count > 0 {
                continue;
            }

            let id = Self::get_string_field(&item, &["id", "volumeId"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::get_string_field(&item, &["name"]);
            let region = Self::get_string_field(&item, &["region"]);
            let size_value = Self::get_f64_field(&item, &["size", "sizeGb"]).unwrap_or(0.0);
            let size_gb = if size_value > 1024.0 {
                size_value / 1024.0 / 1024.0 / 1024.0
            } else {
                size_value
            };
            let normalized_size = if size_gb <= 0.0 { 50.0 } else { size_gb };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "OVHcloud".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Volume".to_string(),
                details: format!(
                    "Unattached OVH volume: {} ({:.0} GB)",
                    if name.is_empty() { id } else { name },
                    normalized_size
                ),
                estimated_monthly_cost: normalized_size * 0.08,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_ips(&self) -> Result<Vec<WastedResource>> {
        let project_id = self.resolve_project_id().await?;
        let path = format!("/cloud/project/{}/ip", project_id);
        let response = self.signed_json_request(Method::GET, &path, None).await?;
        let mut wastes = Vec::new();

        for item in Self::as_items(response) {
            let routed_to = item.get("routedTo");
            let has_target = routed_to
                .and_then(|v| v.as_str())
                .map(|v| !v.is_empty())
                .unwrap_or_else(|| {
                    routed_to
                        .and_then(|v| v.as_array())
                        .map(|arr| !arr.is_empty())
                        .unwrap_or(false)
                });
            if has_target {
                continue;
            }

            let id = Self::get_string_field(&item, &["ip", "id"]);
            if id.is_empty() {
                continue;
            }

            let region = Self::get_string_field(&item, &["region"]);
            wastes.push(WastedResource {
                id,
                provider: "OVHcloud".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Public IP".to_string(),
                details: "Unrouted OVH Public IP".to_string(),
                estimated_monthly_cost: 2.0,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let project_id = self.resolve_project_id().await?;
        let path = format!("/cloud/project/{}/snapshot", project_id);
        let response = self.signed_json_request(Method::GET, &path, None).await?;
        let mut wastes = Vec::new();

        for item in Self::as_items(response) {
            let id = Self::get_string_field(&item, &["id", "snapshotId"]);
            if id.is_empty() {
                continue;
            }

            let created_raw = Self::get_string_field(
                &item,
                &["creationDate", "createdAt", "created_at", "creation_date"],
            );
            let is_old = chrono::DateTime::parse_from_rfc3339(&created_raw)
                .map(|dt| dt.with_timezone(&Utc) < Utc::now() - Duration::days(30))
                .unwrap_or(false);
            if !is_old {
                continue;
            }

            let region = Self::get_string_field(&item, &["region"]);
            let size_value = Self::get_f64_field(&item, &["size", "sizeGb"]).unwrap_or(10.0);
            let size_gb = if size_value > 1024.0 {
                size_value / 1024.0 / 1024.0 / 1024.0
            } else {
                size_value
            };
            let normalized_size = if size_gb <= 0.0 { 10.0 } else { size_gb };

            wastes.push(WastedResource {
                id,
                provider: "OVHcloud".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Snapshot".to_string(),
                details: format!("Old OVH snapshot ({:.0} GB)", normalized_size),
                estimated_monthly_cost: normalized_size * 0.03,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for OvhScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_instances().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_volumes().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_ips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_snapshots().await {
            results.extend(r);
        }
        Ok(results)
    }
}
