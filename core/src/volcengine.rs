use crate::models::WastedResource;
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Credentials, Region};
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::Client as S3Client;
use chrono::{TimeZone, Utc};
use hex;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use urlencoding::encode;

pub struct VolcengineScanner {
    client: Client,
    ak: String,
    sk: String,
    region: String,
}

impl VolcengineScanner {
    pub fn new(ak: &str, sk: &str, region: &str) -> Self {
        Self {
            client: Client::new(),
            ak: ak.to_string(),
            sk: sk.to_string(),
            region: region.to_string(),
        }
    }

    // ECS/VPC Signature (HMAC-SHA256 Query Param)
    fn sign_ecs(
        &self,
        service: &str,
        method: &str,
        path: &str,
        query: &str,
        body: &str,
        timestamp: i64,
    ) -> Result<(String, String)> {
        let date_long = Utc
            .timestamp_opt(timestamp, 0)
            .unwrap()
            .format("%Y%m%dT%H%M%SZ")
            .to_string();
        let date_short = Utc
            .timestamp_opt(timestamp, 0)
            .unwrap()
            .format("%Y%m%d")
            .to_string();
        let host = "open.volcengineapi.com";

        let canonical_headers = format!("host:{}\nx-date:{}\n", host, date_long);
        let signed_headers = "host;x-date";
        let payload_hash = hex::encode(Sha256::digest(body.as_bytes()));

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method, path, query, canonical_headers, signed_headers, payload_hash
        );

        let algorithm = "HMAC-SHA256";
        let credential_scope = format!("{}/{}/{}/request", date_short, self.region, service);
        let request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));

        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm, date_long, credential_scope, request_hash
        );

        let k_date = hmac_sha256(self.sk.as_bytes(), date_short.as_bytes());
        let k_region = hmac_sha256(&k_date, self.region.as_bytes());
        let k_service = hmac_sha256(&k_region, service.as_bytes());
        let k_signing = hmac_sha256(&k_service, b"request");
        let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));

        let auth = format!(
            "{} Credential={}/{}, SignedHeaders={}, Signature={}",
            algorithm, self.ak, credential_scope, signed_headers, signature
        );

        Ok((auth, date_long))
    }

    pub async fn scan_tos(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let endpoints = [
            format!("https://tos-s3-{}.volces.com", self.region),
            format!("https://tos-{}.volces.com", self.region),
        ];

        for endpoint in endpoints {
            let s3_conf = S3ConfigBuilder::new()
                .region(Region::new(self.region.clone()))
                .credentials_provider(Credentials::new(
                    self.ak.clone(),
                    self.sk.clone(),
                    None,
                    None,
                    "volcengine-static-aksk",
                ))
                .endpoint_url(endpoint)
                .force_path_style(true)
                .behavior_version_latest()
                .build();

            let s3_client = S3Client::from_conf(s3_conf);

            let buckets = match s3_client.list_buckets().send().await {
                Ok(resp) => resp,
                Err(_) => continue,
            };

            for bucket in buckets.buckets() {
                let name = match bucket.name().map(|v| v.to_string()) {
                    Some(v) if !v.is_empty() => v,
                    _ => continue,
                };

                let object_probe = s3_client
                    .list_objects_v2()
                    .bucket(&name)
                    .max_keys(1)
                    .send()
                    .await;

                let is_empty_bucket = match object_probe {
                    Ok(output) => {
                        let key_count = output.key_count().unwrap_or(0);
                        let has_contents = !output.contents().is_empty();
                        key_count == 0 && !has_contents
                    }
                    Err(_) => false,
                };

                if is_empty_bucket {
                    wastes.push(WastedResource {
                        id: name,
                        provider: "Volcengine".to_string(),
                        region: self.region.clone(),
                        resource_type: "TOS Bucket".to_string(),
                        details: "Empty bucket (0 objects).".to_string(),
                        estimated_monthly_cost: 1.0,
                        action_type: "DELETE".to_string(),
                    });
                    continue;
                }

                let lifecycle = s3_client
                    .get_bucket_lifecycle_configuration()
                    .bucket(&name)
                    .send()
                    .await;

                let missing_lifecycle = match lifecycle {
                    Ok(cfg) => cfg.rules().is_empty(),
                    Err(err) => err
                        .as_service_error()
                        .and_then(|service_err| service_err.code())
                        .map(|code| code == "NoSuchLifecycleConfiguration")
                        .unwrap_or(false),
                };

                if missing_lifecycle {
                    wastes.push(WastedResource {
                        id: name,
                        provider: "Volcengine".to_string(),
                        region: self.region.clone(),
                        resource_type: "TOS Bucket".to_string(),
                        details: "No lifecycle policy configured. Suggest archiving cold data."
                            .to_string(),
                        estimated_monthly_cost: 5.0,
                        action_type: "ARCHIVE".to_string(),
                    });
                }
            }

            return Ok(wastes);
        }

        Ok(vec![])
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let query = "Action=DescribeInstances&Version=2020-04-01&Status=Stopped";
        let timestamp = Utc::now().timestamp();
        let (auth, date) = self.sign_ecs("ecs", "GET", "/", query, "", timestamp)?;
        let url = format!("https://open.volcengineapi.com/?{}", query);

        let res = self
            .client
            .get(&url)
            .header("Authorization", auth)
            .header("X-Date", date)
            .header("Host", "open.volcengineapi.com")
            .send()
            .await?;

        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: Value = res.json().await?;
        let mut wastes = Vec::new();

        if let Some(instances) = json["Result"]["Instances"].as_array() {
            for i in instances {
                let id = i["InstanceId"].as_str().unwrap_or("unknown").to_string();
                let name = i["InstanceName"].as_str().unwrap_or("").to_string();

                wastes.push(WastedResource {
                    id,
                    provider: "Volcengine".to_string(),
                    region: self.region.clone(),
                    resource_type: "ECS Instance".to_string(),
                    details: format!("Stopped Instance: {}", name),
                    estimated_monthly_cost: 50.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_ebs(&self) -> Result<Vec<WastedResource>> {
        let query = "Action=DescribeVolumes&Version=2020-04-01&Status=Available";
        let timestamp = Utc::now().timestamp();
        let (auth, date) = self.sign_ecs("ecs", "GET", "/", query, "", timestamp)?;
        let url = format!("https://open.volcengineapi.com/?{}", query);

        let res = self
            .client
            .get(&url)
            .header("Authorization", auth)
            .header("X-Date", date)
            .header("Host", "open.volcengineapi.com")
            .send()
            .await?;

        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: Value = res.json().await?;
        let mut wastes = Vec::new();

        if let Some(volumes) = json["Result"]["Volumes"].as_array() {
            for v in volumes {
                let id = v["VolumeId"].as_str().unwrap_or("unknown").to_string();
                let name = v["VolumeName"].as_str().unwrap_or("").to_string();
                let size = v["Size"].as_i64().unwrap_or(0);

                wastes.push(WastedResource {
                    id,
                    provider: "Volcengine".to_string(),
                    region: self.region.clone(),
                    resource_type: "EBS Volume".to_string(),
                    details: format!("Unattached Volume: {} ({} GB)", name, size),
                    estimated_monthly_cost: size as f64 * 0.3,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_eips(&self) -> Result<Vec<WastedResource>> {
        let query = "Action=DescribeEipAddresses&Version=2020-04-01&Status=Available";
        let timestamp = Utc::now().timestamp();
        let (auth, date) = self.sign_ecs("vpc", "GET", "/", query, "", timestamp)?;
        let url = format!("https://open.volcengineapi.com/?{}", query);

        let res = self
            .client
            .get(&url)
            .header("Authorization", auth)
            .header("X-Date", date)
            .header("Host", "open.volcengineapi.com")
            .send()
            .await?;

        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: Value = res.json().await?;
        let mut wastes = Vec::new();

        if let Some(eips) = json["Result"]["EipAddresses"].as_array() {
            for e in eips {
                let id = e["AllocationId"].as_str().unwrap_or("unknown").to_string();
                let ip = e["EipAddress"].as_str().unwrap_or("").to_string();

                wastes.push(WastedResource {
                    id,
                    provider: "Volcengine".to_string(),
                    region: self.region.clone(),
                    resource_type: "EIP".to_string(),
                    details: format!("Unbound EIP: {}", ip),
                    estimated_monthly_cost: 20.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_clb(&self) -> Result<Vec<WastedResource>> {
        let query = "Action=DescribeLoadBalancers&Version=2020-04-01";
        let timestamp = Utc::now().timestamp();
        let (auth, date) = self.sign_ecs("clb", "GET", "/", query, "", timestamp)?;
        let url = format!("https://open.volcengineapi.com/?{}", query);

        let res = self
            .client
            .get(&url)
            .header("Authorization", auth)
            .header("X-Date", date)
            .header("Host", "open.volcengineapi.com")
            .send()
            .await?;

        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: Value = res.json().await?;
        let mut wastes = Vec::new();

        if let Some(lbs) = json["Result"]["LoadBalancers"].as_array() {
            for lb in lbs {
                let id = lb["LoadBalancerId"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let name = lb["LoadBalancerName"].as_str().unwrap_or("").to_string();
                let status = lb["Status"].as_str().unwrap_or("Active");

                if status == "Inactive" {
                    wastes.push(WastedResource {
                        id,
                        provider: "Volcengine".to_string(),
                        region: self.region.clone(),
                        resource_type: "CLB".to_string(),
                        details: format!("Inactive Load Balancer: {}", name),
                        estimated_monthly_cost: 60.0,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    fn sign_vke_redis(
        &self,
        method: &str,
        path: &str,
        query: &str,
        host: &str,
        timestamp: i64,
    ) -> Result<(String, String)> {
        let date_long = Utc
            .timestamp_opt(timestamp, 0)
            .unwrap()
            .format("%Y%m%dT%H%M%SZ")
            .to_string();
        let date_short = Utc
            .timestamp_opt(timestamp, 0)
            .unwrap()
            .format("%Y%m%d")
            .to_string();

        let canonical_headers = format!("host:{}\nx-date:{}\n", host, date_long);
        let signed_headers = "host;x-date";
        let payload_hash = hex::encode(Sha256::digest(b""));
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method, path, query, canonical_headers, signed_headers, payload_hash
        );

        let algorithm = "HMAC-SHA256";
        let credential_scope = format!("{}/{}/{}/request", date_short, self.region, "redis");
        let request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm, date_long, credential_scope, request_hash
        );

        let k_date = hmac_sha256(self.sk.as_bytes(), date_short.as_bytes());
        let k_region = hmac_sha256(&k_date, self.region.as_bytes());
        let k_service = hmac_sha256(&k_region, b"redis");
        let k_signing = hmac_sha256(&k_service, b"request");
        let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));

        let auth = format!(
            "{} Credential={}/{}, SignedHeaders={}, Signature={}",
            algorithm, self.ak, credential_scope, signed_headers, signature
        );

        Ok((auth, date_long))
    }

    pub async fn scan_redis(&self) -> Result<Vec<WastedResource>> {
        let host = "open.volcengineapi.com";
        let query = format!(
            "Action=DescribeDBInstances&Version=2020-12-07&RegionId={}",
            encode(&self.region)
        );

        let timestamp = Utc::now().timestamp();
        let (auth, date) = self.sign_vke_redis("GET", "/", &query, host, timestamp)?;
        let url = format!("https://{}?{}", host, query);

        let res = self
            .client
            .get(&url)
            .header("Authorization", auth)
            .header("X-Date", date)
            .header("Host", host)
            .send()
            .await?;

        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: Value = res.json().await?;
        let mut wastes = Vec::new();

        if let Some(instances) = json["Result"]["DBInstances"].as_array() {
            for instance in instances {
                let status = instance["Status"].as_str().unwrap_or("");
                let conn_count = instance["Connections"].as_i64().unwrap_or(0);
                let id = instance["InstanceId"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let name = instance["InstanceName"].as_str().unwrap_or("").to_string();

                if status.eq_ignore_ascii_case("Running") && conn_count == 0 {
                    wastes.push(WastedResource {
                        id,
                        provider: "Volcengine".to_string(),
                        region: self.region.clone(),
                        resource_type: "Redis Instance".to_string(),
                        details: format!("No active connections: {}", name),
                        estimated_monthly_cost: 80.0,
                        action_type: "DELETE".to_string(),
                    });
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
impl CloudProvider for VolcengineScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_instances().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_ebs().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_eips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_clb().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_redis().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_tos().await {
            results.extend(r);
        }
        Ok(results)
    }
}
