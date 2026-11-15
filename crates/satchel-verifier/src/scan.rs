//! The Disk Scrubber and the Metadata Scanner (#72).
//!
//! Magic Pocket counterpart: two of Pocket Watch's six verifiers [B3].
//! Decision Log: DDL-012. Findings are reported, not repaired: repair is the
//! Master's job (DDL-010).

use std::collections::HashMap;

use satchel_common::cluster::{NodeAddr, Volume, VolumeKind};
use satchel_common::BlockHash;
use serde::Serialize;

use crate::clients::{Clients, Extent, ExtentKind, ListedBlock, ReadError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Check {
    /// Every stored block and fragment, read back and re-hashed.
    Scrubber,
    /// Every Block Index entry, checked against the nodes that should hold it.
    Metadata,
}

impl Check {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scrubber => "scrubber",
            Self::Metadata => "metadata",
        }
    }
}

/// One problem. Names the hash where there is one, the volume and the node
/// (TS-VER-4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub check: Check,
    pub hash: Option<BlockHash>,
    pub volume: u64,
    pub node: String,
    /// The fragment, for coded volumes.
    pub fragment: Option<u32>,
    pub problem: String,
}

/// What one pass looked at and found.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Pass {
    pub blocks_scrubbed: u64,
    pub fragments_scrubbed: u64,
    pub entries_scanned: u64,
    pub findings: Vec<Finding>,
    /// Nodes or index calls that couldn't be reached, so parts of the pass
    /// were skipped. Not findings: a node that's down is the Master's to
    /// notice (DDL-010).
    pub skipped: Vec<String>,
}

/// Everything a pass needs from the index, read once.
struct Snapshot {
    volumes: Vec<Volume>,
    blocks: HashMap<u64, Vec<ListedBlock>>,
    extents: HashMap<String, Vec<Extent>>,
}

async fn snapshot(clients: &Clients, nodes: &[NodeAddr], pass: &mut Pass) -> Option<Snapshot> {
    let volumes = match clients.volumes().await {
        Ok(v) => v,
        Err(e) => {
            pass.skipped.push(format!("index: {e}"));
            return None;
        }
    };
    let mut blocks = HashMap::new();
    for v in &volumes {
        match clients.blocks(v.id).await {
            Ok(b) => {
                blocks.insert(v.id, b);
            }
            Err(e) => pass.skipped.push(format!("index, volume {}: {e}", v.id)),
        }
    }
    let mut extents = HashMap::new();
    for node in nodes {
        match clients.extents(node).await {
            Ok(e) => {
                extents.insert(node.id.clone(), e);
            }
            Err(e) => pass.skipped.push(format!("{}: {e}", node.id)),
        }
    }
    Some(Snapshot {
        volumes,
        blocks,
        extents,
    })
}

/// Runs both checks once over the whole cluster.
pub async fn run_pass(clients: &Clients, nodes: &[NodeAddr]) -> Pass {
    let mut pass = Pass::default();
    let Some(snap) = snapshot(clients, nodes, &mut pass).await else {
        return pass;
    };
    scrub(clients, nodes, &snap, &mut pass).await;
    metadata_scan(nodes, &snap, &mut pass);
    pass
}

/// Disk Scrubber: reads every indexed block from every replica that holds
/// it, and every fragment, and checks each against its hash or checksum.
async fn scrub(clients: &Clients, nodes: &[NodeAddr], snap: &Snapshot, pass: &mut Pass) {
    for node in nodes {
        let Some(extents) = snap.extents.get(&node.id) else {
            continue;
        };
        for extent in extents {
            match extent.kind {
                ExtentKind::Replica => {
                    let Some(blocks) = snap.blocks.get(&extent.volume) else {
                        continue;
                    };
                    // Blocks past the end of the extent are the metadata scan's.
                    for b in blocks
                        .iter()
                        .filter(|b| b.entry.offset + b.entry.length <= extent.size)
                    {
                        pass.blocks_scrubbed += 1;
                        let problem = match clients.read_block(node, &b.entry).await {
                            Ok(bytes) if b.hash.verify(&bytes) => continue,
                            Ok(_) => "the bytes don't match the block's hash".to_owned(),
                            Err(ReadError::Unreachable(why)) => {
                                pass.skipped.push(format!("{}: {why}", node.id));
                                continue;
                            }
                            Err(e) => format!("read failed: {e}"),
                        };
                        pass.findings.push(Finding {
                            check: Check::Scrubber,
                            hash: Some(b.hash),
                            volume: extent.volume,
                            node: node.id.clone(),
                            fragment: None,
                            problem,
                        });
                    }
                }
                ExtentKind::Fragment => {
                    let Some(index) = extent.index else {
                        continue;
                    };
                    pass.fragments_scrubbed += 1;
                    let problem = match clients.read_fragment(node, extent.volume, index).await {
                        Ok(bytes) if bytes.len() as u64 == extent.size => continue,
                        Ok(bytes) => format!(
                            "fragment is {} bytes, the node lists {}",
                            bytes.len(),
                            extent.size
                        ),
                        Err(ReadError::Unreachable(why)) => {
                            pass.skipped.push(format!("{}: {why}", node.id));
                            continue;
                        }
                        // The node checks the fragment's checksum and fails the
                        // read rather than return bad bytes (TS-NODE-5).
                        Err(e) => format!("read failed: {e}"),
                    };
                    pass.findings.push(Finding {
                        check: Check::Scrubber,
                        hash: None,
                        volume: extent.volume,
                        node: node.id.clone(),
                        fragment: Some(index),
                        problem,
                    });
                }
            }
        }
    }
}

/// Metadata Scanner: every index entry must be on every node its volume
/// lists, inside that node's extent.
fn metadata_scan(nodes: &[NodeAddr], snap: &Snapshot, pass: &mut Pass) {
    for volume in &snap.volumes {
        let Some(blocks) = snap.blocks.get(&volume.id) else {
            continue;
        };
        pass.entries_scanned += blocks.len() as u64;
        for (position, node_id) in volume.nodes.iter().enumerate() {
            let mut report = |hash: Option<BlockHash>, fragment, problem: String| {
                pass.findings.push(Finding {
                    check: Check::Metadata,
                    hash,
                    volume: volume.id,
                    node: node_id.clone(),
                    fragment,
                    problem,
                });
            };
            if !nodes.iter().any(|n| &n.id == node_id) {
                report(None, None, format!("{node_id} is not in SATCHEL_NODES"));
                continue;
            }
            // A node we couldn't list was skipped, not missing.
            let Some(extents) = snap.extents.get(node_id) else {
                continue;
            };
            match volume.kind {
                VolumeKind::Replicated => {
                    let replica = extents
                        .iter()
                        .find(|e| e.kind == ExtentKind::Replica && e.volume == volume.id);
                    for b in blocks {
                        let end = b.entry.offset + b.entry.length;
                        match replica {
                            Some(e) if e.size >= end => {}
                            Some(e) => report(
                                Some(b.hash),
                                None,
                                format!("replica ends at byte {}, the block at {end}", e.size),
                            ),
                            None => report(Some(b.hash), None, "replica is missing".to_owned()),
                        }
                    }
                }
                VolumeKind::Rs63 => {
                    let index = position as u32;
                    let has = extents.iter().any(|e| {
                        e.kind == ExtentKind::Fragment
                            && e.volume == volume.id
                            && e.index == Some(index)
                    });
                    if !has && !blocks.is_empty() {
                        report(None, Some(index), format!("fragment {index} is missing"));
                    }
                }
            }
        }
    }
}
