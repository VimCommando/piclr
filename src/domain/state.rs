use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::actions::{ActionConfig, ActionMapping, DecisionSide, DecisionState};
use super::preload::PreloadState;
use super::sorting::{SortMode, sort_indices};
use super::undo::UndoEntry;

mod modal;

pub use modal::ModalView;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMode {
    Init,
    Scanning,
    Ready,
    Viewing,
    Applying,
    Done,
}

#[derive(Clone, Debug)]
pub struct ImageMeta {
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub size: u64,
    pub orientation: Option<u16>,
}

#[derive(Clone, Debug)]
pub struct ImageEntry {
    pub id: u64,
    pub path: PathBuf,
    pub original_order: usize,
    pub decision: DecisionState,
    pub queued_action: Option<ActionConfig>,
    pub rename_sequence: Option<u64>,
    pub meta: ImageMeta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavEntryKind {
    Directory,
    File,
}

#[derive(Clone, Debug)]
pub struct NavEntry {
    pub id: u64,
    pub rel_path: PathBuf,
    pub label: String,
    pub kind: NavEntryKind,
    pub image_id: Option<u64>,
    pub is_parent_link: bool,
}

#[derive(Clone, Debug)]
pub struct AppStateInner {
    pub images: Vec<ImageEntry>,
    pub nav_entries: Vec<NavEntry>,
    pub current_dir: Option<PathBuf>,
    pub order: Vec<usize>,
    pub cursor: usize,
    pub queued_ids: VecDeque<u64>,
    pub scan_version: u64,
    pub queue_mode: bool,
    pub action_mapping: ActionMapping,
    pub sort_mode: SortMode,
    pub undo_stack: Vec<UndoEntry>,
    pub preload: PreloadState,
    pub rename_counter: u64,
    pub root_dir: Option<PathBuf>,
    pub pending_directory_path: Option<PathBuf>,
    pub pending_delete_directory_path: Option<PathBuf>,
    pub selected_entry_id: Option<u64>,
    pub sidebar_expanded: bool,
    pub queue_sidebar_visible: bool,
    pub queue_focus: bool,
    pub selected_queue_image_id: Option<u64>,
    pub directory_actions_entry_id: Option<u64>,
    pub active_modal: Option<ModalView>,
    pub projection: ReadModelProjection,
    pub last_apply_result: Option<ApplyResultSummary>,
    pub nav_direction: Option<NavDirection>,
    pub nav_tick: u64,
}

#[derive(Clone, Debug)]
pub struct ApplyResultSummary {
    pub completed: usize,
    pub total: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ReadModelProjection {
    pub left_action_count: usize,
    pub right_action_count: usize,
    pub queue_count: usize,
    pub stack_start: usize,
    pub stack_end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavDirection {
    Up,
    Down,
}

impl AppStateInner {
    pub fn new(queue_mode: bool, action_mapping: ActionMapping, sort_mode: SortMode) -> Self {
        Self {
            images: Vec::new(),
            nav_entries: Vec::new(),
            current_dir: None,
            order: Vec::new(),
            cursor: 0,
            queued_ids: VecDeque::new(),
            scan_version: 0,
            queue_mode,
            action_mapping,
            sort_mode,
            undo_stack: Vec::new(),
            preload: PreloadState::default(),
            rename_counter: 1,
            root_dir: None,
            pending_directory_path: None,
            pending_delete_directory_path: None,
            selected_entry_id: None,
            sidebar_expanded: false,
            queue_sidebar_visible: false,
            queue_focus: false,
            selected_queue_image_id: None,
            directory_actions_entry_id: None,
            active_modal: None,
            projection: ReadModelProjection::default(),
            last_apply_result: None,
            nav_direction: None,
            nav_tick: 0,
        }
    }

    pub fn set_directory_snapshot(
        &mut self,
        images: Vec<ImageEntry>,
        directories: Vec<PathBuf>,
        root_dir: Option<PathBuf>,
        current_dir: Option<PathBuf>,
    ) {
        if root_dir.is_some() {
            self.root_dir = root_dir.clone();
        }
        self.current_dir = current_dir;
        self.images = images;
        self.scan_version = self.scan_version.saturating_add(1);
        self.order = sort_indices(&self.images, self.sort_mode);
        self.cursor = 0;
        self.undo_stack.clear();
        self.rename_counter = 1;
        self.rebuild_queue_from_order();
        self.rebuild_nav_entries(directories);
        self.rebuild_projection();
        self.update_preload();
        self.sync_selected_to_current_image();
        self.directory_actions_entry_id = None;
    }

    pub fn set_images(&mut self, images: Vec<ImageEntry>, root_dir: Option<PathBuf>) {
        let current_dir = root_dir.clone();
        self.set_directory_snapshot(images, Vec::new(), root_dir, current_dir);
    }

    pub fn current_index(&self) -> Option<usize> {
        self.order.get(self.cursor).copied()
    }

    pub fn current(&self) -> Option<&ImageEntry> {
        self.current_index().and_then(|idx| self.images.get(idx))
    }

    pub fn current_mut(&mut self) -> Option<&mut ImageEntry> {
        let idx = self.current_index()?;
        self.images.get_mut(idx)
    }

    pub fn set_sort_mode(&mut self, mode: SortMode) {
        let current_id = self.current().map(|entry| entry.id);
        self.sort_mode = mode;
        self.order = sort_indices(&self.images, self.sort_mode);
        if let Some(id) = current_id {
            if let Some(position) = self.order.iter().position(|idx| self.images[*idx].id == id) {
                self.cursor = position;
            }
        }
        self.rebuild_queue_from_order();
        self.update_stack_projection(5);
        self.update_preload();
        self.sync_selected_to_current_image();
    }

    pub fn next(&mut self) {
        let position = self.cursor + 1;
        if position < self.order.len() {
            self.move_cursor(position, NavDirection::Down);
        }
    }

    pub fn prev(&mut self) {
        if let Some(position) = self.cursor.checked_sub(1) {
            self.move_cursor(position, NavDirection::Up);
        }
    }

    pub fn jump_next_undecided(&mut self) {
        self.jump_undecided(NavDirection::Down);
    }

    pub fn jump_prev_undecided(&mut self) {
        self.jump_undecided(NavDirection::Up);
    }

    fn jump_undecided(&mut self, direction: NavDirection) {
        if self.order.is_empty() {
            return;
        }
        let is_undecided =
            |&position: &usize| self.images[self.order[position]].decision.is_undecided();
        let position = match direction {
            NavDirection::Up => (0..self.cursor).rev().find(is_undecided),
            NavDirection::Down => (self.cursor + 1..self.order.len()).find(is_undecided),
        };
        if let Some(position) = position {
            self.move_cursor(position, direction);
        }
    }

    fn move_cursor(&mut self, position: usize, direction: NavDirection) {
        self.cursor = position;
        self.record_nav(direction);
        self.update_stack_projection(5);
        self.update_preload();
        self.sync_selected_to_current_image();
        self.directory_actions_entry_id = None;
    }

    pub fn select_image_by_id(&mut self, image_id: u64) -> bool {
        if let Some(position) = self
            .order
            .iter()
            .position(|idx| self.images[*idx].id == image_id)
        {
            let direction = if position > self.cursor {
                Some(NavDirection::Down)
            } else if position < self.cursor {
                Some(NavDirection::Up)
            } else {
                None
            };
            self.cursor = position;
            if let Some(direction) = direction {
                self.record_nav(direction);
            }
            self.update_stack_projection(5);
            self.update_preload();
            self.selected_entry_id = Some(image_id);
            self.queue_focus = false;
            self.directory_actions_entry_id = None;
            return true;
        }
        false
    }

    pub fn select_entry_by_id(&mut self, entry_id: u64) -> bool {
        if let Some(entry) = self.nav_entries.iter().find(|entry| entry.id == entry_id) {
            self.selected_entry_id = Some(entry_id);
            self.queue_focus = false;
            self.directory_actions_entry_id = None;
            if let Some(image_id) = entry.image_id {
                return self.select_image_by_id(image_id);
            }
            return true;
        }
        false
    }

    pub fn select_first_entry(&mut self) -> bool {
        let Some(entry) = self.nav_entries.first() else {
            return false;
        };
        self.select_entry_by_id(entry.id)
    }

    pub fn select_last_entry(&mut self) -> bool {
        let Some(entry) = self.nav_entries.last() else {
            return false;
        };
        self.select_entry_by_id(entry.id)
    }

    pub fn select_first_image(&mut self) -> bool {
        let Some(idx) = self.order.first().copied() else {
            return false;
        };
        let Some(image) = self.images.get(idx) else {
            return false;
        };
        self.select_image_by_id(image.id)
    }

    pub fn select_last_image(&mut self) -> bool {
        let Some(idx) = self.order.last().copied() else {
            return false;
        };
        let Some(image) = self.images.get(idx) else {
            return false;
        };
        self.select_image_by_id(image.id)
    }

    pub fn select_next_entry(&mut self) -> bool {
        self.select_adjacent_entry(NavDirection::Down)
    }

    pub fn select_prev_entry(&mut self) -> bool {
        self.select_adjacent_entry(NavDirection::Up)
    }

    fn select_adjacent_entry(&mut self, direction: NavDirection) -> bool {
        let current = self
            .selected_entry_id
            .and_then(|id| self.nav_entries.iter().position(|entry| entry.id == id));
        let Some(position) = adjacent_selection(current, self.nav_entries.len(), direction) else {
            return false;
        };
        self.select_entry_by_id(self.nav_entries[position].id)
    }

    pub fn selected_entry(&self) -> Option<&NavEntry> {
        let id = self.selected_entry_id?;
        self.nav_entries.iter().find(|entry| entry.id == id)
    }

    pub fn selected_entry_is_directory(&self) -> bool {
        self.selected_entry()
            .map(|entry| entry.kind == NavEntryKind::Directory)
            .unwrap_or(false)
    }

    pub fn selected_directory_path(&self) -> Option<PathBuf> {
        let root = self.root_dir.as_ref()?;
        let entry = self.selected_entry()?;
        if entry.kind != NavEntryKind::Directory {
            return None;
        }
        Some(root.join(&entry.rel_path))
    }

    pub fn navigate_to_selected_directory(&self) -> Option<PathBuf> {
        self.selected_directory_path()
    }

    pub fn navigate_to_parent_directory(&self) -> Option<PathBuf> {
        let (parent, _) = self.parent_directory_paths()?;
        Some(parent.to_path_buf())
    }

    fn parent_directory_paths(&self) -> Option<(&Path, &Path)> {
        let root = self.root_dir.as_ref()?;
        let current = self.current_dir.as_ref()?;
        if current == root {
            return None;
        }
        let parent = current.parent()?;
        let relative = parent.strip_prefix(root).ok()?;
        Some((parent, relative))
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_expanded = !self.sidebar_expanded;
        if !self.sidebar_expanded && self.queue_sidebar_visible {
            self.activate_queue_focus();
        }
    }

    pub fn toggle_queue_sidebar(&mut self) {
        self.queue_sidebar_visible = !self.queue_sidebar_visible;
        if self.queue_sidebar_visible {
            self.queue_focus = true;
            self.ensure_queue_selection();
        } else {
            self.queue_focus = false;
        }
    }

    pub fn activate_queue_focus(&mut self) {
        if !self.queue_sidebar_visible {
            return;
        }
        self.queue_focus = true;
        self.ensure_queue_selection();
    }

    pub fn queue_is_selected(&self) -> bool {
        self.queue_sidebar_visible && self.queue_focus && self.selected_queue_image_id.is_some()
    }

    pub fn select_queue_next(&mut self) -> bool {
        self.select_adjacent_queue_item(NavDirection::Down)
    }

    pub fn select_queue_prev(&mut self) -> bool {
        self.select_adjacent_queue_item(NavDirection::Up)
    }

    fn select_adjacent_queue_item(&mut self, direction: NavDirection) -> bool {
        if self.queued_ids.is_empty() {
            return false;
        }
        self.activate_queue_focus();
        let current = self.selected_queue_image_id.and_then(|id| {
            self.queued_ids
                .iter()
                .position(|queued_id| *queued_id == id)
        });
        let Some(position) = adjacent_selection(current, self.queued_ids.len(), direction) else {
            return false;
        };
        self.selected_queue_image_id = Some(self.queued_ids[position]);
        true
    }

    pub fn select_queue_first(&mut self) -> bool {
        if self.queued_ids.is_empty() {
            return false;
        }
        self.activate_queue_focus();
        self.selected_queue_image_id = self.queued_ids.front().copied();
        true
    }

    pub fn select_queue_last(&mut self) -> bool {
        if self.queued_ids.is_empty() {
            return false;
        }
        self.activate_queue_focus();
        self.selected_queue_image_id = self.queued_ids.back().copied();
        true
    }

    pub fn select_queue_item_by_id(&mut self, image_id: u64) -> bool {
        if !self.queued_ids.contains(&image_id) {
            return false;
        }
        self.activate_queue_focus();
        self.selected_queue_image_id = Some(image_id);
        true
    }

    pub fn queue_selected_item_for_apply(
        &self,
    ) -> Option<(u64, PathBuf, ActionConfig, Option<u64>)> {
        let selected = self.selected_queue_image_id?;
        let image = self.images.iter().find(|image| image.id == selected)?;
        let action = image.queued_action.clone()?;
        Some((selected, image.path.clone(), action, image.rename_sequence))
    }

    pub fn remove_selected_queue_item(&mut self) -> Option<u64> {
        let selected = self.selected_queue_image_id?;
        let selected_pos = self
            .queued_ids
            .iter()
            .position(|queued_id| *queued_id == selected)?;
        let image = self.images.iter_mut().find(|image| image.id == selected)?;
        image.queued_action = None;
        image.decision = DecisionState::Undecided;
        image.rename_sequence = None;
        self.rebuild_queue_from_order();
        self.rebuild_projection();
        if self.queued_ids.is_empty() {
            self.selected_queue_image_id = None;
            self.queue_focus = false;
        } else {
            let next_pos = selected_pos.min(self.queued_ids.len().saturating_sub(1));
            self.selected_queue_image_id = self.queued_ids.get(next_pos).copied();
        }
        Some(selected)
    }

    pub fn override_selected_queue_action(&mut self, side: DecisionSide) -> bool {
        let selected = match self.selected_queue_image_id {
            Some(id) => id,
            None => return false,
        };
        if !self.queued_ids.contains(&selected) {
            return false;
        }
        let action = match side {
            DecisionSide::Left => self.action_mapping.left.clone(),
            DecisionSide::Right => self.action_mapping.right.clone(),
        };
        let next_rename = self.rename_counter;
        let mut consumed_rename = false;
        let Some(image) = self.images.iter_mut().find(|image| image.id == selected) else {
            return false;
        };
        if let ActionConfig::Rename { .. } = action {
            if image.rename_sequence.is_none() {
                image.rename_sequence = Some(next_rename);
                consumed_rename = true;
            }
        } else {
            image.rename_sequence = None;
        }
        image.decision = DecisionState::Decided {
            side,
            action: action.clone(),
        };
        image.queued_action = Some(action);
        if consumed_rename {
            self.rename_counter += 1;
        }
        self.rebuild_queue_from_order();
        self.rebuild_projection();
        true
    }

    pub fn apply_selected_queue_action_result(
        &mut self,
        image_id: u64,
        action: &ActionConfig,
    ) -> bool {
        let image_idx = match self.images.iter().position(|image| image.id == image_id) {
            Some(idx) => idx,
            None => return false,
        };

        let remove_image = matches!(
            action,
            ActionConfig::Delete | ActionConfig::Move { .. } | ActionConfig::Rename { .. }
        );

        if remove_image {
            self.remove_image(image_idx);
        } else {
            let image = &mut self.images[image_idx];
            image.decision = DecisionState::Undecided;
            image.queued_action = None;
            image.rename_sequence = None;
        }

        self.rebuild_queue_from_order();
        self.rebuild_projection();
        self.update_preload();
        self.sync_selected_to_current_image();
        self.directory_actions_entry_id = None;
        true
    }

    fn remove_image(&mut self, image_idx: usize) {
        let image = self.images.remove(image_idx);
        self.order.retain_mut(|idx| {
            if *idx == image_idx {
                return false;
            }
            if *idx > image_idx {
                *idx -= 1;
            }
            true
        });
        self.nav_entries
            .retain(|entry| entry.image_id != Some(image.id));
        self.cursor = self.cursor.min(self.order.len().saturating_sub(1));
    }

    pub fn close_directory_actions(&mut self) {
        self.directory_actions_entry_id = None;
    }

    pub fn open_directory_actions_for_selected(&mut self) -> bool {
        if !self.selected_entry_is_directory() {
            return false;
        }
        if self
            .selected_entry()
            .map(|entry| entry.is_parent_link)
            .unwrap_or(false)
        {
            return false;
        }
        self.directory_actions_entry_id = self.selected_entry_id;
        true
    }

    fn record_nav(&mut self, direction: NavDirection) {
        self.nav_direction = Some(direction);
        self.nav_tick = self.nav_tick.saturating_add(1);
    }

    pub fn apply_decision(&mut self, side: DecisionSide) -> Option<DecisionOutcome> {
        let action = match side {
            DecisionSide::Left => self.action_mapping.left.clone(),
            DecisionSide::Right => self.action_mapping.right.clone(),
        };
        let queue_mode = self.queue_mode;
        let cursor_before = self.cursor;
        let next_rename = self.rename_counter;
        let mut consumed_rename = false;

        let (image_id, previous_decision, previous_queue) = {
            let entry = self.current_mut()?;
            let previous_decision = entry.decision.clone();
            let previous_queue = entry.queued_action.clone();

            if let ActionConfig::Rename { .. } = action {
                if entry.rename_sequence.is_none() {
                    entry.rename_sequence = Some(next_rename);
                    consumed_rename = true;
                }
            }

            entry.decision = DecisionState::Decided {
                side,
                action: action.clone(),
            };

            if queue_mode {
                entry.queued_action = Some(action.clone());
            } else {
                entry.queued_action = None;
            }

            (entry.id, previous_decision, previous_queue)
        };

        if consumed_rename {
            self.rename_counter += 1;
        }

        self.rebuild_queue_from_order();
        self.rebuild_projection_counters();
        self.next();
        let cursor_after = self.cursor;

        let outcome = DecisionOutcome {
            image_id,
            action,
            previous_decision,
            previous_queue,
            cursor_before,
            cursor_after,
            immediate: !queue_mode,
        };
        Some(outcome)
    }

    pub fn reset_queue_state(&mut self) {
        for image in &mut self.images {
            image.decision = DecisionState::Undecided;
            image.queued_action = None;
            image.rename_sequence = None;
        }
        self.queued_ids.clear();
        self.undo_stack.clear();
        self.rename_counter = 1;
        self.rebuild_projection();
        self.update_preload();
        self.selected_queue_image_id = None;
        self.queue_focus = false;
        self.directory_actions_entry_id = None;
    }

    pub fn record_undo(&mut self, entry: UndoEntry) {
        self.undo_stack.push(entry);
    }

    pub fn undo_last(&mut self) -> Option<UndoEntry> {
        if let Some(undo) = self.undo_stack.pop() {
            if let Some(image) = self
                .images
                .iter_mut()
                .find(|image| image.id == undo.image_id)
            {
                image.decision = undo.previous_decision.clone();
                image.queued_action = undo.previous_queue.clone();
            }
            if !self.select_image_by_id(undo.image_id) {
                let max_cursor = self.order.len().saturating_sub(1);
                self.cursor = undo.previous_cursor.min(max_cursor);
                self.sync_selected_to_current_image();
            }
            self.rebuild_queue_from_order();
            self.rebuild_projection();
            self.update_preload();
            return Some(undo);
        }
        None
    }

    pub fn queued_actions_for_apply(&self) -> Vec<(PathBuf, ActionConfig, Option<u64>)> {
        self.queued_ids
            .iter()
            .filter_map(|queued_id| {
                let image = self.images.iter().find(|image| image.id == *queued_id)?;
                let action = image.queued_action.clone()?;
                Some((image.path.clone(), action, image.rename_sequence))
            })
            .collect()
    }

    pub fn clear_queued_actions(&mut self) {
        for image in &mut self.images {
            image.queued_action = None;
        }
        self.queued_ids.clear();
        self.selected_queue_image_id = None;
        self.queue_focus = false;
        self.rebuild_projection_counters();
    }

    pub fn projection(&self) -> &ReadModelProjection {
        &self.projection
    }

    fn update_preload(&mut self) {
        let next_index = self.order.get(self.cursor + 1).copied();
        self.preload.next_path =
            next_index.and_then(|idx| self.images.get(idx).map(|e| e.path.clone()));
    }

    fn rebuild_queue_from_order(&mut self) {
        self.queued_ids.clear();
        for idx in &self.order {
            if let Some(image) = self.images.get(*idx) {
                if image.queued_action.is_some() {
                    self.queued_ids.push_back(image.id);
                }
            }
        }
        self.ensure_queue_selection();
    }

    fn ensure_queue_selection(&mut self) {
        if self.queued_ids.is_empty() {
            self.selected_queue_image_id = None;
            self.queue_focus = false;
            return;
        }
        if self
            .selected_queue_image_id
            .is_some_and(|selected| self.queued_ids.contains(&selected))
        {
            return;
        }
        let from_current = self
            .current()
            .map(|image| image.id)
            .filter(|current_id| self.queued_ids.contains(current_id));
        self.selected_queue_image_id = from_current.or_else(|| self.queued_ids.front().copied());
    }

    fn rebuild_nav_entries(&mut self, mut directories: Vec<PathBuf>) {
        directories.sort();
        let mut entries = Vec::with_capacity(directories.len() + self.images.len());
        if let Some(entry) = self.parent_nav_entry() {
            entries.push(entry);
        }
        for rel_path in directories {
            entries.push(NavEntry {
                id: directory_entry_id(&rel_path),
                label: nav_entry_label(&rel_path),
                rel_path,
                kind: NavEntryKind::Directory,
                image_id: None,
                is_parent_link: false,
            });
        }
        entries.extend(
            self.order
                .iter()
                .filter_map(|&idx| self.images.get(idx))
                .map(|image| self.image_nav_entry(image)),
        );
        self.nav_entries = entries;
    }

    fn parent_nav_entry(&self) -> Option<NavEntry> {
        let (_, rel_path) = self.parent_directory_paths()?;
        Some(NavEntry {
            id: directory_entry_id(rel_path),
            rel_path: rel_path.to_path_buf(),
            label: "..".to_string(),
            kind: NavEntryKind::Directory,
            image_id: None,
            is_parent_link: true,
        })
    }

    fn image_nav_entry(&self, image: &ImageEntry) -> NavEntry {
        let path = image.path.as_path();
        let rel_path = self
            .root_dir
            .as_ref()
            .and_then(|root| path.strip_prefix(root).ok())
            .unwrap_or(path);
        NavEntry {
            id: image.id,
            rel_path: rel_path.to_path_buf(),
            label: nav_entry_label(rel_path),
            kind: NavEntryKind::File,
            image_id: Some(image.id),
            is_parent_link: false,
        }
    }

    fn sync_selected_to_current_image(&mut self) {
        self.selected_entry_id = self.current().map(|entry| entry.id);
    }

    fn update_stack_projection(&mut self, radius: usize) {
        if self.order.is_empty() {
            self.projection.stack_start = 0;
            self.projection.stack_end = 0;
            return;
        }
        self.projection.stack_start = self.cursor.saturating_sub(radius);
        self.projection.stack_end = (self.cursor + radius).min(self.order.len().saturating_sub(1));
    }

    fn rebuild_projection_counters(&mut self) {
        self.projection.left_action_count = 0;
        self.projection.right_action_count = 0;
        for image in &self.images {
            match &image.decision {
                DecisionState::Decided {
                    side: DecisionSide::Left,
                    ..
                } => self.projection.left_action_count += 1,
                DecisionState::Decided {
                    side: DecisionSide::Right,
                    ..
                } => self.projection.right_action_count += 1,
                DecisionState::Undecided => {}
            }
        }
        self.projection.queue_count = self.queued_ids.len();
    }

    fn rebuild_projection(&mut self) {
        self.rebuild_projection_counters();
        self.update_stack_projection(5);
    }
}

fn adjacent_selection(
    current: Option<usize>,
    len: usize,
    direction: NavDirection,
) -> Option<usize> {
    let position = match (current, direction) {
        (None, _) => 0,
        (Some(position), NavDirection::Up) => position.checked_sub(1)?,
        (Some(position), NavDirection::Down) => position + 1,
    };
    (position < len).then_some(position)
}

fn nav_entry_label(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn directory_entry_id(rel_path: &Path) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    rel_path.hash(&mut hasher);
    (hasher.finish() & !(1_u64 << 63)) | (1_u64 << 63)
}

#[derive(Clone, Debug)]
pub struct DecisionOutcome {
    pub image_id: u64,
    pub action: ActionConfig,
    pub previous_decision: DecisionState,
    pub previous_queue: Option<ActionConfig>,
    pub cursor_before: usize,
    pub cursor_after: usize,
    pub immediate: bool,
}

#[derive(Clone, Debug)]
pub struct Init;
#[derive(Clone, Debug)]
pub struct Scanning;
#[derive(Clone, Debug)]
pub struct Ready;
#[derive(Clone, Debug)]
pub struct Viewing;
#[derive(Clone, Debug)]
pub struct Applying;
#[derive(Clone, Debug)]
pub struct Done;

#[derive(Clone, Debug)]
pub struct App<S> {
    pub state: AppStateInner,
    marker: PhantomData<S>,
}

impl App<Init> {
    pub fn new(queue_mode: bool, action_mapping: ActionMapping, sort_mode: SortMode) -> Self {
        Self {
            state: AppStateInner::new(queue_mode, action_mapping, sort_mode),
            marker: PhantomData,
        }
    }

    pub fn start_scan(self) -> App<Scanning> {
        App {
            state: self.state,
            marker: PhantomData,
        }
    }
}

impl App<Scanning> {
    pub fn with_images(mut self, images: Vec<ImageEntry>, root_dir: Option<PathBuf>) -> App<Ready> {
        self.state.set_images(images, root_dir);
        App {
            state: self.state,
            marker: PhantomData,
        }
    }
}

impl App<Ready> {
    pub fn start_viewing(self) -> App<Viewing> {
        App {
            state: self.state,
            marker: PhantomData,
        }
    }
}

impl App<Viewing> {
    pub fn start_applying(self) -> App<Applying> {
        App {
            state: self.state,
            marker: PhantomData,
        }
    }
}

impl App<Applying> {
    pub fn finish(self) -> App<Done> {
        App {
            state: self.state,
            marker: PhantomData,
        }
    }
}

impl App<Done> {
    pub fn restart(self) -> App<Init> {
        App {
            state: self.state,
            marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AppState {
    Init(App<Init>),
    Scanning(App<Scanning>),
    Ready(App<Ready>),
    Viewing(App<Viewing>),
    Applying(App<Applying>),
    Done(App<Done>),
}

impl AppState {
    pub fn new(queue_mode: bool, action_mapping: ActionMapping, sort_mode: SortMode) -> Self {
        AppState::Init(App::new(queue_mode, action_mapping, sort_mode))
    }

    pub fn mode(&self) -> AppMode {
        match self {
            AppState::Init(_) => AppMode::Init,
            AppState::Scanning(_) => AppMode::Scanning,
            AppState::Ready(_) => AppMode::Ready,
            AppState::Viewing(_) => AppMode::Viewing,
            AppState::Applying(_) => AppMode::Applying,
            AppState::Done(_) => AppMode::Done,
        }
    }

    pub fn state(&self) -> &AppStateInner {
        match self {
            AppState::Init(state) => &state.state,
            AppState::Scanning(state) => &state.state,
            AppState::Ready(state) => &state.state,
            AppState::Viewing(state) => &state.state,
            AppState::Applying(state) => &state.state,
            AppState::Done(state) => &state.state,
        }
    }

    pub fn state_mut(&mut self) -> &mut AppStateInner {
        match self {
            AppState::Init(state) => &mut state.state,
            AppState::Scanning(state) => &mut state.state,
            AppState::Ready(state) => &mut state.state,
            AppState::Viewing(state) => &mut state.state,
            AppState::Applying(state) => &mut state.state,
            AppState::Done(state) => &mut state.state,
        }
    }

    pub fn transition_to_scanning(&mut self) {
        if let AppState::Init(state) = self.clone() {
            *self = AppState::Scanning(state.start_scan());
        }
    }

    pub fn transition_to_ready(&mut self, images: Vec<ImageEntry>, root_dir: Option<PathBuf>) {
        if let AppState::Scanning(state) = self.clone() {
            *self = AppState::Ready(state.with_images(images, root_dir));
        }
    }

    pub fn transition_to_viewing(&mut self) {
        if let AppState::Ready(state) = self.clone() {
            *self = AppState::Viewing(state.start_viewing());
        }
    }

    pub fn transition_to_applying(&mut self) {
        if let AppState::Viewing(state) = self.clone() {
            *self = AppState::Applying(state.start_applying());
        }
    }

    pub fn transition_to_done(&mut self) {
        if let AppState::Applying(state) = self.clone() {
            *self = AppState::Done(state.finish());
        }
    }
}
