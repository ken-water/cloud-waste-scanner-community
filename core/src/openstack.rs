use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde_json::Value;

use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct OpenstackScanner {
    client: Client,
    token: String,
    base_url: String,
    project_id: String,
}

impl OpenstackScanner {
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
            return "https://openstack.example.com/v2".to_string();
        }

        let mut value = endpoint.to_string();
        if !value.starts_with("http://") && !value.starts_with("https://") {
            value = format!("https://{}", value);
        }

        value = value.trim_end_matches('/').to_string();

        if value.contains("/v2") {
            return value;
        }

        format!("{}/v2", value)
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
                } else if let Some(number) = node.as_i64() {
                    return number.to_string();
                } else if let Some(number) = node.as_u64() {
                    return number.to_string();
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

    fn scoped_paths(&self, suffixes: &[&str]) -> Vec<String> {
        let mut paths = Vec::new();

        for suffix in suffixes {
            let normalized = if suffix.starts_with('/') {
                (*suffix).to_string()
            } else {
                format!("/{}", suffix)
            };

            if !self.project_id.is_empty() {
                paths.push(format!("/{}{}", self.project_id, normalized));
            }
            paths.push(normalized);
        }

        paths
    }

    async fn request_json(&self, path: &str) -> Result<Value> {
        if self.token.is_empty() {
            return Err(anyhow!("OpenStack API token is required"));
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
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(anyhow!(
                "OpenStack API {} failed ({}): {}",
                normalized_path,
                status.as_u16(),
                snippet
            ));
        }

        serde_json::from_str(&text).map_err(|e| anyhow!("Invalid OpenStack JSON: {}", e))
    }

    async fn request_any_json(&self, paths: &[String]) -> Result<Value> {
        let mut last_err: Option<anyhow::Error> = None;

        for path in paths {
            match self.request_json(path).await {
                Ok(payload) => return Ok(payload),
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("OpenStack API request failed")))
    }

    fn bytes_to_gb(size: f64) -> f64 {
        if size > 1024.0 * 1024.0 * 1024.0 {
            return size / 1024.0 / 1024.0 / 1024.0;
        }

        size
    }

    pub async fn check_auth(&self) -> Result<()> {
        let paths = self.scoped_paths(&[
            "/servers/detail?limit=1",
            "/servers?limit=1",
            "/servers/detail",
            "/servers",
        ]);
        self.request_any_json(&paths).await?;
        Ok(())
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let paths = self.scoped_paths(&[
            "/servers/detail?limit=200",
            "/servers?limit=200",
            "/servers/detail",
            "/servers",
        ]);
        let payload = self.request_any_json(&paths).await?;

        let mut wastes = Vec::new();
        for server in Self::extract_items(
            &payload,
            &["servers", "data", "result"],
            &["servers", "items"],
        ) {
            let status = Self::str_field(
                &server,
                &["status", "server.status", "state", "OS-EXT-STS:vm_state"],
            )
            .to_lowercase();
            let stopped = status.contains("shut")
                || status.contains("stop")
                || status.contains("off")
                || status.contains("suspend")
                || status.contains("inactive");
            if !stopped {
                continue;
            }

            let id = Self::str_field(&server, &["id"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::str_field(&server, &["name", "metadata.name"]);
            let region = Self::str_field(
                &server,
                &["OS-EXT-AZ:availability_zone", "availability_zone", "region"],
            );
            let flavor = Self::str_field(&server, &["flavor.id", "flavor.name", "flavorRef"]);

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "OpenStack".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Instance".to_string(),
                details: format!(
                    "Stopped OpenStack instance: {} ({})",
                    if name.is_empty() { id } else { name },
                    if flavor.is_empty() {
                        "unknown"
                    } else {
                        &flavor
                    }
                ),
                estimated_monthly_cost: 18.0,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        let paths = self.scoped_paths(&[
            "/os-volumes/detail?limit=200",
            "/os-volumes?limit=200",
            "/volumes/detail?limit=200",
            "/volumes?limit=200",
        ]);
        let payload = self.request_any_json(&paths).await?;

        let mut wastes = Vec::new();
        for volume in Self::extract_items(
            &payload,
            &["volumes", "data", "result"],
            &["volumes", "items"],
        ) {
            let status =
                Self::str_field(&volume, &["status", "state", "volume.status"]).to_lowercase();
            let detached = status.contains("available")
                || status.contains("detached")
                || status.contains("unused");
            if !detached {
                continue;
            }

            let has_attachments = Self::value_by_path(&volume, "attachments")
                .and_then(|v| v.as_array())
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            if has_attachments {
                continue;
            }

            let id = Self::str_field(&volume, &["id", "volume_id"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::str_field(&volume, &["name", "display_name"]);
            let region = Self::str_field(
                &volume,
                &["availability_zone", "os-vol-host-attr:host", "region"],
            );
            let size =
                Self::parse_f64(&volume, &["size", "volume.size", "capacity"]).unwrap_or(50.0);
            let normalized = if size <= 0.0 { 50.0 } else { size };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "OpenStack".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Volume".to_string(),
                details: format!(
                    "Unattached OpenStack volume: {} ({:.0} GB)",
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
        let paths =
            self.scoped_paths(&["/os-floating-ips", "/floatingips", "/os-floating-ips-bulk"]);
        let payload = self.request_any_json(&paths).await?;

        let mut wastes = Vec::new();
        for ip in Self::extract_items(
            &payload,
            &["floating_ips", "floatingips", "addresses", "data", "result"],
            &["floating_ips", "floatingips", "items", "public"],
        ) {
            let assigned_to = Self::str_field(
                &ip,
                &[
                    "instance_id",
                    "port_id",
                    "fixed_ip",
                    "fixed_ip_address",
                    "server_id",
                ],
            );
            let has_instance_object = Self::value_by_path(&ip, "instance").is_some();
            if !assigned_to.is_empty() || has_instance_object {
                continue;
            }

            let id = Self::str_field(&ip, &["ip", "floating_ip_address", "address", "id"]);
            if id.is_empty() {
                continue;
            }

            let region = Self::str_field(&ip, &["pool", "region", "availability_zone"]);

            wastes.push(WastedResource {
                id,
                provider: "OpenStack".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Public IP".to_string(),
                details: "Unassigned OpenStack floating IP".to_string(),
                estimated_monthly_cost: 2.5,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let paths = self.scoped_paths(&[
            "/images/detail?limit=200&type=snapshot",
            "/images/detail?limit=200",
            "/images?limit=200",
        ]);
        let payload = self.request_any_json(&paths).await?;

        let mut wastes = Vec::new();
        for image in Self::extract_items(
            &payload,
            &["images", "data", "result"],
            &["images", "items"],
        ) {
            let image_type =
                Self::str_field(&image, &["metadata.image_type", "image_type", "type"])
                    .to_lowercase();
            if !image_type.is_empty()
                && !(image_type.contains("snapshot") || image_type.contains("backup"))
            {
                continue;
            }

            let id = Self::str_field(&image, &["id"]);
            if id.is_empty() {
                continue;
            }

            let created = Self::str_field(&image, &["created", "created_at", "updated"]);
            let is_old = Self::parse_time(&created)
                .map(|dt| dt < Utc::now() - Duration::days(30))
                .unwrap_or(false);
            if !is_old {
                continue;
            }

            let name = Self::str_field(&image, &["name"]);
            let region = Self::str_field(&image, &["region", "availability_zone"]);
            let raw_size = Self::parse_f64(&image, &["OS-EXT-IMG-SIZE:size", "size", "minDisk"])
                .unwrap_or(10.0);
            let size_gb = Self::bytes_to_gb(raw_size).max(1.0);

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "OpenStack".to_string(),
                region: if region.is_empty() {
                    "global".to_string()
                } else {
                    region
                },
                resource_type: "Snapshot".to_string(),
                details: format!(
                    "Old OpenStack snapshot: {} ({:.1} GB)",
                    if name.is_empty() { id } else { name },
                    size_gb
                ),
                estimated_monthly_cost: size_gb * 0.03,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for OpenstackScanner {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_endpoint_and_scoped_paths_work_for_project_and_default() {
        let scanner = OpenstackScanner::new("token", "openstack.local", "proj-1");
        assert_eq!(scanner.base_url, "https://openstack.local/v2");
        let paths = scanner.scoped_paths(&["servers"]);
        assert_eq!(paths[0], "/proj-1/servers");
        assert_eq!(paths[1], "/servers");

        assert_eq!(
            OpenstackScanner::normalize_endpoint(""),
            "https://openstack.example.com/v2"
        );
    }

    #[test]
    fn value_extractors_parse_nested_and_multi_shape_payloads() {
        let payload = json!({
            "server": {"id":"vm-1","flavor":{"id":"m1.small"}},
            "items": [{"id":"a"},{"id":"b"}]
        });
        assert_eq!(
            OpenstackScanner::str_field(&payload, &["server.id"]),
            "vm-1".to_string()
        );
        assert_eq!(
            OpenstackScanner::str_field(&payload, &["server.flavor.id"]),
            "m1.small".to_string()
        );
        let extracted = OpenstackScanner::extract_items(&payload, &["items"], &["items"]);
        assert_eq!(extracted.len(), 4);
    }

    #[test]
    fn parse_helpers_cover_time_and_numeric_variants() {
        let payload = json!({
            "n1": 7,
            "n2": "9.5"
        });
        assert_eq!(OpenstackScanner::parse_f64(&payload, &["n1"]), Some(7.0));
        assert_eq!(OpenstackScanner::parse_f64(&payload, &["n2"]), Some(9.5));
        assert!(OpenstackScanner::parse_time("2026-03-17T01:02:03Z").is_some());
        assert!(OpenstackScanner::parse_time("").is_none());
    }
}
