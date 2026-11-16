//! The file manifest: which blocks make up each file, in order, and how many
//! files use each block. Satchel itself only stores blocks by hash; this is
//! the thin metadata layer above it (DDL-020, a deliberate deviation: Dropbox
//! keeps file metadata in separate systems such as FileJournal [B2]).

use std::path::Path;
use std::sync::Mutex;

use anyhow::Context;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

/// One stored file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileRecord {
    pub id: i64,
    pub name: String,
    pub size: u64,
    /// SHA-256 of the whole file, hex.
    pub sha256: String,
    /// Seconds since the Unix epoch.
    pub created_at: u64,
    pub blocks: u64,
}

/// One block of a file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BlockRef {
    pub index: u64,
    pub hash: String,
    pub size: u64,
}

/// Totals over every file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Totals {
    pub files: u64,
    /// Sum of file sizes, as the user sees them.
    pub logical_bytes: u64,
    pub unique_blocks: u64,
    /// Bytes of distinct blocks: what Satchel stores once before replication.
    pub unique_bytes: u64,
}

pub struct Manifest {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS files (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT    NOT NULL,
    size       INTEGER NOT NULL,
    sha256     TEXT    NOT NULL,
    created_at INTEGER NOT NULL,
    blocks     INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS file_blocks (
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    idx     INTEGER NOT NULL,
    hash    TEXT    NOT NULL,
    size    INTEGER NOT NULL,
    PRIMARY KEY (file_id, idx)
);
CREATE TABLE IF NOT EXISTS blocks (
    hash TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    refs INTEGER NOT NULL
);
";

fn record(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        size: row.get::<_, i64>(2)? as u64,
        sha256: row.get(3)?,
        created_at: row.get::<_, i64>(4)? as u64,
        blocks: row.get::<_, i64>(5)? as u64,
    })
}

impl Manifest {
    /// Opens (or creates) the manifest at `path`.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("create manifest directory {}", dir.display()))?;
        }
        Self::init(Connection::open(path).with_context(|| format!("open {}", path.display()))?)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> anyhow::Result<Self> {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("manifest lock poisoned")
    }

    /// Records a file whose blocks are already stored, and adds one
    /// reference to each of its blocks.
    pub fn add_file(
        &self,
        name: &str,
        size: u64,
        sha256: &str,
        created_at: u64,
        blocks: &[(String, u64)],
    ) -> anyhow::Result<FileRecord> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO files (name, size, sha256, created_at, blocks) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, size as i64, sha256, created_at as i64, blocks.len() as i64],
        )?;
        let id = tx.last_insert_rowid();
        for (i, (hash, len)) in blocks.iter().enumerate() {
            tx.execute(
                "INSERT INTO file_blocks (file_id, idx, hash, size) VALUES (?1, ?2, ?3, ?4)",
                params![id, i as i64, hash, *len as i64],
            )?;
            tx.execute(
                "INSERT INTO blocks (hash, size, refs) VALUES (?1, ?2, 1)
                 ON CONFLICT(hash) DO UPDATE SET refs = refs + 1",
                params![hash, *len as i64],
            )?;
        }
        tx.commit()?;
        Ok(FileRecord {
            id,
            name: name.to_owned(),
            size,
            sha256: sha256.to_owned(),
            created_at,
            blocks: blocks.len() as u64,
        })
    }

    /// Every file, newest first.
    pub fn list(&self) -> anyhow::Result<Vec<FileRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, size, sha256, created_at, blocks FROM files ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], record)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Every file with its ordered blocks. The files page needs this once to
    /// annotate each file from one Block Index inventory request.
    pub fn list_with_blocks(&self) -> anyhow::Result<Vec<(FileRecord, Vec<BlockRef>)>> {
        let files = self.list()?;
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT file_id, idx, hash, size FROM file_blocks ORDER BY file_id, idx")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                BlockRef {
                    index: row.get::<_, i64>(1)? as u64,
                    hash: row.get(2)?,
                    size: row.get::<_, i64>(3)? as u64,
                },
            ))
        })?;
        let mut blocks = std::collections::HashMap::<i64, Vec<BlockRef>>::new();
        for row in rows {
            let (file, block) = row?;
            blocks.entry(file).or_default().push(block);
        }
        Ok(files
            .into_iter()
            .map(|file| {
                let rows = blocks.remove(&file.id).unwrap_or_default();
                (file, rows)
            })
            .collect())
    }

    /// A file and its blocks in order.
    pub fn get(&self, id: i64) -> anyhow::Result<Option<(FileRecord, Vec<BlockRef>)>> {
        let conn = self.conn();
        let Some(file) = conn
            .query_row(
                "SELECT id, name, size, sha256, created_at, blocks FROM files WHERE id = ?1",
                [id],
                record,
            )
            .optional()?
        else {
            return Ok(None);
        };
        let mut stmt = conn
            .prepare("SELECT idx, hash, size FROM file_blocks WHERE file_id = ?1 ORDER BY idx")?;
        let blocks = stmt
            .query_map([id], |row| {
                Ok(BlockRef {
                    index: row.get::<_, i64>(0)? as u64,
                    hash: row.get(1)?,
                    size: row.get::<_, i64>(2)? as u64,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(Some((file, blocks)))
    }

    /// Whether any file already uses this block.
    pub fn has_block(&self, hash: &str) -> anyhow::Result<bool> {
        Ok(self
            .conn()
            .query_row("SELECT 1 FROM blocks WHERE hash = ?1", [hash], |_| Ok(()))
            .optional()?
            .is_some())
    }

    /// Removes a file. Returns the blocks no file uses any more, so the
    /// caller can delete them from Satchel, or `None` if there is no such file.
    pub fn remove(&self, id: i64) -> anyhow::Result<Option<Vec<String>>> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let exists = tx
            .query_row("SELECT 1 FROM files WHERE id = ?1", [id], |_| Ok(()))
            .optional()?
            .is_some();
        if !exists {
            return Ok(None);
        }
        let hashes: Vec<String> = {
            let mut stmt = tx.prepare("SELECT hash FROM file_blocks WHERE file_id = ?1")?;
            let rows = stmt.query_map([id], |row| row.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        for hash in &hashes {
            tx.execute("UPDATE blocks SET refs = refs - 1 WHERE hash = ?1", [hash])?;
        }
        let orphans: Vec<String> = {
            let mut stmt = tx.prepare("SELECT hash FROM blocks WHERE refs <= 0")?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        tx.execute("DELETE FROM blocks WHERE refs <= 0", [])?;
        tx.execute("DELETE FROM file_blocks WHERE file_id = ?1", [id])?;
        tx.execute("DELETE FROM files WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(Some(orphans))
    }

    pub fn totals(&self) -> anyhow::Result<Totals> {
        let conn = self.conn();
        let (files, logical): (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM files",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let (blocks, unique): (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM blocks",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(Totals {
            files: files as u64,
            logical_bytes: logical as u64,
            unique_blocks: blocks as u64,
            unique_bytes: unique as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks(names: &[(&str, u64)]) -> Vec<(String, u64)> {
        names.iter().map(|(h, s)| ((*h).to_owned(), *s)).collect()
    }

    #[test]
    fn files_keep_their_blocks_in_order() {
        let m = Manifest::open_in_memory().unwrap();
        let f = m
            .add_file("a.bin", 10, "aa", 1, &blocks(&[("h2", 4), ("h1", 6)]))
            .unwrap();
        let (got, refs) = m.get(f.id).unwrap().unwrap();
        assert_eq!(got, f);
        assert_eq!(
            refs.iter().map(|b| b.hash.as_str()).collect::<Vec<_>>(),
            ["h2", "h1"]
        );
    }

    #[test]
    fn shared_blocks_are_counted_once_and_kept_until_the_last_file_goes() {
        let m = Manifest::open_in_memory().unwrap();
        let a = m
            .add_file("a", 8, "a", 1, &blocks(&[("x", 4), ("y", 4)]))
            .unwrap();
        let b = m
            .add_file("b", 8, "b", 2, &blocks(&[("x", 4), ("z", 4)]))
            .unwrap();
        let t = m.totals().unwrap();
        assert_eq!(
            (t.files, t.logical_bytes, t.unique_blocks, t.unique_bytes),
            (2, 16, 3, 12)
        );

        let mut orphans = m.remove(a.id).unwrap().unwrap();
        orphans.sort();
        assert_eq!(orphans, ["y"]);
        assert!(m.has_block("x").unwrap());
        let mut orphans = m.remove(b.id).unwrap().unwrap();
        orphans.sort();
        assert_eq!(orphans, ["x", "z"]);
        assert_eq!(m.totals().unwrap(), Totals::default());
    }

    #[test]
    fn a_block_repeated_inside_one_file_is_released_together() {
        let m = Manifest::open_in_memory().unwrap();
        let f = m
            .add_file("a", 8, "a", 1, &blocks(&[("x", 4), ("x", 4)]))
            .unwrap();
        assert_eq!(m.totals().unwrap().unique_blocks, 1);
        assert_eq!(m.remove(f.id).unwrap().unwrap(), ["x"]);
    }

    #[test]
    fn ids_are_never_reused() {
        let m = Manifest::open_in_memory().unwrap();
        let a = m.add_file("a", 1, "a", 1, &blocks(&[("x", 1)])).unwrap();
        m.remove(a.id).unwrap();
        let b = m.add_file("b", 1, "b", 2, &blocks(&[("y", 1)])).unwrap();
        assert!(b.id > a.id);
    }

    #[test]
    fn removing_a_missing_file_is_none() {
        let m = Manifest::open_in_memory().unwrap();
        assert!(m.remove(42).unwrap().is_none());
    }

    #[test]
    fn the_manifest_survives_a_reopen() {
        let dir = std::env::temp_dir().join(format!("satchel-files-test-{}", std::process::id()));
        let path = dir.join("manifest.db");
        {
            let m = Manifest::open(&path).unwrap();
            m.add_file("a", 4, "a", 1, &blocks(&[("x", 4)])).unwrap();
        }
        let m = Manifest::open(&path).unwrap();
        assert_eq!(m.list().unwrap().len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
