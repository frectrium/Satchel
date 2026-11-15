//! Repair (#71) against in-process fakes of the index (#66) and the storage
//! nodes (#67), over real HTTP. The threshold is 200 ms instead of 30 s.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use satchel_common::cluster::{BlockEntry, NodeAddr, Volume, VolumeKind, VolumeState};
use satchel_common::BlockHash;
use satchel_verifier::repair::{RepairConfig, Repairer};

type Shared<T> = Arc<Mutex<T>>;

#[derive(Default)]
struct FakeIndex {
    volumes: Vec<Volume>,
    blocks: Vec<(BlockHash, BlockEntry)>,
    /// Make the next N volume updates lose to another writer.
    conflicts: usize,
}

fn fake_index(index: Shared<FakeIndex>) -> Router {
    async fn list(State(i): State<Shared<FakeIndex>>) -> Json<Vec<Volume>> {
        Json(i.lock().unwrap().volumes.clone())
    }
    async fn one(State(i): State<Shared<FakeIndex>>, Path(id): Path<u64>) -> Response {
        match i.lock().unwrap().volumes.iter().find(|v| v.id == id) {
            Some(v) => Json(v.clone()).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
    async fn update(
        State(i): State<Shared<FakeIndex>>,
        Path(id): Path<u64>,
        Json(next): Json<Volume>,
    ) -> Response {
        let mut i = i.lock().unwrap();
        if i.conflicts > 0 {
            i.conflicts -= 1;
            // Someone else committed first.
            let v = i.volumes.iter_mut().find(|v| v.id == id).unwrap();
            v.generation += 1;
            return StatusCode::CONFLICT.into_response();
        }
        let v = i.volumes.iter_mut().find(|v| v.id == id).unwrap();
        if next.generation != v.generation + 1 {
            return StatusCode::CONFLICT.into_response();
        }
        let size = v.size;
        *v = Volume { size, ..next };
        Json(v.clone()).into_response()
    }
    async fn blocks(
        State(i): State<Shared<FakeIndex>>,
        Query(q): Query<HashMap<String, u64>>,
    ) -> Json<Vec<serde_json::Value>> {
        let i = i.lock().unwrap();
        let rows = i.blocks.iter().filter(|(_, e)| e.volume == q["volume"]);
        Json(
            rows.map(|(h, e)| {
                serde_json::json!({
                    "hash": h.to_string(),
                    "volume": e.volume,
                    "offset": e.offset,
                    "length": e.length,
                })
            })
            .collect(),
        )
    }
    Router::new()
        .route("/blocks", get(blocks))
        .route("/volumes", get(list))
        .route("/volumes/{id}", get(one).put(update))
        .with_state(index)
}

#[derive(Default)]
struct FakeNode {
    down: bool,
    replicas: HashMap<u64, Vec<u8>>,
    /// Every `POST /extents/{volume}`: the volume and its `x-satchel-block`.
    appends: Vec<(u64, String)>,
    fragments: HashMap<(u64, u32), Vec<u8>>,
    checksums: HashMap<(u64, u32), String>,
}

fn fake_node(node: Shared<FakeNode>) -> Router {
    async fn health(State(n): State<Shared<FakeNode>>) -> StatusCode {
        if n.lock().unwrap().down {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::OK
        }
    }
    async fn read(
        State(n): State<Shared<FakeNode>>,
        Path(volume): Path<u64>,
        Query(q): Query<HashMap<String, u64>>,
    ) -> Response {
        let n = n.lock().unwrap();
        if n.down {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        let (o, l) = (q["offset"] as usize, q["length"] as usize);
        match n.replicas.get(&volume) {
            Some(d) if o + l <= d.len() => Bytes::copy_from_slice(&d[o..o + l]).into_response(),
            Some(_) => StatusCode::RANGE_NOT_SATISFIABLE.into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
    /// Appends one block, as the node (#67) does: the header must be the
    /// body's hash, and the answer is the block's offset in the extent.
    async fn append(
        State(n): State<Shared<FakeNode>>,
        Path(volume): Path<u64>,
        headers: HeaderMap,
        body: Bytes,
    ) -> Response {
        let Some(hash) = headers.get("x-satchel-block") else {
            return StatusCode::BAD_REQUEST.into_response();
        };
        let hash = hash.to_str().unwrap().to_owned();
        if hash != BlockHash::of(&body).to_string() {
            return StatusCode::BAD_REQUEST.into_response();
        }
        let mut n = n.lock().unwrap();
        n.appends.push((volume, hash));
        let extent = n.replicas.entry(volume).or_default();
        let offset = extent.len();
        extent.extend_from_slice(&body);
        let body = serde_json::json!({ "offset": offset, "length": body.len() });
        (StatusCode::CREATED, Json(body)).into_response()
    }
    async fn delete_replica(
        State(n): State<Shared<FakeNode>>,
        Path(volume): Path<u64>,
    ) -> StatusCode {
        match n.lock().unwrap().replicas.remove(&volume) {
            Some(_) => StatusCode::NO_CONTENT,
            None => StatusCode::NOT_FOUND,
        }
    }
    /// The node has no whole-replica write.
    async fn put_replica() -> StatusCode {
        StatusCode::METHOD_NOT_ALLOWED
    }
    async fn get_fragment(
        State(n): State<Shared<FakeNode>>,
        Path((v, i)): Path<(u64, u32)>,
    ) -> Response {
        let n = n.lock().unwrap();
        if n.down {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        match n.fragments.get(&(v, i)) {
            Some(d) => Bytes::copy_from_slice(d).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
    async fn put_fragment(
        State(n): State<Shared<FakeNode>>,
        Path((v, i)): Path<(u64, u32)>,
        headers: HeaderMap,
        body: Bytes,
    ) -> StatusCode {
        let sum = headers["x-satchel-checksum"].to_str().unwrap().to_owned();
        let mut n = n.lock().unwrap();
        n.fragments.insert((v, i), body.to_vec());
        n.checksums.insert((v, i), sum);
        StatusCode::CREATED
    }
    Router::new()
        .route("/health", get(health))
        .route(
            "/extents/{volume}",
            get(read)
                .post(append)
                .delete(delete_replica)
                .put(put_replica),
        )
        .route(
            "/extents/{volume}/fragments/{index}",
            get(get_fragment).put(put_fragment),
        )
        .with_state(node)
}

async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

/// A stand-in for RS(6,3) that rebuilds a missing fragment by XOR of the
/// others, enough to test the plumbing. The real codec is tested in #78.
fn xor_codec(fragments: &mut [Option<Vec<u8>>]) -> Result<(), String> {
    let missing: Vec<usize> = (0..fragments.len())
        .filter(|&i| fragments[i].is_none())
        .collect();
    if missing.len() != 1 {
        return Err(format!("{} missing", missing.len()));
    }
    let len = fragments.iter().flatten().next().unwrap().len();
    let mut out = vec![0u8; len];
    for f in fragments.iter().flatten() {
        for (o, b) in out.iter_mut().zip(f) {
            *o ^= b;
        }
    }
    fragments[missing[0]] = Some(out);
    Ok(())
}

struct Cluster {
    index: Shared<FakeIndex>,
    nodes: Vec<Shared<FakeNode>>,
    repairer: Arc<Repairer>,
    config: RepairConfig,
}

impl Cluster {
    fn repairer_config(&self) -> RepairConfig {
        self.config.clone()
    }
}

/// 9 nodes, 3 per zone: node-zZ-K. Volume 1 is open and replicated on
/// node-z1-1, node-z2-1, node-z3-1. Volume 2 is coded over all 9.
async fn cluster(threshold: Duration) -> Cluster {
    let index: Shared<FakeIndex> = Arc::default();
    let mut nodes = Vec::new();
    let mut addrs = Vec::new();
    for z in 1..=3 {
        for k in 1..=3 {
            let n: Shared<FakeNode> = Arc::default();
            addrs.push(NodeAddr {
                id: format!("node-z{z}-{k}"),
                zone: format!("z{z}"),
                url: serve(fake_node(n.clone())).await,
            });
            nodes.push(n);
        }
    }
    // The first 26 bytes are indexed: two blocks, then " blocks," whose
    // index entry was deleted. The tail isn't part of the volume yet.
    let replica = b"0123456789 indexed blocks, then an unindexed tail".to_vec();
    for i in [0, 3, 6] {
        nodes[i].lock().unwrap().replicas.insert(1, replica.clone());
    }
    // Fragments 0..8 whose XOR is zero, so any one can be rebuilt.
    let mut frags: Vec<Vec<u8>> = (0..8u8).map(|f| vec![f + 1; 16]).collect();
    let mut last = vec![0u8; 16];
    for f in &frags {
        for (o, b) in last.iter_mut().zip(f) {
            *o ^= b;
        }
    }
    frags.push(last);
    for (i, f) in frags.into_iter().enumerate() {
        nodes[i].lock().unwrap().fragments.insert((2, i as u32), f);
    }
    {
        let mut idx = index.lock().unwrap();
        for (offset, length) in [(0, 10), (10, 8)] {
            let data = &replica[offset as usize..(offset + length) as usize];
            idx.blocks.push((
                BlockHash::of(data),
                BlockEntry {
                    volume: 1,
                    offset,
                    length,
                },
            ));
        }
        idx.volumes.push(Volume {
            opened_at_secs: 0,
            id: 1,
            state: VolumeState::Open,
            kind: VolumeKind::Replicated,
            generation: 1,
            nodes: ["node-z1-1", "node-z2-1", "node-z3-1"]
                .map(String::from)
                .to_vec(),
            size: 26,
        });
        idx.volumes.push(Volume {
            opened_at_secs: 0,
            id: 2,
            state: VolumeState::Closed,
            kind: VolumeKind::Rs63,
            generation: 2,
            nodes: addrs.iter().map(|n| n.id.clone()).collect(),
            size: 96,
        });
    }
    let config = RepairConfig {
        index_url: serve(fake_index(index.clone())).await,
        nodes: addrs,
        timeout: Duration::from_secs(2),
        repair_after: threshold,
        check_every: Duration::from_millis(50),
    };
    let repairer = Repairer::new(config.clone(), Some(xor_codec));
    Cluster {
        index,
        nodes,
        repairer,
        config,
    }
}

async fn ticks(r: &Repairer, n: usize) {
    for _ in 0..n {
        r.tick().await;
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
}

fn volume(c: &Cluster, id: u64) -> Volume {
    c.index
        .lock()
        .unwrap()
        .volumes
        .iter()
        .find(|v| v.id == id)
        .unwrap()
        .clone()
}

#[tokio::test]
async fn ts_fail_2_a_node_down_past_the_threshold_is_rebuilt_elsewhere() {
    let c = cluster(Duration::from_millis(200)).await;
    // node-z2-1: replica of volume 1, fragment 3 of volume 2.
    c.nodes[3].lock().unwrap().down = true;
    ticks(&c.repairer, 6).await;

    let v1 = volume(&c, 1);
    assert_eq!(v1.state, VolumeState::Closed, "the volume is closed first");
    // Same zone first: node-z2-2 takes the replica.
    assert_eq!(v1.nodes, ["node-z1-1", "node-z2-2", "node-z3-1"]);
    // Closed (gen 2), then the new placement (gen 3).
    assert_eq!(v1.generation, 3);
    // Only the indexed part, the first 26 bytes, is copied, as two blocks
    // and the deleted block's bytes, so the offsets match the index.
    {
        let z22 = c.nodes[4].lock().unwrap();
        assert_eq!(z22.replicas[&1], b"0123456789 indexed blocks,".to_vec());
        let hashes: Vec<String> = z22.appends.iter().map(|(_, h)| h.clone()).collect();
        assert_eq!(
            hashes,
            [
                BlockHash::of(b"0123456789").to_string(),
                BlockHash::of(b" indexed").to_string(),
                BlockHash::of(b" blocks,").to_string(),
            ]
        );
    }

    // Every node holds a fragment of volume 2, so fragment 3 goes to another
    // node in zone z2, which now holds fragments 3 and 4.
    let v2 = volume(&c, 2);
    assert_eq!(v2.nodes[3], "node-z2-2");
    assert_eq!(v2.generation, 3);
    {
        let z22 = c.nodes[4].lock().unwrap();
        assert_eq!(
            z22.fragments[&(2, 3)],
            vec![4u8; 16],
            "the rebuilt fragment"
        );
        assert_eq!(
            z22.checksums[&(2, 3)],
            BlockHash::of(&[4u8; 16]).to_string()
        );
    }

    let stats = c.repairer.stats().await;
    assert_eq!(
        (stats.started, stats.finished, stats.failed),
        (2, 2, 0),
        "{stats:?}"
    );
    assert_eq!(stats.bytes_repaired, 26 + 16);
}

#[tokio::test]
async fn a_node_back_before_the_threshold_triggers_nothing() {
    let c = cluster(Duration::from_millis(500)).await;
    c.nodes[0].lock().unwrap().down = true;
    ticks(&c.repairer, 3).await;
    assert_eq!(c.repairer.stats().await.down.len(), 1);
    c.nodes[0].lock().unwrap().down = false;
    ticks(&c.repairer, 10).await;
    let stats = c.repairer.stats().await;
    assert_eq!(stats.started, 0);
    assert!(stats.down.is_empty());
    assert_eq!(volume(&c, 1).generation, 1);
}

#[tokio::test]
async fn a_node_is_repaired_once_per_outage() {
    let c = cluster(Duration::from_millis(100)).await;
    c.nodes[6].lock().unwrap().down = true;
    ticks(&c.repairer, 8).await;
    assert_eq!(c.repairer.stats().await.started, 2);
    // node-z3-1 has been replaced by node-z3-2 in volume 1.
    assert_eq!(volume(&c, 1).nodes[2], "node-z3-2");
}

#[tokio::test]
async fn a_stale_generation_is_retried_against_the_new_one() {
    let c = cluster(Duration::from_millis(100)).await;
    c.index.lock().unwrap().conflicts = 1;
    c.nodes[0].lock().unwrap().down = true;
    ticks(&c.repairer, 5).await;
    let stats = c.repairer.stats().await;
    assert_eq!(stats.failed, 0, "{stats:?}");
    assert_eq!(volume(&c, 1).nodes[0], "node-z1-2");
}

#[tokio::test]
async fn without_a_codec_coded_volumes_fail_and_say_why() {
    let c = cluster(Duration::from_millis(100)).await;
    let plain = Repairer::new(
        RepairConfig {
            repair_after: Duration::from_millis(100),
            ..c.repairer_config()
        },
        None,
    );
    c.nodes[3].lock().unwrap().down = true;
    ticks(&plain, 5).await;
    let stats = plain.stats().await;
    assert_eq!((stats.finished, stats.failed), (1, 1));
    let failed = stats.history.iter().find(|r| r.error.is_some()).unwrap();
    assert_eq!(failed.volume, 2);
    assert!(failed.error.as_deref().unwrap().contains("codec"));
}

#[tokio::test]
async fn a_corrupt_source_is_skipped_and_leftovers_on_the_target_are_cleared() {
    let c = cluster(Duration::from_millis(100)).await;
    // node-z1-1, the first source, has a flipped byte in its first block.
    c.nodes[0].lock().unwrap().replicas.get_mut(&1).unwrap()[2] = b'X';
    // node-z2-2, the target, holds half of an earlier attempt.
    c.nodes[4]
        .lock()
        .unwrap()
        .replicas
        .insert(1, b"0123456789".to_vec());
    c.nodes[3].lock().unwrap().down = true;
    ticks(&c.repairer, 6).await;

    let stats = c.repairer.stats().await;
    assert_eq!(stats.failed, 0, "{stats:?}");
    assert_eq!(volume(&c, 1).nodes[1], "node-z2-2");
    // Copied from node-z3-1, starting from an empty extent.
    assert_eq!(
        c.nodes[4].lock().unwrap().replicas[&1],
        b"0123456789 indexed blocks,".to_vec()
    );
}
