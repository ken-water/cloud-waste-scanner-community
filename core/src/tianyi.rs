use crate::models::WastedResource;
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use sha2::Sha256;
use uuid::Uuid;

pub struct TianyiScanner {
    client: Client,
    ak: String,
    sk: String,
    region: String,
}

const OPENAPI_BASE_PATTERNS: [&str; 3] = [
    "https://ctapi-{region}.ctyun.cn",
    "https://openapi.ctyun.cn",
    "https://api.ctyun.cn",
];

const HOST_ENDPOINT_PATHS: [&str; 2] = ["/v4/ecs/instances", "/v4/compute/instances"];
const DISK_ENDPOINT_PATHS: [&str; 2] = ["/v4/ebs/volumes", "/v4/disk/volumes"];
const EIP_ENDPOINT_PATHS: [&str; 2] = ["/v4/eip/addresses", "/v4/network/eips"];
const LB_ENDPOINT_PATHS: [&str; 2] = ["/v4/elb/loadbalancers", "/v4/lb/loadbalancers"];

const HOST_ARRAY_PATHS: [&str; 4] = [
    "result.instances",
    "returnObj.instances",
    "data.instances",
    "instances",
];
const DISK_ARRAY_PATHS: [&str; 4] = [
    "result.volumes",
    "returnObj.volumes",
    "data.volumes",
    "volumes",
];
const EIP_ARRAY_PATHS: [&str; 4] = ["result.eips", "returnObj.eips", "data.eips", "eips"];
const LB_ARRAY_PATHS: [&str; 4] = [
    "result.loadBalancers",
    "returnObj.loadBalancers",
    "data.loadBalancers",
    "loadBalancers",
];

const HOST_STATUS_KEYS: [&str; 3] = ["status", "instanceStatus", "state"];
const HOST_ID_KEYS: [&str; 3] = ["instanceId", "id", "uuid"];
const HOST_NAME_KEYS: [&str; 3] = ["instanceName", "name", "displayName"];

const DISK_STATUS_KEYS: [&str; 3] = ["status", "volumeStatus", "state"];
const DISK_ATTACH_KEYS: [&str; 3] = ["instanceId", "serverId", "attachTo"];
const DISK_ID_KEYS: [&str; 3] = ["volumeId", "diskId", "id"];
const DISK_NAME_KEYS: [&str; 3] = ["volumeName", "name", "displayName"];
const DISK_SIZE_KEYS: [&str; 3] = ["size", "volumeSize", "capacity"];

const EIP_STATUS_KEYS: [&str; 3] = ["status", "bindStatus", "state"];
const EIP_BIND_KEYS: [&str; 3] = ["instanceId", "bindInstanceId", "associateId"];
const EIP_ID_KEYS: [&str; 3] = ["allocationId", "id", "eipId"];
const EIP_ADDR_KEYS: [&str; 3] = ["eip", "ipAddress", "address"];

const LB_STATUS_KEYS: [&str; 3] = ["status", "state", "lifecycleState"];
const LB_ID_KEYS: [&str; 3] = ["loadBalancerId", "id", "elbId"];
const LB_NAME_KEYS: [&str; 3] = ["loadBalancerName", "name", "displayName"];
const LB_LISTENER_KEYS: [&str; 3] = ["listenerCount", "listeners", "listenerNum"];
const LB_BACKEND_KEYS: [&str; 3] = ["backendCount", "backendNum", "serverCount"];

impl TianyiScanner {
    pub fn new(ak: &str, sk: &str, region: &str) -> Self {
        Self {
            client: Client::new(),
            ak: ak.to_string(),
            sk: sk.to_string(),
            region: region.to_string(),
        }
    }

    // CTyun OOS (S3 Compatible v2 Signer)
    fn sign_s3(&self, method: &str, bucket: &str, resource: &str, date: &str) -> Result<String> {
        let mut string_to_sign = String::new();
        string_to_sign.push_str(method);
        string_to_sign.push_str("\n\n\n"); // Content-MD5, Content-Type, Date (using x-amz-date header usually, but v2 uses Date header)
        string_to_sign.push_str(date);
        string_to_sign.push_str("\n");
        if !bucket.is_empty() {
            string_to_sign.push_str("/");
            string_to_sign.push_str(bucket);
        }
        string_to_sign.push_str(resource);

        use sha1::Sha1;
        let mut mac = Hmac::<Sha1>::new_from_slice(self.sk.as_bytes()).expect("HMAC error");
        mac.update(string_to_sign.as_bytes());
        let signature = STANDARD.encode(mac.finalize().into_bytes());

        let mut auth = String::from("AWS ");
        auth.push_str(&self.ak);
        auth.push_str(":");
        auth.push_str(&signature);
        Ok(auth)
    }

    fn sign_openapi(&self, method: &str, path: &str, timestamp: i64, nonce: &str) -> String {
        let canonical = format!(
            "{}\n{}\n{}\n{}",
            method.to_uppercase(),
            path,
            timestamp,
            nonce
        );
        let mut mac = Hmac::<Sha256>::new_from_slice(self.sk.as_bytes()).expect("HMAC error");
        mac.update(canonical.as_bytes());
        STANDARD.encode(mac.finalize().into_bytes())
    }

    fn build_openapi_urls(&self, path: &str) -> Vec<String> {
        OPENAPI_BASE_PATTERNS
            .iter()
            .map(|base| {
                let root = base.replace("{region}", &self.region);
                format!("{}{}", root, path)
            })
            .collect()
    }

    fn is_empty_payload(value: &Value) -> bool {
        value.as_object().map(|obj| obj.is_empty()).unwrap_or(false)
    }

    async fn request_openapi_path(&self, method: &str, path: &str) -> Result<Value> {
        let timestamp = Utc::now().timestamp();
        let nonce = Uuid::new_v4().to_string();
        let signature = self.sign_openapi(method, path, timestamp, &nonce);

        // Tianyi OpenAPI endpoints vary by product/region; try a small set of common bases.
        // Fail-soft: if one endpoint fails, continue trying next.
        let endpoints = self.build_openapi_urls(path);

        for url in endpoints {
            let response = self
                .client
                .request(method.parse()?, &url)
                .header("Ctyun-Eop-Request-Id", &nonce)
                .header("Ctyun-Eop-Timestamp", timestamp.to_string())
                .header("Ctyun-Eop-Access-Key", &self.ak)
                .header("Ctyun-Eop-Signature", &signature)
                .header("Ctyun-Eop-Region", &self.region)
                .send()
                .await;

            if let Ok(resp) = response {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<Value>().await {
                        return Ok(json);
                    }
                }
            }
        }

        Ok(serde_json::json!({}))
    }

    async fn request_openapi_candidates(&self, method: &str, paths: &[&str]) -> Result<Value> {
        for path in paths {
            let payload = self.request_openapi_path(method, path).await?;
            if !Self::is_empty_payload(&payload) {
                return Ok(payload);
            }
        }
        Ok(serde_json::json!({}))
    }

    fn pick_array<'a>(json: &'a Value, paths: &[&str]) -> Option<&'a Vec<Value>> {
        for path in paths {
            let mut current = json;
            let mut valid = true;
            for key in path.split('.') {
                if let Some(next) = current.get(key) {
                    current = next;
                } else {
                    valid = false;
                    break;
                }
            }
            if valid {
                if let Some(arr) = current.as_array() {
                    return Some(arr);
                }
            }
        }
        None
    }

    fn str_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
        for key in keys {
            if let Some(v) = value.get(*key).and_then(|x| x.as_str()) {
                return Some(v);
            }
        }
        None
    }

    fn i64_field(value: &Value, keys: &[&str]) -> Option<i64> {
        for key in keys {
            if let Some(v) = value.get(*key).and_then(|x| x.as_i64()) {
                return Some(v);
            }
        }
        None
    }

    async fn oos_get_text(
        &self,
        host: &str,
        bucket: &str,
        resource: &str,
    ) -> Option<(u16, String)> {
        let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let auth = self.sign_s3("GET", bucket, resource, &date).ok()?;
        let url = format!("https://{}{}", host, resource);

        let response = self
            .client
            .get(&url)
            .header("Authorization", auth)
            .header("Date", date)
            .send()
            .await
            .ok()?;

        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        Some((status, body))
    }

    fn extract_xml_tag(payload: &str, tag: &str) -> Option<String> {
        let start_tag = format!("<{}>", tag);
        let end_tag = format!("</{}>", tag);
        let start_idx = payload.find(&start_tag)? + start_tag.len();
        let end_idx = payload[start_idx..].find(&end_tag)? + start_idx;
        let value = payload[start_idx..end_idx].trim();

        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    fn extract_oos_bucket_names(payload: &str) -> Vec<String> {
        let mut names = Vec::new();

        for segment in payload.split("<Bucket>").skip(1) {
            if let Some(block_end) = segment.find("</Bucket>") {
                let block = &segment[..block_end];
                if let Some(name) = Self::extract_xml_tag(block, "Name") {
                    names.push(name);
                }
            }
        }

        names
    }

    fn extract_xml_u64(payload: &str, tag: &str) -> Option<u64> {
        Self::extract_xml_tag(payload, tag).and_then(|value| value.parse::<u64>().ok())
    }

    fn oos_is_empty_bucket(payload: &str) -> Option<bool> {
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

    fn oos_lifecycle_missing(status: u16, payload: &str) -> bool {
        if status == 404 {
            return true;
        }

        let lower = payload.to_lowercase();
        lower.contains("nosuchlifecycleconfiguration") || lower.contains("no such lifecycle")
    }

    pub async fn scan_oos(&self) -> Result<Vec<WastedResource>> {
        let mut host = String::from("oos-");
        host.push_str(&self.region);
        host.push_str(".ctyunapi.cn");

        let Some((status, list_payload)) = self.oos_get_text(&host, "", "/").await else {
            return Ok(vec![]);
        };

        if !(200..300).contains(&status) {
            return Ok(vec![]);
        }

        let mut wastes = Vec::new();
        let bucket_names = Self::extract_oos_bucket_names(&list_payload);

        for bucket_name in bucket_names {
            if bucket_name.trim().is_empty() {
                continue;
            }

            let bucket_host = format!("{}.{}", bucket_name, host);

            let mut empty_bucket = None;
            if let Some((object_status, object_payload)) = self
                .oos_get_text(&bucket_host, &bucket_name, "/?max-keys=1")
                .await
            {
                if (200..300).contains(&object_status) {
                    empty_bucket = Self::oos_is_empty_bucket(&object_payload);
                }
            }

            if empty_bucket == Some(true) {
                wastes.push(WastedResource {
                    id: bucket_name,
                    provider: String::from("Tianyi"),
                    region: self.region.clone(),
                    resource_type: String::from("OOS Bucket"),
                    details: String::from("Empty bucket (0 objects)."),
                    estimated_monthly_cost: 1.0,
                    action_type: String::from("DELETE"),
                });
                continue;
            }

            if let Some((lifecycle_status, lifecycle_payload)) = self
                .oos_get_text(&bucket_host, &bucket_name, "/?lifecycle")
                .await
            {
                if Self::oos_lifecycle_missing(lifecycle_status, &lifecycle_payload) {
                    wastes.push(WastedResource {
                        id: bucket_name,
                        provider: String::from("Tianyi"),
                        region: self.region.clone(),
                        resource_type: String::from("OOS Bucket"),
                        details: String::from(
                            "No lifecycle policy configured. Suggest archiving cold data.",
                        ),
                        estimated_monthly_cost: 5.0,
                        action_type: String::from("ARCHIVE"),
                    });
                }
            }
        }

        Ok(wastes)
    }

    pub async fn scan_host(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let json = self
            .request_openapi_candidates("GET", &HOST_ENDPOINT_PATHS)
            .await?;

        if let Some(items) = Self::pick_array(&json, &HOST_ARRAY_PATHS) {
            for host in items {
                let status = Self::str_field(host, &HOST_STATUS_KEYS)
                    .unwrap_or("")
                    .to_uppercase();
                let id = Self::str_field(host, &HOST_ID_KEYS)
                    .unwrap_or("unknown")
                    .to_string();
                let name = Self::str_field(host, &HOST_NAME_KEYS)
                    .unwrap_or("Unnamed")
                    .to_string();

                if status.contains("STOP")
                    || status.contains("SHUTOFF")
                    || status.contains("OFFLINE")
                {
                    wastes.push(WastedResource {
                        id,
                        provider: "Tianyi".to_string(),
                        region: self.region.clone(),
                        resource_type: "Cloud Host".to_string(),
                        details: format!("Stopped host: {}", name),
                        estimated_monthly_cost: 40.0,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }

        Ok(wastes)
    }

    pub async fn scan_disk(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let json = self
            .request_openapi_candidates("GET", &DISK_ENDPOINT_PATHS)
            .await?;

        if let Some(items) = Self::pick_array(&json, &DISK_ARRAY_PATHS) {
            for disk in items {
                let status = Self::str_field(disk, &DISK_STATUS_KEYS)
                    .unwrap_or("")
                    .to_uppercase();
                let attached_instance = Self::str_field(disk, &DISK_ATTACH_KEYS).unwrap_or("");
                let id = Self::str_field(disk, &DISK_ID_KEYS)
                    .unwrap_or("unknown")
                    .to_string();
                let name = Self::str_field(disk, &DISK_NAME_KEYS)
                    .unwrap_or("Unnamed")
                    .to_string();
                let size = Self::i64_field(disk, &DISK_SIZE_KEYS).unwrap_or(0);

                let is_orphan = status.contains("AVAILABLE")
                    || status.contains("DETACH")
                    || (attached_instance.is_empty() && !status.contains("ATTACH"));

                if is_orphan {
                    wastes.push(WastedResource {
                        id,
                        provider: "Tianyi".to_string(),
                        region: self.region.clone(),
                        resource_type: "Hard Disk".to_string(),
                        details: format!("Unattached disk: {} ({} GB)", name, size),
                        estimated_monthly_cost: (size as f64 * 0.30).max(2.0),
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }

        Ok(wastes)
    }

    pub async fn scan_eips(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let json = self
            .request_openapi_candidates("GET", &EIP_ENDPOINT_PATHS)
            .await?;

        if let Some(items) = Self::pick_array(&json, &EIP_ARRAY_PATHS) {
            for eip in items {
                let status = Self::str_field(eip, &EIP_STATUS_KEYS)
                    .unwrap_or("")
                    .to_uppercase();
                let bound_to = Self::str_field(eip, &EIP_BIND_KEYS).unwrap_or("");
                let id = Self::str_field(eip, &EIP_ID_KEYS)
                    .unwrap_or("unknown")
                    .to_string();
                let ip = Self::str_field(eip, &EIP_ADDR_KEYS)
                    .unwrap_or("unknown")
                    .to_string();

                if status.contains("UNBIND")
                    || status.contains("AVAILABLE")
                    || (bound_to.is_empty()
                        && !status.contains("BIND")
                        && !status.contains("INUSE"))
                {
                    wastes.push(WastedResource {
                        id,
                        provider: "Tianyi".to_string(),
                        region: self.region.clone(),
                        resource_type: "EIP".to_string(),
                        details: format!("Unbound EIP: {}", ip),
                        estimated_monthly_cost: 20.0,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }

        Ok(wastes)
    }

    pub async fn scan_load_balancers(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let json = self
            .request_openapi_candidates("GET", &LB_ENDPOINT_PATHS)
            .await?;

        if let Some(items) = Self::pick_array(&json, &LB_ARRAY_PATHS) {
            for lb in items {
                let status = Self::str_field(lb, &LB_STATUS_KEYS)
                    .unwrap_or("")
                    .to_uppercase();
                let id = Self::str_field(lb, &LB_ID_KEYS)
                    .unwrap_or("unknown")
                    .to_string();
                let name = Self::str_field(lb, &LB_NAME_KEYS)
                    .unwrap_or("Unnamed")
                    .to_string();
                let listener_count = Self::i64_field(lb, &LB_LISTENER_KEYS).unwrap_or(0);
                let backend_count = Self::i64_field(lb, &LB_BACKEND_KEYS).unwrap_or(0);

                if (status.contains("ACTIVE") || status.contains("RUNNING") || status.is_empty())
                    && (listener_count == 0 || backend_count == 0)
                {
                    wastes.push(WastedResource {
                        id,
                        provider: "Tianyi".to_string(),
                        region: self.region.clone(),
                        resource_type: "Load Balancer".to_string(),
                        details: format!("Idle load balancer: {}", name),
                        estimated_monthly_cost: 30.0,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for TianyiScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_oos().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_host().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_disk().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_eips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_load_balancers().await {
            results.extend(r);
        }
        Ok(results)
    }
}
