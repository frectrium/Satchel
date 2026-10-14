//! Satchel storage node. Stores block data (or erasure-coded fragments) on disk.
//!
//! Magic Pocket counterpart: OSD (Object Storage Device).

use satchel_common::service::{self, ServiceConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    service::init_tracing();
    let config = ServiceConfig::from_env("satchel-node", 8090)?;
    let app = service::health_router(&config);
    service::serve(&config, app).await
}
