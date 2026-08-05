use std::path::PathBuf;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use resolve_path::PathResolveExt;
use serde::Deserialize;
use tracing::{info, warn};

use crate::{render::render_doc, state::AppState};

#[derive(Debug, Deserialize)]
pub struct IndexParams {
    path: PathBuf,
}

pub async fn index(
    Query(IndexParams { path }): Query<IndexParams>,
    State(state): State<AppState>,
) -> Response {
    info!("Rendering document {}", path.to_string_lossy());

    let path = match path.try_resolve() {
        Ok(path) => path,
        Err(error) => {
            warn!("Failed to resolve document path: {error}");
            return (StatusCode::BAD_REQUEST, "Unable to render document").into_response();
        }
    };

    match render_doc(&path, true, &state.theme).await {
        Ok(html) => Html(html).into_response(),
        Err(error) => {
            warn!("Failed to render document {}: {error}", path.display());
            (StatusCode::BAD_REQUEST, "Unable to render document").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_path_returns_bad_request() {
        let response = index(
            Query(IndexParams {
                path: PathBuf::new(),
            }),
            State(AppState::new("light".to_string())),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
