//! Walks a folder and reports what changed compared with the index.
//!
//! Runs on a blocking thread: it stats every file, and hashes only files whose
//! size or mtime differ from what the index already knows.

use std::collections::{HashMap, HashSet};
use std::fs::{File, Metadata};
use std::io::BufReader;
use std::path::{Component, Path};

use ignore::WalkBuilder;
use pulse_core::chunker::digest;
use pulse_core::time::system_time_ns;
use pulse_store::{FileChange, KnownFile, ScannedFile};
use tokio_util::sync::CancellationToken;

use crate::{IGNORE_FILE, INTERNAL_DIR, TEMP_PREFIX, has_folder_marker};

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("folder root is not accessible: {0}")]
    Root(std::io::Error),
    /// The `.pulse/` marker is gone (e.g. the drive was unmounted). Nothing
    /// may be recorded as deleted, since the files may still exist elsewhere.
    #[error("folder marker {INTERNAL_DIR}/ is missing")]
    MarkerMissing,
    #[error("scan cancelled")]
    Cancelled,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScanSummary {
    pub files_seen: u64,
    pub files_hashed: u64,
    pub errors: u64,
}

/// Scans `root`, calling `emit` for every new/changed file and, at the end,
/// for every indexed file that no longer exists. `emit` returns `false` to
/// abort (receiver gone).
pub fn scan(
    root: &Path,
    known: &HashMap<String, KnownFile>,
    cancel: &CancellationToken,
    mut emit: impl FnMut(FileChange) -> bool,
) -> Result<ScanSummary, ScanError> {
    std::fs::read_dir(root).map_err(ScanError::Root)?;
    if !has_folder_marker(root) {
        return Err(ScanError::MarkerMissing);
    }

    let mut summary = ScanSummary::default();
    let mut seen: HashSet<String> = HashSet::new();
    // Paths we could not read: never tombstone anything beneath them.
    let mut uncertain: Vec<String> = Vec::new();
    let mut uncertain_everything = false;

    let walker = WalkBuilder::new(root)
        .standard_filters(false)
        .follow_links(false)
        .add_custom_ignore_filename(IGNORE_FILE)
        .filter_entry(|e| !(e.depth() == 1 && e.file_name() == INTERNAL_DIR))
        .build();

    for entry in walker {
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                summary.errors += 1;
                tracing::warn!(%err, "scan error");
                match error_path(&err).and_then(|p| relative_path(root, p)) {
                    Some(rel) => uncertain.push(rel),
                    None => uncertain_everything = true,
                }
                continue;
            }
        };
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        // M1 indexes regular files only; symlinks and special files are skipped.
        if !file_type.is_file() {
            continue;
        }
        let Some(rel) = relative_path(root, entry.path()) else {
            tracing::warn!(path = %entry.path().display(), "skipping non-UTF-8 path");
            continue;
        };
        if file_name(&rel).starts_with(TEMP_PREFIX) {
            continue;
        }
        summary.files_seen += 1;
        seen.insert(rel.clone());

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(err) => {
                summary.errors += 1;
                tracing::warn!(path = %rel, %err, "cannot stat file");
                continue;
            }
        };
        let (size, mtime_ns) = size_mtime(&meta);
        if let Some(k) = known.get(&rel)
            && !k.deleted
            && k.size == size
            && k.mtime_ns == mtime_ns
        {
            continue;
        }

        match hash_stable(entry.path(), size, mtime_ns) {
            Ok(Some(digest)) => {
                summary.files_hashed += 1;
                if !emit(FileChange::Upsert(ScannedFile {
                    path: rel,
                    mtime_ns,
                    digest,
                })) {
                    return Err(ScanError::Cancelled);
                }
            }
            // Still being written; the watcher will report it again.
            Ok(None) => tracing::debug!(path = %rel, "file changed while hashing, deferring"),
            Err(err) => {
                summary.errors += 1;
                tracing::warn!(path = %rel, %err, "cannot read file");
            }
        }
    }

    // Re-check right before tombstoning: the folder may have vanished
    // mid-walk, in which case "not seen" doesn't mean "deleted".
    if !has_folder_marker(root) {
        return Err(ScanError::MarkerMissing);
    }
    if !uncertain_everything {
        for (path, k) in known {
            if k.deleted || seen.contains(path) || is_under_any(path, &uncertain) {
                continue;
            }
            if cancel.is_cancelled() {
                return Err(ScanError::Cancelled);
            }
            if !emit(FileChange::Delete { path: path.clone() }) {
                return Err(ScanError::Cancelled);
            }
        }
    }

    Ok(summary)
}

/// Hashes a file, returning `None` if it changed underneath us.
fn hash_stable(
    path: &Path,
    size: i64,
    mtime_ns: i64,
) -> std::io::Result<Option<pulse_core::chunker::FileDigest>> {
    let file = File::open(path)?;
    let digest = digest(BufReader::with_capacity(256 * 1024, &file))?;
    let after = size_mtime(&file.metadata()?);
    if after != (size, mtime_ns) || digest.size as i64 != size {
        return Ok(None);
    }
    Ok(Some(digest))
}

fn size_mtime(meta: &Metadata) -> (i64, i64) {
    let mtime = meta.modified().map(system_time_ns).unwrap_or(0);
    (meta.len() as i64, mtime)
}

/// `root/a/b.txt` -> `a/b.txt`, always `/`-separated. `None` for non-UTF-8 or
/// paths outside `root`.
pub fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_str()?),
            _ => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn is_under_any(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|p| {
        path == p || (path.starts_with(p.as_str()) && path.as_bytes().get(p.len()) == Some(&b'/'))
    })
}

fn error_path(err: &ignore::Error) -> Option<&Path> {
    match err {
        ignore::Error::WithPath { path, .. } => Some(path),
        ignore::Error::WithDepth { err, .. } | ignore::Error::WithLineNumber { err, .. } => {
            error_path(err)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn run(root: &Path, known: &HashMap<String, KnownFile>) -> (Vec<FileChange>, ScanSummary) {
        let mut out = Vec::new();
        let summary = scan(root, known, &CancellationToken::new(), |c| {
            out.push(c);
            true
        })
        .unwrap();
        (out, summary)
    }

    fn upserted(changes: &[FileChange]) -> Vec<String> {
        let mut v: Vec<_> = changes
            .iter()
            .filter_map(|c| match c {
                FileChange::Upsert(f) => Some(f.path.clone()),
                _ => None,
            })
            .collect();
        v.sort();
        v
    }

    fn as_known(changes: &[FileChange]) -> HashMap<String, KnownFile> {
        changes
            .iter()
            .filter_map(|c| match c {
                FileChange::Upsert(f) => Some((
                    f.path.clone(),
                    KnownFile {
                        size: f.digest.size as i64,
                        mtime_ns: f.mtime_ns,
                        hash: f.digest.hash.to_hex(),
                        deleted: false,
                    },
                )),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn honours_ignore_rules_and_internal_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        crate::init_folder_marker(root).unwrap();
        fs::create_dir_all(root.join("src/build")).unwrap();
        fs::create_dir_all(root.join("logs")).unwrap();
        fs::write(root.join(".pulse/state"), "internal").unwrap();
        fs::write(root.join(".pulseignore"), "logs/\n*.tmp\n").unwrap();
        fs::write(root.join("src/.pulseignore"), "build/\n").unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("src/build/out.o"), "obj").unwrap();
        fs::write(root.join("logs/app.log"), "log").unwrap();
        fs::write(root.join("notes.tmp"), "tmp").unwrap();
        fs::write(root.join(".hidden"), "dotfiles sync too").unwrap();
        fs::write(root.join(".pulse-tmp-abc"), "partial").unwrap();

        let (changes, _) = run(root, &HashMap::new());
        assert_eq!(
            upserted(&changes),
            [".hidden", ".pulseignore", "src/.pulseignore", "src/main.rs"]
        );
    }

    #[test]
    fn rescan_reports_only_changes_and_deletions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        crate::init_folder_marker(root).unwrap();
        fs::write(root.join("keep.txt"), "same").unwrap();
        fs::write(root.join("edit.txt"), "before").unwrap();
        fs::write(root.join("gone.txt"), "bye").unwrap();

        let (first, summary) = run(root, &HashMap::new());
        assert_eq!(summary.files_hashed, 3);
        let known = as_known(&first);

        fs::write(root.join("edit.txt"), "after, longer").unwrap();
        fs::remove_file(root.join("gone.txt")).unwrap();
        fs::write(root.join("new.txt"), "hi").unwrap();

        let (second, summary) = run(root, &known);
        assert_eq!(upserted(&second), ["edit.txt", "new.txt"]);
        assert_eq!(summary.files_hashed, 2);
        let deleted: Vec<_> = second
            .iter()
            .filter_map(|c| match c {
                FileChange::Delete { path } => Some(path.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(deleted, ["gone.txt"]);
    }

    #[test]
    fn missing_marker_never_tombstones() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        crate::init_folder_marker(root).unwrap();
        fs::write(root.join("a.txt"), "a").unwrap();
        let (first, _) = run(root, &HashMap::new());
        let known = as_known(&first);

        // Simulate an unmounted drive: marker and files gone.
        fs::remove_dir(root.join(INTERNAL_DIR)).unwrap();
        fs::remove_file(root.join("a.txt")).unwrap();
        let mut emitted = Vec::new();
        let result = scan(root, &known, &CancellationToken::new(), |c| {
            emitted.push(c);
            true
        });
        assert!(matches!(result, Err(ScanError::MarkerMissing)));
        assert!(emitted.is_empty());
    }

    #[test]
    fn relative_paths_are_slash_separated() {
        let root = Path::new("/data/folder");
        assert_eq!(
            relative_path(root, &root.join("a").join("b.txt")).as_deref(),
            Some("a/b.txt")
        );
        assert_eq!(relative_path(root, root), None);
        assert_eq!(relative_path(root, Path::new("/elsewhere/x")), None);
    }

    #[test]
    fn prefix_matching_respects_path_boundaries() {
        let prefixes = vec!["photos".to_string()];
        assert!(is_under_any("photos/a.jpg", &prefixes));
        assert!(is_under_any("photos", &prefixes));
        assert!(!is_under_any("photos2/a.jpg", &prefixes));
    }
}
