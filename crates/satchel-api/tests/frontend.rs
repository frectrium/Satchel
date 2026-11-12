//! The frontend against in-process fakes of the Block Index (#66) and the
//! storage nodes (#67), over real HTTP. Test names follow the IDs in
//! `docs/design/test-strategy.md`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use satchel_common::cluster::{BlockEntry, NodeAddr, Volume, VolumeKind, VolumeState};
use satchel_common::BlockHash;

#[derive(Default)]
struct FakeIndex {
    blocks: HashMap<String, BlockEntry>,
    volumes: Vec<Volume>,
    /// Makes every call this slow, to test timeouts.
    delay: Option<Duration>,
}

type Index = Arc<Mutex<FakeIndex>>;

fn fake_index(index: Index) -> Router {
    async fn slow(index: &Index) {
        let delay = index.lock().unwrap().delay;
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
    }
    async fn get_block(State(index): State<Index>, Path(hash): Path<String>) -> Response {
        slow(&index).await;
        match index.lock().unwrap().blocks.get(&hash) {
            Some(e) => Json(*e).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
    async fn put_block(
        State(index): State<Index>,
        Path(hash): Path<String>,
        Json(entry): Json<BlockEntry>,
    ) -> StatusCode {
        let mut index = index.lock().unwrap();
        if index.blocks.contains_key(&hash) {
            return StatusCode::OK;
        }
        index.blocks.insert(hash, entry);
        if let Some(v) = index.volumes.iter_mut().find(|v| v.id == entry.volume) {
            v.size = v.size.max(entry.offset + entry.length);
        }
        StatusCode::CREATED
    }
    async fn delete_block(State(index): State<Index>, Path(hash): Path<String>) -> StatusCode {
        match index.lock().unwrap().blocks.remove(&hash) {
            Some(_) => StatusCode::NO_CONTENT,
            None => StatusCode::NOT_FOUND,
        }
    }
    async fn get_volume(State(index): State<Index>, Path(id): Path<u64>) -> Response {
        match index.lock().unwrap().volumes.iter().find(|v| v.id == id) {
            Some(v) => Json(v.clone()).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
    async fn list_volumes(
        State(index): State<Index>,
        Query(q): Query<HashMap<String, String>>,
    ) -> Json<Vec<Volume>> {
        slow(&index).await;
        let open = q.get("state").map(String::as_str) == Some("open");
        Json(
            index
                .lock()
                .unwrap()
                .volumes
                .iter()
                .filter(|v| !open || v.state == VolumeState::Open)
                .cloned()
                .collect(),
        )
    }
    Router::new()
        .route(
            "/blocks/{hash}",
            get(get_block).put(put_block).delete(delete_block),
        )
        .route("/volumes", get(list_volumes))
        .route("/volumes/{id}", get(get_volume))
        .with_state(index)
}

#[derive(Default)]
struct FakeNode {
    extents: HashMap<u64, Vec<u8>>,
    fail_writes: bool,
    fail_reads: bool,
    reads: usize,
}

type Node = Arc<Mutex<FakeNode>>;

fn fake_node(node: Node) -> Router {
    async fn append(
        State(node): State<Node>,
        Path(volume): Path<u64>,
        headers: HeaderMap,
        body: Bytes,
    ) -> Response {
        let mut node = node.lock().unwrap();
        if node.fail_writes {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        // The node API needs the hash header.
        let hash = headers
            .get("x-satchel-block")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.parse::<BlockHash>().ok());
        if !hash.is_some_and(|h| h.verify(&body)) {
            return StatusCode::BAD_REQUEST.into_response();
        }
        let extent = node.extents.entry(volume).or_default();
        let offset = extent.len() as u64;
        extent.extend_from_slice(&body);
        let reply = serde_json::json!({"offset": offset, "length": body.len()});
        (StatusCode::CREATED, Json(reply)).into_response()
    }
    async fn read(
        State(node): State<Node>,
        Path(volume): Path<u64>,
        Query(q): Query<HashMap<String, u64>>,
    ) -> Response {
        let mut node = node.lock().unwrap();
        node.reads += 1;
        if node.fail_reads {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        let (offset, length) = (q["offset"] as usize, q["length"] as usize);
        match node.extents.get(&volume) {
            Some(e) if offset + length <= e.len() => {
                Bytes::copy_from_slice(&e[offset..offset + length]).into_response()
            }
            Some(_) => StatusCode::RANGE_NOT_SATISFIABLE.into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
    Router::new()
        .route("/extents/{volume}", post(append).get(read))
        // axum's default limit is 2 MB. The real node must raise it too (#67).
        .layer(DefaultBodyLimit::max(8 << 20))
        .with_state(node)
}

async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

/// A frontend in zone z1, 6 fake nodes (2 per zone) and 2 open volumes:
/// volume 1 on nodes 0, 1, 2 and volume 2 on nodes 3, 4, 5.
struct Cluster {
    api: String,
    index: Index,
    nodes: Vec<Node>,
    http: reqwest::Client,
}

async fn cluster_with(timeout: Duration) -> Cluster {
    let index = Index::default();
    let index_url = serve(fake_index(index.clone())).await;
    let mut nodes = Vec::new();
    let mut addrs = Vec::new();
    for i in 0..6 {
        let node = Node::default();
        let url = serve(fake_node(node.clone())).await;
        addrs.push(NodeAddr {
            id: format!("n{i}"),
            zone: format!("z{}", i % 3 + 1),
            url,
        });
        nodes.push(node);
    }
    for (id, first) in [(1, 0), (2, 3)] {
        index.lock().unwrap().volumes.push(Volume {
            id,
            state: VolumeState::Open,
            kind: VolumeKind::Replicated,
            generation: 1,
            nodes: (first..first + 3).map(|n| format!("n{n}")).collect(),
            size: 0,
        });
    }
    let api = serve(satchel_api::router(satchel_api::Config {
        index_url,
        nodes: addrs,
        zone: Some("z1".to_owned()),
        timeout,
    }))
    .await;
    Cluster {
        api,
        index,
        nodes,
        http: reqwest::Client::new(),
    }
}

async fn cluster() -> Cluster {
    cluster_with(Duration::from_secs(5)).await
}

/// Seeded test data (#31): the block's bytes come from `seed`.
fn block(seed: u64, len: usize) -> (BlockHash, Vec<u8>) {
    let mut x = seed.max(1);
    let data: Vec<u8> = (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect();
    (BlockHash::of(&data), data)
}

impl Cluster {
    async fn put(&self, hash: &str, body: Vec<u8>) -> u16 {
        self.http
            .put(format!("{}/blocks/{hash}", self.api))
            .body(body)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    }

    async fn get(&self, hash: &str) -> (u16, Vec<u8>) {
        let r = self
            .http
            .get(format!("{}/blocks/{hash}", self.api))
            .send()
            .await
            .unwrap();
        let status = r.status().as_u16();
        (status, r.bytes().await.unwrap().to_vec())
    }

    async fn delete(&self, hash: &str) -> u16 {
        self.http
            .delete(format!("{}/blocks/{hash}", self.api))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    }

    fn stored(&self, node: usize) -> usize {
        self.nodes[node]
            .lock()
            .unwrap()
            .extents
            .values()
            .map(Vec::len)
            .sum()
    }
}

#[tokio::test]
async fn ts_api_1_put_then_get_byte_for_byte() {
    let c = cluster().await;
    for (seed, len) in [(1, 1), (2, 4096), (3, 2 << 20), (4, 4 << 20)] {
        let (hash, data) = block(seed, len);
        assert_eq!(
            c.put(&hash.to_string(), data.clone()).await,
            201,
            "seed {seed}"
        );
        let (status, body) = c.get(&hash.to_string()).await;
        assert_eq!(status, 200, "seed {seed}");
        assert!(body == data, "seed {seed}");
    }
}

#[tokio::test]
async fn ts_api_2_key_must_be_the_content_hash() {
    let c = cluster().await;
    let (hash, _) = block(10, 100);
    let (_, other) = block(11, 100);
    assert_eq!(c.put(&hash.to_string(), other).await, 400);
    assert_eq!(c.put("not-a-hash", vec![1]).await, 400);
    assert_eq!(c.get("not-a-hash").await.0, 400);
}

#[tokio::test]
async fn ts_api_4_duplicate_put_stores_once() {
    let c = cluster().await;
    let (hash, data) = block(20, 1000);
    assert_eq!(c.put(&hash.to_string(), data.clone()).await, 201);
    let before: usize = (0..6).map(|n| c.stored(n)).sum();
    assert_eq!(c.put(&hash.to_string(), data).await, 200);
    let after: usize = (0..6).map(|n| c.stored(n)).sum();
    assert_eq!(before, after);
    assert_eq!(c.index.lock().unwrap().blocks.len(), 1);
}

#[tokio::test]
async fn ts_api_5_unknown_hash_is_404() {
    let c = cluster().await;
    let (hash, _) = block(30, 10);
    assert_eq!(c.get(&hash.to_string()).await.0, 404);
    assert_eq!(c.delete(&hash.to_string()).await, 404);
}

#[tokio::test]
async fn ts_api_6_size_limits() {
    let c = cluster().await;
    let empty = BlockHash::of(b"");
    assert_eq!(c.put(&empty.to_string(), vec![]).await, 400);
    let (hash, data) = block(40, (4 << 20) + 1);
    assert_eq!(c.put(&hash.to_string(), data).await, 413);
    let (hash, data) = block(41, 4 << 20);
    assert_eq!(c.put(&hash.to_string(), data).await, 201);
}

#[tokio::test]
async fn ts_api_7_deleted_block_is_gone() {
    let c = cluster().await;
    let (hash, data) = block(50, 500);
    assert_eq!(c.put(&hash.to_string(), data).await, 201);
    assert_eq!(c.delete(&hash.to_string()).await, 204);
    assert_eq!(c.get(&hash.to_string()).await.0, 404);
}

#[tokio::test]
async fn ts_api_8_slow_index_gives_a_5xx_not_a_hang() {
    let c = cluster_with(Duration::from_millis(200)).await;
    c.index.lock().unwrap().delay = Some(Duration::from_secs(10));
    let (hash, data) = block(60, 100);
    let start = Instant::now();
    assert_eq!(c.put(&hash.to_string(), data).await, 503);
    assert_eq!(c.get(&hash.to_string()).await.0, 503);
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[tokio::test]
async fn ts_rep_1_put_lands_on_three_nodes_one_per_zone() {
    let c = cluster().await;
    let (hash, data) = block(70, 3000);
    assert_eq!(c.put(&hash.to_string(), data).await, 201);
    let entry = c.index.lock().unwrap().blocks[&hash.to_string()];
    let holders: Vec<usize> = (0..6).filter(|&n| c.stored(n) == 3000).collect();
    let expected: Vec<usize> = if entry.volume == 1 {
        vec![0, 1, 2]
    } else {
        vec![3, 4, 5]
    };
    assert_eq!(holders, expected);
    // Nodes 0..3 are in zones z1, z2, z3.
    let zones: Vec<usize> = holders.iter().map(|n| n % 3).collect();
    assert_eq!(zones, [0, 1, 2]);
}

#[tokio::test]
async fn ts_rep_2_get_works_with_one_replica_down() {
    let c = cluster().await;
    let (hash, data) = block(80, 2048);
    assert_eq!(c.put(&hash.to_string(), data.clone()).await, 201);
    let first = if c.index.lock().unwrap().blocks[&hash.to_string()].volume == 1 {
        0
    } else {
        3
    };
    c.nodes[first].lock().unwrap().fail_reads = true;
    let (status, body) = c.get(&hash.to_string()).await;
    assert_eq!(status, 200);
    assert!(body == data);
}

#[tokio::test]
async fn reads_prefer_the_own_zone() {
    let c = cluster().await;
    let (hash, data) = block(90, 64);
    assert_eq!(c.put(&hash.to_string(), data).await, 201);
    for _ in 0..5 {
        assert_eq!(c.get(&hash.to_string()).await.0, 200);
    }
    // The frontend is in z1, which is nodes 0 and 3.
    let reads: Vec<usize> = c.nodes.iter().map(|n| n.lock().unwrap().reads).collect();
    assert_eq!(reads[0] + reads[3], 5, "{reads:?}");
}

#[tokio::test]
async fn failed_replica_write_retries_on_another_volume() {
    let c = cluster().await;
    c.nodes[1].lock().unwrap().fail_writes = true;
    for seed in 100..110 {
        let (hash, data) = block(seed, 100);
        assert_eq!(c.put(&hash.to_string(), data).await, 201, "seed {seed}");
        let entry = c.index.lock().unwrap().blocks[&hash.to_string()];
        assert_eq!(entry.volume, 2, "seed {seed}");
    }
}

#[tokio::test]
async fn no_open_volume_is_503() {
    let c = cluster().await;
    for v in &mut c.index.lock().unwrap().volumes {
        v.state = VolumeState::Closed;
    }
    let (hash, data) = block(120, 100);
    assert_eq!(c.put(&hash.to_string(), data).await, 503);
}

#[tokio::test]
async fn concurrent_puts_keep_replicas_in_step() {
    let c = Arc::new(cluster().await);
    let mut tasks = Vec::new();
    for seed in 200..264 {
        let c = c.clone();
        tasks.push(tokio::spawn(async move {
            let (hash, data) = block(seed, 1000 + seed as usize);
            assert_eq!(c.put(&hash.to_string(), data.clone()).await, 201);
            (hash, data)
        }));
    }
    let mut written = Vec::new();
    for t in tasks {
        written.push(t.await.unwrap());
    }
    // Every replica of a volume holds the same bytes.
    let extent = |n: usize, v: u64| c.nodes[n].lock().unwrap().extents.get(&v).cloned();
    assert_eq!(extent(0, 1), extent(1, 1));
    assert_eq!(extent(1, 1), extent(2, 1));
    assert_eq!(extent(3, 2), extent(4, 2));
    assert_eq!(extent(4, 2), extent(5, 2));
    for (hash, data) in written {
        let (status, body) = c.get(&hash.to_string()).await;
        assert_eq!(status, 200);
        assert!(body == data);
    }
}
