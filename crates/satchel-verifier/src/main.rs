//! Satchel verifier. Background process that checks stored blocks are intact.
//!
//! Magic Pocket counterpart: Pocket Watch.

use satchel_common::service::{self, ServiceConfig};
use satchel_verifier::master::{Master, MasterConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    service::init_tracing();
    let config = ServiceConfig::from_env("satchel-verifier", 8085)?;
    let app = service::health_router(&config);
    let master = Master::new(MasterConfig::from_env()?);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            if let Err(error) = master.reconcile().await {
                tracing::warn!(%error, "master reconciliation failed");
            }
        }
    });
    service::serve(&config, app).await
}
