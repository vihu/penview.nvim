use std::path::PathBuf;

use axum::{
    extract::{Query, State},
    response::Html,
};
use resolve_path::PathResolveExt;
use serde::Deserialize;
use tracing::info;

use crate::{render::render_doc, state::AppState};

#[derive(Debug, Deserialize)]
pub struct IndexParams {
    path: PathBuf,
}

pub async fn index(
    Query(IndexParams { path }): Query<IndexParams>,
    State(state): State<AppState>,
) -> Html<String> {
    info!("Rendering document {}", path.to_string_lossy());

    Html(
        render_doc(path.resolve(), true, &state.theme)
            .await
            .unwrap(),
    )
}
