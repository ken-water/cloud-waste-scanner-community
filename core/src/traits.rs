use crate::models::WastedResource;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait CloudProvider {
    async fn scan(&self) -> Result<Vec<WastedResource>>;
    // async fn clean(&self, resource_id: &str) -> Result<()>;
}
