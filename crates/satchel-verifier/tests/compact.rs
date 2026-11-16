//! `satchel compact` (#110) against the real Block Index, three real storage
//! nodes (one per zone, like the local install) and the real frontend, all in
//! process over HTTP.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use satchel_common::cluster::{NodeAddr, Volume, VolumeKind, VolumeState};
use satchel_common::service::ServiceConfig;
use satchel_common::BlockHash;
use satchel_verifier::master::{Master, MasterConfig};
use serde::Deserialize;

struct Cluster {
    index: String,
    api: String,
    nodes: Vec<NodeAddr>,
    dirs: Vec<PathBuf>,
    http: reqwest::Client,
}

impl Drop for Cluster {
    fn drop(&mut self) {
        for dir in &self.dirs {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

#[derive(Debug, Deserialize)]
struct Extent {
    volume: u64,
    kind: String,
}

async fn serve(app: axum::Router) -> (String, SocketAddr) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), addr)
}

fn temp_dir() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "satchel-compact-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn cluster() -> Cluster {
    let store = satchel_index::Store::open_in_memory().unwrap();
    let (index, _) = serve(satchel_index::router(store)).await;
    let mut nodes = Vec::new();
    let mut dirs = Vec::new();
    for zone in ["z1", "z2", "z3"] {
        let dir = temp_dir();
        let config = ServiceConfig {
            name: "satchel-node",
            addr: "127.0.0.1:0".parse().unwrap(),
            zone: Some(zone.into()),
        };
        let store = satchel_node::NodeStore::open(&dir).unwrap();
        let (url, _) = serve(satchel_node::router(&config, store)).await;
        nodes.push(NodeAddr {
            id: format!("node-{zone}"),
            zone: zone.into(),
            url,
        });
        dirs.push(dir);
    }
    let (api, _) = serve(satchel_api::router(satchel_api::Config {
        index_url: index.clone(),
        nodes: nodes.clone(),
        zone: None,
        timeout: Duration::from_secs(5),
    }))
    .await;
    let c = Cluster {
        index,
        api,
        nodes,
        dirs,
        http: reqwest::Client::new(),
    };
    c.master().reconcile().await.unwrap();
    c
}

impl Cluster {
    /// A Master that never closes a volume on its own, so each test decides.
    fn master(&self) -> Master {
        self.master_with(self.nodes.clone())
    }

    fn master_with(&self, nodes: Vec<NodeAddr>) -> Master {
        Master::new(MasterConfig {
            index_url: self.index.clone(),
            nodes,
            close_bytes: u64::MAX,
            close_after: Duration::from_secs(3600),
        })
    }

    async fn put(&self, seed: u8, len: usize) -> (BlockHash, Vec<u8>) {
        let data: Vec<u8> = (0..len)
            .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
            .collect();
        let hash = BlockHash::of(&data);
        let status = self
            .http
            .put(format!("{}/blocks/{hash}", self.api))
            .body(data.clone())
            .send()
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "put {status}");
        (hash, data)
    }

    async fn get(&self, hash: &BlockHash) -> (u16, Vec<u8>) {
        let r = self
            .http
            .get(format!("{}/blocks/{hash}", self.api))
            .send()
            .await
            .unwrap();
        (r.status().as_u16(), r.bytes().await.unwrap().to_vec())
    }

    async fn delete(&self, hash: &BlockHash) {
        let status = self
            .http
            .delete(format!("{}/blocks/{hash}", self.api))
            .send()
            .await
            .unwrap()
            .status();
        assert!(status.is_success(), "delete {status}");
    }

    async fn volumes(&self) -> Vec<Volume> {
        self.http
            .get(format!("{}/volumes", self.index))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    /// Every extent on every node, as (volume, "replica" or "fragment").
    async fn extents(&self) -> Vec<(u64, String)> {
        let mut all = Vec::new();
        for node in &self.nodes {
            let listed: Vec<Extent> = self
                .http
                .get(format!("{}/extents", node.url))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            all.extend(listed.into_iter().map(|e| (e.volume, e.kind)));
        }
        all
    }

    /// Opens a volume and closes it before anything is written, as the
    /// Master does with a volume that sat idle for ten minutes.
    async fn empty_closed_volume(&self) -> u64 {
        let ids: Vec<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        let mut volume: Volume = self
            .http
            .post(format!("{}/volumes", self.index))
            .json(&serde_json::json!({ "nodes": ids }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        volume.state = VolumeState::Closed;
        volume.generation += 1;
        let status = self
            .http
            .put(format!("{}/volumes/{}", self.index, volume.id))
            .json(&volume)
            .send()
            .await
            .unwrap()
            .status();
        assert!(status.is_success());
        volume.id
    }
}

#[tokio::test]
async fn compaction_retires_empty_volumes_and_codes_written_ones() {
    let c = cluster().await;
    let empty = c.empty_closed_volume().await;
    let blocks = [c.put(1, 70_000).await, c.put(2, 5_000).await];

    let report = c.master().compact().await.unwrap();

    assert_eq!(report.volumes_coded, 1);
    let volumes = c.volumes().await;
    assert!(volumes.iter().all(|v| v.id != empty), "{volumes:?}");
    let coded: Vec<_> = volumes
        .iter()
        .filter(|v| v.kind == VolumeKind::Rs63)
        .collect();
    assert_eq!(coded.len(), 1);
    assert_eq!(coded[0].state, VolumeState::Closed);
    for (hash, data) in &blocks {
        assert_eq!(c.get(hash).await, (200, data.clone()));
    }
    // The replicas are gone: only fragments of the coded volume remain.
    let extents = c.extents().await;
    assert_eq!(extents.len(), 9, "{extents:?}");
    assert!(extents
        .iter()
        .all(|(v, kind)| *v == coded[0].id && kind == "fragment"));
}

#[tokio::test]
async fn deletes_in_a_coded_volume_are_reclaimed_by_the_next_compaction() {
    let c = cluster().await;
    let keep = c.put(3, 40_000).await;
    let gone = c.put(4, 60_000).await;
    c.master().compact().await.unwrap();
    let first = c
        .volumes()
        .await
        .into_iter()
        .find(|v| v.kind == VolumeKind::Rs63)
        .unwrap();

    c.delete(&gone.0).await;
    let report = c.master().compact().await.unwrap();

    assert_eq!(report.volumes_coded, 1);
    assert!(report.bytes_reclaimed > 0);
    assert_eq!(c.get(&keep.0).await, (200, keep.1.clone()));
    assert_eq!(c.get(&gone.0).await.0, 404);
    let volumes = c.volumes().await;
    assert!(volumes.iter().all(|v| v.id != first.id), "{volumes:?}");
    assert!(c.extents().await.iter().all(|(v, _)| *v != first.id));

    // Deleting the last live block empties the coded volume, and the next
    // compaction removes it and its fragments entirely.
    c.delete(&keep.0).await;
    c.master().compact().await.unwrap();
    assert!(c.volumes().await.iter().all(|v| v.kind != VolumeKind::Rs63));
    assert_eq!(c.extents().await, Vec::new());
}

#[tokio::test]
async fn uploads_after_a_compaction_stay_replicated_until_the_next_one() {
    let c = cluster().await;
    let old = c.put(5, 10_000).await;
    c.master().compact().await.unwrap();
    let new = c.put(6, 10_000).await;

    let replicas = c
        .extents()
        .await
        .into_iter()
        .filter(|(_, kind)| kind == "replica")
        .count();
    assert_eq!(replicas, 3);
    assert_eq!(c.get(&old.0).await, (200, old.1.clone()));
    assert_eq!(c.get(&new.0).await, (200, new.1.clone()));

    c.master().compact().await.unwrap();
    assert!(c.extents().await.iter().all(|(_, kind)| kind == "fragment"));
    assert_eq!(c.get(&new.0).await, (200, new.1));
}

#[tokio::test]
async fn a_second_compaction_is_refused_while_one_runs() {
    let c = cluster().await;
    let block = c.put(7, 20_000).await;
    let master = c.master();

    let (a, b) = tokio::join!(master.compact(), master.compact());

    assert!(a.is_ok(), "{a:?}");
    let refused = b.unwrap_err().to_string();
    assert!(refused.contains("already running"), "{refused}");
    assert_eq!(c.get(&block.0).await, (200, block.1));
}

#[tokio::test]
async fn compaction_leaves_data_replicated_when_a_zone_is_down() {
    let c = cluster().await;
    let block = c.put(8, 20_000).await;
    let mut nodes = c.nodes.clone();
    nodes[2].url = "http://127.0.0.1:9".into();

    let error = c.master_with(nodes).compact().await.unwrap_err();

    assert!(error.to_string().contains("all three zones"), "{error}");
    assert!(c
        .volumes()
        .await
        .iter()
        .all(|v| v.kind == VolumeKind::Replicated));
    assert_eq!(c.get(&block.0).await, (200, block.1));
}
