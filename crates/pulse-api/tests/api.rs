use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use pulse_api::AppState;
use pulse_api::dto::{BrowseResponse, FileListResponse, FolderView, HealthResponse};
use pulse_core::DeviceId;
use pulse_fs::FolderManager;
use pulse_store::Store;
use serde_json::{Value, json};
use tokio::sync::broadcast;
use tower::ServiceExt;

const TOKEN: &str = "test-token";

struct Harness {
    app: Router,
    data_path: std::path::PathBuf,
    _data: tempfile::TempDir,
}

async fn harness() -> Harness {
    let data = tempfile::tempdir().unwrap();
    let store = Store::open_in_memory().await.unwrap();
    let (events, _) = broadcast::channel(64);
    let device_id = DeviceId::from_bytes([1; 32]);
    let manager = FolderManager::new(store.clone(), device_id.short(), events.clone());
    let data_path = data.path().to_path_buf();
    let state = AppState {
        store,
        manager,
        events,
        device_id,
        api_token: Arc::from(TOKEN),
        data_dir: data_path.clone(),
    };
    Harness {
        app: pulse_api::router(state),
        data_path,
        _data: data,
    }
}

fn req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1:8417")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    match body {
        Some(v) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

async fn call(app: &Router, r: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(r).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

#[tokio::test]
async fn rejects_missing_token_and_foreign_host() {
    let h = harness().await;

    let no_token = Request::get("/api/health")
        .header(header::HOST, "localhost:8417")
        .body(Body::empty())
        .unwrap();
    assert_eq!(call(&h.app, no_token).await.0, StatusCode::UNAUTHORIZED);

    let mut rebinding = req("GET", "/api/health", None);
    rebinding
        .headers_mut()
        .insert(header::HOST, "attacker.example:8417".parse().unwrap());
    assert_eq!(
        call(&h.app, rebinding).await.0,
        StatusCode::MISDIRECTED_REQUEST
    );

    let cookie = Request::get("/api/health")
        .header(header::HOST, "localhost:8417")
        .header(header::COOKIE, format!("other=1; pulse_token={TOKEN}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(call(&h.app, cookie).await.0, StatusCode::OK);

    let (status, body) = call(&h.app, req("GET", "/api/health", None)).await;
    assert_eq!(status, StatusCode::OK);
    let health: HealthResponse = serde_json::from_value(body).unwrap();
    assert_eq!(health.device_id, DeviceId::from_bytes([1; 32]).to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn folder_lifecycle_and_indexing() {
    let h = harness().await;
    let share = tempfile::tempdir().unwrap();
    std::fs::write(share.path().join("hello.txt"), "hello").unwrap();
    std::fs::create_dir(share.path().join("sub")).unwrap();
    std::fs::write(share.path().join("sub/inner.txt"), "inner").unwrap();

    let (status, body) = call(
        &h.app,
        req(
            "POST",
            "/api/folders",
            Some(json!({ "path": share.path() })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let created: FolderView = serde_json::from_value(body).unwrap();
    assert!(share.path().join(".pulse").is_dir(), "marker created");
    let id = created.folder.id.to_string();

    // Overlapping folders, the data dir and relative paths are rejected.
    let nested = json!({ "path": share.path().join("sub") });
    assert_eq!(
        call(&h.app, req("POST", "/api/folders", Some(nested)))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let data = json!({ "path": h.data_path });
    assert_eq!(
        call(&h.app, req("POST", "/api/folders", Some(data)))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let relative = json!({ "path": "relative/dir" });
    assert_eq!(
        call(&h.app, req("POST", "/api/folders", Some(relative)))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );

    // The background scan indexes both files.
    let mut files = Vec::new();
    for _ in 0..100 {
        let (_, body) = call(
            &h.app,
            req("GET", &format!("/api/folders/{id}/files"), None),
        )
        .await;
        files = serde_json::from_value::<FileListResponse>(body)
            .unwrap()
            .files;
        if files.len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let paths: Vec<_> = files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["hello.txt", "sub/inner.txt"]);

    let (_, body) = call(
        &h.app,
        req("GET", &format!("/api/folders/{id}/files?prefix=sub"), None),
    )
    .await;
    assert_eq!(
        serde_json::from_value::<FileListResponse>(body)
            .unwrap()
            .files
            .len(),
        1
    );

    // Pause and rename, then remove.
    let patch = json!({ "paused": true, "label": "Renamed" });
    let (status, body) = call(
        &h.app,
        req("PATCH", &format!("/api/folders/{id}"), Some(patch)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let updated: FolderView = serde_json::from_value(body).unwrap();
    assert!(updated.folder.paused);
    assert_eq!(updated.folder.label, "Renamed");
    assert_eq!(updated.stats.files, 2);
    let rescan = req("POST", &format!("/api/folders/{id}/rescan"), None);
    assert_eq!(call(&h.app, rescan).await.0, StatusCode::CONFLICT);

    let delete = req("DELETE", &format!("/api/folders/{id}"), None);
    assert_eq!(call(&h.app, delete).await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        call(&h.app, req("GET", &format!("/api/folders/{id}"), None))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert!(
        share.path().join("hello.txt").exists(),
        "files on disk are untouched"
    );
}

#[tokio::test]
async fn browse_lists_directories_only() {
    let h = harness().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Beta")).unwrap();
    std::fs::create_dir(dir.path().join("alpha")).unwrap();
    std::fs::create_dir(dir.path().join(".hidden")).unwrap();
    std::fs::write(dir.path().join("file.txt"), "x").unwrap();

    let uri = format!("/api/fs/browse?path={}", dir.path().display());
    let (status, body) = call(&h.app, req("GET", &uri, None)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listing: BrowseResponse = serde_json::from_value(body).unwrap();
    let names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["alpha", "Beta"]);
    assert!(listing.parent.is_some());

    let (_, body) = call(&h.app, req("GET", &format!("{uri}&showHidden=true"), None)).await;
    assert_eq!(
        serde_json::from_value::<BrowseResponse>(body)
            .unwrap()
            .entries
            .len(),
        3
    );

    let (status, _) = call(
        &h.app,
        req("GET", "/api/fs/browse?path=/definitely/not/here", None),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
