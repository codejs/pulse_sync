//! SQLite metadata store.
//!
//! Holds the configured folders and, per folder, the file index (with
//! tombstones and version vectors) plus each file's chunk list.

use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use pulse_core::chunker::{Chunk, FileDigest};
use pulse_core::model::{FileEntry, Folder, FolderMode, FolderStats};
use pulse_core::{FolderId, VersionVector};
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteRow,
    SqliteSynchronous,
};
use sqlx::{Row, Sqlite, Transaction};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("not found")]
    NotFound,
    #[error("corrupt row: {0}")]
    Corrupt(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// What the indexer remembers about a file, used to skip unchanged files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownFile {
    pub size: i64,
    pub mtime_ns: i64,
    pub hash: String,
    pub deleted: bool,
}

/// A file as found on disk by the scanner.
#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: String,
    pub mtime_ns: i64,
    pub digest: FileDigest,
}

#[derive(Debug, Clone)]
pub enum FileChange {
    Upsert(ScannedFile),
    Delete { path: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplyOutcome {
    /// Files whose contents changed (new version + sequence).
    pub changed: i64,
    /// Files newly tombstoned.
    pub deleted: i64,
}

/// Partial update for a folder's settings.
#[derive(Debug, Clone, Default)]
pub struct FolderPatch {
    pub label: Option<String>,
    pub mode: Option<FolderMode>,
    pub paused: Option<bool>,
    pub rescan_interval_secs: Option<u32>,
}

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub async fn open(path: &Path) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await?;
        Self::init(pool).await
    }

    /// A private in-memory database, for tests.
    pub async fn open_in_memory() -> Result<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        // One connection: each in-memory connection would otherwise be its own database.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await?;
        Self::init(pool).await
    }

    async fn init(pool: SqlitePool) -> Result<Self> {
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    // ---- folders -------------------------------------------------------

    pub async fn list_folders(&self) -> Result<Vec<Folder>> {
        let rows = sqlx::query("SELECT * FROM folders ORDER BY label COLLATE NOCASE, id")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(folder_from_row).collect()
    }

    pub async fn get_folder(&self, id: &FolderId) -> Result<Folder> {
        let row = sqlx::query("SELECT * FROM folders WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::NotFound)?;
        folder_from_row(&row)
    }

    pub async fn insert_folder(&self, f: &Folder) -> Result<()> {
        sqlx::query(
            "INSERT INTO folders (id, label, path, mode, paused, rescan_interval_secs, created_at_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(f.id.as_str())
        .bind(&f.label)
        .bind(&f.path)
        .bind(f.mode.as_str())
        .bind(f.paused)
        .bind(f.rescan_interval_secs)
        .bind(f.created_at_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_folder(&self, id: &FolderId, patch: &FolderPatch) -> Result<Folder> {
        let res = sqlx::query(
            "UPDATE folders SET
                label = COALESCE(?, label),
                mode = COALESCE(?, mode),
                paused = COALESCE(?, paused),
                rescan_interval_secs = COALESCE(?, rescan_interval_secs)
             WHERE id = ?",
        )
        .bind(patch.label.as_deref())
        .bind(patch.mode.map(|m| m.as_str()))
        .bind(patch.paused)
        .bind(patch.rescan_interval_secs)
        .bind(id.as_str())
        .execute(&self.pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        self.get_folder(id).await
    }

    /// Removes the folder and its index. Files on disk are untouched.
    pub async fn delete_folder(&self, id: &FolderId) -> Result<()> {
        let res = sqlx::query("DELETE FROM folders WHERE id = ?")
            .bind(id.as_str())
            .execute(&self.pool)
            .await?;
        if res.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub async fn folder_stats(&self, id: &FolderId) -> Result<FolderStats> {
        let row = sqlx::query(
            "SELECT
                COALESCE(SUM(CASE WHEN deleted = 0 THEN 1 ELSE 0 END), 0) AS files,
                COALESCE(SUM(CASE WHEN deleted = 0 THEN size ELSE 0 END), 0) AS bytes,
                COALESCE(SUM(deleted), 0) AS deleted
             FROM files WHERE folder_id = ?",
        )
        .bind(id.as_str())
        .fetch_one(&self.pool)
        .await?;
        Ok(FolderStats {
            files: row.try_get("files")?,
            bytes: row.try_get("bytes")?,
            deleted: row.try_get("deleted")?,
        })
    }

    // ---- file index ----------------------------------------------------

    pub async fn known_files(&self, id: &FolderId) -> Result<HashMap<String, KnownFile>> {
        let rows = sqlx::query(
            "SELECT path, size, mtime_ns, hash, deleted FROM files WHERE folder_id = ?",
        )
        .bind(id.as_str())
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                Ok((
                    r.try_get("path")?,
                    KnownFile {
                        size: r.try_get("size")?,
                        mtime_ns: r.try_get("mtime_ns")?,
                        hash: r.try_get("hash")?,
                        deleted: r.try_get("deleted")?,
                    },
                ))
            })
            .collect()
    }

    /// Applies scanner results in one transaction. Content changes bump the
    /// file's version vector (for `device`) and get a new folder sequence;
    /// a changed mtime with identical contents only updates the mtime.
    pub async fn apply_changes(
        &self,
        id: &FolderId,
        device: u64,
        changes: &[FileChange],
    ) -> Result<ApplyOutcome> {
        let mut tx = self.pool.begin().await?;
        let mut sequence: i64 = sqlx::query_scalar("SELECT sequence FROM folders WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(StoreError::NotFound)?;
        let mut outcome = ApplyOutcome::default();

        for change in changes {
            let path = match change {
                FileChange::Upsert(f) => &f.path,
                FileChange::Delete { path } => path,
            };
            let existing = sqlx::query(
                "SELECT hash, deleted, version FROM files WHERE folder_id = ? AND path = ?",
            )
            .bind(id.as_str())
            .bind(path)
            .fetch_optional(&mut *tx)
            .await?;
            let (old_hash, old_deleted, mut version) = match &existing {
                Some(r) => (
                    r.try_get::<String, _>("hash")?,
                    r.try_get::<bool, _>("deleted")?,
                    parse_version(&r.try_get::<String, _>("version")?)?,
                ),
                None => (String::new(), true, VersionVector::new()),
            };

            match change {
                FileChange::Upsert(f) => {
                    let new_hash = f.digest.hash.to_hex();
                    if existing.is_some() && !old_deleted && old_hash == new_hash {
                        sqlx::query(
                            "UPDATE files SET mtime_ns = ? WHERE folder_id = ? AND path = ?",
                        )
                        .bind(f.mtime_ns)
                        .bind(id.as_str())
                        .bind(&f.path)
                        .execute(&mut *tx)
                        .await?;
                        continue;
                    }
                    version.bump(device);
                    sequence += 1;
                    upsert_file(&mut tx, id, f, &new_hash, &version, sequence).await?;
                    outcome.changed += 1;
                }
                FileChange::Delete { path } => {
                    if existing.is_none() || old_deleted {
                        continue;
                    }
                    version.bump(device);
                    sequence += 1;
                    sqlx::query("DELETE FROM chunks WHERE folder_id = ? AND path = ?")
                        .bind(id.as_str())
                        .bind(path)
                        .execute(&mut *tx)
                        .await?;
                    sqlx::query(
                        "UPDATE files SET deleted = 1, hash = '', size = 0, chunk_count = 0,
                                version = ?, sequence = ?
                         WHERE folder_id = ? AND path = ?",
                    )
                    .bind(version_json(&version))
                    .bind(sequence)
                    .bind(id.as_str())
                    .bind(path)
                    .execute(&mut *tx)
                    .await?;
                    outcome.deleted += 1;
                }
            }
        }

        sqlx::query("UPDATE folders SET sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id.as_str())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(outcome)
    }

    /// Lists index entries under `prefix` (a directory path, no trailing slash).
    pub async fn list_files(
        &self,
        id: &FolderId,
        prefix: Option<&str>,
        include_deleted: bool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<FileEntry>> {
        let like = match prefix
            .map(|p| p.trim_matches('/'))
            .filter(|p| !p.is_empty())
        {
            Some(p) => format!("{}/%", escape_like(p)),
            None => "%".to_owned(),
        };
        let rows = sqlx::query(
            "SELECT path, size, mtime_ns, hash, deleted, sequence, chunk_count FROM files
             WHERE folder_id = ? AND path LIKE ? ESCAPE '\\' AND (? OR deleted = 0)
             ORDER BY path LIMIT ? OFFSET ?",
        )
        .bind(id.as_str())
        .bind(like)
        .bind(include_deleted)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                Ok(FileEntry {
                    path: r.try_get("path")?,
                    size: r.try_get("size")?,
                    modified_ms: r.try_get::<i64, _>("mtime_ns")? / 1_000_000,
                    hash: r.try_get("hash")?,
                    deleted: r.try_get("deleted")?,
                    sequence: r.try_get("sequence")?,
                    chunk_count: r.try_get("chunk_count")?,
                })
            })
            .collect()
    }

    pub async fn file_chunks(&self, id: &FolderId, path: &str) -> Result<Vec<Chunk>> {
        let rows = sqlx::query(
            "SELECT offset, len, hash FROM chunks WHERE folder_id = ? AND path = ? ORDER BY idx",
        )
        .bind(id.as_str())
        .bind(path)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                let hash: String = r.try_get("hash")?;
                Ok(Chunk {
                    offset: r.try_get::<i64, _>("offset")? as u64,
                    len: r.try_get::<i64, _>("len")? as u32,
                    hash: pulse_core::chunker::Hash::from_hex(&hash)
                        .ok_or_else(|| StoreError::Corrupt(format!("bad chunk hash {hash}")))?,
                })
            })
            .collect()
    }

    pub async fn file_version(&self, id: &FolderId, path: &str) -> Result<VersionVector> {
        let v: String =
            sqlx::query_scalar("SELECT version FROM files WHERE folder_id = ? AND path = ?")
                .bind(id.as_str())
                .bind(path)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(StoreError::NotFound)?;
        parse_version(&v)
    }
}

async fn upsert_file(
    tx: &mut Transaction<'_, Sqlite>,
    id: &FolderId,
    f: &ScannedFile,
    hash: &str,
    version: &VersionVector,
    sequence: i64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO files (folder_id, path, size, mtime_ns, hash, deleted, version, sequence, chunk_count)
         VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?)
         ON CONFLICT (folder_id, path) DO UPDATE SET
            size = excluded.size, mtime_ns = excluded.mtime_ns, hash = excluded.hash,
            deleted = 0, version = excluded.version, sequence = excluded.sequence,
            chunk_count = excluded.chunk_count",
    )
    .bind(id.as_str())
    .bind(&f.path)
    .bind(f.digest.size as i64)
    .bind(f.mtime_ns)
    .bind(hash)
    .bind(version_json(version))
    .bind(sequence)
    .bind(f.digest.chunks.len() as i64)
    .execute(&mut **tx)
    .await?;

    sqlx::query("DELETE FROM chunks WHERE folder_id = ? AND path = ?")
        .bind(id.as_str())
        .bind(&f.path)
        .execute(&mut **tx)
        .await?;
    for (idx, c) in f.digest.chunks.iter().enumerate() {
        sqlx::query("INSERT INTO chunks (folder_id, path, idx, offset, len, hash) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(id.as_str())
            .bind(&f.path)
            .bind(idx as i64)
            .bind(c.offset as i64)
            .bind(c.len as i64)
            .bind(c.hash.to_hex())
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

fn folder_from_row(r: &SqliteRow) -> Result<Folder> {
    let id: String = r.try_get("id")?;
    let mode: String = r.try_get("mode")?;
    Ok(Folder {
        id: id
            .parse()
            .map_err(|_| StoreError::Corrupt(format!("bad folder id {id}")))?,
        label: r.try_get("label")?,
        path: r.try_get("path")?,
        mode: FolderMode::parse(&mode)
            .ok_or_else(|| StoreError::Corrupt(format!("bad mode {mode}")))?,
        paused: r.try_get("paused")?,
        rescan_interval_secs: r.try_get("rescan_interval_secs")?,
        created_at_ms: r.try_get("created_at_ms")?,
    })
}

fn parse_version(s: &str) -> Result<VersionVector> {
    serde_json::from_str(s).map_err(|e| StoreError::Corrupt(format!("bad version vector: {e}")))
}

fn version_json(v: &VersionVector) -> String {
    serde_json::to_string(v).expect("version vector serializes")
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulse_core::VersionOrdering;
    use pulse_core::chunker::digest;

    const DEV: u64 = 0xA;

    fn folder() -> Folder {
        Folder {
            id: FolderId::generate(),
            label: "Docs".into(),
            path: "/tmp/docs".into(),
            mode: FolderMode::SendReceive,
            paused: false,
            rescan_interval_secs: 3600,
            created_at_ms: 0,
        }
    }

    fn scanned(path: &str, contents: &[u8], mtime_ns: i64) -> FileChange {
        FileChange::Upsert(ScannedFile {
            path: path.into(),
            mtime_ns,
            digest: digest(contents).unwrap(),
        })
    }

    #[tokio::test]
    async fn folder_crud() {
        let store = Store::open_in_memory().await.unwrap();
        let f = folder();
        store.insert_folder(&f).await.unwrap();
        assert_eq!(store.list_folders().await.unwrap(), vec![f.clone()]);

        let patch = FolderPatch {
            paused: Some(true),
            mode: Some(FolderMode::SendOnly),
            ..Default::default()
        };
        let updated = store.update_folder(&f.id, &patch).await.unwrap();
        assert!(updated.paused);
        assert_eq!(updated.mode, FolderMode::SendOnly);
        assert_eq!(updated.label, "Docs");

        store.delete_folder(&f.id).await.unwrap();
        assert!(matches!(
            store.get_folder(&f.id).await,
            Err(StoreError::NotFound)
        ));
    }

    #[tokio::test]
    async fn upsert_touch_and_delete_track_versions() {
        let store = Store::open_in_memory().await.unwrap();
        let f = folder();
        store.insert_folder(&f).await.unwrap();

        let out = store
            .apply_changes(
                &f.id,
                DEV,
                &[scanned("a.txt", b"one", 1), scanned("dir/b.txt", b"two", 1)],
            )
            .await
            .unwrap();
        assert_eq!(
            out,
            ApplyOutcome {
                changed: 2,
                deleted: 0
            }
        );
        let v1 = store.file_version(&f.id, "a.txt").await.unwrap();

        // Same contents, new mtime: no new version.
        let out = store
            .apply_changes(&f.id, DEV, &[scanned("a.txt", b"one", 2)])
            .await
            .unwrap();
        assert_eq!(out.changed, 0);
        assert_eq!(store.file_version(&f.id, "a.txt").await.unwrap(), v1);
        assert_eq!(store.known_files(&f.id).await.unwrap()["a.txt"].mtime_ns, 2);

        // Changed contents: newer version, chunks replaced.
        store
            .apply_changes(&f.id, DEV, &[scanned("a.txt", b"one!", 3)])
            .await
            .unwrap();
        let v2 = store.file_version(&f.id, "a.txt").await.unwrap();
        assert_eq!(v2.compare(&v1), VersionOrdering::Newer);
        assert_eq!(store.file_chunks(&f.id, "a.txt").await.unwrap().len(), 1);

        // Delete leaves a tombstone; deleting twice is a no-op.
        let del = FileChange::Delete {
            path: "a.txt".into(),
        };
        assert_eq!(
            store
                .apply_changes(&f.id, DEV, std::slice::from_ref(&del))
                .await
                .unwrap()
                .deleted,
            1
        );
        assert_eq!(
            store
                .apply_changes(&f.id, DEV, &[del])
                .await
                .unwrap()
                .deleted,
            0
        );
        assert!(store.file_chunks(&f.id, "a.txt").await.unwrap().is_empty());

        let stats = store.folder_stats(&f.id).await.unwrap();
        assert_eq!(
            stats,
            FolderStats {
                files: 1,
                bytes: 3,
                deleted: 1
            }
        );

        let live = store.list_files(&f.id, None, false, 100, 0).await.unwrap();
        assert_eq!(
            live.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
            ["dir/b.txt"]
        );
        let all = store.list_files(&f.id, None, true, 100, 0).await.unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.windows(2).all(|w| w[0].sequence != w[1].sequence));
        let in_dir = store
            .list_files(&f.id, Some("dir"), false, 100, 0)
            .await
            .unwrap();
        assert_eq!(in_dir.len(), 1);
    }

    #[tokio::test]
    async fn opens_file_database() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("index.db")).await.unwrap();
        store.insert_folder(&folder()).await.unwrap();
        drop(store);
        let reopened = Store::open(&dir.path().join("index.db")).await.unwrap();
        assert_eq!(reopened.list_folders().await.unwrap().len(), 1);
    }
}
