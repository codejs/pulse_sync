use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::FolderStatus;

/// Pushed to UI clients over `/api/events`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Event {
    FoldersChanged,
    FolderStatusChanged {
        folder_id: String,
        status: FolderStatus,
    },
    IndexUpdated {
        folder_id: String,
        changed: i64,
        deleted: i64,
    },
}
