//! The verifier against in-process fakes of the index (#66) and the storage
//! nodes (#67), over real HTTP.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use satchel_common::cluster::{NodeAddr, Volume, VolumeKind, VolumeState};
use satchel_common::BlockHash;
use satchel_verifier::scan::Check;
use satchel_verifier::{Config, Verifier};
use serde_json::{json, Value};

#[derive(Default)]
struct FakeIndex {
    volumes: Vec<Volume>,
    /// (hash, volume, offset, length)
    blocks: Vec<(BlockHash, u64, u64, u64)>,
}

#[derive(Default)]
struct FakeNode {
    /// Replica extents by volume.
    replicas: HashMap<u64, Vec<u8>>,
    /// Fragments by (volume, index).
    fragments: HashMap<(u64, u32), Vec<u8>>,
    /// Ranges the node refuses to serve, as if its inline hash failed (TS-NODE-5).
    bad_ranges: Vec<(u64, u64, u64)>,
    /// Fragments that fail their checksum on the node.
    bad_fragments: Vec<(u64, u32)>,
}

type Shared<T> = Arc<Mutex<T>>;

fn fake_index(index: Shared<FakeIndex>) -> Router {
    async fn volumes(State(i): State<Shared<FakeIndex>>) -> Json<Vec<Volume>> {
        Json(i.lock().unwrap().volumes.clone())
    }
    async fn blocks(
        State(i): State<Shared<FakeIndex>>,
        Query(q): Query<HashMap<String, String>>,
    ) -> Json<Value> {
        let v: u64 = q["volume"].parse().unwrap();
        let rows: Vec<Value> = i
            .lock()
            .unwrap()
            .blocks
            .iter()
            .filter(|b| b.1 == v)
            .map(|b| json!({"hash": b.0, "volume": b.1, "offset": b.2, "length": b.3}))
            .collect();
        Json(Value::Array(rows))
    }
    Router::new()
        .route("/volumes", get(volumes))
        .route("/blocks", get(blocks))
        .with_state(index)
}

fn fake_node(node: Shared<FakeNode>) -> Router {
    async fn extents(State(n): State<Shared<FakeNode>>) -> Json<Value> {
        let n = n.lock().unwrap();
        let mut out: Vec<Value> = n
            .replicas
            .iter()
            .map(|(v, d)| json!({"volume": v, "kind": "replica", "size": d.len()}))
            .collect();
        out.extend(n.fragments.iter().map(
            |((v, i), d)| json!({"volume": v, "kind": "fragment", "index": i, "size": d.len()}),
        ));
        Json(Value::Array(out))
    }
    async fn read(
        State(n): State<Shared<FakeNode>>,
        Path(volume): Path<u64>,
        Query(q): Query<HashMap<String, u64>>,
    ) -> Response {
        let n = n.lock().unwrap();
        let (offset, length) = (q["offset"], q["length"]);
        if n.bad_ranges.contains(&(volume, offset, length)) {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        match n.replicas.get(&volume) {
            Some(d) if (offset + length) as usize <= d.len() => {
                Bytes::copy_from_slice(&d[offset as usize..(offset + length) as usize])
                    .into_response()
            }
            Some(_) => StatusCode::RANGE_NOT_SATISFIABLE.into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
    async fn fragment(
        State(n): State<Shared<FakeNode>>,
        Path((volume, index)): Path<(u64, u32)>,
    ) -> Response {
        let n = n.lock().unwrap();
        if n.bad_fragments.contains(&(volume, index)) {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        match n.fragments.get(&(volume, index)) {
            Some(d) => Bytes::copy_from_slice(d).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
    Router::new()
        .route("/extents", get(extents))
        .route("/extents/{volume}", get(read))
        .route("/extents/{volume}/fragments/{index}", get(fragment))
        .with_state(node)
}

async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

struct Cluster {
    index: Shared<FakeIndex>,
    nodes: Vec<Shared<FakeNode>>,
    verifier: Arc<Verifier>,
    url: String,
}

/// One replicated volume (1) on nodes 0-2 with two blocks, and one coded
/// volume (2) with its 9 fragments on nodes 0-8.
async fn cluster() -> Cluster {
    let index: Shared<FakeIndex> = Arc::default();
    let nodes: Vec<Shared<FakeNode>> = (0..9).map(|_| Arc::default()).collect();
    let mut addrs = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        addrs.push(NodeAddr {
            id: format!("node-{i}"),
            zone: format!("z{}", i % 3 + 1),
            url: serve(fake_node(n.clone())).await,
        });
    }
    let a = b"first block".to_vec();
    let b = b"second, longer block".to_vec();
    let extent: Vec<u8> = [a.clone(), b.clone()].concat();
    {
        let mut i = index.lock().unwrap();
        i.volumes.push(Volume {
            id: 1,
            state: VolumeState::Open,
            kind: VolumeKind::Replicated,
            generation: 1,
            nodes: (0..3).map(|n| format!("node-{n}")).collect(),
            size: extent.len() as u64,
        });
        i.blocks.push((BlockHash::of(&a), 1, 0, a.len() as u64));
        i.blocks
            .push((BlockHash::of(&b), 1, a.len() as u64, b.len() as u64));
        i.volumes.push(Volume {
            id: 2,
            state: VolumeState::Closed,
            kind: VolumeKind::Rs63,
            generation: 2,
            nodes: (0..9).map(|n| format!("node-{n}")).collect(),
            size: 60,
        });
        i.blocks.push((BlockHash::of(b"coded"), 2, 0, 5));
    }
    for n in &nodes[..3] {
        n.lock().unwrap().replicas.insert(1, extent.clone());
    }
    for (f, n) in nodes.iter().enumerate() {
        n.lock()
            .unwrap()
            .fragments
            .insert((2, f as u32), vec![f as u8; 10]);
    }
    let verifier = Verifier::new(Config {
        index_url: serve(fake_index(index.clone())).await,
        nodes: addrs,
        timeout: Duration::from_secs(2),
        every: Duration::from_secs(3600),
    });
    let url = serve(satchel_verifier::router(verifier.clone())).await;
    Cluster {
        index,
        nodes,
        verifier,
        url,
    }
}

#[tokio::test]
async fn ts_ver_3_a_clean_cluster_reports_nothing() {
    let c = cluster().await;
    let pass = c.verifier.run_once().await;
    assert!(pass.findings.is_empty(), "{:?}", pass.findings);
    assert!(pass.skipped.is_empty(), "{:?}", pass.skipped);
    // 2 blocks on each of 3 replicas, 9 fragments, 3 index entries.
    assert_eq!(pass.blocks_scrubbed, 6);
    assert_eq!(pass.fragments_scrubbed, 9);
    assert_eq!(pass.entries_scanned, 3);
}

#[tokio::test]
async fn ts_ver_1_a_block_corrupted_on_disk_is_reported() {
    let c = cluster().await;
    // Flip a byte of the second block on node-1.
    c.nodes[1].lock().unwrap().replicas.get_mut(&1).unwrap()[15] ^= 0xff;
    // And make node-2 refuse the first block, as its inline hash check would.
    c.nodes[2].lock().unwrap().bad_ranges.push((1, 0, 11));
    let pass = c.verifier.run_once().await;
    let found: Vec<(String, String)> = pass
        .findings
        .iter()
        .map(|f| {
            assert_eq!(f.check, Check::Scrubber);
            (f.node.clone(), f.hash.unwrap().to_string())
        })
        .collect();
    assert_eq!(
        found,
        [
            (
                "node-1".to_owned(),
                BlockHash::of(b"second, longer block").to_string()
            ),
            (
                "node-2".to_owned(),
                BlockHash::of(b"first block").to_string()
            ),
        ]
    );
    assert!(pass.findings[0].problem.contains("hash"));
    assert!(pass.findings[1].problem.contains("HTTP 500"));
}

#[tokio::test]
async fn ts_ver_2_a_block_a_node_is_missing_is_reported() {
    let c = cluster().await;
    c.nodes[0].lock().unwrap().replicas.remove(&1);
    // node-1's replica is cut short: it lost the second block.
    c.nodes[1]
        .lock()
        .unwrap()
        .replicas
        .get_mut(&1)
        .unwrap()
        .truncate(11);
    c.nodes[7].lock().unwrap().fragments.remove(&(2, 7));
    let pass = c.verifier.run_once().await;
    let metadata: Vec<(String, Option<u32>, Option<String>)> = pass
        .findings
        .iter()
        .filter(|f| f.check == Check::Metadata)
        .map(|f| (f.node.clone(), f.fragment, f.hash.map(|h| h.to_string())))
        .collect();
    let first = BlockHash::of(b"first block").to_string();
    let second = BlockHash::of(b"second, longer block").to_string();
    assert_eq!(
        metadata,
        [
            ("node-0".to_owned(), None, Some(first)),
            ("node-0".to_owned(), None, Some(second.clone())),
            ("node-1".to_owned(), None, Some(second)),
            ("node-7".to_owned(), Some(7), None),
        ]
    );
}

#[tokio::test]
async fn ts_ver_4_findings_carry_hash_and_node_and_count_by_check() {
    let c = cluster().await;
    c.nodes[1].lock().unwrap().replicas.get_mut(&1).unwrap()[0] ^= 1;
    c.nodes[2].lock().unwrap().replicas.remove(&1);
    let client = reqwest::Client::new();
    let pass: Value = client
        .post(format!("{}/verify", c.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let f = &pass["findings"][0];
    assert_eq!(f["check"], "scrubber");
    assert_eq!(f["node"], "node-1");
    assert_eq!(f["volume"], 1);
    assert_eq!(f["hash"], BlockHash::of(b"first block").to_string());

    let report: Value = client
        .get(format!("{}/findings", c.url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(report["passes"], 1);
    assert_eq!(report["findings_total"]["scrubber"], 1);
    assert_eq!(report["findings_total"]["metadata"], 2);
    // A second pass adds to the totals.
    c.verifier.run_once().await;
    let totals = c.verifier.report().await.findings_total;
    assert_eq!((totals["scrubber"], totals["metadata"]), (2, 4));
}

#[tokio::test]
async fn a_corrupt_fragment_is_reported_with_its_index() {
    let c = cluster().await;
    c.nodes[4].lock().unwrap().bad_fragments.push((2, 4));
    let pass = c.verifier.run_once().await;
    assert_eq!(pass.findings.len(), 1, "{:?}", pass.findings);
    let f = &pass.findings[0];
    assert_eq!(
        (f.check, f.node.as_str(), f.volume, f.fragment),
        (Check::Scrubber, "node-4", 2, Some(4))
    );
    assert!(f.problem.contains("HTTP 500"));
}

#[tokio::test]
async fn a_volume_on_an_unknown_node_is_reported() {
    let c = cluster().await;
    c.index.lock().unwrap().volumes[1].nodes[8] = "node-gone".into();
    let pass = c.verifier.run_once().await;
    assert_eq!(pass.findings.len(), 1, "{:?}", pass.findings);
    assert_eq!(pass.findings[0].node, "node-gone");
}

#[tokio::test]
async fn an_unreachable_node_is_skipped_not_a_finding() {
    let c = cluster().await;
    // The same index, but node-0 is a closed port.
    let verifier = Verifier::new(Config {
        index_url: serve(fake_index(c.index.clone())).await,
        nodes: vec![NodeAddr {
            id: "node-0".into(),
            zone: "z1".into(),
            url: "http://127.0.0.1:9".into(),
        }],
        timeout: Duration::from_millis(500),
        every: Duration::from_secs(3600),
    });
    let pass = verifier.run_once().await;
    assert!(
        pass.findings.iter().all(|f| f.node != "node-0"),
        "{:?}",
        pass.findings
    );
    assert!(pass.skipped.iter().any(|s| s.starts_with("node-0")));
}
