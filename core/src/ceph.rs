use anyhow::{anyhow, Result};
use async_trait::async_trait;
use aws_sdk_s3::config::{Builder as S3ConfigBuilder, Credentials, Region};
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::Client as S3Client;
use chrono::{Duration, Utc};

use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct CephScanner {
    access_key: String,
    secret_key: String,
    region: String,
    endpoint: String,
}

impl CephScanner {
    pub fn new(access_key: &str, secret_key: &str, region: &str, endpoint: &str) -> Self {
        let normalized_region = if region.trim().is_empty() {
            "us-east-1".to_string()
        } else {
            region.trim().to_string()
        };

        Self {
            access_key: access_key.trim().to_string(),
            secret_key: secret_key.trim().to_string(),
            endpoint: Self::normalize_endpoint(endpoint, &normalized_region),
            region: normalized_region,
        }
    }

    fn normalize_endpoint(raw: &str, _region: &str) -> String {
        let endpoint = raw.trim();
        if endpoint.is_empty() {
            return "http://localhost:7480".to_string();
        }

        let mut value = endpoint.to_string();
        if !value.starts_with("http://") && !value.starts_with("https://") {
            value = format!("https://{}", value);
        }

        value.trim_end_matches('/').to_string()
    }

    fn s3_client(&self) -> Result<S3Client> {
        if self.access_key.is_empty() || self.secret_key.is_empty() {
            return Err(anyhow!("Ceph RGW access key and secret key are required"));
        }

        let config = S3ConfigBuilder::new()
            .region(Region::new(self.region.clone()))
            .credentials_provider(Credentials::new(
                self.access_key.clone(),
                self.secret_key.clone(),
                None,
                None,
                "ceph-static-aksk",
            ))
            .endpoint_url(&self.endpoint)
            .force_path_style(true)
            .behavior_version_latest()
            .build();

        Ok(S3Client::from_conf(config))
    }

    pub async fn check_auth(&self) -> Result<()> {
        let client = self.s3_client()?;
        client
            .list_buckets()
            .send()
            .await
            .map_err(|e| anyhow!("Ceph RGW auth failed: {}", e))?;
        Ok(())
    }

    pub async fn scan_empty_buckets(&self) -> Result<Vec<WastedResource>> {
        let client = self.s3_client()?;
        let mut wastes = Vec::new();

        let buckets = client.list_buckets().send().await?;
        for bucket in buckets.buckets() {
            let name = match bucket.name() {
                Some(value) if !value.trim().is_empty() => value.to_string(),
                _ => continue,
            };

            let probe = client
                .list_objects_v2()
                .bucket(&name)
                .max_keys(1)
                .send()
                .await;

            let is_empty = match probe {
                Ok(output) => {
                    let key_count = output.key_count().unwrap_or(0);
                    key_count == 0 && output.contents().is_empty()
                }
                Err(_) => false,
            };

            if is_empty {
                wastes.push(WastedResource {
                    id: name,
                    provider: "Ceph RGW".to_string(),
                    region: self.region.clone(),
                    resource_type: "S3 Bucket".to_string(),
                    details: "Empty bucket (0 objects).".to_string(),
                    estimated_monthly_cost: 1.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_missing_lifecycle(&self) -> Result<Vec<WastedResource>> {
        let client = self.s3_client()?;
        let mut wastes = Vec::new();

        let buckets = client.list_buckets().send().await?;
        for bucket in buckets.buckets() {
            let name = match bucket.name() {
                Some(value) if !value.trim().is_empty() => value.to_string(),
                _ => continue,
            };

            let lifecycle = client
                .get_bucket_lifecycle_configuration()
                .bucket(&name)
                .send()
                .await;

            let missing_lifecycle = match lifecycle {
                Ok(config) => config.rules().is_empty(),
                Err(err) => err
                    .as_service_error()
                    .and_then(|svc| svc.code())
                    .map(|code| code == "NoSuchLifecycleConfiguration")
                    .unwrap_or(false),
            };

            if missing_lifecycle {
                wastes.push(WastedResource {
                    id: name,
                    provider: "Ceph RGW".to_string(),
                    region: self.region.clone(),
                    resource_type: "S3 Bucket".to_string(),
                    details: "No lifecycle policy configured.".to_string(),
                    estimated_monthly_cost: 5.0,
                    action_type: "ARCHIVE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_orphan_multipart_uploads(&self) -> Result<Vec<WastedResource>> {
        let client = self.s3_client()?;
        let mut wastes = Vec::new();

        let buckets = client.list_buckets().send().await?;
        for bucket in buckets.buckets() {
            let name = match bucket.name() {
                Some(value) if !value.trim().is_empty() => value.to_string(),
                _ => continue,
            };

            let uploads = client
                .list_multipart_uploads()
                .bucket(&name)
                .max_uploads(100)
                .send()
                .await;

            let upload_count = match uploads {
                Ok(output) => output.uploads().len(),
                Err(_) => 0,
            };

            if upload_count > 0 {
                wastes.push(WastedResource {
                    id: name,
                    provider: "Ceph RGW".to_string(),
                    region: self.region.clone(),
                    resource_type: "S3 Multipart Upload".to_string(),
                    details: format!("{} incomplete multipart uploads found.", upload_count),
                    estimated_monthly_cost: upload_count as f64 * 0.5,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_old_versions(&self) -> Result<Vec<WastedResource>> {
        let client = self.s3_client()?;
        let mut wastes = Vec::new();
        let cutoff = (Utc::now() - Duration::days(30)).timestamp();

        let buckets = client.list_buckets().send().await?;
        for bucket in buckets.buckets() {
            let name = match bucket.name() {
                Some(value) if !value.trim().is_empty() => value.to_string(),
                _ => continue,
            };

            let versions = client
                .list_object_versions()
                .bucket(&name)
                .max_keys(200)
                .send()
                .await;

            let mut old_count = 0usize;
            if let Ok(output) = versions {
                for version in output.versions() {
                    if let Some(last_modified) = version.last_modified() {
                        if last_modified.secs() < cutoff {
                            old_count += 1;
                        }
                    }
                }
            }

            if old_count > 0 {
                wastes.push(WastedResource {
                    id: name,
                    provider: "Ceph RGW".to_string(),
                    region: self.region.clone(),
                    resource_type: "S3 Object Version".to_string(),
                    details: format!("{} object versions older than 30 days.", old_count),
                    estimated_monthly_cost: old_count as f64 * 0.05,
                    action_type: "ARCHIVE".to_string(),
                });
            }
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for CephScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();

        if let Ok(items) = self.scan_empty_buckets().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_missing_lifecycle().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_orphan_multipart_uploads().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_old_versions().await {
            results.extend(items);
        }

        Ok(results)
    }
}
