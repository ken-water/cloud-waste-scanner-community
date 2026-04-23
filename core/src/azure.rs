use crate::models::{ScanPolicy, WastedResource};
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

pub struct AzureScanner {
    client: Client,
    subscription_id: String,
    tenant_id: String,
    client_id: String,
    client_secret: String,
    policy: Option<ScanPolicy>,
}

#[derive(Deserialize)]
struct AzureTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct AzureDiskList {
    value: Vec<AzureDisk>,
}

#[derive(Deserialize)]
struct AzureDisk {
    name: String,
    location: String,
    properties: AzureDiskProperties,
}

#[derive(Deserialize)]
struct AzureDiskProperties {
    #[serde(rename = "diskState")]
    disk_state: String,
    #[serde(rename = "diskSizeGB")]
    disk_size_gb: i32,
}

#[derive(Deserialize)]
struct AzurePublicIpList {
    value: Vec<AzurePublicIp>,
}

#[derive(Deserialize)]
struct AzurePublicIp {
    name: String,
    location: String,
    properties: AzurePublicIpProperties,
}

#[derive(Deserialize)]
struct AzurePublicIpProperties {
    #[serde(rename = "ipConfiguration")]
    ip_configuration: Option<Value>,
}

#[derive(Deserialize)]
struct AzureAspList {
    value: Vec<AzureAsp>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AzureAsp {
    id: String,
    name: String,
    location: String,
    properties: AzureAspProperties,
    sku: AzureAspSku,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AzureAspProperties {
    #[serde(rename = "numberOfSites")]
    number_of_sites: i32,
    status: String, // Ready
}

#[derive(Deserialize)]
struct AzureAspSku {
    name: String,
    tier: String,
}

#[derive(Deserialize)]
struct AzureNicList {
    value: Vec<AzureNic>,
}

#[derive(Deserialize)]
struct AzureNic {
    id: String,
    name: String,
    location: String,
    properties: AzureNicProperties,
}

#[derive(Deserialize)]
struct AzureNicProperties {
    #[serde(rename = "virtualMachine")]
    virtual_machine: Option<Value>,
}

#[derive(Deserialize)]
struct AzureSqlServerList {
    value: Vec<AzureResource>,
}

#[derive(Deserialize)]
struct AzureSqlDbList {
    value: Vec<AzureResource>,
}

#[derive(Deserialize)]
struct AzureResource {
    id: String,
    name: String,
    location: String,
}

#[derive(Deserialize)]
struct AzureVmList {
    value: Vec<AzureVm>,
}

#[derive(Deserialize)]
struct AzureVm {
    id: String,
    name: String,
    location: String,
    properties: AzureVmProperties,
}

#[derive(Deserialize)]
struct AzureVmProperties {
    #[serde(rename = "instanceView")]
    instance_view: Option<AzureVmInstanceView>,
}

#[derive(Deserialize)]
struct AzureVmInstanceView {
    statuses: Vec<AzureVmStatus>,
}

#[derive(Deserialize)]
struct AzureVmStatus {
    code: String, // e.g. "PowerState/running"
}

// Azure Monitor Response
#[derive(Deserialize)]
struct AzureMetricResponse {
    value: Vec<AzureMetricValue>,
}

#[derive(Deserialize)]
struct AzureMetricValue {
    timeseries: Vec<AzureMetricTimeseries>,
}

#[derive(Deserialize)]
struct AzureMetricTimeseries {
    data: Vec<AzureMetricData>,
}

#[derive(Deserialize)]
struct AzureMetricData {
    maximum: Option<f64>,
}

#[derive(Deserialize)]
struct AzureStorageAccountList {
    value: Vec<AzureStorageAccount>,
}

#[derive(Deserialize)]
struct AzureStorageAccount {
    id: String,
    name: String,
    location: String,
    #[allow(dead_code)]
    sku: AzureSku,
}

#[derive(Deserialize)]
struct AzureSku {
    #[allow(dead_code)]
    name: String, // e.g. Standard_LRS
}

#[derive(Deserialize)]
struct AzureSnapshotList {
    value: Vec<AzureSnapshot>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AzureSnapshot {
    id: String,
    name: String,
    location: String,
    properties: AzureSnapshotProperties,
}

#[derive(Deserialize)]
struct AzureSnapshotProperties {
    #[serde(rename = "timeCreated")]
    time_created: String,
    #[serde(rename = "diskSizeGB")]
    disk_size_gb: i32,
}

impl AzureScanner {
    fn is_running_instance_view(instance_view: Option<&AzureVmInstanceView>) -> bool {
        instance_view
            .map(|v| v.statuses.iter().any(|s| s.code == "PowerState/running"))
            .unwrap_or(false)
    }

    fn parse_snapshot_time_utc(raw: &str) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|v| v.with_timezone(&Utc))
    }

    fn should_flag_snapshot(raw_created_at: &str, cutoff: DateTime<Utc>) -> bool {
        Self::parse_snapshot_time_utc(raw_created_at)
            .map(|created| created < cutoff)
            .unwrap_or(false)
    }

    pub fn new(
        sub: String,
        tenant: String,
        client: String,
        secret: String,
        policy: Option<ScanPolicy>,
    ) -> Self {
        Self {
            client: Client::new(),
            subscription_id: sub,
            tenant_id: tenant,
            client_id: client,
            client_secret: secret,
            policy,
        }
    }

    async fn get_token(&self) -> Result<String> {
        let url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant_id
        );
        let body = format!(
            "grant_type=client_credentials&client_id={}&client_secret={}&scope=https://management.azure.com/.default",
            self.client_id, self.client_secret
        );

        let res = self
            .client
            .post(&url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await?;

        let json: AzureTokenResponse = res.json().await?;
        Ok(json.access_token)
    }

    pub async fn scan_disks(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.Compute/disks?api-version=2023-04-02", 
            self.subscription_id
        );

        let res = self
            .client
            .get(&url)
            .bearer_auth(token.clone())
            .send()
            .await?;
        let list: AzureDiskList = res.json().await?;

        let mut wastes = Vec::new();
        for disk in list.value {
            if disk.properties.disk_state == "Unattached" {
                let cost = disk.properties.disk_size_gb as f64 * 0.15;
                wastes.push(WastedResource {
                    id: disk.name,
                    provider: "Azure".to_string(),
                    region: disk.location,
                    resource_type: "Managed Disk".to_string(),
                    details: format!("Unattached {}GB", disk.properties.disk_size_gb),
                    estimated_monthly_cost: cost,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_public_ips(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.Network/publicIPAddresses?api-version=2023-04-01", 
            self.subscription_id
        );

        let res = self.client.get(&url).bearer_auth(token).send().await?;
        let list: AzurePublicIpList = res.json().await?;

        let mut wastes = Vec::new();
        for ip in list.value {
            if ip.properties.ip_configuration.is_none() {
                wastes.push(WastedResource {
                    id: ip.name,
                    provider: "Azure".to_string(),
                    region: ip.location,
                    resource_type: "Public IP".to_string(),
                    details: "Unattached Public IP".to_string(),
                    estimated_monthly_cost: 3.65,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_app_service_plans(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.Web/serverfarms?api-version=2022-03-01", 
            self.subscription_id
        );

        let res = self.client.get(&url).bearer_auth(token).send().await?;
        let list: AzureAspList = res.json().await?;

        let mut wastes = Vec::new();
        for asp in list.value {
            // Check 1: Empty Plan (0 sites)
            if asp.properties.number_of_sites == 0 {
                // Cost estimation based on tier (Simplified)
                let cost = match asp.sku.tier.as_str() {
                    "PremiumV2" => 146.0, // P1v2
                    "Standard" => 73.0,   // S1
                    "Basic" => 54.75,     // B1
                    "Free" | "Shared" => 0.0,
                    _ => 73.0,
                };

                if cost > 0.0 {
                    wastes.push(WastedResource {
                        id: asp.name, // Use name for display, id for deletion usually needs full resource ID
                        provider: "Azure".to_string(),
                        region: asp.location,
                        resource_type: "App Service Plan".to_string(),
                        details: format!("Empty Plan (0 Apps), Tier: {}", asp.sku.name),
                        estimated_monthly_cost: cost,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_nics(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.Network/networkInterfaces?api-version=2023-04-01", 
            self.subscription_id
        );

        let res = self.client.get(&url).bearer_auth(token).send().await?;
        let list: AzureNicList = res.json().await?;

        let mut wastes = Vec::new();
        for nic in list.value {
            if nic.properties.virtual_machine.is_none() {
                wastes.push(WastedResource {
                    id: nic.id, // Full resource ID for easy deletion
                    provider: "Azure".to_string(),
                    region: nic.location,
                    resource_type: "Network Interface".to_string(),
                    details: format!("Orphaned NIC: {}", nic.name),
                    estimated_monthly_cost: 0.50, // NIC cost is minimal, but management cost high
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    async fn get_metric_max(&self, resource_id: &str, metric: &str) -> Result<f64> {
        let token = self.get_token().await?;
        let url = format!(
            "https://management.azure.com{}/providers/microsoft.insights/metrics?api-version=2018-01-01&metricnames={}&timespan=P7D&aggregation=Maximum",
            resource_id, metric
        );

        let res = self.client.get(&url).bearer_auth(token).send().await?;
        let json: AzureMetricResponse = res.json().await?;

        let max_val = json
            .value
            .first()
            .and_then(|v| v.timeseries.first())
            .and_then(|ts| {
                ts.data
                    .iter()
                    .filter_map(|d| d.maximum)
                    .fold(None, |a: Option<f64>, b| Some(a.unwrap_or(0.0).max(b)))
            })
            .unwrap_or(0.0);

        Ok(max_val)
    }

    pub async fn scan_idle_sql(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let mut wastes = Vec::new();

        // 1. List Servers
        let srv_url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.Sql/servers?api-version=2021-11-01",
            self.subscription_id
        );
        let srv_res = self
            .client
            .get(&srv_url)
            .bearer_auth(token.clone())
            .send()
            .await?;
        let servers: AzureSqlServerList = srv_res.json().await?;

        for srv in servers.value {
            // 2. List Databases per Server
            let db_url = format!(
                "https://management.azure.com{}/databases?api-version=2021-11-01",
                srv.id
            );
            let db_res = self
                .client
                .get(&db_url)
                .bearer_auth(token.clone())
                .send()
                .await?;
            let dbs: AzureSqlDbList = db_res.json().await?;

            for db in dbs.value {
                if db.name == "master" {
                    continue;
                }

                // Check DTU consumption first
                let max_dtu = self
                    .get_metric_max(&db.id, "dtu_consumption_percent")
                    .await?;
                // If DTU is 0 (maybe vCore model), check CPU
                let metric_val = if max_dtu > 0.0 {
                    max_dtu
                } else {
                    self.get_metric_max(&db.id, "cpu_percent").await?
                };

                let threshold = self.policy.as_ref().map(|p| p.cpu_percent).unwrap_or(2.0);
                if metric_val < threshold {
                    wastes.push(WastedResource {
                        id: db.name,
                        provider: "Azure".to_string(),
                        region: db.location,
                        resource_type: "SQL Database".to_string(),
                        details: format!("Idle DB (Max Load {:.1}%)", metric_val),
                        estimated_monthly_cost: 14.72, // Approx 5 DTU Basic
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    async fn get_vm_cpu_max(&self, vm_id: &str) -> Result<f64> {
        self.get_metric_max(vm_id, "Percentage CPU").await
    }

    pub async fn scan_idle_vms(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        // We need instanceView to check PowerState
        let url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.Compute/virtualMachines?api-version=2023-03-01&$expand=instanceView",
            self.subscription_id
        );

        let res = self.client.get(&url).bearer_auth(token).send().await?;
        let list: AzureVmList = res.json().await?;

        let mut wastes = Vec::new();
        for vm in list.value {
            // Check if Running
            let is_running = Self::is_running_instance_view(vm.properties.instance_view.as_ref());

            if is_running {
                let max_cpu = self.get_vm_cpu_max(&vm.id).await?;
                let threshold = self.policy.as_ref().map(|p| p.cpu_percent).unwrap_or(2.0);

                if max_cpu < threshold {
                    wastes.push(WastedResource {
                        id: vm.name,
                        provider: "Azure".to_string(),
                        region: vm.location,
                        resource_type: "Virtual Machine".to_string(),
                        details: format!("Idle CPU (Max {:.1}%)", max_cpu),
                        estimated_monthly_cost: 70.0, // Avg D2s_v3 cost, simplification
                        action_type: "DELETE".to_string(),
                    });
                } else if max_cpu < 20.0 {
                    wastes.push(WastedResource {
                        id: vm.name,
                        provider: "Azure".to_string(),
                        region: vm.location,
                        resource_type: "Virtual Machine".to_string(),
                        details: format!("Low CPU (Max {:.1}%). Suggest Downgrade.", max_cpu),
                        estimated_monthly_cost: 35.0, // Avg savings
                        action_type: "RIGHTSIZE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_storage_containers(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.Storage/storageAccounts?api-version=2023-01-01", 
            self.subscription_id
        );

        let res = self
            .client
            .get(&url)
            .bearer_auth(token.clone())
            .send()
            .await?;
        let list: AzureStorageAccountList = res.json().await?;

        let mut wastes = Vec::new();
        for acc in list.value {
            let mut estimated_bytes = 0.0;
            let metric_url = format!(
                "https://management.azure.com{}/providers/microsoft.insights/metrics?api-version=2018-01-01&metricnames=UsedCapacity&timespan=P7D&aggregation=Maximum",
                acc.id
            );

            if let Ok(metric_res) = self
                .client
                .get(&metric_url)
                .bearer_auth(token.clone())
                .send()
                .await
            {
                if metric_res.status().is_success() {
                    if let Ok(metric_json) = metric_res.json::<AzureMetricResponse>().await {
                        estimated_bytes = metric_json
                            .value
                            .first()
                            .and_then(|v| v.timeseries.first())
                            .and_then(|ts| {
                                ts.data
                                    .iter()
                                    .filter_map(|d| d.maximum)
                                    .fold(None, |a: Option<f64>, b| Some(a.unwrap_or(0.0).max(b)))
                            })
                            .unwrap_or(0.0);
                    }
                }
            }

            let size_gb = estimated_bytes / 1024.0 / 1024.0 / 1024.0;

            if size_gb == 0.0 {
                wastes.push(WastedResource {
                    id: acc.name.clone(),
                    provider: "Azure".to_string(),
                    region: acc.location.clone(),
                    resource_type: "Storage Account".to_string(),
                    details: "Storage account appears empty (0 GB used). Review for deletion."
                        .to_string(),
                    estimated_monthly_cost: 0.0,
                    action_type: "DELETE".to_string(),
                });
                continue;
            }

            // Check Lifecycle Policy
            let policy_url = format!(
                "https://management.azure.com{}/managementPolicies/default?api-version=2023-01-01",
                acc.id
            );

            let policy_res = self
                .client
                .get(&policy_url)
                .bearer_auth(token.clone())
                .send()
                .await?;

            // If 404 Not Found, it means no policy exists
            if policy_res.status() == 404 {
                wastes.push(WastedResource {
                    id: acc.name,
                    provider: "Azure".to_string(),
                    region: acc.location,
                    resource_type: "Storage Account".to_string(),
                    details: format!(
                        "No lifecycle policy configured for non-empty storage account ({:.1} GB estimated). Suggest auto-tiering to Cool/Archive.",
                        size_gb
                    ),
                    estimated_monthly_cost: if size_gb > 0.0 { size_gb * 0.010 } else { 5.0 },
                    action_type: "ARCHIVE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.Compute/snapshots?api-version=2023-04-02", 
            self.subscription_id
        );

        let res = self.client.get(&url).bearer_auth(token).send().await?;
        let list: AzureSnapshotList = res.json().await?;

        let mut wastes = Vec::new();
        let cutoff = Utc::now() - Duration::days(180);

        for snap in list.value {
            if Self::should_flag_snapshot(&snap.properties.time_created, cutoff) {
                let cost = snap.properties.disk_size_gb as f64 * 0.05; // Approx snapshot cost
                wastes.push(WastedResource {
                    id: snap.name,
                    provider: "Azure".to_string(),
                    region: snap.location,
                    resource_type: "Snapshot".to_string(),
                    details: format!(
                        "Old Snapshot (>180 days). Size: {}GB",
                        snap.properties.disk_size_gb
                    ),
                    estimated_monthly_cost: cost,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_aks_node_pools(&self) -> Result<Vec<WastedResource>> {
        let token = self.get_token().await?;
        let url = format!(
            "https://management.azure.com/subscriptions/{}/providers/Microsoft.ContainerService/managedClusters?api-version=2024-02-01",
            self.subscription_id
        );

        let resp = self.client.get(&url).bearer_auth(token).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let json: Value = resp.json().await?;
        let mut wastes = Vec::new();

        if let Some(clusters) = json.get("value").and_then(|v| v.as_array()) {
            for cluster in clusters {
                let cluster_name = cluster
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown-cluster");
                let region = cluster
                    .get("location")
                    .and_then(|v| v.as_str())
                    .unwrap_or("global");

                if let Some(agent_pools) = cluster
                    .get("properties")
                    .and_then(|v| v.get("agentPoolProfiles"))
                    .and_then(|v| v.as_array())
                {
                    for pool in agent_pools {
                        let pool_name = pool
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("nodepool");
                        let vm_size = pool
                            .get("vmSize")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let mode = pool
                            .get("mode")
                            .and_then(|v| v.as_str())
                            .unwrap_or("User");
                        let count = pool.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                        let min_count = pool.get("minCount").and_then(|v| v.as_i64()).unwrap_or(0);
                        let autoscaling = pool
                            .get("enableAutoScaling")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        let should_flag = (count > 2 && !autoscaling) || (autoscaling && min_count > 1);
                        if should_flag {
                            let savings = (count.max(1) as f64) * 20.0;
                            wastes.push(WastedResource {
                                id: format!("{}/{}", cluster_name, pool_name),
                                provider: "Azure".to_string(),
                                region: region.to_string(),
                                resource_type: "K8s Node Pool (AKS)".to_string(),
                                details: format!(
                                    "AKS pool '{}' (mode {}, vm {}) has count {}, autoscaling {}, min {}. Review baseline node count and scaling floor.",
                                    pool_name, mode, vm_size, count, autoscaling, min_count
                                ),
                                estimated_monthly_cost: savings,
                                action_type: "RIGHTSIZE".to_string(),
                            });
                        }
                    }
                }
            }
        }

        Ok(wastes)
    }

    pub async fn delete_resource(&self, resource_id: &str) -> Result<()> {
        // Azure Resource ID format: /subscriptions/{sub}/resourceGroups/{rg}/providers/.../{name}
        let token = self.get_token().await?;
        let url = format!(
            "https://management.azure.com/{}?api-version=2023-04-01",
            resource_id
        );

        let res = self.client.delete(&url).bearer_auth(token).send().await?;

        if res.status().is_success() {
            Ok(())
        } else {
            let text = res.text().await?;
            Err(anyhow::anyhow!("Azure Delete Failed: {}", text))
        }
    }
}

#[async_trait]
impl CloudProvider for AzureScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_disks().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_public_ips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_app_service_plans().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_nics().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_idle_sql().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_idle_vms().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_storage_containers().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_snapshots().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_aks_node_pools().await {
            results.extend(r);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_running_helper_detects_power_state_running() {
        let running = AzureVmInstanceView {
            statuses: vec![AzureVmStatus {
                code: "PowerState/running".to_string(),
            }],
        };
        let stopped = AzureVmInstanceView {
            statuses: vec![AzureVmStatus {
                code: "PowerState/stopped".to_string(),
            }],
        };
        assert!(AzureScanner::is_running_instance_view(Some(&running)));
        assert!(!AzureScanner::is_running_instance_view(Some(&stopped)));
        assert!(!AzureScanner::is_running_instance_view(None));
    }

    #[test]
    fn snapshot_age_helper_handles_valid_and_invalid_rfc3339() {
        let cutoff = Utc::now() - Duration::days(180);
        let old_time = (Utc::now() - Duration::days(181)).to_rfc3339();
        let fresh_time = (Utc::now() - Duration::days(30)).to_rfc3339();
        assert!(AzureScanner::should_flag_snapshot(&old_time, cutoff));
        assert!(!AzureScanner::should_flag_snapshot(&fresh_time, cutoff));
        assert!(!AzureScanner::should_flag_snapshot("not-a-time", cutoff));
    }
}
