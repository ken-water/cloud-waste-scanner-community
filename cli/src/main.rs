use aws_config::meta::region::RegionProviderChain;
use aws_sdk_cloudwatch::Client as CwClient;
use aws_sdk_ec2::{config::Region, Client as Ec2Client};
use aws_sdk_elasticloadbalancingv2::Client as ElbClient;
use aws_sdk_rds::Client as RdsClient;
use aws_sdk_s3::Client as S3Client;
use clap::Parser;
use cloud_waste_scanner_core::{Db, Scanner, WastedResource};
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Table};
use directories::ProjectDirs;
use std::env;
use std::sync::{Arc, Mutex};
use tokio::task::JoinSet;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// AWS Region to scan. If omitted, scans ALL enabled regions.
    #[arg(short, long)]
    region: Option<String>,

    /// AWS Profile to use (from ~/.aws/credentials)
    #[arg(short, long)]
    profile: Option<String>,

    /// Show detailed debug logs
    #[arg(short, long)]
    verbose: bool,

    /// Run in mock mode (no AWS credentials required)
    #[arg(short, long)]
    mock: bool,

    /// Show recent scan history
    #[arg(long)]
    history: bool,

    /// Show cumulative statistics
    #[arg(long)]
    stats: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Setup DB
    let db = if let Some(proj_dirs) = ProjectDirs::from("com", "CloudWasteScanner", "scanner") {
        let db_path = proj_dirs.data_local_dir().join("data.db");
        match Db::new(db_path.clone()).await {
            Ok(d) => Some(d),
            Err(e) => {
                if args.history || args.stats {
                    eprintln!(
                        "❌ Error: Database not available at {:?}. Cannot show history/stats.",
                        db_path
                    );
                    return Ok(());
                }
                eprintln!(
                    "⚠️ Warning: Failed to initialize database at {:?}: {}\n",
                    db_path, e
                );
                None
            }
        }
    } else {
        if args.history || args.stats {
            eprintln!("❌ Error: Could not determine data directory.");
            return Ok(());
        }
        eprintln!("⚠️ Warning: Could not determine data directory. History will not be saved.");
        None
    };

    // Handle History Command
    if args.history {
        if let Some(db) = db {
            let history = db.get_recent_scans(10).await?;
            if history.is_empty() {
                println!("No scan history found.");
            } else {
                let mut table = Table::new();
                table
                    .load_preset(UTF8_FULL)
                    .apply_modifier(UTF8_ROUND_CORNERS)
                    .set_header(vec![
                        "Timestamp (UTC)",
                        "Profile",
                        "Region",
                        "Potential Savings",
                    ]);

                for rec in history {
                    table.add_row(vec![
                        rec.timestamp.format("%Y-%m-%d %H:%M").to_string(),
                        rec.profile,
                        rec.region,
                        format!("$ {:.2}", rec.total_cost),
                    ]);
                }
                println!("\n📜 Recent Scan History:\n{}", table);
            }
        }
        return Ok(());
    }

    // Handle Stats Command
    if args.stats {
        if let Some(db) = db {
            let (count, total) = db.get_total_stats().await?;
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_header(vec!["Metric", "Value"]);

            table.add_row(vec!["Total Scans Run", &count.to_string()]);
            table.add_row(vec![
                "Cumulative Waste Detected",
                &format!("$ {:.2}", total),
            ]);

            println!("\n📊 Cumulative Statistics:\n{}", table);
        }
        return Ok(());
    }

    // Handle Profile
    if let Some(ref profile) = args.profile {
        env::set_var("AWS_PROFILE", profile);
    }

    let all_wastes = Arc::new(Mutex::new(Vec::new()));

    if args.mock {
        println!("🧪 Running in MOCK mode. No actual AWS calls will be made.");
        let mut wastes = all_wastes.lock().unwrap();
        wastes.push(WastedResource {
            id: "vol-mock-1".to_string(),
            provider: "AWS".to_string(),
            region: "mock-region".to_string(),
            resource_type: "EBS Volume".to_string(),
            details: "100 GB (gp3)".to_string(),
            estimated_monthly_cost: 8.0,
            action_type: "DELETE".to_string(),
        });
        wastes.push(WastedResource {
            id: "1.2.3.4".to_string(),
            provider: "AWS".to_string(),
            region: "mock-region".to_string(),
            resource_type: "Elastic IP".to_string(),
            details: "Unattached".to_string(),
            estimated_monthly_cost: 3.6,
            action_type: "DELETE".to_string(),
        });
        wastes.push(WastedResource {
            id: "snap-mock-1".to_string(),
            provider: "AWS".to_string(),
            region: "mock-region".to_string(),
            resource_type: "EBS Snapshot".to_string(),
            details: "500 GB, 45 days old".to_string(),
            estimated_monthly_cost: 25.0,
            action_type: "ARCHIVE".to_string(),
        });
        wastes.push(WastedResource {
            id: "arn:aws:elasticloadbalancing:us-east-1:123456789012:loadbalancer/app/mock-lb/50dc6c495c0c9188".to_string(),
            provider: "AWS".to_string(),
            region: "mock-region".to_string(),
            resource_type: "Load Balancer".to_string(),
            details: "No Target Groups attached (mock-lb)".to_string(),
            estimated_monthly_cost: 16.20,
            action_type: "DELETE".to_string(),
        });
        wastes.push(WastedResource {
            id: "rds-snap-manual-2023".to_string(),
            provider: "AWS".to_string(),
            region: "mock-region".to_string(),
            resource_type: "RDS Manual Snapshot".to_string(),
            details: "200 GB, 120 days old".to_string(),
            estimated_monthly_cost: 19.0,
            action_type: "ARCHIVE".to_string(),
        });
        wastes.push(WastedResource {
            id: "/aws/lambda/old-function".to_string(),
            provider: "AWS".to_string(),
            region: "mock-region".to_string(),
            resource_type: "CloudWatch Log Group".to_string(),
            details: "5.50 GB, Never Expire".to_string(),
            estimated_monthly_cost: 0.165,
            action_type: "ARCHIVE".to_string(),
        });
    } else {
        // Initial setup to discover regions
        let region_provider =
            RegionProviderChain::default_provider().or_else(Region::new("us-east-1"));
        let base_config = aws_config::from_env().region(region_provider).load().await;

        let regions_to_scan: Vec<String> = if let Some(ref r) = args.region {
            vec![r.clone()]
        } else {
            println!("🌍 No region specified. Discovering all enabled regions...");
            let client = Ec2Client::new(&base_config);
            match client.describe_regions().send().await {
                Ok(resp) => resp
                    .regions
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|r| r.region_name)
                    .collect(),
                Err(e) => {
                    eprintln!("⚠️ Failed to list regions: {}. Defaulting to us-east-1.", e);
                    vec!["us-east-1".to_string()]
                }
            }
        };

        println!(
            "🚀 Starting scan across {} regions...",
            regions_to_scan.len()
        );

        let mut set = JoinSet::new();

        for region_name in regions_to_scan {
            let wastes_clone = all_wastes.clone();

            set.spawn(async move {
                // Create a client specific for this region
                // Note: We need to override the region in the config
                let config = aws_config::from_env()
                    .region(Region::new(region_name.clone()))
                    .load()
                    .await;

                let ec2_client = Ec2Client::new(&config);
                let elb_client = ElbClient::new(&config);
                let cw_client = CwClient::new(&config);
                let rds_client = RdsClient::new(&config);
                let s3_client = S3Client::new(&config);
                let scanner = Scanner::new(
                    ec2_client,
                    elb_client,
                    cw_client,
                    rds_client,
                    s3_client,
                    region_name.clone(),
                    None,
                    None,
                );

                // We'll collect local results to minimize lock contention
                let mut local_wastes = Vec::new();

                match scanner.scan_ebs_volumes().await {
                    Ok(mut w) => local_wastes.append(&mut w),
                    Err(_) => {} // Silently fail for regions with no access or errors
                }
                match scanner.scan_idle_instances().await {
                    Ok(mut w) => local_wastes.append(&mut w),
                    Err(_) => {}
                }
                match scanner.scan_idle_rds().await {
                    Ok(mut w) => local_wastes.append(&mut w),
                    Err(_) => {}
                }
                match scanner.scan_idle_nat_gateways().await {
                    Ok(mut w) => local_wastes.append(&mut w),
                    Err(_) => {}
                }
                match scanner.scan_old_amis().await {
                    Ok(mut w) => local_wastes.append(&mut w),
                    Err(_) => {}
                }
                match scanner.scan_underutilized_ebs().await {
                    Ok(mut w) => local_wastes.append(&mut w),
                    Err(_) => {}
                }
                match scanner.scan_cloudwatch_logs().await {
                    Ok(mut w) => local_wastes.append(&mut w),
                    Err(_) => {}
                }

                if !local_wastes.is_empty() {
                    let mut lock = wastes_clone.lock().unwrap();
                    lock.append(&mut local_wastes);
                }
            });
        }

        while let Some(res) = set.join_next().await {
            if let Err(e) = res {
                eprintln!("Thread error: {}\n", e);
            }
        }
    }

    let final_wastes = all_wastes.lock().unwrap();

    // SAVE TO DB
    if let Some(db) = db {
        let profile = args.profile.unwrap_or("default".to_string());
        let region = args.region.unwrap_or("all".to_string());

        match db.save_scan(&profile, &region, &final_wastes).await {
            Ok(id) => println!("💾 Saved scan results to history (ID: {}\n)", id),
            Err(e) => eprintln!("⚠️ Failed to save scan results: {}\n", e),
        }
    }

    if final_wastes.is_empty() {
        println!("✅ No wasted resources found! Great job.");
        return Ok(());
    }

    // Sort by cost (descending)
    let mut sorted_wastes: Vec<&WastedResource> = final_wastes.iter().collect();
    sorted_wastes.sort_by(|a, b| {
        b.estimated_monthly_cost
            .partial_cmp(&a.estimated_monthly_cost)
            .unwrap()
    });

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            "Region",
            "Type",
            "ID / ARN",
            "Details",
            "Est. Cost ($/mo)",
        ]);

    let mut total_cost = 0.0;
    let mut cleanup_commands = Vec::new();

    cleanup_commands.push("#!/bin/bash".to_string());
    cleanup_commands.push("# Auto-generated cleanup script. Review carefully!".to_string());
    cleanup_commands.push("set -e".to_string());

    for waste in sorted_wastes {
        table.add_row(vec![
            &waste.region,
            &waste.resource_type,
            &waste.id,
            &waste.details,
            &format!("{:.2}", waste.estimated_monthly_cost),
        ]);
        total_cost += waste.estimated_monthly_cost;

        let cmd = match waste.resource_type.as_str() {
            "EBS Volume" => format!("aws ec2 delete-volume --volume-id {} --region {}", waste.id, waste.region),
            "Elastic IP" => format!("aws ec2 release-address --public-ip {} --region {}", waste.id, waste.region),
            "EBS Snapshot" => format!("aws ec2 delete-snapshot --snapshot-id {} --region {}", waste.id, waste.region),
            "Load Balancer" => format!("aws elbv2 delete-load-balancer --load-balancer-arn {} --region {}", waste.id, waste.region),
            "RDS Instance" => format!("aws rds delete-db-instance --db-instance-identifier {} --skip-final-snapshot --region {}", waste.id, waste.region),
            "RDS Manual Snapshot" => format!("aws rds delete-db-snapshot --db-snapshot-identifier {} --region {}", waste.id, waste.region),
            "NAT Gateway" => format!("aws ec2 delete-nat-gateway --nat-gateway-id {} --region {}", waste.id, waste.region),
            "Old AMI" => format!("aws ec2 deregister-image --image-id {} --region {}", waste.id, waste.region),
            "CloudWatch Log Group" => format!("aws logs put-retention-policy --log-group-name \"{}\" --retention-in-days 30 --region {}", waste.id, waste.region),
            _ => format!("# Unknown: {}", waste.id),
        };
        cleanup_commands.push(cmd);
    }

    println!("\n{}\n", table);
    println!("\n💰 Potential Monthly Savings: ${:.2}\n", total_cost);

    let cleanup_file = "cleanup.sh";
    std::fs::write(cleanup_file, cleanup_commands.join("\n"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(mut perms) = std::fs::metadata(cleanup_file).map(|m| m.permissions()) {
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(cleanup_file, perms);
        }
    }

    println!("\n🧹 Cleanup script: ./{}\n", cleanup_file);

    Ok(())
}
