use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{Mutex, broadcast};

#[derive(Clone)]
pub struct AppState {
    pub channels: Arc<Mutex<HashMap<PathBuf, broadcast::Sender<String>>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_or_create_channel(&self, path: &Path) -> broadcast::Sender<String> {
        let mut channels = self.channels.lock().await;
        channels
            .entry(path.to_path_buf())
            .or_insert_with(|| broadcast::channel(16).0)
            .clone()
    }
}
