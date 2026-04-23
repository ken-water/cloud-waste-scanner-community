use chrono::Utc;
use cloud_waste_scanner_core::{NotificationChannel, Policy, PolicyCondition, WastedResource};
use serde::Serialize;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Pool, Row, Sqlite,
};
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ScanRuleTemplate {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub description: String,
    pub default_params: String, // JSON
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AccountRuleConfig {
    pub account_id: String,
    pub rule_id: String,
    pub enabled: bool,
    pub custom_params: Option<String>, // JSON
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CleanupRecord {
    pub id: i64,
    pub resource_id: String,
    pub resource_type: String,
    pub saved_amount: f64,
    pub cleaned_at: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: i64,
    pub action: String,
    pub target: String,
    pub details: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CloudProfile {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub credentials: String,
    pub created_at: i64,
    pub timeout_seconds: Option<i64>,
    pub policy_custom: Option<String>,
    pub proxy_profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProxyProfile {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: i64,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DbPolicy {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub target_type: String,
    pub conditions: String, // JSON
    pub logic: String,
    pub is_active: bool,
    pub priority: i32,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ScanHistoryItem {
    pub id: i64,
    pub scanned_at: i64,
    pub total_waste: f64,
    pub resource_count: i64,
    pub status: String,
    pub results_json: String,
    pub scan_meta: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct HandledResource {
    pub resource_id: String,
    pub provider: String,
    pub handled_at: i64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MonitorMetric {
    pub id: String,
    pub provider: String,
    pub region: String,
    pub resource_type: String,
    pub name: Option<String>,
    pub status: String,
    pub cpu_utilization: Option<f64>,
    pub network_in_mb: Option<f64>,
    pub connections: Option<i64>,
    pub updated_at: i64,
    pub source: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MonitorSnapshot {
    pub collected_at: i64,
    pub total_resources: i64,
    pub idle_resources: i64,
    pub high_load_resources: i64,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub total_savings: f64,
    pub wasted_resource_count: i64,
    pub cleanup_count: i64,
    pub history: Vec<(i64, f64)>,
}

pub async fn init_db<P: AsRef<Path>>(path: P) -> Result<Pool<Sqlite>, String> {
    // ... existing init logic ...
    let path_str = path.as_ref().to_string_lossy();
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path_str))
        .map_err(|e| e.to_string())?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("PRAGMA journal_mode=WAL;")
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let queries = [
        // ... existing tables ...
        "CREATE TABLE IF NOT EXISTS resource_metrics (
            id TEXT PRIMARY KEY,
            provider TEXT,
            region TEXT,
            resource_type TEXT,
            name TEXT,
            status TEXT,
            cpu_utilization REAL,
            network_in_mb REAL,
            connections INTEGER,
            source TEXT,
            account_id TEXT,
            updated_at INTEGER
        )",
        "CREATE TABLE IF NOT EXISTS resource_metrics_history (
            id INTEGER PRIMARY KEY,
            resource_id TEXT NOT NULL,
            provider TEXT,
            region TEXT,
            resource_type TEXT,
            name TEXT,
            status TEXT,
            cpu_utilization REAL,
            network_in_mb REAL,
            connections INTEGER,
            source TEXT,
            account_id TEXT,
            collected_at INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS cleanup_history (
            id INTEGER PRIMARY KEY,
            resource_id TEXT NOT NULL,
            resource_type TEXT NOT NULL,
            saved_amount REAL NOT NULL,
            cleaned_at INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS license_usage (
            license_id TEXT PRIMARY KEY,
            used_at INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS scanned_resources (
            id TEXT PRIMARY KEY,
            provider TEXT NOT NULL,
            region TEXT NOT NULL,
            resource_type TEXT NOT NULL,
            details TEXT NOT NULL,
            estimated_monthly_cost REAL NOT NULL,
            scanned_at INTEGER NOT NULL,
            action_type TEXT DEFAULT 'DELETE'
        )",
        "CREATE TABLE IF NOT EXISTS audit_logs (
            id INTEGER PRIMARY KEY,
            action TEXT NOT NULL,
            target TEXT NOT NULL,
            details TEXT,
            created_at INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS cloud_profiles (
            id TEXT PRIMARY KEY,
            provider TEXT NOT NULL,
            name TEXT NOT NULL,
            credentials TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            proxy_profile_id TEXT
        )",
        "CREATE TABLE IF NOT EXISTS proxy_profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            protocol TEXT NOT NULL,
            host TEXT NOT NULL,
            port INTEGER NOT NULL,
            auth_username TEXT,
            auth_password TEXT,
            created_at INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT
        )",
        "CREATE TABLE IF NOT EXISTS policies (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            target_type TEXT NOT NULL,
            conditions TEXT NOT NULL,
            logic TEXT NOT NULL,
            is_active BOOLEAN NOT NULL DEFAULT 1,
            priority INTEGER DEFAULT 0
        )",
        "CREATE TABLE IF NOT EXISTS notification_channels (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            method TEXT NOT NULL,
            config TEXT NOT NULL,
            is_active BOOLEAN NOT NULL DEFAULT 1,
            proxy_profile_id TEXT,
            trigger_mode TEXT,
            min_savings REAL,
            min_findings INTEGER
        )",
        "CREATE TABLE IF NOT EXISTS scan_history (
            id INTEGER PRIMARY KEY,
            scanned_at INTEGER NOT NULL,
            total_waste REAL NOT NULL,
            resource_count INTEGER NOT NULL,
            status TEXT DEFAULT 'completed',
            results_json TEXT NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS handled_resources (
            resource_id TEXT PRIMARY KEY,
            provider TEXT NOT NULL,
            handled_at INTEGER NOT NULL,
            note TEXT
        )",
        // NEW TABLES for Data-Driven Rules
        "CREATE TABLE IF NOT EXISTS scan_rule_templates (
            id TEXT PRIMARY KEY,
            provider TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            default_params TEXT
        )",
        "CREATE TABLE IF NOT EXISTS account_scan_config (
            account_id TEXT NOT NULL,
            rule_id TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT 1,
            custom_params TEXT,
            PRIMARY KEY (account_id, rule_id)
        )",
    ];

    for q in queries {
        sqlx::query(q)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Auto-migrations
    let _ =
        sqlx::query("ALTER TABLE scanned_resources ADD COLUMN action_type TEXT DEFAULT 'DELETE'")
            .execute(&pool)
            .await;
    let _ = sqlx::query("ALTER TABLE cloud_profiles ADD COLUMN timeout_seconds INTEGER")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE cloud_profiles ADD COLUMN policy_custom TEXT")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE cloud_profiles ADD COLUMN proxy_profile_id TEXT")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE notification_channels ADD COLUMN proxy_profile_id TEXT")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE notification_channels ADD COLUMN trigger_mode TEXT")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE notification_channels ADD COLUMN min_savings REAL")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE notification_channels ADD COLUMN min_findings INTEGER")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE proxy_profiles ADD COLUMN auth_username TEXT")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE proxy_profiles ADD COLUMN auth_password TEXT")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE scan_history ADD COLUMN scan_meta TEXT")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE resource_metrics ADD COLUMN network_in_mb REAL")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE resource_metrics ADD COLUMN connections INTEGER")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE resource_metrics ADD COLUMN source TEXT")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE resource_metrics ADD COLUMN account_id TEXT")
        .execute(&pool)
        .await;

    // Seed Data
    seed_default_policies(&pool).await?;
    seed_scan_rules(&pool).await?;

    Ok(pool)
}

async fn seed_scan_rules(pool: &Pool<Sqlite>) -> Result<(), String> {
    // Master list of all capabilities hardcoded in the engine.
    // In the future, this list could come from a JSON file or remote config.
    let rules = vec![
        // AWS
        (
            "aws_ec2_idle",
            "aws",
            "Idle EC2 Instances",
            "Check for stopped or low-usage EC2 instances.",
            r#"{"cpu_threshold": 2.0}"#,
        ),
        (
            "aws_ebs_unattached",
            "aws",
            "Unattached EBS Volumes",
            "Identify volumes not attached to any instance.",
            "{}",
        ),
        (
            "aws_eip_unused",
            "aws",
            "Unused Elastic IPs",
            "Find allocated IPs not associated with a running instance.",
            "{}",
        ),
        (
            "aws_snapshot_old",
            "aws",
            "Old Snapshots",
            "List snapshots older than configured days.",
            r#"{"days": 30}"#,
        ),
        (
            "aws_elb_idle",
            "aws",
            "Idle Load Balancers",
            "Check for ELBs with no targets or low request count.",
            "{}",
        ),
        (
            "aws_rds_idle",
            "aws",
            "Idle/Stopped RDS",
            "Identify stopped or low-connection RDS instances.",
            "{}",
        ),
        (
            "aws_nat_idle",
            "aws",
            "Idle NAT Gateways",
            "Check for NAT Gateways with little to no traffic.",
            "{}",
        ),
        (
            "aws_ami_old",
            "aws",
            "Old AMIs",
            "Identify old machine images safe for cleanup.",
            "{}",
        ),
        (
            "aws_ebs_underutilized",
            "aws",
            "Underutilized EBS",
            "Find EBS volumes with low real utilization.",
            "{}",
        ),
        (
            "aws_ec2_oversized",
            "aws",
            "Oversized EC2 Instances",
            "Detect oversized EC2 instances for rightsizing recommendations.",
            "{}",
        ),
        (
            "aws_s3_no_lifecycle",
            "aws",
            "S3 Buckets without Lifecycle",
            "Detect S3 buckets missing lifecycle policies.",
            "{}",
        ),
        (
            "aws_log_retention",
            "aws",
            "CloudWatch Logs Cleanup",
            "Detect log groups with oversized retention cost.",
            "{}",
        ),
        (
            "aws_eks_node_idle",
            "aws",
            "EKS Node Baseline Review",
            "Detect EKS-tagged nodes with sustained low utilization for node-group rightsizing review.",
            r#"{"cpu_threshold": 2.0}"#,
        ),
        // Azure
        (
            "azure_vm_idle",
            "azure",
            "Idle Virtual Machines",
            "Check for stopped or low-usage VMs.",
            r#"{"cpu_threshold": 5.0}"#,
        ),
        (
            "azure_disk_unused",
            "azure",
            "Unattached Managed Disks",
            "Find disks with 'Unattached' status.",
            "{}",
        ),
        (
            "azure_ip_unused",
            "azure",
            "Unused Public IPs",
            "Identify Public IP addresses not associated with any resource.",
            "{}",
        ),
        (
            "azure_nic_orphan",
            "azure",
            "Orphaned NICs",
            "Find Network Interfaces not attached to any VM.",
            "{}",
        ),
        (
            "azure_sql_idle",
            "azure",
            "Idle SQL Databases",
            "Check for SQL Databases with low DTU usage.",
            "{}",
        ),
        (
            "azure_snapshot_old",
            "azure",
            "Old Snapshots",
            "Find stale snapshots for cleanup.",
            "{}",
        ),
        (
            "azure_plan_idle",
            "azure",
            "Idle App Service Plans",
            "Detect underutilized App Service Plans.",
            "{}",
        ),
        (
            "azure_blob_no_lifecycle",
            "azure",
            "Blob Containers without Lifecycle",
            "Detect Blob containers/storage accounts missing lifecycle policies.",
            "{}",
        ),
        (
            "azure_aks_nodepool_review",
            "azure",
            "AKS Node Pool Baseline Review",
            "Detect AKS node pools with high baseline or scaling floor for rightsizing review.",
            "{}",
        ),
        // GCP
        (
            "gcp_vm_idle",
            "gcp",
            "Idle VM Recommendations",
            "Retrieve Google's native idle VM recommendations.",
            "{}",
        ),
        (
            "gcp_disk_orphan",
            "gcp",
            "Orphaned Persistent Disks",
            "Find disks not attached to any instance.",
            "{}",
        ),
        (
            "gcp_ip_unused",
            "gcp",
            "Unused External IPs",
            "Identify static IPs not bound to a resource.",
            "{}",
        ),
        (
            "gcp_snapshot_old",
            "gcp",
            "Old Snapshots",
            "Find stale GCP snapshots for cleanup.",
            "{}",
        ),
        (
            "gcp_storage_no_lifecycle",
            "gcp",
            "Cloud Storage without Lifecycle",
            "Detect Cloud Storage buckets missing lifecycle policies.",
            "{}",
        ),
        (
            "gcp_gke_nodepool_review",
            "gcp",
            "GKE Node Pool Baseline Review",
            "Detect GKE node pools with fixed baseline capacity and disabled autoscaling.",
            "{}",
        ),
        // Alibaba
        (
            "ali_disk_orphan",
            "alibaba",
            "Orphaned ECS Disks",
            "Find available disks not attached to instances.",
            "{}",
        ),
        (
            "ali_eip_unused",
            "alibaba",
            "Unused EIPs",
            "Find EIPs in 'Available' status.",
            "{}",
        ),
        (
            "ali_oss_unused",
            "alibaba",
            "Empty/Idle OSS Buckets",
            "Check for empty buckets or missing lifecycle policies.",
            "{}",
        ),
        (
            "ali_slb_idle",
            "alibaba",
            "Idle SLB",
            "Check for inactive Load Balancers.",
            "{}",
        ),
        (
            "ali_snapshot_old",
            "alibaba",
            "Old Snapshots",
            "Find stale snapshots for cleanup.",
            "{}",
        ),
        (
            "ali_rds_idle",
            "alibaba",
            "Idle RDS",
            "Check for low-usage RDS instances.",
            "{}",
        ),
        // Tencent
        (
            "tc_cvm_idle",
            "tencent",
            "Idle CVM Instances",
            "Check for stopped instances.",
            "{}",
        ),
        (
            "tc_cbs_orphan",
            "tencent",
            "Unattached CBS Disks",
            "Find available cloud disks.",
            "{}",
        ),
        (
            "tc_eip_unused",
            "tencent",
            "Unused EIPs",
            "Find unbound EIPs.",
            "{}",
        ),
        (
            "tc_clb_idle",
            "tencent",
            "Idle CLB",
            "Find CLB load balancers with no useful traffic.",
            "{}",
        ),
        (
            "tc_cdb_idle",
            "tencent",
            "Idle CDB",
            "Find low-usage CDB instances.",
            "{}",
        ),
        (
            "tc_cos_unused",
            "tencent",
            "Idle COS Buckets",
            "Detect low-value object storage buckets.",
            "{}",
        ),
        // Baidu
        (
            "bd_bcc_idle",
            "baidu",
            "Idle BCC Instances",
            "Check for stopped or idle BCC instances.",
            "{}",
        ),
        (
            "bd_cds_orphan",
            "baidu",
            "Unattached CDS Disks",
            "Find unattached cloud disks.",
            "{}",
        ),
        (
            "bd_eip_unused",
            "baidu",
            "Unused EIPs",
            "Find unused EIP resources.",
            "{}",
        ),
        (
            "bd_blb_idle",
            "baidu",
            "Idle BLB",
            "Find BLB load balancers without active listeners/backends.",
            "{}",
        ),
        (
            "bd_bos_unused",
            "baidu",
            "Idle BOS Buckets",
            "Detect idle object storage buckets.",
            "{}",
        ),
        // Huawei
        (
            "hw_ecs_idle",
            "huawei",
            "Idle ECS Instances",
            "Check for idle or stopped ECS instances.",
            "{}",
        ),
        (
            "hw_evs_orphan",
            "huawei",
            "Unattached EVS Disks",
            "Find unattached EVS disks.",
            "{}",
        ),
        (
            "hw_eip_unused",
            "huawei",
            "Unused EIPs",
            "Find unbound EIPs.",
            "{}",
        ),
        (
            "hw_rds_idle",
            "huawei",
            "Idle RDS Instances",
            "Find Huawei RDS instances in stopped or inactive-like states.",
            "{}",
        ),
        (
            "hw_elb_idle",
            "huawei",
            "Idle Load Balancers",
            "Find Huawei ELB load balancers without listeners/backends.",
            "{}",
        ),
        (
            "hw_obs_unused",
            "huawei",
            "OBS Buckets without Lifecycle",
            "Detect OBS buckets missing lifecycle policies.",
            "{}",
        ),
        // Volcengine
        (
            "volc_ecs_idle",
            "volcengine",
            "Idle ECS Instances",
            "Check for idle ECS instances.",
            "{}",
        ),
        (
            "volc_ebs_orphan",
            "volcengine",
            "Unattached EBS Volumes",
            "Find unattached EBS volumes.",
            "{}",
        ),
        (
            "volc_eip_unused",
            "volcengine",
            "Unused EIPs",
            "Find unbound EIPs.",
            "{}",
        ),
        (
            "volc_clb_idle",
            "volcengine",
            "Idle CLB",
            "Find CLB balancers with no useful traffic.",
            "{}",
        ),
        (
            "volc_redis_idle",
            "volcengine",
            "Idle Redis Instances",
            "Find Redis instances with no active connections.",
            "{}",
        ),
        (
            "volc_tos_unused",
            "volcengine",
            "Idle TOS Buckets",
            "Detect idle object storage buckets.",
            "{}",
        ),
        // Others (Simplified for brevity, can expand)
        (
            "dig_droplet_idle",
            "digitalocean",
            "Idle Droplets",
            "Check for powered off Droplets.",
            "{}",
        ),
        (
            "dig_vol_unused",
            "digitalocean",
            "Unattached Volumes",
            "Find volumes not attached to Droplets.",
            "{}",
        ),
        (
            "dig_ip_unused",
            "digitalocean",
            "Unused Floating IPs",
            "Find floating IPs not mapped to active workloads.",
            "{}",
        ),
        (
            "dig_lb_idle",
            "digitalocean",
            "Idle Load Balancers",
            "Find load balancers without attached droplets.",
            "{}",
        ),
        (
            "dig_snap_old",
            "digitalocean",
            "Old Snapshots",
            "Find old snapshots for cleanup.",
            "{}",
        ),
        (
            "dig_spaces_unused",
            "digitalocean",
            "Idle Spaces",
            "Detect low-value Spaces object storage.",
            "{}",
        ),
        (
            "lin_linode_idle",
            "linode",
            "Idle Linodes",
            "Check for offline Linode instances.",
            "{}",
        ),
        (
            "lin_vol_unused",
            "linode",
            "Unattached Volumes",
            "Find unattached Linode block volumes.",
            "{}",
        ),
        (
            "lin_ip_unused",
            "linode",
            "Unused Reserved IPs",
            "Find reserved IPs not attached to Linodes.",
            "{}",
        ),
        (
            "lin_nodebal_unused",
            "linode",
            "Idle NodeBalancers",
            "Find NodeBalancers without active backends.",
            "{}",
        ),
        (
            "lin_snap_old",
            "linode",
            "Old Snapshots",
            "Find old private images/snapshots for cleanup.",
            "{}",
        ),
        (
            "lin_oversized",
            "linode",
            "Oversized Linodes",
            "Find running Linodes with low CPU and high memory profile.",
            "{}",
        ),
        (
            "lin_obj_unused",
            "linode",
            "Idle Object Storage",
            "Detect low-value object storage buckets.",
            "{}",
        ),
        (
            "akamai_instance_idle",
            "akamai",
            "Idle Instances",
            "Check for stopped Akamai Connected Cloud instances.",
            "{}",
        ),
        (
            "akamai_volume_orphan",
            "akamai",
            "Unattached Volumes",
            "Find unattached Akamai block volumes.",
            "{}",
        ),
        (
            "akamai_ip_unused",
            "akamai",
            "Unused Reserved IPs",
            "Find reserved Akamai IPs not attached to instances.",
            "{}",
        ),
        (
            "akamai_lb_idle",
            "akamai",
            "Idle NodeBalancers",
            "Find Akamai NodeBalancers without active backends.",
            "{}",
        ),
        (
            "akamai_snapshot_old",
            "akamai",
            "Old Snapshots",
            "Find old Akamai snapshots/images for cleanup.",
            "{}",
        ),
        (
            "akamai_instance_oversized",
            "akamai",
            "Oversized Instances",
            "Find running Akamai instances with low CPU and high memory profile.",
            "{}",
        ),
        (
            "akamai_obj_unused",
            "akamai",
            "Idle Object Storage",
            "Detect low-value Akamai object storage buckets.",
            "{}",
        ),
        (
            "vultr_vps_idle",
            "vultr",
            "Idle Instances",
            "Check for stopped VPS.",
            "{}",
        ),
        (
            "vultr_blk_unused",
            "vultr",
            "Unattached Block Storage",
            "Find unattached block storage.",
            "{}",
        ),
        (
            "vultr_snap_old",
            "vultr",
            "Old Snapshots",
            "Find stale snapshots for cleanup.",
            "{}",
        ),
        (
            "vultr_ip_unused",
            "vultr",
            "Unused Reserved IPs",
            "Find reserved IPs not attached to instances.",
            "{}",
        ),
        (
            "vultr_lb_idle",
            "vultr",
            "Idle Load Balancers",
            "Find load balancers without attached instances.",
            "{}",
        ),
        (
            "vultr_obj_unused",
            "vultr",
            "Idle Object Storage",
            "Detect low-value object storage.",
            "{}",
        ),
        (
            "oracle_compute_idle",
            "oracle",
            "Idle Compute",
            "Check for stopped OCI instances.",
            "{}",
        ),
        (
            "oracle_boot_orphan",
            "oracle",
            "Orphaned Boot Volumes",
            "Find unattached OCI boot volumes.",
            "{}",
        ),
        (
            "oracle_block_orphan",
            "oracle",
            "Unattached Block Volumes",
            "Find unattached OCI block volumes.",
            "{}",
        ),
        (
            "oracle_lb_idle",
            "oracle",
            "Idle Load Balancers",
            "Find OCI load balancers with no backend sets.",
            "{}",
        ),
        (
            "oracle_ip_unused",
            "oracle",
            "Unused Reserved IPs",
            "Find unassigned OCI reserved public IPs.",
            "{}",
        ),
        (
            "oracle_obj_unused",
            "oracle",
            "Idle Object Storage",
            "Detect low-value OCI object storage.",
            "{}",
        ),
        (
            "ibm_vpc_idle",
            "ibm",
            "Idle VPC Instances",
            "Check for stopped instances.",
            "{}",
        ),
        (
            "ibm_fip_unused",
            "ibm",
            "Unused Floating IPs",
            "Find floating IPs not attached to instances.",
            "{}",
        ),
        (
            "ibm_block_orphan",
            "ibm",
            "Orphaned Block Storage",
            "Find unattached IBM block storage volumes.",
            "{}",
        ),
        (
            "ibm_lb_idle",
            "ibm",
            "Idle Load Balancers",
            "Find IBM load balancers with no backend pools.",
            "{}",
        ),
        (
            "ibm_snap_old",
            "ibm",
            "Old Snapshots",
            "Find old IBM snapshots for cleanup.",
            "{}",
        ),
        (
            "ibm_cos_unused",
            "ibm",
            "Idle COS Buckets",
            "Detect low-value IBM COS buckets.",
            "{}",
        ),
        (
            "cloudflare_dns_exposed",
            "cloudflare",
            "Exposed DNS Records",
            "Check for A records pointing to private IPs.",
            "{}",
        ),
        (
            "cloudflare_r2_unused",
            "cloudflare",
            "R2 Buckets Review",
            "List R2 buckets for lifecycle and usage review.",
            "{}",
        ),
        (
            "cloudflare_tunnel_unused",
            "cloudflare",
            "Unused Tunnels",
            "Find inactive Cloudflare tunnels.",
            "{}",
        ),
        (
            "cloudflare_worker_unused",
            "cloudflare",
            "Unused Workers",
            "Find workers with no active usage.",
            "{}",
        ),
        (
            "cloudflare_pages_unused",
            "cloudflare",
            "Unused Pages Projects",
            "Find pages projects with low activity.",
            "{}",
        ),
        // Tianyi
        (
            "tianyi_host_idle",
            "tianyi",
            "Idle Cloud Hosts",
            "Find stopped or offline Tianyi cloud hosts.",
            "{}",
        ),
        (
            "tianyi_disk_orphan",
            "tianyi",
            "Unattached Hard Disks",
            "Find detached Tianyi cloud disks.",
            "{}",
        ),
        (
            "tianyi_eip_unused",
            "tianyi",
            "Unused EIPs",
            "Find Tianyi EIPs not bound to workloads.",
            "{}",
        ),
        (
            "tianyi_lb_idle",
            "tianyi",
            "Idle Load Balancers",
            "Find Tianyi load balancers without healthy backend traffic.",
            "{}",
        ),
        (
            "tianyi_oos_unused",
            "tianyi",
            "Idle OOS Buckets",
            "Detect low-value OOS storage buckets.",
            "{}",
        ),
        // OVHcloud
        (
            "ovh_instance_idle",
            "ovh",
            "Idle Instances",
            "Find stopped/shelved OVH Public Cloud instances.",
            "{}",
        ),
        (
            "ovh_volume_orphan",
            "ovh",
            "Unattached Volumes",
            "Find OVH block volumes not attached to instances.",
            "{}",
        ),
        (
            "ovh_ip_unused",
            "ovh",
            "Unused Public IPs",
            "Find OVH Public IPs that are not routed to workloads.",
            "{}",
        ),
        (
            "ovh_snapshot_old",
            "ovh",
            "Old Snapshots",
            "Find OVH snapshots older than 30 days.",
            "{}",
        ),
        // Hetzner
        (
            "hetzner_server_idle",
            "hetzner",
            "Idle Servers",
            "Find stopped Hetzner Cloud servers.",
            "{}",
        ),
        (
            "hetzner_volume_orphan",
            "hetzner",
            "Unattached Volumes",
            "Find Hetzner volumes not attached to servers.",
            "{}",
        ),
        (
            "hetzner_ip_unused",
            "hetzner",
            "Unused Floating IPs",
            "Find Hetzner floating IPs not assigned to servers.",
            "{}",
        ),
        (
            "hetzner_snapshot_old",
            "hetzner",
            "Old Snapshots",
            "Find Hetzner snapshots older than 30 days.",
            "{}",
        ),
        // Scaleway
        (
            "scw_server_idle",
            "scaleway",
            "Idle Instances",
            "Find stopped Scaleway instances.",
            "{}",
        ),
        (
            "scw_volume_orphan",
            "scaleway",
            "Unattached Volumes",
            "Find Scaleway block volumes not attached to instances.",
            "{}",
        ),
        (
            "scw_ip_unused",
            "scaleway",
            "Unused Public IPs",
            "Find Scaleway public IPs not assigned to instances.",
            "{}",
        ),
        (
            "scw_snapshot_old",
            "scaleway",
            "Old Snapshots",
            "Find Scaleway snapshots older than 30 days.",
            "{}",
        ),
        // Exoscale
        (
            "exo_instance_idle",
            "exoscale",
            "Idle Instances",
            "Find stopped Exoscale compute instances.",
            "{}",
        ),
        (
            "exo_volume_orphan",
            "exoscale",
            "Unattached Volumes",
            "Find Exoscale block volumes not attached to instances.",
            "{}",
        ),
        (
            "exo_ip_unused",
            "exoscale",
            "Unused Public IPs",
            "Find Exoscale public IPs not attached to workloads.",
            "{}",
        ),
        (
            "exo_snapshot_old",
            "exoscale",
            "Old Snapshots",
            "Find Exoscale snapshots older than 30 days.",
            "{}",
        ),
        // Leaseweb
        (
            "lw_instance_idle",
            "leaseweb",
            "Idle Instances",
            "Find stopped Leaseweb compute instances.",
            "{}",
        ),
        (
            "lw_volume_orphan",
            "leaseweb",
            "Unattached Volumes",
            "Find Leaseweb block volumes not attached to instances.",
            "{}",
        ),
        (
            "lw_ip_unused",
            "leaseweb",
            "Unused Public IPs",
            "Find Leaseweb public IPs not assigned to workloads.",
            "{}",
        ),
        (
            "lw_snapshot_old",
            "leaseweb",
            "Old Snapshots",
            "Find Leaseweb snapshots older than 30 days.",
            "{}",
        ),
        // UpCloud
        (
            "upc_instance_idle",
            "upcloud",
            "Idle Instances",
            "Find stopped UpCloud compute instances.",
            "{}",
        ),
        (
            "upc_volume_orphan",
            "upcloud",
            "Unattached Volumes",
            "Find UpCloud block volumes not attached to instances.",
            "{}",
        ),
        (
            "upc_ip_unused",
            "upcloud",
            "Unused Public IPs",
            "Find UpCloud public IPs not assigned to workloads.",
            "{}",
        ),
        (
            "upc_snapshot_old",
            "upcloud",
            "Old Snapshots",
            "Find UpCloud snapshots older than 30 days.",
            "{}",
        ),
        // Gcore
        (
            "gcore_instance_idle",
            "gcore",
            "Idle Instances",
            "Find stopped Gcore compute instances.",
            "{}",
        ),
        (
            "gcore_volume_orphan",
            "gcore",
            "Unattached Volumes",
            "Find Gcore block volumes not attached to instances.",
            "{}",
        ),
        (
            "gcore_ip_unused",
            "gcore",
            "Unused Public IPs",
            "Find Gcore public IPs not assigned to workloads.",
            "{}",
        ),
        (
            "gcore_snapshot_old",
            "gcore",
            "Old Snapshots",
            "Find Gcore snapshots older than 30 days.",
            "{}",
        ),
        // Contabo
        (
            "contabo_instance_idle",
            "contabo",
            "Idle Instances",
            "Find stopped Contabo compute instances.",
            "{}",
        ),
        (
            "contabo_volume_orphan",
            "contabo",
            "Unattached Volumes",
            "Find Contabo block volumes not attached to instances.",
            "{}",
        ),
        (
            "contabo_ip_unused",
            "contabo",
            "Unused Public IPs",
            "Find Contabo public IPs not assigned to workloads.",
            "{}",
        ),
        (
            "contabo_snapshot_old",
            "contabo",
            "Old Snapshots",
            "Find Contabo snapshots older than 30 days.",
            "{}",
        ),
        // Civo
        (
            "civo_instance_idle",
            "civo",
            "Idle Instances",
            "Find stopped Civo compute instances.",
            "{}",
        ),
        (
            "civo_volume_orphan",
            "civo",
            "Unattached Volumes",
            "Find Civo block volumes not attached to instances.",
            "{}",
        ),
        (
            "civo_ip_unused",
            "civo",
            "Unused Public IPs",
            "Find Civo reserved/public IPs not assigned to workloads.",
            "{}",
        ),
        (
            "civo_snapshot_old",
            "civo",
            "Old Snapshots",
            "Find Civo snapshots older than 30 days.",
            "{}",
        ),
        // Equinix Metal
        (
            "equinix_instance_idle",
            "equinix",
            "Idle Devices",
            "Find inactive Equinix Metal devices.",
            "{}",
        ),
        (
            "equinix_volume_orphan",
            "equinix",
            "Unattached Volumes",
            "Find Equinix Metal volumes not attached to devices.",
            "{}",
        ),
        (
            "equinix_ip_unused",
            "equinix",
            "Unused Reserved IPs",
            "Find Equinix Metal reserved IP blocks without active assignments.",
            "{}",
        ),
        (
            "equinix_snapshot_old",
            "equinix",
            "Old Snapshots",
            "Find Equinix volume snapshots older than 30 days.",
            "{}",
        ),
        // Rackspace
        (
            "rackspace_instance_idle",
            "rackspace",
            "Idle Instances",
            "Find stopped/suspended Rackspace cloud servers.",
            "{}",
        ),
        (
            "rackspace_volume_orphan",
            "rackspace",
            "Unattached Volumes",
            "Find Rackspace block volumes not attached to instances.",
            "{}",
        ),
        (
            "rackspace_ip_unused",
            "rackspace",
            "Unused Floating IPs",
            "Find Rackspace floating IPs not assigned to workloads.",
            "{}",
        ),
        (
            "rackspace_snapshot_old",
            "rackspace",
            "Old Snapshots",
            "Find Rackspace snapshots older than 30 days.",
            "{}",
        ),
        // OpenStack
        (
            "openstack_instance_idle",
            "openstack",
            "Idle Instances",
            "Find stopped/suspended OpenStack servers.",
            "{}",
        ),
        (
            "openstack_volume_orphan",
            "openstack",
            "Unattached Volumes",
            "Find OpenStack volumes not attached to servers.",
            "{}",
        ),
        (
            "openstack_ip_unused",
            "openstack",
            "Unused Floating IPs",
            "Find OpenStack floating IPs not assigned to workloads.",
            "{}",
        ),
        (
            "openstack_snapshot_old",
            "openstack",
            "Old Snapshots",
            "Find OpenStack snapshots older than 30 days.",
            "{}",
        ),
        // Wasabi
        (
            "wasabi_bucket_empty",
            "wasabi",
            "Empty Buckets",
            "Find Wasabi buckets with no objects.",
            "{}",
        ),
        (
            "wasabi_lifecycle_missing",
            "wasabi",
            "Missing Lifecycle Policies",
            "Find Wasabi buckets without lifecycle rules.",
            "{}",
        ),
        (
            "wasabi_multipart_orphan",
            "wasabi",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "wasabi_old_versions",
            "wasabi",
            "Old Object Versions",
            "Find Wasabi object versions older than 30 days.",
            "{}",
        ),
        // Backblaze B2
        (
            "backblaze_bucket_empty",
            "backblaze",
            "Empty Buckets",
            "Find Backblaze B2 buckets with no objects.",
            "{}",
        ),
        (
            "backblaze_lifecycle_missing",
            "backblaze",
            "Missing Lifecycle Policies",
            "Find Backblaze B2 buckets without lifecycle rules.",
            "{}",
        ),
        (
            "backblaze_multipart_orphan",
            "backblaze",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "backblaze_old_versions",
            "backblaze",
            "Old Object Versions",
            "Find Backblaze B2 object versions older than 30 days.",
            "{}",
        ),
        // IDrive e2
        (
            "idrive_bucket_empty",
            "idrive",
            "Empty Buckets",
            "Find IDrive e2 buckets with no objects.",
            "{}",
        ),
        (
            "idrive_lifecycle_missing",
            "idrive",
            "Missing Lifecycle Policies",
            "Find IDrive e2 buckets without lifecycle rules.",
            "{}",
        ),
        (
            "idrive_multipart_orphan",
            "idrive",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "idrive_old_versions",
            "idrive",
            "Old Object Versions",
            "Find IDrive e2 object versions older than 30 days.",
            "{}",
        ),
        // Storj DCS
        (
            "storj_bucket_empty",
            "storj",
            "Empty Buckets",
            "Find Storj DCS buckets with no objects.",
            "{}",
        ),
        (
            "storj_lifecycle_missing",
            "storj",
            "Missing Lifecycle Policies",
            "Find Storj DCS buckets without lifecycle rules.",
            "{}",
        ),
        (
            "storj_multipart_orphan",
            "storj",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "storj_old_versions",
            "storj",
            "Old Object Versions",
            "Find Storj DCS object versions older than 30 days.",
            "{}",
        ),
        // DreamHost DreamObjects
        (
            "dreamhost_bucket_empty",
            "dreamhost",
            "Empty Buckets",
            "Find DreamHost DreamObjects buckets with no objects.",
            "{}",
        ),
        (
            "dreamhost_lifecycle_missing",
            "dreamhost",
            "Missing Lifecycle Policies",
            "Find DreamHost DreamObjects buckets without lifecycle rules.",
            "{}",
        ),
        (
            "dreamhost_multipart_orphan",
            "dreamhost",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "dreamhost_old_versions",
            "dreamhost",
            "Old Object Versions",
            "Find DreamHost DreamObjects object versions older than 30 days.",
            "{}",
        ),
        // Cloudian HyperStore
        (
            "cloudian_bucket_empty",
            "cloudian",
            "Empty Buckets",
            "Find Cloudian HyperStore buckets with no objects.",
            "{}",
        ),
        (
            "cloudian_lifecycle_missing",
            "cloudian",
            "Missing Lifecycle Policies",
            "Find Cloudian HyperStore buckets without lifecycle rules.",
            "{}",
        ),
        (
            "cloudian_multipart_orphan",
            "cloudian",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "cloudian_old_versions",
            "cloudian",
            "Old Object Versions",
            "Find Cloudian HyperStore object versions older than 30 days.",
            "{}",
        ),
        // Generic S3-Compatible
        (
            "s3compatible_bucket_empty",
            "s3compatible",
            "Empty Buckets",
            "Find S3-compatible buckets with no objects.",
            "{}",
        ),
        (
            "s3compatible_lifecycle_missing",
            "s3compatible",
            "Missing Lifecycle Policies",
            "Find S3-compatible buckets without lifecycle rules.",
            "{}",
        ),
        (
            "s3compatible_multipart_orphan",
            "s3compatible",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "s3compatible_old_versions",
            "s3compatible",
            "Old Object Versions",
            "Find S3-compatible object versions older than 30 days.",
            "{}",
        ),
        // MinIO
        (
            "minio_bucket_empty",
            "minio",
            "Empty Buckets",
            "Find MinIO buckets with no objects.",
            "{}",
        ),
        (
            "minio_lifecycle_missing",
            "minio",
            "Missing Lifecycle Policies",
            "Find MinIO buckets without lifecycle rules.",
            "{}",
        ),
        (
            "minio_multipart_orphan",
            "minio",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "minio_old_versions",
            "minio",
            "Old Object Versions",
            "Find MinIO object versions older than 30 days.",
            "{}",
        ),
        // Ceph RGW
        (
            "ceph_bucket_empty",
            "ceph",
            "Empty Buckets",
            "Find Ceph RGW buckets with no objects.",
            "{}",
        ),
        (
            "ceph_lifecycle_missing",
            "ceph",
            "Missing Lifecycle Policies",
            "Find Ceph RGW buckets without lifecycle rules.",
            "{}",
        ),
        (
            "ceph_multipart_orphan",
            "ceph",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "ceph_old_versions",
            "ceph",
            "Old Object Versions",
            "Find Ceph RGW object versions older than 30 days.",
            "{}",
        ),
        // Seagate Lyve Cloud
        (
            "lyve_bucket_empty",
            "lyve",
            "Empty Buckets",
            "Find Seagate Lyve Cloud buckets with no objects.",
            "{}",
        ),
        (
            "lyve_lifecycle_missing",
            "lyve",
            "Missing Lifecycle Policies",
            "Find Seagate Lyve Cloud buckets without lifecycle rules.",
            "{}",
        ),
        (
            "lyve_multipart_orphan",
            "lyve",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "lyve_old_versions",
            "lyve",
            "Old Object Versions",
            "Find Seagate Lyve Cloud object versions older than 30 days.",
            "{}",
        ),
        // Dell EMC ECS
        (
            "dell_bucket_empty",
            "dell",
            "Empty Buckets",
            "Find Dell EMC ECS buckets with no objects.",
            "{}",
        ),
        (
            "dell_lifecycle_missing",
            "dell",
            "Missing Lifecycle Policies",
            "Find Dell EMC ECS buckets without lifecycle rules.",
            "{}",
        ),
        (
            "dell_multipart_orphan",
            "dell",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "dell_old_versions",
            "dell",
            "Old Object Versions",
            "Find Dell EMC ECS object versions older than 30 days.",
            "{}",
        ),
        // NetApp StorageGRID
        (
            "storagegrid_bucket_empty",
            "storagegrid",
            "Empty Buckets",
            "Find NetApp StorageGRID buckets with no objects.",
            "{}",
        ),
        (
            "storagegrid_lifecycle_missing",
            "storagegrid",
            "Missing Lifecycle Policies",
            "Find NetApp StorageGRID buckets without lifecycle rules.",
            "{}",
        ),
        (
            "storagegrid_multipart_orphan",
            "storagegrid",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "storagegrid_old_versions",
            "storagegrid",
            "Old Object Versions",
            "Find NetApp StorageGRID object versions older than 30 days.",
            "{}",
        ),
        // Scality
        (
            "scality_bucket_empty",
            "scality",
            "Empty Buckets",
            "Find Scality buckets with no objects.",
            "{}",
        ),
        (
            "scality_lifecycle_missing",
            "scality",
            "Missing Lifecycle Policies",
            "Find Scality buckets without lifecycle rules.",
            "{}",
        ),
        (
            "scality_multipart_orphan",
            "scality",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "scality_old_versions",
            "scality",
            "Old Object Versions",
            "Find Scality object versions older than 30 days.",
            "{}",
        ),
        // Hitachi HCP
        (
            "hcp_bucket_empty",
            "hcp",
            "Empty Buckets",
            "Find Hitachi HCP buckets with no objects.",
            "{}",
        ),
        (
            "hcp_lifecycle_missing",
            "hcp",
            "Missing Lifecycle Policies",
            "Find Hitachi HCP buckets without lifecycle rules.",
            "{}",
        ),
        (
            "hcp_multipart_orphan",
            "hcp",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "hcp_old_versions",
            "hcp",
            "Old Object Versions",
            "Find Hitachi HCP object versions older than 30 days.",
            "{}",
        ),
        // Qumulo
        (
            "qumulo_bucket_empty",
            "qumulo",
            "Empty Buckets",
            "Find Qumulo buckets with no objects.",
            "{}",
        ),
        (
            "qumulo_lifecycle_missing",
            "qumulo",
            "Missing Lifecycle Policies",
            "Find Qumulo buckets without lifecycle rules.",
            "{}",
        ),
        (
            "qumulo_multipart_orphan",
            "qumulo",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "qumulo_old_versions",
            "qumulo",
            "Old Object Versions",
            "Find Qumulo object versions older than 30 days.",
            "{}",
        ),
        // Nutanix Objects
        (
            "nutanix_bucket_empty",
            "nutanix",
            "Empty Buckets",
            "Find Nutanix Objects buckets with no objects.",
            "{}",
        ),
        (
            "nutanix_lifecycle_missing",
            "nutanix",
            "Missing Lifecycle Policies",
            "Find Nutanix Objects buckets without lifecycle rules.",
            "{}",
        ),
        (
            "nutanix_multipart_orphan",
            "nutanix",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "nutanix_old_versions",
            "nutanix",
            "Old Object Versions",
            "Find Nutanix Objects versions older than 30 days.",
            "{}",
        ),
        // Pure Storage FlashBlade
        (
            "flashblade_bucket_empty",
            "flashblade",
            "Empty Buckets",
            "Find FlashBlade buckets with no objects.",
            "{}",
        ),
        (
            "flashblade_lifecycle_missing",
            "flashblade",
            "Missing Lifecycle Policies",
            "Find FlashBlade buckets without lifecycle rules.",
            "{}",
        ),
        (
            "flashblade_multipart_orphan",
            "flashblade",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "flashblade_old_versions",
            "flashblade",
            "Old Object Versions",
            "Find FlashBlade object versions older than 30 days.",
            "{}",
        ),
        // HPE GreenLake
        (
            "greenlake_bucket_empty",
            "greenlake",
            "Empty Buckets",
            "Find HPE GreenLake buckets with no objects.",
            "{}",
        ),
        (
            "greenlake_lifecycle_missing",
            "greenlake",
            "Missing Lifecycle Policies",
            "Find HPE GreenLake buckets without lifecycle rules.",
            "{}",
        ),
        (
            "greenlake_multipart_orphan",
            "greenlake",
            "Orphan Multipart Uploads",
            "Find incomplete multipart uploads that may incur storage cost.",
            "{}",
        ),
        (
            "greenlake_old_versions",
            "greenlake",
            "Old Object Versions",
            "Find HPE GreenLake object versions older than 30 days.",
            "{}",
        ),
        // IONOS Cloud
        (
            "ionos_instance_idle",
            "ionos",
            "Idle Instances",
            "Find stopped IONOS Cloud servers.",
            "{}",
        ),
        (
            "ionos_volume_orphan",
            "ionos",
            "Unattached Volumes",
            "Find IONOS block volumes not attached to servers.",
            "{}",
        ),
        (
            "ionos_ipblock_unused",
            "ionos",
            "Unused IP Blocks",
            "Find IONOS IP blocks without assigned addresses.",
            "{}",
        ),
        (
            "ionos_snapshot_old",
            "ionos",
            "Old Snapshots",
            "Find IONOS snapshots older than 30 days.",
            "{}",
        ),
    ];

    for (id, provider, name, desc, params) in rules {
        sqlx::query("INSERT OR IGNORE INTO scan_rule_templates (id, provider, name, description, default_params) VALUES (?, ?, ?, ?, ?)")
            .bind(id).bind(provider).bind(name).bind(desc).bind(params)
            .execute(pool).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub async fn get_account_rules_config(
    pool: &Pool<Sqlite>,
    account_id: &str,
) -> Result<Vec<serde_json::Value>, String> {
    // Return a merged view: Template + User Config
    // If user config exists, use it. If not, use default enabled=true.
    let provider = if account_id.starts_with("aws_local:") {
        "aws".to_string()
    } else {
        let provider_row = sqlx::query("SELECT provider FROM cloud_profiles WHERE id = ?")
            .bind(account_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

        match provider_row {
            Some(row) => row.get::<String, _>("provider"),
            None => return Err("Account not found".into()),
        }
    };

    // Left Join to get config if exists
    let rows = sqlx::query(
        "SELECT t.id, t.name, t.description, t.default_params, 
                COALESCE(c.enabled, 1) as enabled, 
                COALESCE(c.custom_params, t.default_params) as params
         FROM scan_rule_templates t
         LEFT JOIN account_scan_config c ON t.id = c.rule_id AND c.account_id = ?
         WHERE t.provider = ?",
    )
    .bind(account_id)
    .bind(&provider)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for r in rows {
        results.push(serde_json::json!({
            "id": r.get::<String, _>("id"),
            "name": r.get::<String, _>("name"),
            "description": r.get::<String, _>("description"),
            "enabled": r.get::<bool, _>("enabled"),
            "params": r.get::<String, _>("params")
        }));
    }
    Ok(results)
}

pub async fn get_provider_rules_config(
    pool: &Pool<Sqlite>,
    provider: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let normalized_provider = if provider == "aws_local" {
        "aws"
    } else {
        provider
    };

    let rows = sqlx::query(
        "SELECT id, name, description, default_params
         FROM scan_rule_templates
         WHERE provider = ?",
    )
    .bind(normalized_provider)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for row in rows {
        results.push(serde_json::json!({
            "id": row.get::<String, _>("id"),
            "name": row.get::<String, _>("name"),
            "description": row.get::<String, _>("description"),
            "enabled": true,
            "params": row.get::<String, _>("default_params")
        }));
    }

    Ok(results)
}

pub async fn update_account_rule_config(
    pool: &Pool<Sqlite>,
    account_id: &str,
    rule_id: &str,
    enabled: bool,
    params: Option<String>,
) -> Result<(), String> {
    sqlx::query("INSERT OR REPLACE INTO account_scan_config (account_id, rule_id, enabled, custom_params) VALUES (?, ?, ?, ?)")
        .bind(account_id).bind(rule_id).bind(enabled).bind(params)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn seed_default_policies(pool: &Pool<Sqlite>) -> Result<(), String> {
    let count: i64 = sqlx::query("SELECT COUNT(*) as c FROM policies")
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?
        .get("c");
    if count == 0 {
        // 1. Idle EC2
        let p1 = Policy {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Aggressive Idle EC2 Check".into(),
            description: Some(
                "Flag instances with very low CPU (< 2%) and Network traffic.".into(),
            ),
            target_type: "ec2".into(),
            conditions: vec![
                PolicyCondition {
                    metric: "cpu".into(),
                    operator: "<".into(),
                    value: 2.0,
                    unit: Some("%".into()),
                },
                PolicyCondition {
                    metric: "network_in".into(),
                    operator: "<".into(),
                    value: 5.0,
                    unit: Some("MB".into()),
                },
            ],
            logic: "AND".into(),
            is_active: true,
            priority: 10,
        };
        save_policy(pool, &p1).await?;

        // 2. Unattached Disks
        let p2 = Policy {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Orphaned EBS Volumes".into(),
            description: Some("Identify disks not attached to any instance.".into()),
            target_type: "disk".into(),
            conditions: vec![PolicyCondition {
                metric: "status".into(),
                operator: "=".into(),
                value: 0.0,
                unit: Some("available".into()),
            }],
            logic: "AND".into(),
            is_active: true,
            priority: 20,
        };
        save_policy(pool, &p2).await?;
    }
    Ok(())
}

pub async fn save_policy(pool: &Pool<Sqlite>, policy: &Policy) -> Result<(), String> {
    let conditions_json = serde_json::to_string(&policy.conditions).map_err(|e| e.to_string())?;
    sqlx::query("INSERT OR REPLACE INTO policies (id, name, description, target_type, conditions, logic, is_active, priority) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&policy.id).bind(&policy.name).bind(&policy.description).bind(&policy.target_type)
        .bind(conditions_json).bind(&policy.logic).bind(policy.is_active).bind(policy.priority)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn list_policies(pool: &Pool<Sqlite>) -> Result<Vec<Policy>, String> {
    let rows = sqlx::query_as::<_, DbPolicy>("SELECT * FROM policies ORDER BY priority DESC")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut policies = Vec::new();
    for r in rows {
        policies.push(Policy {
            id: r.id,
            name: r.name,
            description: r.description,
            target_type: r.target_type,
            conditions: serde_json::from_str(&r.conditions).unwrap_or_default(),
            logic: r.logic,
            is_active: r.is_active,
            priority: r.priority,
        });
    }
    Ok(policies)
}

pub async fn delete_policy(pool: &Pool<Sqlite>, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM policies WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn save_cloud_profile(
    pool: &Pool<Sqlite>,
    provider: &str,
    name: &str,
    creds: &str,
    timeout: Option<i64>,
    policy: Option<String>,
    proxy_profile_id: Option<String>,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO cloud_profiles (id, provider, name, credentials, created_at, timeout_seconds, policy_custom, proxy_profile_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(provider)
    .bind(name)
    .bind(creds)
    .bind(now)
    .bind(timeout)
    .bind(policy)
    .bind(proxy_profile_id)
    .execute(pool).await.map_err(|e| e.to_string())?;

    Ok(id)
}

pub async fn update_cloud_profile(
    pool: &Pool<Sqlite>,
    id: &str,
    provider: &str,
    name: &str,
    creds: &str,
    timeout: Option<i64>,
    policy: Option<String>,
    proxy_profile_id: Option<String>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE cloud_profiles SET provider = ?, name = ?, credentials = ?, timeout_seconds = ?, policy_custom = ?, proxy_profile_id = ? WHERE id = ?"
    )
    .bind(provider)
    .bind(name)
    .bind(creds)
    .bind(timeout)
    .bind(policy)
    .bind(proxy_profile_id)
    .bind(id)
    .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn list_cloud_profiles(pool: &Pool<Sqlite>) -> Result<Vec<CloudProfile>, String> {
    let profiles = sqlx::query_as::<_, CloudProfile>("SELECT * FROM cloud_profiles")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(profiles)
}

pub async fn save_proxy_profile(
    pool: &Pool<Sqlite>,
    id: Option<String>,
    name: &str,
    protocol: &str,
    host: &str,
    port: i64,
    auth_username: Option<&str>,
    auth_password: Option<&str>,
) -> Result<String, String> {
    let profile_id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO proxy_profiles (id, name, protocol, host, port, auth_username, auth_password, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, protocol = excluded.protocol, host = excluded.host, port = excluded.port, auth_username = excluded.auth_username, auth_password = excluded.auth_password",
    )
    .bind(&profile_id)
    .bind(name)
    .bind(protocol)
    .bind(host)
    .bind(port)
    .bind(auth_username)
    .bind(auth_password)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(profile_id)
}

pub async fn list_proxy_profiles(pool: &Pool<Sqlite>) -> Result<Vec<ProxyProfile>, String> {
    let profiles =
        sqlx::query_as::<_, ProxyProfile>("SELECT * FROM proxy_profiles ORDER BY created_at DESC")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(profiles)
}

pub async fn get_proxy_profile(
    pool: &Pool<Sqlite>,
    id: &str,
) -> Result<Option<ProxyProfile>, String> {
    let profile = sqlx::query_as::<_, ProxyProfile>("SELECT * FROM proxy_profiles WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(profile)
}

pub async fn delete_proxy_profile(pool: &Pool<Sqlite>, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM proxy_profiles WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    let _ =
        sqlx::query("UPDATE cloud_profiles SET proxy_profile_id = NULL WHERE proxy_profile_id = ?")
            .bind(id)
            .execute(pool)
            .await;
    let _ = sqlx::query(
        "UPDATE notification_channels SET proxy_profile_id = NULL WHERE proxy_profile_id = ?",
    )
    .bind(id)
    .execute(pool)
    .await;
    Ok(())
}

// Settings Table
pub async fn save_setting(pool: &Pool<Sqlite>, key: &str, value: &str) -> Result<(), String> {
    sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_setting(pool: &Pool<Sqlite>, key: &str) -> Result<String, String> {
    let row = sqlx::query("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(row.map(|r| r.get("value")).unwrap_or_default())
}

pub async fn get_or_create_machine_id(pool: &Pool<Sqlite>) -> Result<String, String> {
    let key = "machine_id";
    if let Ok(val) = get_setting(pool, key).await {
        if !val.is_empty() {
            return Ok(val);
        }
    }
    let new_id = uuid::Uuid::new_v4().to_string();
    save_setting(pool, key, &new_id).await?;
    Ok(new_id)
}

pub async fn delete_cloud_profile(pool: &Pool<Sqlite>, id: &str) -> Result<String, String> {
    let row = sqlx::query("SELECT name FROM cloud_profiles WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

    let name = row
        .map(|r| r.get::<String, _>("name"))
        .unwrap_or_else(|| "Unknown Account".to_string());

    sqlx::query("DELETE FROM cloud_profiles WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(name)
}

pub async fn record_audit_log(
    pool: &Pool<Sqlite>,
    action: &str,
    target: &str,
    details: &str,
) -> Result<(), String> {
    let now = Utc::now().timestamp();
    sqlx::query("INSERT INTO audit_logs (action, target, details, created_at) VALUES (?, ?, ?, ?)")
        .bind(action)
        .bind(target)
        .bind(details)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn record_cleanup(
    pool: &Pool<Sqlite>,
    r_id: &str,
    r_type: &str,
    amount: f64,
) -> Result<(), String> {
    let now = Utc::now().timestamp();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO cleanup_history (resource_id, resource_type, saved_amount, cleaned_at) VALUES (?, ?, ?, ?)").bind(r_id).bind(r_type).bind(amount).bind(now).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM scanned_resources WHERE id = ?")
        .bind(r_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO audit_logs (action, target, details, created_at) VALUES (?, ?, ?, ?)")
        .bind("CLEANUP")
        .bind(r_id)
        .bind(format!("Deleted {} (Saved ${})", r_type, amount))
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn clear_scan_results(pool: &Pool<Sqlite>) -> Result<(), String> {
    let now = Utc::now().timestamp();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM scanned_resources")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO audit_logs (action, target, details, created_at) VALUES (?, ?, ?, ?)")
        .bind("RESET")
        .bind("ALL")
        .bind("Cleared all scan results")
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn clear_audit_logs(pool: &Pool<Sqlite>) -> Result<(), String> {
    sqlx::query("DELETE FROM audit_logs")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_audit_logs(
    pool: &Pool<Sqlite>,
    date_from: Option<i64>,
    date_to: Option<i64>,
    limit: i64,
    offset: i64,
) -> Result<Vec<AuditLog>, String> {
    let mut sql = "SELECT * FROM audit_logs WHERE 1=1".to_string();

    if date_from.is_some() {
        sql.push_str(&format!(" AND created_at >= {}", date_from.unwrap()));
    }
    if date_to.is_some() {
        sql.push_str(&format!(" AND created_at <= {}", date_to.unwrap()));
    }

    sql.push_str(&format!(
        " ORDER BY created_at DESC LIMIT {} OFFSET {}",
        limit, offset
    ));

    let rows = sqlx::query_as::<_, AuditLog>(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub async fn get_stats(pool: &Pool<Sqlite>) -> Result<Stats, String> {
    // 1. Potential Savings (from scan)
    let scan_row = sqlx::query("SELECT COALESCE(SUM(estimated_monthly_cost), 0.0) as total, COUNT(*) as count FROM scanned_resources")
        .fetch_one(pool).await.map_err(|e| e.to_string())?;
    let total_savings: f64 = scan_row.try_get("total").unwrap_or(0.0);
    let wasted_resource_count: i64 = scan_row.try_get("count").unwrap_or(0);

    // 2. Historical Cleanups
    let clean_row = sqlx::query("SELECT COUNT(*) as count FROM cleanup_history")
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    let cleanup_count: i64 = clean_row.try_get("count").unwrap_or(0);

    // 3. History Graph
    let history_rows =
        sqlx::query("SELECT cleaned_at, saved_amount FROM cleanup_history ORDER BY cleaned_at ASC")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

    let mut history = Vec::new();
    let mut cumulative = 0.0;
    for r in history_rows {
        let ts: i64 = r.try_get("cleaned_at").unwrap_or(0);
        let amount: f64 = r.try_get("saved_amount").unwrap_or(0.0);
        cumulative += amount;
        history.push((ts, cumulative));
    }

    Ok(Stats {
        total_savings,
        wasted_resource_count,
        cleanup_count,
        history,
    })
}

pub async fn is_license_used(pool: &Pool<Sqlite>, license_id: &str) -> Result<bool, String> {
    let rec = sqlx::query("SELECT COUNT(*) as count FROM license_usage WHERE license_id = ?")
        .bind(license_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    let count: i64 = rec.try_get("count").unwrap_or(0);
    Ok(count > 0)
}

pub async fn mark_license_used(pool: &Pool<Sqlite>, license_id: &str) -> Result<(), String> {
    let now = Utc::now().timestamp();
    sqlx::query("INSERT OR IGNORE INTO license_usage (license_id, used_at) VALUES (?, ?)")
        .bind(license_id)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn save_scan_results(
    pool: &Pool<Sqlite>,
    resources: &[WastedResource],
) -> Result<(), String> {
    let now = Utc::now().timestamp();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM scanned_resources")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    for r in resources {
        sqlx::query("INSERT INTO scanned_resources (id, provider, region, resource_type, details, estimated_monthly_cost, scanned_at, action_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&r.id)
            .bind(&r.provider)
            .bind(&r.region)
            .bind(&r.resource_type)
            .bind(&r.details)
            .bind(r.estimated_monthly_cost)
            .bind(now)
            .bind(&r.action_type)
            .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_scan_results(pool: &Pool<Sqlite>) -> Result<Vec<WastedResource>, String> {
    let rows = sqlx::query("SELECT id, provider, region, resource_type, details, estimated_monthly_cost, action_type FROM scanned_resources").fetch_all(pool).await.map_err(|e| e.to_string())?;
    let mut resources = Vec::new();
    for r in rows {
        resources.push(WastedResource {
            id: r.try_get("id").unwrap_or_default(),
            provider: r.try_get("provider").unwrap_or_default(),
            region: r.try_get("region").unwrap_or_default(),
            resource_type: r.try_get("resource_type").unwrap_or_default(),
            details: r.try_get("details").unwrap_or_default(),
            estimated_monthly_cost: r.try_get("estimated_monthly_cost").unwrap_or(0.0),
            action_type: r.try_get("action_type").unwrap_or("DELETE".to_string()),
        });
    }
    Ok(resources)
}

pub async fn save_resource_metrics(
    pool: &Pool<Sqlite>,
    metrics: &[MonitorMetric],
) -> Result<(), String> {
    if metrics.is_empty() {
        return Ok(());
    }

    let now = Utc::now().timestamp();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM resource_metrics")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    for m in metrics {
        sqlx::query(
            "INSERT OR REPLACE INTO resource_metrics (id, provider, region, resource_type, name, status, cpu_utilization, network_in_mb, connections, source, account_id, updated_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&m.id)
        .bind(&m.provider)
        .bind(&m.region)
        .bind(&m.resource_type)
        .bind(&m.name)
        .bind(&m.status)
        .bind(m.cpu_utilization)
        .bind(m.network_in_mb)
        .bind(m.connections)
        .bind(&m.source)
        .bind(&m.account_id)
        .bind(now)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

        sqlx::query(
            "INSERT INTO resource_metrics_history (resource_id, provider, region, resource_type, name, status, cpu_utilization, network_in_mb, connections, source, account_id, collected_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&m.id)
        .bind(&m.provider)
        .bind(&m.region)
        .bind(&m.resource_type)
        .bind(&m.name)
        .bind(&m.status)
        .bind(m.cpu_utilization)
        .bind(m.network_in_mb)
        .bind(m.connections)
        .bind(&m.source)
        .bind(&m.account_id)
        .bind(now)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_all_metrics(pool: &Pool<Sqlite>) -> Result<Vec<MonitorMetric>, String> {
    let metrics = sqlx::query_as::<_, MonitorMetric>(
        "SELECT id, provider, region, resource_type, name, status, cpu_utilization, network_in_mb, connections, COALESCE(updated_at, 0) as updated_at, source, account_id
         FROM resource_metrics
         ORDER BY COALESCE(updated_at, 0) DESC, COALESCE(cpu_utilization, -1) DESC"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(metrics)
}

pub async fn get_monitor_snapshots(
    pool: &Pool<Sqlite>,
    limit: i64,
) -> Result<Vec<MonitorSnapshot>, String> {
    let mut snapshots = sqlx::query_as::<_, MonitorSnapshot>(
        "SELECT collected_at,
                COUNT(*) as total_resources,
                SUM(CASE WHEN cpu_utilization IS NOT NULL AND cpu_utilization < 2 THEN 1 ELSE 0 END) as idle_resources,
                SUM(CASE WHEN cpu_utilization IS NOT NULL AND cpu_utilization > 80 THEN 1 ELSE 0 END) as high_load_resources
         FROM resource_metrics_history
         GROUP BY collected_at
         ORDER BY collected_at DESC
         LIMIT ?"
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    snapshots.reverse();
    Ok(snapshots)
}

pub async fn get_monitor_snapshots_window(
    pool: &Pool<Sqlite>,
    window_days: i64,
) -> Result<Vec<MonitorSnapshot>, String> {
    let days = window_days.clamp(1, 90);
    let now = Utc::now().timestamp();
    let since_ts = now - days * 86_400;
    let bucket_seconds = if days <= 7 {
        3 * 3600
    } else if days <= 30 {
        12 * 3600
    } else {
        24 * 3600
    };
    let max_points = if days <= 7 {
        80
    } else if days <= 30 {
        100
    } else {
        120
    };

    let snapshots = sqlx::query_as::<_, MonitorSnapshot>(
        "WITH per_snapshot AS (
             SELECT collected_at,
                    COUNT(*) as total_resources,
                    SUM(CASE WHEN cpu_utilization IS NOT NULL AND cpu_utilization < 2 THEN 1 ELSE 0 END) as idle_resources,
                    SUM(CASE WHEN cpu_utilization IS NOT NULL AND cpu_utilization > 80 THEN 1 ELSE 0 END) as high_load_resources
             FROM resource_metrics_history
             WHERE collected_at >= ?
             GROUP BY collected_at
         ),
         bucketed AS (
             SELECT ((collected_at / ?) * ?) as bucket_ts,
                    AVG(total_resources) as total_resources_avg,
                    AVG(idle_resources) as idle_resources_avg,
                    AVG(high_load_resources) as high_load_resources_avg
             FROM per_snapshot
             GROUP BY ((collected_at / ?) * ?)
         )
         SELECT collected_at,
                total_resources,
                idle_resources,
                high_load_resources
         FROM (
             SELECT bucket_ts as collected_at,
                    CAST(ROUND(total_resources_avg, 0) AS INTEGER) as total_resources,
                    CAST(ROUND(idle_resources_avg, 0) AS INTEGER) as idle_resources,
                    CAST(ROUND(high_load_resources_avg, 0) AS INTEGER) as high_load_resources
             FROM bucketed
             ORDER BY bucket_ts DESC
             LIMIT ?
         ) recent
         ORDER BY collected_at ASC"
    )
    .bind(since_ts)
    .bind(bucket_seconds)
    .bind(bucket_seconds)
    .bind(bucket_seconds)
    .bind(bucket_seconds)
    .bind(max_points)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(snapshots)
}

pub async fn save_notification_channel(
    pool: &Pool<Sqlite>,
    channel: &NotificationChannel,
) -> Result<(), String> {
    sqlx::query(
        "INSERT OR REPLACE INTO notification_channels (id, name, method, config, is_active, proxy_profile_id, trigger_mode, min_savings, min_findings) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&channel.id)
    .bind(&channel.name)
    .bind(&channel.method)
    .bind(&channel.config)
    .bind(channel.is_active)
    .bind(channel.proxy_profile_id.clone())
    .bind(channel.trigger_mode.clone())
    .bind(channel.min_savings)
    .bind(channel.min_findings)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn list_notification_channels(
    pool: &Pool<Sqlite>,
) -> Result<Vec<NotificationChannel>, String> {
    let channels = sqlx::query_as::<_, NotificationChannel>("SELECT * FROM notification_channels")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(channels)
}

pub async fn delete_notification_channel(pool: &Pool<Sqlite>, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM notification_channels WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
// History & Handling
pub async fn save_scan_history(
    pool: &Pool<Sqlite>,
    waste: f64,
    count: i64,
    results: &[WastedResource],
    meta: &serde_json::Value,
) -> Result<i64, String> {
    let now = Utc::now().timestamp();
    let json = serde_json::to_string(results).map_err(|e| e.to_string())?;
    let meta_str = meta.to_string();

    let id = sqlx::query("INSERT INTO scan_history (scanned_at, total_waste, resource_count, status, results_json, scan_meta) VALUES (?, ?, ?, 'completed', ?, ?) RETURNING id")
        .bind(now).bind(waste).bind(count).bind(json).bind(meta_str)
        .fetch_one(pool).await.map_err(|e| e.to_string())?
        .get("id");

    Ok(id)
}

pub async fn get_scan_history(pool: &Pool<Sqlite>) -> Result<Vec<ScanHistoryItem>, String> {
    // Return summary only (empty json to save bandwidth if list is long? No, user might want to export)
    // Actually for listing, we don't need the full JSON.
    // Let's create a lightweight struct or just select specific fields?
    // For simplicity, fetch all.
    let rows = sqlx::query_as::<_, ScanHistoryItem>("SELECT id, scanned_at, total_waste, resource_count, status, results_json, scan_meta FROM scan_history ORDER BY scanned_at DESC")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;
    Ok(rows)
}

pub async fn delete_scan_history(pool: &Pool<Sqlite>, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM scan_history WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn mark_resource_handled(
    pool: &Pool<Sqlite>,
    r_id: &str,
    provider: &str,
    note: Option<String>,
) -> Result<(), String> {
    let now = Utc::now().timestamp();
    sqlx::query("INSERT OR REPLACE INTO handled_resources (resource_id, provider, handled_at, note) VALUES (?, ?, ?, ?)")
        .bind(r_id).bind(provider).bind(now).bind(note)
        .execute(pool).await.map_err(|e| e.to_string())?;

    // Also remove from current scan results if present
    sqlx::query("DELETE FROM scanned_resources WHERE id = ?")
        .bind(r_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub async fn get_handled_resources(pool: &Pool<Sqlite>) -> Result<Vec<String>, String> {
    let rows = sqlx::query("SELECT resource_id FROM handled_resources")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    let ids = rows.iter().map(|r| r.get("resource_id")).collect();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("cws-db-test-{}-{}.sqlite", name, Uuid::new_v4()))
    }

    async fn fresh_db(name: &str) -> (PathBuf, Pool<Sqlite>) {
        let path = temp_db_path(name);
        let pool = init_db(&path).await.expect("init db");
        (path, pool)
    }

    #[tokio::test]
    async fn init_db_creates_expected_tables_and_settings_round_trip() {
        let (path, pool) = fresh_db("settings").await;

        save_setting(&pool, "proxy_mode", "custom")
            .await
            .expect("save setting");
        assert_eq!(
            get_setting(&pool, "proxy_mode").await.expect("get setting"),
            "custom"
        );
        assert_eq!(
            get_setting(&pool, "missing").await.expect("get missing"),
            ""
        );

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn proxy_profiles_save_get_and_list_in_newest_first_order() {
        let (path, pool) = fresh_db("proxy-profiles").await;

        let first_id = save_proxy_profile(
            &pool,
            Some("proxy-old".to_string()),
            "Old Proxy",
            "http",
            "old.example.com",
            8080,
            None,
            None,
        )
        .await
        .expect("save old proxy");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let second_id = save_proxy_profile(
            &pool,
            Some("proxy-new".to_string()),
            "New Proxy",
            "socks5h",
            "new.example.com",
            1080,
            Some("ops"),
            Some("secret"),
        )
        .await
        .expect("save new proxy");

        let fetched = get_proxy_profile(&pool, &second_id)
            .await
            .expect("get proxy")
            .expect("proxy exists");
        assert_eq!(fetched.name, "New Proxy");
        assert_eq!(fetched.protocol, "socks5h");
        assert_eq!(fetched.auth_username.as_deref(), Some("ops"));

        let listed = list_proxy_profiles(&pool).await.expect("list proxies");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, second_id);
        assert_eq!(listed[1].id, first_id);

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn audit_logs_return_descending_created_at() {
        let (path, pool) = fresh_db("audit").await;

        sqlx::query(
            "INSERT INTO audit_logs (action, target, details, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("SCAN")
        .bind("aws-prod")
        .bind("first")
        .bind(100_i64)
        .execute(&pool)
        .await
        .expect("insert first");
        sqlx::query(
            "INSERT INTO audit_logs (action, target, details, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind("SCAN")
        .bind("azure-finance")
        .bind("second")
        .bind(200_i64)
        .execute(&pool)
        .await
        .expect("insert second");

        let rows = get_audit_logs(&pool, None, None, 10, 0)
            .await
            .expect("get audit logs");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].created_at, 200);
        assert_eq!(rows[0].target, "azure-finance");
        assert_eq!(rows[1].created_at, 100);

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn audit_logs_support_date_filters_and_pagination() {
        let (path, pool) = fresh_db("audit-filters").await;

        for (created_at, target) in [
            (100_i64, "aws-dev"),
            (200_i64, "aws-prod"),
            (300_i64, "azure-finance"),
        ] {
            sqlx::query(
                "INSERT INTO audit_logs (action, target, details, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind("SCAN")
            .bind(target)
            .bind("detail")
            .bind(created_at)
            .execute(&pool)
            .await
            .expect("insert audit log");
        }

        let filtered = get_audit_logs(&pool, Some(150), Some(250), 10, 0)
            .await
            .expect("get filtered logs");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].target, "aws-prod");

        let paged = get_audit_logs(&pool, None, None, 1, 1)
            .await
            .expect("get paged logs");
        assert_eq!(paged.len(), 1);
        assert_eq!(paged[0].target, "aws-prod");

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn notification_channels_round_trip_all_enterprise_fields() {
        let (path, pool) = fresh_db("notification-channels").await;

        let first = NotificationChannel {
            id: "email-finance".to_string(),
            name: "Finance Email".to_string(),
            method: "email".to_string(),
            config: r#"{"emails":["finops@example.com"]}"#.to_string(),
            is_active: true,
            proxy_profile_id: Some("proxy-finance".to_string()),
            trigger_mode: Some("waste_only".to_string()),
            min_savings: Some(250.0),
            min_findings: Some(3),
        };
        let second = NotificationChannel {
            id: "slack-ops".to_string(),
            name: "Ops Slack".to_string(),
            method: "slack".to_string(),
            config: r#"{"url":"https://hooks.slack.com/services/a/b/c"}"#.to_string(),
            is_active: false,
            proxy_profile_id: None,
            trigger_mode: Some("scan_complete".to_string()),
            min_savings: None,
            min_findings: None,
        };

        save_notification_channel(&pool, &first)
            .await
            .expect("save first channel");
        save_notification_channel(&pool, &second)
            .await
            .expect("save second channel");

        let channels = list_notification_channels(&pool)
            .await
            .expect("list channels");
        assert_eq!(channels.len(), 2);

        let first_loaded = channels
            .iter()
            .find(|channel| channel.id == "email-finance")
            .expect("email channel exists");
        assert_eq!(
            first_loaded.proxy_profile_id.as_deref(),
            Some("proxy-finance")
        );
        assert_eq!(first_loaded.trigger_mode.as_deref(), Some("waste_only"));
        assert_eq!(first_loaded.min_savings, Some(250.0));
        assert_eq!(first_loaded.min_findings, Some(3));

        delete_notification_channel(&pool, "slack-ops")
            .await
            .expect("delete second channel");
        let remaining = list_notification_channels(&pool)
            .await
            .expect("list remaining channels");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "email-finance");

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn record_cleanup_removes_resource_and_writes_audit_history() {
        let (path, pool) = fresh_db("cleanup").await;

        sqlx::query(
            "INSERT INTO scanned_resources (id, provider, region, resource_type, details, estimated_monthly_cost, scanned_at, action_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("vol-123")
        .bind("AWS")
        .bind("us-east-1")
        .bind("EBS Volume")
        .bind("Unattached")
        .bind(42.5_f64)
        .bind(100_i64)
        .bind("DELETE")
        .execute(&pool)
        .await
        .expect("insert scanned resource");

        record_cleanup(&pool, "vol-123", "EBS Volume", 42.5)
            .await
            .expect("record cleanup");

        let remaining = get_scan_results(&pool).await.expect("load results");
        assert!(remaining.is_empty());

        let stats = get_stats(&pool).await.expect("load stats");
        assert_eq!(stats.cleanup_count, 1);
        assert_eq!(stats.history.len(), 1);

        let logs = get_audit_logs(&pool, None, None, 10, 0)
            .await
            .expect("load audit logs");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action, "CLEANUP");
        assert_eq!(logs[0].target, "vol-123");

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn mark_resource_handled_hides_resource_without_cleanup_history() {
        let (path, pool) = fresh_db("handled").await;

        sqlx::query(
            "INSERT INTO scanned_resources (id, provider, region, resource_type, details, estimated_monthly_cost, scanned_at, action_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("disk-1")
        .bind("GCP")
        .bind("us-central1")
        .bind("Persistent Disk")
        .bind("unused")
        .bind(15.0_f64)
        .bind(100_i64)
        .bind("DELETE")
        .execute(&pool)
        .await
        .expect("insert scanned resource");

        mark_resource_handled(&pool, "disk-1", "GCP", Some("owner accepted".to_string()))
            .await
            .expect("mark handled");

        let handled = get_handled_resources(&pool)
            .await
            .expect("get handled resources");
        assert_eq!(handled, vec!["disk-1".to_string()]);
        assert!(get_scan_results(&pool)
            .await
            .expect("load results")
            .is_empty());

        let clean_row = sqlx::query("SELECT COUNT(*) as count FROM cleanup_history")
            .fetch_one(&pool)
            .await
            .expect("count cleanup history");
        let cleanup_count: i64 = clean_row.try_get("count").unwrap_or(0);
        assert_eq!(cleanup_count, 0);

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn delete_scan_history_removes_target_row_only() {
        let (path, pool) = fresh_db("delete-history").await;
        let meta = serde_json::json!({"source":"test"});
        let item = WastedResource {
            id: "i-1".to_string(),
            provider: "AWS".to_string(),
            region: "us-east-1".to_string(),
            resource_type: "EC2".to_string(),
            details: "idle".to_string(),
            estimated_monthly_cost: 21.0,
            action_type: "DELETE".to_string(),
        };

        let id1 = save_scan_history(&pool, 21.0, 1, std::slice::from_ref(&item), &meta)
            .await
            .expect("save first history");
        let id2 = save_scan_history(&pool, 21.0, 1, &[item], &meta)
            .await
            .expect("save second history");
        assert!(id1 != id2);

        delete_scan_history(&pool, id1)
            .await
            .expect("delete first history");
        let rows = get_scan_history(&pool).await.expect("load history");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id2);

        drop(pool);
        let _ = std::fs::remove_file(path);
    }
}
