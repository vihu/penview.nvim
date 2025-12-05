use std::path::PathBuf;

use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use notify::{Config, RecommendedWatcher, Watcher};
use resolve_path::PathResolveExt;
use serde::Deserialize;
use tracing::info;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct WatchParams {
    /// The path to watch for changes.
    path: PathBuf,
}

/// A WebSocket endpoint that watches files for changes and notifies the client when they occur.
/// Also receives live preview updates from Neovim via the broadcast channel.
pub async fn watch(
    ws: WebSocketUpgrade,
    Query(params): Query<WatchParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, params, state))
}

async fn handle_ws(mut socket: WebSocket, WatchParams { path }: WatchParams, state: AppState) {
    let (file_tx, mut file_rx) = tokio::sync::mpsc::unbounded_channel();

    let resolved_path = path.resolve().to_path_buf();

    // Set up file watcher for save-triggered updates
    let mut watcher = RecommendedWatcher::new(
        move |event| {
            let _ = file_tx.send(event);
        },
        Config::default(),
    )
    .unwrap();

    watcher
        .watch(&resolved_path, notify::RecursiveMode::NonRecursive)
        .unwrap();

    // Subscribe to broadcast channel for live preview updates
    let tx = state.get_or_create_channel(&resolved_path).await;
    let mut preview_rx = tx.subscribe();

    info!(
        "Browser connected for watch: {}",
        resolved_path.to_string_lossy()
    );

    loop {
        tokio::select! {
            // Live preview update from Neovim
            Ok(html) = preview_rx.recv() => {
                if socket.send(Message::Text(html.into())).await.is_err() {
                    break;
                }
            }
            // File change on disk (save-triggered)
            Some(_event) = file_rx.recv() => {
                info!("Received file change event for {}", resolved_path.to_string_lossy());
                // Send empty message to trigger full reload
                if socket.send(Message::Text("".into())).await.is_err() {
                    break;
                }
            }
        }
    }

    info!("Browser disconnected: {}", resolved_path.to_string_lossy());
}
