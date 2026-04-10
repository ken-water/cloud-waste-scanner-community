use anyhow::{anyhow, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use sha1::Sha1;

use crate::models::WastedResource;
use crate::traits::CloudProvider;

type HmacSha1 = Hmac<Sha1>;

pub struct ExoscaleScanner {
    client: Client,
    api_key: String,
    secret_key: String,
    endpoint: String,
}

impl ExoscaleScanner {
    pub fn new(api_key: &str, secret_key: &str, endpoint: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.trim().to_string(),
            secret_key: secret_key.trim().to_string(),
            endpoint: Self::normalize_endpoint(endpoint),
        }
    }

    fn normalize_endpoint(raw: &str) -> String {
        let endpoint = raw.trim();
        if endpoint.is_empty() {
            return "https://api.exoscale.com/compute".to_string();
        }

        let mut value = endpoint.to_string();
        if !value.starts_with("http://") && !value.starts_with("https://") {
            value = format!("https://{}", value);
        }

        if value.contains("/compute") {
            return value.trim_end_matches('/').to_string();
        }

        format!("{}/compute", value.trim_end_matches('/'))
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

    fn parse_bool(value: &Value, key: &str) -> bool {
        if let Some(boolean) = value.get(key).and_then(|v| v.as_bool()) {
            return boolean;
        }

        value
            .get(key)
            .and_then(|v| v.as_str())
            .map(|v| {
                let normalized = v.trim().to_lowercase();
                normalized == "true" || normalized == "1" || normalized == "yes"
            })
            .unwrap_or(false)
    }

    fn bytes_to_gb(size: f64) -> f64 {
        if size > 1024.0 * 1024.0 * 1024.0 {
            return size / 1024.0 / 1024.0 / 1024.0;
        }

        size
    }

    fn sign_query(&self, params: &[(String, String)]) -> Result<String> {
        let mut sorted = params.to_vec();
        sorted.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        let canonical = sorted
            .iter()
            .map(|(k, v)| format!("{}={}", k.to_lowercase(), urlencoding::encode(v)))
            .collect::<Vec<String>>()
            .join("&")
            .to_lowercase();

        let mut mac = HmacSha1::new_from_slice(self.secret_key.as_bytes())
            .map_err(|e| anyhow!("Invalid Exoscale secret key: {}", e))?;
        mac.update(canonical.as_bytes());
        let signature = STANDARD.encode(mac.finalize().into_bytes());
        Ok(signature)
    }

    async fn request_json(&self, command: &str, extra_params: &[(&str, String)]) -> Result<Value> {
        if self.api_key.is_empty() || self.secret_key.is_empty() {
            return Err(anyhow!("Exoscale API key and secret key are required"));
        }

        let mut params = vec![
            ("apikey".to_string(), self.api_key.clone()),
            ("command".to_string(), command.to_string()),
            ("response".to_string(), "json".to_string()),
        ];

        for (key, value) in extra_params {
            if !value.trim().is_empty() {
                params.push(((*key).to_string(), value.trim().to_string()));
            }
        }

        let signature = self.sign_query(&params)?;

        let mut query = String::new();
        for (index, (key, value)) in params.iter().enumerate() {
            if index > 0 {
                query.push('&');
            }
            query.push_str(key);
            query.push('=');
            query.push_str(&urlencoding::encode(value));
        }

        query.push_str("&signature=");
        query.push_str(&urlencoding::encode(&signature));

        let url = format!("{}?{}", self.endpoint, query);
        let response = self.client.get(&url).send().await?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        if !status.is_success() {
            let snippet: String = text.chars().take(300).collect();
            return Err(anyhow!(
                "Exoscale API {} failed ({}): {}",
                command,
                status.as_u16(),
                snippet
            ));
        }

        let value: Value =
            serde_json::from_str(&text).map_err(|e| anyhow!("Invalid Exoscale JSON: {}", e))?;

        if let Some(error) = value.get("errorresponse") {
            let message = Self::str_field(error, &["errortext", "error"]);
            let code = Self::str_field(error, &["errorcode"]);
            return Err(anyhow!(
                "Exoscale API {} error{}{}",
                command,
                if code.is_empty() { "" } else { " (code " },
                if code.is_empty() {
                    if message.is_empty() {
                        "unknown".to_string()
                    } else {
                        message
                    }
                } else {
                    format!(
                        "{}): {}",
                        code,
                        if message.is_empty() {
                            "unknown"
                        } else {
                            &message
                        }
                    )
                }
            ));
        }

        Ok(value)
    }

    fn command_items(payload: &Value, response_key: &str, item_key: &str) -> Vec<Value> {
        payload
            .get(response_key)
            .and_then(|v| v.get(item_key))
            .and_then(|v| v.as_array())
            .map(|items| items.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn parse_time(raw: &str) -> Option<DateTime<Utc>> {
        if raw.trim().is_empty() {
            return None;
        }

        let variants = [
            "%Y-%m-%dT%H:%M:%S%z",
            "%Y-%m-%dT%H:%M:%S%.3f%z",
            "%Y-%m-%dT%H:%M:%S%.f%z",
            "%Y-%m-%dT%H:%M:%SZ",
            "%Y-%m-%dT%H:%M:%S%.3fZ",
            "%Y-%m-%dT%H:%M:%S%.fZ",
        ];

        for fmt in variants {
            if let Ok(dt) = DateTime::parse_from_str(raw, fmt) {
                return Some(dt.with_timezone(&Utc));
            }
        }

        chrono::DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }

    pub async fn check_auth(&self) -> Result<()> {
        self.request_json(
            "listVirtualMachines",
            &[
                ("listall", "true".to_string()),
                ("pagesize", "1".to_string()),
                ("page", "1".to_string()),
            ],
        )
        .await?;

        Ok(())
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_json("listVirtualMachines", &[("listall", "true".to_string())])
            .await?;

        let mut wastes = Vec::new();
        for vm in Self::command_items(&payload, "listvirtualmachinesresponse", "virtualmachine") {
            let state = Self::str_field(&vm, &["state", "status"]).to_lowercase();
            if !(state == "stopped" || state == "stopping") {
                continue;
            }

            let id = Self::str_field(&vm, &["id"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::str_field(&vm, &["displayname", "name"]);
            let zone = Self::str_field(&vm, &["zonename", "zoneid"]);
            let offering = Self::str_field(&vm, &["serviceofferingname"]);

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "Exoscale".to_string(),
                region: if zone.is_empty() {
                    "global".to_string()
                } else {
                    zone
                },
                resource_type: "Instance".to_string(),
                details: format!(
                    "Stopped Exoscale instance: {} ({})",
                    if name.is_empty() { id } else { name },
                    if offering.is_empty() {
                        "unknown"
                    } else {
                        &offering
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
            .request_json("listVolumes", &[("listall", "true".to_string())])
            .await?;

        let mut wastes = Vec::new();
        for volume in Self::command_items(&payload, "listvolumesresponse", "volume") {
            let attached = !Self::str_field(&volume, &["virtualmachineid", "vmid"]).is_empty();
            if attached {
                continue;
            }

            let id = Self::str_field(&volume, &["id"]);
            if id.is_empty() {
                continue;
            }

            let name = Self::str_field(&volume, &["name"]);
            let zone = Self::str_field(&volume, &["zonename", "zoneid"]);
            let size_raw = Self::parse_f64(&volume, &["size", "physicalsize"]).unwrap_or(0.0);
            let size_gb = Self::bytes_to_gb(size_raw);
            let normalized = if size_gb <= 0.0 { 10.0 } else { size_gb };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "Exoscale".to_string(),
                region: if zone.is_empty() {
                    "global".to_string()
                } else {
                    zone
                },
                resource_type: "Volume".to_string(),
                details: format!(
                    "Unattached Exoscale volume: {} ({:.0} GB)",
                    if name.is_empty() { id } else { name },
                    normalized
                ),
                estimated_monthly_cost: normalized * 0.08,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_public_ips(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_json("listPublicIpAddresses", &[("listall", "true".to_string())])
            .await?;

        let mut wastes = Vec::new();
        for ip in Self::command_items(&payload, "listpublicipaddressesresponse", "publicipaddress")
        {
            let attached_vm = !Self::str_field(&ip, &["virtualmachineid"]).is_empty();
            let attached_network = !Self::str_field(&ip, &["associatednetworkid"]).is_empty();
            let is_static_nat = Self::parse_bool(&ip, "isstaticnat");
            let is_source_nat = Self::parse_bool(&ip, "issourcenat");

            if attached_vm || attached_network || is_static_nat || is_source_nat {
                continue;
            }

            let id = Self::str_field(&ip, &["ipaddress", "id"]);
            if id.is_empty() {
                continue;
            }

            let zone = Self::str_field(&ip, &["zonename", "zoneid"]);

            wastes.push(WastedResource {
                id,
                provider: "Exoscale".to_string(),
                region: if zone.is_empty() {
                    "global".to_string()
                } else {
                    zone
                },
                resource_type: "Public IP".to_string(),
                details: "Unassigned Exoscale public IP".to_string(),
                estimated_monthly_cost: 2.0,
                action_type: "DELETE".to_string(),
            });
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let payload = self
            .request_json("listSnapshots", &[("listall", "true".to_string())])
            .await?;

        let mut wastes = Vec::new();
        for snapshot in Self::command_items(&payload, "listsnapshotsresponse", "snapshot") {
            let id = Self::str_field(&snapshot, &["id"]);
            if id.is_empty() {
                continue;
            }

            let created_at = Self::str_field(&snapshot, &["created"]);
            let is_old = Self::parse_time(&created_at)
                .map(|dt| dt < Utc::now() - Duration::days(30))
                .unwrap_or(false);
            if !is_old {
                continue;
            }

            let name = Self::str_field(&snapshot, &["name"]);
            let zone = Self::str_field(&snapshot, &["zonename", "zoneid"]);
            let size_raw = Self::parse_f64(&snapshot, &["physicalsize", "size"]).unwrap_or(0.0);
            let size_gb = Self::bytes_to_gb(size_raw);
            let normalized = if size_gb <= 0.0 { 20.0 } else { size_gb };

            wastes.push(WastedResource {
                id: id.clone(),
                provider: "Exoscale".to_string(),
                region: if zone.is_empty() {
                    "global".to_string()
                } else {
                    zone
                },
                resource_type: "Snapshot".to_string(),
                details: format!(
                    "Old Exoscale snapshot: {} ({:.0} GB)",
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
impl CloudProvider for ExoscaleScanner {
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
