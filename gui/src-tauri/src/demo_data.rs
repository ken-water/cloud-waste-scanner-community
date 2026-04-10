use crate::db::Stats;
use chrono::{Duration, Utc};
use cloud_waste_scanner_core::{ResourceMetric, WastedResource};

pub fn generate_demo_metrics() -> Vec<ResourceMetric> {
    vec![
        ResourceMetric {
            id: "acct-aws-prod-main".to_string(),
            provider: "AWS".to_string(),
            region: "global".to_string(),
            resource_type: "Connected Account".to_string(),
            name: Some("aws-prod-main".to_string()),
            status: "configured".to_string(),
            cpu_utilization: None,
            network_in_mb: None,
            connections: None,
        },
        ResourceMetric {
            id: "acct-azure-finance".to_string(),
            provider: "Azure".to_string(),
            region: "global".to_string(),
            resource_type: "Connected Account".to_string(),
            name: Some("azure-finance".to_string()),
            status: "configured".to_string(),
            cpu_utilization: None,
            network_in_mb: None,
            connections: None,
        },
        ResourceMetric {
            id: "acct-gcp-analytics".to_string(),
            provider: "GCP".to_string(),
            region: "global".to_string(),
            resource_type: "Connected Account".to_string(),
            name: Some("gcp-analytics".to_string()),
            status: "configured".to_string(),
            cpu_utilization: None,
            network_in_mb: None,
            connections: None,
        },
        ResourceMetric {
            id: "i-0987654321fedcba".to_string(),
            provider: "AWS".to_string(),
            region: "eu-central-1".to_string(),
            resource_type: "EC2".to_string(),
            name: Some("prod-web-01".to_string()),
            status: "running".to_string(),
            cpu_utilization: Some(12.5),
            network_in_mb: Some(450.0),
            connections: Some(80),
        },
        ResourceMetric {
            id: "i-0123456789abcdef0".to_string(),
            provider: "AWS".to_string(),
            region: "us-west-2".to_string(),
            resource_type: "EC2".to_string(),
            name: Some("legacy-worker".to_string()),
            status: "running".to_string(),
            cpu_utilization: Some(0.1),
            network_in_mb: Some(0.0),
            connections: Some(0),
        },
        ResourceMetric {
            id: "i-0aaaabbbbccccdddd".to_string(),
            provider: "AWS".to_string(),
            region: "ap-southeast-1".to_string(),
            resource_type: "EC2".to_string(),
            name: Some("batch-converter-02".to_string()),
            status: "running".to_string(),
            cpu_utilization: Some(1.4),
            network_in_mb: Some(12.0),
            connections: Some(2),
        },
        ResourceMetric {
            id: "db-prod-replica-02".to_string(),
            provider: "AWS".to_string(),
            region: "us-east-1".to_string(),
            resource_type: "RDS".to_string(),
            name: Some("analytics-db".to_string()),
            status: "available".to_string(),
            cpu_utilization: Some(28.0),
            network_in_mb: Some(1500.0),
            connections: Some(5),
        },
        ResourceMetric {
            id: "nat-gateway-0a1b2c3d".to_string(),
            provider: "AWS".to_string(),
            region: "ap-southeast-1".to_string(),
            resource_type: "NAT Gateway".to_string(),
            name: Some("egress-nat-legacy".to_string()),
            status: "running".to_string(),
            cpu_utilization: Some(0.3),
            network_in_mb: Some(2.5),
            connections: Some(1),
        },
        ResourceMetric {
            id: "vm-sql-replica-04".to_string(),
            provider: "Azure".to_string(),
            region: "West Europe".to_string(),
            resource_type: "VM".to_string(),
            name: Some("sql-secondary".to_string()),
            status: "running".to_string(),
            cpu_utilization: Some(4.2),
            network_in_mb: Some(120.0),
            connections: Some(12),
        },
        ResourceMetric {
            id: "vm-finance-batch-03".to_string(),
            provider: "Azure".to_string(),
            region: "East US".to_string(),
            resource_type: "VM".to_string(),
            name: Some("finance-batch".to_string()),
            status: "running".to_string(),
            cpu_utilization: Some(86.2),
            network_in_mb: Some(860.0),
            connections: Some(190),
        },
        ResourceMetric {
            id: "vm-dev-test-box".to_string(),
            provider: "GCP".to_string(),
            region: "asia-northeast1".to_string(),
            resource_type: "VM".to_string(),
            name: Some("dev-sandbox".to_string()),
            status: "running".to_string(),
            cpu_utilization: Some(0.0),
            network_in_mb: Some(0.0),
            connections: Some(0),
        },
        ResourceMetric {
            id: "gke-nodepool-legacy".to_string(),
            provider: "GCP".to_string(),
            region: "us-central1".to_string(),
            resource_type: "GKE Node".to_string(),
            name: Some("legacy-nodepool".to_string()),
            status: "running".to_string(),
            cpu_utilization: Some(78.4),
            network_in_mb: Some(430.0),
            connections: Some(61),
        },
        ResourceMetric {
            id: "linode-vol-554433".to_string(),
            provider: "Linode".to_string(),
            region: "eu-west".to_string(),
            resource_type: "Volume".to_string(),
            name: Some("archive-volume".to_string()),
            status: "active".to_string(),
            cpu_utilization: Some(0.4),
            network_in_mb: Some(1.1),
            connections: Some(0),
        },
    ]
}

pub fn generate_demo_stats() -> Stats {
    let resources = generate_demo_data();
    let total_savings: f64 = resources.iter().map(|r| r.estimated_monthly_cost).sum();
    let wasted_count = resources.len() as i64;

    // Simulate a history of cleanups over the last 30 days
    let mut history = Vec::new();
    let mut cumulative = 120.0;
    let now = Utc::now();

    for i in (0..30).rev() {
        let date = now - Duration::days(i);
        // Every few days, add a "cleanup event"
        if i % 5 == 0 {
            cumulative += 50.0 + (i as f64 * 2.0); // Randomish increase
        }
        history.push((date.timestamp(), cumulative));
    }

    Stats {
        total_savings: total_savings, // Potential (current)
        wasted_resource_count: wasted_count,
        cleanup_count: 15, // Pretend we already cleaned 15 items
        history,           // Past actual savings
    }
}

pub fn generate_demo_data() -> Vec<WastedResource> {
    vec![
        // --- AWS ---
        WastedResource {
            id: "vol-0abc123def456".to_string(),
            provider: "AWS".to_string(),
            region: "us-east-1".to_string(),
            resource_type: "EBS Volume".to_string(),
            details: "Available (Unattached) - 500GB gp3".to_string(),
            estimated_monthly_cost: 40.0,
            action_type: "DELETE".to_string(),
        },
        WastedResource {
            id: "i-0987654321fedcba".to_string(),
            provider: "AWS".to_string(),
            region: "eu-central-1".to_string(),
            resource_type: "Oversized EC2".to_string(),
            details: "Utilization < 12%. Suggest Downgrade c5.2xlarge -> c5.xlarge.".to_string(),
            estimated_monthly_cost: 170.0, // Savings
            action_type: "RIGHTSIZE".to_string(),
        },
        WastedResource {
            id: "bucket-logs-2022-archive".to_string(),
            provider: "AWS".to_string(),
            region: "us-west-2".to_string(),
            resource_type: "S3 Bucket".to_string(),
            details: "14TB Cold Data (>180d). Suggest Move Standard -> Glacier Deep Archive."
                .to_string(),
            estimated_monthly_cost: 315.0, // Savings ($0.023 vs $0.00099)
            action_type: "ARCHIVE".to_string(),
        },
        WastedResource {
            id: "nat-gateway-0a1b2c3d".to_string(),
            provider: "AWS".to_string(),
            region: "ap-southeast-1".to_string(),
            resource_type: "NAT Gateway".to_string(),
            details: "Idle Traffic (0 Bytes / 7 Days)".to_string(),
            estimated_monthly_cost: 32.85,
            action_type: "DELETE".to_string(),
        },
        // --- Azure ---
        WastedResource {
            id: "disk-unused-azure-1".to_string(),
            provider: "Azure".to_string(),
            region: "East US".to_string(),
            resource_type: "Managed Disk".to_string(),
            details: "Unattached Premium SSD 128GB".to_string(),
            estimated_monthly_cost: 19.2,
            action_type: "DELETE".to_string(),
        },
        WastedResource {
            id: "vm-sql-replica-04".to_string(),
            provider: "Azure".to_string(),
            region: "West Europe".to_string(),
            resource_type: "Virtual Machine".to_string(),
            details: "CPU < 5% avg. Suggest Resize D4s_v3 -> D2s_v3".to_string(),
            estimated_monthly_cost: 140.0,
            action_type: "RIGHTSIZE".to_string(),
        },
        // --- GCP ---
        WastedResource {
            id: "vm-dev-test-box".to_string(),
            provider: "GCP".to_string(),
            region: "asia-northeast1".to_string(),
            resource_type: "Idle VM".to_string(),
            details: "Idle Recommendation: Stop Instance (n1-standard-4)".to_string(),
            estimated_monthly_cost: 48.50,
            action_type: "DELETE".to_string(),
        },
        WastedResource {
            id: "ip-frontend-legacy".to_string(),
            provider: "GCP".to_string(),
            region: "us-central1".to_string(),
            resource_type: "External IP".to_string(),
            details: "Unused Static IP Address".to_string(),
            estimated_monthly_cost: 7.20,
            action_type: "DELETE".to_string(),
        },
        // --- Alibaba Cloud ---
        WastedResource {
            id: "d-bp1j...".to_string(),
            provider: "Alibaba".to_string(),
            region: "cn-hangzhou".to_string(),
            resource_type: "Cloud Disk".to_string(),
            details: "Unattached ESSD PL1 (200GB)".to_string(),
            estimated_monthly_cost: 25.0,
            action_type: "DELETE".to_string(),
        },
        WastedResource {
            id: "oss-backup-video-raw".to_string(),
            provider: "Alibaba".to_string(),
            region: "cn-shanghai".to_string(),
            resource_type: "OSS Bucket".to_string(),
            details: "50TB Inactive. Suggest Move Standard -> Archive Tier.".to_string(),
            estimated_monthly_cost: 800.0,
            action_type: "ARCHIVE".to_string(),
        },
        // --- DigitalOcean / Linode / Others ---
        WastedResource {
            id: "vol-554433".to_string(),
            provider: "Linode".to_string(),
            region: "eu-west".to_string(),
            resource_type: "Volume".to_string(),
            details: "Unattached Volume (100GB)".to_string(),
            estimated_monthly_cost: 10.0,
            action_type: "DELETE".to_string(),
        },
        WastedResource {
            id: "192.168.x.x".to_string(),
            provider: "DigitalOcean".to_string(),
            region: "nyc1".to_string(),
            resource_type: "Floating IP".to_string(),
            details: "Unassigned IP Address".to_string(),
            estimated_monthly_cost: 4.0,
            action_type: "DELETE".to_string(),
        },
        // --- Oracle ---
        WastedResource {
            id: "ocid1.instance.oc1...".to_string(),
            provider: "Oracle".to_string(),
            region: "us-ashburn-1".to_string(),
            resource_type: "Compute Instance".to_string(),
            details: "Stopped (Idle) - k8s-node-3".to_string(),
            estimated_monthly_cost: 0.0, // Oracle Free Tier? Let's say 0 but waste of quota
            action_type: "DELETE".to_string(),
        },
    ]
}
