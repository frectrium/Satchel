//! The Master keeps a usable pool of replicated volumes and encodes closed
//! volumes. Its only durable state is the Replication Table in the index.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context};
use satchel_common::cluster::{parse_nodes, NodeAddr, Volume, VolumeKind, VolumeState};
use satchel_common::ec;
use satchel_common::BlockHash;

#[derive(Clone, Debug)]
pub struct MasterConfig {
    pub index_url: String,
    pub nodes: Vec<NodeAddr>,
    pub close_bytes: u64,
    pub close_after: Duration,
    pub encode: bool,
}

impl MasterConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let index_url =
            std::env::var("SATCHEL_INDEX_URL").unwrap_or_else(|_| "http://index:8081".into());
        let nodes = parse_nodes(&std::env::var("SATCHEL_NODES").unwrap_or_default())
            .map_err(anyhow::Error::msg)?;
        let close_bytes = env_u64("SATCHEL_VOLUME_BYTES", 64 * 1024 * 1024)?;
        let close_after = Duration::from_secs(env_u64("SATCHEL_VOLUME_SECS", 10 * 60)?);
        let encode = !matches!(std::env::var("SATCHEL_ENCODE").as_deref(), Ok("off"));
        Ok(Self {
            index_url: index_url.trim_end_matches('/').into(),
            nodes,
            close_bytes,
            close_after,
            encode,
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

    async fn close(&self, volume: &Volume, healthy: &[NodeAddr]) -> anyhow::Result<()> {
        let mut closed = volume.clone();
        closed.state = VolumeState::Closed;
        closed.generation += 1;
        // Encode the size the index committed, not the one listed before:
        // blocks indexed in between grow it, and once the volume is closed
        // the index refuses new blocks for it (DDL-019), so this size is final.
        let committed = self.update_volume(&closed).await?;

        if self.config.encode && committed.size > 0 {
            let fragments = self.encode(&committed, healthy).await?;
            let mut coded = committed.clone();
            coded.kind = VolumeKind::Rs63;
            coded.nodes = fragments;
            coded.generation += 1;
            self.update_volume(&coded).await?;
            self.delete_replicas(&committed, healthy).await?;
        }
        Ok(())
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

    async fn delete_replicas(&self, volume: &Volume, healthy: &[NodeAddr]) -> anyhow::Result<()> {
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

/// Selects three distinct nodes per zone for RS(6,3), or no placement unless
/// all three zones have enough healthy nodes.
pub fn fragment_placement(nodes: &[NodeAddr]) -> Vec<NodeAddr> {
    let mut zones: BTreeMap<String, Vec<NodeAddr>> = BTreeMap::new();
    for node in nodes {
        zones
            .entry(node.zone.clone())
            .or_default()
            .push(node.clone());
    }
    if zones.len() < 3 || zones.values().any(|members| members.len() < 3) {
        return Vec::new();
    }
    zones
        .into_values()
        .take(3)
        .flat_map(|members| members.into_iter().take(3))
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
    async fn closing_commits_coded_layout_before_removing_replicas() {
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
            encode: true,
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
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].state, VolumeState::Closed);
        assert_eq!(updates[0].kind, VolumeKind::Replicated);
        assert_eq!(updates[0].generation, 2);
        assert_eq!(updates[1].kind, VolumeKind::Rs63);
        assert_eq!(updates[1].generation, 3);

        let events = state.events.lock().unwrap();
        let close = events
            .iter()
            .position(|event| event == "volume:2:replicated")
            .unwrap();
        let first_fragment = events.iter().position(|event| event == "fragment").unwrap();
        let coded = events
            .iter()
            .position(|event| event == "volume:3:rs63")
            .unwrap();
        let first_delete = events.iter().position(|event| event == "delete").unwrap();
        // The encode read the committed 8 bytes, not the 3 the listing had.
        assert!(events.iter().any(|event| event == "read:8"), "{events:?}");
        assert_eq!(updates[1].size, 8);
        assert!(close < first_fragment);
        assert!(first_fragment < coded);
        assert!(coded < first_delete);
        assert_eq!(events.iter().filter(|event| *event == "delete").count(), 3);
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
}
