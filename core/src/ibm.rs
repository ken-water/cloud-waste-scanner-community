use anyhow::Result;
use async_trait::async_trait;
use reqwest::{Client, RequestBuilder};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct IbmScanner {
    client: Client,
    api_key: String,
    region: String,
    cos_endpoint: String,
    cos_service_instance_id: Option<String>,
}

#[derive(Deserialize)]
struct IbmTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct IbmInstanceList {
    instances: Vec<IbmInstance>,
}

#[derive(Deserialize)]
struct IbmInstance {
    id: String,
    name: String,
    status: String,
    #[allow(dead_code)]
    primary_network_interface: Option<IbmNic>,
}

#[derive(Deserialize)]
struct IbmNic {
    #[allow(dead_code)]
    primary_ipv4_address: Option<String>,
}

#[derive(Deserialize)]
struct IbmFloatingIpList {
    floating_ips: Vec<IbmFloatingIp>,
}

#[derive(Deserialize)]
struct IbmFloatingIp {
    id: String,
    address: String,
    status: String,
    target: Option<Value>,
}

#[derive(Deserialize)]
struct IbmCosListAllMyBucketsResult {
    #[serde(rename = "Buckets")]
    buckets: Option<IbmCosBuckets>,
}

#[derive(Deserialize)]
struct IbmCosBuckets {
    #[serde(rename = "Bucket")]
    bucket: Option<Vec<IbmCosBucket>>,
}

#[derive(Deserialize)]
struct IbmCosBucket {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Location")]
    location: Option<String>,
}

#[derive(Deserialize)]
struct IbmVolumeList {
    volumes: Vec<IbmVolume>,
}

#[derive(Deserialize)]
struct IbmVolume {
    id: String,
    name: String,
    status: String,
    capacity: Option<u64>,
    profile: Option<IbmVolumeProfile>,
}

#[derive(Deserialize)]
struct IbmVolumeProfile {
    name: Option<String>,
}

#[derive(Deserialize)]
struct IbmLoadBalancerList {
    load_balancers: Vec<IbmLoadBalancer>,
}

#[derive(Deserialize)]
struct IbmLoadBalancer {
    id: String,
    name: String,
    pools: Option<Vec<Value>>,
    provisioning_status: Option<String>,
}

#[derive(Deserialize)]
struct IbmSnapshotList {
    snapshots: Vec<IbmSnapshot>,
}

#[derive(Deserialize)]
struct IbmSnapshot {
    id: String,
    name: String,
    created_at: Option<String>,
}

impl IbmScanner {
    pub fn new(
        key: &str,
        region: &str,
        cos_endpoint: Option<&str>,
        cos_service_instance_id: Option<&str>,
    ) -> Self {
        let normalized_region = if region.trim().is_empty() {
            "us-south".to_string()
        } else {
            region.trim().to_string()
        };

        let normalized_cos_instance_id =
            cos_service_instance_id.and_then(Self::normalize_service_instance_id);

        Self {
            client: Client::new(),
            api_key: key.trim().to_string(),
            region: normalized_region.clone(),
            cos_endpoint: Self::normalize_cos_endpoint(
                cos_endpoint.unwrap_or(""),
                &normalized_region,
            ),
            cos_service_instance_id: normalized_cos_instance_id,
        }
    }

    fn normalize_service_instance_id(raw: &str) -> Option<String> {
        let value = raw.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }

    fn normalize_cos_endpoint(raw: &str, region: &str) -> String {
        let endpoint = raw.trim();
        if endpoint.is_empty() {
            return format!("https://s3.{}.cloud-object-storage.appdomain.cloud", region);
        }

        let mut normalized = endpoint.to_string();
        if !normalized.starts_with("http://") && !normalized.starts_with("https://") {
            normalized = format!("https://{}", normalized);
        }

        normalized.trim_end_matches('/').to_string()
    }

    fn cos_request(
        &self,
        token: &str,
        url: &str,
        service_instance_id: Option<&str>,
    ) -> RequestBuilder {
        let mut request = self.client.get(url).bearer_auth(token);
        if let Some(service_instance_id) =
            service_instance_id.and_then(Self::normalize_service_instance_id)
        {
            request = request.header("ibm-service-instance-id", service_instance_id);
        }
        request
    }

    fn extract_resource_guid_from_id(raw: &str) -> Option<String> {
        let value = raw.trim();
        if value.is_empty() {
            return None;
        }

        let tail = value.rsplit('/').next().unwrap_or(value).trim();
        if tail.is_empty() {
            None
        } else {
            Some(tail.to_string())
        }
    }

    fn push_unique_string(items: &mut Vec<String>, seen: &mut HashSet<String>, raw: &str) {
        let value = raw.trim();
        if value.is_empty() {
            return;
        }

        if seen.insert(value.to_string()) {
            items.push(value.to_string());
        }
    }

    fn is_cos_resource_instance(resource: &Value) -> bool {
        let mut search = String::new();
        for key in ["resource_id", "crn", "name", "resource_name", "type"] {
            if let Some(text) = resource[key].as_str() {
                search.push_str(text);
                search.push(' ');
            }
        }

        let lower = search.to_lowercase();
        lower.contains("cloud-object-storage") || lower.contains("cloud object storage")
    }

    async fn discover_cos_service_instance_ids(&self, token: &str) -> Vec<String> {
        let mut discovered = Vec::new();
        let mut seen = HashSet::new();

        let url = "https://resource-controller.cloud.ibm.com/v2/resource_instances?type=service_instance&limit=200";
        let response = match self.client.get(url).bearer_auth(token).send().await {
            Ok(resp) => resp,
            Err(_) => return discovered,
        };

        if !response.status().is_success() {
            return discovered;
        }

        let payload: Value = match response.json().await {
            Ok(value) => value,
            Err(_) => return discovered,
        };

        let Some(resources) = payload["resources"].as_array() else {
            return discovered;
        };

        for resource in resources {
            if !Self::is_cos_resource_instance(resource) {
                continue;
            }

            if let Some(guid) = resource["guid"].as_str() {
                Self::push_unique_string(&mut discovered, &mut seen, guid);
            }
            if let Some(id) = resource["id"].as_str() {
                Self::push_unique_string(&mut discovered, &mut seen, id);
                if let Some(guid) = Self::extract_resource_guid_from_id(id) {
                    Self::push_unique_string(&mut discovered, &mut seen, &guid);
                }
            }
            if let Some(crn) = resource["crn"].as_str() {
                Self::push_unique_string(&mut discovered, &mut seen, crn);
            }
        }

        discovered
    }

    async fn resolve_cos_service_instance_candidates(
        &self,
        token: &str,
    ) -> (Vec<Option<String>>, usize) {
        let discovered_ids = self.discover_cos_service_instance_ids(token).await;
        let discovered_count = discovered_ids.len();
        let mut candidates: Vec<Option<String>> = Vec::new();

        if let Some(configured) = self.cos_service_instance_id.clone() {
            candidates.push(Some(configured));
        }

        candidates.push(None);

        for discovered in discovered_ids {
            let exists = candidates
                .iter()
                .any(|value| value.as_deref() == Some(discovered.as_str()));
            if !exists {
                candidates.push(Some(discovered));
            }
        }

        (candidates, discovered_count)
    }

    async fn cos_get_text_with_candidates(
        &self,
        token: &str,
        url: &str,
        candidates: &[Option<String>],
    ) -> Option<(u16, String, Option<String>)> {
        let mut first_failure: Option<(u16, String, Option<String>)> = None;

        for candidate in candidates {
            let response = match self
                .cos_request(token, url, candidate.as_deref())
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(_) => continue,
            };

            let status = response.status().as_u16();
            let body = match response.text().await {
                Ok(text) => text,
                Err(_) => continue,
            };

            if (200..300).contains(&status) || status == 404 {
                return Some((status, body, candidate.clone()));
            }

            if first_failure.is_none() {
                first_failure = Some((status, body, candidate.clone()));
            }
        }

        first_failure
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
        lower.contains("nosuchlifecycleconfiguration")
            || lower.contains("no such lifecycle")
            || lower.contains("lifecycle configuration does not exist")
    }

    async fn get_token(&self) -> Result<String> {
        let url = "https://iam.cloud.ibm.com/identity/token";
        let params = [
            ("grant_type", "urn:ibm:params:oauth:grant-type:apikey"),
            ("apikey", &self.api_key),
        ];

        let res = self
            .client
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?
            .error_for_status()?;

        let json: IbmTokenResponse = res.json().await?;
        Ok(json.access_token)
    }

    pub async fn check_auth(&self) -> Result<()> {
        self.get_token().await.map(|_| ())
    }

    pub async fn check_cos_access_summary(&self) -> Result<String> {
        let token = self.get_token().await?;
        let (candidates, discovered_count) =
            self.resolve_cos_service_instance_candidates(&token).await;

        let Some((status, _, selected_candidate)) = self
            .cos_get_text_with_candidates(&token, &self.cos_endpoint, &candidates)
            .await
        else {
            return Ok(format!(
                "COS check: endpoint unreachable (auto-discovered candidates: {}).",
                discovered_count
            ));
        };

        let selected_source = match selected_candidate.as_deref() {
            Some(value) if self.cos_service_instance_id.as_deref() == Some(value) => {
                "configured service instance id"
            }
            Some(_) => "auto-discovered service instance id",
            None => "no service instance id header",
        };

        if (200..300).contains(&status) {
            Ok(format!(
                "COS check: reachable using {} (auto-discovered candidates: {}).",
                selected_source, discovered_count
            ))
        } else {
            Ok(format!(
                "COS check: returned HTTP {} using {} (auto-discovered candidates: {}).",
                status, selected_source, discovered_count
            ))
        }
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://{}.iaas.cloud.ibm.com/v1/instances?version=2023-01-01&generation=2",
            self.region
        );

        let res = self.client.get(&url).bearer_auth(&token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: IbmInstanceList = res.json().await?;
        let mut wastes = Vec::new();

        for instance in list.instances {
            if instance.status == "stopped" {
                wastes.push(WastedResource {
                    id: instance.id,
                    provider: "IBM".to_string(),
                    region: self.region.clone(),
                    resource_type: "VPC Instance".to_string(),
                    details: format!("Stopped Instance: {}", instance.name),
                    estimated_monthly_cost: 40.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_floating_ips(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://{}.iaas.cloud.ibm.com/v1/floating_ips?version=2023-01-01&generation=2",
            self.region
        );

        let res = self.client.get(&url).bearer_auth(&token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: IbmFloatingIpList = res.json().await?;
        let mut wastes = Vec::new();

        for ip in list.floating_ips {
            if ip.status == "available" && ip.target.is_none() {
                wastes.push(WastedResource {
                    id: ip.id,
                    provider: "IBM".to_string(),
                    region: self.region.clone(),
                    resource_type: "Floating IP".to_string(),
                    details: format!("Unbound IP: {}", ip.address),
                    estimated_monthly_cost: 1.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_cos(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let (candidates, _) = self.resolve_cos_service_instance_candidates(&token).await;

        let Some((list_status, list_payload, working_instance)) = self
            .cos_get_text_with_candidates(&token, &self.cos_endpoint, &candidates)
            .await
        else {
            return Ok(vec![]);
        };

        if !(200..300).contains(&list_status) {
            return Ok(vec![]);
        }

        let parsed = serde_xml_rs::from_str::<IbmCosListAllMyBucketsResult>(&list_payload).ok();
        let Some(buckets) = parsed
            .and_then(|result| result.buckets)
            .and_then(|items| items.bucket)
        else {
            return Ok(vec![]);
        };

        let mut followup_candidates = vec![working_instance];
        for candidate in candidates {
            let exists = followup_candidates
                .iter()
                .any(|value| value.as_ref() == candidate.as_ref());
            if !exists {
                followup_candidates.push(candidate);
            }
        }

        let mut wastes = Vec::new();

        for bucket in buckets {
            let bucket_name = bucket.name.trim();
            if bucket_name.is_empty() {
                continue;
            }

            let bucket_region = bucket.location.unwrap_or_else(|| self.region.clone());
            let object_url = format!("{}/{}?max-keys=1", self.cos_endpoint, bucket_name);
            let lifecycle_url = format!("{}/{}?lifecycle", self.cos_endpoint, bucket_name);

            let mut empty_bucket = None;
            if let Some((status, payload, _)) = self
                .cos_get_text_with_candidates(&token, &object_url, &followup_candidates)
                .await
            {
                if (200..300).contains(&status) {
                    empty_bucket = Self::cos_is_empty_bucket(&payload);
                }
            }

            if empty_bucket == Some(true) {
                wastes.push(WastedResource {
                    id: bucket_name.to_string(),
                    provider: "IBM".to_string(),
                    region: bucket_region,
                    resource_type: "COS Bucket".to_string(),
                    details: "Empty bucket (0 objects).".to_string(),
                    estimated_monthly_cost: 1.0,
                    action_type: "DELETE".to_string(),
                });
                continue;
            }

            if let Some((status, payload, _)) = self
                .cos_get_text_with_candidates(&token, &lifecycle_url, &followup_candidates)
                .await
            {
                if Self::cos_lifecycle_missing(status, &payload) {
                    wastes.push(WastedResource {
                        id: bucket_name.to_string(),
                        provider: "IBM".to_string(),
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

        Ok(wastes)
    }

    pub async fn scan_block_storage(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://{}.iaas.cloud.ibm.com/v1/volumes?version=2023-01-01&generation=2",
            self.region
        );

        let res = self.client.get(&url).bearer_auth(&token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: IbmVolumeList = res.json().await?;
        let mut wastes = Vec::new();

        for volume in list.volumes {
            if volume.status == "available" || volume.status == "unattached" {
                let cap = volume.capacity.unwrap_or(0) as f64;
                let unit_cost = volume
                    .profile
                    .as_ref()
                    .and_then(|profile| profile.name.as_ref())
                    .map(|profile| {
                        if profile.contains("5iops") {
                            0.12
                        } else {
                            0.10
                        }
                    })
                    .unwrap_or(0.10);

                wastes.push(WastedResource {
                    id: volume.id,
                    provider: "IBM".to_string(),
                    region: self.region.clone(),
                    resource_type: "Block Storage".to_string(),
                    details: format!("Unattached block volume: {}", volume.name),
                    estimated_monthly_cost: (cap * unit_cost).max(2.0),
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_load_balancers(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://{}.iaas.cloud.ibm.com/v1/load_balancers?version=2023-01-01&generation=2",
            self.region
        );

        let res = self.client.get(&url).bearer_auth(&token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: IbmLoadBalancerList = res.json().await?;
        let mut wastes = Vec::new();

        for lb in list.load_balancers {
            let pools_count = lb.pools.as_ref().map(|pools| pools.len()).unwrap_or(0);
            if pools_count == 0 && lb.provisioning_status.as_deref() == Some("active") {
                wastes.push(WastedResource {
                    id: lb.id,
                    provider: "IBM".to_string(),
                    region: self.region.clone(),
                    resource_type: "Load Balancer".to_string(),
                    details: format!("No backend pools: {}", lb.name),
                    estimated_monthly_cost: 45.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://{}.iaas.cloud.ibm.com/v1/snapshots?version=2023-01-01&generation=2",
            self.region
        );

        let res = self.client.get(&url).bearer_auth(&token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: IbmSnapshotList = res.json().await?;
        let cutoff = chrono::Utc::now() - chrono::Duration::days(30);
        let mut wastes = Vec::new();

        for snapshot in list.snapshots {
            let is_old = snapshot
                .created_at
                .as_ref()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|date| date.with_timezone(&chrono::Utc) < cutoff)
                .unwrap_or(false);

            if is_old {
                wastes.push(WastedResource {
                    id: snapshot.id,
                    provider: "IBM".to_string(),
                    region: self.region.clone(),
                    resource_type: "Snapshot".to_string(),
                    details: format!("Old snapshot: {}", snapshot.name),
                    estimated_monthly_cost: 5.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for IbmScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(resources) = self.scan_instances().await {
            results.extend(resources);
        }
        if let Ok(resources) = self.scan_floating_ips().await {
            results.extend(resources);
        }
        if let Ok(resources) = self.scan_block_storage().await {
            results.extend(resources);
        }
        if let Ok(resources) = self.scan_load_balancers().await {
            results.extend(resources);
        }
        if let Ok(resources) = self.scan_snapshots().await {
            results.extend(resources);
        }
        if let Ok(resources) = self.scan_cos().await {
            results.extend(resources);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::IbmScanner;
    use serde_json::json;

    #[test]
    fn normalize_helpers_trim_and_apply_defaults() {
        assert_eq!(
            IbmScanner::normalize_service_instance_id("  abc-123  "),
            Some("abc-123".to_string())
        );
        assert_eq!(IbmScanner::normalize_service_instance_id(" "), None);

        assert_eq!(
            IbmScanner::normalize_cos_endpoint("", "eu-de"),
            "https://s3.eu-de.cloud-object-storage.appdomain.cloud"
        );
        assert_eq!(
            IbmScanner::normalize_cos_endpoint(
                "s3.us-south.cloud-object-storage.appdomain.cloud/",
                "us-south"
            ),
            "https://s3.us-south.cloud-object-storage.appdomain.cloud"
        );
    }

    #[test]
    fn resource_guid_and_cos_detection_handle_mixed_shapes() {
        assert_eq!(
            IbmScanner::extract_resource_guid_from_id(
                "crn:v1:bluemix:public:cloud-object-storage:global:a/123:abc-def::"
            ),
            Some("123:abc-def::".to_string())
        );
        assert_eq!(
            IbmScanner::extract_resource_guid_from_id("/v2/resource_instances/abcd-efgh"),
            Some("abcd-efgh".to_string())
        );
        assert_eq!(IbmScanner::extract_resource_guid_from_id("   "), None);

        assert!(IbmScanner::is_cos_resource_instance(&json!({
            "resource_id": "cloud-object-storage",
            "name": "prod-cos"
        })));
        assert!(!IbmScanner::is_cos_resource_instance(&json!({
            "resource_id": "kms",
            "name": "key-protect"
        })));
    }

    #[test]
    fn cos_xml_helpers_cover_non_empty_and_empty_buckets() {
        let non_empty = r#"<ListBucketResult><Contents><Key>a</Key></Contents></ListBucketResult>"#;
        assert_eq!(IbmScanner::cos_is_empty_bucket(non_empty), Some(false));

        let empty = r#"<ListBucketResult><KeyCount>0</KeyCount></ListBucketResult>"#;
        assert_eq!(IbmScanner::cos_is_empty_bucket(empty), Some(true));

        assert_eq!(IbmScanner::extract_xml_u64(empty, "KeyCount"), Some(0));
        assert_eq!(IbmScanner::extract_xml_u64("<x></x>", "KeyCount"), None);
    }
}
