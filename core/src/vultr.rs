use crate::models::WastedResource;
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

pub struct VultrScanner {
    client: Client,
    api_key: String,
}

#[derive(Deserialize)]
struct VultrInstanceList {
    instances: Vec<VultrInstance>,
}

#[derive(Deserialize)]
struct VultrInstance {
    id: String,
    label: String,
    status: String,
    region: String,
}

#[derive(Deserialize)]
struct VultrBlockList {
    blocks: Vec<VultrBlock>,
}

#[derive(Deserialize)]
struct VultrBlock {
    id: String,
    label: String,
    size_gb: u64,
    attached_to_instance: String,
}

#[derive(Deserialize)]
struct VultrReservedIpList {
    reserved_ips: Vec<VultrReservedIp>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct VultrReservedIp {
    id: String,
    ip_type: String,
    instance_id: String,
}

#[derive(Deserialize)]
struct VultrSnapshotList {
    snapshots: Vec<VultrSnapshot>,
}

#[derive(Deserialize)]
struct VultrSnapshot {
    id: String,
    description: String,
    size: u64,
    status: String,
}

#[derive(Deserialize)]
struct VultrObjectStorageList {
    object_storages: Vec<VultrObjectStorage>,
}

#[derive(Deserialize)]
struct VultrLoadBalancerList {
    load_balancers: Vec<VultrLoadBalancer>,
}

#[derive(Deserialize)]
struct VultrLoadBalancer {
    id: String,
    label: String,
    region: String,
    instances: Vec<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct VultrObjectStorage {
    id: String,
    label: String,
    s3_hostname: String,
}

impl VultrScanner {
    pub fn new(key: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: key.to_string(),
        }
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.vultr.com/v2/instances";
        let res = self
            .client
            .get(url)
            .bearer_auth(&self.api_key)
            .send()
            .await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }
        let list: VultrInstanceList = res.json().await?;
        let mut wastes = Vec::new();
        for i in list.instances {
            if i.status == "stopped" {
                wastes.push(WastedResource {
                    id: i.id,
                    provider: "Vultr".to_string(),
                    region: i.region,
                    resource_type: "VPS Instance".to_string(),
                    details: format!("Stopped: {}", i.label),
                    estimated_monthly_cost: 5.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_blocks(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.vultr.com/v2/blocks";
        let res = self
            .client
            .get(url)
            .bearer_auth(&self.api_key)
            .send()
            .await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }
        let list: VultrBlockList = res.json().await?;
        let mut wastes = Vec::new();
        for b in list.blocks {
            if b.attached_to_instance.is_empty() {
                wastes.push(WastedResource {
                    id: b.id,
                    provider: "Vultr".to_string(),
                    region: "Global".to_string(),
                    resource_type: "Block Storage".to_string(),
                    details: format!("Unattached: {} ({}GB)", b.label, b.size_gb),
                    estimated_monthly_cost: b.size_gb as f64 * 0.10,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.vultr.com/v2/snapshots";
        let res = self
            .client
            .get(url)
            .bearer_auth(&self.api_key)
            .send()
            .await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }
        let list: VultrSnapshotList = res.json().await?;
        let mut wastes = Vec::new();
        for s in list.snapshots {
            if s.status == "complete" {
                wastes.push(WastedResource {
                    id: s.id,
                    provider: "Vultr".to_string(),
                    region: "Global".to_string(),
                    resource_type: "Snapshot".to_string(),
                    details: format!(
                        "Old Snapshot: {} ({}GB)",
                        s.description,
                        s.size / 1024 / 1024 / 1024
                    ),
                    estimated_monthly_cost: (s.size as f64 / 1024.0 / 1024.0 / 1024.0) * 0.05,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_reserved_ips(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.vultr.com/v2/reserved-ips";
        let res = self
            .client
            .get(url)
            .bearer_auth(&self.api_key)
            .send()
            .await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }
        let list: VultrReservedIpList = res.json().await?;
        let mut wastes = Vec::new();
        for ip in list.reserved_ips {
            if ip.instance_id.is_empty() {
                wastes.push(WastedResource {
                    id: ip.id,
                    provider: "Vultr".to_string(),
                    region: "Global".to_string(),
                    resource_type: "Reserved IP".to_string(),
                    details: "Unassigned IP".to_string(),
                    estimated_monthly_cost: 3.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_object_storage(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.vultr.com/v2/object-storage";
        let res = self
            .client
            .get(url)
            .bearer_auth(&self.api_key)
            .send()
            .await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }
        let list: VultrObjectStorageList = res.json().await?;
        let mut wastes = Vec::new();
        for os in list.object_storages {
            wastes.push(WastedResource {
                id: os.id,
                provider: "Vultr".to_string(),
                region: "Global".to_string(),
                resource_type: "Object Storage".to_string(),
                details: format!("Subscription: {}", os.label),
                estimated_monthly_cost: 5.0,
                action_type: "REVIEW".to_string(),
            });
        }
        Ok(wastes)
    }

    pub async fn scan_load_balancers(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.vultr.com/v2/load-balancers";
        let res = self
            .client
            .get(url)
            .bearer_auth(&self.api_key)
            .send()
            .await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: VultrLoadBalancerList = res.json().await?;
        let mut wastes = Vec::new();

        for lb in list.load_balancers {
            if lb.instances.is_empty() {
                wastes.push(WastedResource {
                    id: lb.id,
                    provider: "Vultr".to_string(),
                    region: lb.region,
                    resource_type: "Load Balancer".to_string(),
                    details: format!("No attached instances: {}", lb.label),
                    estimated_monthly_cost: 10.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for VultrScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_instances().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_blocks().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_snapshots().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_reserved_ips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_load_balancers().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_object_storage().await {
            results.extend(r);
        }
        Ok(results)
    }
}
