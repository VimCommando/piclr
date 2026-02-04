use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionConfig {
    Delete,
    Keep,
    Move { target: PathBuf },
    Rename { prefix: String },
    MetadataEdit { key: String, value: String },
}

#[derive(Clone, Debug)]
pub struct ActionMapping {
    pub left: ActionConfig,
    pub right: ActionConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionSide {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionState {
    Undecided,
    Decided {
        side: DecisionSide,
        action: ActionConfig,
    },
}

impl DecisionState {
    pub fn is_undecided(&self) -> bool {
        matches!(self, DecisionState::Undecided)
    }
}
