use crate::models::WastedResource;
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use hex;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;

// --- XML Structs for BOS ---
#[derive(Deserialize)]
#[allow(dead_code)]
struct ListAllMyBucketsResult {
    #[serde(rename = "Owner")]
    owner: Option<Owner>,
    #[serde(rename = "Buckets")]
    buckets: Option<Buckets>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Owner {
    #[serde(rename = "ID")]
    id: String,
}

#[derive(Deserialize)]
struct Buckets {
    #[serde(rename = "Bucket")]
    bucket: Option<Vec<Bucket>>,
}

#[derive(Deserialize)]
struct Bucket {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Location", default)]
    location: String,
}

pub struct BaiduScanner {
    client: Client,
    ak: String,
    sk: String,
    region: String,
}

impl BaiduScanner {
    pub fn new(ak: &str, sk: &str, region: &str) -> Self {
        Self {
            client: Client::new(),
            ak: ak.to_string(),
            sk: sk.to_string(),
            region: region.to_string(),
        }
    }

    fn sign(
        &self,
        method: &str,
        uri: &str,
        params: &str,
        host: &str,
        timestamp: i64,
    ) -> Result<String> {
        let date = Utc
            .timestamp_opt(timestamp, 0)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        let auth_string = format!("bce-auth-v1/{}/{}/1800", self.ak, date);

        let signing_key = hex::encode(hmac_sha256(self.sk.as_bytes(), auth_string.as_bytes()));

        let canonical_uri = uri; // Should be urlencoded but basic paths are safe
        let canonical_query = params; // Should be sorted
        let canonical_headers = format!("host:{}", host);

        let canonical_request = format!(
            "{}\n{}\n{}\n{}",
            method, canonical_uri, canonical_query, canonical_headers
        );

        let signature = hex::encode(hmac_sha256(
            signing_key.as_bytes(),
            canonical_request.as_bytes(),
        ));

        Ok(format!("{}/host/{}", auth_string, signature))
    }

    async fn request(&self, service: &str, uri: &str, params: &str) -> Result<Value> {
        let host = format!("{}.{}.baidubce.com", service, self.region);
        let url = format!("https://{}{}", host, uri);
        let timestamp = Utc::now().timestamp();

        let auth = self.sign("GET", uri, params, &host, timestamp)?;
        let date = Utc
            .timestamp_opt(timestamp, 0)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();

        let mut full_url = url;
        if !params.is_empty() {
            full_url.push('?');
            full_url.push_str(params);
        }

        let res = self
            .client
            .get(&full_url)
            .header("Authorization", auth)
            .header("x-bce-date", date)
            .header("Host", host)
            .send()
            .await?;

        if !res.status().is_success() {
            return Ok(serde_json::json!({}));
        }

        Ok(res.json().await?)
    }

    pub async fn scan_bcc(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        if let Ok(json) = self.request("bcc", "/v1/instance", "").await {
            if let Some(instances) = json["instances"].as_array() {
                for i in instances {
                    let id = i["instanceId"].as_str().unwrap_or("").to_string();
                    let name = i["name"].as_str().unwrap_or("").to_string();
                    let status = i["status"].as_str().unwrap_or("");
                    if status == "STOPPED" {
                        wastes.push(WastedResource {
                            id,
                            provider: "Baidu".to_string(),
                            region: self.region.clone(),
                            resource_type: "BCC Instance".to_string(),
                            details: format!("Stopped: {}", name),
                            estimated_monthly_cost: 0.0,
                            action_type: "DELETE".to_string(),
                        });
                    }
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_cds(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        if let Ok(json) = self.request("cds", "/v1/volume", "").await {
            if let Some(volumes) = json["volumes"].as_array() {
                for v in volumes {
                    let id = v["id"].as_str().unwrap_or("").to_string();
                    let size = v["size"].as_i64().unwrap_or(0);
                    let status = v["status"].as_str().unwrap_or("");
                    if status == "AVAILABLE" {
                        wastes.push(WastedResource {
                            id,
                            provider: "Baidu".to_string(),
                            region: self.region.clone(),
                            resource_type: "CDS Disk".to_string(),
                            details: format!("Unattached {}GB", size),
                            estimated_monthly_cost: size as f64 * 0.4,
                            action_type: "DELETE".to_string(),
                        });
                    }
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_eips(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        if let Ok(json) = self.request("eip", "/v1/eip", "").await {
            if let Some(eips) = json["eipList"].as_array() {
                for e in eips {
                    let status = e["status"].as_str().unwrap_or("");
                    if status == "available" {
                        let ip = e["eip"].as_str().unwrap_or("").to_string();
                        wastes.push(WastedResource {
                            id: ip.clone(),
                            provider: "Baidu".to_string(),
                            region: self.region.clone(),
                            resource_type: "EIP".to_string(),
                            details: format!("Unbound: {}", ip),
                            estimated_monthly_cost: 25.0,
                            action_type: "DELETE".to_string(),
                        });
                    }
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_blb(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        if let Ok(json) = self.request("blb", "/v2/loadBalancer", "").await {
            if let Some(load_balancers) = json["loadBalancerList"].as_array() {
                for balancer in load_balancers {
                    let id = balancer["loadBalancerId"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let name = balancer["name"].as_str().unwrap_or("").to_string();
                    let status = balancer["status"].as_str().unwrap_or("");
                    let listener_count = balancer["listenerCount"].as_i64().unwrap_or(0);
                    let backend_count = balancer["backendCount"].as_i64().unwrap_or(0);

                    if status == "active" && (listener_count == 0 || backend_count == 0) {
                        wastes.push(WastedResource {
                            id,
                            provider: "Baidu".to_string(),
                            region: self.region.clone(),
                            resource_type: "BLB".to_string(),
                            details: format!("Idle Load Balancer: {}", name),
                            estimated_monthly_cost: 18.0,
                            action_type: "DELETE".to_string(),
                        });
                    }
                }
            }
        }
        Ok(wastes)
    }

    async fn bos_get_text(&self, host: &str, uri: &str, params: &str) -> Option<(u16, String)> {
        let timestamp = Utc::now().timestamp();
        let auth = self.sign("GET", uri, params, host, timestamp).ok()?;
        let date = Utc
            .timestamp_opt(timestamp, 0)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();

        let mut url = format!("https://{}{}", host, uri);
        if !params.is_empty() {
            url.push('?');
            url.push_str(params);
        }

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth)
            .header("x-bce-date", date)
            .header("Host", host)
            .send()
            .await
            .ok()?;

        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        Some((status, body))
    }

    fn extract_xml_u64(payload: &str, tag: &str) -> Option<u64> {
        let start_tag = format!("<{}>", tag);
        let end_tag = format!("</{}>", tag);
        let start_idx = payload.find(&start_tag)? + start_tag.len();
        let end_idx = payload[start_idx..].find(&end_tag)? + start_idx;
        payload[start_idx..end_idx].trim().parse::<u64>().ok()
    }

    fn bos_is_empty_bucket(payload: &str) -> Option<bool> {
        if let Some(key_count) = Self::extract_xml_u64(payload, "KeyCount") {
            return Some(key_count == 0);
        }

        if payload.contains("<Contents>") {
            return Some(false);
        }

        if payload.contains("<ListBucketResult") {
            return Some(true);
        }

        None
    }

    fn bos_lifecycle_missing(status: u16, payload: &str) -> bool {
        if status == 404 {
            return true;
        }

        let lower = payload.to_lowercase();
        lower.contains("nosuchlifecycleconfiguration") || lower.contains("no such lifecycle")
    }

    pub async fn scan_bos(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let host = format!("bos.{}.baidubce.com", self.region);

        let Some((status, payload)) = self.bos_get_text(&host, "/", "").await else {
            return Ok(vec![]);
        };

        if !(200..300).contains(&status) {
            return Ok(vec![]);
        }

        let parsed = serde_xml_rs::from_str::<ListAllMyBucketsResult>(&payload).ok();
        if let Some(buckets) = parsed
            .and_then(|result| result.buckets)
            .and_then(|items| items.bucket)
        {
            for b in buckets {
                if b.name.trim().is_empty() {
                    continue;
                }

                let bucket_region = if b.location.trim().is_empty() {
                    self.region.clone()
                } else {
                    b.location.clone()
                };

                let b_host = format!("{}.{}", b.name, host);

                let mut empty_bucket = None;
                for object_query in ["maxKeys=1", "max-keys=1"] {
                    if let Some((object_status, object_payload)) =
                        self.bos_get_text(&b_host, "/", object_query).await
                    {
                        if (200..300).contains(&object_status) {
                            empty_bucket = Self::bos_is_empty_bucket(&object_payload);
                            if empty_bucket.is_some() {
                                break;
                            }
                        }
                    }
                }

                if empty_bucket == Some(true) {
                    wastes.push(WastedResource {
                        id: b.name,
                        provider: "Baidu".to_string(),
                        region: bucket_region,
                        resource_type: "BOS Bucket".to_string(),
                        details: "Empty bucket (0 objects).".to_string(),
                        estimated_monthly_cost: 1.0,
                        action_type: "DELETE".to_string(),
                    });
                    continue;
                }

                if let Some((lifecycle_status, lifecycle_payload)) =
                    self.bos_get_text(&b_host, "/", "lifecycle").await
                {
                    if Self::bos_lifecycle_missing(lifecycle_status, &lifecycle_payload) {
                        wastes.push(WastedResource {
                            id: b.name,
                            provider: "Baidu".to_string(),
                            region: bucket_region,
                            resource_type: "BOS Bucket".to_string(),
                            details: "No lifecycle policy configured. Suggest archiving cold data."
                                .to_string(),
                            estimated_monthly_cost: 5.0,
                            action_type: "ARCHIVE".to_string(),
                        });
                    }
                }
            }
        }

        Ok(wastes)
    }
}

fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

#[async_trait]
impl CloudProvider for BaiduScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_bcc().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_cds().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_eips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_blb().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_bos().await {
            results.extend(r);
        }
        Ok(results)
    }
}
