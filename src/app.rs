use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock};

use crate::domain::{
    ActionConfig, ActionMapping, AppStateMachine, SortDirection, SortKey, SortMode,
};

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
                left: ActionConfig::Delete,
                right: ActionConfig::Keep,
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
    pub state: Arc<RwLock<AppStateMachine>>,
    pub config: AppConfig,
    pub cache: Arc<RwLock<HashMap<PathBuf, Bytes>>>,
    pub sse_tx: broadcast::Sender<axum::response::sse::Event>,
}

impl AppContext {
    pub fn new(state: AppStateMachine, config: AppConfig) -> Self {
        let (sse_tx, _) = broadcast::channel(64);
        Self {
            state: Arc::new(RwLock::new(state)),
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            sse_tx,
        }
    }
}
