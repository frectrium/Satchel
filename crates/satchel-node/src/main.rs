//! Satchel storage node. Stores block data (or erasure-coded fragments) on disk.
//!
//! Magic Pocket counterpart: OSD (Object Storage Device).

use satchel_common::service::{self, ServiceConfig};
use satchel_node::NodeStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    service::init_tracing();
    let config = ServiceConfig::from_env("satchel-node", 8090)?;
    let store = NodeStore::open_from_env()?;
    let app = satchel_node::router(&config, store);
    service::serve(&config, app).await
}
