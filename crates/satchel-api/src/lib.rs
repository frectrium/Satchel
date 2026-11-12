//! Satchel frontend (#68): `PUT`, `GET` and `DELETE /blocks/{hash}`.
//!
//! A put is checked, deduplicated through the Block Index, appended to the
//! 3 replicas of an open volume and then recorded in the index. A get reads
//! one replica, own zone first, and checks the hash before replying.
//!
//! Magic Pocket counterpart: Frontends [B2]. Decision Log: DDL-001,
//! DDL-002, DDL-006, DDL-013.

pub mod clients;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::put,
    Router,
};
use satchel_common::cluster::{BlockEntry, NodeAddr, Volume, VolumeKind};
use satchel_common::BlockHash;

use clients::{http_client, CallError, IndexClient, NodeClient};

/// Largest block, 4 MiB (DDL-001).
pub const MAX_BLOCK: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Config {
    pub index_url: String,
    pub nodes: Vec<NodeAddr>,
    /// This frontend's zone. Reads try replicas here first.
    pub zone: Option<String>,
    /// Limit on every call to the index or a node (TS-API-8).
    pub timeout: Duration,
}

impl Config {
    /// Reads `SATCHEL_INDEX_URL`, `SATCHEL_NODES`, `SATCHEL_ZONE` and
    /// `SATCHEL_TIMEOUT_MS` (default 5,000).
    pub fn from_env() -> anyhow::Result<Self> {
        let env = |key: &str| std::env::var(key).ok();
        let timeout_ms: u64 = match env("SATCHEL_TIMEOUT_MS") {
            Some(ms) => ms.parse()?,
            None => 5_000,
        };
        Ok(Self {
            index_url: env("SATCHEL_INDEX_URL")
                .unwrap_or_else(|| "http://localhost:8081".to_owned())
                .trim_end_matches('/')
                .to_owned(),
            nodes: satchel_common::cluster::parse_nodes(&env("SATCHEL_NODES").unwrap_or_default())
                .map_err(anyhow::Error::msg)?,
            zone: env("SATCHEL_ZONE"),
            timeout: Duration::from_millis(timeout_ms),
        })
    }
}

struct AppState {
    config: Config,
    index: IndexClient,
    nodes: NodeClient,
    /// One writer per volume, so all 3 replicas append blocks in the same
    /// order and return the same offsets (DDL-004).
    volume_locks: Mutex<HashMap<u64, Arc<tokio::sync::Mutex<()>>>>,
    /// Volumes whose replicas may have diverged after a failed write. Not
    /// written to again; the Master closes them.
    broken: Mutex<HashSet<u64>>,
    /// Spreads puts over the open volumes.
    next_volume: AtomicUsize,
}

/// The frontend's routes, without `/health`.
pub fn router(config: Config) -> Router {
    let client = http_client(config.timeout);
    let state = Arc::new(AppState {
        index: IndexClient {
            client: client.clone(),
            url: config.index_url.clone(),
        },
        nodes: NodeClient { client },
        config,
        volume_locks: Mutex::default(),
        broken: Mutex::default(),
        next_volume: AtomicUsize::new(0),
    });
    Router::new()
        .route(
            "/blocks/{hash}",
            put(put_block).get(get_block).delete(delete_block),
        )
        // One byte over the limit, so an oversized body reaches the 413 check.
        .layer(DefaultBodyLimit::max(MAX_BLOCK + 1))
        .with_state(state)
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, message.into()).into_response()
}

fn unavailable(e: CallError) -> Response {
    match e {
        CallError::NotFound => error(StatusCode::SERVICE_UNAVAILABLE, "not found upstream"),
        CallError::Failed(why) => error(StatusCode::SERVICE_UNAVAILABLE, why),
    }
}

fn bad_hash() -> Response {
    error(
        StatusCode::BAD_REQUEST,
        "the hash must be 64 hex characters",
    )
}

async fn put_block(
    State(state): State<Arc<AppState>>,
    Path(raw): Path<String>,
    body: Bytes,
) -> Response {
    let Ok(hash) = raw.parse::<BlockHash>() else {
        return bad_hash();
    };
    if body.is_empty() {
        return error(StatusCode::BAD_REQUEST, "empty blocks are not stored");
    }
    if body.len() > MAX_BLOCK {
        return error(StatusCode::PAYLOAD_TOO_LARGE, "blocks are at most 4 MiB");
    }
    if !hash.verify(&body) {
        return error(StatusCode::BAD_REQUEST, "the body does not hash to the key");
    }

    // Dedup: identical content is stored once (DDL-002).
    match state.index.get_block(&hash).await {
        Ok(_) => return StatusCode::OK.into_response(),
        Err(CallError::NotFound) => {}
        Err(e) => return unavailable(e),
    }

    let volumes = match state.index.open_volumes().await {
        Ok(v) => v,
        Err(e) => return unavailable(e),
    };
    let candidates: Vec<&Volume> = {
        let broken = state.broken.lock().expect("lock");
        volumes
            .iter()
            .filter(|v| v.kind == VolumeKind::Replicated && !broken.contains(&v.id))
            .collect()
    };
    if candidates.is_empty() {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no open volume to write to",
        );
    }

    // Try each open volume once, starting from a rotating position (DDL-006).
    let start = state.next_volume.fetch_add(1, Ordering::Relaxed);
    let mut failures = Vec::new();
    for i in 0..candidates.len() {
        let volume = candidates[(start + i) % candidates.len()];
        match write_replicas(&state, volume, &hash, &body).await {
            Ok(offset) => {
                let entry = BlockEntry {
                    volume: volume.id,
                    offset,
                    length: body.len() as u64,
                };
                return match state.index.put_block(&hash, &entry).await {
                    Ok(true) => StatusCode::CREATED.into_response(),
                    // Another put of the same block won the race.
                    Ok(false) => StatusCode::OK.into_response(),
                    Err(e) => unavailable(e),
                };
            }
            Err(why) => {
                tracing::warn!(volume = volume.id, %hash, "put failed, trying another volume: {why}");
                failures.push(why);
            }
        }
    }
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        format!("no open volume took the write: {}", failures.join("; ")),
    )
}

/// Appends to every replica of `volume`. All must succeed at the same
/// offset, or the volume is marked broken and the put moves on.
async fn write_replicas(
    state: &AppState,
    volume: &Volume,
    hash: &BlockHash,
    body: &Bytes,
) -> Result<u64, String> {
    let replicas: Vec<&NodeAddr> = volume
        .nodes
        .iter()
        .map(|id| {
            state
                .config
                .nodes
                .iter()
                .find(|n| &n.id == id)
                .ok_or_else(|| format!("volume {} uses unknown node {id}", volume.id))
        })
        .collect::<Result<_, _>>()?;
    if replicas.is_empty() {
        return Err(format!("volume {} has no replicas", volume.id));
    }

    let lock = state
        .volume_locks
        .lock()
        .expect("lock")
        .entry(volume.id)
        .or_default()
        .clone();
    let _writer = lock.lock().await;
    if state.broken.lock().expect("lock").contains(&volume.id) {
        return Err(format!("volume {} is broken", volume.id));
    }

    // The lock keeps the order, so the 3 appends can run at once.
    let writes: Vec<_> = replicas
        .iter()
        .map(|&node| {
            let (nodes, node, hash, body, id) = (
                state.nodes.clone(),
                node.clone(),
                *hash,
                body.clone(),
                volume.id,
            );
            tokio::spawn(async move { nodes.append(&node, id, &hash, body).await })
        })
        .collect();
    let mut offsets = Vec::new();
    let mut errors = Vec::new();
    for write in writes {
        match write.await {
            Ok(Ok(offset)) => offsets.push(offset),
            Ok(Err(CallError::Failed(why))) => errors.push(why),
            Ok(Err(CallError::NotFound)) => errors.push("replica extent not found".to_owned()),
            Err(e) => errors.push(format!("write task failed: {e}")),
        }
    }
    let agreed = errors.is_empty() && offsets.windows(2).all(|w| w[0] == w[1]);
    if !agreed {
        state.broken.lock().expect("lock").insert(volume.id);
        if errors.is_empty() {
            errors.push(format!("replicas returned different offsets {offsets:?}"));
        }
        return Err(format!("volume {}: {}", volume.id, errors.join(", ")));
    }
    Ok(offsets[0])
}

async fn get_block(State(state): State<Arc<AppState>>, Path(raw): Path<String>) -> Response {
    let Ok(hash) = raw.parse::<BlockHash>() else {
        return bad_hash();
    };
    let entry = match state.index.get_block(&hash).await {
        Ok(e) => e,
        Err(CallError::NotFound) => return error(StatusCode::NOT_FOUND, "no such block"),
        Err(e) => return unavailable(e),
    };
    let volume = match state.index.get_volume(entry.volume).await {
        Ok(v) => v,
        Err(e) => return unavailable(e),
    };
    if volume.kind == VolumeKind::Rs63 {
        // Coded reads arrive with the erasure coding work (#70).
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "reads from coded volumes are not supported yet",
        );
    }

    // Own zone first, then the others in placement order.
    let mut replicas: Vec<&NodeAddr> = volume
        .nodes
        .iter()
        .filter_map(|id| state.config.nodes.iter().find(|n| &n.id == id))
        .collect();
    if let Some(zone) = &state.config.zone {
        replicas.sort_by_key(|n| &n.zone != zone);
    }
    let mut failures = Vec::new();
    for node in replicas {
        match state
            .nodes
            .read(node, volume.id, entry.offset, entry.length)
            .await
        {
            Ok(bytes) if hash.verify(&bytes) => return (StatusCode::OK, bytes).into_response(),
            Ok(_) => failures.push(format!("{} returned bytes that fail the hash", node.id)),
            Err(CallError::Failed(why)) => failures.push(why),
            Err(CallError::NotFound) => failures.push(format!("{} has no such extent", node.id)),
        }
    }
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        format!("no replica could serve the block: {}", failures.join("; ")),
    )
}

async fn delete_block(State(state): State<Arc<AppState>>, Path(raw): Path<String>) -> Response {
    let Ok(hash) = raw.parse::<BlockHash>() else {
        return bad_hash();
    };
    // The bytes stay on disk; only the index entry goes (DDL-013).
    match state.index.delete_block(&hash).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(CallError::NotFound) => error(StatusCode::NOT_FOUND, "no such block"),
        Err(e) => unavailable(e),
    }
}
