use std::path::{Path, PathBuf};

use axum::Json;
use axum::extract::Query;

use crate::dto::{BrowseQuery, BrowseResponse, DirEntry};
use crate::error::ApiError;

pub fn home_dir() -> Result<PathBuf, ApiError> {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .ok_or_else(|| ApiError::internal("cannot determine home directory"))
}

/// Expands a leading `~` and requires the result to be absolute.
pub fn resolve_user_path(input: &str) -> Result<PathBuf, ApiError> {
    let input = input.trim();
    let path = if input == "~" {
        home_dir()?
    } else if let Some(rest) = input
        .strip_prefix("~/")
        .or_else(|| input.strip_prefix("~\\"))
    {
        home_dir()?.join(rest)
    } else {
        PathBuf::from(input)
    };
    if !path.is_absolute() {
        return Err(ApiError::bad_request(
            "path must be absolute (or start with ~)",
        ));
    }
    Ok(path)
}

pub fn to_utf8(path: &Path) -> Result<String, ApiError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| ApiError::bad_request("path is not valid UTF-8"))
}

/// Lists subdirectories, so the UI can offer a folder picker (browsers can't
/// reveal real filesystem paths).
pub async fn browse(Query(q): Query<BrowseQuery>) -> Result<Json<BrowseResponse>, ApiError> {
    let home = home_dir()?;
    let requested = match q.path.as_deref().filter(|p| !p.trim().is_empty()) {
        Some(p) => resolve_user_path(p)?,
        None => home.clone(),
    };
    let show_hidden = q.show_hidden.unwrap_or(false);

    tokio::task::spawn_blocking(move || {
        let dir = requested.canonicalize().map_err(|e| {
            ApiError::bad_request(format!("cannot open {}: {e}", requested.display()))
        })?;
        if !dir.is_dir() {
            return Err(ApiError::bad_request(format!(
                "{} is not a directory",
                dir.display()
            )));
        }
        let read = std::fs::read_dir(&dir)
            .map_err(|e| ApiError::bad_request(format!("cannot list {}: {e}", dir.display())))?;

        let mut entries: Vec<DirEntry> = read
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                if !show_hidden && name.starts_with('.') {
                    return None;
                }
                let path = e.path().into_os_string().into_string().ok()?;
                Some(DirEntry { name, path })
            })
            .collect();
        entries.sort_by_key(|e| e.name.to_lowercase());

        Ok(Json(BrowseResponse {
            path: to_utf8(&dir)?,
            parent: dir.parent().and_then(|p| p.to_str()).map(str::to_owned),
            home: to_utf8(&home)?,
            entries,
        }))
    })
    .await
    .map_err(ApiError::internal)?
}
