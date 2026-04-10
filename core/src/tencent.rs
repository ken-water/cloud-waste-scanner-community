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
use sha1::Sha1;
use sha2::{Digest, Sha256};

// --- XML Structs for COS ---
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

pub struct TencentScanner {
    client: Client,
    secret_id: String,
    secret_key: String,
    region: String,
}

impl TencentScanner {
    pub fn new(id: &str, key: &str, region: &str) -> Self {
        Self {
            client: Client::new(),
            secret_id: id.to_string(),
            secret_key: key.to_string(),
            region: region.to_string(),
        }
    }

    fn sign_cvm(&self, service: &str, body: &str, timestamp: i64) -> Result<String> {
        let date = Utc
            .timestamp_opt(timestamp, 0)
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();

        let canonical_uri = "/";
        let canonical_query = "";
        let host = format!("{}.tencentcloudapi.com", service);
        let canonical_headers = format!(
            "content-type:application/json; charset=utf-8\nhost:{}\n",
            host
        );
        let signed_headers = "content-type;host";
        let hashed_payload = hex::encode(Sha256::digest(body.as_bytes()));

        let canonical_request = format!(
            "POST\n{}\n{}\n{}\n{}\n{}",
            canonical_uri, canonical_query, canonical_headers, signed_headers, hashed_payload
        );

        let algorithm = "TC3-HMAC-SHA256";
        let credential_scope = format!("{}/{}/tc3_request", date, service);
        let hashed_canonical_request = hex::encode(Sha256::digest(canonical_request.as_bytes()));

        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm, timestamp, credential_scope, hashed_canonical_request
        );

        let k_key = format!("TC3{}", self.secret_key);
        let k_date = hmac_sha256(k_key.as_bytes(), date.as_bytes());
        let k_service = hmac_sha256(&k_date, service.as_bytes());
        let k_signing = hmac_sha256(&k_service, b"tc3_request");
        let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));

        Ok(format!(
            "{} Credential={}/{}, SignedHeaders={}, Signature={}",
            algorithm, self.secret_id, credential_scope, signed_headers, signature
        ))
    }

    // COS Signature (HMAC-SHA1)
    fn sign_cos(&self, method: &str, uri: &str, _host: &str) -> String {
        let now = Utc::now().timestamp();
        let end = now + 600;
        let key_time = format!("{};{}", now, end);

        let mut mac = Hmac::<Sha1>::new_from_slice(self.secret_key.as_bytes()).expect("HMAC error");
        mac.update(key_time.as_bytes());
        let sign_key = hex::encode(mac.finalize().into_bytes());

        let http_string = format!("{}\n{}\n\n\n", method.to_lowercase(), uri);
        let string_to_sign = format!("sha1\n{}\n{}\n", key_time, sha1_hex(&http_string));

        let mut mac2 = Hmac::<Sha1>::new_from_slice(sign_key.as_bytes()).expect("HMAC error");
        mac2.update(string_to_sign.as_bytes());
        let signature = hex::encode(mac2.finalize().into_bytes());

        format!("q-sign-algorithm=sha1&q-ak={}&q-sign-time={}&q-key-time={}&q-header-list=&q-url-param-list=&q-signature={}", 
            self.secret_id, key_time, key_time, signature)
    }

    async fn cos_get_text(
        &self,
        host: &str,
        path_with_query: &str,
        canonical_uri: &str,
    ) -> Option<(u16, String)> {
        let auth = self.sign_cos("GET", canonical_uri, host);
        let separator = if path_with_query.contains('?') {
            '&'
        } else {
            '?'
        };
        let url = format!("https://{}{}{}{}", host, path_with_query, separator, auth);

        let response = self.client.get(&url).send().await.ok()?;
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

    fn cos_is_empty_bucket(payload: &str) -> Option<bool> {
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

    fn cos_lifecycle_missing(status: u16, payload: &str) -> bool {
        if status == 404 {
            return true;
        }

        let lower = payload.to_lowercase();
        lower.contains("nosuchlifecycleconfiguration") || lower.contains("no such lifecycle")
    }

    async fn post(
        &self,
        service: &str,
        action: &str,
        version: &str,
        params: serde_json::Value,
    ) -> Result<Value> {
        let host = format!("{}.tencentcloudapi.com", service);
        let url = format!("https://{}", host);
        let timestamp = Utc::now().timestamp();
        let body = serde_json::to_string(&params)?;

        let auth_header = self.sign_cvm(service, &body, timestamp)?;

        let res = self
            .client
            .post(&url)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Host", host)
            .header("X-TC-Action", action)
            .header("X-TC-Version", version)
            .header("X-TC-Timestamp", timestamp.to_string())
            .header("X-TC-Region", &self.region)
            .body(body)
            .send()
            .await?;

        if !res.status().is_success() {
            return Ok(serde_json::json!({}));
        }

        Ok(res.json().await?)
    }

    pub async fn scan_cvm(&self) -> Result<Vec<WastedResource>> {
        let params = serde_json::json!({
            "Limit": 100,
            "Filters": [{ "Name": "instance-state", "Values": ["STOPPED"] }]
        });
        let json = self
            .post("cvm", "DescribeInstances", "2017-03-12", params)
            .await?;
        let mut wastes = Vec::new();
        if let Some(instances) = json["Response"]["InstanceSet"].as_array() {
            for i in instances {
                let id = i["InstanceId"].as_str().unwrap_or("").to_string();
                let name = i["InstanceName"].as_str().unwrap_or("").to_string();
                let cpu = i["CPU"].as_i64().unwrap_or(0);
                let mem = i["Memory"].as_i64().unwrap_or(0);
                wastes.push(WastedResource {
                    id,
                    provider: "Tencent".to_string(),
                    region: self.region.clone(),
                    resource_type: "CVM Instance".to_string(),
                    details: format!("Stopped: {} ({}C/{}G)", name, cpu, mem),
                    estimated_monthly_cost: (cpu as f64 * 30.0),
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_cbs(&self) -> Result<Vec<WastedResource>> {
        let params = serde_json::json!({
            "Limit": 100,
            "Filters": [{ "Name": "disk-state", "Values": ["UNATTACHED"] }]
        });
        let json = self
            .post("cbs", "DescribeDisks", "2017-03-12", params)
            .await?;
        let mut wastes = Vec::new();
        if let Some(disks) = json["Response"]["DiskSet"].as_array() {
            for d in disks {
                let id = d["DiskId"].as_str().unwrap_or("").to_string();
                let size = d["DiskSize"].as_i64().unwrap_or(0);
                wastes.push(WastedResource {
                    id,
                    provider: "Tencent".to_string(),
                    region: self.region.clone(),
                    resource_type: "CBS Disk".to_string(),
                    details: format!("Unattached ({} GB)", size),
                    estimated_monthly_cost: size as f64 * 0.35,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_clb(&self) -> Result<Vec<WastedResource>> {
        let params = serde_json::json!({ "Limit": 100 });
        let json = self
            .post("clb", "DescribeLoadBalancers", "2018-03-17", params)
            .await?;
        let mut wastes = Vec::new();

        if let Some(load_balancers) = json["Response"]["LoadBalancerSet"].as_array() {
            for lb in load_balancers {
                let id = lb["LoadBalancerId"].as_str().unwrap_or("").to_string();
                if id.is_empty() {
                    continue;
                }

                let name = lb["LoadBalancerName"].as_str().unwrap_or("").to_string();

                let listeners_json = self
                    .post(
                        "clb",
                        "DescribeListeners",
                        "2018-03-17",
                        serde_json::json!({ "LoadBalancerId": id }),
                    )
                    .await
                    .unwrap_or_else(|_| serde_json::json!({}));

                if !listeners_json["Response"].is_object() {
                    continue;
                }

                if has_tencent_api_error(&listeners_json) {
                    continue;
                }

                let listener_count = listeners_json["Response"]["Listeners"]
                    .as_array()
                    .map(|v| v.len())
                    .or_else(|| {
                        listeners_json["Response"]["ListenerSet"]
                            .as_array()
                            .map(|v| v.len())
                    })
                    .unwrap_or(0);

                if listener_count == 0 {
                    wastes.push(WastedResource {
                        id,
                        provider: "Tencent".to_string(),
                        region: self.region.clone(),
                        resource_type: "CLB".to_string(),
                        details: format!("Idle (no listeners): {}", name),
                        estimated_monthly_cost: 60.0,
                        action_type: "DELETE".to_string(),
                    });
                    continue;
                }

                let mut backend_count = count_tencent_backend_targets(&listeners_json);

                if backend_count == 0 {
                    let listeners = listeners_json["Response"]["Listeners"]
                        .as_array()
                        .or_else(|| listeners_json["Response"]["ListenerSet"].as_array());

                    if let Some(listeners) = listeners {
                        for listener in listeners {
                            let listener_id = listener["ListenerId"].as_str().unwrap_or("");
                            if listener_id.is_empty() {
                                continue;
                            }

                            let targets_json = self
                                .post(
                                    "clb",
                                    "DescribeTargets",
                                    "2018-03-17",
                                    serde_json::json!({
                                        "LoadBalancerId": lb["LoadBalancerId"].as_str().unwrap_or(""),
                                        "ListenerIds": [listener_id]
                                    }),
                                )
                                .await
                                .unwrap_or_else(|_| serde_json::json!({}));

                            if !targets_json["Response"].is_object() {
                                continue;
                            }

                            if has_tencent_api_error(&targets_json) {
                                continue;
                            }

                            backend_count += count_tencent_backend_targets(&targets_json);
                            if backend_count > 0 {
                                break;
                            }
                        }
                    }
                }

                if backend_count == 0 {
                    wastes.push(WastedResource {
                        id,
                        provider: "Tencent".to_string(),
                        region: self.region.clone(),
                        resource_type: "CLB".to_string(),
                        details: format!(
                            "Idle ({} listeners, 0 backend targets): {}",
                            listener_count, name
                        ),
                        estimated_monthly_cost: 60.0,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }

        Ok(wastes)
    }

    pub async fn scan_cdb(&self) -> Result<Vec<WastedResource>> {
        let params = serde_json::json!({ "Limit": 100, "StatusCodes": [4, 5] });
        let json = self
            .post("cdb", "DescribeDBInstances", "2017-03-20", params)
            .await?;
        let mut wastes = Vec::new();
        if let Some(dbs) = json["Response"]["Items"].as_array() {
            for db in dbs {
                let id = db["InstanceId"].as_str().unwrap_or("").to_string();
                wastes.push(WastedResource {
                    id,
                    provider: "Tencent".to_string(),
                    region: self.region.clone(),
                    resource_type: "CDB".to_string(),
                    details: "Isolated DB".to_string(),
                    estimated_monthly_cost: 100.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_eips(&self) -> Result<Vec<WastedResource>> {
        let params = serde_json::json!({
            "Limit": 100,
            "Filters": [{ "Name": "status", "Values": ["UNBIND"] }]
        });
        let json = self
            .post("vpc", "DescribeAddresses", "2017-03-12", params)
            .await?;
        let mut wastes = Vec::new();
        if let Some(addrs) = json["Response"]["AddressSet"].as_array() {
            for a in addrs {
                let id = a["AddressId"].as_str().unwrap_or("").to_string();
                let ip = a["AddressIp"].as_str().unwrap_or("").to_string();
                wastes.push(WastedResource {
                    id,
                    provider: "Tencent".to_string(),
                    region: self.region.clone(),
                    resource_type: "EIP".to_string(),
                    details: format!("Unbound: {}", ip),
                    estimated_monthly_cost: 20.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_cos(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let Some((status, list_payload)) = self
            .cos_get_text("service.cos.myqcloud.com", "/", "/")
            .await
        else {
            return Ok(vec![]);
        };

        if !(200..300).contains(&status) {
            return Ok(vec![]);
        }

        let parsed = serde_xml_rs::from_str::<ListAllMyBucketsResult>(&list_payload).ok();
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

                // Determine Bucket Host (e.g., example-1250000000.cos.ap-guangzhou.myqcloud.com)
                let b_host = format!("{}.cos.{}.myqcloud.com", b.name, bucket_region);

                let mut empty_bucket = None;
                if let Some((object_status, object_payload)) = self
                    .cos_get_text(&b_host, "/?max-keys=1", "/?max-keys=1")
                    .await
                {
                    if (200..300).contains(&object_status) {
                        empty_bucket = Self::cos_is_empty_bucket(&object_payload);
                    }
                }

                if empty_bucket == Some(true) {
                    wastes.push(WastedResource {
                        id: b.name,
                        provider: "Tencent".to_string(),
                        region: bucket_region,
                        resource_type: "COS Bucket".to_string(),
                        details: "Empty bucket (0 objects).".to_string(),
                        estimated_monthly_cost: 1.0,
                        action_type: "DELETE".to_string(),
                    });
                    continue;
                }

                if let Some((lifecycle_status, lifecycle_payload)) = self
                    .cos_get_text(&b_host, "/?lifecycle", "/?lifecycle")
                    .await
                {
                    if Self::cos_lifecycle_missing(lifecycle_status, &lifecycle_payload) {
                        wastes.push(WastedResource {
                            id: b.name,
                            provider: "Tencent".to_string(),
                            region: bucket_region,
                            resource_type: "COS Bucket".to_string(),
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
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC error");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

fn has_tencent_api_error(value: &Value) -> bool {
    value["Response"]["Error"].is_object()
}

fn count_tencent_backend_targets(value: &Value) -> usize {
    match value {
        Value::Object(map) => {
            let mut total = 0;

            for key in ["Targets", "TargetSet", "BackendSet", "RealServerSet"] {
                if let Some(items) = map.get(key).and_then(|v| v.as_array()) {
                    total += items.len();
                }
            }

            for key in [
                "Listeners",
                "ListenerSet",
                "Rules",
                "RuleSet",
                "LocationSet",
                "Locations",
            ] {
                if let Some(items) = map.get(key).and_then(|v| v.as_array()) {
                    for item in items {
                        total += count_tencent_backend_targets(item);
                    }
                }
            }

            total
        }
        Value::Array(items) => items.iter().map(count_tencent_backend_targets).sum(),
        _ => 0,
    }
}

fn sha1_hex(msg: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(msg.as_bytes());
    hex::encode(hasher.finalize())
}

#[async_trait]
impl CloudProvider for TencentScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_cvm().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_cbs().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_clb().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_cdb().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_eips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_cos().await {
            results.extend(r);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scanner() -> TencentScanner {
        TencentScanner::new("sid-test", "sk-test", "ap-singapore")
    }

    #[test]
    fn cos_helpers_parse_xml_and_status_fallbacks() {
        let payload = "<ListBucketResult><KeyCount>0</KeyCount></ListBucketResult>";
        assert_eq!(
            TencentScanner::extract_xml_u64(payload, "KeyCount"),
            Some(0)
        );
        assert_eq!(TencentScanner::cos_is_empty_bucket(payload), Some(true));
        assert!(TencentScanner::cos_lifecycle_missing(404, ""));
        assert!(TencentScanner::cos_lifecycle_missing(
            200,
            "NoSuchLifecycleConfiguration"
        ));
    }

    #[test]
    fn sign_cos_has_expected_shape() {
        let auth = scanner().sign_cos("GET", "/", "bucket.cos.ap.sg.tencent.com");
        assert!(auth.contains("q-sign-algorithm=sha1"));
        assert!(auth.contains("q-ak=sid-test"));
        assert!(auth.contains("q-signature="));
    }

    #[test]
    fn sign_cvm_returns_tc3_authorization_header() {
        let auth = scanner()
            .sign_cvm("cvm", "{\"Limit\":1}", 1_710_000_000)
            .expect("sign cvm");
        assert!(auth.starts_with("TC3-HMAC-SHA256 Credential=sid-test/"));
        assert!(auth.contains("SignedHeaders=content-type;host"));
        assert!(auth.contains("Signature="));
    }

    #[test]
    fn backend_counter_walks_nested_listener_structures() {
        let payload = serde_json::json!({
            "Listeners": [
                { "Targets": [{"id":"a"}, {"id":"b"}] },
                { "Rules": [{ "TargetSet": [{"id":"c"}] }] }
            ]
        });
        assert_eq!(count_tencent_backend_targets(&payload), 3);
        assert!(has_tencent_api_error(
            &serde_json::json!({"Response":{"Error":{"code":"x"}}})
        ));
    }
}
