use mcp_ads::server::AdsServer;
use mcp_ads::store::AdsStore;
use rmcp::{ServiceExt, transport::stdio};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse().unwrap()),
        )
        .init();
    let store = Arc::new(AdsStore::new());
    let server = AdsServer { store };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
