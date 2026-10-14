//! Satchel frontend. Accepts put, get and delete requests for blocks.
//!
//! Magic Pocket counterpart: Frontends.

use satchel_common::service::{self, ServiceConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    service::init_tracing();
    let config = ServiceConfig::from_env("satchel-api", 8080)?;
    let app = service::health_router(&config);
    service::serve(&config, app).await
}
