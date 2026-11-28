use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use pulse_core::FolderId;
use pulse_core::events::Event;
use pulse_core::model::{Folder, FolderMode};
use pulse_core::time::now_ms;
use pulse_store::FolderPatch;

use super::fs::{resolve_user_path, to_utf8};
use crate::AppState;
use crate::dto::{
    CreateFolderRequest, FileListQuery, FileListResponse, FolderView, UpdateFolderRequest,
};
use crate::error::ApiError;

const DEFAULT_RESCAN_SECS: u32 = 3600;
const MIN_RESCAN_SECS: u32 = 10;
const MAX_FILE_PAGE: i64 = 5000;

fn parse_id(raw: &str) -> Result<FolderId, ApiError> {
    raw.parse()
        .map_err(|_| ApiError::not_found("no such folder"))
}

async fn view(state: &AppState, folder: Folder) -> Result<FolderView, ApiError> {
    let stats = state.store.folder_stats(&folder.id).await?;
    let status = state.manager.status(&folder.id);
    Ok(FolderView {
        folder,
        status,
        stats,
    })
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<FolderView>>, ApiError> {
    let mut out = Vec::new();
    for folder in state.store.list_folders().await? {
        out.push(view(&state, folder).await?);
    }
    Ok(Json(out))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<FolderView>, ApiError> {
    let folder = state.store.get_folder(&parse_id(&id)?).await?;
    Ok(Json(view(&state, folder).await?))
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateFolderRequest>,
) -> Result<(StatusCode, Json<FolderView>), ApiError> {
    let requested = resolve_user_path(&req.path)?;
    let root = requested
        .canonicalize()
        .map_err(|e| ApiError::bad_request(format!("cannot open {}: {e}", requested.display())))?;
    if !root.is_dir() {
        return Err(ApiError::bad_request(format!(
            "{} is not a directory",
            root.display()
        )));
    }
    if root.parent().is_none() {
        return Err(ApiError::bad_request(
            "refusing to share the filesystem root",
        ));
    }

    let data_dir = state
        .data_dir
        .canonicalize()
        .unwrap_or_else(|_| state.data_dir.clone());
    if root.starts_with(&data_dir) || data_dir.starts_with(&root) {
        return Err(ApiError::bad_request(format!(
            "folder overlaps the daemon's data directory {}",
            data_dir.display()
        )));
    }
    for existing in state.store.list_folders().await? {
        let other = std::path::Path::new(&existing.path);
        if root.starts_with(other) || other.starts_with(&root) {
            return Err(ApiError::conflict(format!(
                "overlaps the existing folder \"{}\" ({})",
                existing.label, existing.path
            )));
        }
    }

    let label = req
        .label
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .or_else(|| root.file_name().and_then(|n| n.to_str()).map(str::to_owned))
        .unwrap_or_else(|| "Folder".to_owned());

    let folder = Folder {
        id: FolderId::generate(),
        label,
        path: to_utf8(&root)?,
        mode: req.mode.unwrap_or(FolderMode::SendReceive),
        paused: false,
        rescan_interval_secs: DEFAULT_RESCAN_SECS,
        created_at_ms: now_ms(),
    };

    pulse_fs::init_folder_marker(&root)
        .map_err(|e| ApiError::bad_request(format!("cannot write to {}: {e}", root.display())))?;
    state.store.insert_folder(&folder).await?;
    state.manager.start(folder.clone()).await;
    let _ = state.events.send(Event::FoldersChanged);

    Ok((StatusCode::CREATED, Json(view(&state, folder).await?)))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateFolderRequest>,
) -> Result<Json<FolderView>, ApiError> {
    let id = parse_id(&id)?;
    let before = state.store.get_folder(&id).await?;
    if let Some(label) = &req.label
        && label.trim().is_empty()
    {
        return Err(ApiError::bad_request("label cannot be empty"));
    }
    if req
        .rescan_interval_secs
        .is_some_and(|s| s < MIN_RESCAN_SECS)
    {
        return Err(ApiError::bad_request(format!(
            "rescan interval must be at least {MIN_RESCAN_SECS}s"
        )));
    }

    let patch = FolderPatch {
        label: req.label.map(|l| l.trim().to_owned()),
        mode: req.mode,
        paused: req.paused,
        rescan_interval_secs: req.rescan_interval_secs,
    };
    let after = state.store.update_folder(&id, &patch).await?;
    if after.paused != before.paused || after.rescan_interval_secs != before.rescan_interval_secs {
        state.manager.start(after.clone()).await;
    }
    let _ = state.events.send(Event::FoldersChanged);
    Ok(Json(view(&state, after).await?))
}

/// Stops syncing and forgets the index. Files on disk are not touched.
pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let id = parse_id(&id)?;
    state.store.get_folder(&id).await?;
    state.manager.remove(&id).await;
    state.store.delete_folder(&id).await?;
    let _ = state.events.send(Event::FoldersChanged);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn rescan(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let id = parse_id(&id)?;
    let folder = state.store.get_folder(&id).await?;
    if folder.paused {
        return Err(ApiError::conflict("folder is paused"));
    }
    state.manager.rescan(&id);
    Ok(StatusCode::ACCEPTED)
}

pub async fn files(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<FileListQuery>,
) -> Result<Json<FileListResponse>, ApiError> {
    let id = parse_id(&id)?;
    state.store.get_folder(&id).await?;
    let limit = q.limit.unwrap_or(500).clamp(1, MAX_FILE_PAGE);
    let offset = q.offset.unwrap_or(0).max(0);
    let files = state
        .store
        .list_files(
            &id,
            q.prefix.as_deref(),
            q.include_deleted.unwrap_or(false),
            limit,
            offset,
        )
        .await?;
    Ok(Json(FileListResponse {
        files,
        limit,
        offset,
    }))
}
