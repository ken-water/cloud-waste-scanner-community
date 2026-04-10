use crate::models::WastedResource;
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use hex;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use sha1::Sha1;
use sha2::{Digest, Sha256};

type HmacSha1 = Hmac<Sha1>;

// --- XML Structs for OBS ---
#[derive(Deserialize)]
struct ListAllMyBucketsResult {
    #[serde(rename = "Buckets")]
    buckets: Option<Buckets>,
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

pub struct HuaweiScanner {
    client: Client,
    ak: String,
    sk: String,
    region: String,
    project_id: String,
}

impl HuaweiScanner {
    pub fn new(ak: &str, sk: &str, region: &str, project_id: &str) -> Self {
        Self {
            client: Client::new(),
            ak: ak.to_string(),
            sk: sk.to_string(),
            region: region.to_string(),
            project_id: project_id.to_string(),
        }
    }

    async fn request(&self, service: &str, method: &str, path: &str, query: &str) -> Result<Value> {
        let mut host = String::from(service);
        host.push_str(".");
        host.push_str(&self.region);
        host.push_str(".myhuaweicloud.com");

        let mut url = String::from("https://");
        url.push_str(&host);
        url.push_str(path);
        if !query.is_empty() {
            url.push_str("?");
            url.push_str(query);
        }

        let date_long = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let mut canonical_headers = String::from("host:");
        canonical_headers.push_str(&host);
        canonical_headers.push_str("\nx-sdk-date:");
        canonical_headers.push_str(&date_long);
        canonical_headers.push_str("\n");

        let signed_headers = "host;x-sdk-date";
        let payload_hash = hex::encode(Sha256::digest(b""));

        let mut canonical_request = String::new();
        canonical_request.push_str(method);
        canonical_request.push_str("\n");
        canonical_request.push_str(path);
        canonical_request.push_str("\n");
        canonical_request.push_str(query);
        canonical_request.push_str("\n");
        canonical_request.push_str(&canonical_headers);
        canonical_request.push_str("\n");
        canonical_request.push_str(signed_headers);
        canonical_request.push_str("\n");
        canonical_request.push_str(&payload_hash);

        let algorithm = "SDK-HMAC-SHA256";
        let request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
        let mut string_to_sign = String::new();
        string_to_sign.push_str(algorithm);
        string_to_sign.push_str("\n");
        string_to_sign.push_str(&date_long);
        string_to_sign.push_str("\n");
        string_to_sign.push_str(&request_hash);

        let mut mac = Hmac::<Sha256>::new_from_slice(self.sk.as_bytes()).expect("HMAC error");
        mac.update(string_to_sign.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let mut auth = String::from(algorithm);
        auth.push_str(" Access=");
        auth.push_str(&self.ak);
        auth.push_str(", SignedHeaders=");
        auth.push_str(signed_headers);
        auth.push_str(", Signature=");
        auth.push_str(&signature);

        let res = self
            .client
            .request(method.parse()?, &url)
            .header("Authorization", auth)
            .header("X-Sdk-Date", date_long)
            .header("Host", host)
            .header("X-Project-Id", &self.project_id)
            .send()
            .await?;

        if !res.status().is_success() {
            return Ok(serde_json::json!({}));
        }
        Ok(res.json().await?)
    }

    fn sign_obs(&self, method: &str, date: &str, canonical_resource: &str) -> String {
        let string_to_sign = format!("{}\n\n\n{}\n{}", method, date, canonical_resource);

        let mut mac = HmacSha1::new_from_slice(self.sk.as_bytes()).expect("HMAC error");
        mac.update(string_to_sign.as_bytes());
        STANDARD.encode(mac.finalize().into_bytes())
    }

    fn obs_authorization(&self, method: &str, date: &str, canonical_resource: &str) -> String {
        let signature = self.sign_obs(method, date, canonical_resource);
        format!("OBS {}:{}", self.ak, signature)
    }

    fn obs_bucket_hosts(&self, bucket_name: &str, bucket_region: &str) -> Vec<String> {
        let mut hosts = Vec::new();

        if !bucket_region.trim().is_empty() {
            hosts.push(format!(
                "{}.obs.{}.myhuaweicloud.com",
                bucket_name, bucket_region
            ));
        }

        hosts.push(format!("{}.obs.myhuaweicloud.com", bucket_name));
        hosts
    }

    async fn obs_request_text(
        &self,
        host: &str,
        path_with_query: &str,
        canonical_resource: &str,
    ) -> Option<(u16, String)> {
        let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let authorization = self.obs_authorization("GET", &date, canonical_resource);
        let url = format!("https://{}{}", host, path_with_query);

        let response = self
            .client
            .get(&url)
            .header("Authorization", authorization)
            .header("Date", date)
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

    fn obs_is_empty_bucket(payload: &str) -> Option<bool> {
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

    fn obs_lifecycle_missing(status: u16, payload: &str) -> bool {
        if status == 404 {
            return true;
        }

        let body_lower = payload.to_lowercase();
        body_lower.contains("nosuchlifecycleconfiguration")
            || body_lower.contains("no such lifecycle")
    }

    async fn list_obs_buckets(&self) -> Vec<Bucket> {
        let mut hosts = Vec::new();
        if !self.region.trim().is_empty() {
            hosts.push(format!("obs.{}.myhuaweicloud.com", self.region));
        }
        hosts.push(String::from("obs.myhuaweicloud.com"));

        for host in hosts {
            if let Some((status, body)) = self.obs_request_text(&host, "/", "/").await {
                if !(200..300).contains(&status) {
                    continue;
                }

                if let Ok(result) = serde_xml_rs::from_str::<ListAllMyBucketsResult>(&body) {
                    if let Some(bucket_list) = result.buckets.and_then(|items| items.bucket) {
                        return bucket_list;
                    }
                }
            }
        }

        Vec::new()
    }

    pub async fn scan_obs(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let buckets = self.list_obs_buckets().await;

        for bucket in buckets {
            if bucket.name.trim().is_empty() {
                continue;
            }

            let bucket_region = if bucket.location.trim().is_empty() {
                self.region.clone()
            } else {
                bucket.location.clone()
            };

            let bucket_hosts = self.obs_bucket_hosts(&bucket.name, &bucket_region);

            let mut empty_bucket: Option<bool> = None;
            let mut lifecycle_missing: Option<bool> = None;

            for host in bucket_hosts {
                if empty_bucket.is_none() {
                    let object_probe = self
                        .obs_request_text(&host, "/?max-keys=1", &format!("/{}/", bucket.name))
                        .await;

                    if let Some((status, payload)) = object_probe {
                        if (200..300).contains(&status) {
                            empty_bucket = Self::obs_is_empty_bucket(&payload);
                        }
                    }
                }

                if lifecycle_missing.is_none() {
                    let lifecycle_probe = self
                        .obs_request_text(
                            &host,
                            "/?lifecycle",
                            &format!("/{}?lifecycle", bucket.name),
                        )
                        .await;

                    if let Some((status, payload)) = lifecycle_probe {
                        if (200..300).contains(&status) {
                            lifecycle_missing = Some(false);
                        } else if Self::obs_lifecycle_missing(status, &payload) {
                            lifecycle_missing = Some(true);
                        }
                    }
                }

                if empty_bucket.is_some() && lifecycle_missing.is_some() {
                    break;
                }
            }

            if empty_bucket == Some(true) {
                wastes.push(WastedResource {
                    id: bucket.name.clone(),
                    provider: String::from("Huawei"),
                    region: bucket_region.clone(),
                    resource_type: String::from("OBS Bucket"),
                    details: String::from("Empty OBS bucket (0 objects)."),
                    estimated_monthly_cost: 1.0,
                    action_type: String::from("DELETE"),
                });
                continue;
            }

            if lifecycle_missing == Some(true) {
                wastes.push(WastedResource {
                    id: bucket.name,
                    provider: String::from("Huawei"),
                    region: bucket_region,
                    resource_type: String::from("OBS Bucket"),
                    details: String::from(
                        "No lifecycle policy configured. Suggest archiving cold data.",
                    ),
                    estimated_monthly_cost: 5.0,
                    action_type: String::from("ARCHIVE"),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_ecs(&self) -> Result<Vec<WastedResource>> {
        let mut path = String::from("/v1/");
        path.push_str(&self.project_id);
        path.push_str("/cloudservers/detail");
        if let Ok(json) = self.request("ecs", "GET", &path, "status=SHUTOFF").await {
            let mut wastes = Vec::new();
            if let Some(servers) = json["servers"].as_array() {
                for s in servers {
                    let name = s["name"].as_str().unwrap_or("").to_string();
                    wastes.push(WastedResource {
                        id: s["id"].as_str().unwrap_or("").to_string(),
                        provider: String::from("Huawei"),
                        region: self.region.clone(),
                        resource_type: String::from("ECS Instance"),
                        details: name,
                        estimated_monthly_cost: 0.0,
                        action_type: String::from("DELETE"),
                    });
                }
            }
            return Ok(wastes);
        }
        Ok(vec![])
    }

    pub async fn scan_evs(&self) -> Result<Vec<WastedResource>> {
        let mut path = String::from("/v2/");
        path.push_str(&self.project_id);
        path.push_str("/cloudvolumes/detail");
        if let Ok(json) = self.request("evs", "GET", &path, "status=available").await {
            let mut wastes = Vec::new();
            if let Some(volumes) = json["volumes"].as_array() {
                for v in volumes {
                    let name = v["name"].as_str().unwrap_or("").to_string();
                    wastes.push(WastedResource {
                        id: v["id"].as_str().unwrap_or("").to_string(),
                        provider: String::from("Huawei"),
                        region: self.region.clone(),
                        resource_type: String::from("EVS Disk"),
                        details: name,
                        estimated_monthly_cost: 10.0,
                        action_type: String::from("DELETE"),
                    });
                }
            }
            return Ok(wastes);
        }
        Ok(vec![])
    }

    pub async fn scan_eips(&self) -> Result<Vec<WastedResource>> {
        let mut path = String::from("/v1/");
        path.push_str(&self.project_id);
        path.push_str("/publicips");
        if let Ok(json) = self.request("vpc", "GET", &path, "").await {
            let mut wastes = Vec::new();
            if let Some(ips) = json["publicips"].as_array() {
                for ip in ips {
                    if ip["status"].as_str().unwrap_or("") == "DOWN" {
                        wastes.push(WastedResource {
                            id: ip["id"].as_str().unwrap_or("").to_string(),
                            provider: String::from("Huawei"),
                            region: self.region.clone(),
                            resource_type: String::from("EIP"),
                            details: String::from("Unbound"),
                            estimated_monthly_cost: 20.0,
                            action_type: String::from("DELETE"),
                        });
                    }
                }
            }
            return Ok(wastes);
        }
        Ok(vec![])
    }

    pub async fn scan_rds(&self) -> Result<Vec<WastedResource>> {
        let mut path = String::from("/v3/");
        path.push_str(&self.project_id);
        path.push_str("/instances");

        let json = self.request("rds", "GET", &path, "").await?;
        let mut wastes = Vec::new();

        if let Some(instances) = json["instances"].as_array() {
            for instance in instances {
                let id = instance["id"].as_str().unwrap_or_default().to_string();
                let name = instance["name"].as_str().unwrap_or("Unnamed").to_string();
                let status = instance["status"].as_str().unwrap_or("").to_lowercase();

                let is_idle_like = status.contains("shutdown")
                    || status.contains("stopped")
                    || status.contains("paused")
                    || status.contains("frozen")
                    || status.contains("abnormal");

                if is_idle_like {
                    wastes.push(WastedResource {
                        id,
                        provider: String::from("Huawei"),
                        region: self.region.clone(),
                        resource_type: String::from("RDS Instance"),
                        details: format!("Potentially idle RDS: {} (status: {})", name, status),
                        estimated_monthly_cost: 30.0,
                        action_type: String::from("DELETE"),
                    });
                }
            }
        }

        Ok(wastes)
    }

    pub async fn scan_load_balancers(&self) -> Result<Vec<WastedResource>> {
        let mut lb_path = String::from("/v2/");
        lb_path.push_str(&self.project_id);
        lb_path.push_str("/elb/loadbalancers");

        let json = self.request("elb", "GET", &lb_path, "").await?;
        let mut wastes = Vec::new();

        if let Some(load_balancers) = json["loadbalancers"].as_array() {
            for lb in load_balancers {
                let lb_id = lb["id"].as_str().unwrap_or_default().to_string();
                let name = lb["name"].as_str().unwrap_or("Unnamed").to_string();
                let operating_status = lb["operating_status"].as_str().unwrap_or("").to_lowercase();

                let mut listener_path = String::from("/v2/");
                listener_path.push_str(&self.project_id);
                listener_path.push_str("/elb/listeners");
                let listener_query = format!("loadbalancer_id={}", lb_id);
                let listener_json = self
                    .request("elb", "GET", &listener_path, &listener_query)
                    .await
                    .unwrap_or_else(|_| serde_json::json!({}));
                let listener_count = listener_json["listeners"]
                    .as_array()
                    .map(|v| v.len())
                    .unwrap_or(0);

                let mut pool_path = String::from("/v2/");
                pool_path.push_str(&self.project_id);
                pool_path.push_str("/elb/pools");
                let pool_query = format!("loadbalancer_id={}", lb_id);
                let pool_json = self
                    .request("elb", "GET", &pool_path, &pool_query)
                    .await
                    .unwrap_or_else(|_| serde_json::json!({}));
                let pool_count = pool_json["pools"].as_array().map(|v| v.len()).unwrap_or(0);

                let is_idle = listener_count == 0
                    || pool_count == 0
                    || operating_status.contains("offline")
                    || operating_status.contains("disabled");

                if is_idle {
                    wastes.push(WastedResource {
                        id: lb_id,
                        provider: String::from("Huawei"),
                        region: self.region.clone(),
                        resource_type: String::from("Load Balancer"),
                        details: format!(
                            "Idle ELB: {} (listeners={}, pools={}, status={})",
                            name, listener_count, pool_count, operating_status
                        ),
                        estimated_monthly_cost: 45.0,
                        action_type: String::from("DELETE"),
                    });
                }
            }
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for HuaweiScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_ecs().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_evs().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_eips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_rds().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_load_balancers().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_obs().await {
            results.extend(r);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scanner() -> HuaweiScanner {
        HuaweiScanner::new("ak-test", "sk-test", "ap-southeast-1", "project-1")
    }

    #[test]
    fn obs_signature_and_auth_helpers_are_deterministic() {
        let signature = scanner().sign_obs("GET", "Mon, 17 Mar 2026 00:00:00 GMT", "/bucket/");
        assert!(!signature.is_empty());
        let auth = scanner().obs_authorization("GET", "Mon, 17 Mar 2026 00:00:00 GMT", "/bucket/");
        assert!(auth.starts_with("OBS ak-test:"));
    }

    #[test]
    fn obs_xml_and_lifecycle_helpers_cover_fallbacks() {
        let payload = "<ListBucketResult><KeyCount>1</KeyCount></ListBucketResult>";
        assert_eq!(HuaweiScanner::extract_xml_u64(payload, "KeyCount"), Some(1));
        assert_eq!(HuaweiScanner::obs_is_empty_bucket(payload), Some(false));
        assert!(HuaweiScanner::obs_lifecycle_missing(404, ""));
        assert!(HuaweiScanner::obs_lifecycle_missing(
            200,
            "NoSuchLifecycleConfiguration"
        ));
    }

    #[test]
    fn obs_bucket_hosts_prioritize_bucket_region() {
        let hosts = scanner().obs_bucket_hosts("logs", "cn-north-4");
        assert_eq!(hosts[0], "logs.obs.cn-north-4.myhuaweicloud.com");
        assert_eq!(hosts[1], "logs.obs.myhuaweicloud.com");
    }
}
