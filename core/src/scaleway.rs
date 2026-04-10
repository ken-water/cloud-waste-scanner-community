use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use reqwest::Client;
use serde_json::Value;

use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct ScalewayScanner {
    client: Client,
    token: String,
    zones: Vec<String>,
}

impl ScalewayScanner {
    pub fn new(token: &str, zones_csv: &str) -> Self {
        Self {
            client: Client::new(),
            token: token.to_string(),
            zones: Self::parse_zones(zones_csv),
        }
    }

    fn parse_zones(raw: &str) -> Vec<String> {
        let zones: Vec<String> = raw
            .split(',')
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string())
            .collect();

        if zones.is_empty() {
            return vec![
                "fr-par-1".to_string(),
                "nl-ams-1".to_string(),
                "pl-waw-1".to_string(),
            ];
        }

        zones
    }

    async fn request_json(&self, url: &str) -> Result<Value> {
        let response = self
            .client
            .get(url)
            .header("X-Auth-Token", &self.token)
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(anyhow!(
                "Scaleway API failed ({}): {}",
                status.as_u16(),
                snippet
            ));
        }

        serde_json::from_str(&text).map_err(|e| anyhow!("Invalid Scaleway JSON: {}", e))
    }

    async fn list_zone_resource(
        &self,
        zone: &str,
        resource: &str,
        key: &str,
    ) -> Result<Vec<Value>> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/{}?page=1&per_page=100",
            zone, resource
        );
        let payload = self.request_json(&url).await?;
        Ok(payload
            .get(key)
            .and_then(|v| v.as_array())
            .map(|items| items.iter().cloned().collect())
            .unwrap_or_default())
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

    fn bytes_to_gb(value: f64) -> f64 {
        if value <= 0.0 {
            return 0.0;
        }
        value / 1024.0 / 1024.0 / 1024.0
    }

    pub async fn check_auth(&self) -> Result<()> {
        for zone in &self.zones {
            let url = format!(
                "https://api.scaleway.com/instance/v1/zones/{}/servers?page=1&per_page=1",
                zone
            );
            if self.request_json(&url).await.is_ok() {
                return Ok(());
            }
        }

        Err(anyhow!(
            "Scaleway token validation failed for configured zones"
        ))
    }

    pub async fn scan_servers(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();

        for zone in &self.zones {
            let servers = self.list_zone_resource(zone, "servers", "servers").await?;
            for server in servers {
                let state = Self::str_field(&server, &["state", "status"]).to_lowercase();
                if !(state == "stopped" || state == "stopping") {
                    continue;
                }

                let id = Self::str_field(&server, &["id"]);
                if id.is_empty() {
                    continue;
                }

                let name = Self::str_field(&server, &["name"]);
                let server_type = Self::str_field(&server, &["commercial_type", "commercialType"]);

                wastes.push(WastedResource {
                    id: id.clone(),
                    provider: "Scaleway".to_string(),
                    region: zone.clone(),
                    resource_type: "Instance".to_string(),
                    details: format!(
                        "Stopped Scaleway instance: {} ({})",
                        if name.is_empty() { id } else { name },
                        if server_type.is_empty() {
                            "unknown"
                        } else {
                            &server_type
                        }
                    ),
                    estimated_monthly_cost: 8.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();

        for zone in &self.zones {
            let volumes = self.list_zone_resource(zone, "volumes", "volumes").await?;
            for volume in volumes {
                let attached = volume
                    .get("server")
                    .and_then(|s| s.get("id"))
                    .and_then(|v| v.as_str())
                    .map(|v| !v.is_empty())
                    .unwrap_or(false);
                if attached {
                    continue;
                }

                let id = Self::str_field(&volume, &["id"]);
                if id.is_empty() {
                    continue;
                }

                let name = Self::str_field(&volume, &["name"]);
                let size_bytes = volume
                    .get("size")
                    .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|n| n as f64)))
                    .unwrap_or(0.0);
                let size_gb = Self::bytes_to_gb(size_bytes);
                let normalized = if size_gb <= 0.0 { 10.0 } else { size_gb };

                wastes.push(WastedResource {
                    id: id.clone(),
                    provider: "Scaleway".to_string(),
                    region: zone.clone(),
                    resource_type: "Volume".to_string(),
                    details: format!(
                        "Unattached Scaleway volume: {} ({:.0} GB)",
                        if name.is_empty() { id } else { name },
                        normalized
                    ),
                    estimated_monthly_cost: normalized * 0.08,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_ips(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();

        for zone in &self.zones {
            let ips = self.list_zone_resource(zone, "ips", "ips").await?;
            for ip in ips {
                let attached = ip
                    .get("server")
                    .and_then(|s| s.get("id"))
                    .and_then(|v| v.as_str())
                    .map(|v| !v.is_empty())
                    .unwrap_or(false);
                if attached {
                    continue;
                }

                let id = Self::str_field(&ip, &["address", "id"]);
                if id.is_empty() {
                    continue;
                }

                wastes.push(WastedResource {
                    id,
                    provider: "Scaleway".to_string(),
                    region: zone.clone(),
                    resource_type: "Public IP".to_string(),
                    details: "Unassigned Scaleway Public IP".to_string(),
                    estimated_monthly_cost: 1.5,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();

        for zone in &self.zones {
            let snapshots = self
                .list_zone_resource(zone, "snapshots", "snapshots")
                .await?;
            for snapshot in snapshots {
                let id = Self::str_field(&snapshot, &["id"]);
                if id.is_empty() {
                    continue;
                }

                let created_at = Self::str_field(&snapshot, &["creation_date", "created_at"]);
                let is_old = chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map(|dt| dt.with_timezone(&Utc) < Utc::now() - Duration::days(30))
                    .unwrap_or(false);
                if !is_old {
                    continue;
                }

                let name = Self::str_field(&snapshot, &["name"]);
                let size_bytes = snapshot
                    .get("size")
                    .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|n| n as f64)))
                    .unwrap_or(0.0);
                let size_gb = Self::bytes_to_gb(size_bytes);
                let normalized = if size_gb <= 0.0 { 20.0 } else { size_gb };

                wastes.push(WastedResource {
                    id: id.clone(),
                    provider: "Scaleway".to_string(),
                    region: zone.clone(),
                    resource_type: "Snapshot".to_string(),
                    details: format!(
                        "Old Scaleway snapshot: {} ({:.1} GB)",
                        if name.is_empty() { id } else { name },
                        normalized
                    ),
                    estimated_monthly_cost: normalized * 0.02,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for ScalewayScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_servers().await {
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
