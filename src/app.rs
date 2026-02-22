use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{RwLock, broadcast};

use crate::domain::{ActionConfig, ActionMapping, AppState, SortDirection, SortKey, SortMode};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub initial_path: Option<PathBuf>,
    pub queue_mode: bool,
    pub destructive_delete: bool,
    pub action_mapping: ActionMapping,
    pub sort_mode: SortMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            initial_path: None,
            queue_mode: true,
            destructive_delete: false,
            action_mapping: ActionMapping {
                left: ActionConfig::Move {
                    target: PathBuf::from("trash"),
                },
                right: ActionConfig::Move {
                    target: PathBuf::from("keep"),
                },
            },
            sort_mode: SortMode {
                key: SortKey::Filesystem,
                direction: SortDirection::Asc,
            },
        }
    }
}

#[derive(Clone)]
pub struct AppContext {
    pub state: Arc<RwLock<AppState>>,
    pub config: AppConfig,
    pub rendered_stack_ids: Arc<RwLock<Vec<u64>>>,
    pub sse_tx: broadcast::Sender<axum::response::sse::Event>,
}

impl AppContext {
    pub fn new(state: AppState, config: AppConfig) -> Self {
        // Local/Tauri runs can burst command updates (keyboard repeats), so use a
        // wider channel to reduce lag probability before explicit resync handling.
        let (sse_tx, _) = broadcast::channel(256);
        Self {
            state: Arc::new(RwLock::new(state)),
            config,
            rendered_stack_ids: Arc::new(RwLock::new(Vec::new())),
            sse_tx,
        }
    }
}
