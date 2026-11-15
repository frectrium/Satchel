//! Satchel files service: whole files on top of a storage backend, the API
//! the website and the local install talk to.
//!
//! **Satchel backend** (the local install): a file is split into blocks of
//! up to 4 MiB (DDL-001), each stored by its SHA-256 through the frontend,
//! so identical blocks are stored once (DDL-002). The manifest
//! ([`manifest`]) keeps each file's block list. Dropbox splits files into
//! blocks the same way and keeps file metadata in separate systems such as
//! FileJournal [B2]. Here that metadata is a small SQLite database next to
//! the service, a deliberate deviation (DDL-020).
//!
//! **S3 backend** (the hosted site): each file is one whole object in S3,
//! what Dropbox moved away from. Only signed-in admins may use it
//! ([`auth`]).
//!
//! Routes, all under `/v1`: `GET /info`, `GET /stats`, `GET /files`,
//! `POST /files?name=`, `GET /files/{id}`, `GET /files/{id}/content`,
//! `DELETE /files/{id}`, and for the S3 backend `POST /login`,
//! `POST /logout` and `GET /me`.

pub mod auth;
pub mod manifest;
pub mod s3;
pub mod satchel;

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
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

pub enum Backend {
    Satchel {
        client: SatchelClient,
        manifest: Manifest,
    },
    S3(s3::S3Store),
}

pub struct AppState {
    backend: Backend,
    auth: Option<auth::Auth>,
    config: Config,
}

impl AppState {
    /// The local install: Satchel underneath, no login.
    pub fn new(client: SatchelClient, manifest: Manifest, config: Config) -> Arc<Self> {
        Arc::new(Self {
            backend: Backend::Satchel { client, manifest },
            auth: None,
            config,
        })
    }

    /// The hosted site: S3 underneath, admins only.
    pub fn s3(store: s3::S3Store, auth: auth::Auth, config: Config) -> Arc<Self> {
        Arc::new(Self {
            backend: Backend::S3(store),
            auth: Some(auth),
            config,
        })
    }

    fn backend_name(&self) -> &'static str {
        match self.backend {
            Backend::Satchel { .. } => "satchel",
            Backend::S3(_) => "s3",
        }
    }

    /// `Some(401)` when a login is required and the request has none.
    fn deny(&self, headers: &HeaderMap) -> Option<Response> {
        let auth = self.auth.as_ref()?;
        if auth.user(headers).is_some() {
            None
        } else {
            Some(error(StatusCode::UNAUTHORIZED, "sign in as an admin first"))
        }
    }
}

/// The files API, with CORS for the website.
pub fn router(state: Arc<AppState>) -> Router {
    let cors = cors(&state.config.allowed_origins);
    Router::new()
        .route("/v1/info", get(info))
        .route("/v1/login", post(login))
        .route("/v1/logout", post(logout))
        .route("/v1/me", get(me))
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

// ---------- info and login ----------

#[derive(Serialize)]
struct Info {
    service: &'static str,
    version: &'static str,
    backend: &'static str,
    login_required: bool,
    block_bytes: Option<usize>,
    replicas: Option<u32>,
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
    let (block_bytes, replicas, nodes) = match &state.backend {
        Backend::Satchel { client, .. } => (
            Some(BLOCK_BYTES),
            Some(REPLICAS),
            client
                .nodes()
                .iter()
                .map(|n| NodeInfo {
                    id: n.id.clone(),
                    zone: n.zone.clone(),
                })
                .collect(),
        ),
        Backend::S3(_) => (None, None, Vec::new()),
    };
    Json(Info {
        service: "satchel-files",
        version: env!("CARGO_PKG_VERSION"),
        backend: state.backend_name(),
        login_required: state.auth.is_some(),
        block_bytes,
        replicas,
        nodes,
        max_file_bytes: state.config.limits.max_file_bytes,
        max_total_bytes: state.config.limits.max_total_bytes,
    })
    .into_response()
}

#[derive(Deserialize)]
struct Login {
    user: String,
    password: String,
}

#[derive(Serialize)]
struct Me {
    user: Option<String>,
}

async fn login(State(state): State<Arc<AppState>>, Json(l): Json<Login>) -> Response {
    let Some(auth) = &state.auth else {
        return error(StatusCode::NOT_FOUND, "this install has no login");
    };
    if let Some(wait) = auth.retry_after(&l.user) {
        let secs = wait.as_secs().max(1);
        return (
            [(header::RETRY_AFTER, secs.to_string())],
            error(
                StatusCode::TOO_MANY_REQUESTS,
                format!("too many wrong passwords, try again in {secs} s"),
            ),
        )
            .into_response();
    }
    // At most a couple of Argon2 runs at once (memory and guessing), each on
    // the blocking pool since it's deliberately slow.
    let Some(slot) = auth.check_slot() else {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "too many sign-ins at once, try again in a moment",
        );
    };
    let checker = state.clone();
    let user = l.user.clone();
    let ok = tokio::task::spawn_blocking(move || {
        let _slot = slot;
        checker
            .auth
            .as_ref()
            .is_some_and(|a| a.verify(&user, &l.password))
    })
    .await
    .unwrap_or(false);
    auth.record(&l.user, ok);
    if !ok {
        return error(StatusCode::UNAUTHORIZED, "wrong user name or password");
    }
    let cookie = auth.start(&l.user);
    (
        [(header::SET_COOKIE, cookie)],
        Json(Me { user: Some(l.user) }),
    )
        .into_response()
}

async fn logout(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    match &state.auth {
        Some(auth) => (
            [(header::SET_COOKIE, auth.end(&headers))],
            Json(Me { user: None }),
        )
            .into_response(),
        None => Json(Me { user: None }).into_response(),
    }
}

async fn me(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let user = state.auth.as_ref().and_then(|a| a.user(&headers));
    Json(Me { user }).into_response()
}

// ---------- stats and listing ----------

#[derive(Serialize)]
pub struct Stats {
    pub backend: &'static str,
    pub files: u64,
    pub logical_bytes: u64,
    /// Satchel only: distinct blocks and their bytes.
    pub unique_blocks: Option<u64>,
    pub unique_bytes: Option<u64>,
    /// Satchel only: bytes not stored again because a block was already there.
    pub dedup_saved_bytes: Option<u64>,
    pub replicas: Option<u32>,
    /// Satchel only: what the storage nodes hold on disk, summed. S3 doesn't
    /// say how many copies it keeps or where.
    pub raw_bytes: Option<u64>,
    pub nodes: Vec<NodeUsage>,
}

async fn stats(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Some(r) = state.deny(&headers) {
        return r;
    }
    match &state.backend {
        Backend::Satchel { client, manifest } => {
            let totals = match manifest.totals() {
                Ok(t) => t,
                Err(e) => return internal(e),
            };
            let nodes = client.node_usage().await;
            Json(Stats {
                backend: "satchel",
                files: totals.files,
                logical_bytes: totals.logical_bytes,
                unique_blocks: Some(totals.unique_blocks),
                unique_bytes: Some(totals.unique_bytes),
                dedup_saved_bytes: Some(totals.logical_bytes.saturating_sub(totals.unique_bytes)),
                replicas: Some(REPLICAS),
                raw_bytes: Some(nodes.iter().map(|n| n.bytes).sum()),
                nodes,
            })
            .into_response()
        }
        Backend::S3(store) => match store.list().await {
            Ok(files) => Json(Stats {
                backend: "s3",
                files: files.len() as u64,
                logical_bytes: files.iter().map(|f| f.size).sum(),
                unique_blocks: None,
                unique_bytes: None,
                dedup_saved_bytes: None,
                replicas: None,
                raw_bytes: None,
                nodes: Vec::new(),
            })
            .into_response(),
            Err(e) => internal(e),
        },
    }
}

async fn list(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Some(r) = state.deny(&headers) {
        return r;
    }
    let files = match &state.backend {
        Backend::Satchel { manifest, .. } => manifest.list(),
        Backend::S3(store) => store.list().await,
    };
    match files {
        Ok(files) => Json(files).into_response(),
        Err(e) => internal(e),
    }
}

// ---------- upload ----------

#[derive(Deserialize)]
struct UploadQuery {
    name: Option<String>,
}

/// What one upload to Satchel did, for the website's live counters.
#[derive(Serialize)]
pub struct UploadReport {
    pub backend: &'static str,
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

/// What one upload to S3 did: one object, however big.
#[derive(Serialize)]
pub struct S3UploadReport {
    pub backend: &'static str,
    pub file: FileRecord,
    pub objects: u64,
    pub bytes: u64,
    pub millis: u64,
}

async fn upload(
    State(state): State<Arc<AppState>>,
    Query(q): Query<UploadQuery>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if let Some(r) = state.deny(&headers) {
        return r;
    }
    let name = clean_name(q.name.as_deref());
    match &state.backend {
        Backend::Satchel { client, manifest } => {
            upload_satchel(client, manifest, &state.config.limits, &name, body).await
        }
        Backend::S3(store) => upload_s3(store, &state.config.limits, &name, &headers, body).await,
    }
}

async fn upload_s3(
    store: &s3::S3Store,
    limits: &Limits,
    name: &str,
    headers: &HeaderMap,
    body: Body,
) -> Response {
    let started = Instant::now();
    let Some(size) = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
    else {
        return error(
            StatusCode::LENGTH_REQUIRED,
            "send the file with a Content-Length",
        );
    };
    if limits.max_file_bytes.is_some_and(|max| size > max) {
        return error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "the file is over the size limit",
        );
    }
    if let Some(max) = limits.max_total_bytes {
        let used: u64 = match store.list().await {
            Ok(files) => files.iter().map(|f| f.size).sum(),
            Err(e) => return internal(e),
        };
        if used + size > max {
            return error(StatusCode::INSUFFICIENT_STORAGE, "storage is full");
        }
    }
    match store.put(name, size, body.into_data_stream()).await {
        Ok(file) => (
            StatusCode::CREATED,
            Json(S3UploadReport {
                backend: "s3",
                objects: 1,
                bytes: file.size,
                millis: started.elapsed().as_millis() as u64,
                file,
            }),
        )
            .into_response(),
        Err(e) => internal(e),
    }
}

async fn upload_satchel(
    client: &SatchelClient,
    manifest: &Manifest,
    limits: &Limits,
    name: &str,
    body: Body,
) -> Response {
    let started = Instant::now();
    let used = match manifest.totals() {
        Ok(t) => t.logical_bytes,
        Err(e) => return internal(e),
    };

    let mut stream = body.into_data_stream();
    let mut pending: Vec<u8> = Vec::with_capacity(BLOCK_BYTES);
    let mut whole = Sha256::new();
    let mut size = 0u64;
    let mut blocks: Vec<(String, u64)> = Vec::new();
    let (mut new_blocks, mut new_bytes) = (0u64, 0u64);

    let store = |block: Vec<u8>| async move {
        let hash = BlockHash::of(&block).to_string();
        let len = block.len() as u64;
        let written = client.put_block(&hash, Bytes::from(block)).await?;
        anyhow::Ok((hash, len, written))
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
    let file = match manifest.add_file(name, size, &sha256, now_secs(), &blocks) {
        Ok(f) => f,
        Err(e) => return internal(e),
    };
    let report = UploadReport {
        backend: "satchel",
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

// ---------- one file ----------

#[derive(Serialize)]
struct FileDetail {
    #[serde(flatten)]
    file: FileRecord,
    block_list: Vec<manifest::BlockRef>,
}

async fn file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    if let Some(r) = state.deny(&headers) {
        return r;
    }
    let found = match &state.backend {
        Backend::Satchel { manifest, .. } => manifest.get(id),
        Backend::S3(store) => store
            .find(id)
            .await
            .map(|f| f.map(|(_, file)| (file, Vec::new()))),
    };
    match found {
        Ok(Some((file, block_list))) => Json(FileDetail { file, block_list }).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "no such file"),
        Err(e) => internal(e),
    }
}

/// Satchel: streams the file back block by block, each checked against its
/// hash by the frontend. S3: redirects to a short-lived download URL.
async fn content(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    if let Some(r) = state.deny(&headers) {
        return r;
    }
    let (client, manifest) = match &state.backend {
        Backend::Satchel { client, manifest } => (client, manifest),
        Backend::S3(store) => {
            return match store.find(id).await {
                Ok(Some((key, file))) => match store.download_url(&key, &file.name).await {
                    Ok(url) => Redirect::to(&url).into_response(),
                    Err(e) => internal(e),
                },
                Ok(None) => error(StatusCode::NOT_FOUND, "no such file"),
                Err(e) => internal(e),
            }
        }
    };
    let (file, blocks) = match manifest.get(id) {
        Ok(Some(found)) => found,
        Ok(None) => return error(StatusCode::NOT_FOUND, "no such file"),
        Err(e) => return internal(e),
    };
    let client = client.clone();
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
    /// Blocks no other file used, now dropped from Satchel's index. Always 0
    /// for S3, which deletes the whole object.
    released_blocks: u64,
}

async fn remove(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    if let Some(r) = state.deny(&headers) {
        return r;
    }
    let (client, manifest) = match &state.backend {
        Backend::Satchel { client, manifest } => (client, manifest),
        Backend::S3(store) => {
            return match store.find(id).await {
                Ok(Some((key, _))) => match store.delete(&key).await {
                    Ok(()) => Json(Removed {
                        id,
                        released_blocks: 0,
                    })
                    .into_response(),
                    Err(e) => internal(e),
                },
                Ok(None) => error(StatusCode::NOT_FOUND, "no such file"),
                Err(e) => internal(e),
            }
        }
    };
    let orphans = match manifest.remove(id) {
        Ok(Some(o)) => o,
        Ok(None) => return error(StatusCode::NOT_FOUND, "no such file"),
        Err(e) => return internal(e),
    };
    for hash in &orphans {
        if let Err(e) = client.delete_block(hash).await {
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
