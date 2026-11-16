//! Client for the Satchel cluster: block puts, gets and deletes through the
//! frontend, and bytes stored per storage node.

use std::time::Duration;

use axum::body::Bytes;
use satchel_common::cluster::{BlockEntry, NodeAddr, Volume};
use satchel_common::BlockHash;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct SatchelClient {
    http: reqwest::Client,
    api_url: String,
    index_url: String,
    nodes: Vec<NodeAddr>,
}

/// Bytes on one storage node, from its `GET /extents`.
#[derive(Debug, Clone, Serialize)]
pub struct NodeUsage {
    pub id: String,
    pub zone: String,
    pub up: bool,
    pub bytes: u64,
    pub replicated_bytes: u64,
    pub coded_bytes: u64,
    pub extents: u64,
}

#[derive(Deserialize)]
struct Extent {
    kind: String,
    size: u64,
}

/// All block locations and volume layouts from one index request.
#[derive(Debug, Clone, Deserialize)]
pub struct Inventory {
    pub volumes: Vec<Volume>,
    pub blocks: Vec<LocatedBlock>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LocatedBlock {
    pub hash: BlockHash,
    #[serde(flatten)]
    pub entry: BlockEntry,
}

impl SatchelClient {
    pub fn new(api_url: &str, index_url: &str, nodes: Vec<NodeAddr>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("HTTP client builds"),
            api_url: api_url.trim_end_matches('/').to_owned(),
            index_url: index_url.trim_end_matches('/').to_owned(),
            nodes,
        }
    }

    pub fn nodes(&self) -> &[NodeAddr] {
        &self.nodes
    }

    /// Stores one block. Returns `true` if Satchel wrote it, `false` if it
    /// already had it (dedup, DDL-002). Retries while no volume is open,
    /// which happens for a moment each time the Master closes a full one.
    pub async fn put_block(&self, hash: &str, bytes: Bytes) -> anyhow::Result<bool> {
        let url = format!("{}/blocks/{hash}", self.api_url);
        let mut last = String::new();
        for attempt in 0..40u64 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis((100 * attempt).min(1_000))).await;
            }
            match self.http.put(&url).body(bytes.clone()).send().await {
                Ok(r) if r.status().as_u16() == 201 => return Ok(true),
                Ok(r) if r.status().as_u16() == 200 => return Ok(false),
                Ok(r) if r.status().is_server_error() => {
                    last = format!(
                        "HTTP {}: {}",
                        r.status(),
                        r.text().await.unwrap_or_default()
                    );
                }
                Ok(r) => {
                    anyhow::bail!(
                        "put {hash}: HTTP {}: {}",
                        r.status(),
                        r.text().await.unwrap_or_default()
                    )
                }
                Err(e) => last = e.to_string(),
            }
        }
        anyhow::bail!("put {hash} kept failing: {last}")
    }

    pub async fn get_block(&self, hash: &str) -> anyhow::Result<Bytes> {
        let r = self
            .http
            .get(format!("{}/blocks/{hash}", self.api_url))
            .send()
            .await?;
        if !r.status().is_success() {
            anyhow::bail!("get {hash}: HTTP {}", r.status());
        }
        Ok(r.bytes().await?)
    }

    /// Reads the storage inventory in one request, avoiding a lookup for each
    /// file block when the website explains the current layout.
    pub async fn inventory(&self) -> anyhow::Result<Inventory> {
        let response = self
            .http
            .get(format!("{}/inventory", self.index_url))
            .send()
            .await?
            .error_for_status()?;
        Ok(response.json().await?)
    }

    /// Drops a block's index entry. The bytes stay on disk (DDL-013).
    pub async fn delete_block(&self, hash: &str) -> anyhow::Result<()> {
        let r = self
            .http
            .delete(format!("{}/blocks/{hash}", self.api_url))
            .send()
            .await?;
        match r.status().as_u16() {
            204 | 404 => Ok(()),
            s => anyhow::bail!("delete {hash}: HTTP {s}"),
        }
    }

    /// Raw bytes on every storage node: replicas and fragments, so the real
    /// cost of the replication factor.
    pub async fn node_usage(&self) -> Vec<NodeUsage> {
        let mut out = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let listed = async {
                let r = self
                    .http
                    .get(format!("{}/extents", node.url))
                    .timeout(Duration::from_secs(2))
                    .send()
                    .await
                    .ok()?;
                r.json::<Vec<Extent>>().await.ok()
            }
            .await;
            out.push(match listed {
                Some(extents) => NodeUsage {
                    id: node.id.clone(),
                    zone: node.zone.clone(),
                    up: true,
                    bytes: extents.iter().map(|e| e.size).sum(),
                    replicated_bytes: extents
                        .iter()
                        .filter(|e| e.kind == "replica")
                        .map(|e| e.size)
                        .sum(),
                    coded_bytes: extents
                        .iter()
                        .filter(|e| e.kind == "fragment")
                        .map(|e| e.size)
                        .sum(),
                    extents: extents.len() as u64,
                },
                None => NodeUsage {
                    id: node.id.clone(),
                    zone: node.zone.clone(),
                    up: false,
                    bytes: 0,
                    replicated_bytes: 0,
                    coded_bytes: 0,
                    extents: 0,
                },
            });
        }
        out
    }
}
