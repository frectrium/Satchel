//! The Block Index and the Replication Table in one SQLite database
//! (DDL-003).
//!
//! Magic Pocket keeps both tables in MySQL [B2]. Satchel keeps the same
//! split in two tables of one file.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use satchel_common::cluster::{BlockEntry, Volume, VolumeKind, VolumeState};
use satchel_common::BlockHash;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS volumes (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    state      TEXT    NOT NULL CHECK (state IN ('open', 'closed')),
    kind       TEXT    NOT NULL CHECK (kind IN ('replicated', 'rs63')),
    generation INTEGER NOT NULL,
    opened_at INTEGER NOT NULL DEFAULT 0,
    nodes      TEXT    NOT NULL,
    size       INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS blocks (
    hash   TEXT    PRIMARY KEY,
    volume INTEGER NOT NULL REFERENCES volumes (id),
    offset INTEGER NOT NULL,
    length INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS blocks_by_volume ON blocks (volume, offset);
";

/// What a block insert did.
#[derive(Debug, PartialEq, Eq)]
pub enum PutBlock {
    Created,
    /// The hash was already there. Nothing changed (TS-IDX-3).
    Exists(BlockEntry),
    NoVolume,
    /// Only open volumes take blocks. A closed volume may already be encoded,
    /// so a late block would be missing from its fragments.
    VolumeNotOpen,
}

/// What a volume update did.
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateVolume {
    Updated(Volume),
    NotFound,
    /// The change was made against an older generation (DDL-010).
    Stale {
        current: u64,
    },
}

pub struct Store {
    conn: Mutex<Connection>,
}

fn state_str(s: VolumeState) -> &'static str {
    match s {
        VolumeState::Open => "open",
        VolumeState::Closed => "closed",
    }
}

fn kind_str(k: VolumeKind) -> &'static str {
    match k {
        VolumeKind::Replicated => "replicated",
        VolumeKind::Rs63 => "rs63",
    }
}

fn volume_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Volume> {
    let bad = |i, what: &str| {
        rusqlite::Error::FromSqlConversionFailure(
            i,
            rusqlite::types::Type::Text,
            format!("bad {what}").into(),
        )
    };
    let state = match row.get::<_, String>(1)?.as_str() {
        "open" => VolumeState::Open,
        "closed" => VolumeState::Closed,
        _ => return Err(bad(1, "state")),
    };
    let kind = match row.get::<_, String>(2)?.as_str() {
        "replicated" => VolumeKind::Replicated,
        "rs63" => VolumeKind::Rs63,
        _ => return Err(bad(2, "kind")),
    };
    let nodes: String = row.get(5)?;
    Ok(Volume {
        id: row.get(0)?,
        state,
        kind,
        generation: row.get(3)?,
        opened_at_secs: row.get(4)?,
        nodes: serde_json::from_str(&nodes).map_err(|_| bad(5, "nodes"))?,
        size: row.get(6)?,
    })
}

const VOLUME_COLUMNS: &str = "id, state, kind, generation, opened_at, nodes, size";

impl Store {
    /// Opens or creates the database at `path`. Writes are synced to disk
    /// before they return, so an acknowledged entry survives a crash.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        let has_opened_at = conn
            .prepare("PRAGMA table_info(volumes)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .iter()
            .any(|name| name == "opened_at");
        if !has_opened_at {
            conn.execute(
                "ALTER TABLE volumes ADD COLUMN opened_at INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("index lock")
    }

    pub fn get_block(&self, hash: &BlockHash) -> rusqlite::Result<Option<BlockEntry>> {
        self.conn()
            .query_row(
                "SELECT volume, offset, length FROM blocks WHERE hash = ?1",
                [hash.to_string()],
                |r| {
                    Ok(BlockEntry {
                        volume: r.get(0)?,
                        offset: r.get(1)?,
                        length: r.get(2)?,
                    })
                },
            )
            .optional()
    }

    /// Records a block and raises its volume's size to the block's end.
    pub fn put_block(&self, hash: &BlockHash, entry: &BlockEntry) -> rusqlite::Result<PutBlock> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let state: Option<String> = tx
            .query_row(
                "SELECT state FROM volumes WHERE id = ?1",
                [entry.volume],
                |r| r.get(0),
            )
            .optional()?;
        match state.as_deref() {
            None => return Ok(PutBlock::NoVolume),
            Some("open") => {}
            Some(_) => return Ok(PutBlock::VolumeNotOpen),
        }
        let existing = tx
            .query_row(
                "SELECT volume, offset, length FROM blocks WHERE hash = ?1",
                [hash.to_string()],
                |r| {
                    Ok(BlockEntry {
                        volume: r.get(0)?,
                        offset: r.get(1)?,
                        length: r.get(2)?,
                    })
                },
            )
            .optional()?;
        if let Some(e) = existing {
            return Ok(PutBlock::Exists(e));
        }
        tx.execute(
            "INSERT INTO blocks (hash, volume, offset, length) VALUES (?1, ?2, ?3, ?4)",
            params![hash.to_string(), entry.volume, entry.offset, entry.length],
        )?;
        tx.execute(
            "UPDATE volumes SET size = MAX(size, ?2) WHERE id = ?1",
            params![entry.volume, entry.offset + entry.length],
        )?;
        tx.commit()?;
        Ok(PutBlock::Created)
    }

    /// Removes the entry. The bytes stay in their volume (DDL-013).
    pub fn delete_block(&self, hash: &BlockHash) -> rusqlite::Result<bool> {
        Ok(self
            .conn()
            .execute("DELETE FROM blocks WHERE hash = ?1", [hash.to_string()])?
            > 0)
    }

    /// Every entry in a volume, by offset, for the metadata scan (DDL-012).
    pub fn blocks_in_volume(&self, volume: u64) -> rusqlite::Result<Vec<(BlockHash, BlockEntry)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT hash, volume, offset, length FROM blocks WHERE volume = ?1 ORDER BY offset",
        )?;
        let rows = stmt.query_map([volume], |r| {
            let hash: String = r.get(0)?;
            Ok((
                hash,
                BlockEntry {
                    volume: r.get(1)?,
                    offset: r.get(2)?,
                    length: r.get(3)?,
                },
            ))
        })?;
        rows.map(|row| {
            let (hash, entry) = row?;
            let hash = hash.parse().map_err(|_| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    "bad hash".into(),
                )
            })?;
            Ok((hash, entry))
        })
        .collect()
    }

    /// Opens a new replicated volume on `nodes`, generation 1.
    pub fn create_volume(&self, nodes: &[String]) -> rusqlite::Result<Volume> {
        let conn = self.conn();
        let opened_at_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_secs();
        conn.execute(
            "INSERT INTO volumes (state, kind, generation, opened_at, nodes, size) VALUES ('open', 'replicated', 1, ?1, ?2, 0)",
            params![opened_at_secs, serde_json::to_string(nodes).expect("node list is JSON")],
        )?;
        let id = conn.last_insert_rowid() as u64;
        Ok(Volume {
            id,
            state: VolumeState::Open,
            kind: VolumeKind::Replicated,
            generation: 1,
            opened_at_secs,
            nodes: nodes.to_vec(),
            size: 0,
        })
    }

    pub fn get_volume(&self, id: u64) -> rusqlite::Result<Option<Volume>> {
        self.conn()
            .query_row(
                &format!("SELECT {VOLUME_COLUMNS} FROM volumes WHERE id = ?1"),
                [id],
                volume_row,
            )
            .optional()
    }

    pub fn volumes(&self, state: Option<VolumeState>) -> rusqlite::Result<Vec<Volume>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {VOLUME_COLUMNS} FROM volumes WHERE ?1 IS NULL OR state = ?1 ORDER BY id"
        ))?;
        let rows = stmt.query_map([state.map(state_str)], volume_row)?;
        rows.collect()
    }

    /// Commits a change to a volume if `next.generation` is exactly one more
    /// than the stored generation, so a stale Master or repair write can't
    /// overwrite a newer one (DDL-010). `size` belongs to the index: it only
    /// grows through block puts, so the stored value is kept.
    pub fn update_volume(&self, next: &Volume) -> rusqlite::Result<UpdateVolume> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let current: Option<u64> = tx
            .query_row(
                "SELECT generation FROM volumes WHERE id = ?1",
                [next.id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(current) = current else {
            return Ok(UpdateVolume::NotFound);
        };
        if next.generation != current + 1 {
            return Ok(UpdateVolume::Stale { current });
        }
        tx.execute(
            "UPDATE volumes SET state = ?2, kind = ?3, generation = ?4, nodes = ?5 WHERE id = ?1",
            params![
                next.id,
                state_str(next.state),
                kind_str(next.kind),
                next.generation,
                serde_json::to_string(&next.nodes).expect("node list is JSON"),
            ],
        )?;
        let updated = tx.query_row(
            &format!("SELECT {VOLUME_COLUMNS} FROM volumes WHERE id = ?1"),
            [next.id],
            volume_row,
        )?;
        tx.commit()?;
        Ok(UpdateVolume::Updated(updated))
    }

    /// Number of blocks and volumes, for the metrics (#21).
    pub fn counts(&self) -> rusqlite::Result<(u64, u64)> {
        let conn = self.conn();
        let blocks = conn.query_row("SELECT COUNT(*) FROM blocks", [], |r| r.get(0))?;
        let volumes = conn.query_row("SELECT COUNT(*) FROM volumes", [], |r| r.get(0))?;
        Ok((blocks, volumes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(n: u8) -> BlockHash {
        BlockHash::of(&[n])
    }

    fn nodes() -> Vec<String> {
        ["node-z1-1", "node-z2-1", "node-z3-1"]
            .map(String::from)
            .to_vec()
    }

    fn entry(volume: u64, offset: u64, length: u64) -> BlockEntry {
        BlockEntry {
            volume,
            offset,
            length,
        }
    }

    #[test]
    fn ts_idx_1_insert_lookup_delete() {
        let s = Store::open_in_memory().unwrap();
        let v = s.create_volume(&nodes()).unwrap();
        let e = entry(v.id, 0, 100);
        assert_eq!(s.put_block(&hash(1), &e).unwrap(), PutBlock::Created);
        assert_eq!(s.get_block(&hash(1)).unwrap(), Some(e));
        assert!(s.delete_block(&hash(1)).unwrap());
        assert_eq!(s.get_block(&hash(1)).unwrap(), None);
        assert!(!s.delete_block(&hash(1)).unwrap());
    }

    #[test]
    fn ts_idx_2_unknown_hash_is_none() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.get_block(&hash(9)).unwrap(), None);
    }

    #[test]
    fn ts_idx_3_second_put_keeps_the_first_entry() {
        let s = Store::open_in_memory().unwrap();
        let v = s.create_volume(&nodes()).unwrap();
        let first = entry(v.id, 0, 100);
        s.put_block(&hash(1), &first).unwrap();
        assert_eq!(
            s.put_block(&hash(1), &entry(v.id, 100, 100)).unwrap(),
            PutBlock::Exists(first)
        );
        assert_eq!(s.get_block(&hash(1)).unwrap(), Some(first));
        assert_eq!(s.get_volume(v.id).unwrap().unwrap().size, 100);
    }

    #[test]
    fn block_puts_raise_the_volume_size_to_the_last_byte() {
        let s = Store::open_in_memory().unwrap();
        let v = s.create_volume(&nodes()).unwrap();
        s.put_block(&hash(1), &entry(v.id, 0, 100)).unwrap();
        s.put_block(&hash(2), &entry(v.id, 100, 50)).unwrap();
        assert_eq!(s.get_volume(v.id).unwrap().unwrap().size, 150);
    }

    #[test]
    fn blocks_need_an_open_volume() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(
            s.put_block(&hash(1), &entry(7, 0, 1)).unwrap(),
            PutBlock::NoVolume
        );
        let v = s.create_volume(&nodes()).unwrap();
        let closed = Volume {
            state: VolumeState::Closed,
            generation: 2,
            ..v.clone()
        };
        s.update_volume(&closed).unwrap();
        assert_eq!(
            s.put_block(&hash(1), &entry(v.id, 0, 1)).unwrap(),
            PutBlock::VolumeNotOpen
        );
    }

    #[test]
    fn stale_generation_is_rejected() {
        let s = Store::open_in_memory().unwrap();
        let v = s.create_volume(&nodes()).unwrap();
        let next = Volume {
            state: VolumeState::Closed,
            generation: 2,
            ..v.clone()
        };
        assert!(matches!(
            s.update_volume(&next).unwrap(),
            UpdateVolume::Updated(_)
        ));
        // A second writer that also read generation 1 loses.
        assert_eq!(
            s.update_volume(&next).unwrap(),
            UpdateVolume::Stale { current: 2 }
        );
        // Skipping a generation is also stale.
        let skip = Volume {
            generation: 4,
            ..next
        };
        assert_eq!(
            s.update_volume(&skip).unwrap(),
            UpdateVolume::Stale { current: 2 }
        );
    }

    #[test]
    fn update_keeps_the_size_the_index_owns() {
        let s = Store::open_in_memory().unwrap();
        let v = s.create_volume(&nodes()).unwrap();
        s.put_block(&hash(1), &entry(v.id, 0, 4096)).unwrap();
        let coded = Volume {
            state: VolumeState::Closed,
            kind: VolumeKind::Rs63,
            generation: 2,
            nodes: (1..=9).map(|n| format!("node-{n}")).collect(),
            size: 0,
            ..v
        };
        let UpdateVolume::Updated(after) = s.update_volume(&coded).unwrap() else {
            panic!("update failed");
        };
        assert_eq!(after.size, 4096);
        assert_eq!(after.kind, VolumeKind::Rs63);
        assert_eq!(after.nodes.len(), 9);
    }

    #[test]
    fn lists_volumes_by_state_and_blocks_by_volume() {
        let s = Store::open_in_memory().unwrap();
        let a = s.create_volume(&nodes()).unwrap();
        let b = s.create_volume(&nodes()).unwrap();
        s.update_volume(&Volume {
            state: VolumeState::Closed,
            generation: 2,
            ..b.clone()
        })
        .unwrap();
        let open: Vec<u64> = s
            .volumes(Some(VolumeState::Open))
            .unwrap()
            .iter()
            .map(|v| v.id)
            .collect();
        assert_eq!(open, [a.id]);
        assert_eq!(s.volumes(None).unwrap().len(), 2);

        s.put_block(&hash(2), &entry(a.id, 10, 5)).unwrap();
        s.put_block(&hash(1), &entry(a.id, 0, 10)).unwrap();
        let listed: Vec<u64> = s
            .blocks_in_volume(a.id)
            .unwrap()
            .iter()
            .map(|(_, e)| e.offset)
            .collect();
        assert_eq!(listed, [0, 10]);
        assert_eq!(s.counts().unwrap(), (2, 2));
    }

    #[test]
    fn entries_survive_reopening_the_file() {
        let dir = std::env::temp_dir().join(format!("satchel-index-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("index.sqlite3");
        let _ = std::fs::remove_file(&path);
        let id = {
            let s = Store::open(&path).unwrap();
            let v = s.create_volume(&nodes()).unwrap();
            s.put_block(&hash(1), &entry(v.id, 0, 7)).unwrap();
            v.id
        };
        let s = Store::open(&path).unwrap();
        assert_eq!(s.get_block(&hash(1)).unwrap(), Some(entry(id, 0, 7)));
        assert_eq!(s.get_volume(id).unwrap().unwrap().size, 7);
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
