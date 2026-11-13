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
        let mut next = volume.clone();
        next.state = VolumeState::Closed;
        next.generation += 1;
        if self.config.encode && volume.size > 0 {
            let fragments = self.encode(volume, healthy).await?;
            next.kind = VolumeKind::Rs63;
            next.nodes = fragments;
        }
        self.http
            .put(format!("{}/volumes/{}", self.config.index_url, volume.id))
            .json(&next)
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

/// Selects one node per zone. With an unavailable zone, this intentionally
/// opens a smaller replica set instead of placing two replicas in one zone.
pub fn replica_placement(nodes: &[NodeAddr]) -> Vec<NodeAddr> {
    let mut by_zone = BTreeMap::new();
    for node in nodes {
        by_zone
            .entry(node.zone.clone())
            .or_insert_with(|| node.clone());
    }
    by_zone.into_values().take(3).collect()
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

    use super::*;

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
    fn replica_placement_never_duplicates_a_zone() {
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
