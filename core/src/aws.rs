use anyhow::Result;
use aws_sdk_cloudwatch::Client as CwClient;
use aws_sdk_ec2::Client as Ec2Client;
use aws_sdk_elasticloadbalancingv2::Client as ElbClient;
use aws_sdk_rds::Client as RdsClient;
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::Client as S3Client;
use chrono::{Duration, Utc};
use std::collections::{HashMap, HashSet};

use crate::models::{Policy, ResourceMetric, ScanPolicy, WastedResource};
use crate::policy::evaluate;
use crate::traits::CloudProvider;
use async_trait::async_trait;

pub struct Scanner {
    ec2_client: Ec2Client,
    elb_client: ElbClient,
    cw_client: CwClient,
    rds_client: RdsClient,
    s3_client: S3Client,
    region: String,
    policy: ScanPolicy,
    policies: Vec<Policy>,
}

impl Scanner {
    pub fn new(
        ec2_client: Ec2Client,
        elb_client: ElbClient,
        cw_client: CwClient,
        rds_client: RdsClient,
        s3_client: S3Client,
        region: String,
        policy: Option<ScanPolicy>,
        policies: Option<Vec<Policy>>,
    ) -> Self {
        Self {
            ec2_client,
            elb_client,
            cw_client,
            rds_client,
            s3_client,
            region,
            policy: policy.unwrap_or_default(),
            policies: policies.unwrap_or_default(),
        }
    }

    fn estimate_instance_monthly_cost(instance_type: &str) -> f64 {
        match instance_type {
            t if t.starts_with("t") => 10.0,
            t if t.starts_with("m") => 70.0,
            t if t.starts_with("c") => 90.0,
            t if t.starts_with("r") => 120.0,
            _ => 40.0,
        }
    }

    fn estimate_oversized_current_monthly_cost(instance_type: &str) -> f64 {
        match instance_type {
            t if t.starts_with("m") => 100.0,
            t if t.starts_with("c") => 150.0,
            t if t.starts_with("r") => 200.0,
            _ => 50.0,
        }
    }

    fn ebs_price_per_gb(volume_type: &str) -> f64 {
        match volume_type {
            "gp3" => 0.08,
            "gp2" => 0.10,
            "io1" | "io2" => 0.125,
            "standard" => 0.05,
            _ => 0.08,
        }
    }

    fn is_k8s_pv_tag_key(tag_key: &str) -> bool {
        tag_key.starts_with("kubernetes.io/created-for/pv/")
            || tag_key.starts_with("kubernetes.io/created-for/pvc/")
            || tag_key.starts_with("kubernetes.io/cluster/")
    }

    fn is_k8s_identity_tag_key(tag_key: &str) -> bool {
        let key = tag_key.to_ascii_lowercase();
        key == "owner"
            || key == "team"
            || key == "cost-center"
            || key == "cost_center"
            || key == "service"
            || key == "application"
            || key == "app"
    }

    fn is_large_k8s_node_instance(instance_type: &str) -> bool {
        instance_type.starts_with("m")
            || instance_type.starts_with("c")
            || instance_type.starts_with("r")
            || instance_type.starts_with("x")
            || instance_type.starts_with("z")
    }

    // Helper to get max metric
    async fn get_max_metric(
        &self,
        namespace: &str,
        metric: &str,
        dim_name: &str,
        dim_val: &str,
        days: i64,
    ) -> Result<f64> {
        let start_time = Utc::now() - Duration::days(days);
        let end_time = Utc::now();

        let resp = self
            .cw_client
            .get_metric_statistics()
            .namespace(namespace)
            .metric_name(metric)
            .dimensions(
                aws_sdk_cloudwatch::types::Dimension::builder()
                    .name(dim_name)
                    .value(dim_val)
                    .build(),
            )
            .start_time(aws_smithy_types::DateTime::from_secs(
                start_time.timestamp(),
            ))
            .end_time(aws_smithy_types::DateTime::from_secs(end_time.timestamp()))
            .period(86400) // Daily
            .statistics(aws_sdk_cloudwatch::types::Statistic::Maximum)
            .send()
            .await?;

        let max_val = resp
            .datapoints
            .unwrap_or_default()
            .iter()
            .map(|dp| dp.maximum.unwrap_or(0.0))
            .fold(0.0, f64::max);

        Ok(max_val)
    }

    async fn get_sum_metric(
        &self,
        namespace: &str,
        metric: &str,
        dim_name: &str,
        dim_val: &str,
        days: i64,
    ) -> Result<f64> {
        let start_time = Utc::now() - Duration::days(days);
        let end_time = Utc::now();

        let resp = self
            .cw_client
            .get_metric_statistics()
            .namespace(namespace)
            .metric_name(metric)
            .dimensions(
                aws_sdk_cloudwatch::types::Dimension::builder()
                    .name(dim_name)
                    .value(dim_val)
                    .build(),
            )
            .start_time(aws_smithy_types::DateTime::from_secs(
                start_time.timestamp(),
            ))
            .end_time(aws_smithy_types::DateTime::from_secs(end_time.timestamp()))
            .period(86400)
            .statistics(aws_sdk_cloudwatch::types::Statistic::Sum)
            .send()
            .await?;

        let sum_val = resp
            .datapoints
            .unwrap_or_default()
            .iter()
            .map(|dp| dp.sum.unwrap_or(0.0))
            .sum();

        Ok(sum_val)
    }

    // --- S3 Scanning Logic ---
    pub async fn scan_s3_buckets(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let resp = self.s3_client.list_buckets().send().await?;

        for bucket in resp.buckets.unwrap_or_default() {
            let name = bucket.name.unwrap_or_default();
            if name.is_empty() {
                continue;
            }

            let object_probe = self
                .s3_client
                .list_objects_v2()
                .bucket(&name)
                .max_keys(1)
                .send()
                .await;

            let mut confirmed_non_empty = false;
            if let Ok(output) = object_probe {
                let key_count = output.key_count().unwrap_or(0);
                let has_contents = !output.contents().is_empty();

                if key_count == 0 && !has_contents {
                    wastes.push(WastedResource {
                        id: name.clone(),
                        provider: "AWS".to_string(),
                        region: self.region.clone(),
                        resource_type: "S3 Bucket".to_string(),
                        details: "Empty bucket (0 objects). Suggest deletion if no longer needed."
                            .to_string(),
                        estimated_monthly_cost: 0.0,
                        action_type: "DELETE".to_string(),
                    });
                    continue;
                }

                confirmed_non_empty = has_contents || key_count > 0;
            }

            // 1. Get Size via CloudWatch (fallback/estimation)
            let size_bytes = self
                .get_max_metric("AWS/S3", "BucketSizeBytes", "BucketName", &name, 3)
                .await
                .unwrap_or(0.0);

            let size_gb = size_bytes / 1024.0 / 1024.0 / 1024.0;

            // CASE 2: Non-empty bucket without Lifecycle -> ARCHIVE
            let lifecycle = self
                .s3_client
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

            let has_non_trivial_data = confirmed_non_empty || size_gb > 1.0;
            if missing_lifecycle && has_non_trivial_data {
                let savings = if size_gb > 0.0 { size_gb * 0.022 } else { 5.0 };

                wastes.push(WastedResource {
                    id: name.clone(),
                    provider: "AWS".to_string(),
                    region: self.region.clone(),
                    resource_type: "S3 Bucket".to_string(),
                    details: format!(
                        "No lifecycle policy configured for non-empty bucket ({:.1} GB estimated). Suggest moving cold data to cheaper tiers.",
                        size_gb
                    ),
                    estimated_monthly_cost: savings,
                    action_type: "ARCHIVE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_idle_instances(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let resp = self
            .ec2_client
            .describe_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("instance-state-name")
                    .values("running")
                    .build(),
            )
            .send()
            .await?;

        // Policy values (Legacy)
        let cpu_limit = self.policy.cpu_percent;
        let net_limit = self.policy.network_mb * 1024.0 * 1024.0;
        let days = self.policy.lookback_days;

        for res in resp.reservations.unwrap_or_default() {
            for ins in res.instances.unwrap_or_default() {
                let id = ins.instance_id.unwrap_or_default();
                let type_ = ins
                    .instance_type
                    .as_ref()
                    .map(|t| t.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                // 1. Check CPU
                let max_cpu = self
                    .get_max_metric("AWS/EC2", "CPUUtilization", "InstanceId", &id, days)
                    .await
                    .unwrap_or(0.0);

                // 2. Check NetworkIn (Total over period)
                let network_in = self
                    .get_max_metric("AWS/EC2", "NetworkIn", "InstanceId", &id, days)
                    .await
                    .unwrap_or(0.0);

                let metric = ResourceMetric {
                    id: id.clone(),
                    provider: "AWS".to_string(),
                    region: self.region.clone(),
                    resource_type: "EC2 Instance".to_string(),
                    name: None,
                    status: "running".to_string(),
                    cpu_utilization: Some(max_cpu),
                    network_in_mb: Some(network_in / 1_000_000.0),
                    connections: None,
                };

                let is_wasted = if !self.policies.is_empty() {
                    evaluate(&metric, &self.policies)
                } else {
                    max_cpu < cpu_limit && network_in < net_limit
                };

                if is_wasted {
                    let est_cost = Self::estimate_instance_monthly_cost(&type_);

                    let reason = if !self.policies.is_empty() {
                        "Violates Custom Policy"
                    } else {
                        "Idle (Low CPU/Network)"
                    };

                    wastes.push(WastedResource {
                        id: id.clone(),
                        provider: "AWS".to_string(),
                        region: self.region.clone(),
                        resource_type: "EC2 Instance".to_string(),
                        details: format!(
                            "{} (CPU: {:.1}%, NetIn: {:.1}MB)",
                            reason,
                            max_cpu,
                            network_in / 1_000_000.0
                        ),
                        estimated_monthly_cost: est_cost,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_oversized_instances(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let resp = self
            .ec2_client
            .describe_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("instance-state-name")
                    .values("running")
                    .build(),
            )
            .send()
            .await?;

        let days = self.policy.lookback_days;
        let cpu_floor = self.policy.cpu_percent.max(0.0);
        let cpu_ceiling = 20.0;

        // If idle threshold is set too high, there is no meaningful rightsizing band.
        if cpu_floor >= cpu_ceiling {
            return Ok(wastes);
        }

        for res in resp.reservations.unwrap_or_default() {
            for ins in res.instances.unwrap_or_default() {
                let id = ins.instance_id.unwrap_or_default();
                let type_ = ins
                    .instance_type
                    .as_ref()
                    .map(|t| t.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Skip burstable tiny instances (t2/t3/t4g nano/micro/small) as they are likely cheap enough
                if type_.contains("nano") || type_.contains("micro") || type_.contains("small") {
                    continue;
                }

                // Check CPU
                let max_cpu = self
                    .get_max_metric("AWS/EC2", "CPUUtilization", "InstanceId", &id, days)
                    .await
                    .unwrap_or(0.0);

                // If CPU is low but active (cpu_floor < CPU < cpu_ceiling), suggest resizing.
                // CPU <= cpu_floor is handled by idle checks (DELETE).
                if max_cpu > cpu_floor && max_cpu < cpu_ceiling {
                    let current_cost = Self::estimate_oversized_current_monthly_cost(&type_);

                    let savings = current_cost * 0.5; // Rough estimate: halving resources saves ~50%

                    wastes.push(WastedResource {
                        id: id.clone(),
                        provider: "AWS".to_string(),
                        region: self.region.clone(),
                        resource_type: "Oversized EC2".to_string(),
                        details: format!(
                            "Low Utilization (Max CPU: {:.1}%, idle threshold: {:.1}%). Suggest Downgrade.",
                            max_cpu, cpu_floor
                        ),
                        estimated_monthly_cost: savings,
                        action_type: "RIGHTSIZE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_underutilized_ebs(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let resp = self
            .ec2_client
            .describe_volumes()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("status")
                    .values("in-use")
                    .build(),
            )
            .send()
            .await?;

        let days = self.policy.lookback_days;

        if let Some(volumes) = resp.volumes {
            for vol in volumes {
                let id = vol.volume_id.unwrap_or_default();
                // Check Read/Write Ops
                let read_ops = self
                    .get_max_metric("AWS/EBS", "VolumeReadOps", "VolumeId", &id, days)
                    .await
                    .unwrap_or(0.0);
                let write_ops = self
                    .get_max_metric("AWS/EBS", "VolumeWriteOps", "VolumeId", &id, days)
                    .await
                    .unwrap_or(0.0);

                if read_ops < 100.0 && write_ops < 100.0 {
                    // Essentially idle
                    let size = vol.size.unwrap_or(0);
                    wastes.push(WastedResource {
                        id,
                        provider: "AWS".to_string(),
                        region: self.region.clone(),
                        resource_type: "Underutilized EBS".to_string(),
                        details: format!("Low IOPS (Attached but Idle, {}GB)", size),
                        estimated_monthly_cost: size as f64 * 0.08,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    pub async fn collect_ec2_metrics(&self) -> Result<Vec<ResourceMetric>> {
        let mut metrics = Vec::new();
        let resp = self
            .ec2_client
            .describe_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("instance-state-name")
                    .values("running")
                    .build(),
            )
            .send()
            .await?;

        for res in resp.reservations.unwrap_or_default() {
            for ins in res.instances.unwrap_or_default() {
                let id = ins.instance_id.unwrap_or_default();
                // Get Name tag
                let name = ins
                    .tags
                    .unwrap_or_default()
                    .iter()
                    .find(|t| t.key.as_deref() == Some("Name"))
                    .map(|t| t.value.as_deref().unwrap_or("").to_string());

                // Fetch Metrics using policy lookback
                let cpu = self
                    .get_max_metric(
                        "AWS/EC2",
                        "CPUUtilization",
                        "InstanceId",
                        &id,
                        self.policy.lookback_days,
                    )
                    .await
                    .unwrap_or(0.0);
                let net_in = self
                    .get_max_metric(
                        "AWS/EC2",
                        "NetworkIn",
                        "InstanceId",
                        &id,
                        self.policy.lookback_days,
                    )
                    .await
                    .unwrap_or(0.0);

                metrics.push(ResourceMetric {
                    id,
                    provider: "AWS".to_string(),
                    region: self.region.clone(),
                    resource_type: "EC2 Instance".to_string(),
                    name,
                    status: "running".to_string(),
                    cpu_utilization: Some(cpu),
                    network_in_mb: Some(net_in / 1_000_000.0),
                    connections: None,
                });
            }
        }
        Ok(metrics)
    }

    pub async fn scan_old_amis(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let cutoff = Utc::now() - Duration::days(180); // 6 months (Keep fixed for now, or add AMI policy later)

        let instances = self.ec2_client.describe_instances().send().await?;
        let mut used_amis = Vec::new();

        if let Some(reservations) = instances.reservations {
            for res in reservations {
                if let Some(instances) = res.instances {
                    for ins in instances {
                        if let Some(img_id) = ins.image_id {
                            used_amis.push(img_id);
                        }
                    }
                }
            }
        }

        let resp = self
            .ec2_client
            .describe_images()
            .owners("self")
            .send()
            .await?;

        for img in resp.images.unwrap_or_default() {
            let id = img.image_id.unwrap_or_default();
            let date_str = img.creation_date.unwrap_or_default();

            if let Ok(date) = chrono::DateTime::parse_from_rfc3339(&date_str) {
                if date.with_timezone(&Utc) < cutoff && !used_amis.contains(&id) {
                    wastes.push(WastedResource {
                        id,
                        provider: "AWS".to_string(),
                        region: self.region.clone(),
                        resource_type: "Old AMI".to_string(),
                        details: format!("Created: {}", date.format("%Y-%m-%d")),
                        estimated_monthly_cost: 5.0,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_idle_nat_gateways(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let resp = self
            .ec2_client
            .describe_nat_gateways()
            .filter(
                aws_sdk_ec2::types::Filter::builder()
                    .name("state")
                    .values("available")
                    .build(),
            )
            .send()
            .await?;

        let days = self.policy.lookback_days;

        for nat in resp.nat_gateways.unwrap_or_default() {
            let id = nat.nat_gateway_id.unwrap_or_default();
            let max_bytes = self
                .get_max_metric(
                    "AWS/NATGateway",
                    "BytesOutToDestination",
                    "NatGatewayId",
                    &id,
                    days,
                )
                .await
                .unwrap_or(0.0);

            // Threshold: < 10 MB in N days
            if max_bytes < 10_000_000.0 {
                wastes.push(WastedResource {
                    id: id.clone(),
                    provider: "AWS".to_string(),
                    region: self.region.clone(),
                    resource_type: "NAT Gateway".to_string(),
                    details: format!("Idle Traffic (Max {:.2} MB)", max_bytes / 1_000_000.0),
                    estimated_monthly_cost: 32.85,
                    action_type: "DELETE".to_string(),
                });
            }
        }
        Ok(wastes)
    }

    pub async fn scan_ebs_volumes(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();

        let resp = self
            .ec2_client
            .describe_volumes()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("status")
                    .values("available")
                    .build(),
            )
            .send()
            .await?;

        if let Some(volumes) = resp.volumes {
            for vol in volumes {
                let id = vol.volume_id.unwrap_or_default();
                let size = vol.size.unwrap_or(0);
                let vol_type = vol
                    .volume_type
                    .map(|t| t.as_str().to_string())
                    .unwrap_or("standard".to_string());

                let price_per_gb = Self::ebs_price_per_gb(&vol_type);

                let cost = size as f64 * price_per_gb;

                wastes.push(WastedResource {
                    id,
                    provider: "AWS".to_string(),
                    region: self.region.clone(),
                    resource_type: "EBS Volume".to_string(),
                    details: format!("{} GB ({})", size, vol_type),
                    estimated_monthly_cost: cost,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_elastic_ips(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let resp = self.ec2_client.describe_addresses().send().await?;

        if let Some(addresses) = resp.addresses {
            let mut instance_ids = Vec::new();
            for addr in &addresses {
                if let Some(iid) = &addr.instance_id {
                    instance_ids.push(iid.clone());
                }
            }

            let mut stopped_instances = Vec::new();
            if !instance_ids.is_empty() {
                let ins_resp = self
                    .ec2_client
                    .describe_instances()
                    .set_instance_ids(Some(instance_ids))
                    .send()
                    .await?;

                for r in ins_resp.reservations.unwrap_or_default() {
                    for i in r.instances.unwrap_or_default() {
                        if i.state
                            .map(|s| s.name.unwrap().as_str() == "stopped")
                            .unwrap_or(false)
                        {
                            stopped_instances.push(i.instance_id.unwrap());
                        }
                    }
                }
            }

            for addr in addresses {
                let is_unattached = addr.instance_id.is_none();
                let is_attached_to_stopped = addr
                    .instance_id
                    .as_ref()
                    .map(|id| stopped_instances.contains(id))
                    .unwrap_or(false);

                if is_unattached || is_attached_to_stopped {
                    let reason = if is_unattached {
                        "Unattached"
                    } else {
                        "Attached to Stopped Instance"
                    };

                    wastes.push(WastedResource {
                        id: addr.public_ip.unwrap_or_default(),
                        provider: "AWS".to_string(),
                        region: self.region.clone(),
                        resource_type: "Elastic IP".to_string(),
                        details: reason.to_string(),
                        estimated_monthly_cost: 3.6,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_load_balancers(&self) -> Result<Vec<WastedResource>> {
        let wastes = Vec::new();
        let resp = self.elb_client.describe_load_balancers().send().await?;
        if let Some(lbs) = resp.load_balancers {
            for _lb in lbs {
                // Logic simplified
            }
        }
        Ok(wastes)
    }

    pub async fn scan_snapshots(&self, days: i64) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let cutoff = Utc::now() - Duration::days(days);
        let cutoff_secs = cutoff.timestamp();

        let resp = self
            .ec2_client
            .describe_snapshots()
            .owner_ids("self")
            .send()
            .await?;

        if let Some(snapshots) = resp.snapshots {
            for snap in snapshots {
                if let Some(start_time) = snap.start_time {
                    if start_time.secs() < cutoff_secs {
                        let size = snap.volume_size.unwrap_or(0);
                        let cost = size as f64 * 0.05;

                        wastes.push(WastedResource {
                            id: snap.snapshot_id.unwrap_or_default(),
                            provider: "AWS".to_string(),
                            region: self.region.clone(),
                            resource_type: "EBS Snapshot".to_string(),
                            details: format!("Old Snapshot ({} GB)", size),
                            estimated_monthly_cost: cost,
                            action_type: "DELETE".to_string(),
                        });
                    }
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_idle_rds(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let resp = self.rds_client.describe_db_instances().send().await?;

        let days = self.policy.lookback_days;

        if let Some(dbs) = resp.db_instances {
            for db in dbs {
                let status = db.db_instance_status.unwrap_or_default();
                let id = db.db_instance_identifier.unwrap_or_default();

                if status == "available" {
                    let max_conns = self
                        .get_max_metric(
                            "AWS/RDS",
                            "DatabaseConnections",
                            "DBInstanceIdentifier",
                            &id,
                            days,
                        )
                        .await
                        .unwrap_or(0.0);

                    if max_conns == 0.0 {
                        let class = db.db_instance_class.unwrap_or_default();
                        let est_cost = if class.contains("micro") { 15.0 } else { 80.0 };

                        wastes.push(WastedResource {
                            id,
                            provider: "AWS".to_string(),
                            region: self.region.clone(),
                            resource_type: "RDS Instance".to_string(),
                            details: format!("Zero Connections ({} days), Class: {}", days, class),
                            estimated_monthly_cost: est_cost,
                            action_type: "DELETE".to_string(),
                        });
                    }
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_stopped_rds_instances(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let resp = self.rds_client.describe_db_instances().send().await?;

        if let Some(dbs) = resp.db_instances {
            for db in dbs {
                let status = db.db_instance_status.unwrap_or_default();
                if status == "stopped" {
                    let allocated_storage = db.allocated_storage.unwrap_or(20);
                    let cost = allocated_storage as f64 * 0.10;

                    wastes.push(WastedResource {
                        id: db.db_instance_identifier.unwrap_or_default(),
                        provider: "AWS".to_string(),
                        region: self.region.clone(),
                        resource_type: "RDS Instance".to_string(),
                        details: format!("Stopped Instance ({} GB)", allocated_storage),
                        estimated_monthly_cost: cost,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_cloudwatch_logs(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let mut log_groups = HashSet::new();
        let lookback_days = self.policy.lookback_days.max(7);

        for metric_name in ["IncomingBytes", "StoredBytes"] {
            let mut next_token: Option<String> = None;

            loop {
                let mut req = self
                    .cw_client
                    .list_metrics()
                    .namespace("AWS/Logs")
                    .metric_name(metric_name);

                if let Some(token) = next_token.as_deref() {
                    req = req.next_token(token);
                }

                let resp = req.send().await?;

                for metric in resp.metrics.unwrap_or_default() {
                    for dim in metric.dimensions.unwrap_or_default() {
                        if dim.name.as_deref() == Some("LogGroupName") {
                            if let Some(v) = dim.value {
                                log_groups.insert(v);
                            }
                        }
                    }
                }

                next_token = resp.next_token;
                if next_token.is_none() {
                    break;
                }
            }
        }

        for group in log_groups {
            let stored_bytes = self
                .get_max_metric(
                    "AWS/Logs",
                    "StoredBytes",
                    "LogGroupName",
                    &group,
                    lookback_days,
                )
                .await
                .unwrap_or(0.0);

            let incoming_bytes = self
                .get_sum_metric(
                    "AWS/Logs",
                    "IncomingBytes",
                    "LogGroupName",
                    &group,
                    lookback_days,
                )
                .await
                .unwrap_or(0.0);

            if stored_bytes <= 0.0 && incoming_bytes <= 0.0 {
                continue;
            }

            let stored_gb = stored_bytes / 1024.0 / 1024.0 / 1024.0;
            let stale = incoming_bytes < 1.0;
            let oversized = stored_gb >= 5.0;

            if stale || oversized {
                let details = if stale && oversized {
                    format!(
                        "No ingest in last {} days, stored {:.2} GB. Consider retention/purge.",
                        lookback_days, stored_gb
                    )
                } else if stale {
                    format!(
                        "No ingest in last {} days, stored {:.2} GB.",
                        lookback_days, stored_gb
                    )
                } else {
                    format!(
                        "Large log group {:.2} GB. Review retention policy.",
                        stored_gb
                    )
                };

                let est_monthly = (stored_gb * 0.03).max(0.1);

                wastes.push(WastedResource {
                    id: group,
                    provider: "AWS".to_string(),
                    region: self.region.clone(),
                    resource_type: "CloudWatch Log Group".to_string(),
                    details,
                    estimated_monthly_cost: est_monthly,
                    action_type: "ARCHIVE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_eks_idle_nodes(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let days = self.policy.lookback_days;
        let cpu_limit = self.policy.cpu_percent;

        let resp = self
            .ec2_client
            .describe_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("instance-state-name")
                    .values("running")
                    .build(),
            )
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag-key")
                    .values("eks:cluster-name")
                    .build(),
            )
            .send()
            .await?;

        for reservation in resp.reservations.unwrap_or_default() {
            for instance in reservation.instances.unwrap_or_default() {
                let instance_id = instance.instance_id.unwrap_or_default();
                if instance_id.is_empty() {
                    continue;
                }

                let mut cluster_name = "unknown".to_string();
                if let Some(tags) = instance.tags {
                    for tag in tags {
                        if tag.key.as_deref() == Some("eks:cluster-name") {
                            if let Some(value) = tag.value {
                                if !value.trim().is_empty() {
                                    cluster_name = value;
                                }
                            }
                            break;
                        }
                    }
                }

                let max_cpu = self
                    .get_max_metric(
                        "AWS/EC2",
                        "CPUUtilization",
                        "InstanceId",
                        &instance_id,
                        days,
                    )
                    .await
                    .unwrap_or(0.0);

                if max_cpu < cpu_limit {
                    let instance_type = instance
                        .instance_type
                        .as_ref()
                        .map(|v| v.as_str())
                        .unwrap_or("unknown");
                    let savings = Self::estimate_instance_monthly_cost(instance_type) * 0.35;

                    wastes.push(WastedResource {
                        id: instance_id,
                        provider: "AWS".to_string(),
                        region: self.region.clone(),
                        resource_type: "K8s Node (EKS)".to_string(),
                        details: format!(
                            "EKS cluster '{}' node with low utilization (Max CPU {:.1}% over {}d). Review node-group rightsize/drain plan.",
                            cluster_name, max_cpu, days
                        ),
                        estimated_monthly_cost: savings,
                        action_type: "RIGHTSIZE".to_string(),
                    });
                }
            }
        }

        Ok(wastes)
    }

    pub async fn scan_eks_nodegroup_floor_risk(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let days = self.policy.lookback_days;
        let cpu_limit = self.policy.cpu_percent;

        let resp = self
            .ec2_client
            .describe_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("instance-state-name")
                    .values("running")
                    .build(),
            )
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag-key")
                    .values("eks:cluster-name")
                    .build(),
            )
            .send()
            .await?;

        // Key: cluster/nodegroup -> (node_count, sum_cpu, estimated_monthly_sum)
        let mut grouped: HashMap<String, (usize, f64, f64)> = HashMap::new();

        for reservation in resp.reservations.unwrap_or_default() {
            for instance in reservation.instances.unwrap_or_default() {
                let instance_id = instance.instance_id.unwrap_or_default();
                if instance_id.is_empty() {
                    continue;
                }

                let instance_type = instance
                    .instance_type
                    .as_ref()
                    .map(|v| v.as_str())
                    .unwrap_or("unknown");
                let mut cluster_name = "unknown-cluster".to_string();
                let mut nodegroup_name = "unknown-nodegroup".to_string();
                if let Some(tags) = instance.tags {
                    for tag in tags {
                        if tag.key.as_deref() == Some("eks:cluster-name") {
                            if let Some(value) = tag.value.clone() {
                                if !value.trim().is_empty() {
                                    cluster_name = value;
                                }
                            }
                        }
                        if tag.key.as_deref() == Some("eks:nodegroup-name") {
                            if let Some(value) = tag.value {
                                if !value.trim().is_empty() {
                                    nodegroup_name = value;
                                }
                            }
                        }
                    }
                }

                let max_cpu = self
                    .get_max_metric(
                        "AWS/EC2",
                        "CPUUtilization",
                        "InstanceId",
                        &instance_id,
                        days,
                    )
                    .await
                    .unwrap_or(0.0);

                let key = format!("{}/{}", cluster_name, nodegroup_name);
                let entry = grouped.entry(key).or_insert((0, 0.0, 0.0));
                entry.0 += 1;
                entry.1 += max_cpu;
                entry.2 += Self::estimate_instance_monthly_cost(instance_type);
            }
        }

        for (group_key, (node_count, sum_cpu, est_monthly)) in grouped {
            if node_count < 3 {
                continue;
            }
            let avg_cpu = sum_cpu / node_count as f64;
            if avg_cpu < (cpu_limit * 2.0).max(5.0) {
                wastes.push(WastedResource {
                    id: group_key.clone(),
                    provider: "AWS".to_string(),
                    region: self.region.clone(),
                    resource_type: "K8s NodeGroup (EKS)".to_string(),
                    details: format!(
                        "Node group '{}' has {} running nodes with low average CPU ({:.1}% over {}d). Review baseline node-group floor.",
                        group_key, node_count, avg_cpu, days
                    ),
                    estimated_monthly_cost: est_monthly * 0.30,
                    action_type: "RIGHTSIZE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_eks_orphan_pv_volumes(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let resp = self
            .ec2_client
            .describe_volumes()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("status")
                    .values("available")
                    .build(),
            )
            .send()
            .await?;

        if let Some(volumes) = resp.volumes {
            for vol in volumes {
                let tags = vol.tags.unwrap_or_default();
                let has_k8s_pv_tag = tags.iter().any(|tag| {
                    tag.key
                        .as_deref()
                        .map(Self::is_k8s_pv_tag_key)
                        .unwrap_or(false)
                });
                if !has_k8s_pv_tag {
                    continue;
                }

                let id = vol.volume_id.unwrap_or_default();
                let size = vol.size.unwrap_or(0);
                let vol_type = vol
                    .volume_type
                    .map(|t| t.as_str().to_string())
                    .unwrap_or_else(|| "standard".to_string());

                let cost = size as f64 * Self::ebs_price_per_gb(&vol_type);
                wastes.push(WastedResource {
                    id,
                    provider: "AWS".to_string(),
                    region: self.region.clone(),
                    resource_type: "K8s Orphan PV (EKS)".to_string(),
                    details: format!(
                        "Unattached Kubernetes-tagged volume ({} GB, {}). Review stale PVC/PV cleanup.",
                        size, vol_type
                    ),
                    estimated_monthly_cost: cost,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_eks_orphan_enis(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let resp = self
            .ec2_client
            .describe_network_interfaces()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("status")
                    .values("available")
                    .build(),
            )
            .send()
            .await?;

        if let Some(enis) = resp.network_interfaces {
            for eni in enis {
                let id = eni.network_interface_id.unwrap_or_default();
                if id.is_empty() {
                    continue;
                }

                let description = eni.description.unwrap_or_default();
                let desc_lower = description.to_ascii_lowercase();
                let has_k8s_desc = desc_lower.contains("eks")
                    || desc_lower.contains("kubernetes")
                    || desc_lower.contains("aws-k8s");

                let tags = eni.tag_set.unwrap_or_default();
                let has_k8s_tag = tags.iter().any(|tag| {
                    tag.key
                        .as_deref()
                        .map(Self::is_k8s_pv_tag_key)
                        .unwrap_or(false)
                });

                if !has_k8s_desc && !has_k8s_tag {
                    continue;
                }

                wastes.push(WastedResource {
                    id,
                    provider: "AWS".to_string(),
                    region: self.region.clone(),
                    resource_type: "K8s Orphan ENI (EKS)".to_string(),
                    details: format!(
                        "Available network interface with Kubernetes/EKS footprint ('{}'). Review stale CNI attachment lifecycle.",
                        description
                    ),
                    estimated_monthly_cost: 3.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_eks_missing_owner_tags(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let resp = self
            .ec2_client
            .describe_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("instance-state-name")
                    .values("running")
                    .build(),
            )
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag-key")
                    .values("eks:cluster-name")
                    .build(),
            )
            .send()
            .await?;

        for reservation in resp.reservations.unwrap_or_default() {
            for instance in reservation.instances.unwrap_or_default() {
                let id = instance.instance_id.unwrap_or_default();
                if id.is_empty() {
                    continue;
                }
                let instance_type = instance
                    .instance_type
                    .as_ref()
                    .map(|v| v.as_str())
                    .unwrap_or("unknown");

                let tags = instance.tags.unwrap_or_default();
                let has_owner_tag = tags.iter().any(|tag| {
                    tag.key
                        .as_deref()
                        .map(Self::is_k8s_identity_tag_key)
                        .unwrap_or(false)
                });
                if has_owner_tag {
                    continue;
                }

                let cluster_name = tags
                    .iter()
                    .find(|t| t.key.as_deref() == Some("eks:cluster-name"))
                    .and_then(|t| t.value.as_deref())
                    .unwrap_or("unknown-cluster");

                wastes.push(WastedResource {
                    id,
                    provider: "AWS".to_string(),
                    region: self.region.clone(),
                    resource_type: "K8s Ownership Gap (EKS)".to_string(),
                    details: format!(
                        "EKS node in cluster '{}' has no owner/team/cost tag. Add ownership tags to enable namespace/team chargeback.",
                        cluster_name
                    ),
                    estimated_monthly_cost: Self::estimate_instance_monthly_cost(instance_type)
                        * 0.10,
                    action_type: "RIGHTSIZE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_eks_requests_limits_drift_proxy(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let days = self.policy.lookback_days;

        let resp = self
            .ec2_client
            .describe_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("instance-state-name")
                    .values("running")
                    .build(),
            )
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag-key")
                    .values("eks:cluster-name")
                    .build(),
            )
            .send()
            .await?;

        // key -> (nodes, sum_cpu, sum_monthly_cost, large_node_count)
        let mut grouped: HashMap<String, (usize, f64, f64, usize)> = HashMap::new();
        for reservation in resp.reservations.unwrap_or_default() {
            for instance in reservation.instances.unwrap_or_default() {
                let instance_id = instance.instance_id.unwrap_or_default();
                if instance_id.is_empty() {
                    continue;
                }
                let instance_type = instance
                    .instance_type
                    .as_ref()
                    .map(|v| v.as_str())
                    .unwrap_or("unknown");

                let mut cluster_name = "unknown-cluster".to_string();
                let mut nodegroup_name = "unknown-nodegroup".to_string();
                if let Some(tags) = instance.tags {
                    for tag in tags {
                        if tag.key.as_deref() == Some("eks:cluster-name") {
                            if let Some(value) = tag.value.clone() {
                                if !value.trim().is_empty() {
                                    cluster_name = value;
                                }
                            }
                        }
                        if tag.key.as_deref() == Some("eks:nodegroup-name") {
                            if let Some(value) = tag.value {
                                if !value.trim().is_empty() {
                                    nodegroup_name = value;
                                }
                            }
                        }
                    }
                }

                let max_cpu = self
                    .get_max_metric(
                        "AWS/EC2",
                        "CPUUtilization",
                        "InstanceId",
                        &instance_id,
                        days,
                    )
                    .await
                    .unwrap_or(0.0);
                let key = format!("{}/{}", cluster_name, nodegroup_name);
                let entry = grouped.entry(key).or_insert((0, 0.0, 0.0, 0));
                entry.0 += 1;
                entry.1 += max_cpu;
                entry.2 += Self::estimate_instance_monthly_cost(instance_type);
                if Self::is_large_k8s_node_instance(instance_type) {
                    entry.3 += 1;
                }
            }
        }

        for (group_key, (node_count, sum_cpu, est_monthly, large_nodes)) in grouped {
            if node_count < 2 || large_nodes == 0 {
                continue;
            }
            let avg_cpu = sum_cpu / node_count as f64;
            if avg_cpu <= 12.0 {
                wastes.push(WastedResource {
                    id: group_key.clone(),
                    provider: "AWS".to_string(),
                    region: self.region.clone(),
                    resource_type: "K8s Requests/Limits Drift Proxy (EKS)".to_string(),
                    details: format!(
                        "Node group '{}' runs {} node(s) ({} large class) with low avg CPU {:.1}% over {}d. Review pod requests/limits and binpack policy.",
                        group_key, node_count, large_nodes, avg_cpu, days
                    ),
                    estimated_monthly_cost: est_monthly * 0.20,
                    action_type: "RIGHTSIZE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn delete_volume(&self, volume_id: &str) -> Result<()> {
        self.ec2_client
            .delete_volume()
            .volume_id(volume_id)
            .send()
            .await?;
        Ok(())
    }

    pub async fn release_eip(&self, public_ip: &str) -> Result<()> {
        let resp = self
            .ec2_client
            .describe_addresses()
            .public_ips(public_ip)
            .send()
            .await?;
        if let Some(addrs) = resp.addresses {
            if let Some(addr) = addrs.first() {
                if let Some(alloc_id) = &addr.allocation_id {
                    self.ec2_client
                        .release_address()
                        .allocation_id(alloc_id)
                        .send()
                        .await?;
                }
            }
        }
        Ok(())
    }

    pub async fn delete_snapshot(&self, snapshot_id: &str) -> Result<()> {
        self.ec2_client
            .delete_snapshot()
            .snapshot_id(snapshot_id)
            .send()
            .await?;
        Ok(())
    }

    pub async fn delete_load_balancer(&self, arn: &str) -> Result<()> {
        self.elb_client
            .delete_load_balancer()
            .load_balancer_arn(arn)
            .send()
            .await?;
        Ok(())
    }

    pub async fn deregister_image(&self, ami_id: &str) -> Result<()> {
        self.ec2_client
            .deregister_image()
            .image_id(ami_id)
            .send()
            .await?;
        Ok(())
    }

    pub async fn delete_nat_gateway(&self, nat_id: &str) -> Result<()> {
        self.ec2_client
            .delete_nat_gateway()
            .nat_gateway_id(nat_id)
            .send()
            .await?;
        Ok(())
    }

    pub async fn terminate_rds_instance(&self, db_instance_id: &str) -> Result<()> {
        self.rds_client
            .delete_db_instance()
            .db_instance_identifier(db_instance_id)
            .skip_final_snapshot(true)
            .send()
            .await?;
        Ok(())
    }
}

#[async_trait]
impl CloudProvider for Scanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut all_results = Vec::new();
        if let Ok(mut r) = self.scan_ebs_volumes().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_elastic_ips().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_snapshots(self.policy.lookback_days).await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_load_balancers().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_stopped_rds_instances().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_idle_instances().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_oversized_instances().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_idle_rds().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_idle_nat_gateways().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_old_amis().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_underutilized_ebs().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_s3_buckets().await {
            all_results.append(&mut r);
        } // S3 Scanning
        if let Ok(mut r) = self.scan_eks_idle_nodes().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_eks_nodegroup_floor_risk().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_eks_orphan_pv_volumes().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_eks_orphan_enis().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_eks_missing_owner_tags().await {
            all_results.append(&mut r);
        }
        if let Ok(mut r) = self.scan_eks_requests_limits_drift_proxy().await {
            all_results.append(&mut r);
        }
        Ok(all_results)
    }
}

#[cfg(test)]
mod tests {
    use super::Scanner;

    #[test]
    fn instance_cost_helpers_map_common_families() {
        assert_eq!(Scanner::estimate_instance_monthly_cost("t3.micro"), 10.0);
        assert_eq!(Scanner::estimate_instance_monthly_cost("m6i.large"), 70.0);
        assert_eq!(Scanner::estimate_instance_monthly_cost("c7g.xlarge"), 90.0);
        assert_eq!(
            Scanner::estimate_instance_monthly_cost("r6a.2xlarge"),
            120.0
        );
        assert_eq!(Scanner::estimate_instance_monthly_cost("z1d.large"), 40.0);
    }

    #[test]
    fn oversized_cost_helper_maps_compute_families() {
        assert_eq!(
            Scanner::estimate_oversized_current_monthly_cost("m6i.large"),
            100.0
        );
        assert_eq!(
            Scanner::estimate_oversized_current_monthly_cost("c7g.xlarge"),
            150.0
        );
        assert_eq!(
            Scanner::estimate_oversized_current_monthly_cost("r6a.2xlarge"),
            200.0
        );
        assert_eq!(
            Scanner::estimate_oversized_current_monthly_cost("z1d.large"),
            50.0
        );
    }

    #[test]
    fn ebs_price_helper_handles_known_and_fallback_types() {
        assert_eq!(Scanner::ebs_price_per_gb("gp3"), 0.08);
        assert_eq!(Scanner::ebs_price_per_gb("gp2"), 0.10);
        assert_eq!(Scanner::ebs_price_per_gb("io1"), 0.125);
        assert_eq!(Scanner::ebs_price_per_gb("io2"), 0.125);
        assert_eq!(Scanner::ebs_price_per_gb("standard"), 0.05);
        assert_eq!(Scanner::ebs_price_per_gb("unknown"), 0.08);
    }

    #[test]
    fn k8s_pv_tag_key_helper_matches_expected_patterns() {
        assert!(Scanner::is_k8s_pv_tag_key(
            "kubernetes.io/created-for/pv/name"
        ));
        assert!(Scanner::is_k8s_pv_tag_key(
            "kubernetes.io/created-for/pvc/namespace"
        ));
        assert!(Scanner::is_k8s_pv_tag_key("kubernetes.io/cluster/demo"));
        assert!(!Scanner::is_k8s_pv_tag_key("Name"));
    }

    #[test]
    fn k8s_identity_tag_helper_matches_expected_patterns() {
        assert!(Scanner::is_k8s_identity_tag_key("owner"));
        assert!(Scanner::is_k8s_identity_tag_key("team"));
        assert!(Scanner::is_k8s_identity_tag_key("cost-center"));
        assert!(Scanner::is_k8s_identity_tag_key("application"));
        assert!(!Scanner::is_k8s_identity_tag_key("environment"));
    }

    #[test]
    fn large_k8s_instance_helper_matches_expected_patterns() {
        assert!(Scanner::is_large_k8s_node_instance("m6i.large"));
        assert!(Scanner::is_large_k8s_node_instance("c7g.xlarge"));
        assert!(Scanner::is_large_k8s_node_instance("r6a.2xlarge"));
        assert!(!Scanner::is_large_k8s_node_instance("t3.medium"));
    }
}
