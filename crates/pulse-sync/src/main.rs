//! `pulsed`: the pulse-sync daemon.

mod secrets;
mod web;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::Parser;
use pulse_api::AppState;
use pulse_fs::FolderManager;
use pulse_store::Store;
use tokio::sync::broadcast;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "pulsed", version, about = "Pulse Sync daemon")]
struct Args {
    /// Where the index database, device key and API token live.
    #[arg(long, env = "PULSE_DATA_DIR")]
    data_dir: Option<PathBuf>,

    /// Address for the local API and web UI. Must be a loopback address.
    #[arg(long, env = "PULSE_LISTEN", default_value = "127.0.0.1:8417")]
    listen: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    if !args.listen.ip().is_loopback() {
        bail!(
            "--listen must be a loopback address (got {}); remote access is not supported yet",
            args.listen
        );
    }

    let data_dir = match args.data_dir {
        Some(d) => d,
        None => directories::ProjectDirs::from("dev", "pulse-sync", "pulse-sync")
            .context("cannot determine a data directory; pass --data-dir")?
            .data_dir()
            .to_path_buf(),
    };
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("cannot create {}", data_dir.display()))?;
    let data_dir = data_dir.canonicalize()?;

    let device_id = secrets::load_or_create_device_id(&data_dir)?;
    let api_token: Arc<str> = secrets::load_or_create_api_token(&data_dir)?.into();
    let store = Store::open(&data_dir.join("index.db"))
        .await
        .context("cannot open index database")?;

    let (events, _) = broadcast::channel(1024);
    let manager = FolderManager::new(store.clone(), device_id.short(), events.clone());
    manager.start_all().await?;

    let state = AppState {
        store,
        manager: manager.clone(),
        events,
        device_id,
        api_token: api_token.clone(),
        data_dir: data_dir.clone(),
    };
    let app = pulse_api::router(state).fallback(web_handler(api_token));

    let listener = tokio::net::TcpListener::bind(args.listen)
        .await
        .with_context(|| format!("cannot listen on {}", args.listen))?;
    tracing::info!(%device_id, data_dir = %data_dir.display(), "pulsed started");
    tracing::info!("web UI: http://{}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    manager.shutdown().await;
    tracing::info!("pulsed stopped");
    Ok(())
}

#[cfg(feature = "embed-web")]
fn web_handler(token: Arc<str>) -> axum::routing::MethodRouter {
    axum::routing::get(move |uri, headers| web::serve(uri, headers, token.clone()))
}

#[cfg(not(feature = "embed-web"))]
fn web_handler(_token: Arc<str>) -> axum::routing::MethodRouter {
    axum::routing::get(web::serve)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
