//! Satchel files service: files on top of the block store (#36).

use std::path::PathBuf;

use satchel_common::service::{self, ServiceConfig};
use satchel_files::{manifest::Manifest, satchel::SatchelClient, AppState, Config, Limits};

fn env_u64(key: &str) -> anyhow::Result<Option<u64>> {
    std::env::var(key)
        .ok()
        .map(|v| {
            v.parse()
                .map_err(|_| anyhow::anyhow!("{key} must be a number"))
        })
        .transpose()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    service::init_tracing();
    let config = ServiceConfig::from_env("satchel-files", satchel_files::DEFAULT_PORT)?;
    let api_url =
        std::env::var("SATCHEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let nodes =
        satchel_common::cluster::parse_nodes(&std::env::var("SATCHEL_NODES").unwrap_or_default())
            .map_err(anyhow::Error::msg)?;
    let dir = PathBuf::from(std::env::var("SATCHEL_DATA_DIR").unwrap_or_else(|_| "/data".into()));
    let manifest = Manifest::open(&dir.join("manifest.db"))?;
    let allowed_origins = std::env::var("SATCHEL_ALLOWED_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(|o| o.trim().trim_end_matches('/').to_owned())
        .filter(|o| !o.is_empty())
        .collect();
    let limits = Limits {
        max_file_bytes: env_u64("SATCHEL_MAX_FILE_BYTES")?,
        max_total_bytes: env_u64("SATCHEL_MAX_TOTAL_BYTES")?,
    };
    tracing::info!(%api_url, nodes = nodes.len(), dir = %dir.display(), "files config");
    let state = AppState::new(
        SatchelClient::new(&api_url, nodes),
        manifest,
        Config {
            allowed_origins,
            limits,
        },
    );
    let app = service::health_router(&config).merge(satchel_files::router(state));
    service::serve(&config, app).await
}
