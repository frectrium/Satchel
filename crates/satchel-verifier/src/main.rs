//! Satchel verifier. Background process that checks stored blocks are intact.
//!
//! Magic Pocket counterpart: Pocket Watch.

use satchel_common::service::{self, ServiceConfig};

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
    let verifier = satchel_verifier::Verifier::new(verify);
    tokio::spawn(verifier.clone().run_forever());
    let app = service::health_router(&config).merge(satchel_verifier::router(verifier));
    service::serve(&config, app).await
}
