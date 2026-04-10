use crate::models::WastedResource;
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize)]
pub struct GcpServiceAccountKey {
    pub project_id: String,
    pub private_key: String,
    pub client_email: String,
    pub token_uri: String,
}

#[derive(Serialize)]
struct GcpClaims {
    iss: String,
    scope: String,
    aud: String,
    exp: u64,
    iat: u64,
}

#[derive(Deserialize)]
struct GcpTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct GcpBucketList {
    items: Option<Vec<GcpBucket>>,
}

#[derive(Debug, Deserialize)]
struct GcpBucket {
    name: String,
    location: Option<String>,
    #[serde(rename = "storageClass")]
    storage_class: Option<String>,
    lifecycle: Option<GcpLifecycle>,
}

#[derive(Debug, Deserialize)]
struct GcpLifecycle {
    rule: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct GcpRecommendationList {
    recommendations: Option<Vec<GcpRecommendation>>,
}

#[derive(Deserialize)]
struct GcpRecommendation {
    name: Option<String>,
    description: String,
    #[serde(rename = "primaryImpact")]
    primary_impact: GcpImpact,
}

#[derive(Deserialize)]
struct GcpImpact {
    #[serde(rename = "costProjection")]
    cost_projection: Option<GcpCostProjection>,
}

#[derive(Deserialize)]
struct GcpCostProjection {
    cost: GcpCost,
}

#[derive(Deserialize)]
struct GcpCost {
    units: Option<String>,
}

pub struct GcpScanner {
    client: Client,
    creds: GcpServiceAccountKey,
}

impl GcpScanner {
    fn fallback_recommender_locations() -> Vec<String> {
        vec![
            "us-central1".to_string(),
            "us-east1".to_string(),
            "us-west1".to_string(),
            "europe-west1".to_string(),
            "asia-east1".to_string(),
        ]
    }

    fn extract_recommender_locations(json: &Value) -> Vec<String> {
        let mut locations = Vec::new();
        if let Some(items) = json.get("locations").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(full_name) = item.get("name").and_then(|v| v.as_str()) {
                    if let Some(short_name) = full_name.rsplit('/').next() {
                        let short_name = short_name.trim();
                        if short_name != "global" && short_name.split('-').count() == 2 {
                            locations.push(short_name.to_string());
                        }
                    }
                }
            }
        }
        locations.sort();
        locations.dedup();
        locations
    }

    fn extract_up_zones(json: &Value) -> Vec<String> {
        let mut zones = Vec::new();
        if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
            for zone in items {
                let status = zone.get("status").and_then(|v| v.as_str()).unwrap_or("");
                if status != "UP" {
                    continue;
                }
                if let Some(name) = zone.get("name").and_then(|v| v.as_str()) {
                    zones.push(name.to_string());
                }
            }
        }
        zones.sort();
        zones.dedup();
        zones
    }

    pub fn new(json_key: &str) -> Result<Self> {
        let creds: GcpServiceAccountKey = serde_json::from_str(json_key)?;
        Ok(Self {
            client: Client::new(),
            creds,
        })
    }

    async fn get_token(&self) -> Result<String> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let claims = GcpClaims {
            iss: self.creds.client_email.clone(),
            scope: "https://www.googleapis.com/auth/cloud-platform".to_string(),
            aud: self.creds.token_uri.clone(),
            exp: now + 3600,
            iat: now,
        };
        let encoding_key = EncodingKey::from_rsa_pem(self.creds.private_key.as_bytes())?;
        let jwt = encode(&Header::new(Algorithm::RS256), &claims, &encoding_key)?;
        let body = format!(
            "grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer&assertion={}",
            jwt
        );
        let res = self
            .client
            .post(&self.creds.token_uri)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await?;
        let json: GcpTokenResponse = res.json().await?;
        Ok(json.access_token)
    }

    pub async fn list_recommender_locations(&self) -> Result<Vec<String>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://recommender.googleapis.com/v1/projects/{}/locations",
            self.creds.project_id
        );

        let res = self.client.get(&url).bearer_auth(&token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: Value = res.json().await?;
        let mut locations = Self::extract_recommender_locations(&json);

        if locations.is_empty() {
            locations = Self::fallback_recommender_locations();
        }

        Ok(locations)
    }

    pub async fn list_compute_zones(&self) -> Result<Vec<String>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/zones",
            self.creds.project_id
        );
        let res = self.client.get(&url).bearer_auth(&token).send().await?;

        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: Value = res.json().await?;
        Ok(Self::extract_up_zones(&json))
    }

    pub async fn list_idle_vm_recommendation_regions(&self) -> Result<Vec<String>> {
        let mut regions = self.list_recommender_locations().await.unwrap_or_default();

        if regions.is_empty() {
            let zones = self.list_compute_zones().await.unwrap_or_default();
            let mut zone_regions: HashSet<String> = HashSet::new();
            for zone in zones {
                if let Some((prefix, _suffix)) = zone.rsplit_once('-') {
                    zone_regions.insert(prefix.to_string());
                }
            }
            regions.extend(zone_regions);
        }

        regions.sort();
        regions.dedup();
        Ok(regions)
    }

    pub async fn scan_disks(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/aggregated/disks",
            self.creds.project_id
        );
        let res = self.client.get(&url).bearer_auth(token).send().await?;
        let json: Value = res.json().await?;
        let mut wastes = Vec::new();
        if let Some(items) = json.get("items").and_then(|i| i.as_object()) {
            for (zone_key, zone_data) in items {
                if let Some(disks) = zone_data.get("disks").and_then(|d| d.as_array()) {
                    for d in disks {
                        if d.get("users")
                            .map_or(true, |u| u.as_array().map_or(true, |a| a.is_empty()))
                        {
                            let name = d["name"].as_str().unwrap_or("").to_string();
                            wastes.push(WastedResource {
                                id: name.clone(),
                                provider: "GCP".to_string(),
                                region: zone_key.replace("zones/", ""),
                                resource_type: "Persistent Disk".to_string(),
                                details: format!("Orphaned Disk: {}", name),
                                estimated_monthly_cost: 10.0,
                                action_type: "DELETE".to_string(),
                            });
                        }
                    }
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_addresses(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/aggregated/addresses",
            self.creds.project_id
        );
        let res = self.client.get(&url).bearer_auth(token).send().await?;
        let json: Value = res.json().await?;
        let mut wastes = Vec::new();
        if let Some(items) = json.get("items").and_then(|i| i.as_object()) {
            for (region_key, region_data) in items {
                if let Some(addrs) = region_data.get("addresses").and_then(|d| d.as_array()) {
                    for a in addrs {
                        if a.get("status")
                            .map_or(false, |s| s.as_str() == Some("RESERVED"))
                            && a.get("users")
                                .map_or(true, |u| u.as_array().map_or(true, |a| a.is_empty()))
                        {
                            wastes.push(WastedResource {
                                id: a["name"].as_str().unwrap_or("").into(),
                                provider: "GCP".to_string(),
                                region: region_key.replace("regions/", ""),
                                resource_type: "External IP".to_string(),
                                details: "Unused IP".into(),
                                estimated_monthly_cost: 2.5,
                                action_type: "DELETE".to_string(),
                            });
                        }
                    }
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_idle_vm_recommendations(&self, region: &str) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!("https://recommender.googleapis.com/v1/projects/{}/locations/{}/recommenders/google.compute.instance.IdleResourceRecommender/recommendations", self.creds.project_id, region);
        let res = self.client.get(&url).bearer_auth(token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }
        let list: GcpRecommendationList = res.json().await?;
        let mut wastes = Vec::new();
        for (idx, rec) in list
            .recommendations
            .unwrap_or_default()
            .into_iter()
            .enumerate()
        {
            let cost_str = rec
                .primary_impact
                .cost_projection
                .and_then(|p| p.cost.units)
                .unwrap_or_else(|| "0".into());
            let cost = cost_str.parse::<f64>().unwrap_or(0.0).abs();
            let recommendation_id = rec
                .name
                .as_deref()
                .and_then(|value| value.rsplit('/').next())
                .filter(|value| !value.is_empty())
                .map(|value| format!("gcp-rec-{}", value))
                .unwrap_or_else(|| format!("gcp-rec-{}-{}", region, idx + 1));

            wastes.push(WastedResource {
                id: recommendation_id,
                provider: "GCP".to_string(),
                region: region.into(),
                resource_type: "Compute Instance".into(),
                details: rec.description,
                estimated_monthly_cost: cost,
                action_type: "RIGHTSIZE".into(),
            });
        }
        Ok(wastes)
    }

    pub async fn scan_storage_buckets(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://storage.googleapis.com/storage/v1/b?project={}",
            self.creds.project_id
        );
        let resp = self.client.get(&url).bearer_auth(&token).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }
        let data: GcpBucketList = resp.json().await?;
        let mut wastes = Vec::new();
        for bucket in data.items.unwrap_or_default() {
            let object_probe_url = format!(
                "https://storage.googleapis.com/storage/v1/b/{}/o?maxResults=1",
                bucket.name
            );

            let object_probe = self
                .client
                .get(&object_probe_url)
                .bearer_auth(&token)
                .send()
                .await;

            if let Ok(probe_resp) = object_probe {
                if probe_resp.status().is_success() {
                    if let Ok(probe_json) = probe_resp.json::<Value>().await {
                        let empty_bucket = probe_json
                            .get("items")
                            .and_then(|v| v.as_array())
                            .map(|items| items.is_empty())
                            .unwrap_or(true);

                        if empty_bucket {
                            wastes.push(WastedResource {
                                id: bucket.name.clone(),
                                provider: "GCP".to_string(),
                                region: bucket.location.clone().unwrap_or_else(|| "global".into()),
                                resource_type: "GCS Bucket".to_string(),
                                details: "Empty bucket (0 objects). Review for deletion."
                                    .to_string(),
                                estimated_monthly_cost: 0.0,
                                action_type: "DELETE".to_string(),
                            });
                            continue;
                        }
                    }
                }
            }

            let class = bucket.storage_class.unwrap_or_else(|| "STANDARD".into());
            let has_lifecycle = bucket
                .lifecycle
                .and_then(|l| l.rule)
                .map_or(false, |r| !r.is_empty());
            if !has_lifecycle {
                wastes.push(WastedResource {
                    id: bucket.name.clone(), provider: "GCP".to_string(), region: bucket.location.unwrap_or_else(|| "global".into()),
                    resource_type: "GCS Bucket".to_string(),
                    details: format!(
                        "No lifecycle policy configured for {} storage class bucket. Suggest archiving cold objects.",
                        class
                    ),
                    estimated_monthly_cost: 5.0,
                    action_type: "ARCHIVE".into(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/global/snapshots",
            self.creds.project_id
        );
        let resp = self.client.get(&url).bearer_auth(token).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let json: Value = resp.json().await?;
        let mut wastes = Vec::new();
        let cutoff = Utc::now() - Duration::days(30);

        if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
            for snapshot in items {
                let name = snapshot
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let region = snapshot
                    .get("storageLocations")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("global")
                    .to_string();
                let created = snapshot
                    .get("creationTimestamp")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let is_old = DateTime::parse_from_rfc3339(created)
                    .map(|dt| dt.with_timezone(&Utc) < cutoff)
                    .unwrap_or(false);

                if is_old {
                    wastes.push(WastedResource {
                        id: name.clone(),
                        provider: "GCP".to_string(),
                        region,
                        resource_type: "Snapshot".to_string(),
                        details: format!("Old Snapshot: {}", name),
                        estimated_monthly_cost: 3.0,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }

        Ok(wastes)
    }

    pub async fn delete_disk(&self, _region: &str, _name: &str) -> Result<()> {
        Ok(())
    }
    pub async fn release_address(&self, _region: &str, _name: &str) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl CloudProvider for GcpScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_disks().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_addresses().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_snapshots().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_storage_buckets().await {
            results.extend(r);
        }
        if let Ok(regions) = self.list_idle_vm_recommendation_regions().await {
            for region in regions {
                if let Ok(r) = self.scan_idle_vm_recommendations(&region).await {
                    results.extend(r);
                }
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_recommender_locations_filters_global_and_zone_like_values() {
        let payload = json!({
            "locations": [
                {"name": "projects/demo/locations/global"},
                {"name": "projects/demo/locations/us-central1"},
                {"name": "projects/demo/locations/europe-west1"},
                {"name": "projects/demo/locations/us-central1-a"},
                {"name": "projects/demo/locations/us-central1"}
            ]
        });
        let locations = GcpScanner::extract_recommender_locations(&payload);
        assert_eq!(
            locations,
            vec!["europe-west1".to_string(), "us-central1".to_string()]
        );
    }

    #[test]
    fn extract_up_zones_returns_only_up_status_and_deduped_sorted() {
        let payload = json!({
            "items": [
                {"name": "us-central1-b", "status": "UP"},
                {"name": "us-central1-a", "status": "UP"},
                {"name": "us-central1-b", "status": "UP"},
                {"name": "us-west1-a", "status": "DOWN"}
            ]
        });
        let zones = GcpScanner::extract_up_zones(&payload);
        assert_eq!(
            zones,
            vec!["us-central1-a".to_string(), "us-central1-b".to_string()]
        );
    }
}
