use std::cmp::Ordering;

use super::state::ImageEntry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Filesystem,
    CreatedAt,
    LastModified,
    Alphabetical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortMode {
    pub key: SortKey,
    pub direction: SortDirection,
}

pub fn sort_indices(entries: &[ImageEntry], mode: SortMode) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..entries.len()).collect();
    indices.sort_by(|a, b| compare_entries(&entries[*a], &entries[*b], mode));
    indices
}

fn compare_entries(a: &ImageEntry, b: &ImageEntry, mode: SortMode) -> Ordering {
    let ordering = match mode.key {
        SortKey::Filesystem => a.original_order.cmp(&b.original_order),
        SortKey::Alphabetical => a
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .cmp(&b.path.file_name().and_then(|s| s.to_str())),
        SortKey::CreatedAt => a.meta.created.cmp(&b.meta.created),
        SortKey::LastModified => a.meta.modified.cmp(&b.meta.modified),
    };

    match mode.direction {
        SortDirection::Asc => ordering,
        SortDirection::Desc => ordering.reverse(),
    }
}
