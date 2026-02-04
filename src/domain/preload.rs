use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct PreloadState {
    pub next_path: Option<PathBuf>,
}
