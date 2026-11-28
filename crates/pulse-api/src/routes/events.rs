use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use pulse_core::events::Event;
use tokio::sync::broadcast::error::RecvError;

use crate::AppState;

pub async fn events(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    let rx = state.events.subscribe();
    ws.on_upgrade(move |socket| forward(socket, rx))
}

async fn forward(mut socket: WebSocket, mut rx: tokio::sync::broadcast::Receiver<Event>) {
    loop {
        tokio::select! {
            event = rx.recv() => {
                let event = match event {
                    Ok(e) => e,
                    // We dropped events; tell the client to refetch everything.
                    Err(RecvError::Lagged(_)) => Event::FoldersChanged,
                    Err(RecvError::Closed) => break,
                };
                let Ok(text) = serde_json::to_string(&event) else { continue };
                if socket.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
        }
    }
}
