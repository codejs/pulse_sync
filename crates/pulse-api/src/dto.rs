//! Request/response bodies. Exported to `web/src/api/generated/`.

use pulse_core::model::{FileEntry, Folder, FolderMode, FolderStats, FolderStatus};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HealthResponse {
    pub version: String,
    pub device_id: String,
}

/// A folder with its live status and index statistics.
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderView {
    #[serde(flatten)]
    pub folder: Folder,
    pub status: FolderStatus,
    pub stats: FolderStats,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreateFolderRequest {
    /// Absolute path, or starting with `~`.
    pub path: String,
    #[ts(optional)]
    pub label: Option<String>,
    #[ts(optional)]
    pub mode: Option<FolderMode>,
}

#[derive(Debug, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateFolderRequest {
    #[ts(optional)]
    pub label: Option<String>,
    #[ts(optional)]
    pub mode: Option<FolderMode>,
    #[ts(optional)]
    pub paused: Option<bool>,
    #[ts(optional)]
    pub rescan_interval_secs: Option<u32>,
}

#[derive(Debug, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileListQuery {
    /// Directory to list beneath (recursive), relative to the folder root.
    #[ts(optional)]
    pub prefix: Option<String>,
    #[ts(optional)]
    pub include_deleted: Option<bool>,
    #[ts(optional)]
    pub limit: Option<i64>,
    #[ts(optional)]
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileListResponse {
    pub files: Vec<FileEntry>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrowseQuery {
    /// Defaults to the user's home directory. `~` is expanded.
    #[ts(optional)]
    pub path: Option<String>,
    #[ts(optional)]
    pub show_hidden: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrowseResponse {
    pub path: String,
    pub parent: Option<String>,
    pub home: String,
    pub entries: Vec<DirEntry>,
}
