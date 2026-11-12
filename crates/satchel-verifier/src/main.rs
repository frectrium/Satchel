//! Satchel verifier. Background process that checks stored blocks are intact.
//!
//! Magic Pocket counterpart: Pocket Watch.

use std::time::Duration;

use satchel_common::service::{self, ServiceConfig};
use satchel_verifier::repair::{self, RepairConfig, Repairer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    service::init_tracing();
    let config = ServiceConfig::from_env("satchel-verifier", 8085)?;
    let verify = satchel_verifier::Config::from_env()?;
    tracing::info!(
        index = %verify.index_url,
        nodes = verify.nodes.len(),
        every_secs = verify.every.as_secs(),
        "verifier config"
    );
    let repair_after = std::env::var("SATCHEL_REPAIR_AFTER").unwrap_or_else(|_| "15m".into());
    let repairer = Repairer::new(
        RepairConfig {
            index_url: verify.index_url.clone(),
            nodes: verify.nodes.clone(),
            timeout: verify.timeout,
            repair_after: repair::parse_duration(&repair_after).map_err(anyhow::Error::msg)?,
            check_every: Duration::from_secs(5),
        },
        // RS(6,3) rebuilds use satchel_common::ec::reconstruct once the codec
        // (#78) is merged. Until then coded volumes can't be repaired.
        None,
    );
    tracing::info!(repair_after, "repair config");
    let verifier = satchel_verifier::Verifier::new(verify);
    tokio::spawn(verifier.clone().run_forever());
    tokio::spawn(repairer.clone().run_forever());
    let app = service::health_router(&config)
        .merge(satchel_verifier::router(verifier))
        .merge(satchel_verifier::repair_router(repairer));
    service::serve(&config, app).await
}
