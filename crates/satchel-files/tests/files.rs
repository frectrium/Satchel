//! The files API against a fake Satchel frontend and storage node.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use satchel_common::cluster::NodeAddr;
use satchel_files::{
    manifest::Manifest, satchel::SatchelClient, AppState, Config, Limits, BLOCK_BYTES,
};
use serde_json::Value;

#[derive(Default)]
struct Fake {
    blocks: Mutex<HashMap<String, Bytes>>,
    /// Puts that answer 503 first, like the moment a volume closes.
    fail_next_puts: AtomicU32,
    puts: AtomicU32,
    coded: AtomicBool,
    /// The Block Index answers 503, as while it restarts.
    index_down: AtomicBool,
    /// Puts after this many succeed are refused for good, like a full disk.
    refuse_after: Mutex<Option<u32>>,
}

async fn fake_put(State(f): State<Arc<Fake>>, Path(hash): Path<String>, body: Bytes) -> StatusCode {
    let n = f.puts.fetch_add(1, Ordering::SeqCst);
    if f.refuse_after.lock().unwrap().is_some_and(|ok| n >= ok) {
        // A client error, so the files service stops at once instead of
        // retrying for half a minute as it does on a 503.
        return StatusCode::BAD_REQUEST;
    }
    let left = f.fail_next_puts.load(Ordering::SeqCst);
    if left > 0 {
        f.fail_next_puts.store(left - 1, Ordering::SeqCst);
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    assert_eq!(
        satchel_common::BlockHash::of(&body).to_string(),
        hash,
        "the files service sent a block under the wrong hash"
    );
    assert!(body.len() <= BLOCK_BYTES);
    match f.blocks.lock().unwrap().entry(hash) {
        std::collections::hash_map::Entry::Occupied(_) => StatusCode::OK,
        std::collections::hash_map::Entry::Vacant(slot) => {
            slot.insert(body);
            StatusCode::CREATED
        }
    }
}

async fn fake_get(
    State(f): State<Arc<Fake>>,
    Path(hash): Path<String>,
) -> Result<Bytes, StatusCode> {
    f.blocks
        .lock()
        .unwrap()
        .get(&hash)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)
}

async fn fake_delete(State(f): State<Arc<Fake>>, Path(hash): Path<String>) -> StatusCode {
    match f.blocks.lock().unwrap().remove(&hash) {
        Some(_) => StatusCode::NO_CONTENT,
        None => StatusCode::NOT_FOUND,
    }
}

async fn fake_extents(State(f): State<Arc<Fake>>) -> Json<Value> {
    let bytes: usize = f.blocks.lock().unwrap().values().map(|b| b.len()).sum();
    let kind = if f.coded.load(Ordering::SeqCst) {
        "fragment"
    } else {
        "replica"
    };
    Json(serde_json::json!([{"volume": 1, "kind": kind, "index": null, "size": bytes}]))
}

async fn fake_inventory(State(f): State<Arc<Fake>>) -> Result<Json<Value>, StatusCode> {
    if f.index_down.load(Ordering::SeqCst) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let kind = if f.coded.load(Ordering::SeqCst) {
        "rs63"
    } else {
        "replicated"
    };
    let blocks = f.blocks.lock().unwrap();
    let mut offset = 0;
    let entries: Vec<_> = blocks
        .iter()
        .map(|(hash, bytes)| {
            let entry = serde_json::json!({
                "hash": hash,
                "volume": 1,
                "offset": offset,
                "length": bytes.len(),
            });
            offset += bytes.len();
            entry
        })
        .collect();
    Ok(Json(serde_json::json!({
        "volumes": [{"id": 1, "state": "closed", "kind": kind, "generation": 1,
            "opened_at_secs": 0, "nodes": ["node-z1-1"], "size": offset}],
        "blocks": entries,
    })))
}

async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

struct Harness {
    url: String,
    fake: Arc<Fake>,
    http: reqwest::Client,
}

async fn harness(limits: Limits) -> Harness {
    let fake = Arc::new(Fake::default());
    let satchel = serve(
        Router::new()
            .route(
                "/blocks/{hash}",
                put(fake_put).get(fake_get).delete(fake_delete),
            )
            .route("/extents", get(fake_extents))
            .route("/inventory", get(fake_inventory))
            .layer(axum::extract::DefaultBodyLimit::max(2 * BLOCK_BYTES))
            .with_state(fake.clone()),
    )
    .await;
    let nodes = vec![NodeAddr {
        id: "node-z1-1".into(),
        zone: "z1".into(),
        url: satchel.clone(),
    }];
    let state = AppState::new(
        SatchelClient::new(&satchel, &satchel, nodes),
        Manifest::open_in_memory().unwrap(),
        Config {
            allowed_origins: vec!["https://satchel.example".into()],
            limits,
        },
    );
    Harness {
        url: serve(satchel_files::router(state)).await,
        fake,
        http: reqwest::Client::new(),
    }
}

impl Harness {
    async fn upload(&self, name: &str, data: &[u8]) -> (u16, Value) {
        let r = self
            .http
            .post(format!("{}/v1/files?name={name}", self.url))
            .body(data.to_vec())
            .send()
            .await
            .unwrap();
        (r.status().as_u16(), r.json().await.unwrap())
    }

    async fn get_json(&self, path: &str) -> Value {
        self.http
            .get(format!("{}{path}", self.url))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}

/// Deterministic bytes that don't repeat within a block.
fn data(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u32).wrapping_mul(2_654_435_761).to_le_bytes()[i % 4] ^ seed)
        .collect()
}

#[tokio::test]
async fn a_file_round_trips_through_blocks() {
    let h = harness(Limits::default()).await;
    let original = data(2 * BLOCK_BYTES + 12_345, 1);
    let (status, report) = h.upload("photo.raw", &original).await;
    assert_eq!(status, 201, "{report}");
    assert_eq!(report["blocks"], 3);
    assert_eq!(report["new_blocks"], 3);
    assert_eq!(report["replicas"], 3);
    assert_eq!(report["raw_bytes_written"], 3 * original.len() as u64);
    assert_eq!(report["file"]["name"], "photo.raw");

    let id = report["file"]["id"].as_i64().unwrap();
    let r = h
        .http
        .get(format!("{}/v1/files/{id}/content", h.url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.headers()["content-disposition"],
        "attachment; filename=\"photo.raw\""
    );
    assert_eq!(r.bytes().await.unwrap().to_vec(), original);

    let detail = h.get_json(&format!("/v1/files/{id}")).await;
    let sizes: Vec<u64> = detail["block_list"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["size"].as_u64().unwrap())
        .collect();
    assert_eq!(sizes, [BLOCK_BYTES as u64, BLOCK_BYTES as u64, 12_345]);
}

#[tokio::test]
async fn files_and_stats_show_replicated_then_coded_storage() {
    let h = harness(Limits::default()).await;
    let (status, _) = h
        .upload("notes.txt", b"compaction changes this layout")
        .await;
    assert_eq!(status, 201);

    let before = h.get_json("/v1/files").await;
    assert_eq!(before[0]["storage"]["replicated_blocks"], 1);
    assert_eq!(before[0]["storage"]["coded_blocks"], 0);
    assert_eq!(before[0]["storage"]["compacted"], false);

    h.fake.coded.store(true, Ordering::SeqCst);
    let after = h.get_json("/v1/files").await;
    assert_eq!(after[0]["storage"]["replicated_blocks"], 0);
    assert_eq!(after[0]["storage"]["coded_blocks"], 1);
    assert_eq!(after[0]["storage"]["compacted"], true);
    let stats = h.get_json("/v1/stats").await;
    assert!(stats["zones"][0]["coded_bytes"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn files_are_still_listed_while_the_index_is_unreachable() {
    let h = harness(Limits::default()).await;
    assert_eq!(h.upload("kept.txt", b"still listed").await.0, 201);
    h.fake.index_down.store(true, Ordering::SeqCst);

    let files = h.get_json("/v1/files").await;
    assert_eq!(files[0]["name"], "kept.txt");
    assert!(files[0].get("storage").is_none(), "{files}");
    let stats = h.get_json("/v1/stats").await;
    assert_eq!(stats["files"], 1);
    assert!(stats["reclaimable_bytes"].is_null(), "{stats}");
}

#[tokio::test]
async fn files_from_a_folder_keep_their_path() {
    let h = harness(Limits::default()).await;
    let (status, report) = h.upload("trip%2F2024%2Fbeach.jpg", b"sand").await;
    assert_eq!(status, 201, "{report}");
    assert_eq!(report["file"]["name"], "trip/2024/beach.jpg");
    let id = report["file"]["id"].as_i64().unwrap();
    let r = h
        .http
        .get(format!("{}/v1/files/{id}/content", h.url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.headers()["content-disposition"],
        "attachment; filename=\"beach.jpg\""
    );
    let (_, sneaky) = h.upload("..%2F..%2Fsecret.txt", b"x").await;
    assert_eq!(sneaky["file"]["name"], "secret.txt");
}

#[tokio::test]
async fn the_same_content_is_stored_once() {
    let h = harness(Limits::default()).await;
    let original = data(BLOCK_BYTES + 10, 2);
    h.upload("a.bin", &original).await;
    let (_, again) = h.upload("copy-of-a.bin", &original).await;
    assert_eq!(again["new_blocks"], 0);
    assert_eq!(again["dedup_blocks"], 2);

    let stats = h.get_json("/v1/stats").await;
    assert_eq!(stats["files"], 2);
    assert_eq!(stats["logical_bytes"], 2 * original.len() as u64);
    assert_eq!(stats["unique_bytes"], original.len() as u64);
    assert_eq!(stats["dedup_saved_bytes"], original.len() as u64);
    assert_eq!(stats["raw_bytes"], original.len() as u64);
    assert_eq!(stats["nodes"][0]["up"], true);
}

/// Waits for the task that releases a failed upload's blocks.
async fn stored_blocks_settle(h: &Harness, want: usize) {
    for _ in 0..100 {
        if h.fake.blocks.lock().unwrap().len() == want {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!(
        "{} blocks still stored, want {want}",
        h.fake.blocks.lock().unwrap().len()
    );
}

#[tokio::test]
async fn a_failed_upload_releases_the_blocks_it_wrote() {
    let h = harness(Limits::default()).await;
    *h.fake.refuse_after.lock().unwrap() = Some(2);

    let (status, _) = h.upload("big.bin", &data(3 * BLOCK_BYTES, 9)).await;

    assert!(status >= 500, "{status}");
    stored_blocks_settle(&h, 0).await;
    assert_eq!(h.get_json("/v1/files").await, serde_json::json!([]));
}

#[tokio::test]
async fn a_failed_upload_keeps_blocks_that_files_use() {
    let h = harness(Limits::default()).await;
    let shared = data(BLOCK_BYTES, 10);
    assert_eq!(h.upload("kept.bin", &shared).await.0, 201);
    let mut failing = shared.clone();
    failing.extend(data(BLOCK_BYTES, 11));
    failing.extend(data(10, 12));
    // kept.bin took one put; the failing upload gets two more, then a refusal.
    *h.fake.refuse_after.lock().unwrap() = Some(3);

    assert!(h.upload("failing.bin", &failing).await.0 >= 500);

    stored_blocks_settle(&h, 1).await;
    let files = h.get_json("/v1/files").await;
    let id = files[0]["id"].as_i64().unwrap();
    let r = h
        .http
        .get(format!("{}/v1/files/{id}/content", h.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.bytes().await.unwrap().to_vec(), shared);
}

#[tokio::test]
async fn an_upload_cut_off_by_the_client_releases_its_blocks() {
    use tokio::io::AsyncWriteExt;
    let h = harness(Limits::default()).await;
    let addr = h.url.trim_start_matches("http://");
    let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let total = 3 * BLOCK_BYTES;
    let head = format!(
        "POST /v1/files?name=cut.bin HTTP/1.1\r\nHost: {addr}\r\nContent-Length: {total}\r\n\r\n"
    );
    socket.write_all(head.as_bytes()).await.unwrap();
    // One whole block and a bit, then the browser tab closes.
    socket
        .write_all(&data(BLOCK_BYTES + 1000, 13))
        .await
        .unwrap();
    for _ in 0..100 {
        if !h.fake.blocks.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(h.fake.blocks.lock().unwrap().len(), 1);
    drop(socket);

    stored_blocks_settle(&h, 0).await;
    assert_eq!(h.get_json("/v1/files").await, serde_json::json!([]));
}

#[tokio::test]
async fn deleting_releases_only_blocks_no_other_file_uses() {
    let h = harness(Limits::default()).await;
    let shared = data(BLOCK_BYTES, 3);
    let mut a = shared.clone();
    a.extend(data(100, 4));
    let mut b = shared.clone();
    b.extend(data(100, 5));
    let (_, ra) = h.upload("a", &a).await;
    let (_, rb) = h.upload("b", &b).await;
    assert_eq!(h.fake.blocks.lock().unwrap().len(), 3);

    let id_a = ra["file"]["id"].as_i64().unwrap();
    let r: Value = h
        .http
        .delete(format!("{}/v1/files/{id_a}", h.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["released_blocks"], 1);
    assert_eq!(h.fake.blocks.lock().unwrap().len(), 2);

    let id_b = rb["file"]["id"].as_i64().unwrap();
    let r = h
        .http
        .get(format!("{}/v1/files/{id_b}/content", h.url))
        .send()
        .await
        .unwrap();
    assert_eq!(r.bytes().await.unwrap().to_vec(), b);

    let files = h.get_json("/v1/files").await;
    assert_eq!(files.as_array().unwrap().len(), 1);
    let gone = h
        .http
        .delete(format!("{}/v1/files/{id_a}", h.url))
        .send()
        .await
        .unwrap();
    assert_eq!(gone.status(), 404);
}

#[tokio::test]
async fn puts_retry_while_no_volume_is_open() {
    let h = harness(Limits::default()).await;
    h.fake.fail_next_puts.store(3, Ordering::SeqCst);
    let (status, report) = h.upload("x", &data(1000, 6)).await;
    assert_eq!(status, 201, "{report}");
    assert_eq!(h.fake.puts.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn an_empty_file_has_no_blocks() {
    let h = harness(Limits::default()).await;
    let (status, report) = h.upload("empty.txt", b"").await;
    assert_eq!(status, 201);
    assert_eq!(report["blocks"], 0);
    let id = report["file"]["id"].as_i64().unwrap();
    let r = h
        .http
        .get(format!("{}/v1/files/{id}/content", h.url))
        .send()
        .await
        .unwrap();
    assert!(r.bytes().await.unwrap().is_empty());
}

#[tokio::test]
async fn limits_are_enforced() {
    let h = harness(Limits {
        max_file_bytes: Some(1_000),
        max_total_bytes: Some(1_500),
    })
    .await;
    let (status, _) = h.upload("big", &data(1_001, 7)).await;
    assert_eq!(status, 413);
    let (status, _) = h.upload("ok", &data(1_000, 8)).await;
    assert_eq!(status, 201);
    let (status, _) = h.upload("full", &data(600, 9)).await;
    assert_eq!(status, 507);
}

#[tokio::test]
async fn the_website_may_call_a_local_install() {
    let h = harness(Limits::default()).await;
    let preflight = |origin: &'static str| {
        h.http
            .request(reqwest::Method::OPTIONS, format!("{}/v1/files", h.url))
            .header("origin", origin)
            .header("access-control-request-method", "POST")
            .header("access-control-request-private-network", "true")
            .send()
    };
    let ok = preflight("https://satchel.example").await.unwrap();
    assert_eq!(
        ok.headers()["access-control-allow-origin"],
        "https://satchel.example"
    );
    assert_eq!(ok.headers()["access-control-allow-private-network"], "true");

    let other = preflight("https://evil.example").await.unwrap();
    assert!(other.headers().get("access-control-allow-origin").is_none());
}
