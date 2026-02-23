use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tokio::fs;
#[cfg(feature = "metadata")]
use tokio::task::spawn_blocking;
use tracing::warn;
use walkdir::WalkDir;

use crate::domain::undo::UndoAction;
use crate::domain::{ActionConfig, ImageEntry, ImageMeta};

#[derive(Clone, Debug)]
pub struct FsConfig {
    pub root_dir: PathBuf,
    pub destructive_delete: bool,
}

impl FsConfig {
    pub fn new(root_dir: PathBuf, destructive_delete: bool) -> Self {
        Self {
            root_dir,
            destructive_delete,
        }
    }
}

pub async fn scan_images(root: &Path) -> Vec<ImageEntry> {
    let mut entries = Vec::new();
    let mut id_counter = 1u64;
    let extensions = supported_extensions();

    for (order, entry) in WalkDir::new(root).max_depth(1).into_iter().enumerate() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!(%err, "Failed to read directory entry");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path().to_path_buf();
        if !is_supported(&path, &extensions) {
            continue;
        }

        if let Err(err) = fs::File::open(&path).await {
            warn!(%err, path = %path.display(), "Skipping unreadable image");
            continue;
        }

        let meta = match fs::metadata(&path).await {
            Ok(meta) => meta,
            Err(err) => {
                warn!(%err, path = %path.display(), "Skipping image with unreadable metadata");
                continue;
            }
        };

        let created = meta.created().ok().or_else(|| meta.modified().ok());
        let modified = meta.modified().ok();
        let size = meta.len();
        let orientation = read_orientation(&path).await;

        entries.push(ImageEntry {
            id: id_counter,
            path,
            original_order: order,
            decision: crate::domain::DecisionState::Undecided,
            queued_action: None,
            rename_sequence: None,
            meta: ImageMeta {
                created,
                modified,
                size,
                orientation,
            },
        });
        id_counter += 1;
    }

    entries
}

pub async fn scan_directories(root: &Path, launch_root: &Path) -> Vec<PathBuf> {
    let mut entries = Vec::new();

    for entry in WalkDir::new(root).max_depth(1).into_iter() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!(%err, "Failed to read directory entry");
                continue;
            }
        };
        if !entry.file_type().is_dir() {
            continue;
        }
        if entry.path() == root {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(launch_root) else {
            continue;
        };
        entries.push(rel.to_path_buf());
    }

    entries.sort();
    entries
}

pub async fn load_image_bytes(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    fs::read(path).await
}

pub async fn apply_action(
    config: &FsConfig,
    path: &Path,
    action: &ActionConfig,
    rename_sequence: Option<u64>,
) -> Result<(), std::io::Error> {
    apply_action_with_undo(config, path, action, rename_sequence)
        .await
        .map(|_| ())
}

pub async fn apply_action_with_undo(
    config: &FsConfig,
    path: &Path,
    action: &ActionConfig,
    rename_sequence: Option<u64>,
) -> Result<Option<UndoAction>, std::io::Error> {
    match action {
        ActionConfig::Keep => Ok(None),
        ActionConfig::Delete => {
            if config.destructive_delete {
                fs::remove_file(path).await?;
                Ok(None)
            } else {
                let trash_dir = config.root_dir.join("trash");
                fs::create_dir_all(&trash_dir).await?;
                let file_name = path.file_name().unwrap_or_default();
                let target = trash_dir.join(file_name);
                fs::rename(path, &target).await?;
                Ok(Some(UndoAction::Trash {
                    original: path.to_path_buf(),
                    trashed: target,
                }))
            }
        }
        ActionConfig::Move { target } => {
            let target_path = if target.is_absolute() {
                target.clone()
            } else {
                config.root_dir.join(target)
            };
            fs::create_dir_all(&target_path).await?;
            let file_name = path.file_name().unwrap_or_default();
            let destination = target_path.join(file_name);
            fs::rename(path, &destination).await?;
            Ok(Some(UndoAction::Move {
                from: destination,
                to: path.to_path_buf(),
            }))
        }
        ActionConfig::Rename { prefix } => {
            let sequence = rename_sequence.unwrap_or(0);
            let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            let base = format!("{}{:06}", prefix, sequence);
            let new_name = if extension.is_empty() {
                base
            } else {
                format!("{}.{}", base, extension)
            };
            let destination = path.with_file_name(new_name);
            fs::rename(path, &destination).await?;
            Ok(Some(UndoAction::Rename {
                from: destination,
                to: path.to_path_buf(),
            }))
        }
        ActionConfig::MetadataEdit { key, value } => {
            let previous = read_metadata_tag(path, key).await;
            apply_metadata_edit(path, key, value).await?;
            Ok(Some(UndoAction::Metadata {
                path: path.to_path_buf(),
                key: key.clone(),
                previous,
            }))
        }
    }
}

pub async fn apply_undo_action(action: &UndoAction) -> Result<(), std::io::Error> {
    match action {
        UndoAction::Move { from, to } => {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).await.ok();
            }
            fs::rename(from, to).await
        }
        UndoAction::Rename { from, to } => fs::rename(from, to).await,
        UndoAction::Trash { original, trashed } => {
            if let Some(parent) = original.parent() {
                fs::create_dir_all(parent).await.ok();
            }
            fs::rename(trashed, original).await
        }
        UndoAction::Metadata {
            path,
            key,
            previous,
        } => {
            if let Some(value) = previous {
                apply_metadata_edit(path, key, value).await
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(feature = "metadata")]
async fn apply_metadata_edit(path: &Path, key: &str, value: &str) -> Result<(), std::io::Error> {
    let path = path.to_path_buf();
    let key = key.to_string();
    let value = value.to_string();
    spawn_blocking(move || {
        let mut metadata = rexiv2::Metadata::new_from_path(&path)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;
        metadata
            .set_tag_string(&key, &value)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;
        metadata
            .save_to_file(&path)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;
        Ok(())
    })
    .await
    .unwrap_or_else(|err| {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            err.to_string(),
        ))
    })
}

#[cfg(not(feature = "metadata"))]
async fn apply_metadata_edit(_path: &Path, _key: &str, _value: &str) -> Result<(), std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "metadata feature disabled",
    ))
}

#[cfg(feature = "metadata")]
async fn read_metadata_tag(path: &Path, key: &str) -> Option<String> {
    let path = path.to_path_buf();
    let key = key.to_string();
    spawn_blocking(move || {
        let metadata = rexiv2::Metadata::new_from_path(&path).ok()?;
        metadata.get_tag_string(&key).ok()
    })
    .await
    .unwrap_or(None)
}

#[cfg(not(feature = "metadata"))]
async fn read_metadata_tag(_path: &Path, _key: &str) -> Option<String> {
    None
}

#[cfg(feature = "metadata")]
async fn read_orientation(path: &Path) -> Option<u16> {
    let path = path.to_path_buf();
    spawn_blocking(move || {
        let metadata = rexiv2::Metadata::new_from_path(&path).ok()?;
        metadata
            .get_tag_string("Exif.Image.Orientation")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
    })
    .await
    .unwrap_or(None)
}

#[cfg(not(feature = "metadata"))]
async fn read_orientation(_path: &Path) -> Option<u16> {
    None
}

fn supported_extensions() -> HashSet<&'static str> {
    ["jpg", "jpeg", "png", "gif", "webp", "heic", "svg"]
        .into_iter()
        .collect()
}

pub fn is_supported_image_path(path: &Path) -> bool {
    is_supported(path, &supported_extensions())
}

fn is_supported(path: &Path, extensions: &HashSet<&'static str>) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let lower = ext.to_lowercase();
            extensions.contains(lower.as_str())
        })
        .unwrap_or(false)
}
