//! Boilerplate every Satchel service shares: logging, config from the
//! environment, a `/health` endpoint and graceful shutdown.

use std::net::SocketAddr;

use anyhow::Context;
use axum::{routing::get, Json, Router};
use serde::Serialize;
use tracing_subscriber::EnvFilter;

/// Runtime settings read from `SATCHEL_*` environment variables.
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub name: &'static str,
    pub addr: SocketAddr,
    /// Availability zone this instance runs in (e.g. `z1`), if any.
    pub zone: Option<String>,
}

impl ServiceConfig {
    /// Reads `SATCHEL_PORT` (falling back to `default_port`) and `SATCHEL_ZONE`.
    pub fn from_env(name: &'static str, default_port: u16) -> anyhow::Result<Self> {
        let port = match std::env::var("SATCHEL_PORT") {
            Ok(p) => p.parse().context("SATCHEL_PORT must be a port number")?,
            Err(_) => default_port,
        };
        Ok(Self {
            name,
            addr: SocketAddr::from(([0, 0, 0, 0], port)),
            zone: std::env::var("SATCHEL_ZONE").ok(),
        })
    }
}

/// Initialises `tracing` output, filtered by `RUST_LOG` (default `info`).
pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
}

#[derive(Serialize)]
struct Health {
    service: &'static str,
    version: &'static str,
    zone: Option<String>,
    status: &'static str,
}

/// Returns a router exposing `GET /health`. Merge service routes into it.
pub fn health_router(config: &ServiceConfig) -> Router {
    let name = config.name;
    let zone = config.zone.clone();
    Router::new().route(
        "/health",
        get(move || async move {
            Json(Health {
                service: name,
                version: env!("CARGO_PKG_VERSION"),
                zone,
                status: "ok",
            })
        }),
    )
}

/// Binds `config.addr` and serves `app` until Ctrl-C or SIGTERM.
pub async fn serve(config: &ServiceConfig, app: Router) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(config.addr)
        .await
        .with_context(|| format!("binding {}", config.addr))?;
    tracing::info!(service = config.name, addr = %config.addr, zone = ?config.zone, "listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
