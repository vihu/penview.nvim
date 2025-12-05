use axum::{Router, routing::get};

use crate::state::AppState;

mod index;
mod preview;
mod watch;

use index::index;
use preview::preview;
use watch::watch;

pub fn construct_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/watch", get(watch))
        .route("/api/preview", get(preview))
        .with_state(state)
}
