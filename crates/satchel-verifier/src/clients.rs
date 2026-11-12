//! What the verifier reads: the Block Index and Replication Table (#66) and
//! the storage nodes' extents (#67).

use std::time::Duration;

use satchel_common::cluster::{BlockEntry, NodeAddr, Volume};
use satchel_common::BlockHash;
use serde::Deserialize;

/// One extent a node holds, from `GET /extents` (#67).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Extent {
    pub volume: u64,
    pub kind: ExtentKind,
    /// Fragment index, 0 to 8. Absent for a replica.
    #[serde(default)]
    pub index: Option<u32>,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtentKind {
    Replica,
    Fragment,
}

/// One row of `GET /blocks?volume=` (#66).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ListedBlock {
    pub hash: BlockHash,
    #[serde(flatten)]
    pub entry: BlockEntry,
}

/// Why a read failed.
#[derive(Debug)]
pub enum ReadError {
    /// 404: the node has no such extent.
    Missing,
    /// Any other status, which includes the node's own checksum failures (5xx).
    Status(u16),
    /// Unreachable or timed out.
    Unreachable(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => f.write_str("extent not found"),
            Self::Status(s) => write!(f, "HTTP {s}"),
            Self::Unreachable(why) => write!(f, "unreachable: {why}"),
        }
    }
}

#[derive(Clone)]
pub struct Clients {
    http: reqwest::Client,
    index_url: String,
}

impl Clients {
    pub fn new(index_url: &str, timeout: Duration) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .expect("HTTP client builds"),
            index_url: index_url.trim_end_matches('/').to_owned(),
        }
    }

    async fn json<T: serde::de::DeserializeOwned>(&self, url: String) -> Result<T, String> {
        let r = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("{url}: {e}"))?;
        if !r.status().is_success() {
            return Err(format!("{url}: HTTP {}", r.status()));
        }
        r.json().await.map_err(|e| format!("{url}: {e}"))
    }

    pub async fn volumes(&self) -> Result<Vec<Volume>, String> {
        self.json(format!("{}/volumes", self.index_url)).await
    }

    pub async fn blocks(&self, volume: u64) -> Result<Vec<ListedBlock>, String> {
        self.json(format!("{}/blocks?volume={volume}", self.index_url))
            .await
    }

    pub async fn extents(&self, node: &NodeAddr) -> Result<Vec<Extent>, String> {
        self.json(format!("{}/extents", node.url)).await
    }

    async fn bytes(&self, url: String) -> Result<axum::body::Bytes, ReadError> {
        let r = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ReadError::Unreachable(e.to_string()))?;
        match r.status().as_u16() {
            200 => r
                .bytes()
                .await
                .map_err(|e| ReadError::Unreachable(e.to_string())),
            404 => Err(ReadError::Missing),
            s => Err(ReadError::Status(s)),
        }
    }

    /// One block from a replica extent. The node checks its inline hash first.
    pub async fn read_block(
        &self,
        node: &NodeAddr,
        entry: &BlockEntry,
    ) -> Result<axum::body::Bytes, ReadError> {
        self.bytes(format!(
            "{}/extents/{}?offset={}&length={}",
            node.url, entry.volume, entry.offset, entry.length
        ))
        .await
    }

    /// A whole fragment. The node checks its checksum first.
    pub async fn read_fragment(
        &self,
        node: &NodeAddr,
        volume: u64,
        index: u32,
    ) -> Result<axum::body::Bytes, ReadError> {
        self.bytes(format!("{}/extents/{volume}/fragments/{index}", node.url))
            .await
    }
}
