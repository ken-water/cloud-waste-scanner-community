use crate::models::WastedResource;
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::Deserialize;

pub struct LinodeScanner {
    client: Client,
    token: String,
}

#[derive(Deserialize)]
struct LinodeListResponse<T> {
    data: Vec<T>,
}

#[derive(Deserialize)]
struct LinodeInstance {
    id: u64,
    label: String,
    status: String,
    region: String,
    specs: LinodeSpecs,
}

#[derive(Deserialize)]
struct LinodeSpecs {
    #[allow(dead_code)]
    disk: u64,
    memory: u64,
}

#[derive(Deserialize)]
struct LinodeStats {
    data: LinodeStatsData,
}

#[derive(Deserialize)]
struct LinodeStatsData {
    cpu: Vec<Vec<f64>>,
}

#[derive(Deserialize)]
struct LinodeVolume {
    id: u64,
    label: String,
    size: u64,
    region: String,
    linode_id: Option<u64>,
}

#[derive(Deserialize)]
struct LinodeIp {
    address: String,
    region: String,
    linode_id: Option<u64>,
    #[serde(rename = "type")]
    ip_type: String,
}

#[derive(Deserialize)]
struct LinodeImage {
    id: String,
    label: String,
    size: u64,
    created: String,
    is_public: bool,
}

#[derive(Deserialize)]
struct LinodeNodeBalancer {
    id: u64,
    label: String,
    region: String,
}

#[derive(Deserialize)]
struct LinodeNodeBalancerConfig {
    nodes_status: Option<LinodeNodesStatus>,
}

#[derive(Deserialize)]
struct LinodeNodesStatus {
    up: u64,
}

#[derive(Deserialize)]
struct LinodeBucket {
    label: String,
    region: String,
    #[allow(dead_code)]
    created: String,
    size: u64, // Bytes
}

impl LinodeScanner {
    pub fn new(token: &str) -> Self {
        Self {
            client: Client::new(),
            token: token.to_string(),
        }
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.linode.com/v4/linode/instances";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }
        let json: LinodeListResponse<LinodeInstance> = res.json().await?;
        let mut wastes = Vec::new();
        for i in json.data {
            if i.status == "offline" {
                let est_cost = (i.specs.memory as f64 / 1024.0) * 5.0;
                wastes.push(WastedResource {
                    id: i.id.to_string(),
                    provider: "Linode".to_string(),
                    region: i.region.clone(),
                    resource_type: "Linode Instance".to_string(),
                    details: format!("Stopped: {}", i.label),
                    estimated_monthly_cost: est_cost,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_buckets(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.linode.com/v4/object-storage/buckets";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: LinodeListResponse<LinodeBucket> = res.json().await?;
        let mut wastes = Vec::new();
        for b in json.data {
            if b.size == 0 {
                wastes.push(WastedResource {
                    id: b.label,
                    provider: "Linode".to_string(),
                    region: b.region,
                    resource_type: "Object Storage".to_string(),
                    details: "Empty Bucket".to_string(),
                    estimated_monthly_cost: 5.0, // Linode base price for Object Storage
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.linode.com/v4/volumes";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }
        let json: LinodeListResponse<LinodeVolume> = res.json().await?;
        let mut wastes = Vec::new();
        for v in json.data {
            if v.linode_id.is_none() {
                wastes.push(WastedResource {
                    id: v.id.to_string(),
                    provider: "Linode".to_string(),
                    region: v.region,
                    resource_type: "Volume".to_string(),
                    details: format!("Unattached: {} ({}GB)", v.label, v.size),
                    estimated_monthly_cost: v.size as f64 * 0.10,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_ips(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.linode.com/v4/networking/ips";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }
        let json: LinodeListResponse<LinodeIp> = res.json().await?;
        let mut wastes = Vec::new();
        for ip in json.data {
            if ip.linode_id.is_none() && ip.ip_type == "ipv4" {
                wastes.push(WastedResource {
                    id: ip.address.clone(),
                    provider: "Linode".to_string(),
                    region: ip.region,
                    resource_type: "Reserved IP".to_string(),
                    details: "Unassigned Public IP".to_string(),
                    estimated_monthly_cost: 2.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_nodebalancers(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.linode.com/v4/nodebalancers";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: LinodeListResponse<LinodeNodeBalancer> = res.json().await?;
        let mut wastes = Vec::new();

        for nb in json.data {
            let cfg_url = format!("https://api.linode.com/v4/nodebalancers/{}/configs", nb.id);
            let cfg_res = self
                .client
                .get(&cfg_url)
                .bearer_auth(&self.token)
                .send()
                .await;

            let mut active_backends = false;
            let mut has_configs = false;

            if let Ok(resp) = cfg_res {
                if resp.status().is_success() {
                    if let Ok(cfg_json) = resp
                        .json::<LinodeListResponse<LinodeNodeBalancerConfig>>()
                        .await
                    {
                        has_configs = !cfg_json.data.is_empty();
                        active_backends = cfg_json.data.iter().any(|config| {
                            config
                                .nodes_status
                                .as_ref()
                                .map(|s| s.up > 0)
                                .unwrap_or(false)
                        });
                    }
                }
            }

            if !has_configs || !active_backends {
                wastes.push(WastedResource {
                    id: nb.id.to_string(),
                    provider: "Linode".to_string(),
                    region: nb.region,
                    resource_type: "NodeBalancer".to_string(),
                    details: format!("No active backends: {}", nb.label),
                    estimated_monthly_cost: 20.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.linode.com/v4/images?is_public=false";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: LinodeListResponse<LinodeImage> = res.json().await?;
        let cutoff = Utc::now() - Duration::days(30);
        let mut wastes = Vec::new();

        for image in json.data {
            if image.is_public {
                continue;
            }

            if let Ok(created) = DateTime::parse_from_rfc3339(&image.created) {
                let created_utc = created.with_timezone(&Utc);
                if created_utc < cutoff {
                    let size_gb = image.size as f64 / 1024.0;
                    wastes.push(WastedResource {
                        id: image.id,
                        provider: "Linode".to_string(),
                        region: "global".to_string(),
                        resource_type: "Snapshot".to_string(),
                        details: format!("Old private image: {}", image.label),
                        estimated_monthly_cost: size_gb.max(1.0) * 0.10,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }

        Ok(wastes)
    }

    pub async fn scan_oversized_instances(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.linode.com/v4/linode/instances";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: LinodeListResponse<LinodeInstance> = res.json().await?;
        let mut wastes = Vec::new();

        for instance in json.data {
            if instance.status != "running" || instance.specs.memory < 4096 {
                continue;
            }

            let stats_url = format!(
                "https://api.linode.com/v4/linode/instances/{}/stats",
                instance.id
            );
            let stats_res = self
                .client
                .get(&stats_url)
                .bearer_auth(&self.token)
                .send()
                .await;

            let avg_cpu = if let Ok(resp) = stats_res {
                if resp.status().is_success() {
                    if let Ok(stats) = resp.json::<LinodeStats>().await {
                        let cpu_points: Vec<f64> = stats
                            .data
                            .cpu
                            .iter()
                            .filter_map(|point| point.get(1).copied())
                            .collect();

                        if cpu_points.is_empty() {
                            None
                        } else {
                            Some(cpu_points.iter().sum::<f64>() / cpu_points.len() as f64)
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(cpu) = avg_cpu {
                if cpu < 2.0 {
                    let memory_gb = instance.specs.memory as f64 / 1024.0;
                    wastes.push(WastedResource {
                        id: instance.id.to_string(),
                        provider: "Linode".to_string(),
                        region: instance.region,
                        resource_type: "Linode Instance".to_string(),
                        details: format!(
                            "Likely oversized: {} (avg CPU {:.2}%)",
                            instance.label, cpu
                        ),
                        estimated_monthly_cost: (memory_gb * 4.0).max(8.0),
                        action_type: "RIGHTSIZE".to_string(),
                    });
                }
            }
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for LinodeScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_instances().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_oversized_instances().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_volumes().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_ips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_nodebalancers().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_snapshots().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_buckets().await {
            results.extend(r);
        }
        Ok(results)
    }
}
