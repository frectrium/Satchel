//! Satchel verifier. Background process that checks stored blocks are intact.
//!
//! Magic Pocket counterpart: Pocket Watch.

use satchel_common::service::{self, ServiceConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    service::init_tracing();
    let config = ServiceConfig::from_env("satchel-verifier", 8085)?;
    let app = service::health_router(&config);
    service::serve(&config, app).await
}
