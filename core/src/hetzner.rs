use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use reqwest::Client;
use serde_json::Value;

use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct HetznerScanner {
    client: Client,
    token: String,
}

impl HetznerScanner {
    pub fn new(token: &str) -> Self {
        Self {
            client: Client::new(),
            token: token.to_string(),
        }
    }

    async fn request_page(&self, path: &str, page: i64) -> Result<Value> {
        let separator = if path.contains('?') { '&' } else { '?' };
        let url = format!(
            "https://api.hetzner.cloud/v1/{}{}page={}&per_page=50",
            path, separator, page
        );

        let response = self.client.get(url).bearer_auth(&self.token).send().await?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            let snippet: String = body.chars().take(300).collect();
            return Err(anyhow!(
                "Hetzner API {} failed ({}): {}",
                path,
                status.as_u16(),
                snippet
            ));
        }

        serde_json::from_str(&body).map_err(|e| anyhow!("Invalid Hetzner API JSON: {}", e))
    }

    fn extract_next_page(value: &Value) -> Option<i64> {
        value
            .get("meta")
            .and_then(|m| m.get("pagination"))
            .and_then(|p| p.get("next_page"))
            .and_then(|n| n.as_i64())
    }

    async fn list_collection(&self, path: &str, collection_key: &str) -> Result<Vec<Value>> {
        let mut page = 1;
        let mut all_items = Vec::new();

        loop {
            let payload = self.request_page(path, page).await?;
            if let Some(items) = payload.get(collection_key).and_then(|v| v.as_array()) {
                all_items.extend(items.iter().cloned());
            }

            if let Some(next_page) = Self::extract_next_page(&payload) {
                if next_page <= page {
                    break;
                }
                page = next_page;
            } else {
                break;
            }

            if page > 200 {
                break;
            }
        }

        Ok(all_items)
    }

    fn get_string_field(value: &Value, keys: &[&str]) -> String {
        for key in keys {
            if let Some(text) = value.get(*key).and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    return text.to_string();
                }
            }
        }
        String::new()
    }

    fn parse_server_monthly_cost(server: &Value) -> Option<f64> {
        let prices = server
            .get("server_type")
            .and_then(|v| v.get("prices"))
            .and_then(|v| v.as_array())?;

        for item in prices {
            if let Some(gross) = item
                .get("price_monthly")
                .and_then(|v| v.get("gross"))
                .and_then(|v| v.as_str())
            {
                if let Ok(value) = gross.parse::<f64>() {
                    return Some(value);
                }
            }
        }

        None
    }

    pub async fn check_auth(&self) -> Result<()> {
        self.list_collection("servers", "servers").await?;
        Ok(())
    }

    pub async fn scan_servers(&self) -> Result<Vec<WastedResource>> {
        let servers = self.list_collection("servers", "servers").await?;
        let mut wastes = Vec::new();

        for server in servers {
            let status = Self::get_string_field(&server, &["status"]).to_lowercase();
            if !(status == "off" || status == "stopped") {
                continue;
            }

            let id = server
                .get("id")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string())
                .unwrap_or_else(|| Self::get_string_field(&server, &["name"]));
            if id.is_empty() {
                continue;
            }

            let name = Self::get_string_field(&server, &["name"]);
            let region = server
                .get("datacenter")
                .and_then(|v| v.get("location"))
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("global")
                .to_string();
            let server_type = server
                .get("server_type")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let monthly_cost = Self::parse_server_monthly_cost(&server).unwrap_or(8.0);

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "Hetzner".to_string(),
                region,
                resource_type: "Server".to_string(),
                details: format!(
                    "Stopped Hetzner server: {} ({})",
                    if name.is_empty() { id } else { name },
                    server_type
                ),
                estimated_monthly_cost: monthly_cost,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        let volumes = self.list_collection("volumes", "volumes").await?;
        let mut wastes = Vec::new();

        for volume in volumes {
            let server_attached = volume.get("server").and_then(|v| v.as_i64()).is_some();
            let servers_attached = volume
                .get("servers")
                .and_then(|v| v.as_array())
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            if server_attached || servers_attached {
                continue;
            }

            let id = volume
                .get("id")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string())
                .unwrap_or_default();
            if id.is_empty() {
                continue;
            }

            let name = Self::get_string_field(&volume, &["name"]);
            let region = volume
                .get("location")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("global")
                .to_string();
            let size = volume.get("size").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let normalized_size = if size <= 0.0 { 10.0 } else { size };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "Hetzner".to_string(),
                region,
                resource_type: "Volume".to_string(),
                details: format!(
                    "Unattached Hetzner volume: {} ({:.0} GB)",
                    if name.is_empty() { id } else { name },
                    normalized_size
                ),
                estimated_monthly_cost: normalized_size * 0.05,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_floating_ips(&self) -> Result<Vec<WastedResource>> {
        let floating_ips = self.list_collection("floating_ips", "floating_ips").await?;
        let mut wastes = Vec::new();

        for ip in floating_ips {
            let attached = ip.get("server").and_then(|v| v.as_i64()).is_some();
            if attached {
                continue;
            }

            let id = Self::get_string_field(&ip, &["ip"]);
            if id.is_empty() {
                continue;
            }

            let region = ip
                .get("home_location")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("global")
                .to_string();

            wastes.push(WastedResource {
                id,
                provider: "Hetzner".to_string(),
                region,
                resource_type: "Floating IP".to_string(),
                details: "Unassigned Hetzner Floating IP".to_string(),
                estimated_monthly_cost: 1.5,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let images = self
            .list_collection("images?type=snapshot", "images")
            .await?;
        let mut wastes = Vec::new();

        for image in images {
            let image_type = Self::get_string_field(&image, &["type"]).to_lowercase();
            if image_type != "snapshot" {
                continue;
            }

            let created = Self::get_string_field(&image, &["created"]);
            let is_old = chrono::DateTime::parse_from_rfc3339(&created)
                .map(|dt| dt.with_timezone(&Utc) < Utc::now() - Duration::days(30))
                .unwrap_or(false);
            if !is_old {
                continue;
            }

            let id = image
                .get("id")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string())
                .unwrap_or_default();
            if id.is_empty() {
                continue;
            }

            let description = Self::get_string_field(&image, &["description", "name"]);
            let size = image
                .get("image_size")
                .and_then(|v| v.as_f64())
                .unwrap_or(20.0);
            let normalized_size = if size <= 0.0 { 20.0 } else { size };

            wastes.push(WastedResource {
                id,
                provider: "Hetzner".to_string(),
                region: "global".to_string(),
                resource_type: "Snapshot".to_string(),
                details: format!(
                    "Old Hetzner snapshot: {} ({:.1} GB)",
                    if description.is_empty() {
                        "unnamed".to_string()
                    } else {
                        description
                    },
                    normalized_size
                ),
                estimated_monthly_cost: normalized_size * 0.01,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for HetznerScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_servers().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_volumes().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_floating_ips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_snapshots().await {
            results.extend(r);
        }
        Ok(results)
    }
}
