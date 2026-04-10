use crate::models::WastedResource;
use crate::traits::CloudProvider;
use anyhow::Result;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use reqwest::Client;
use rsa::sha2::Sha256;
use rsa::signature::{SignatureEncoding, Signer};
use rsa::{pkcs1v15::SigningKey, pkcs8::DecodePrivateKey, RsaPrivateKey};
use serde::Deserialize;
use serde_json::Value;

pub struct OracleScanner {
    client: Client,
    tenancy_id: String,
    user_id: String,
    fingerprint: String,
    private_key_pem: String,
    region: String,
}

#[derive(Deserialize)]
struct OracleBucket {
    name: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct OracleBucketDetails {
    name: String,
    #[serde(rename = "approximateSize")]
    approximate_size: Option<u64>,
}

impl OracleScanner {
    pub fn new(tenancy: &str, user: &str, fingerprint: &str, key: &str, region: &str) -> Self {
        Self {
            client: Client::new(),
            tenancy_id: tenancy.to_string(),
            user_id: user.to_string(),
            fingerprint: fingerprint.to_string(),
            private_key_pem: key.to_string(),
            region: region.to_string(),
        }
    }

    fn sign_request(&self, method: &str, path: &str, host: &str, date: &str) -> Result<String> {
        let mut signing_string = String::from("(request-target): ");
        signing_string.push_str(&method.to_lowercase());
        signing_string.push_str(" ");
        signing_string.push_str(path);
        signing_string.push_str("\nhost: ");
        signing_string.push_str(host);
        signing_string.push_str("\ndate: ");
        signing_string.push_str(date);

        let priv_key = RsaPrivateKey::from_pkcs8_pem(&self.private_key_pem)?;
        let signing_key = SigningKey::<Sha256>::new(priv_key);

        let signature = signing_key.sign(signing_string.as_bytes());
        let signature_base64 = STANDARD.encode(signature.to_vec());

        let mut key_id = String::new();
        key_id.push_str(&self.tenancy_id);
        key_id.push_str("/");
        key_id.push_str(&self.user_id);
        key_id.push_str("/");
        key_id.push_str(&self.fingerprint);

        let mut auth_header = String::from("Signature version=\"1\",keyId=\"");
        auth_header.push_str(&key_id);
        auth_header.push_str(
            "\",algorithm=\"rsa-sha256\",headers=\" (request-target) host date\",signature=\"",
        );
        auth_header.push_str(&signature_base64);
        auth_header.push_str("\"");

        Ok(auth_header)
    }

    async fn request(&self, service_prefix: &str, path: &str) -> Result<Value> {
        let mut host = String::from(service_prefix);
        host.push_str(".");
        host.push_str(&self.region);
        host.push_str(".oraclecloud.com");

        let mut url = String::from("https://");
        url.push_str(&host);
        url.push_str(path);

        let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let auth = self.sign_request("GET", path, &host, &date)?;

        let res = self
            .client
            .get(&url)
            .header("date", date)
            .header("authorization", auth)
            .send()
            .await?;

        if !res.status().is_success() {
            let err = res.text().await?;
            return Err(anyhow::anyhow!("OCI Error: {}", err));
        }
        Ok(res.json().await?)
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        let path = format!(
            "/20160918/instances?compartmentId={}&lifecycleState=STOPPED",
            self.tenancy_id
        );
        let mut wastes = Vec::new();
        if let Ok(json) = self.request("iaas", &path).await {
            if let Some(instances) = json.as_array() {
                for i in instances {
                    let name = i["displayName"].as_str().unwrap_or_default().to_string();
                    wastes.push(WastedResource {
                        id: i["id"].as_str().unwrap_or_default().to_string(),
                        provider: "Oracle".to_string(),
                        region: self.region.clone(),
                        resource_type: "Compute Instance".to_string(),
                        details: format!("Stopped: {}", name),
                        estimated_monthly_cost: 0.0,
                        action_type: "DELETE".to_string(),
                    });
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_object_storage(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        // 1. Get Namespace
        if let Ok(ns_val) = self.request("objectstorage", "/n/").await {
            if let Some(ns) = ns_val.as_str() {
                // 2. List Buckets
                let b_path = format!("/n/{}/b?compartmentId={}", ns, self.tenancy_id);
                if let Ok(b_json) = self.request("objectstorage", &b_path).await {
                    if let Some(buckets) = b_json.as_array() {
                        for b_val in buckets {
                            if let Ok(b_base) =
                                serde_json::from_value::<OracleBucket>(b_val.clone())
                            {
                                // 3. Get Bucket Details for Size
                                let d_path = format!("/n/{}/b/{}", ns, b_base.name);
                                if let Ok(d_json) = self.request("objectstorage", &d_path).await {
                                    if let Ok(details) =
                                        serde_json::from_value::<OracleBucketDetails>(d_json)
                                    {
                                        let size_bytes = details.approximate_size.unwrap_or(0);
                                        let size_gb = size_bytes as f64 / 1024.0 / 1024.0 / 1024.0;

                                        if size_bytes == 0 {
                                            wastes.push(WastedResource {
                                                id: b_base.name,
                                                provider: "Oracle".to_string(),
                                                region: self.region.clone(),
                                                resource_type: "Object Storage".to_string(),
                                                details: "Empty Bucket".to_string(),
                                                estimated_monthly_cost: 0.0,
                                                action_type: "DELETE".to_string(),
                                            });
                                        } else if size_gb > 1.0 {
                                            // Assume missing lifecycle if it's large (simplified OCI check)
                                            wastes.push(WastedResource {
                                                id: b_base.name,
                                                provider: "Oracle".to_string(),
                                                region: self.region.clone(),
                                                resource_type: "Object Storage".to_string(),
                                                details: format!(
                                                    "Bucket ({:.1} GB) lacks Lifecycle Policy.",
                                                    size_gb
                                                ),
                                                estimated_monthly_cost: size_gb * 0.02,
                                                action_type: "ARCHIVE".to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(wastes)
    }

    pub async fn scan_boot_volumes(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let path = format!(
            "/20160918/bootVolumes?compartmentId={}&lifecycleState=AVAILABLE",
            self.tenancy_id
        );

        if let Ok(json) = self.request("iaas", &path).await {
            if let Some(volumes) = json.as_array() {
                for volume in volumes {
                    let id = volume["id"].as_str().unwrap_or_default().to_string();
                    let name = volume["displayName"]
                        .as_str()
                        .unwrap_or("Unnamed")
                        .to_string();
                    let state = volume["lifecycleState"].as_str().unwrap_or("");
                    let size_gb = volume["sizeInGBs"].as_f64().unwrap_or_else(|| {
                        volume["sizeInGBs"]
                            .as_i64()
                            .map(|n| n as f64)
                            .unwrap_or(0.0)
                    });

                    if state == "AVAILABLE" {
                        wastes.push(WastedResource {
                            id,
                            provider: "Oracle".to_string(),
                            region: self.region.clone(),
                            resource_type: "Boot Volume".to_string(),
                            details: format!("Unattached boot volume: {}", name),
                            estimated_monthly_cost: (size_gb * 0.025).max(2.0),
                            action_type: "DELETE".to_string(),
                        });
                    }
                }
            }
        }

        Ok(wastes)
    }

    pub async fn scan_block_volumes(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let path = format!(
            "/20160918/volumes?compartmentId={}&lifecycleState=AVAILABLE",
            self.tenancy_id
        );

        if let Ok(json) = self.request("iaas", &path).await {
            if let Some(volumes) = json.as_array() {
                for volume in volumes {
                    let id = volume["id"].as_str().unwrap_or_default().to_string();
                    let name = volume["displayName"]
                        .as_str()
                        .unwrap_or("Unnamed")
                        .to_string();
                    let state = volume["lifecycleState"].as_str().unwrap_or("");
                    let size_gb = volume["sizeInGBs"].as_f64().unwrap_or_else(|| {
                        volume["sizeInGBs"]
                            .as_i64()
                            .map(|n| n as f64)
                            .unwrap_or(0.0)
                    });

                    if state == "AVAILABLE" {
                        wastes.push(WastedResource {
                            id,
                            provider: "Oracle".to_string(),
                            region: self.region.clone(),
                            resource_type: "Block Volume".to_string(),
                            details: format!("Unattached block volume: {}", name),
                            estimated_monthly_cost: (size_gb * 0.025).max(3.0),
                            action_type: "DELETE".to_string(),
                        });
                    }
                }
            }
        }

        Ok(wastes)
    }

    pub async fn scan_load_balancers(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let path = format!("/20170115/loadBalancers?compartmentId={}", self.tenancy_id);

        if let Ok(json) = self.request("loadbalancer", &path).await {
            if let Some(lbs) = json.as_array() {
                for lb in lbs {
                    let id = lb["id"].as_str().unwrap_or_default().to_string();
                    let name = lb["displayName"].as_str().unwrap_or("Unnamed").to_string();
                    let state = lb["lifecycleState"].as_str().unwrap_or("");
                    let backend_sets = lb["backendSets"].as_object().map(|o| o.len()).unwrap_or(0);

                    if state == "ACTIVE" && backend_sets == 0 {
                        wastes.push(WastedResource {
                            id,
                            provider: "Oracle".to_string(),
                            region: self.region.clone(),
                            resource_type: "Load Balancer".to_string(),
                            details: format!("No backend sets: {}", name),
                            estimated_monthly_cost: 20.0,
                            action_type: "DELETE".to_string(),
                        });
                    }
                }
            }
        }

        Ok(wastes)
    }

    pub async fn scan_reserved_ips(&self) -> Result<Vec<WastedResource>> {
        let mut wastes = Vec::new();
        let path = format!(
            "/20160918/publicIps?scope=REGION&lifetime=RESERVED&compartmentId={}",
            self.tenancy_id
        );

        if let Ok(json) = self.request("iaas", &path).await {
            if let Some(ips) = json.as_array() {
                for ip in ips {
                    let id = ip["id"].as_str().unwrap_or_default().to_string();
                    let address = ip["ipAddress"].as_str().unwrap_or("unknown").to_string();
                    let state = ip["lifecycleState"].as_str().unwrap_or("");
                    let assigned = ip["assignedEntityId"].as_str().unwrap_or("");

                    if (state == "AVAILABLE" || state == "UNASSIGNED") && assigned.is_empty() {
                        wastes.push(WastedResource {
                            id,
                            provider: "Oracle".to_string(),
                            region: self.region.clone(),
                            resource_type: "Reserved IP".to_string(),
                            details: format!("Unassigned Reserved IP: {}", address),
                            estimated_monthly_cost: 3.0,
                            action_type: "DELETE".to_string(),
                        });
                    }
                }
            }
        }

        Ok(wastes)
    }
}

#[async_trait]
impl CloudProvider for OracleScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();
        if let Ok(r) = self.scan_instances().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_boot_volumes().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_block_volumes().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_load_balancers().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_reserved_ips().await {
            results.extend(r);
        }
        if let Ok(r) = self.scan_object_storage().await {
            results.extend(r);
        }
        Ok(results)
    }
}
