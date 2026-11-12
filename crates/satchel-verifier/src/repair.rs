//! Repair (#71): rebuilds what a node held once it has been unreachable for
//! longer than `SATCHEL_REPAIR_AFTER`.
//!
//! Magic Pocket counterpart: repair after an OSD is offline for 15 minutes.
//! The Master closes the OSD's volumes, rebuilds them elsewhere and commits
//! each move through the volume's generation [B2]. Decision Log: DDL-010.
//! Satchel rebuilds each volume from one node, not spread over hundreds, and
//! has no Reconcile step to roll back a half-done repair.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

use satchel_common::cluster::{NodeAddr, Volume, VolumeKind, VolumeState};
use satchel_common::BlockHash;
use serde::Serialize;
use tokio::sync::Mutex;

/// Rebuilds the `None` slots of 9 fragments from any 6 others. Passed in
/// so this module doesn't depend on the codec's crate: `main` gives it
/// `satchel_common::ec::reconstruct` (#78).
pub type Reconstruct = fn(&mut [Option<Vec<u8>>]) -> Result<(), String>;

#[derive(Debug, Clone)]
pub struct RepairConfig {
    pub index_url: String,
    pub nodes: Vec<NodeAddr>,
    pub timeout: Duration,
    /// How long a node must be unreachable before it's rebuilt (DDL-010).
    pub repair_after: Duration,
    /// How often node health is polled.
    pub check_every: Duration,
}

/// Parses `SATCHEL_REPAIR_AFTER`: seconds, or a number ending in `s` or
/// `m`, for example `30s` or `15m`.
pub fn parse_duration(raw: &str) -> Result<Duration, String> {
    let raw = raw.trim();
    let (number, scale) = match raw.strip_suffix('m') {
        Some(n) => (n, 60),
        None => (raw.strip_suffix('s').unwrap_or(raw), 1),
    };
    number
        .parse::<u64>()
        .map(|n| Duration::from_secs(n * scale))
        .map_err(|_| format!("{raw:?} is not a duration like 30s or 15m"))
}

/// One volume repaired, or why it couldn't be.
#[derive(Debug, Clone, Serialize)]
pub struct RepairRecord {
    pub volume: u64,
    pub failed_node: String,
    pub new_node: Option<String>,
    pub bytes: u64,
    pub millis: u64,
    pub error: Option<String>,
}

/// What `GET /repairs` returns. The same numbers become metrics once #21
/// lands: repairs started and finished, bytes repaired, time to repair.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RepairStats {
    pub started: u64,
    pub finished: u64,
    pub failed: u64,
    pub bytes_repaired: u64,
    /// Nodes that are down now, with how long they've been down.
    pub down: BTreeMap<String, u64>,
    pub history: Vec<RepairRecord>,
}

#[derive(Default)]
struct NodeHealth {
    down_since: Option<Instant>,
    /// Set once the node's data has been rebuilt for this outage.
    repaired: bool,
}

pub struct Repairer {
    config: RepairConfig,
    http: reqwest::Client,
    reconstruct: Option<Reconstruct>,
    health: Mutex<HashMap<String, NodeHealth>>,
    stats: Mutex<RepairStats>,
}

fn err(what: &str) -> impl Fn(reqwest::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

impl Repairer {
    pub fn new(config: RepairConfig, reconstruct: Option<Reconstruct>) -> Arc<Self> {
        Arc::new(Self {
            http: reqwest::Client::builder()
                .timeout(config.timeout)
                .build()
                .expect("HTTP client builds"),
            config,
            reconstruct,
            health: Mutex::default(),
            stats: Mutex::default(),
        })
    }

    pub async fn stats(&self) -> RepairStats {
        self.stats.lock().await.clone()
    }

    async fn is_up(&self, node: &NodeAddr) -> bool {
        matches!(
            self.http.get(format!("{}/health", node.url)).send().await,
            Ok(r) if r.status().is_success()
        )
    }

    /// Polls every node once, and repairs any that has now been down for
    /// longer than the threshold. A node that comes back first triggers
    /// nothing.
    pub async fn tick(&self) {
        let now = Instant::now();
        let mut due = Vec::new();
        let mut down = BTreeMap::new();
        for node in &self.config.nodes {
            let up = self.is_up(node).await;
            let mut health = self.health.lock().await;
            let h = health.entry(node.id.clone()).or_default();
            if up {
                if h.down_since.is_some() {
                    tracing::info!(node = %node.id, "node is back");
                }
                *h = NodeHealth::default();
                continue;
            }
            let since = *h.down_since.get_or_insert(now);
            down.insert(node.id.clone(), now.duration_since(since).as_secs());
            if !h.repaired && now.duration_since(since) >= self.config.repair_after {
                h.repaired = true;
                due.push(node.clone());
            }
        }
        self.stats.lock().await.down = down;
        for node in due {
            tracing::warn!(node = %node.id, "node down past the repair threshold, rebuilding");
            self.repair_node(&node).await;
        }
    }

    pub async fn run_forever(self: Arc<Self>) {
        loop {
            self.tick().await;
            tokio::time::sleep(self.config.check_every).await;
        }
    }

    async fn volumes(&self) -> Result<Vec<Volume>, String> {
        let url = format!("{}/volumes", self.config.index_url);
        let r = self.http.get(&url).send().await.map_err(err(&url))?;
        if !r.status().is_success() {
            return Err(format!("{url}: HTTP {}", r.status()));
        }
        r.json().await.map_err(err(&url))
    }

    async fn get_volume(&self, id: u64) -> Result<Volume, String> {
        let url = format!("{}/volumes/{id}", self.config.index_url);
        let r = self.http.get(&url).send().await.map_err(err(&url))?;
        if !r.status().is_success() {
            return Err(format!("{url}: HTTP {}", r.status()));
        }
        r.json().await.map_err(err(&url))
    }

    /// Commits `change` to the volume's latest generation, re-reading and
    /// retrying once if another writer got there first (DDL-010).
    async fn commit(&self, id: u64, change: impl Fn(&mut Volume)) -> Result<Volume, String> {
        for _ in 0..2 {
            let mut v = self.get_volume(id).await?;
            change(&mut v);
            v.generation += 1;
            let url = format!("{}/volumes/{id}", self.config.index_url);
            let r = self
                .http
                .put(&url)
                .json(&v)
                .send()
                .await
                .map_err(err(&url))?;
            match r.status().as_u16() {
                200 => return r.json().await.map_err(err(&url)),
                409 => continue,
                s => return Err(format!("{url}: HTTP {s}")),
            }
        }
        Err(format!("volume {id}: generation kept changing"))
    }

    fn node(&self, id: &str) -> Option<&NodeAddr> {
        self.config.nodes.iter().find(|n| n.id == id)
    }

    /// A healthy node for the lost copy: same zone first, and one the volume
    /// doesn't use yet. A coded volume keeps at most 3 fragments in a zone
    /// (DDL-007). On 9 nodes every node already holds a fragment, so a coded
    /// volume falls back to a healthy node in the failed node's zone, which
    /// then holds two fragments: a zone loss is still survivable, but that
    /// node's loss now costs 2 of the 3 spare fragments (our inference).
    async fn pick_target(&self, volume: &Volume, failed: &NodeAddr) -> Option<NodeAddr> {
        let health = self.health.lock().await;
        let healthy = |n: &&NodeAddr| health.get(&n.id).is_none_or(|h| h.down_since.is_none());
        let per_zone = |zone: &str| {
            volume
                .nodes
                .iter()
                .filter(|id| *id != &failed.id)
                .filter_map(|id| self.node(id))
                .filter(|n| n.zone == zone)
                .count()
        };
        let limit = match volume.kind {
            VolumeKind::Replicated => 1,
            VolumeKind::Rs63 => 3,
        };
        let mut candidates: Vec<&NodeAddr> = self
            .config
            .nodes
            .iter()
            .filter(|n| !volume.nodes.contains(&n.id))
            .filter(healthy)
            .collect();
        // Same zone first, then zones with room, then anything healthy.
        candidates.sort_by_key(|n| {
            (
                n.zone != failed.zone,
                per_zone(&n.zone) >= limit,
                n.id.clone(),
            )
        });
        if let Some(n) = candidates.first() {
            return Some((*n).clone());
        }
        if volume.kind == VolumeKind::Rs63 {
            return self
                .config
                .nodes
                .iter()
                .filter(|n| n.zone == failed.zone && n.id != failed.id)
                .find(healthy)
                .cloned();
        }
        None
    }

    async fn repair_node(&self, failed: &NodeAddr) {
        let volumes = match self.volumes().await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("repair of {}: can't list volumes: {e}", failed.id);
                return;
            }
        };
        for v in volumes.into_iter().filter(|v| v.nodes.contains(&failed.id)) {
            self.stats.lock().await.started += 1;
            let start = Instant::now();
            let result = self.repair_volume(v.id, failed).await;
            let mut stats = self.stats.lock().await;
            let record = match result {
                Ok((node, bytes)) => {
                    stats.finished += 1;
                    stats.bytes_repaired += bytes;
                    tracing::info!(volume = v.id, from = %failed.id, to = %node, bytes, "repaired");
                    RepairRecord {
                        volume: v.id,
                        failed_node: failed.id.clone(),
                        new_node: Some(node),
                        bytes,
                        millis: start.elapsed().as_millis() as u64,
                        error: None,
                    }
                }
                Err(e) => {
                    stats.failed += 1;
                    tracing::error!(volume = v.id, node = %failed.id, "repair failed: {e}");
                    RepairRecord {
                        volume: v.id,
                        failed_node: failed.id.clone(),
                        new_node: None,
                        bytes: 0,
                        millis: start.elapsed().as_millis() as u64,
                        error: Some(e),
                    }
                }
            };
            stats.history.push(record);
        }
    }

    /// Closes the volume if it's open, rebuilds the lost copy on another
    /// node and commits the new placement. Returns the node and bytes written.
    async fn repair_volume(&self, id: u64, failed: &NodeAddr) -> Result<(String, u64), String> {
        // No more writes to a volume that has lost a copy (DDL-010).
        let mut v = self.get_volume(id).await?;
        if v.state == VolumeState::Open {
            v = self.commit(id, |v| v.state = VolumeState::Closed).await?;
        }
        let position = v
            .nodes
            .iter()
            .position(|n| n == &failed.id)
            .ok_or("the node is no longer in the volume")?;
        let target = self
            .pick_target(&v, failed)
            .await
            .ok_or("no healthy node to rebuild on")?;
        let bytes = match v.kind {
            VolumeKind::Replicated => self.copy_replica(&v, failed, &target).await?,
            VolumeKind::Rs63 => self.rebuild_fragment(&v, position, &target).await?,
        };
        let new_id = target.id.clone();
        self.commit(id, |v| {
            if v.nodes.get(position) == Some(&failed.id) {
                v.nodes[position] = new_id.clone();
            }
        })
        .await?;
        Ok((target.id, bytes))
    }

    /// Copies the first `size` bytes of a healthy replica, the indexed part,
    /// to `target` with `PUT /extents/{volume}`.
    async fn copy_replica(
        &self,
        v: &Volume,
        failed: &NodeAddr,
        target: &NodeAddr,
    ) -> Result<u64, String> {
        let mut last = String::from("no other replica");
        for source in v
            .nodes
            .iter()
            .filter(|id| *id != &failed.id)
            .filter_map(|id| self.node(id))
        {
            let url = format!("{}/extents/{}?offset=0&length={}", source.url, v.id, v.size);
            let bytes = match self.http.get(&url).send().await {
                Ok(r) if r.status().is_success() => r.bytes().await.map_err(err(&url))?,
                Ok(r) => {
                    last = format!("{url}: HTTP {}", r.status());
                    continue;
                }
                Err(e) => {
                    last = format!("{url}: {e}");
                    continue;
                }
            };
            if bytes.len() as u64 != v.size {
                last = format!("{url}: got {} bytes, wanted {}", bytes.len(), v.size);
                continue;
            }
            let put = format!("{}/extents/{}", target.url, v.id);
            let r = self
                .http
                .put(&put)
                .body(bytes)
                .send()
                .await
                .map_err(err(&put))?;
            if r.status().as_u16() != 201 {
                return Err(format!("{put}: HTTP {}", r.status()));
            }
            return Ok(v.size);
        }
        Err(last)
    }

    /// Reads the surviving fragments, rebuilds fragment `index` and stores
    /// it on `target` with its checksum (#67).
    async fn rebuild_fragment(
        &self,
        v: &Volume,
        index: usize,
        target: &NodeAddr,
    ) -> Result<u64, String> {
        let reconstruct = self
            .reconstruct
            .ok_or("no erasure coding codec configured (#78)")?;
        let mut fragments: Vec<Option<Vec<u8>>> = vec![None; v.nodes.len()];
        for (i, id) in v.nodes.iter().enumerate() {
            if i == index {
                continue;
            }
            let Some(node) = self.node(id) else { continue };
            let url = format!("{}/extents/{}/fragments/{i}", node.url, v.id);
            if let Ok(r) = self.http.get(&url).send().await {
                if r.status().is_success() {
                    if let Ok(b) = r.bytes().await {
                        fragments[i] = Some(b.to_vec());
                    }
                }
            }
        }
        reconstruct(&mut fragments)?;
        let rebuilt = fragments[index]
            .take()
            .ok_or("the codec returned no fragment")?;
        let len = rebuilt.len() as u64;
        let put = format!("{}/extents/{}/fragments/{index}", target.url, v.id);
        let r = self
            .http
            .put(&put)
            .header("x-satchel-checksum", BlockHash::of(&rebuilt).to_string())
            .body(rebuilt)
            .send()
            .await
            .map_err(err(&put))?;
        if r.status().as_u16() != 201 {
            return Err(format!("{put}: HTTP {}", r.status()));
        }
        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repair_thresholds() {
        assert_eq!(parse_duration("30s"), Ok(Duration::from_secs(30)));
        assert_eq!(parse_duration("15m"), Ok(Duration::from_secs(900)));
        assert_eq!(parse_duration("45"), Ok(Duration::from_secs(45)));
        assert!(parse_duration("soon").is_err());
        assert!(parse_duration("1h").is_err());
    }
}
