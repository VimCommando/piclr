use std::marker::PhantomData;
use std::path::PathBuf;
use std::time::SystemTime;

use super::actions::{ActionConfig, ActionMapping, DecisionSide, DecisionState};
use super::preload::PreloadState;
use super::sorting::{sort_indices, SortMode};
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
    pub queue_mode: bool,
    pub action_mapping: ActionMapping,
    pub sort_mode: SortMode,
    pub undo_stack: Vec<UndoEntry>,
    pub preload: PreloadState,
    pub rename_counter: u64,
    pub root_dir: Option<PathBuf>,
    pub view_stack: Vec<ModalView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalView {
    OpenDirectory,
    DeleteConfirm,
    Queue,
    Files,
    Help,
}

impl AppStateInner {
    pub fn new(queue_mode: bool, action_mapping: ActionMapping, sort_mode: SortMode) -> Self {
        Self {
            images: Vec::new(),
            order: Vec::new(),
            cursor: 0,
            queue_mode,
            action_mapping,
            sort_mode,
            undo_stack: Vec::new(),
            preload: PreloadState::default(),
            rename_counter: 1,
            root_dir: None,
            view_stack: Vec::new(),
        }
    }

    pub fn set_images(&mut self, images: Vec<ImageEntry>, root_dir: Option<PathBuf>) {
        self.images = images;
        self.order = sort_indices(&self.images, self.sort_mode);
        self.cursor = 0;
        self.root_dir = root_dir;
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
        self.update_preload();
    }

    pub fn next(&mut self) {
        if self.cursor + 1 < self.order.len() {
            self.cursor += 1;
            self.update_preload();
        }
    }

    pub fn prev(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
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
            self.update_preload();
        }
    }

    pub fn select_image_by_id(&mut self, image_id: u64) -> bool {
        if let Some(position) = self
            .order
            .iter()
            .position(|idx| self.images[*idx].id == image_id)
        {
            self.cursor = position;
            self.update_preload();
            return true;
        }
        false
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

        self.next();
        let cursor_after = self.cursor;

        Some(DecisionOutcome {
            image_id,
            action,
            previous_decision,
            previous_queue,
            cursor_before,
            cursor_after,
            immediate: !queue_mode,
        })
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
            self.update_preload();
            return Some(undo);
        }
        None
    }

    pub fn show_view(&mut self, view: ModalView) {
        self.view_stack.retain(|existing| *existing != view);
        self.view_stack.push(view);
    }

    pub fn close_view(&mut self) -> Option<ModalView> {
        self.view_stack.pop()
    }

    pub fn hide_view(&mut self, view: ModalView) {
        self.view_stack.retain(|existing| *existing != view);
    }

    pub fn has_view(&self, view: ModalView) -> bool {
        self.view_stack.contains(&view)
    }

    fn update_preload(&mut self) {
        let next_index = self.order.get(self.cursor + 1).copied();
        self.preload.next_path =
            next_index.and_then(|idx| self.images.get(idx).map(|e| e.path.clone()));
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
pub struct AppState<S> {
    pub inner: AppStateInner,
    marker: PhantomData<S>,
}

impl AppState<Init> {
    pub fn new(queue_mode: bool, action_mapping: ActionMapping, sort_mode: SortMode) -> Self {
        Self {
            inner: AppStateInner::new(queue_mode, action_mapping, sort_mode),
            marker: PhantomData,
        }
    }

    pub fn start_scan(self) -> AppState<Scanning> {
        AppState {
            inner: self.inner,
            marker: PhantomData,
        }
    }
}

impl AppState<Scanning> {
    pub fn with_images(
        mut self,
        images: Vec<ImageEntry>,
        root_dir: Option<PathBuf>,
    ) -> AppState<Ready> {
        self.inner.set_images(images, root_dir);
        AppState {
            inner: self.inner,
            marker: PhantomData,
        }
    }
}

impl AppState<Ready> {
    pub fn start_viewing(self) -> AppState<Viewing> {
        AppState {
            inner: self.inner,
            marker: PhantomData,
        }
    }
}

impl AppState<Viewing> {
    pub fn start_applying(self) -> AppState<Applying> {
        AppState {
            inner: self.inner,
            marker: PhantomData,
        }
    }
}

impl AppState<Applying> {
    pub fn finish(self) -> AppState<Done> {
        AppState {
            inner: self.inner,
            marker: PhantomData,
        }
    }
}

impl AppState<Done> {
    pub fn restart(self) -> AppState<Init> {
        AppState {
            inner: self.inner,
            marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub enum AppStateMachine {
    Init(AppState<Init>),
    Scanning(AppState<Scanning>),
    Ready(AppState<Ready>),
    Viewing(AppState<Viewing>),
    Applying(AppState<Applying>),
    Done(AppState<Done>),
}

impl AppStateMachine {
    pub fn new(queue_mode: bool, action_mapping: ActionMapping, sort_mode: SortMode) -> Self {
        AppStateMachine::Init(AppState::new(queue_mode, action_mapping, sort_mode))
    }

    pub fn mode(&self) -> AppMode {
        match self {
            AppStateMachine::Init(_) => AppMode::Init,
            AppStateMachine::Scanning(_) => AppMode::Scanning,
            AppStateMachine::Ready(_) => AppMode::Ready,
            AppStateMachine::Viewing(_) => AppMode::Viewing,
            AppStateMachine::Applying(_) => AppMode::Applying,
            AppStateMachine::Done(_) => AppMode::Done,
        }
    }

    pub fn inner(&self) -> &AppStateInner {
        match self {
            AppStateMachine::Init(state) => &state.inner,
            AppStateMachine::Scanning(state) => &state.inner,
            AppStateMachine::Ready(state) => &state.inner,
            AppStateMachine::Viewing(state) => &state.inner,
            AppStateMachine::Applying(state) => &state.inner,
            AppStateMachine::Done(state) => &state.inner,
        }
    }

    pub fn inner_mut(&mut self) -> &mut AppStateInner {
        match self {
            AppStateMachine::Init(state) => &mut state.inner,
            AppStateMachine::Scanning(state) => &mut state.inner,
            AppStateMachine::Ready(state) => &mut state.inner,
            AppStateMachine::Viewing(state) => &mut state.inner,
            AppStateMachine::Applying(state) => &mut state.inner,
            AppStateMachine::Done(state) => &mut state.inner,
        }
    }

    pub fn transition_to_scanning(&mut self) {
        if let AppStateMachine::Init(state) = self.clone() {
            *self = AppStateMachine::Scanning(state.start_scan());
        }
    }

    pub fn transition_to_ready(&mut self, images: Vec<ImageEntry>, root_dir: Option<PathBuf>) {
        if let AppStateMachine::Scanning(state) = self.clone() {
            *self = AppStateMachine::Ready(state.with_images(images, root_dir));
        }
    }

    pub fn transition_to_viewing(&mut self) {
        if let AppStateMachine::Ready(state) = self.clone() {
            *self = AppStateMachine::Viewing(state.start_viewing());
        }
    }

    pub fn transition_to_applying(&mut self) {
        if let AppStateMachine::Viewing(state) = self.clone() {
            *self = AppStateMachine::Applying(state.start_applying());
        }
    }

    pub fn transition_to_done(&mut self) {
        if let AppStateMachine::Applying(state) = self.clone() {
            *self = AppStateMachine::Done(state.finish());
        }
    }
}
