//! Satchel Block Index. Maps each block hash to its volume, and each volume
//! to the storage nodes holding it.
//!
//! Magic Pocket counterpart: Block Index and Replication Table.

use std::path::PathBuf;

use anyhow::Context;
use satchel_common::service::{self, ServiceConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    service::init_tracing();
    let config = ServiceConfig::from_env("satchel-index", 8081)?;
    let dir = PathBuf::from(std::env::var("SATCHEL_DATA_DIR").unwrap_or_else(|_| "/data".into()));
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join("index.sqlite3");
    let store =
        satchel_index::Store::open(&path).with_context(|| format!("opening {}", path.display()))?;
    tracing::info!(db = %path.display(), "index database open");
    let app = service::health_router(&config).merge(satchel_index::router(store));
    service::serve(&config, app).await
}
