//! Satchel files service: files on top of the block store or S3 (#36, #63).
//!
//!   satchel-files                  serve, configured by SATCHEL_* variables
//!   satchel-files hash-password    read a password on stdin, print its Argon2id hash
//!
//! `SATCHEL_BACKEND=satchel` (default) stores files in a Satchel cluster;
//! `SATCHEL_BACKEND=s3` stores them in `SATCHEL_S3_BUCKET` and needs
//! `SATCHEL_ADMINS` (`name:hash`, several separated by spaces) for the login.

use std::io::BufRead;
use std::path::PathBuf;

use anyhow::Context;
use satchel_common::service::{self, ServiceConfig};
use satchel_files::{
    auth::{self, Auth},
    manifest::Manifest,
    s3::S3Store,
    satchel::SatchelClient,
    AppState, Config, Limits,
};

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
    if std::env::args().nth(1).as_deref() == Some("hash-password") {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        let password = line.trim_end_matches(['\r', '\n']);
        anyhow::ensure!(
            password.len() >= 12,
            "use a password of at least 12 characters"
        );
        println!("{}", auth::hash_password(password));
        return Ok(());
    }

    service::init_tracing();
    let config = ServiceConfig::from_env("satchel-files", satchel_files::DEFAULT_PORT)?;
    let allowed_origins = std::env::var("SATCHEL_ALLOWED_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(|o| o.trim().trim_end_matches('/').to_owned())
        .filter(|o| !o.is_empty())
        .collect();
    let files_config = Config {
        allowed_origins,
        limits: Limits {
            max_file_bytes: env_u64("SATCHEL_MAX_FILE_BYTES")?,
            max_total_bytes: env_u64("SATCHEL_MAX_TOTAL_BYTES")?,
        },
    };

    let state = match std::env::var("SATCHEL_BACKEND").as_deref() {
        Ok("s3") => {
            let bucket = std::env::var("SATCHEL_S3_BUCKET").context("SATCHEL_S3_BUCKET")?;
            let region = std::env::var("SATCHEL_S3_REGION").unwrap_or_else(|_| "ap-south-1".into());
            let endpoint = std::env::var("SATCHEL_S3_ENDPOINT").ok();
            let admins = std::env::var("SATCHEL_ADMINS").context("SATCHEL_ADMINS")?;
            let secure = std::env::var("SATCHEL_INSECURE_COOKIE").is_err();
            tracing::info!(%bucket, %region, "files config: S3 backend");
            AppState::s3(
                S3Store::new(&bucket, &region, endpoint.as_deref())?,
                Auth::parse(&admins, secure)?,
                files_config,
            )
        }
        Ok("satchel") | Err(_) => {
            let api_url =
                std::env::var("SATCHEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
            let nodes = satchel_common::cluster::parse_nodes(
                &std::env::var("SATCHEL_NODES").unwrap_or_default(),
            )
            .map_err(anyhow::Error::msg)?;
            let dir =
                PathBuf::from(std::env::var("SATCHEL_DATA_DIR").unwrap_or_else(|_| "/data".into()));
            tracing::info!(%api_url, nodes = nodes.len(), dir = %dir.display(), "files config: Satchel backend");
            AppState::new(
                SatchelClient::new(&api_url, nodes),
                Manifest::open(&dir.join("manifest.db"))?,
                files_config,
            )
        }
        Ok(other) => anyhow::bail!("SATCHEL_BACKEND must be satchel or s3, not {other}"),
    };
    let app = service::health_router(&config).merge(satchel_files::router(state));
    service::serve(&config, app).await
}
