use crate::models::WastedResource;
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

pub async fn check_auth(token: &str) -> bool {
    let client = Client::new();
    let res = client
        .get("https://api.digitalocean.com/v2/account")
        .bearer_auth(token)
        .send()
        .await;
    match res {
        Ok(r) => r.status().is_success(),
        Err(_) => false,
    }
}

pub struct DigitalOceanScanner {
    client: Client,
    token: String,
}

#[derive(Deserialize)]
struct DropletList {
    droplets: Vec<Droplet>,
}

#[derive(Deserialize)]
struct Droplet {
    id: u64,
    name: String,
    status: String,
    size_slug: String,
    region: Region,
}

#[derive(Deserialize)]
struct Region {
    slug: String,
}

#[derive(Deserialize)]
struct VolumeList {
    volumes: Vec<Volume>,
}

#[derive(Deserialize)]
struct Volume {
    id: String,
    #[allow(dead_code)]
    name: String,
    region: Region,
    droplet_ids: Vec<u64>,
    size_gigabytes: u64,
}

#[derive(Deserialize)]
struct SnapshotList {
    snapshots: Vec<Snapshot>,
}

#[derive(Deserialize)]
struct Snapshot {
    id: String,
    #[allow(dead_code)]
    name: String,
    regions: Vec<String>,
    min_disk_size: u64,
    created_at: String,
}

#[derive(Deserialize)]
struct FloatingIpList {
    floating_ips: Vec<FloatingIp>,
}

#[derive(Deserialize)]
struct FloatingIp {
    ip: String,
    droplet: Option<Value>,
    region: Region,
}

#[derive(Deserialize)]
struct CdnEndpointList {
    endpoints: Vec<CdnEndpoint>,
}

#[derive(Deserialize)]
struct CdnEndpoint {
    id: String,
    origin: String, // e.g. "space-name.nyc3.digitaloceanspaces.com"
}

#[derive(Deserialize)]
struct LoadBalancerList {
    load_balancers: Vec<LoadBalancer>,
}

#[derive(Deserialize)]
struct LoadBalancer {
    id: String,
    name: String,
    region: Region,
    droplet_ids: Vec<u64>,
}

impl DigitalOceanScanner {
    pub fn new(token: &str) -> Self {
        Self {
            client: Client::new(),
            token: token.to_string(),
        }
    }

    pub async fn scan_droplets(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.digitalocean.com/v2/droplets";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: DropletList = res.json().await?;
        let mut wastes = Vec::new();
        for d in list.droplets {
            if d.status == "off" {
                wastes.push(WastedResource {
                    id: d.id.to_string(),
                    provider: "DigitalOcean".to_string(),
                    region: d.region.slug,
                    resource_type: "Droplet".to_string(),
                    details: format!("Stopped Droplet: {} ({})", d.name, d.size_slug),
                    estimated_monthly_cost: 5.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.digitalocean.com/v2/volumes";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: VolumeList = res.json().await?;
        let mut wastes = Vec::new();
        for v in list.volumes {
            if v.droplet_ids.is_empty() {
                let cost = v.size_gigabytes as f64 * 0.10;
                wastes.push(WastedResource {
                    id: v.id,
                    provider: "DigitalOcean".to_string(),
                    region: v.region.slug,
                    resource_type: "Volume".to_string(),
                    details: format!("Unattached Volume ({} GB)", v.size_gigabytes),
                    estimated_monthly_cost: cost,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_ips(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.digitalocean.com/v2/floating_ips";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: FloatingIpList = res.json().await?;
        let mut wastes = Vec::new();
        for ip in list.floating_ips {
            if ip.droplet.is_none() {
                wastes.push(WastedResource {
                    id: ip.ip,
                    provider: "DigitalOcean".to_string(),
                    region: ip.region.slug,
                    resource_type: "Floating IP".to_string(),
                    details: "Unassigned Floating IP".to_string(),
                    estimated_monthly_cost: 4.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_spaces(&self) -> Result<Vec<WastedResource>> {
        // Indirect Scan via CDN Endpoints
        let url = "https://api.digitalocean.com/v2/cdn/endpoints";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;

        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: CdnEndpointList = res.json().await?;
        let mut wastes = Vec::new();

        for ep in list.endpoints {
            // Origin usually looks like: bucket-name.nyc3.digitaloceanspaces.com
            if ep.origin.contains("digitaloceanspaces.com") {
                wastes.push(WastedResource {
                    id: ep.id,
                    provider: "DigitalOcean".to_string(),
                    region: "global".to_string(),
                    resource_type: "Space (CDN)".to_string(),
                    details: format!("Active CDN Endpoint for Space: {}", ep.origin),
                    estimated_monthly_cost: 5.0, // Base cost for Spaces
                    action_type: "REVIEW".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.digitalocean.com/v2/snapshots?page=1&per_page=200";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: SnapshotList = res.json().await?;
        let mut wastes = Vec::new();

        for snapshot in list.snapshots {
            let created_at = chrono::DateTime::parse_from_rfc3339(&snapshot.created_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok();
            let is_old = created_at
                .map(|dt| dt < chrono::Utc::now() - chrono::Duration::days(30))
                .unwrap_or(false);

            if is_old {
                wastes.push(WastedResource {
                    id: snapshot.id,
                    provider: "DigitalOcean".to_string(),
                    region: snapshot
                        .regions
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "global".to_string()),
                    resource_type: "Snapshot".to_string(),
                    details: format!("Old Snapshot ({} GB)", snapshot.min_disk_size),
                    estimated_monthly_cost: snapshot.min_disk_size as f64 * 0.05,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_load_balancers(&self) -> Result<Vec<WastedResource>> {
        let url = "https://api.digitalocean.com/v2/load_balancers";
        let res = self.client.get(url).bearer_auth(&self.token).send().await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let list: LoadBalancerList = res.json().await?;
        let mut wastes = Vec::new();

        for lb in list.load_balancers {
            if lb.droplet_ids.is_empty() {
                wastes.push(WastedResource {
                    id: lb.id,
                    provider: "DigitalOcean".to_string(),
                    region: lb.region.slug,
                    resource_type: "Load Balancer".to_string(),
                    details: format!("No attached droplets: {}", lb.name),
                    estimated_monthly_cost: 12.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for DigitalOceanScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_droplets().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_volumes().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_ips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_load_balancers().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_snapshots().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_spaces().await {
            results.extend(r);
        }
        Ok(results)
    }
}
