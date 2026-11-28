//! Runs one background task per active folder: an initial scan, then a rescan
//! whenever the watcher reports changes, the user asks, or the periodic
//! interval elapses.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use pulse_core::FolderId;
use pulse_core::events::Event;
use pulse_core::model::{Folder, FolderState, FolderStatus};
use pulse_core::time::now_ms;
use pulse_store::{ApplyOutcome, FileChange, Store};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::scanner::{self, ScanError};
use crate::{INTERNAL_DIR, TEMP_PREFIX, has_folder_marker};

const DEBOUNCE: Duration = Duration::from_millis(1500);
const MIN_RESCAN_INTERVAL: Duration = Duration::from_secs(10);
const BATCH_SIZE: usize = 256;

#[derive(Clone)]
pub struct FolderManager {
    inner: Arc<Inner>,
}

struct Inner {
    store: Store,
    device: u64,
    events: broadcast::Sender<Event>,
    runners: Mutex<HashMap<FolderId, Runner>>,
    statuses: RwLock<HashMap<FolderId, FolderStatus>>,
}

struct Runner {
    rescan: mpsc::Sender<()>,
    cancel: CancellationToken,
    task: JoinHandle<()>,
}

impl FolderManager {
    /// `device` is this device's short id, recorded in version vectors.
    pub fn new(store: Store, device: u64, events: broadcast::Sender<Event>) -> Self {
        Self {
            inner: Arc::new(Inner {
                store,
                device,
                events,
                runners: Mutex::new(HashMap::new()),
                statuses: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub async fn start_all(&self) -> pulse_store::Result<()> {
        for folder in self.inner.store.list_folders().await? {
            self.start(folder).await;
        }
        Ok(())
    }

    /// (Re)starts the runner for `folder` with its current settings.
    pub async fn start(&self, folder: Folder) {
        self.stop(&folder.id).await;
        if folder.paused {
            self.inner.set_status(
                &folder.id,
                FolderStatus {
                    state: FolderState::Paused,
                    ..Default::default()
                },
            );
            return;
        }
        let (rescan_tx, rescan_rx) = mpsc::channel(1);
        let cancel = CancellationToken::new();
        let id = folder.id.clone();
        let task = tokio::spawn(run_folder(
            self.inner.clone(),
            folder,
            cancel.clone(),
            rescan_rx,
        ));
        self.inner.runners.lock().unwrap().insert(
            id,
            Runner {
                rescan: rescan_tx,
                cancel,
                task,
            },
        );
    }

    /// Stops the folder's runner (if any) and waits for it to finish.
    pub async fn stop(&self, id: &FolderId) {
        let runner = self.inner.runners.lock().unwrap().remove(id);
        if let Some(r) = runner {
            r.cancel.cancel();
            let _ = r.task.await;
        }
    }

    /// Stops the runner and forgets the folder's status (folder removed).
    pub async fn remove(&self, id: &FolderId) {
        self.stop(id).await;
        self.inner.statuses.write().unwrap().remove(id);
    }

    /// Requests a rescan. Returns `false` if the folder is not running.
    pub fn rescan(&self, id: &FolderId) -> bool {
        let runners = self.inner.runners.lock().unwrap();
        match runners.get(id) {
            Some(r) => {
                // A full channel already means "rescan pending", which is fine.
                let _ = r.rescan.try_send(());
                true
            }
            None => false,
        }
    }

    pub fn status(&self, id: &FolderId) -> FolderStatus {
        self.inner
            .statuses
            .read()
            .unwrap()
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    pub async fn shutdown(&self) {
        let ids: Vec<FolderId> = self.inner.runners.lock().unwrap().keys().cloned().collect();
        for id in ids {
            self.stop(&id).await;
        }
    }
}

impl Inner {
    fn set_status(&self, id: &FolderId, status: FolderStatus) {
        self.statuses
            .write()
            .unwrap()
            .insert(id.clone(), status.clone());
        let _ = self.events.send(Event::FolderStatusChanged {
            folder_id: id.to_string(),
            status,
        });
    }

    fn last_scan(&self, id: &FolderId) -> Option<i64> {
        self.statuses
            .read()
            .unwrap()
            .get(id)
            .and_then(|s| s.last_scan_at_ms)
    }
}

async fn run_folder(
    inner: Arc<Inner>,
    folder: Folder,
    cancel: CancellationToken,
    mut rescan_rx: mpsc::Receiver<()>,
) {
    let root = PathBuf::from(&folder.path);
    let (fs_tx, mut fs_rx) = mpsc::channel::<()>(1);

    // Keep the debouncer alive for the life of the task; dropping it stops watching.
    let _watcher = {
        let watch_root = root.clone();
        let handler = move |res: DebounceEventResult| match res {
            Ok(events) => {
                if events
                    .iter()
                    .flat_map(|e| e.paths.iter())
                    .any(|p| is_relevant(&watch_root, p))
                {
                    let _ = fs_tx.try_send(());
                }
            }
            Err(errors) => {
                for err in errors {
                    tracing::warn!(%err, "watch error");
                }
                // Something may have been missed; rescan to be safe.
                let _ = fs_tx.try_send(());
            }
        };
        match new_debouncer(DEBOUNCE, None, handler) {
            Ok(mut d) => match d.watch(&root, RecursiveMode::Recursive) {
                Ok(()) => Some(d),
                Err(err) => {
                    tracing::warn!(folder = %folder.id, %err, "cannot watch folder; relying on periodic rescans");
                    None
                }
            },
            Err(err) => {
                tracing::warn!(%err, "cannot create file watcher");
                None
            }
        }
    };

    let period = Duration::from_secs(folder.rescan_interval_secs.into()).max(MIN_RESCAN_INTERVAL);
    let mut interval = tokio::time::interval(period);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = interval.tick() => {}
            Some(()) = fs_rx.recv() => {}
            Some(()) = rescan_rx.recv() => {}
        }
        // Coalesce triggers that arrived while we were waiting.
        while fs_rx.try_recv().is_ok() {}
        while rescan_rx.try_recv().is_ok() {}

        scan_once(&inner, &folder, &root, &cancel).await;
        interval.reset();
    }
}

async fn scan_once(inner: &Arc<Inner>, folder: &Folder, root: &Path, cancel: &CancellationToken) {
    let id = &folder.id;
    let last_scan_at_ms = inner.last_scan(id);
    let fail = |msg: String| {
        tracing::warn!(folder = %id, "{msg}");
        inner.set_status(
            id,
            FolderStatus {
                state: FolderState::Error,
                error: Some(msg),
                last_scan_at_ms,
            },
        );
    };

    if !root.is_dir() {
        return fail(format!(
            "Folder path {} does not exist or is not a directory",
            root.display()
        ));
    }
    if !has_folder_marker(root) {
        return fail(marker_missing_message(root));
    }

    inner.set_status(
        id,
        FolderStatus {
            state: FolderState::Scanning,
            error: None,
            last_scan_at_ms,
        },
    );

    let known = match inner.store.known_files(id).await {
        Ok(k) => k,
        Err(err) => return fail(format!("Cannot read index: {err}")),
    };

    let (tx, mut rx) = mpsc::channel::<FileChange>(BATCH_SIZE * 2);
    let scan_root = root.to_path_buf();
    let scan_cancel = cancel.child_token();
    let blocking_cancel = scan_cancel.clone();
    let scan = tokio::task::spawn_blocking(move || {
        scanner::scan(&scan_root, &known, &blocking_cancel, |c| {
            tx.blocking_send(c).is_ok()
        })
    });

    let mut total = ApplyOutcome::default();
    let mut batch = Vec::with_capacity(BATCH_SIZE);
    let mut store_error = None;
    loop {
        let change = rx.recv().await;
        let done = change.is_none();
        batch.extend(change);
        if (done || batch.len() >= BATCH_SIZE)
            && let Err(err) = flush(inner, id, &mut batch, &mut total).await
        {
            store_error = Some(err);
            break;
        }
        if done {
            break;
        }
    }
    // Dropping the receiver makes the scanner stop early on a store error.
    drop(rx);
    if store_error.is_some() {
        scan_cancel.cancel();
    }
    let result = scan.await;

    if total.changed > 0 || total.deleted > 0 {
        let _ = inner.events.send(Event::IndexUpdated {
            folder_id: id.to_string(),
            changed: total.changed,
            deleted: total.deleted,
        });
    }

    if let Some(err) = store_error {
        return fail(format!("Cannot update index: {err}"));
    }
    match result {
        Ok(Ok(summary)) => {
            tracing::info!(
                folder = %id,
                seen = summary.files_seen,
                hashed = summary.files_hashed,
                changed = total.changed,
                deleted = total.deleted,
                errors = summary.errors,
                "scan finished"
            );
            let error = (summary.errors > 0).then(|| {
                format!(
                    "{} file(s) could not be read; see the daemon log",
                    summary.errors
                )
            });
            inner.set_status(
                id,
                FolderStatus {
                    state: FolderState::Idle,
                    error,
                    last_scan_at_ms: Some(now_ms()),
                },
            );
        }
        Ok(Err(ScanError::Cancelled)) => {}
        Ok(Err(ScanError::MarkerMissing)) => fail(marker_missing_message(root)),
        Ok(Err(err)) => fail(err.to_string()),
        Err(join_err) => fail(format!("Scanner crashed: {join_err}")),
    }
}

fn marker_missing_message(root: &Path) -> String {
    format!(
        "Folder marker {INTERNAL_DIR}/ is missing in {}. If the drive is unmounted, remount it; \
         otherwise re-add the folder.",
        root.display()
    )
}

async fn flush(
    inner: &Inner,
    id: &FolderId,
    batch: &mut Vec<FileChange>,
    total: &mut ApplyOutcome,
) -> pulse_store::Result<()> {
    if batch.is_empty() {
        return Ok(());
    }
    let out = inner.store.apply_changes(id, inner.device, batch).await?;
    total.changed += out.changed;
    total.deleted += out.deleted;
    batch.clear();
    Ok(())
}

/// Filters out watcher noise from our own internal directory and temp files.
fn is_relevant(root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        // Unexpected shape (e.g. symlinked root); rescanning is harmless.
        return true;
    };
    let mut components = rel.components();
    if components
        .next()
        .is_some_and(|c| c.as_os_str() == INTERNAL_DIR)
    {
        return false;
    }
    !rel.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(TEMP_PREFIX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulse_core::model::FolderMode;

    async fn wait_for<F: Fn() -> bool>(what: &str, f: F) {
        for _ in 0..100 {
            if f() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("timed out waiting for {what}");
    }

    #[test]
    fn ignores_internal_and_temp_paths() {
        let root = Path::new("/f");
        assert!(!is_relevant(root, Path::new("/f/.pulse/x")));
        assert!(!is_relevant(root, Path::new("/f/a/.pulse-tmp-123")));
        assert!(is_relevant(root, Path::new("/f/a/b.txt")));
        assert!(is_relevant(root, Path::new("/f/a/.pulse")));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn indexes_and_follows_changes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        crate::init_folder_marker(&root).unwrap();
        std::fs::write(root.join("a.txt"), "hello").unwrap();

        let store = Store::open_in_memory().await.unwrap();
        let folder = Folder {
            id: FolderId::generate(),
            label: "t".into(),
            path: root.to_string_lossy().into_owned(),
            mode: FolderMode::SendReceive,
            paused: false,
            rescan_interval_secs: 3600,
            created_at_ms: 0,
        };
        store.insert_folder(&folder).await.unwrap();
        let (events, _) = broadcast::channel(64);
        let manager = FolderManager::new(store.clone(), 1, events);
        manager.start(folder.clone()).await;

        let count = |store: Store, id: FolderId| async move {
            store.folder_stats(&id).await.unwrap().files
        };
        for _ in 0..100 {
            if count(store.clone(), folder.id.clone()).await == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert_eq!(count(store.clone(), folder.id.clone()).await, 1);
        wait_for("idle", || {
            manager.status(&folder.id).state == FolderState::Idle
        })
        .await;

        // A new file is picked up by the watcher (or the explicit rescan as a fallback).
        std::fs::write(root.join("b.txt"), "world").unwrap();
        tokio::time::sleep(DEBOUNCE + Duration::from_millis(500)).await;
        manager.rescan(&folder.id);
        for _ in 0..100 {
            if count(store.clone(), folder.id.clone()).await == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert_eq!(count(store.clone(), folder.id.clone()).await, 2);

        // Removing the marker must not tombstone everything.
        std::fs::remove_dir(root.join(INTERNAL_DIR)).unwrap();
        std::fs::remove_file(root.join("a.txt")).unwrap();
        manager.rescan(&folder.id);
        wait_for("error state", || {
            manager.status(&folder.id).state == FolderState::Error
        })
        .await;
        assert_eq!(store.folder_stats(&folder.id).await.unwrap().deleted, 0);

        manager.shutdown().await;
    }
}
