use std::path::PathBuf;

use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{render::render_content, state::AppState};

#[derive(Debug, Deserialize)]
pub struct PreviewParams {
    path: PathBuf,
}

/// Input message from Neovim containing buffer content and cursor position.
#[derive(Debug, Deserialize)]
struct PreviewInput {
    content: String,
    cursor_line: usize,
    #[serde(default = "default_sync_scroll")]
    sync_scroll: bool,
}

fn default_sync_scroll() -> bool {
    true
}

/// Output message to browser containing rendered HTML and cursor line.
#[derive(Debug, Serialize)]
struct PreviewOutput {
    html: String,
    cursor_line: usize,
    sync_scroll: bool,
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
        if let Message::Text(text) = msg {
            // Try to parse as JSON first, fall back to plain text for backwards compatibility
            let (content, cursor_line, sync_scroll) =
                match serde_json::from_str::<PreviewInput>(&text) {
                    Ok(input) => (input.content, input.cursor_line, input.sync_scroll),
                    Err(_) => {
                        // Backwards compatibility: plain markdown text
                        (text.to_string(), 1, false)
                    }
                };

            match render_content(&content, &path).await {
                Ok(html) => {
                    let output = PreviewOutput {
                        html,
                        cursor_line,
                        sync_scroll,
                    };

                    if let Ok(json) = serde_json::to_string(&output) {
                        let _ = tx.send(json);
                    }
                }
                Err(e) => {
                    info!("Render error: {}", e);
                }
            }
        }
    }

    info!("Neovim disconnected: {}", path.display());
}
