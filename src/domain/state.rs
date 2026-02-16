use std::collections::VecDeque;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::time::SystemTime;

use super::actions::{ActionConfig, ActionMapping, DecisionSide, DecisionState};
use super::preload::PreloadState;
use super::sorting::{SortMode, sort_indices};
use super::undo::UndoEntry;

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

#[derive(Clone, Debug)]
pub struct AppStateInner {
    pub images: Vec<ImageEntry>,
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
pub enum ModalView {
    DeleteConfirm,
    Queue,
    Files,
    Help,
    ApplyResult,
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
            active_modal: None,
            projection: ReadModelProjection::default(),
            last_apply_result: None,
            nav_direction: None,
            nav_tick: 0,
        }
    }

    pub fn set_images(&mut self, images: Vec<ImageEntry>, root_dir: Option<PathBuf>) {
        self.images = images;
        self.scan_version = self.scan_version.saturating_add(1);
        self.order = sort_indices(&self.images, self.sort_mode);
        self.cursor = 0;
        self.root_dir = root_dir;
        self.rebuild_queue_from_order();
        self.rebuild_projection();
        self.update_preload();
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
    }

    pub fn next(&mut self) {
        if self.cursor + 1 < self.order.len() {
            self.cursor += 1;
            self.record_nav(NavDirection::Down);
            self.update_stack_projection(5);
            self.update_preload();
        }
    }

    pub fn prev(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.record_nav(NavDirection::Up);
            self.update_stack_projection(5);
            self.update_preload();
        }
    }

    pub fn jump_next_undecided(&mut self) {
        if self.order.is_empty() {
            return;
        }
        let start = self.cursor + 1;
        if let Some((offset, _)) = self
            .order
            .iter()
            .skip(start)
            .enumerate()
            .find(|(_, idx)| self.images[**idx].decision.is_undecided())
        {
            self.cursor = start + offset;
            self.record_nav(NavDirection::Down);
            self.update_stack_projection(5);
            self.update_preload();
        }
    }

    pub fn jump_prev_undecided(&mut self) {
        if self.order.is_empty() {
            return;
        }
        if self.cursor == 0 {
            return;
        }
        if let Some(position) = (0..self.cursor)
            .rev()
            .find(|pos| self.images[self.order[*pos]].decision.is_undecided())
        {
            self.cursor = position;
            self.record_nav(NavDirection::Up);
            self.update_stack_projection(5);
            self.update_preload();
        }
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
            return true;
        }
        false
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
            self.cursor = undo.previous_cursor;
            self.rebuild_queue_from_order();
            self.rebuild_projection();
            self.update_preload();
            return Some(undo);
        }
        None
    }

    pub fn show_view(&mut self, view: ModalView) {
        self.active_modal = Some(view);
    }

    pub fn close_view(&mut self) -> Option<ModalView> {
        self.active_modal.take()
    }

    pub fn hide_view(&mut self, view: ModalView) {
        if self.active_modal == Some(view) {
            self.active_modal = None;
        }
    }

    pub fn has_view(&self, view: ModalView) -> bool {
        self.active_modal == Some(view)
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
