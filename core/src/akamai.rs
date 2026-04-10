use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;

use crate::linode::LinodeScanner;
use crate::models::WastedResource;
use crate::traits::CloudProvider;

pub struct AkamaiScanner {
    client: Client,
    token: String,
    inner: LinodeScanner,
}

impl AkamaiScanner {
    pub fn new(token: &str) -> Self {
        let trimmed = token.trim().to_string();
        Self {
            client: Client::new(),
            token: trimmed.clone(),
            inner: LinodeScanner::new(&trimmed),
        }
    }

    fn normalize_provider(mut items: Vec<WastedResource>) -> Vec<WastedResource> {
        for item in &mut items {
            item.provider = "Akamai".to_string();
            item.resource_type = item.resource_type.replace("Linode", "Akamai");
        }
        items
    }

    pub async fn check_auth(&self) -> Result<()> {
        if self.token.is_empty() {
            return Err(anyhow!("Akamai API token is required"));
        }

        let response = self
            .client
            .get("https://api.linode.com/v4/profile")
            .bearer_auth(&self.token)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Akamai authentication failed (status {})",
                response.status().as_u16()
            ));
        }

        Ok(())
    }

    pub async fn scan_instances(&self) -> Result<Vec<WastedResource>> {
        self.inner
            .scan_instances()
            .await
            .map(Self::normalize_provider)
    }

    pub async fn scan_volumes(&self) -> Result<Vec<WastedResource>> {
        self.inner
            .scan_volumes()
            .await
            .map(Self::normalize_provider)
    }

    pub async fn scan_ips(&self) -> Result<Vec<WastedResource>> {
        self.inner.scan_ips().await.map(Self::normalize_provider)
    }

    pub async fn scan_nodebalancers(&self) -> Result<Vec<WastedResource>> {
        self.inner
            .scan_nodebalancers()
            .await
            .map(Self::normalize_provider)
    }

    pub async fn scan_snapshots(&self) -> Result<Vec<WastedResource>> {
        self.inner
            .scan_snapshots()
            .await
            .map(Self::normalize_provider)
    }

    pub async fn scan_oversized_instances(&self) -> Result<Vec<WastedResource>> {
        self.inner
            .scan_oversized_instances()
            .await
            .map(Self::normalize_provider)
    }

    pub async fn scan_buckets(&self) -> Result<Vec<WastedResource>> {
        self.inner
            .scan_buckets()
            .await
            .map(Self::normalize_provider)
    }
}

#[async_trait]
impl CloudProvider for AkamaiScanner {
    async fn scan(&self) -> Result<Vec<WastedResource>> {
        let mut results = Vec::new();

        if let Ok(items) = self.scan_instances().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_oversized_instances().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_volumes().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_ips().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_nodebalancers().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_snapshots().await {
            results.extend(items);
        }
        if let Ok(items) = self.scan_buckets().await {
            results.extend(items);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item() -> WastedResource {
        WastedResource {
            id: "linode-1".to_string(),
            provider: "Linode".to_string(),
            region: "us-east".to_string(),
            resource_type: "Linode Instance".to_string(),
            details: "idle".to_string(),
            estimated_monthly_cost: 12.5,
            action_type: "DELETE".to_string(),
        }
    }

    #[test]
    fn normalize_provider_rewrites_branding_consistently() {
        let items = AkamaiScanner::normalize_provider(vec![sample_item()]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].provider, "Akamai");
        assert_eq!(items[0].resource_type, "Akamai Instance");
    }

    #[tokio::test]
    async fn check_auth_requires_non_empty_token() {
        let scanner = AkamaiScanner::new("   ");
        let err = scanner
            .check_auth()
            .await
            .expect_err("empty token must fail");
        assert!(err.to_string().contains("API token is required"));
    }
}
