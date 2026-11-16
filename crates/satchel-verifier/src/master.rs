//! The Master keeps a usable pool of replicated volumes. Manual compaction
//! encodes closed volumes after it has reclaimed dead blocks.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context};
use satchel_common::cluster::{parse_nodes, NodeAddr, Volume, VolumeKind, VolumeState};
use satchel_common::ec;
use satchel_common::BlockHash;
use serde::Deserialize;

use crate::clients::ListedBlock;

#[derive(Clone, Debug)]
pub struct MasterConfig {
    pub index_url: String,
    pub nodes: Vec<NodeAddr>,
    pub close_bytes: u64,
    pub close_after: Duration,
}

impl MasterConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let index_url =
            std::env::var("SATCHEL_INDEX_URL").unwrap_or_else(|_| "http://index:8081".into());
        let nodes = parse_nodes(&std::env::var("SATCHEL_NODES").unwrap_or_default())
            .map_err(anyhow::Error::msg)?;
        let close_bytes = env_u64("SATCHEL_VOLUME_BYTES", 64 * 1024 * 1024)?;
        let close_after = Duration::from_secs(env_u64("SATCHEL_VOLUME_SECS", 10 * 60)?);
        Ok(Self {
            index_url: index_url.trim_end_matches('/').into(),
            nodes,
            close_bytes,
            close_after,
        })
    }
}

fn env_u64(name: &str, default: u64) -> anyhow::Result<u64> {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .with_context(|| format!("{name} must be an integer")),
        Err(_) => Ok(default),
    }
}

#[derive(Clone)]
pub struct Master {
    config: MasterConfig,
    http: reqwest::Client,
}

impl Master {
    pub fn new(config: MasterConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .expect("HTTP client builds");
        Self { config, http }
    }

    /// One restart-safe reconciliation pass. Call it periodically; no mutable
    /// Master state is retained between calls.
    pub async fn reconcile(&self) -> anyhow::Result<()> {
        let healthy = self.healthy_nodes().await;
        if healthy.is_empty() {
            bail!("no storage nodes are healthy");
        }
        let open = self.open_volumes().await?;
        for volume in &open {
            if self.should_close(volume) {
                self.close(volume, &healthy).await?;
            }
        }
        if self.open_volumes().await?.is_empty() {
            self.create_open(&healthy).await?;
        }
        Ok(())
    }

    fn should_close(&self, volume: &Volume) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        volume.size >= self.config.close_bytes
            || now.saturating_sub(volume.opened_at_secs) >= self.config.close_after.as_secs()
    }

    async fn healthy_nodes(&self) -> Vec<NodeAddr> {
        let mut healthy = Vec::new();
        for node in &self.config.nodes {
            if self
                .http
                .get(format!("{}/health", node.url))
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
            {
                healthy.push(node.clone());
            }
        }
        healthy
    }

    async fn open_volumes(&self) -> anyhow::Result<Vec<Volume>> {
        self.http
            .get(format!("{}/volumes?state=open", self.config.index_url))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .map_err(Into::into)
    }

    async fn create_open(&self, healthy: &[NodeAddr]) -> anyhow::Result<Volume> {
        let nodes: Vec<String> = replica_placement(healthy)
            .into_iter()
            .map(|node| node.id)
            .collect();
        if nodes.is_empty() {
            bail!("no zone has a healthy storage node");
        }
        self.http
            .post(format!("{}/volumes", self.config.index_url))
            .json(&serde_json::json!({"nodes": nodes}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .map_err(Into::into)
    }

    async fn close(&self, volume: &Volume, _healthy: &[NodeAddr]) -> anyhow::Result<()> {
        let mut closed = volume.clone();
        closed.state = VolumeState::Closed;
        closed.generation += 1;
        self.update_volume(&closed).await?;
        Ok(())
    }

    /// Closes written volumes, then compacts one closed volume at a time. A
    /// rewrite keeps the old layout until every live block is readable from
    /// its replacement.
    pub async fn compact(&self) -> anyhow::Result<CompactReport> {
        let healthy = self.healthy_nodes().await;
        if fragment_placement(&healthy).len() != ec::TOTAL {
            bail!("compaction needs healthy nodes in all three zones");
        }
        for volume in self.open_volumes().await? {
            if volume.size > 0 {
                self.close(&volume, &healthy).await?;
            }
        }
        if self.open_volumes().await?.is_empty() {
            self.create_open(&healthy).await?;
        }
        let volumes: Vec<Volume> = self
            .http
            .get(format!("{}/volumes", self.config.index_url))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let mut report = CompactReport::default();
        for volume in volumes
            .into_iter()
            .filter(|v| v.state == VolumeState::Closed)
        {
            let blocks = self.blocks(&volume).await?;
            if blocks.is_empty() {
                self.retire(&volume, &healthy).await?;
                report.bytes_before += raw_bytes(&volume);
                tracing::info!(volume = volume.id, "compaction retired an empty volume");
                continue;
            }
            if has_dead_bytes(&blocks, volume.size)? {
                self.rewrite(&volume, &blocks, &healthy).await?;
                report.bytes_before += raw_bytes(&volume);
                report.bytes_after += compacted_bytes(&blocks);
                report.bytes_reclaimed +=
                    raw_bytes(&volume).saturating_sub(compacted_bytes(&blocks));
                report.volumes_coded += 1;
                tracing::info!(
                    volume = volume.id,
                    "compaction rewrote live blocks into a coded volume"
                );
            } else if volume.kind == VolumeKind::Replicated {
                self.code_in_place(&volume, &blocks, &healthy).await?;
                report.bytes_before += raw_bytes(&volume);
                report.bytes_after += volume.size.saturating_mul(3).div_ceil(2);
                report.bytes_reclaimed += volume
                    .size
                    .saturating_mul(3)
                    .saturating_sub(volume.size.saturating_mul(3).div_ceil(2));
                report.volumes_coded += 1;
                tracing::info!(volume = volume.id, "compaction coded a replicated volume");
            }
        }
        Ok(report)
    }

    async fn code_in_place(
        &self,
        volume: &Volume,
        blocks: &[ListedBlock],
        healthy: &[NodeAddr],
    ) -> anyhow::Result<()> {
        let fragments = self.encode(volume, healthy).await?;
        self.verify_coded(volume.id, volume.size, &fragments, healthy)
            .await?;
        let mut coded = volume.clone();
        coded.kind = VolumeKind::Rs63;
        coded.nodes = fragments;
        coded.generation += 1;
        self.update_volume(&coded).await?;
        for block in blocks {
            self.read_live_block(&coded, block, healthy).await?;
        }
        self.delete_layout(volume, healthy).await
    }

    async fn rewrite(
        &self,
        source: &Volume,
        blocks: &[ListedBlock],
        healthy: &[NodeAddr],
    ) -> anyhow::Result<()> {
        let target = self.create_open(healthy).await?;
        let mut moved = Vec::with_capacity(blocks.len());
        for block in blocks {
            let bytes = self.read_live_block(source, block, healthy).await?;
            let entry = self
                .append_to_replicas(&target, healthy, block.hash, &bytes)
                .await?;
            self.move_block(block, &entry).await?;
            moved.push(ListedBlock {
                hash: block.hash,
                entry,
            });
        }
        let target = self.get_volume(target.id).await?;
        let fragments = self.encode(&target, healthy).await?;
        self.verify_coded(target.id, target.size, &fragments, healthy)
            .await?;
        let mut coded = target.clone();
        coded.state = VolumeState::Closed;
        coded.kind = VolumeKind::Rs63;
        coded.nodes = fragments;
        coded.generation += 1;
        let coded = self.update_volume(&coded).await?;
        for block in &moved {
            self.read_live_block(&coded, block, healthy).await?;
        }
        self.delete_layout(&target, healthy).await?;
        self.retire(source, healthy).await
    }

    async fn get_volume(&self, id: u64) -> anyhow::Result<Volume> {
        self.http
            .get(format!("{}/volumes/{id}", self.config.index_url))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .map_err(Into::into)
    }

    async fn blocks(&self, volume: &Volume) -> anyhow::Result<Vec<ListedBlock>> {
        self.http
            .get(format!(
                "{}/blocks?volume={}",
                self.config.index_url, volume.id
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .map_err(Into::into)
    }

    async fn read_live_block(
        &self,
        volume: &Volume,
        block: &ListedBlock,
        healthy: &[NodeAddr],
    ) -> anyhow::Result<Vec<u8>> {
        let bytes = match volume.kind {
            VolumeKind::Replicated => {
                let replica = volume
                    .nodes
                    .iter()
                    .find_map(|id| healthy.iter().find(|node| &node.id == id))
                    .context("no healthy replica remains for compaction")?;
                self.http
                    .get(format!(
                        "{}/extents/{}?offset={}&length={}",
                        replica.url, volume.id, block.entry.offset, block.entry.length
                    ))
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?
                    .to_vec()
            }
            VolumeKind::Rs63 => self.read_coded(volume, healthy).await?
                [block.entry.offset as usize..(block.entry.offset + block.entry.length) as usize]
                .to_vec(),
        };
        if BlockHash::of(&bytes) != block.hash {
            bail!(
                "volume {} has a corrupt live block {}",
                volume.id,
                block.hash
            );
        }
        Ok(bytes)
    }

    async fn read_coded(&self, volume: &Volume, healthy: &[NodeAddr]) -> anyhow::Result<Vec<u8>> {
        if volume.nodes.len() != ec::TOTAL {
            bail!("coded volume {} has an invalid fragment layout", volume.id);
        }
        let mut fragments = Vec::with_capacity(ec::TOTAL);
        for (index, id) in volume.nodes.iter().enumerate() {
            let fragment = healthy
                .iter()
                .find(|node| &node.id == id)
                .context("coded volume has an unhealthy fragment")?;
            let response = self
                .http
                .get(format!(
                    "{}/extents/{}/fragments/{index}",
                    fragment.url, volume.id
                ))
                .send()
                .await?;
            fragments.push(response.error_for_status()?.bytes().await?.to_vec());
        }
        ec::decode(
            &mut fragments.into_iter().map(Some).collect::<Vec<_>>(),
            volume.size,
        )
        .map_err(anyhow::Error::msg)
    }

    async fn append_to_replicas(
        &self,
        volume: &Volume,
        healthy: &[NodeAddr],
        hash: BlockHash,
        bytes: &[u8],
    ) -> anyhow::Result<satchel_common::cluster::BlockEntry> {
        let mut offset = None;
        for id in &volume.nodes {
            let node = healthy
                .iter()
                .find(|node| &node.id == id)
                .context("new volume lost a replica")?;
            let appended: Appended = self
                .http
                .post(format!("{}/extents/{}", node.url, volume.id))
                .header("x-satchel-block", hash.to_string())
                .body(bytes.to_vec())
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if offset
                .replace(appended.offset)
                .is_some_and(|at| at != appended.offset)
            {
                bail!("replicas appended block {hash} at different offsets");
            }
            if appended.length != bytes.len() as u64 {
                bail!("replica returned the wrong append length");
            }
        }
        Ok(satchel_common::cluster::BlockEntry {
            volume: volume.id,
            offset: offset.unwrap_or(0),
            length: bytes.len() as u64,
        })
    }

    async fn move_block(
        &self,
        block: &ListedBlock,
        to: &satchel_common::cluster::BlockEntry,
    ) -> anyhow::Result<()> {
        self.http
            .post(format!(
                "{}/blocks/{}/move",
                self.config.index_url, block.hash
            ))
            .json(&serde_json::json!({"from": block.entry, "to": to}))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn verify_coded(
        &self,
        volume: u64,
        size: u64,
        nodes: &[String],
        healthy: &[NodeAddr],
    ) -> anyhow::Result<()> {
        let mut fragments = Vec::with_capacity(ec::TOTAL);
        for (index, id) in nodes.iter().enumerate() {
            let node = healthy
                .iter()
                .find(|node| &node.id == id)
                .context("coded fragment node is no longer healthy")?;
            fragments.push(
                self.http
                    .get(format!("{}/extents/{volume}/fragments/{index}", node.url))
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?
                    .to_vec(),
            );
        }
        ec::decode(
            &mut fragments.into_iter().map(Some).collect::<Vec<_>>(),
            size,
        )
        .map(|_| ())
        .map_err(anyhow::Error::msg)
    }

    /// Commits a new generation and returns the row as the index stored it.
    async fn update_volume(&self, volume: &Volume) -> anyhow::Result<Volume> {
        self.http
            .put(format!("{}/volumes/{}", self.config.index_url, volume.id))
            .json(volume)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .map_err(Into::into)
    }

    async fn delete_layout(&self, volume: &Volume, healthy: &[NodeAddr]) -> anyhow::Result<()> {
        for node in healthy
            .iter()
            .filter(|node| volume.nodes.iter().any(|id| id == &node.id))
        {
            self.http
                .delete(format!("{}/extents/{}", node.url, volume.id))
                .send()
                .await?
                .error_for_status()?;
        }
        Ok(())
    }

    async fn retire(&self, volume: &Volume, healthy: &[NodeAddr]) -> anyhow::Result<()> {
        self.delete_layout(volume, healthy).await?;
        self.http
            .delete(format!("{}/volumes/{}", self.config.index_url, volume.id))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn encode(&self, volume: &Volume, healthy: &[NodeAddr]) -> anyhow::Result<Vec<String>> {
        let placement = fragment_placement(healthy);
        if placement.len() != ec::TOTAL {
            bail!("encoding needs nine healthy placements across three zones");
        }
        let replica = volume
            .nodes
            .iter()
            .find_map(|id| healthy.iter().find(|node| &node.id == id))
            .context("no healthy replica remains for encoding")?;
        let bytes = self
            .http
            .get(format!(
                "{}/extents/{}?offset=0&length={}",
                replica.url, volume.id, volume.size
            ))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        let encoded = ec::encode(&bytes).map_err(anyhow::Error::msg)?;
        for (index, (node, fragment)) in placement.iter().zip(encoded).enumerate() {
            self.http
                .put(format!(
                    "{}/extents/{}/fragments/{index}",
                    node.url, volume.id
                ))
                .header("x-satchel-checksum", BlockHash::of(&fragment).to_string())
                .body(fragment)
                .send()
                .await?
                .error_for_status()?;
        }
        Ok(placement.into_iter().map(|node| node.id).collect())
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CompactReport {
    pub volumes_coded: u64,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub bytes_reclaimed: u64,
}

#[derive(Deserialize)]
struct Appended {
    offset: u64,
    length: u64,
}

fn raw_bytes(volume: &Volume) -> u64 {
    match volume.kind {
        VolumeKind::Replicated => volume.size.saturating_mul(3),
        VolumeKind::Rs63 => volume.size.saturating_mul(3).div_ceil(2),
    }
}

fn compacted_bytes(blocks: &[ListedBlock]) -> u64 {
    blocks
        .iter()
        .map(|block| block.entry.length)
        .sum::<u64>()
        .saturating_mul(3)
        .div_ceil(2)
}

fn has_dead_bytes(blocks: &[ListedBlock], size: u64) -> anyhow::Result<bool> {
    let mut end = 0;
    for block in blocks {
        if block.entry.offset != end {
            return Ok(true);
        }
        end = block
            .entry
            .offset
            .checked_add(block.entry.length)
            .context("block offset overflows")?;
        if end > size {
            bail!("block {} goes past its volume", block.hash);
        }
    }
    Ok(end != size)
}

/// Selects three replicas across at least two zones. When a zone is down, the
/// third replica shares a surviving zone so writes continue with three copies.
pub fn replica_placement(nodes: &[NodeAddr]) -> Vec<NodeAddr> {
    let mut by_zone: BTreeMap<String, Vec<NodeAddr>> = BTreeMap::new();
    for node in nodes {
        by_zone
            .entry(node.zone.clone())
            .or_default()
            .push(node.clone());
    }
    if by_zone.len() < 2 {
        return Vec::new();
    }

    let mut placement: Vec<NodeAddr> = by_zone
        .values()
        .filter_map(|members| members.first().cloned())
        .take(3)
        .collect();
    for members in by_zone.values() {
        for node in members.iter().skip(1) {
            if placement.len() == 3 {
                return placement;
            }
            placement.push(node.clone());
        }
    }
    if placement.len() == 3 {
        placement
    } else {
        Vec::new()
    }
}

/// Selects three fragment slots per zone for RS(6,3). A local three-node
/// cluster deliberately uses one node for all three slots in its zone; a
/// larger deployment spreads them across the nodes available in that zone.
pub fn fragment_placement(nodes: &[NodeAddr]) -> Vec<NodeAddr> {
    let mut zones: BTreeMap<String, Vec<NodeAddr>> = BTreeMap::new();
    for node in nodes {
        zones
            .entry(node.zone.clone())
            .or_default()
            .push(node.clone());
    }
    if zones.len() < 3 || zones.values().any(Vec::is_empty) {
        return Vec::new();
    }
    zones
        .into_values()
        .take(3)
        .flat_map(|members| (0..3).map(move |index| members[index % members.len()].clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    use super::*;
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::routing::{get, put};
    use axum::{Json, Router};

    #[derive(Default)]
    struct CloseState {
        events: Mutex<Vec<String>>,
        updates: Mutex<Vec<Volume>>,
    }

    async fn health() -> StatusCode {
        StatusCode::OK
    }

    #[derive(serde::Deserialize)]
    struct Range {
        length: usize,
    }

    async fn read_replica(
        State(state): State<Arc<CloseState>>,
        axum::extract::Query(range): axum::extract::Query<Range>,
    ) -> Vec<u8> {
        state
            .events
            .lock()
            .unwrap()
            .push(format!("read:{}", range.length));
        vec![7; range.length]
    }

    async fn write_fragment(State(state): State<Arc<CloseState>>) -> StatusCode {
        state.events.lock().unwrap().push("fragment".into());
        StatusCode::CREATED
    }

    async fn delete_replica(State(state): State<Arc<CloseState>>) -> StatusCode {
        state.events.lock().unwrap().push("delete".into());
        StatusCode::NO_CONTENT
    }

    /// Answers like the index: with the stored row. A block indexed after
    /// the Master listed the volume makes the closed row 5 bytes bigger.
    async fn update_volume(
        State(state): State<Arc<CloseState>>,
        Json(mut volume): Json<Volume>,
    ) -> Json<Volume> {
        state.events.lock().unwrap().push(format!(
            "volume:{}:{}",
            volume.generation,
            match volume.kind {
                VolumeKind::Replicated => "replicated",
                VolumeKind::Rs63 => "rs63",
            }
        ));
        if volume.state == VolumeState::Closed && volume.kind == VolumeKind::Replicated {
            volume.size += 5;
        }
        state.updates.lock().unwrap().push(volume.clone());
        Json(volume)
    }

    async fn close_server() -> (String, Arc<CloseState>) {
        let state = Arc::new(CloseState::default());
        let app = Router::new()
            .route("/health", get(health))
            .route(
                "/extents/{volume}",
                get(read_replica).delete(delete_replica),
            )
            .route(
                "/extents/{volume}/fragments/{fragment}",
                put(write_fragment),
            )
            .route("/volumes/{volume}", put(update_volume))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{address}"), state)
    }

    fn nodes() -> Vec<NodeAddr> {
        ["z1", "z2", "z3"]
            .into_iter()
            .flat_map(|zone| {
                (1..=3).map(move |number| NodeAddr {
                    id: format!("node-{zone}-{number}"),
                    zone: zone.into(),
                    url: format!("http://{zone}-{number}"),
                })
            })
            .collect()
    }

    #[test]
    fn replica_placement_uses_one_node_per_zone_when_all_zones_are_healthy() {
        let placement = replica_placement(&nodes());
        assert_eq!(placement.len(), 3);
        assert_eq!(
            placement
                .iter()
                .map(|node| &node.zone)
                .collect::<HashSet<_>>()
                .len(),
            3
        );
    }

    #[test]
    fn replica_placement_keeps_three_replicas_when_a_zone_is_down() {
        let healthy: Vec<_> = nodes()
            .into_iter()
            .filter(|node| node.zone != "z3")
            .collect();
        let placement = replica_placement(&healthy);

        assert_eq!(placement.len(), 3);
        assert_eq!(
            placement
                .iter()
                .map(|node| &node.zone)
                .collect::<HashSet<_>>()
                .len(),
            2
        );
    }

    #[test]
    fn replica_placement_requires_two_healthy_zones() {
        let healthy: Vec<_> = nodes()
            .into_iter()
            .filter(|node| node.zone == "z1")
            .collect();
        assert!(replica_placement(&healthy).is_empty());
    }

    #[tokio::test]
    async fn closing_keeps_the_replicated_layout_for_compaction() {
        let (url, state) = close_server().await;
        let nodes: Vec<_> = ["z1", "z2", "z3"]
            .into_iter()
            .flat_map(|zone| {
                let url = url.clone();
                (1..=3).map(move |number| NodeAddr {
                    id: format!("node-{zone}-{number}"),
                    zone: zone.into(),
                    url: url.clone(),
                })
            })
            .collect();
        let master = Master::new(MasterConfig {
            index_url: url,
            nodes: nodes.clone(),
            close_bytes: 1,
            close_after: Duration::ZERO,
        });
        let volume = Volume {
            id: 7,
            state: VolumeState::Open,
            kind: VolumeKind::Replicated,
            generation: 1,
            opened_at_secs: 0,
            nodes: ["node-z1-1", "node-z2-1", "node-z3-1"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            size: 3,
        };

        master.close(&volume, &nodes).await.unwrap();

        let updates = state.updates.lock().unwrap().clone();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].state, VolumeState::Closed);
        assert_eq!(updates[0].kind, VolumeKind::Replicated);
        assert_eq!(updates[0].generation, 2);

        let events = state.events.lock().unwrap();
        assert_eq!(events.as_slice(), ["volume:2:replicated"]);
    }

    #[test]
    fn fragments_place_exactly_three_per_zone() {
        let placement = fragment_placement(&nodes());
        let counts = placement.iter().fold(BTreeMap::new(), |mut counts, node| {
            *counts.entry(&node.zone).or_insert(0) += 1;
            counts
        });
        assert_eq!(placement.len(), 9);
        assert!(counts.values().all(|count| *count == 3));
    }

    #[test]
    fn fragments_share_a_node_when_a_zone_has_only_one() {
        let nodes: Vec<_> = ["z1", "z2", "z3"]
            .into_iter()
            .map(|zone| NodeAddr {
                id: format!("node-{zone}-1"),
                zone: zone.into(),
                url: format!("http://{zone}-1"),
            })
            .collect();
        let placement = fragment_placement(&nodes);

        assert_eq!(placement.len(), 9);
        for zone in ["z1", "z2", "z3"] {
            let in_zone: Vec<_> = placement.iter().filter(|node| node.zone == zone).collect();
            assert_eq!(in_zone.len(), 3);
            assert!(in_zone
                .iter()
                .all(|node| node.id == format!("node-{zone}-1")));
        }
    }
}
