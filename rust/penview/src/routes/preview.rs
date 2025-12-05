use std::path::PathBuf;

use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use serde::Deserialize;
use tracing::info;

use crate::{render::render_content, state::AppState};

#[derive(Debug, Deserialize)]
pub struct PreviewParams {
    path: PathBuf,
}

/// WebSocket endpoint for Neovim to push buffer content for live preview.
pub async fn preview(
    ws: WebSocketUpgrade,
    Query(params): Query<PreviewParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_preview(socket, params.path, state))
}

async fn handle_preview(mut socket: WebSocket, path: PathBuf, state: AppState) {
    info!("Neovim connected for preview: {}", path.display());

    let tx = state.get_or_create_channel(&path).await;

    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(content) = msg {
            match render_content(&content, &path).await {
                Ok(html) => {
                    let _ = tx.send(html);
                }
                Err(e) => {
                    info!("Render error: {}", e);
                }
            }
        }
    }

    info!("Neovim disconnected: {}", path.display());
}
