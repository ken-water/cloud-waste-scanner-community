use crate::models::WastedResource;
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

#[derive(Deserialize)]
struct OssListBucketsResult {
    #[serde(rename = "Buckets")]
    buckets: Option<OssBuckets>,
}

#[derive(Deserialize)]
struct OssBuckets {
    #[serde(rename = "Bucket")]
    bucket: Option<Vec<OssBucket>>,
}

#[derive(Deserialize)]
struct OssBucket {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Location")]
    location: String,
}

#[derive(Deserialize)]
struct OssBucketStat {
    #[serde(rename = "ObjectCount")]
    object_count: u64,
    #[serde(rename = "StorageSize")]
    storage_size: u64,
}

pub struct AlibabaScanner {
    client: Client,
    access_key_id: String,
    access_key_secret: String,
    region_id: String,
}

impl AlibabaScanner {
    pub fn new(key: &str, secret: &str, region: &str) -> Self {
        Self {
            client: Client::new(),
            access_key_id: key.to_string(),
            access_key_secret: secret.to_string(),
            region_id: region.to_string(),
        }
    }

    fn sign(&self, params: &Vec<(&str, String)>) -> String {
        let mut sorted_params = params.clone();
        sorted_params.sort_by_key(|k| k.0);

        let canonicalized_query_string = sorted_params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<String>>()
            .join("&");

        let string_to_sign = format!(
            "GET&%2F&{}",
            urlencoding::encode(&canonicalized_query_string)
        );

        let mut mac = HmacSha1::new_from_slice(format!("{}&", self.access_key_secret).as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(string_to_sign.as_bytes());
        let result = mac.finalize();
        STANDARD.encode(result.into_bytes())
    }

    async fn request(
        &self,
        domain: &str,
        version: &str,
        action: &str,
        extra_params: Vec<(&str, String)>,
    ) -> Result<Value> {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let nonce = uuid::Uuid::new_v4().to_string();

        let mut params = vec![
            ("Format", "JSON".to_string()),
            ("Version", version.to_string()),
            ("AccessKeyId", self.access_key_id.clone()),
            ("SignatureMethod", "HMAC-SHA1".to_string()),
            ("Timestamp", now),
            ("SignatureVersion", "1.0".to_string()),
            ("SignatureNonce", nonce),
            ("Action", action.to_string()),
            ("RegionId", self.region_id.clone()),
        ];

        params.extend(extra_params);

        let signature = self.sign(&params);

        let mut url = format!("https://{}/?", domain);
        for (i, (k, v)) in params.iter().enumerate() {
            if i > 0 {
                url.push('&');
            }
            url.push_str(&format!("{}={}", k, urlencoding::encode(v)));
        }
        url.push_str(&format!("&Signature={}", urlencoding::encode(&signature)));

        let res = self.client.get(&url).send().await?;
        let json: Value = res.json().await?;
        Ok(json)
    }

    fn has_api_error(json: &Value) -> bool {
        json["Code"].is_string()
    }

    fn slb_listener_count(data: &Value) -> usize {
        data["ListenerPorts"]["ListenerPort"]
            .as_array()
            .map(|v| v.len())
            .unwrap_or(0)
            + data["ListenerPortsAndProtocol"]["ListenerPortAndProtocol"]
                .as_array()
                .map(|v| v.len())
                .unwrap_or(0)
    }

    fn slb_backend_count(data: &Value) -> usize {
        data["BackendServers"]["BackendServer"]
            .as_array()
            .map(|v| v.len())
            .unwrap_or(0)
            + data["VServerGroups"]["VServerGroup"]
                .as_array()
                .map(|v| v.len())
                .unwrap_or(0)
    }

    fn parse_rds_performance_sum(data: &Value) -> Option<f64> {
        let mut sum = 0.0;
        let mut has_samples = false;

        if let Some(keys) = data["PerformanceKeys"]["PerformanceKey"].as_array() {
            for key in keys {
                if let Some(values) = key["Values"]["PerformanceValue"].as_array() {
                    for value in values {
                        if let Some(v) = value["Value"].as_str() {
                            if let Ok(parsed) = v.parse::<f64>() {
                                sum += parsed;
                                has_samples = true;
                            }
                        }
                    }
                }
            }
        }

        if has_samples {
            Some(sum)
        } else {
            None
        }
    }

    fn is_transient_rds_status(status: &str) -> bool {
        matches!(
            status,
            "creating"
                | "configuring"
                | "rebooting"
                | "switching"
                | "classchanging"
                | "nettypechanging"
                | "minorversionupgrading"
                | "majorversionupgrading"
                | "migrating"
                | "restoring"
                | "inspec"
        )
    }

    fn estimate_rds_monthly_cost(db: &Value) -> f64 {
        let storage_gb = db["DBInstanceStorage"]
            .as_str()
            .and_then(|v| v.parse::<f64>().ok())
            .or_else(|| db["DBInstanceStorage"].as_f64())
            .unwrap_or(20.0)
            .max(20.0);

        let class = db["DBInstanceClass"].as_str().unwrap_or("").to_lowercase();

        let class_factor = if class.contains("8xlarge") {
            4.0
        } else if class.contains("4xlarge") {
            3.0
        } else if class.contains("2xlarge") {
            2.0
        } else if class.contains("xlarge") {
            1.5
        } else {
            1.0
        };

        (storage_gb * 0.25 * class_factor).max(20.0)
    }

    async fn fetch_rds_idle_signal(&self, db_instance_id: &str) -> Result<Option<f64>> {
        let end_time = Utc::now();
        let start_time = end_time - Duration::days(7);
        let start = start_time.format("%Y-%m-%dT%H:%MZ").to_string();
        let end = end_time.format("%Y-%m-%dT%H:%MZ").to_string();

        for metric_key in [
            "MySQL_NetworkTraffic",
            "SQLServer_TotalTPS",
            "pg_stat_database_tup_fetched",
        ] {
            let json = self
                .request(
                    "rds.aliyuncs.com",
                    "2014-08-15",
                    "DescribeDBInstancePerformance",
                    vec![
                        ("DBInstanceId", db_instance_id.to_string()),
                        ("Key", metric_key.to_string()),
                        ("StartTime", start.clone()),
                        ("EndTime", end.clone()),
                    ],
                )
                .await?;

            if Self::has_api_error(&json) {
                continue;
            }

            if let Some(sum) = Self::parse_rds_performance_sum(&json) {
                return Ok(Some(sum));
            }
        }

        Ok(None)
    }

    pub async fn scan_disks(&self) -> Result<Vec<WastedResource>> {
        let json = self
            .request(
                "ecs.aliyuncs.com",
                "2014-05-26",
                "DescribeDisks",
                vec![("Status", "Available".to_string())],
            )
            .await?;
        let mut wastes = Vec::new();

        if let Some(disks) = json["Disks"]["Disk"].as_array() {
            for d in disks {
                let size = d["Size"].as_f64().unwrap_or(0.0);
                let id = d["DiskId"].as_str().unwrap_or("unknown").to_string();
                let category = d["Category"].as_str().unwrap_or("cloud_efficiency");
                let rate = if category.contains("ssd") { 0.14 } else { 0.05 };
                let cost = size * rate;

                wastes.push(WastedResource {
                    id,
                    provider: "Alibaba".to_string(),
                    region: self.region_id.clone(),
                    resource_type: "ECS Disk".to_string(),
                    details: format!("Orphaned {}GB ({})", size, category),
                    estimated_monthly_cost: cost,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_eips(&self) -> Result<Vec<WastedResource>> {
        let json = self
            .request(
                "ecs.aliyuncs.com",
                "2014-05-26",
                "DescribeEipAddresses",
                vec![("Status", "Available".to_string())],
            )
            .await?;
        let mut wastes = Vec::new();

        if let Some(eips) = json["EipAddresses"]["EipAddress"].as_array() {
            for e in eips {
                let id = e["AllocationId"].as_str().unwrap_or("unknown").to_string();
                let ip = e["IpAddress"].as_str().unwrap_or("").to_string();

                wastes.push(WastedResource {
                    id,
                    provider: "Alibaba".to_string(),
                    region: self.region_id.clone(),
                    resource_type: "EIP".to_string(),
                    details: format!("Unattached IP: {}", ip),
                    estimated_monthly_cost: 2.50,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_slb(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();

        let mut page_number = 1u64;
        loop {
            let json = self
                .request(
                    "slb.aliyuncs.com",
                    "2014-05-15",
                    "DescribeLoadBalancers",
                    vec![
                        ("PageSize", "100".to_string()),
                        ("PageNumber", page_number.to_string()),
                    ],
                )
                .await?;

            if Self::has_api_error(&json) {
                break;
            }

            let Some(load_balancers) = json["LoadBalancers"]["LoadBalancer"].as_array() else {
                break;
            };
            if load_balancers.is_empty() {
                break;
            }

            for lb in load_balancers {
                let id = lb["LoadBalancerId"].as_str().unwrap_or("").to_string();
                if id.is_empty() {
                    continue;
                }

                let name = lb["LoadBalancerName"].as_str().unwrap_or("").to_string();
                let status = lb["LoadBalancerStatus"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_lowercase();

                let mut listener_count = Self::slb_listener_count(lb);
                let mut backend_count = Self::slb_backend_count(lb);

                if listener_count == 0 || backend_count == 0 {
                    let attribute_json = self
                        .request(
                            "slb.aliyuncs.com",
                            "2014-05-15",
                            "DescribeLoadBalancerAttribute",
                            vec![("LoadBalancerId", id.clone())],
                        )
                        .await
                        .unwrap_or_else(|_| serde_json::json!({}));

                    if !Self::has_api_error(&attribute_json) {
                        listener_count =
                            listener_count.max(Self::slb_listener_count(&attribute_json));
                        backend_count = backend_count.max(Self::slb_backend_count(&attribute_json));
                    }
                }

                let is_idle = status.contains("inactive")
                    || status.contains("stopped")
                    || listener_count == 0
                    || backend_count == 0;

                if is_idle {
                    wastes.push(WastedResource {
                        id,
                        provider: "Alibaba".to_string(),
                        region: self.region_id.clone(),
                        resource_type: "SLB".to_string(),
                        details: format!(
                            "Idle Load Balancer: {} (status={}, listeners={}, backends={})",
                            name, status, listener_count, backend_count
                        ),
                        estimated_monthly_cost: 15.0,
                        action_type: "DELETE".to_string(),
                    });
                }
            }

            let total_count = json["TotalCount"].as_u64().unwrap_or(0);
            let page_size = json["PageSize"].as_u64().unwrap_or(100);
            if total_count == 0 || page_number.saturating_mul(page_size) >= total_count {
                break;
            }
            page_number += 1;
        }

        Ok(wastes)
    }

    pub async fn scan_rds(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();

        let mut page_number = 1u64;
        loop {
            let json = self
                .request(
                    "rds.aliyuncs.com",
                    "2014-08-15",
                    "DescribeDBInstances",
                    vec![
                        ("PageSize", "100".to_string()),
                        ("PageNumber", page_number.to_string()),
                    ],
                )
                .await?;

            if Self::has_api_error(&json) {
                break;
            }

            let Some(databases) = json["Items"]["DBInstance"].as_array() else {
                break;
            };
            if databases.is_empty() {
                break;
            }

            for db in databases {
                let id = db["DBInstanceId"].as_str().unwrap_or("").to_string();
                if id.is_empty() {
                    continue;
                }

                let name = db["DBInstanceDescription"]
                    .as_str()
                    .unwrap_or("Unnamed")
                    .to_string();
                let status = db["DBInstanceStatus"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_lowercase();
                let lock_mode = db["LockMode"].as_str().unwrap_or("none").to_lowercase();
                let est_cost = Self::estimate_rds_monthly_cost(db);

                let mut marked = false;

                if status == "running" {
                    if let Ok(Some(signal_sum)) = self.fetch_rds_idle_signal(&id).await {
                        if signal_sum < 1.0 {
                            wastes.push(WastedResource {
                                id: id.clone(),
                                provider: "Alibaba".to_string(),
                                region: self.region_id.clone(),
                                resource_type: "RDS".to_string(),
                                details: format!(
                                    "Running DB with near-zero performance signal over 7d: {}",
                                    name
                                ),
                                estimated_monthly_cost: est_cost,
                                action_type: "REVIEW".to_string(),
                            });
                            marked = true;
                        }
                    }
                }

                if !marked && lock_mode.contains("expiration") {
                    wastes.push(WastedResource {
                        id: id.clone(),
                        provider: "Alibaba".to_string(),
                        region: self.region_id.clone(),
                        resource_type: "RDS".to_string(),
                        details: format!("Locked by expiration: {}", name),
                        estimated_monthly_cost: est_cost,
                        action_type: "REVIEW".to_string(),
                    });
                    marked = true;
                }

                if !marked && status != "running" && !Self::is_transient_rds_status(&status) {
                    wastes.push(WastedResource {
                        id,
                        provider: "Alibaba".to_string(),
                        region: self.region_id.clone(),
                        resource_type: "RDS".to_string(),
                        details: format!("Non-running DB instance: {} (status={})", name, status),
                        estimated_monthly_cost: est_cost,
                        action_type: "REVIEW".to_string(),
                    });
                }
            }

            let total_count = json["TotalRecordCount"].as_u64().unwrap_or(0);
            let page_size = json["PageRecordCount"].as_u64().unwrap_or(100);
            if total_count == 0 || page_number.saturating_mul(page_size) >= total_count {
                break;
            }
            page_number += 1;
        }

        Ok(wastes)
    }

    // ... sign_oss ...

    fn sign_oss(&self, method: &str, date: &str, resource: &str) -> String {
        let string_to_sign = format!(
            "{}


{}
{}",
            method, date, resource
        );

        let mut mac =
            HmacSha1::new_from_slice(self.access_key_secret.as_bytes()).expect("HMAC error");
        mac.update(string_to_sign.as_bytes());
        let result = mac.finalize();
        STANDARD.encode(result.into_bytes())
    }

    pub async fn scan_oss_buckets(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let host = format!("oss-{}.aliyuncs.com", self.region_id);
        let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let resource = "/";

        let signature = self.sign_oss("GET", &date, resource);
        let url = format!("https://{}", host);

        let res = self
            .client
            .get(&url)
            .header("Date", date.clone())
            .header(
                "Authorization",
                format!("OSS {}:{}", self.access_key_id, signature),
            )
            .send()
            .await?;

        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let text = res.text().await?;
        let list: OssListBucketsResult = serde_xml_rs::from_str(&text)?;

        if let Some(buckets) = list.buckets {
            if let Some(bucket_list) = buckets.bucket {
                for b in bucket_list {
                    // 1. Get Stats (Size & Count)
                    let b_host = format!("{}.{}", b.name, host);
                    let b_date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
                    let b_resource = format!("/{}?stat", b.name);
                    let b_sig = self.sign_oss("GET", &b_date, &b_resource);
                    let b_url = format!("https://{}/?stat", b_host);

                    if let Ok(stat_res) = self
                        .client
                        .get(&b_url)
                        .header("Date", b_date)
                        .header(
                            "Authorization",
                            format!("OSS {}:{}", self.access_key_id, b_sig),
                        )
                        .send()
                        .await
                    {
                        if stat_res.status().is_success() {
                            let stat_text = stat_res.text().await?;
                            if let Ok(stat) = serde_xml_rs::from_str::<OssBucketStat>(&stat_text) {
                                // CASE 1: Empty Bucket -> DELETE
                                if stat.object_count == 0 {
                                    wastes.push(WastedResource {
                                        id: b.name.clone(),
                                        provider: "Alibaba".to_string(),
                                        region: b.location.clone(),
                                        resource_type: "OSS Bucket".to_string(),
                                        details: "Empty Bucket (0 Objects).".to_string(),
                                        estimated_monthly_cost: 0.0,
                                        action_type: "DELETE".to_string(),
                                    });
                                    continue;
                                }

                                // CASE 2: Non-empty (>1GB) without Lifecycle -> ARCHIVE
                                let size_gb = stat.storage_size as f64 / 1024.0 / 1024.0 / 1024.0;
                                if size_gb > 1.0 {
                                    let l_date =
                                        Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
                                    let l_resource = format!("/{}?lifecycle", b.name);
                                    let l_sig = self.sign_oss("GET", &l_date, &l_resource);
                                    let l_url = format!("https://{}/?lifecycle", b_host);

                                    let l_res = self
                                        .client
                                        .get(&l_url)
                                        .header("Date", l_date)
                                        .header(
                                            "Authorization",
                                            format!("OSS {}:{}", self.access_key_id, l_sig),
                                        )
                                        .send()
                                        .await;

                                    // 404 means no lifecycle policy exists
                                    if let Ok(r) = l_res {
                                        if r.status().as_u16() == 404 {
                                            let savings = size_gb * 0.10; // Est monthly savings (CNY)
                                            wastes.push(WastedResource {
                                                id: b.name,
                                                provider: "Alibaba".to_string(),
                                                region: b.location,
                                                resource_type: "OSS Bucket".to_string(),
                                                details: format!("Bucket ({:.1} GB) has NO Lifecycle Policy. Suggest auto-archiving cold data.", size_gb),
                                                estimated_monthly_cost: savings,
                                                action_type: "ARCHIVE".to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let json = self
            .request(
                "ecs.aliyuncs.com",
                "2014-05-26",
                "DescribeSnapshots",
                vec![("Status", "accomplished".to_string())],
            )
            .await?;

        let mut wastes = Vec::new();
        let cutoff = Utc::now() - Duration::days(180);

        if let Some(snaps) = json["Snapshots"]["Snapshot"].as_array() {
            for s in snaps {
                let id = s["SnapshotId"].as_str().unwrap_or("unknown").to_string();
                let name = s["SnapshotName"].as_str().unwrap_or("").to_string();
                let created = s["CreationTime"].as_str().unwrap_or("");
                let size = s["SourceDiskSize"]
                    .as_str()
                    .unwrap_or("0")
                    .parse::<f64>()
                    .unwrap_or(0.0);
                let s_type = s["SnapshotType"].as_str().unwrap_or("user");

                if let Ok(ts) = DateTime::parse_from_rfc3339(created) {
                    if ts.with_timezone(&Utc) < cutoff {
                        // Snapshot pricing varies, approx $0.04/GB
                        let cost = size * 0.04;
                        wastes.push(WastedResource {
                            id,
                            provider: "Alibaba".to_string(),
                            region: self.region_id.clone(),
                            resource_type: "ECS Snapshot".to_string(),
                            details: format!("Old {} Snapshot (>180 days). Name: {}", s_type, name),
                            estimated_monthly_cost: cost,
                            action_type: "DELETE".to_string(),
                        });
                    }
                }
            }
        }
        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for AlibabaScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_disks().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_eips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_oss_buckets().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_snapshots().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_slb().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_rds().await {
            results.extend(r);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scanner() -> AlibabaScanner {
        AlibabaScanner::new("ak-test", "sk-test", "cn-hangzhou")
    }

    #[test]
    fn sign_and_slb_helpers_produce_expected_values() {
        let params = vec![
            ("Action", "DescribeDisks".to_string()),
            ("Format", "JSON".to_string()),
        ];
        let signature = scanner().sign(&params);
        assert!(!signature.is_empty());

        let payload = json!({
            "ListenerPorts": {"ListenerPort":[80,443]},
            "ListenerPortsAndProtocol": {"ListenerPortAndProtocol":[{"Port":8080}]},
            "BackendServers": {"BackendServer":[{"id":"i-1"}]},
            "VServerGroups": {"VServerGroup":[{"id":"v-1"},{"id":"v-2"}]}
        });
        assert_eq!(AlibabaScanner::slb_listener_count(&payload), 3);
        assert_eq!(AlibabaScanner::slb_backend_count(&payload), 3);
    }

    #[test]
    fn rds_helpers_parse_metric_sum_and_status() {
        let perf = json!({
            "PerformanceKeys": {
                "PerformanceKey": [
                    {"Values":{"PerformanceValue":[{"Value":"1.2"},{"Value":"3.3"}]}}
                ]
            }
        });
        let sum = AlibabaScanner::parse_rds_performance_sum(&perf).expect("sum");
        assert!((sum - 4.5).abs() < f64::EPSILON);
        assert!(AlibabaScanner::is_transient_rds_status("creating"));
        assert!(!AlibabaScanner::is_transient_rds_status("running"));
    }

    #[test]
    fn rds_monthly_cost_has_floor_and_class_factor() {
        let small = json!({
            "DBInstanceStorage": "10",
            "DBInstanceClass": "rds.mysql.s2.medium"
        });
        let large = json!({
            "DBInstanceStorage": "100",
            "DBInstanceClass": "rds.mysql.8xlarge"
        });
        assert!(AlibabaScanner::estimate_rds_monthly_cost(&small) >= 20.0);
        assert!(AlibabaScanner::estimate_rds_monthly_cost(&large) > 20.0);
    }
}
