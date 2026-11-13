//! Durable, append-only storage for Satchel replicas and fragments.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
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

        let mut records = self
            .inner
            .replicas
            .lock()
            .expect("replica index lock poisoned");
        let path = replica_path(&self.inner.dir, volume);
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("open replica extent {}", path.display()))?;
        let data_offset = file.seek(SeekFrom::End(0))? + RECORD_HEADER_LEN;
        let offset = records
            .get(&volume)
            .and_then(|entries| entries.last())
            .map_or(0, |record| record.offset + record.length);

        file.write_all(&RECORD_MAGIC)?;
        file.write_all(&expected_hash)?;
        file.write_all(&(bytes.len() as u64).to_le_bytes())?;
        file.write_all(bytes)?;
        file.sync_data().context("fsync appended block")?;

        records.entry(volume).or_default().push(BlockRecord {
            offset,
            length: bytes.len() as u64,
            hash: expected_hash,
            data_offset,
        });
        self.refresh_metric()?;
        Ok(offset)
    }

    fn read_replica(&self, volume: u64, offset: u64, length: u64) -> anyhow::Result<Vec<u8>> {
        let records = self
            .inner
            .replicas
            .lock()
            .expect("replica index lock poisoned");
        let entries = records.get(&volume).context("unknown replica volume")?;
        let end = offset.checked_add(length).context("range overflow")?;
        let extent_len = entries
            .last()
            .map_or(0, |record| record.offset + record.length);
        if end > extent_len {
            bail!("range is outside replica extent");
        }

        let mut output = Vec::with_capacity(length as usize);
        let mut file = File::open(replica_path(&self.inner.dir, volume))?;
        for record in entries {
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
        let temporary_path = final_path.with_extension("tmp");
        let mut file = File::create(&temporary_path)?;
        file.write_all(&expected_checksum)?;
        file.write_all(bytes)?;
        file.sync_all().context("fsync fragment")?;
        fs::rename(&temporary_path, &final_path)?;
        self.refresh_metric()?;
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
        let path = replica_path(&self.inner.dir, volume);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(path)?;
        self.inner
            .replicas
            .lock()
            .expect("replica index lock poisoned")
            .remove(&volume);
        self.refresh_metric()?;
        Ok(true)
    }

    fn delete_fragment(&self, volume: u64, fragment: u8) -> anyhow::Result<bool> {
        let path = fragment_path(&self.inner.dir, volume, fragment);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(path)?;
        self.refresh_metric()?;
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

    fn refresh_metric(&self) -> anyhow::Result<()> {
        let bytes = stored_bytes(&self.inner.dir)?;
        metrics::node_stored_bytes().set(i64::try_from(bytes).unwrap_or(i64::MAX));
        Ok(())
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

async fn append_block(
    State(store): State<NodeStore>,
    AxumPath(volume): AxumPath<u64>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let hash = match parse_hash(&headers, &BLOCK_HEADER) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match store.append_block(volume, hash, &body) {
        Ok(offset) => (
            StatusCode::CREATED,
            Json(AppendResponse {
                offset,
                length: body.len() as u64,
            }),
        )
            .into_response(),
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
    match store.read_replica(volume, range.offset, range.length) {
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
        Err(response) => return response,
    };
    match store.write_fragment(volume, fragment, checksum, &body) {
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
    match store.read_fragment(volume, fragment, range) {
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
    match store.delete_replica(volume) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => internal_error(error),
    }
}

async fn delete_fragment(
    State(store): State<NodeStore>,
    AxumPath((volume, fragment)): AxumPath<(u64, u8)>,
) -> Response {
    match store.delete_fragment(volume, fragment) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => internal_error(error),
    }
}

async fn list_extents(State(store): State<NodeStore>) -> Response {
    match store.list_extents() {
        Ok(extents) => Json(extents).into_response(),
        Err(error) => internal_error(error),
    }
}

fn parse_hash(headers: &HeaderMap, name: &HeaderName) -> Result<[u8; 32], Response> {
    let Some(value) = headers.get(name).and_then(|value| value.to_str().ok()) else {
        return Err((StatusCode::BAD_REQUEST, format!("missing {name}")).into_response());
    };
    let hash: BlockHash = value
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, format!("invalid {name}")).into_response())?;
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
        let mut file = File::open(entry.path())?;
        let mut logical_offset = 0;
        let mut physical_offset = 0;
        let mut records = Vec::new();
        loop {
            let mut magic = [0; 4];
            match file.read_exact(&mut magic) {
                Ok(()) => {}
                Err(error)
                    if error.kind() == std::io::ErrorKind::UnexpectedEof
                        && physical_offset == file.metadata()?.len() =>
                {
                    break
                }
                Err(error) => return Err(error).context("read replica record magic"),
            }
            if magic != RECORD_MAGIC {
                bail!("invalid replica record magic in volume {volume}");
            }
            let mut hash = [0; 32];
            let mut length = [0; 8];
            file.read_exact(&mut hash)?;
            file.read_exact(&mut length)?;
            let length = u64::from_le_bytes(length);
            let data_offset = physical_offset + RECORD_HEADER_LEN;
            let next_offset = data_offset
                .checked_add(length)
                .context("replica length overflow")?;
            if next_offset > file.metadata()?.len() {
                bail!("truncated replica record in volume {volume}");
            }
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
}
