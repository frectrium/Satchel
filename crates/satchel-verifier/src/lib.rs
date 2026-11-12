//! Satchel verifier (#72): runs the Disk Scrubber and Metadata Scanner on a
//! loop and reports what it finds.
//!
//! Magic Pocket counterpart: Pocket Watch [B3]. Decision Log: DDL-012.

pub mod clients;
pub mod scan;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use axum::{extract::State, routing::get, routing::post, Json, Router};
use satchel_common::cluster::NodeAddr;
use serde::Serialize;
use tokio::sync::Mutex;

use clients::Clients;
use scan::{Check, Pass};

#[derive(Debug, Clone)]
pub struct Config {
    pub index_url: String,
    pub nodes: Vec<NodeAddr>,
    /// Limit on every call to the index or a node.
    pub timeout: Duration,
    /// Pause between passes.
    pub every: Duration,
}

impl Config {
    /// Reads `SATCHEL_INDEX_URL`, `SATCHEL_NODES`, `SATCHEL_TIMEOUT_MS`
    /// (default 5,000) and `SATCHEL_VERIFY_EVERY_SECS` (default 60).
    pub fn from_env() -> anyhow::Result<Self> {
        let env = |key: &str| std::env::var(key).ok();
        let num = |key: &str, default: u64| -> anyhow::Result<u64> {
            Ok(match env(key) {
                Some(v) => v.parse()?,
                None => default,
            })
        };
        Ok(Self {
            index_url: env("SATCHEL_INDEX_URL").unwrap_or_else(|| "http://localhost:8081".into()),
            nodes: satchel_common::cluster::parse_nodes(&env("SATCHEL_NODES").unwrap_or_default())
                .map_err(anyhow::Error::msg)?,
            timeout: Duration::from_millis(num("SATCHEL_TIMEOUT_MS", 5_000)?),
            every: Duration::from_secs(num("SATCHEL_VERIFY_EVERY_SECS", 60)?),
        })
    }
}

/// What `GET /findings` returns.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub passes: u64,
    /// Findings since the verifier started, by check. The same numbers
    /// `satchel_verifier_findings_total{check}` exports (#21).
    pub findings_total: BTreeMap<&'static str, u64>,
    pub last_pass: Option<Pass>,
}

pub struct Verifier {
    config: Config,
    clients: Clients,
    report: Mutex<Report>,
}

impl Verifier {
    pub fn new(config: Config) -> Arc<Self> {
        Arc::new(Self {
            clients: Clients::new(&config.index_url, config.timeout),
            config,
            report: Mutex::new(Report {
                findings_total: [Check::Scrubber, Check::Metadata]
                    .into_iter()
                    .map(|c| (c.as_str(), 0))
                    .collect(),
                ..Report::default()
            }),
        })
    }

    /// Runs one pass, logs each finding and adds it to the totals.
    pub async fn run_once(&self) -> Pass {
        let pass = scan::run_pass(&self.clients, &self.config.nodes).await;
        for f in &pass.findings {
            tracing::warn!(
                check = f.check.as_str(),
                hash = f.hash.map(|h| h.to_string()),
                volume = f.volume,
                node = %f.node,
                fragment = f.fragment,
                "{}",
                f.problem
            );
        }
        for why in &pass.skipped {
            tracing::info!("verifier skipped: {why}");
        }
        let mut report = self.report.lock().await;
        report.passes += 1;
        for f in &pass.findings {
            *report.findings_total.entry(f.check.as_str()).or_default() += 1;
        }
        report.last_pass = Some(pass.clone());
        pass
    }

    /// Runs passes forever, `every` apart.
    pub async fn run_forever(self: Arc<Self>) {
        loop {
            self.run_once().await;
            tokio::time::sleep(self.config.every).await;
        }
    }

    pub async fn report(&self) -> Report {
        self.report.lock().await.clone()
    }
}

/// `GET /findings` for the latest results, and `POST /verify` to run a pass
/// now, which the failure tests and experiment E4 use instead of waiting.
pub fn router(verifier: Arc<Verifier>) -> Router {
    Router::new()
        .route(
            "/findings",
            get(|State(v): State<Arc<Verifier>>| async move { Json(v.report().await) }),
        )
        .route(
            "/verify",
            post(|State(v): State<Arc<Verifier>>| async move { Json(v.run_once().await) }),
        )
        .with_state(verifier)
}
