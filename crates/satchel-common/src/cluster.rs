//! Types the services exchange: the storage node list, Block Index entries
//! and Replication Table volumes. The APIs are written out in #66 (index)
//! and #67 (node).

use serde::{Deserialize, Serialize};

/// A storage node, from `SATCHEL_NODES`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeAddr {
    /// Compose service name, for example `node-z1-1`.
    pub id: String,
    pub zone: String,
    /// Base URL without a trailing slash.
    pub url: String,
}

/// Parses `SATCHEL_NODES`: `id@zone=url` entries separated by commas, for
/// example `node-z1-1@z1=http://node-z1-1:8090`. An empty string is no nodes.
pub fn parse_nodes(raw: &str) -> Result<Vec<NodeAddr>, String> {
    let mut nodes: Vec<NodeAddr> = Vec::new();
    for entry in raw.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let bad = || format!("SATCHEL_NODES entry {entry:?} is not id@zone=url");
        let (name, url) = entry.split_once('=').ok_or_else(bad)?;
        let (id, zone) = name.split_once('@').ok_or_else(bad)?;
        let url = url.trim_end_matches('/');
        if id.is_empty() || zone.is_empty() || !url.starts_with("http") {
            return Err(bad());
        }
        if nodes.iter().any(|n| n.id == id) {
            return Err(format!("SATCHEL_NODES lists {id} twice"));
        }
        nodes.push(NodeAddr {
            id: id.to_owned(),
            zone: zone.to_owned(),
            url: url.to_owned(),
        });
    }
    Ok(nodes)
}

/// Where a block lives: a Block Index entry (DDL-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockEntry {
    pub volume: u64,
    /// The block's first byte in the volume.
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeKind {
    /// 3 replicas, one per zone (DDL-006).
    Replicated,
    /// RS(6,3), 9 fragments (DDL-007).
    Rs63,
}

/// A Replication Table row (DDL-003).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volume {
    pub id: u64,
    pub state: VolumeState,
    pub kind: VolumeKind,
    /// Bumped by every committed change (DDL-010).
    pub generation: u64,
    /// Node ids in placement order: replicas, or fragments 0 to 8.
    pub nodes: Vec<String>,
    /// End of the last block written.
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_node_list() {
        let nodes =
            parse_nodes("node-z1-1@z1=http://node-z1-1:8090/, node-z2-1@z2=http://10.0.2.4:8090")
                .unwrap();
        assert_eq!(
            nodes,
            [
                NodeAddr {
                    id: "node-z1-1".into(),
                    zone: "z1".into(),
                    url: "http://node-z1-1:8090".into()
                },
                NodeAddr {
                    id: "node-z2-1".into(),
                    zone: "z2".into(),
                    url: "http://10.0.2.4:8090".into()
                },
            ]
        );
        assert_eq!(parse_nodes(""), Ok(vec![]));
    }

    #[test]
    fn rejects_bad_node_lists() {
        for bad in [
            "node-z1-1=http://x",
            "node-z1-1@z1",
            "@z1=http://x",
            "node-z1-1@=http://x",
            "node-z1-1@z1=ftp://x",
            "a@z1=http://x,a@z2=http://y",
        ] {
            assert!(parse_nodes(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn volume_json_matches_the_index_api() {
        let json = r#"{"id":17,"state":"open","kind":"replicated","generation":1,
                       "nodes":["node-z1-1","node-z2-1","node-z3-1"],"size":4096}"#;
        let v: Volume = serde_json::from_str(json).unwrap();
        assert_eq!(v.state, VolumeState::Open);
        assert_eq!(v.kind, VolumeKind::Replicated);
        let coded: VolumeKind = serde_json::from_str(r#""rs63""#).unwrap();
        assert_eq!(coded, VolumeKind::Rs63);
    }
}
