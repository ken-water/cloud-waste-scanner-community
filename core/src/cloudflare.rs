use crate::models::WastedResource;
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::net::IpAddr;

pub struct CloudflareScanner {
    client: Client,
    api_token: String,
    account_id: String,
}

#[derive(Deserialize)]
struct CfResponse<T> {
    result: Option<T>,
    result_info: Option<CfResultInfo>,
}

#[derive(Deserialize)]
struct CfResultInfo {
    total_pages: Option<u32>,
}

// R2
#[derive(Deserialize)]
struct R2Buckets {
    buckets: Vec<R2Bucket>,
}

#[derive(Deserialize)]
struct R2Bucket {
    name: String,
    creation_date: Option<String>,
}

// DNS
#[derive(Deserialize)]
struct Zone {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct DnsRecord {
    id: String,
    name: Option<String>,
    content: Option<String>,
    #[serde(rename = "type")]
    record_type: Option<String>,
    proxied: Option<bool>,
}

#[derive(Deserialize)]
struct WorkerScript {
    id: String,
    modified_on: Option<String>,
}

#[derive(Deserialize)]
struct WorkerRoute {
    script: Option<String>,
}

#[derive(Deserialize)]
struct Tunnel {
    id: String,
    name: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct PageProject {
    name: String,
    created_on: Option<String>,
    domains: Option<Vec<String>>,
}

impl CloudflareScanner {
    pub fn new(token: &str, account_id: &str) -> Self {
        Self {
            client: Client::new(),
            api_token: token.to_string(),
            account_id: account_id.to_string(),
        }
    }

    async fn fetch_paginated<T: DeserializeOwned>(&self, base_url: &str) -> Result<Vec<T>> {
        let mut page = 1u32;
        let mut all_items = Vec::new();

        loop {
            let delimiter = if base_url.contains('?') { '&' } else { '?' };
            let url = format!("{}{delimiter}page={}&per_page=100", base_url, page);

            let resp = self
                .client
                .get(&url)
                .bearer_auth(&self.api_token)
                .send()
                .await?;
            if !resp.status().is_success() {
                break;
            }

            let payload: CfResponse<Vec<T>> = resp.json().await?;
            if let Some(mut result) = payload.result {
                all_items.append(&mut result);
            }

            let total_pages = payload
                .result_info
                .as_ref()
                .and_then(|info| info.total_pages)
                .unwrap_or(1);

            if total_pages <= page {
                break;
            }

            page += 1;
        }

        Ok(all_items)
    }

    async fn fetch_json(&self, url: &str) -> Result<Option<Value>> {
        let resp = self
            .client
            .get(url)
            .bearer_auth(&self.api_token)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Ok(None);
        }

        let json: Value = resp.json().await?;
        Ok(Some(json))
    }

    fn collect_timestamp_candidates(value: &Value, out: &mut Vec<DateTime<Utc>>) {
        match value {
            Value::String(s) => {
                if let Ok(parsed) = DateTime::parse_from_rfc3339(s) {
                    out.push(parsed.with_timezone(&Utc));
                }
            }
            Value::Array(items) => {
                for item in items {
                    Self::collect_timestamp_candidates(item, out);
                }
            }
            Value::Object(map) => {
                for v in map.values() {
                    Self::collect_timestamp_candidates(v, out);
                }
            }
            _ => {}
        }
    }

    fn latest_timestamp_in_value(value: &Value) -> Option<DateTime<Utc>> {
        let mut timestamps = Vec::new();
        Self::collect_timestamp_candidates(value, &mut timestamps);
        timestamps.into_iter().max()
    }

    fn age_days_from_datetime(dt: DateTime<Utc>) -> i64 {
        (Utc::now() - dt).num_days()
    }

    async fn list_worker_route_usage(&self) -> Result<HashMap<String, usize>> {
        let mut usage = HashMap::new();
        let zones = self
            .fetch_paginated::<Zone>("https://api.cloudflare.com/client/v4/zones")
            .await?;

        for zone in zones {
            let route_url = format!(
                "https://api.cloudflare.com/client/v4/zones/{}/workers/routes",
                zone.id
            );
            let routes = self
                .fetch_paginated::<WorkerRoute>(&route_url)
                .await
                .unwrap_or_default();

            for route in routes {
                if let Some(script) = route.script {
                    if !script.trim().is_empty() {
                        *usage.entry(script).or_insert(0) += 1;
                    }
                }
            }
        }

        Ok(usage)
    }

    fn age_days(timestamp: &str) -> Option<i64> {
        let parsed = DateTime::parse_from_rfc3339(timestamp).ok()?;
        Some((Utc::now() - parsed.with_timezone(&Utc)).num_days())
    }

    fn is_older_than(timestamp: Option<&str>, days: i64) -> bool {
        timestamp
            .and_then(Self::age_days)
            .map(|age| age >= days)
            .unwrap_or(false)
    }

    fn is_private_or_local_ip(value: &str) -> bool {
        match value.parse::<IpAddr>() {
            Ok(IpAddr::V4(v4)) => {
                v4.is_private()
                    || v4.is_loopback()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || (v4.octets()[0] == 100 && (64..=127).contains(&v4.octets()[1]))
            }
            Ok(IpAddr::V6(v6)) => v6.is_loopback() || v6.is_unique_local() || v6.is_unspecified(),
            Err(_) => false,
        }
    }

    async fn get_worker_schedule_count(&self, script_name: &str) -> usize {
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/workers/scripts/{}/schedules",
            self.account_id, script_name
        );

        let json = match self.fetch_json(&url).await {
            Ok(Some(v)) => v,
            _ => return 0,
        };

        if let Some(arr) = json.get("result").and_then(|v| v.as_array()) {
            return arr.len();
        }

        if let Some(arr) = json
            .get("result")
            .and_then(|v| v.get("schedules"))
            .and_then(|v| v.as_array())
        {
            return arr.len();
        }

        0
    }

    async fn get_worker_latest_deployment_age_days(&self, script_name: &str) -> Option<i64> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/workers/scripts/{}/deployments?per_page=20",
            self.account_id, script_name
        );

        let json = self.fetch_json(&url).await.ok().flatten()?;
        let result = json.get("result")?;
        let latest = Self::latest_timestamp_in_value(result)?;
        Some(Self::age_days_from_datetime(latest))
    }

    async fn get_pages_latest_deployment_age_days(&self, project_name: &str) -> Option<i64> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}/deployments?per_page=20",
            self.account_id, project_name
        );

        let json = self.fetch_json(&url).await.ok().flatten()?;
        let result = json.get("result")?;
        let latest = Self::latest_timestamp_in_value(result)?;
        Some(Self::age_days_from_datetime(latest))
    }

    pub async fn scan_r2(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/r2/buckets",
            self.account_id
        );
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_token)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let json: CfResponse<R2Buckets> = resp.json().await?;

        if let Some(data) = json.result {
            for b in data.buckets {
                if Self::is_older_than(b.creation_date.as_deref(), 90) {
                    let age = b
                        .creation_date
                        .as_deref()
                        .and_then(Self::age_days)
                        .map(|d| format!("{} days", d))
                        .unwrap_or_else(|| "unknown age".to_string());

                    wastes.push(WastedResource {
                        id: b.name.clone(),
                        provider: "Cloudflare".to_string(),
                        region: "Global".to_string(),
                        resource_type: "R2 Bucket".to_string(),
                        details: format!("Old R2 bucket without lifecycle usage signal ({})", age),
                        estimated_monthly_cost: 1.0,
                        action_type: "REVIEW".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_workers(&self) -> Result<Vec<WastedResource>> {
        let route_usage = self.list_worker_route_usage().await.unwrap_or_default();
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/workers/scripts",
            self.account_id
        );
        let scripts = self.fetch_paginated::<WorkerScript>(&url).await?;
        let mut wastes = Vec::new();

        for s in scripts {
            let route_count = route_usage.get(&s.id).copied().unwrap_or(0);
            let schedule_count = self.get_worker_schedule_count(&s.id).await;
            let deployment_age = self.get_worker_latest_deployment_age_days(&s.id).await;
            let modified_stale = Self::is_older_than(s.modified_on.as_deref(), 30);
            let deployment_stale = deployment_age.map(|days| days >= 30).unwrap_or(true);

            if route_count == 0 && schedule_count == 0 && modified_stale && deployment_stale {
                let modified_age = s
                    .modified_on
                    .as_deref()
                    .and_then(Self::age_days)
                    .map(|d| format!("{} days", d))
                    .unwrap_or_else(|| "unknown".to_string());

                let deploy_age = deployment_age
                    .map(|d| format!("{} days", d))
                    .unwrap_or_else(|| "none".to_string());

                wastes.push(WastedResource {
                    id: s.id,
                    provider: "Cloudflare".to_string(),
                    region: "Global".to_string(),
                    resource_type: "Worker Script".to_string(),
                    details: format!(
                        "No routes/schedules; stale worker (modified {}, latest deploy {})",
                        modified_age, deploy_age
                    ),
                    estimated_monthly_cost: 0.50,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_tunnels(&self) -> Result<Vec<WastedResource>> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/tunnels",
            self.account_id
        );
        let res = self
            .client
            .get(&url)
            .bearer_auth(&self.api_token)
            .send()
            .await?;
        if !res.status().is_success() {
            return Ok(vec![]);
        }

        let json: CfResponse<Vec<Tunnel>> = res.json().await?;
        let mut wastes = Vec::new();

        if let Some(tunnels) = json.result {
            for t in tunnels {
                let status = t.status.unwrap_or_default();
                if status.eq_ignore_ascii_case("inactive") || status.eq_ignore_ascii_case("down") {
                    wastes.push(WastedResource {
                        id: t.id,
                        provider: "Cloudflare".to_string(),
                        region: "Global".to_string(),
                        resource_type: "Argo Tunnel".to_string(),
                        details: format!("Inactive Tunnel: {}", t.name.unwrap_or_default()),
                        estimated_monthly_cost: 0.0,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_pages(&self) -> Result<Vec<WastedResource>> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects",
            self.account_id
        );
        let projects = self.fetch_paginated::<PageProject>(&url).await?;
        let mut wastes = Vec::new();

        for p in projects {
            let domains_count = p.domains.as_ref().map(|d| d.len()).unwrap_or(0);
            let project_age_days = p.created_on.as_deref().and_then(Self::age_days);
            let latest_deploy_age = self.get_pages_latest_deployment_age_days(&p.name).await;

            let stale_by_deployment = latest_deploy_age.map(|days| days >= 30).unwrap_or(true);
            let stale_by_project = project_age_days.map(|days| days >= 30).unwrap_or(false);

            if domains_count == 0 && stale_by_deployment && stale_by_project {
                let project_age = project_age_days
                    .map(|d| format!("{} days", d))
                    .unwrap_or_else(|| "unknown".to_string());

                let deploy_age = latest_deploy_age
                    .map(|d| format!("{} days", d))
                    .unwrap_or_else(|| "none".to_string());

                wastes.push(WastedResource {
                    id: p.name.clone(),
                    provider: "Cloudflare".to_string(),
                    region: "Global".to_string(),
                    resource_type: "Pages Project".to_string(),
                    details: format!(
                        "No custom domains; stale project (created {}, latest deploy {})",
                        project_age, deploy_age
                    ),
                    estimated_monthly_cost: 0.0,
                    action_type: "DELETE".to_string(),
                });
            }
        }

        Ok(wastes)
    }

    pub async fn scan_dns(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();

        let zones = self
            .fetch_paginated::<Zone>("https://api.cloudflare.com/client/v4/zones")
            .await?;

        for zone in zones {
            let dns_url = format!(
                "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
                zone.id
            );
            let records = self
                .fetch_paginated::<DnsRecord>(&dns_url)
                .await
                .unwrap_or_default();

            for r in records {
                let record_type = r.record_type.unwrap_or_default();
                let proxied = r.proxied.unwrap_or(false);
                let content = r.content.unwrap_or_default();

                if (record_type == "A" || record_type == "AAAA")
                    && !proxied
                    && Self::is_private_or_local_ip(&content)
                {
                    wastes.push(WastedResource {
                        id: r.id,
                        provider: "Cloudflare".to_string(),
                        region: "Global".to_string(),
                        resource_type: "DNS Record".to_string(),
                        details: format!(
                            "Exposed private/local IP ({}) - {} (zone: {})",
                            content,
                            r.name.unwrap_or_default(),
                            zone.name
                        ),
                        estimated_monthly_cost: 0.0,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for CloudflareScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_dns().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_r2().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_tunnels().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_workers().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_pages().await {
            results.extend(r);
        }
        Ok(results)
    }
}
