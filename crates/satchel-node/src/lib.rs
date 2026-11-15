//! Durable, append-only storage for Satchel replicas and fragments.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context};
use axum::body::Bytes;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, HeaderMap, HeaderName, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use satchel_common::metrics;
use satchel_common::service::{self, ServiceConfig};
use satchel_common::BlockHash;
use serde::{Deserialize, Serialize};

const RECORD_MAGIC: [u8; 4] = *b"SBLK";
const RECORD_HEADER_LEN: u64 = 4 + 32 + 8;
const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
const BLOCK_HEADER: HeaderName = HeaderName::from_static("x-satchel-block");
const CHECKSUM_HEADER: HeaderName = HeaderName::from_static("x-satchel-checksum");

#[derive(Clone)]
pub struct NodeStore {
    inner: Arc<Inner>,
}

struct Inner {
    dir: PathBuf,
    replicas: Mutex<BTreeMap<u64, Vec<BlockRecord>>>,
    append_locks: Mutex<BTreeMap<u64, Arc<Mutex<()>>>>,
    stored_bytes: AtomicU64,
}

#[derive(Clone)]
struct BlockRecord {
    offset: u64,
    length: u64,
    hash: [u8; 32],
    data_offset: u64,
}

#[derive(Debug, Deserialize)]
struct RangeQuery {
    offset: u64,
    length: u64,
}

#[derive(Debug, Deserialize)]
struct OptionalRangeQuery {
    offset: Option<u64>,
    length: Option<u64>,
}

#[derive(Debug, Serialize)]
struct AppendResponse {
    offset: u64,
    length: u64,
}

#[derive(Debug, Serialize)]
struct ExtentInfo {
    volume: u64,
    kind: &'static str,
    index: Option<u8>,
    size: u64,
}

impl NodeStore {
    /// Opens the store and reconstructs its in-memory lookup index from disk.
    pub fn open_from_env() -> anyhow::Result<Self> {
        let dir = std::env::var("SATCHEL_DATA_DIR").unwrap_or_else(|_| "/data".into());
        Self::open(dir)
    }

    pub fn open(dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)
            .with_context(|| format!("create node data directory {}", dir.display()))?;

        let replicas = scan_replicas(&dir)?;
        let stored_bytes = stored_bytes(&dir)?;
        metrics::node_stored_bytes().set(i64::try_from(stored_bytes).unwrap_or(i64::MAX));

        Ok(Self {
            inner: Arc::new(Inner {
                dir,
                replicas: Mutex::new(replicas),
                append_locks: Mutex::default(),
                stored_bytes: AtomicU64::new(stored_bytes),
            }),
        })
    }

    fn append_block(
        &self,
        volume: u64,
        expected_hash: [u8; 32],
        bytes: &[u8],
    ) -> anyhow::Result<u64> {
        let actual_hash = sha256(bytes);
        if actual_hash != expected_hash {
            bail!("block checksum does not match x-satchel-block");
        }

        let append_lock = self
            .inner
            .append_locks
            .lock()
            .expect("append lock index poisoned")
            .entry(volume)
            .or_default()
            .clone();
        let _writer = append_lock.lock().expect("append lock poisoned");

        let offset = self
            .inner
            .replicas
            .lock()
            .expect("replica index lock poisoned")
            .get(&volume)
            .and_then(|entries| entries.last())
            .map_or(0, |record| record.offset + record.length);
        let path = replica_path(&self.inner.dir, volume);
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("open replica extent {}", path.display()))?;
        let start = file.seek(SeekFrom::End(0))?;
        let data_offset = start + RECORD_HEADER_LEN;

        let written = (|| -> std::io::Result<()> {
            file.write_all(&RECORD_MAGIC)?;
            file.write_all(&expected_hash)?;
            file.write_all(&(bytes.len() as u64).to_le_bytes())?;
            file.write_all(bytes)?;
            file.sync_data()
        })();
        if let Err(error) = written {
            // Cut the partial record off, so the next append starts on a
            // record boundary and the next restart still parses the extent.
            let _ = file.set_len(start).and_then(|()| file.sync_data());
            return Err(error).context("append block");
        }

        self.inner
            .replicas
            .lock()
            .expect("replica index lock poisoned")
            .entry(volume)
            .or_default()
            .push(BlockRecord {
                offset,
                length: bytes.len() as u64,
                hash: expected_hash,
                data_offset,
            });
        self.adjust_stored_bytes(0, bytes.len() as u64);
        Ok(offset)
    }

    fn read_replica(&self, volume: u64, offset: u64, length: u64) -> anyhow::Result<Vec<u8>> {
        // Copy this volume's records and let go of the node-wide lock before
        // any file I/O, so reads and appends don't queue behind each other.
        let entries = {
            let map = self
                .inner
                .replicas
                .lock()
                .expect("replica index lock poisoned");
            map.get(&volume)
                .cloned()
                .context("unknown replica volume")?
        };
        let end = offset.checked_add(length).context("range overflow")?;
        let extent_len = entries
            .last()
            .map_or(0, |record| record.offset + record.length);
        if end > extent_len {
            bail!("range is outside replica extent");
        }

        let mut output = Vec::with_capacity(length as usize);
        let mut file = File::open(replica_path(&self.inner.dir, volume))?;
        for record in &entries {
            let record_end = record.offset + record.length;
            let start = offset.max(record.offset);
            let finish = end.min(record_end);
            if start >= finish {
                continue;
            }
            let relative_start = start - record.offset;
            let piece_len = finish - start;
            let mut piece = vec![0; piece_len as usize];
            file.seek(SeekFrom::Start(record.data_offset + relative_start))?;
            file.read_exact(&mut piece)?;
            if start == record.offset && finish == record_end && sha256(&piece) != record.hash {
                bail!("stored replica block checksum mismatch");
            }
            output.extend_from_slice(&piece);
        }
        if output.len() != length as usize {
            bail!("replica extent is malformed");
        }
        Ok(output)
    }

    fn write_fragment(
        &self,
        volume: u64,
        fragment: u8,
        expected_checksum: [u8; 32],
        bytes: &[u8],
    ) -> anyhow::Result<()> {
        if sha256(bytes) != expected_checksum {
            bail!("fragment checksum does not match x-satchel-checksum");
        }
        let final_path = fragment_path(&self.inner.dir, volume, fragment);
        let previous_len = fs::metadata(&final_path)
            .map(|metadata| metadata.len().saturating_sub(32))
            .unwrap_or(0);
        let temporary_path = final_path.with_extension("tmp");
        let mut file = File::create(&temporary_path)?;
        file.write_all(&expected_checksum)?;
        file.write_all(bytes)?;
        file.sync_all().context("fsync fragment")?;
        fs::rename(&temporary_path, &final_path)?;
        self.adjust_stored_bytes(previous_len, bytes.len() as u64);
        Ok(())
    }

    fn read_fragment(
        &self,
        volume: u64,
        fragment: u8,
        range: Option<RangeQuery>,
    ) -> anyhow::Result<Vec<u8>> {
        let path = fragment_path(&self.inner.dir, volume, fragment);
        let mut file = File::open(&path).context("unknown fragment")?;
        let mut checksum = [0; 32];
        file.read_exact(&mut checksum)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if sha256(&bytes) != checksum {
            bail!("stored fragment checksum mismatch");
        }
        match range {
            None => Ok(bytes),
            Some(range) => {
                let end = range
                    .offset
                    .checked_add(range.length)
                    .context("range overflow")?;
                if end > bytes.len() as u64 {
                    bail!("range is outside fragment");
                }
                Ok(bytes[range.offset as usize..end as usize].to_vec())
            }
        }
    }

    fn delete_replica(&self, volume: u64) -> anyhow::Result<bool> {
        let append_lock = self
            .inner
            .append_locks
            .lock()
            .expect("append lock index poisoned")
            .entry(volume)
            .or_default()
            .clone();
        let _writer = append_lock.lock().expect("append lock poisoned");
        let path = replica_path(&self.inner.dir, volume);
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        }
        let removed = self
            .inner
            .replicas
            .lock()
            .expect("replica index lock poisoned")
            .remove(&volume)
            .unwrap_or_default()
            .iter()
            .map(|record| record.length)
            .sum();
        self.adjust_stored_bytes(removed, 0);
        Ok(true)
    }

    fn delete_fragment(&self, volume: u64, fragment: u8) -> anyhow::Result<bool> {
        let path = fragment_path(&self.inner.dir, volume, fragment);
        let removed = match fs::metadata(&path) {
            Ok(metadata) => metadata.len().saturating_sub(32),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        }
        self.adjust_stored_bytes(removed, 0);
        Ok(true)
    }

    fn list_extents(&self) -> anyhow::Result<Vec<ExtentInfo>> {
        let mut result = Vec::new();
        for entry in fs::read_dir(&self.inner.dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let metadata = entry.metadata()?;
            if let Some(volume) = name
                .strip_suffix(".replica")
                .and_then(|value| value.parse().ok())
            {
                result.push(ExtentInfo {
                    volume,
                    kind: "replica",
                    index: None,
                    size: logical_replica_size(&entry.path())?,
                });
            } else if let Some((volume, index)) = parse_fragment_name(&name) {
                result.push(ExtentInfo {
                    volume,
                    kind: "fragment",
                    index: Some(index),
                    size: metadata.len().saturating_sub(32),
                });
            }
        }
        result.sort_by_key(|extent| (extent.volume, extent.kind, extent.index));
        Ok(result)
    }

    fn adjust_stored_bytes(&self, removed: u64, added: u64) {
        let mut current = self.inner.stored_bytes.load(Ordering::Relaxed);
        loop {
            let next = current.saturating_sub(removed).saturating_add(added);
            match self.inner.stored_bytes.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    metrics::node_stored_bytes().set(i64::try_from(next).unwrap_or(i64::MAX));
                    return;
                }
                Err(actual) => current = actual,
            }
        }
    }
}

pub fn router(config: &ServiceConfig, store: NodeStore) -> Router {
    Router::<NodeStore>::new()
        .route("/extents", get(list_extents))
        .route(
            "/extents/{volume}",
            post(append_block).get(read_replica).delete(delete_replica),
        )
        .route(
            "/extents/{volume}/fragments/{fragment}",
            put(write_fragment)
                .get(read_fragment)
                .delete(delete_fragment),
        )
        .with_state(store)
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_BYTES))
        .merge(service::health_router(config))
}

/// Runs a store call on Tokio's blocking pool. File I/O and fsync would
/// otherwise stall every other request on the same async worker.
async fn blocking<T: Send + 'static>(
    store: &NodeStore,
    f: impl FnOnce(&NodeStore) -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    let store = store.clone();
    tokio::task::spawn_blocking(move || f(&store))
        .await
        .map_err(|e| anyhow::anyhow!("storage task failed: {e}"))?
}

async fn append_block(
    State(store): State<NodeStore>,
    AxumPath(volume): AxumPath<u64>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let hash = match parse_hash(&headers, &BLOCK_HEADER) {
        Ok(value) => value,
        Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
    };
    let length = body.len() as u64;
    match blocking(&store, move |s| s.append_block(volume, hash, &body)).await {
        Ok(offset) => {
            (StatusCode::CREATED, Json(AppendResponse { offset, length })).into_response()
        }
        Err(error) if error.to_string().contains("checksum") => {
            (StatusCode::BAD_REQUEST, error.to_string()).into_response()
        }
        Err(error) => internal_error(error),
    }
}

async fn read_replica(
    State(store): State<NodeStore>,
    AxumPath(volume): AxumPath<u64>,
    Query(range): Query<RangeQuery>,
) -> Response {
    let (offset, length) = (range.offset, range.length);
    match blocking(&store, move |s| s.read_replica(volume, offset, length)).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response(),
        Err(error) if error.to_string().contains("unknown replica") => {
            StatusCode::NOT_FOUND.into_response()
        }
        Err(error) if error.to_string().contains("range") => {
            StatusCode::RANGE_NOT_SATISFIABLE.into_response()
        }
        Err(error) => internal_error(error),
    }
}

async fn write_fragment(
    State(store): State<NodeStore>,
    AxumPath((volume, fragment)): AxumPath<(u64, u8)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let checksum = match parse_hash(&headers, &CHECKSUM_HEADER) {
        Ok(value) => value,
        Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
    };
    match blocking(&store, move |s| {
        s.write_fragment(volume, fragment, checksum, &body)
    })
    .await
    {
        Ok(()) => StatusCode::CREATED.into_response(),
        Err(error) if error.to_string().contains("checksum") => {
            (StatusCode::BAD_REQUEST, error.to_string()).into_response()
        }
        Err(error) => internal_error(error),
    }
}

async fn read_fragment(
    State(store): State<NodeStore>,
    AxumPath((volume, fragment)): AxumPath<(u64, u8)>,
    Query(query): Query<OptionalRangeQuery>,
) -> Response {
    let range = match (query.offset, query.length) {
        (None, None) => None,
        (Some(offset), Some(length)) => Some(RangeQuery { offset, length }),
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    match blocking(&store, move |s| s.read_fragment(volume, fragment, range)).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response(),
        Err(error) if error.to_string().contains("unknown fragment") => {
            StatusCode::NOT_FOUND.into_response()
        }
        Err(error) if error.to_string().contains("range") => {
            StatusCode::RANGE_NOT_SATISFIABLE.into_response()
        }
        Err(error) => internal_error(error),
    }
}

async fn delete_replica(
    State(store): State<NodeStore>,
    AxumPath(volume): AxumPath<u64>,
) -> Response {
    match blocking(&store, move |s| s.delete_replica(volume)).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => internal_error(error),
    }
}

async fn delete_fragment(
    State(store): State<NodeStore>,
    AxumPath((volume, fragment)): AxumPath<(u64, u8)>,
) -> Response {
    match blocking(&store, move |s| s.delete_fragment(volume, fragment)).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => internal_error(error),
    }
}

async fn list_extents(State(store): State<NodeStore>) -> Response {
    match blocking(&store, |s| s.list_extents()).await {
        Ok(extents) => Json(extents).into_response(),
        Err(error) => internal_error(error),
    }
}

fn parse_hash(headers: &HeaderMap, name: &HeaderName) -> Result<[u8; 32], String> {
    let Some(value) = headers.get(name).and_then(|value| value.to_str().ok()) else {
        return Err(format!("missing {name}"));
    };
    let hash: BlockHash = value.parse().map_err(|_| format!("invalid {name}"))?;
    let decoded = hex::decode(hash.to_string()).expect("BlockHash always formats as hex");
    Ok(decoded.try_into().expect("SHA-256 is 32 bytes"))
}

fn internal_error(error: anyhow::Error) -> Response {
    tracing::error!(error = ?error, "node storage request failed");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

fn replica_path(dir: &Path, volume: u64) -> PathBuf {
    dir.join(format!("{volume}.replica"))
}

fn fragment_path(dir: &Path, volume: u64, fragment: u8) -> PathBuf {
    dir.join(format!("{volume}.fragment-{fragment}"))
}

fn parse_fragment_name(name: &str) -> Option<(u64, u8)> {
    let (volume, index) = name.split_once(".fragment-")?;
    Some((volume.parse().ok()?, index.parse().ok()?))
}

fn scan_replicas(dir: &Path) -> anyhow::Result<BTreeMap<u64, Vec<BlockRecord>>> {
    let mut replicas = BTreeMap::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(volume) = name
            .strip_suffix(".replica")
            .and_then(|value| value.parse().ok())
        else {
            continue;
        };
        let path = entry.path();
        let mut file = File::open(&path)?;
        let len = file.metadata()?.len();
        let mut logical_offset = 0;
        let mut physical_offset = 0;
        let mut records = Vec::new();
        // A record is acknowledged only after its fsync, so a record cut
        // short at the end was never acknowledged and is safe to drop.
        let mut torn = None;
        while physical_offset < len {
            if physical_offset + RECORD_HEADER_LEN > len {
                torn = Some("a partial record header");
                break;
            }
            let mut magic = [0; 4];
            let mut hash = [0; 32];
            let mut length = [0; 8];
            file.read_exact(&mut magic)?;
            file.read_exact(&mut hash)?;
            file.read_exact(&mut length)?;
            if magic != RECORD_MAGIC {
                torn = Some("bytes that are not a record");
                break;
            }
            let length = u64::from_le_bytes(length);
            let data_offset = physical_offset + RECORD_HEADER_LEN;
            let Some(next_offset) = data_offset.checked_add(length).filter(|n| *n <= len) else {
                torn = Some("a record cut short");
                break;
            };
            file.seek(SeekFrom::Start(next_offset))?;
            records.push(BlockRecord {
                offset: logical_offset,
                length,
                hash,
                data_offset,
            });
            logical_offset += length;
            physical_offset = next_offset;
        }
        if let Some(what) = torn {
            tracing::warn!(
                volume,
                dropped_bytes = len - physical_offset,
                "replica extent ends in {what}; cutting it back to the last whole record"
            );
            let file = OpenOptions::new().write(true).open(&path)?;
            file.set_len(physical_offset)?;
            file.sync_all()?;
        }
        replicas.insert(volume, records);
    }
    Ok(replicas)
}

fn logical_replica_size(path: &Path) -> anyhow::Result<u64> {
    let mut file = File::open(path)?;
    let mut total = 0;
    loop {
        let mut magic = [0; 4];
        match file.read_exact(&mut magic) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        }
        if magic != RECORD_MAGIC {
            bail!("invalid replica record magic in {}", path.display());
        }
        let mut ignored_hash = [0; 32];
        let mut length = [0; 8];
        file.read_exact(&mut ignored_hash)?;
        file.read_exact(&mut length)?;
        let length = u64::from_le_bytes(length);
        file.seek(SeekFrom::Current(
            i64::try_from(length).context("replica length too large")?,
        ))?;
        total += length;
    }
    Ok(total)
}

fn stored_bytes(dir: &Path) -> anyhow::Result<u64> {
    fs::read_dir(dir)?.try_fold(0u64, |total, entry| {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".replica") {
            Ok(total + logical_replica_size(&entry.path())?)
        } else if parse_fragment_name(&name).is_some() {
            Ok(total + entry.metadata()?.len().saturating_sub(32))
        } else {
            Ok(total)
        }
    })
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let hash = BlockHash::of(bytes);
    hex::decode(hash.to_string())
        .expect("BlockHash always formats as hex")
        .try_into()
        .expect("SHA-256 is 32 bytes")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use axum::body::Body;
    use tower::ServiceExt;

    use super::*;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let nonce = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "satchel-node-test-{}-{nanos}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn test_router(dir: &Path) -> Router {
        let config = ServiceConfig {
            name: "node-test",
            addr: "127.0.0.1:0".parse().unwrap(),
            zone: None,
        };
        router(&config, NodeStore::open(dir).unwrap())
    }

    #[tokio::test]
    async fn block_and_fragment_round_trip() {
        let dir = TestDir::new();
        let app = test_router(&dir.0);
        let block = b"block bytes";
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::post("/extents/7")
                    .header(BLOCK_HEADER, BlockHash::of(block).to_string())
                    .body(Body::from(block.as_slice()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::get("/extents/7?offset=0&length=11")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            axum::body::to_bytes(response.into_body(), MAX_BODY_BYTES)
                .await
                .unwrap()
                .as_ref(),
            block
        );

        let fragment = b"fragment";
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::put("/extents/7/fragments/2")
                    .header(CHECKSUM_HEADER, BlockHash::of(fragment).to_string())
                    .body(Body::from(fragment.as_slice()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let response = app
            .oneshot(
                axum::http::Request::get("/extents/7/fragments/2?offset=1&length=4")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            axum::body::to_bytes(response.into_body(), MAX_BODY_BYTES)
                .await
                .unwrap()
                .as_ref(),
            b"ragm"
        );
    }

    #[test]
    fn records_survive_restart() {
        let dir = TestDir::new();
        let bytes = b"persist me";
        let hash = sha256(bytes);
        NodeStore::open(&dir.0)
            .unwrap()
            .append_block(3, hash, bytes)
            .unwrap();
        let reopened = NodeStore::open(&dir.0).unwrap();
        assert_eq!(
            reopened.read_replica(3, 0, bytes.len() as u64).unwrap(),
            bytes
        );
    }

    #[test]
    fn a_torn_last_record_is_cut_off_on_restart() {
        let dir = TestDir::new();
        let (first, second) = (b"first block".as_slice(), b"second block".as_slice());
        let store = NodeStore::open(&dir.0).unwrap();
        store.append_block(4, sha256(first), first).unwrap();
        store.append_block(4, sha256(second), second).unwrap();
        drop(store);
        let path = replica_path(&dir.0, 4);
        let whole = fs::metadata(&path).unwrap().len();
        let one_record = RECORD_HEADER_LEN + first.len() as u64;

        for torn_len in [whole - 3, one_record + 10, one_record + 1] {
            // A crash in the middle of the second append.
            OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(torn_len)
                .unwrap();
            let reopened = NodeStore::open(&dir.0).unwrap();
            assert_eq!(
                reopened.read_replica(4, 0, first.len() as u64).unwrap(),
                first
            );
            assert!(reopened
                .read_replica(4, 0, (first.len() + 1) as u64)
                .is_err());
            assert_eq!(
                fs::metadata(&path).unwrap().len(),
                one_record,
                "cut at {torn_len}"
            );
            // Appends continue on a record boundary and survive another restart.
            reopened.append_block(4, sha256(second), second).unwrap();
            drop(reopened);
            let again = NodeStore::open(&dir.0).unwrap();
            assert_eq!(
                again
                    .read_replica(4, first.len() as u64, second.len() as u64)
                    .unwrap(),
                second
            );
        }
    }

    #[test]
    fn garbage_after_the_last_record_is_cut_off_on_restart() {
        let dir = TestDir::new();
        let block = b"kept".as_slice();
        NodeStore::open(&dir.0)
            .unwrap()
            .append_block(5, sha256(block), block)
            .unwrap();
        let path = replica_path(&dir.0, 5);
        let good = fs::metadata(&path).unwrap().len();
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&[0xAB; 100]).unwrap();
        drop(file);
        let reopened = NodeStore::open(&dir.0).unwrap();
        assert_eq!(
            reopened.read_replica(5, 0, block.len() as u64).unwrap(),
            block
        );
        assert_eq!(fs::metadata(&path).unwrap().len(), good);
    }

    #[test]
    fn corrupt_exact_block_is_not_returned() {
        let dir = TestDir::new();
        let bytes = b"corruption";
        let store = NodeStore::open(&dir.0).unwrap();
        store.append_block(4, sha256(bytes), bytes).unwrap();
        let mut file = OpenOptions::new()
            .write(true)
            .open(replica_path(&dir.0, 4))
            .unwrap();
        file.seek(SeekFrom::Start(RECORD_HEADER_LEN)).unwrap();
        file.write_all(b"X").unwrap();
        file.sync_all().unwrap();
        assert!(store.read_replica(4, 0, bytes.len() as u64).is_err());
    }

    #[test]
    fn append_does_not_rescan_extents_to_refresh_the_metric() {
        let dir = TestDir::new();
        let store = NodeStore::open(&dir.0).unwrap();
        fs::write(replica_path(&dir.0, 99), b"not a replica record").unwrap();

        let bytes = b"new block";
        store.append_block(4, sha256(bytes), bytes).unwrap();
    }
}
