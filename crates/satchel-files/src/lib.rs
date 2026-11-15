//! Satchel files service: whole files on top of Satchel's block store, the
//! API the website and the local install talk to.
//!
//! A file is split into blocks of up to 4 MiB (DDL-001), each stored by its
//! SHA-256 through the frontend, so identical blocks are stored once
//! (DDL-002). The manifest ([`manifest`]) keeps each file's block list.
//! Dropbox splits files into blocks the same way and keeps file metadata in
//! separate systems such as FileJournal [B2]. Here that metadata is a small
//! SQLite database next to the service, a deliberate deviation (DDL-020).
//!
//! Routes, all under `/v1`: `GET /info`, `GET /stats`, `GET /files`,
//! `POST /files?name=`, `GET /files/{id}`, `GET /files/{id}/content` and
//! `DELETE /files/{id}`.

pub mod manifest;
pub mod satchel;

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use futures_util::StreamExt;
use satchel_common::BlockHash;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tower_http::cors::{AllowOrigin, CorsLayer};

use manifest::{FileRecord, Manifest};
use satchel::{NodeUsage, SatchelClient};

/// Largest block, 4 MiB (DDL-001).
pub const BLOCK_BYTES: usize = 4 * 1024 * 1024;

/// Copies of each block while its volume is open (DDL-006). The local
/// install keeps volumes replicated, since RS(6,3) needs 9 nodes (DDL-007).
pub const REPLICAS: u32 = 3;

/// Port of the local install's files API, also what the website probes.
pub const DEFAULT_PORT: u16 = 47800;

#[derive(Debug, Clone, Default)]
pub struct Limits {
    pub max_file_bytes: Option<u64>,
    pub max_total_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Origins allowed to call the API from a browser, besides any
    /// `http://localhost` or `http://127.0.0.1` page.
    pub allowed_origins: Vec<String>,
    pub limits: Limits,
}

pub struct AppState {
    client: SatchelClient,
    manifest: Manifest,
    config: Config,
}

impl AppState {
    pub fn new(client: SatchelClient, manifest: Manifest, config: Config) -> Arc<Self> {
        Arc::new(Self {
            client,
            manifest,
            config,
        })
    }
}

/// The files API, with CORS for the website.
pub fn router(state: Arc<AppState>) -> Router {
    let cors = cors(&state.config.allowed_origins);
    Router::new()
        .route("/v1/info", get(info))
        .route("/v1/stats", get(stats))
        .route("/v1/files", get(list).post(upload))
        .route("/v1/files/{id}", get(file).delete(remove))
        .route("/v1/files/{id}/content", get(content))
        .layer(DefaultBodyLimit::disable())
        .with_state(state)
        .layer(cors)
}

/// Browsers ask before a public page may call a local address (Private
/// Network Access), so preflights also allow that.
fn cors(allowed: &[String]) -> CorsLayer {
    let allowed: Vec<String> = allowed.to_vec();
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin: &HeaderValue, _| {
            let Ok(origin) = origin.to_str() else {
                return false;
            };
            local_origin(origin) || allowed.iter().any(|a| a == origin)
        }))
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE])
        .expose_headers([header::CONTENT_DISPOSITION])
        .allow_private_network(true)
}

fn local_origin(origin: &str) -> bool {
    ["http://localhost", "http://127.0.0.1"]
        .iter()
        .any(|base| origin == *base || origin.starts_with(&format!("{base}:")))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    #[derive(Serialize)]
    struct Body {
        error: String,
    }
    (
        status,
        Json(Body {
            error: message.into(),
        }),
    )
        .into_response()
}

fn internal(e: anyhow::Error) -> Response {
    tracing::error!(error = %e, "files request failed");
    error(StatusCode::SERVICE_UNAVAILABLE, format!("{e:#}"))
}

/// A file name safe to store and to send back in a header: the last path
/// part, without control characters or quotes, at most 255 characters.
pub fn clean_name(raw: Option<&str>) -> String {
    let base = raw
        .unwrap_or("")
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .take(255)
        .collect::<String>();
    let base = base.trim();
    if base.is_empty() || base == "." || base == ".." {
        "untitled".to_owned()
    } else {
        base.to_owned()
    }
}

#[derive(Serialize)]
struct Info {
    service: &'static str,
    version: &'static str,
    backend: &'static str,
    block_bytes: usize,
    replicas: u32,
    nodes: Vec<NodeInfo>,
    max_file_bytes: Option<u64>,
    max_total_bytes: Option<u64>,
}

#[derive(Serialize)]
struct NodeInfo {
    id: String,
    zone: String,
}

async fn info(State(state): State<Arc<AppState>>) -> Response {
    Json(Info {
        service: "satchel-files",
        version: env!("CARGO_PKG_VERSION"),
        backend: "satchel",
        block_bytes: BLOCK_BYTES,
        replicas: REPLICAS,
        nodes: state
            .client
            .nodes()
            .iter()
            .map(|n| NodeInfo {
                id: n.id.clone(),
                zone: n.zone.clone(),
            })
            .collect(),
        max_file_bytes: state.config.limits.max_file_bytes,
        max_total_bytes: state.config.limits.max_total_bytes,
    })
    .into_response()
}

#[derive(Serialize)]
pub struct Stats {
    pub files: u64,
    pub logical_bytes: u64,
    pub unique_blocks: u64,
    pub unique_bytes: u64,
    /// Bytes not stored again because a block was already there.
    pub dedup_saved_bytes: u64,
    pub replicas: u32,
    /// What the storage nodes hold on disk, summed.
    pub raw_bytes: u64,
    pub nodes: Vec<NodeUsage>,
}

async fn stats(State(state): State<Arc<AppState>>) -> Response {
    let totals = match state.manifest.totals() {
        Ok(t) => t,
        Err(e) => return internal(e),
    };
    let nodes = state.client.node_usage().await;
    Json(Stats {
        files: totals.files,
        logical_bytes: totals.logical_bytes,
        unique_blocks: totals.unique_blocks,
        unique_bytes: totals.unique_bytes,
        dedup_saved_bytes: totals.logical_bytes.saturating_sub(totals.unique_bytes),
        replicas: REPLICAS,
        raw_bytes: nodes.iter().map(|n| n.bytes).sum(),
        nodes,
    })
    .into_response()
}

async fn list(State(state): State<Arc<AppState>>) -> Response {
    match state.manifest.list() {
        Ok(files) => Json(files).into_response(),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
struct UploadQuery {
    name: Option<String>,
}

/// What one upload did, for the website's live counters.
#[derive(Serialize)]
pub struct UploadReport {
    pub file: FileRecord,
    pub blocks: u64,
    /// Blocks Satchel had to write. The rest were already stored.
    pub new_blocks: u64,
    pub dedup_blocks: u64,
    pub new_bytes: u64,
    pub replicas: u32,
    /// New bytes times the replica count: what landed on the nodes.
    pub raw_bytes_written: u64,
    pub millis: u64,
}

async fn upload(
    State(state): State<Arc<AppState>>,
    Query(q): Query<UploadQuery>,
    body: Body,
) -> Response {
    let started = Instant::now();
    let name = clean_name(q.name.as_deref());
    let limits = &state.config.limits;
    let used = match state.manifest.totals() {
        Ok(t) => t.logical_bytes,
        Err(e) => return internal(e),
    };

    let mut stream = body.into_data_stream();
    let mut pending: Vec<u8> = Vec::with_capacity(BLOCK_BYTES);
    let mut whole = Sha256::new();
    let mut size = 0u64;
    let mut blocks: Vec<(String, u64)> = Vec::new();
    let (mut new_blocks, mut new_bytes) = (0u64, 0u64);

    let store = |block: Vec<u8>| {
        let state = state.clone();
        async move {
            let hash = BlockHash::of(&block).to_string();
            let len = block.len() as u64;
            let written = state.client.put_block(&hash, Bytes::from(block)).await?;
            anyhow::Ok((hash, len, written))
        }
    };

    loop {
        let chunk = match stream.next().await {
            Some(Ok(c)) => c,
            Some(Err(e)) => {
                return error(StatusCode::BAD_REQUEST, format!("upload broke off: {e}"))
            }
            None => break,
        };
        size += chunk.len() as u64;
        if limits.max_file_bytes.is_some_and(|max| size > max) {
            return error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "the file is over the size limit",
            );
        }
        if limits.max_total_bytes.is_some_and(|max| used + size > max) {
            return error(StatusCode::INSUFFICIENT_STORAGE, "storage is full");
        }
        whole.update(&chunk);
        let mut rest: &[u8] = &chunk;
        while !rest.is_empty() {
            let take = (BLOCK_BYTES - pending.len()).min(rest.len());
            pending.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if pending.len() == BLOCK_BYTES {
                let block = std::mem::replace(&mut pending, Vec::with_capacity(BLOCK_BYTES));
                match store(block).await {
                    Ok((hash, len, written)) => {
                        if written {
                            new_blocks += 1;
                            new_bytes += len;
                        }
                        blocks.push((hash, len));
                    }
                    Err(e) => return internal(e),
                }
            }
        }
    }
    if !pending.is_empty() {
        match store(pending).await {
            Ok((hash, len, written)) => {
                if written {
                    new_blocks += 1;
                    new_bytes += len;
                }
                blocks.push((hash, len));
            }
            Err(e) => return internal(e),
        }
    }

    let sha256 = hex::encode(whole.finalize());
    let file = match state
        .manifest
        .add_file(&name, size, &sha256, now_secs(), &blocks)
    {
        Ok(f) => f,
        Err(e) => return internal(e),
    };
    let report = UploadReport {
        blocks: blocks.len() as u64,
        new_blocks,
        dedup_blocks: blocks.len() as u64 - new_blocks,
        new_bytes,
        replicas: REPLICAS,
        raw_bytes_written: new_bytes * u64::from(REPLICAS),
        millis: started.elapsed().as_millis() as u64,
        file,
    };
    (StatusCode::CREATED, Json(report)).into_response()
}

#[derive(Serialize)]
struct FileDetail {
    #[serde(flatten)]
    file: FileRecord,
    block_list: Vec<manifest::BlockRef>,
}

async fn file(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    match state.manifest.get(id) {
        Ok(Some((file, block_list))) => Json(FileDetail { file, block_list }).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "no such file"),
        Err(e) => internal(e),
    }
}

/// Streams the file back, block by block, each checked against its hash by
/// the frontend before it leaves Satchel.
async fn content(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let (file, blocks) = match state.manifest.get(id) {
        Ok(Some(found)) => found,
        Ok(None) => return error(StatusCode::NOT_FOUND, "no such file"),
        Err(e) => return internal(e),
    };
    let client = state.client.clone();
    let stream = futures_util::stream::iter(blocks).then(move |b| {
        let client = client.clone();
        async move {
            client
                .get_block(&b.hash)
                .await
                .map_err(|e| std::io::Error::other(format!("block {}: {e}", b.index)))
        }
    });
    let disposition = format!("attachment; filename=\"{}\"", file.name);
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, file.size)
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposition)
                .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
        )
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "bad response"))
}

#[derive(Serialize)]
struct Removed {
    id: i64,
    /// Blocks no other file used, now dropped from Satchel's index.
    released_blocks: u64,
}

async fn remove(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let orphans = match state.manifest.remove(id) {
        Ok(Some(o)) => o,
        Ok(None) => return error(StatusCode::NOT_FOUND, "no such file"),
        Err(e) => return internal(e),
    };
    for hash in &orphans {
        if let Err(e) = state.client.delete_block(hash).await {
            // The file is gone from the manifest either way. A block left in
            // the index only costs space, like Magic Pocket without
            // compaction (DDL-013).
            tracing::warn!(%hash, "could not drop a released block: {e:#}");
        }
    }
    Json(Removed {
        id,
        released_blocks: orphans.len() as u64,
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_paths_and_quotes() {
        assert_eq!(clean_name(Some("../../etc/passwd")), "passwd");
        assert_eq!(
            clean_name(Some("C:\\x\\report \"v2\".pdf")),
            "report v2.pdf"
        );
        assert_eq!(clean_name(Some("a\nb")), "ab");
        assert_eq!(clean_name(None), "untitled");
        assert_eq!(clean_name(Some("..")), "untitled");
        assert_eq!(clean_name(Some(&"x".repeat(300))).len(), 255);
    }

    #[test]
    fn only_local_pages_and_listed_origins_are_trusted() {
        assert!(local_origin("http://localhost:3000"));
        assert!(local_origin("http://127.0.0.1"));
        assert!(!local_origin("http://localhost.evil.example"));
        assert!(!local_origin("https://example.com"));
    }
}
