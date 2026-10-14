//! Satchel Block Index. Maps each block hash to the storage nodes holding it.
//!
//! Magic Pocket counterpart: Block Index.

use satchel_common::service::{self, ServiceConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    service::init_tracing();
    let config = ServiceConfig::from_env("satchel-index", 8081)?;
    let app = service::health_router(&config);
    service::serve(&config, app).await
}
