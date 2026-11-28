//! Types shared by the store, the indexer and the HTTP API.
//! Anything deriving `TS` is exported to `web/src/api/generated/`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How a device participates in a shared folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FolderMode {
    /// Local changes are sent and remote changes are applied (default).
    SendReceive,
    /// This device is the source of truth; remote changes are not applied.
    SendOnly,
    /// Backup target: remote changes are applied, local changes are not sent.
    ReceiveOnly,
}

impl FolderMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            FolderMode::SendReceive => "sendReceive",
            FolderMode::SendOnly => "sendOnly",
            FolderMode::ReceiveOnly => "receiveOnly",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "sendReceive" => Some(FolderMode::SendReceive),
            "sendOnly" => Some(FolderMode::SendOnly),
            "receiveOnly" => Some(FolderMode::ReceiveOnly),
            _ => None,
        }
    }
}

/// A folder shared from this device. `path` is local to this device; peers
/// match folders by `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Folder {
    #[ts(type = "string")]
    pub id: crate::FolderId,
    pub label: String,
    pub path: String,
    pub mode: FolderMode,
    pub paused: bool,
    /// Full rescan interval, a fallback in case the watcher misses events.
    pub rescan_interval_secs: u32,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FolderState {
    Idle,
    Scanning,
    Paused,
    Error,
}

/// Runtime status of a folder (not persisted).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderStatus {
    pub state: FolderState,
    pub error: Option<String>,
    pub last_scan_at_ms: Option<i64>,
}

impl Default for FolderStatus {
    fn default() -> Self {
        Self {
            state: FolderState::Idle,
            error: None,
            last_scan_at_ms: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderStats {
    pub files: i64,
    pub bytes: i64,
    pub deleted: i64,
}

/// One entry in a folder's index. Deleted files stay as tombstones so the
/// deletion can propagate to peers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileEntry {
    /// Relative to the folder root, `/`-separated on every platform.
    pub path: String,
    pub size: i64,
    pub modified_ms: i64,
    /// Hex BLAKE3 of the contents; empty for deleted files.
    pub hash: String,
    pub deleted: bool,
    /// Per-folder change sequence; peers use it to ask "what changed since N".
    pub sequence: i64,
    pub chunk_count: i64,
}
