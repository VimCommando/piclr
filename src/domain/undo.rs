use std::path::PathBuf;

use super::{ActionConfig, DecisionState};

#[derive(Clone, Debug)]
pub enum UndoAction {
    Move {
        from: PathBuf,
        to: PathBuf,
    },
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    Trash {
        original: PathBuf,
        trashed: PathBuf,
    },
    Metadata {
        path: PathBuf,
        key: String,
        previous: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub struct UndoEntry {
    pub image_id: u64,
    pub previous_decision: DecisionState,
    pub previous_queue: Option<ActionConfig>,
    pub previous_cursor: usize,
    pub undo_action: Option<UndoAction>,
}
