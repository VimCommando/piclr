use askama::Template;
use asynk_strim::{Yielder, stream_fn};
use axum::Router;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive};
use axum::response::{Html, IntoResponse, Response, Sse};
use axum::routing::{get, patch, post};
use bytes::Bytes;
use core::convert::Infallible;
use datastar::axum::ReadSignals;
use datastar::prelude::{ElementPatchMode, PatchElements, PatchSignals};
use mime_guess::MimeGuess;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Component, PathBuf};
#[cfg(test)]
use std::sync::{Arc, OnceLock};
use tokio::sync::broadcast::error::RecvError;
use tracing::warn;

use crate::app::AppContext;
use crate::domain::{
    ActionConfig, DecisionSide, ImageEntry, ModalView, NavEntryKind, SortDirection, SortKey,
    SortMode,
};
use crate::fs::{
    FsConfig, apply_action, apply_action_with_undo, apply_undo_action, load_image_bytes,
    scan_directories, scan_images,
};

#[cfg(test)]
static APPLY_DECISION_BEFORE_FS_BARRIER: OnceLock<
    tokio::sync::Mutex<Option<Arc<tokio::sync::Barrier>>>,
> = OnceLock::new();

#[derive(Clone)]
pub struct WebState {
    pub ctx: AppContext,
}

#[derive(Clone, Copy)]
struct UiPatch {
    header: bool,
    viewer: bool,
    stack: bool,
    stack_reset: bool,
    sidebar: bool,
    queue: bool,
    modals: bool,
    signals: bool,
}

impl UiPatch {
    const ALL: Self = Self {
        header: true,
        viewer: true,
        stack: true,
        stack_reset: true,
        sidebar: true,
        queue: true,
        modals: true,
        signals: true,
    };

    const VIEWER_AND_SIGNALS: Self = Self {
        header: false,
        viewer: false,
        stack: true,
        stack_reset: false,
        sidebar: true,
        queue: true,
        modals: false,
        signals: true,
    };

    const MODALS_ONLY: Self = Self {
        header: false,
        viewer: false,
        stack: false,
        stack_reset: false,
        sidebar: false,
        queue: false,
        modals: true,
        signals: false,
    };

    const VIEWER_MODALS_AND_SIGNALS: Self = Self {
        header: false,
        viewer: false,
        stack: true,
        stack_reset: false,
        sidebar: true,
        queue: true,
        modals: true,
        signals: true,
    };

    const RESET_QUEUE: Self = Self {
        header: false,
        viewer: false,
        stack: true,
        stack_reset: true,
        sidebar: true,
        queue: true,
        modals: true,
        signals: true,
    };
}

pub fn router(ctx: AppContext) -> Router {
    let state = WebState { ctx };
    Router::new()
        .route("/", get(index))
        .route("/cmd/left", post(cmd_left))
        .route("/cmd/right", post(cmd_right))
        .route("/cmd/left-image", post(cmd_left_image))
        .route("/cmd/right-image", post(cmd_right_image))
        .route("/cmd/next", post(cmd_next))
        .route("/cmd/prev", post(cmd_prev))
        .route("/cmd/jump-next", post(cmd_jump_next))
        .route("/cmd/jump-prev", post(cmd_jump_prev))
        .route("/cmd/home", post(cmd_home))
        .route("/cmd/end", post(cmd_end))
        .route("/cmd/undo", post(cmd_undo))
        .route("/cmd/apply", post(cmd_apply))
        .route("/cmd/apply/request", post(cmd_apply_request))
        .route("/cmd/apply-confirm", post(cmd_apply_confirm))
        .route("/cmd/queue/reset", post(cmd_reset_queue))
        .route("/cmd/queue/prev", post(cmd_queue_prev))
        .route("/cmd/queue/next", post(cmd_queue_next))
        .route("/cmd/queue/select/{id}", post(cmd_queue_select))
        .route("/cmd/queue/apply-selected", post(cmd_queue_apply_selected))
        .route("/cmd/queue/remove-selected", post(cmd_queue_remove_selected))
        .route("/cmd/sidebar/toggle", post(cmd_toggle_sidebar))
        .route("/cmd/sidebar/root/select", post(cmd_sidebar_root_select))
        .route("/cmd/sidebar/open", post(cmd_sidebar_open))
        .route("/cmd/sidebar/sort/{mode}", post(cmd_sidebar_sort))
        .route("/cmd/sidebar/open-parent", post(cmd_sidebar_open_parent))
        .route("/cmd/sidebar/rename", patch(cmd_sidebar_rename))
        .route("/cmd/sidebar/delete", post(cmd_sidebar_delete_request))
        .route(
            "/cmd/sidebar/delete/confirm",
            post(cmd_sidebar_delete_confirm),
        )
        .route(
            "/cmd/sidebar/change-directory/cancel",
            post(cmd_sidebar_change_directory_cancel),
        )
        .route(
            "/cmd/sidebar/change-directory/apply",
            post(cmd_sidebar_change_directory_apply),
        )
        .route(
            "/cmd/sidebar/change-directory/clear",
            post(cmd_sidebar_change_directory_clear),
        )
        .route("/cmd/queue/show", post(cmd_show_queue))
        .route("/cmd/help", post(cmd_help))
        .route("/cmd/select/{id}", post(cmd_select))
        .route("/cmd/sidebar/open-entry/{id}", post(cmd_sidebar_open_entry))
        .route("/cmd/close", post(cmd_close))
        .route("/events", get(events))
        .route("/image/{id}", get(image))
        .route("/image/by-path/{rel}", get(image_by_rel_path))
        .route("/favicon.ico", get(favicon_ico))
        .route("/assets/datastar.js", get(datastar_js))
        .route("/assets/app.css", get(app_css))
        .with_state(state)
}

async fn index(State(state): State<WebState>) -> Html<String> {
    let ctx = state.ctx.clone();
    render_full_page(&ctx).await
}

async fn cmd_left(State(state): State<WebState>) -> impl IntoResponse {
    let (selected_directory, target_parent) = {
        let mut guard = state.ctx.state.write().await;
        let app_state = guard.state_mut();
        app_state.close_directory_actions();
        let selected_directory = app_state.selected_entry_is_directory();
        let target_parent = if selected_directory {
            app_state.navigate_to_parent_directory()
        } else {
            None
        };
        (selected_directory, target_parent)
    };
    if let Some(path) = target_parent {
        attempt_directory_change(&state.ctx, path).await;
    } else if selected_directory {
        let ctx = state.ctx.clone();
        broadcast_viewer_and_signals(&ctx).await;
    } else {
        let ctx = state.ctx.clone();
        apply_decision(ctx, DecisionSide::Left).await;
    }
    StatusCode::NO_CONTENT
}

async fn cmd_right(State(state): State<WebState>) -> impl IntoResponse {
    let target_dir = {
        let mut guard = state.ctx.state.write().await;
        let app_state = guard.state_mut();
        app_state.close_directory_actions();
        if app_state.selected_entry_is_directory() {
            app_state.navigate_to_selected_directory()
        } else {
            None
        }
    };
    if let Some(path) = target_dir {
        attempt_directory_change(&state.ctx, path).await;
    } else {
        let ctx = state.ctx.clone();
        apply_decision(ctx, DecisionSide::Right).await;
    }
    StatusCode::NO_CONTENT
}

async fn cmd_left_image(State(state): State<WebState>) -> impl IntoResponse {
    let ctx = state.ctx.clone();
    apply_decision(ctx, DecisionSide::Left).await;
    StatusCode::NO_CONTENT
}

async fn cmd_right_image(State(state): State<WebState>) -> impl IntoResponse {
    let ctx = state.ctx.clone();
    apply_decision(ctx, DecisionSide::Right).await;
    StatusCode::NO_CONTENT
}

async fn cmd_next(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    app_state.close_directory_actions();
    if app_state.sidebar_expanded {
        app_state.select_next_entry();
    } else {
        app_state.next();
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_prev(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    app_state.close_directory_actions();
    if app_state.sidebar_expanded {
        app_state.select_prev_entry();
    } else {
        app_state.prev();
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_jump_next(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().jump_next_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_jump_prev(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().jump_prev_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_home(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    app_state.close_directory_actions();
    if app_state.queue_is_selected() {
        app_state.select_queue_first();
    } else if app_state.sidebar_expanded {
        app_state.select_first_entry();
    } else {
        app_state.select_first_image();
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_end(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    app_state.close_directory_actions();
    if app_state.queue_is_selected() {
        app_state.select_queue_last();
    } else if app_state.sidebar_expanded {
        app_state.select_last_entry();
    } else {
        app_state.select_last_image();
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_undo(State(state): State<WebState>) -> impl IntoResponse {
    let (undo_action, undone_image_id) = {
        let mut guard = state.ctx.state.write().await;
        let undone = guard
            .state_mut()
            .undo_last()
            .map(|entry| (entry.undo_action, entry.image_id));
        undone.unwrap_or((None, 0))
    };

    if let Some(action) = undo_action {
        if let Err(err) = apply_undo_action(&action).await {
            warn!(%err, "Failed to undo action");
        }
    }

    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    if undone_image_id != 0 {
        patch_stack_card_if_visible(&ctx, undone_image_id).await;
    }
    StatusCode::NO_CONTENT
}

async fn cmd_apply(State(state): State<WebState>) -> impl IntoResponse {
    let needs_confirm = {
        let mut guard = state.ctx.state.write().await;
        guard.state_mut().hide_view(ModalView::ApplyConfirm);
        drop(guard);
        let guard = state.ctx.state.read().await;
        let has_delete = guard
            .state()
            .images
            .iter()
            .any(|image| matches!(image.queued_action, Some(ActionConfig::Delete)));
        state.ctx.config.destructive_delete && has_delete
    };

    if needs_confirm {
        let mut guard = state.ctx.state.write().await;
        guard.state_mut().show_view(ModalView::DeleteConfirm);
        drop(guard);
        let ctx = state.ctx.clone();
        broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
        return StatusCode::NO_CONTENT;
    }

    let summary = apply_queue(&state.ctx).await;
    finalize_apply_result(&state.ctx, summary).await;
    StatusCode::NO_CONTENT
}

async fn cmd_apply_request(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    app_state.close_directory_actions();
    app_state.show_view(ModalView::ApplyConfirm);
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

async fn cmd_apply_confirm(State(state): State<WebState>) -> impl IntoResponse {
    {
        let mut guard = state.ctx.state.write().await;
        guard.state_mut().hide_view(ModalView::DeleteConfirm);
    }
    let summary = apply_queue(&state.ctx).await;
    finalize_apply_result(&state.ctx, summary).await;
    StatusCode::NO_CONTENT
}

async fn cmd_reset_queue(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().reset_queue_state();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::RESET_QUEUE).await;
    StatusCode::NO_CONTENT
}

async fn cmd_queue_prev(State(state): State<WebState>) -> impl IntoResponse {
    move_queue_selection(&state, false).await
}

async fn cmd_queue_next(State(state): State<WebState>) -> impl IntoResponse {
    move_queue_selection(&state, true).await
}

async fn move_queue_selection(state: &WebState, forward: bool) -> StatusCode {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    let moved = if forward {
        app_state.select_queue_next()
    } else {
        app_state.select_queue_prev()
    };
    if moved {
        if let Some(selected) = app_state.selected_queue_image_id {
            activate_queue_item_selection(app_state, selected);
        }
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_queue_select(State(state): State<WebState>, Path(id): Path<u64>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    let selected = activate_queue_item_selection(app_state, id);
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    if selected {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn cmd_queue_apply_selected(State(state): State<WebState>) -> impl IntoResponse {
    let selected = {
        let guard = state.ctx.state.read().await;
        guard.state().queue_selected_item_for_apply()
    };
    let Some((image_id, path, action, rename_sequence)) = selected else {
        return StatusCode::NO_CONTENT;
    };

    let (root_dir, destructive) = {
        let guard = state.ctx.state.read().await;
        (
            guard.state().root_dir.clone(),
            state.ctx.config.destructive_delete,
        )
    };
    let Some(root_dir) = root_dir else {
        return StatusCode::NO_CONTENT;
    };
    let config = FsConfig::new(root_dir, destructive);
    let apply_result = apply_action(&config, &path, &action, rename_sequence).await;
    if let Err(err) = apply_result {
        warn!(%err, path = %path.display(), "Failed to apply selected queued action");
        return StatusCode::NO_CONTENT;
    }

    {
        let mut guard = state.ctx.state.write().await;
        let app_state = guard.state_mut();
        app_state.apply_selected_queue_action_result(image_id, &action);
    }
    let ctx = state.ctx.clone();
    // Force a full viewer/stack refresh so removed files immediately disappear
    // from the image stack after single-item queue apply.
    broadcast_patch(&ctx, UiPatch::ALL).await;
    StatusCode::NO_CONTENT
}

fn activate_queue_item_selection(
    app_state: &mut crate::domain::state::AppStateInner,
    image_id: u64,
) -> bool {
    let selected = app_state.select_queue_item_by_id(image_id);
    if selected {
        app_state.select_image_by_id(image_id);
        app_state.activate_queue_focus();
    }
    selected
}

async fn cmd_queue_remove_selected(State(state): State<WebState>) -> impl IntoResponse {
    let removed = {
        let mut guard = state.ctx.state.write().await;
        guard.state_mut().remove_selected_queue_item()
    };
    let Some(image_id) = removed else {
        return StatusCode::NO_CONTENT;
    };
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    patch_stack_card_if_visible(&ctx, image_id).await;
    StatusCode::NO_CONTENT
}

#[derive(Default)]
struct ApplySummary {
    total: usize,
    completed: usize,
    failed: usize,
    errors: Vec<String>,
}

async fn apply_queue(ctx: &AppContext) -> ApplySummary {
    let (root_dir, destructive) = {
        let guard = ctx.state.read().await;
        (
            guard.state().root_dir.clone(),
            ctx.config.destructive_delete,
        )
    };

    let Some(root_dir) = root_dir else {
        return ApplySummary::default();
    };

    let config = FsConfig::new(root_dir, destructive);
    let queued_actions = {
        let guard = ctx.state.read().await;
        guard.state().queued_actions_for_apply()
    };
    let mut summary = ApplySummary::default();
    for (path, action, rename_sequence) in queued_actions {
        summary.total += 1;
        if let Err(err) = apply_action(&config, &path, &action, rename_sequence).await {
            summary.failed += 1;
            summary.errors.push(format!("{}: {}", path.display(), err));
            warn!(%err, path = %path.display(), "Failed to apply queued action");
        } else {
            summary.completed += 1;
        }
    }
    let mut guard = ctx.state.write().await;
    guard.state_mut().clear_queued_actions();
    summary
}

async fn finalize_apply_result(ctx: &AppContext, summary: ApplySummary) {
    refresh_images_after_apply(ctx).await;
    {
        let mut guard = ctx.state.write().await;
        let state = guard.state_mut();
        state.last_apply_result = Some(crate::domain::state::ApplyResultSummary {
            completed: summary.completed,
            total: summary.total,
            failed: summary.failed,
            errors: summary.errors,
        });
        state.show_view(ModalView::ApplyResult);
    }
    broadcast_patch(ctx, UiPatch::VIEWER_MODALS_AND_SIGNALS).await;
}

async fn refresh_images_after_apply(ctx: &AppContext) {
    let current_dir = {
        let guard = ctx.state.read().await;
        guard.state().current_dir.clone()
    };
    if let Some(path) = current_dir {
        run_scan(ctx, path).await;
    }
}

#[derive(Deserialize)]
struct RenameDirectorySignals {
    #[serde(alias = "directoryName", default)]
    directory_name: String,
}

#[derive(Deserialize)]
struct RootDirectorySignals {
    #[serde(default)]
    path: String,
}

async fn cmd_toggle_sidebar(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().toggle_sidebar();
    guard.state_mut().close_directory_actions();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_viewer_and_signals(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_sidebar_root_select(
    State(state): State<WebState>,
    Json(signals): Json<RootDirectorySignals>,
) -> impl IntoResponse {
    if !cfg!(feature = "tauri") {
        return StatusCode::FORBIDDEN;
    }

    let selected = signals.path.trim();
    if selected.is_empty() {
        return StatusCode::BAD_REQUEST;
    }

    let canonical = match tokio::fs::canonicalize(PathBuf::from(selected)).await {
        Ok(path) => path,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
    match tokio::fs::metadata(&canonical).await {
        Ok(meta) if meta.is_dir() => {}
        _ => return StatusCode::BAD_REQUEST,
    }

    run_scan_with_root(&state.ctx, canonical.clone(), Some(canonical)).await;
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::ALL).await;
    StatusCode::NO_CONTENT
}

async fn cmd_sidebar_open(State(state): State<WebState>) -> impl IntoResponse {
    let path = {
        let guard = state.ctx.state.read().await;
        guard.state().navigate_to_selected_directory()
    };
    if let Some(path) = path {
        attempt_directory_change(&state.ctx, path).await;
    } else {
        let ctx = state.ctx.clone();
        broadcast_viewer_and_signals(&ctx).await;
    }
    StatusCode::NO_CONTENT
}

async fn cmd_sidebar_sort(State(state): State<WebState>, Path(mode): Path<String>) -> impl IntoResponse {
    let sort_mode = match mode.as_str() {
        "name-asc" => Some(SortMode {
            key: SortKey::Alphabetical,
            direction: SortDirection::Asc,
        }),
        "name-desc" => Some(SortMode {
            key: SortKey::Alphabetical,
            direction: SortDirection::Desc,
        }),
        "modified-asc" => Some(SortMode {
            key: SortKey::LastModified,
            direction: SortDirection::Asc,
        }),
        "modified-desc" => Some(SortMode {
            key: SortKey::LastModified,
            direction: SortDirection::Desc,
        }),
        "size-asc" => Some(SortMode {
            key: SortKey::Size,
            direction: SortDirection::Asc,
        }),
        "size-desc" => Some(SortMode {
            key: SortKey::Size,
            direction: SortDirection::Desc,
        }),
        _ => None,
    };
    let Some(sort_mode) = sort_mode else {
        return StatusCode::BAD_REQUEST;
    };

    let current_dir = {
        let mut guard = state.ctx.state.write().await;
        let app_state = guard.state_mut();
        app_state.set_sort_mode(sort_mode);
        app_state.current_dir.clone()
    };

    if let Some(current_dir) = current_dir {
        run_scan(&state.ctx, current_dir).await;
    }
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::ALL).await;
    StatusCode::NO_CONTENT
}

async fn cmd_sidebar_open_parent(State(state): State<WebState>) -> impl IntoResponse {
    let path = {
        let guard = state.ctx.state.read().await;
        guard.state().navigate_to_parent_directory()
    };
    if let Some(path) = path {
        attempt_directory_change(&state.ctx, path).await;
    } else {
        let ctx = state.ctx.clone();
        broadcast_viewer_and_signals(&ctx).await;
    }
    StatusCode::NO_CONTENT
}

async fn cmd_sidebar_change_directory_cancel(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    app_state.pending_directory_path = None;
    app_state.pending_delete_directory_path = None;
    if app_state.active_modal == Some(ModalView::QueueNotEmptyConfirm) {
        app_state.active_modal = None;
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

async fn cmd_sidebar_change_directory_apply(State(state): State<WebState>) -> impl IntoResponse {
    let target_path = {
        let mut guard = state.ctx.state.write().await;
        let app_state = guard.state_mut();
        let target = app_state.pending_directory_path.take();
        app_state.pending_delete_directory_path = None;
        if app_state.active_modal == Some(ModalView::QueueNotEmptyConfirm) {
            app_state.active_modal = None;
        }
        target
    };
    let Some(target_path) = target_path else {
        return StatusCode::NO_CONTENT;
    };

    let _ = apply_queue(&state.ctx).await;
    run_scan(&state.ctx, target_path).await;
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::ALL).await;
    StatusCode::NO_CONTENT
}

async fn cmd_sidebar_change_directory_clear(State(state): State<WebState>) -> impl IntoResponse {
    let target_path = {
        let mut guard = state.ctx.state.write().await;
        let app_state = guard.state_mut();
        let target = app_state.pending_directory_path.take();
        app_state.pending_delete_directory_path = None;
        app_state.clear_queued_actions();
        if app_state.active_modal == Some(ModalView::QueueNotEmptyConfirm) {
            app_state.active_modal = None;
        }
        target
    };
    let Some(target_path) = target_path else {
        return StatusCode::NO_CONTENT;
    };

    run_scan(&state.ctx, target_path).await;
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::ALL).await;
    StatusCode::NO_CONTENT
}

async fn cmd_sidebar_rename(
    State(state): State<WebState>,
    ReadSignals(signals): ReadSignals<RenameDirectorySignals>,
) -> impl IntoResponse {
    let new_name = signals.directory_name.trim();
    if new_name.is_empty() {
        return StatusCode::NO_CONTENT;
    }
    let (source, target) = {
        let guard = state.ctx.state.read().await;
        let Some(source) = guard.state().selected_directory_path() else {
            return StatusCode::NO_CONTENT;
        };
        let Some(parent) = source.parent() else {
            return StatusCode::BAD_REQUEST;
        };
        let target = parent.join(new_name);
        let root = guard.state().root_dir.clone();
        if let Some(root) = root {
            if !target.starts_with(&root) {
                return StatusCode::BAD_REQUEST;
            }
        }
        (source, target)
    };
    if let Err(err) = tokio::fs::rename(&source, &target).await {
        warn!(%err, source=%source.display(), target=%target.display(), "Failed to rename directory");
    }
    let current_dir = {
        let guard = state.ctx.state.read().await;
        guard.state().current_dir.clone()
    };
    if let Some(current_dir) = current_dir {
        run_scan(&state.ctx, current_dir).await;
    }
    let ctx = state.ctx.clone();
    broadcast_viewer_and_signals(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_sidebar_delete_request(State(state): State<WebState>) -> impl IntoResponse {
    let path = {
        let mut guard = state.ctx.state.write().await;
        let app_state = guard.state_mut();
        let is_parent_link = app_state
            .selected_entry()
            .map(|entry| entry.is_parent_link)
            .unwrap_or(false);
        let path = if is_parent_link {
            None
        } else {
            app_state.selected_directory_path()
        };
        if let Some(path) = path.clone() {
            app_state.pending_delete_directory_path = Some(path);
            app_state.active_modal = Some(ModalView::DirectoryDeleteConfirm);
        }
        path
    };
    let Some(_) = path else {
        return StatusCode::NO_CONTENT;
    };
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

async fn cmd_sidebar_delete_confirm(State(state): State<WebState>) -> impl IntoResponse {
    let path = {
        let mut guard = state.ctx.state.write().await;
        let app_state = guard.state_mut();
        let path = app_state.pending_delete_directory_path.take();
        if app_state.active_modal == Some(ModalView::DirectoryDeleteConfirm) {
            app_state.active_modal = None;
        }
        path
    };
    let Some(path) = path else {
        return StatusCode::NO_CONTENT;
    };
    if let Err(err) = tokio::fs::remove_dir(&path).await {
        warn!(%err, path=%path.display(), "Failed to delete directory");
    }
    let current_dir = {
        let guard = state.ctx.state.read().await;
        guard.state().current_dir.clone()
    };
    if let Some(current_dir) = current_dir {
        run_scan(&state.ctx, current_dir).await;
    }
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::VIEWER_MODALS_AND_SIGNALS).await;
    StatusCode::NO_CONTENT
}

async fn cmd_show_queue(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    app_state.close_directory_actions();
    app_state.toggle_queue_sidebar();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_viewer_and_signals(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_help(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    app_state.close_directory_actions();
    if app_state.active_modal == Some(ModalView::Help) {
        app_state.close_view();
    } else {
        app_state.show_view(ModalView::Help);
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

async fn cmd_select(State(state): State<WebState>, Path(id): Path<u64>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let selected = guard.state_mut().select_entry_by_id(id);
    if selected {
        guard.state_mut().close_directory_actions();
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::VIEWER_MODALS_AND_SIGNALS).await;
    if selected {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn cmd_sidebar_open_entry(
    State(state): State<WebState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    let path = {
        let mut guard = state.ctx.state.write().await;
        let app_state = guard.state_mut();
        if !app_state.select_entry_by_id(id) {
            return StatusCode::NOT_FOUND;
        }
        app_state.navigate_to_selected_directory()
    };
    if let Some(path) = path {
        attempt_directory_change(&state.ctx, path).await;
    } else {
        let ctx = state.ctx.clone();
        broadcast_viewer_and_signals(&ctx).await;
    }
    StatusCode::NO_CONTENT
}

async fn cmd_close(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().close_view();
    guard.state_mut().close_directory_actions();
    guard.state_mut().pending_directory_path = None;
    guard.state_mut().pending_delete_directory_path = None;
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

async fn apply_decision(ctx: AppContext, side: DecisionSide) {
    let (root_dir, destructive) = {
        let guard = ctx.state.read().await;
        (
            guard.state().root_dir.clone(),
            ctx.config.destructive_delete,
        )
    };

    let (changed_image_id, mut undo_entry, immediate_apply) = {
        let mut guard = ctx.state.write().await;
        let outcome = guard.state_mut().apply_decision(side);
        let changed_image_id = outcome.as_ref().map(|o| o.image_id);
        let mut undo_entry = outcome
            .as_ref()
            .map(|outcome| crate::domain::undo::UndoEntry {
                image_id: outcome.image_id,
                previous_decision: outcome.previous_decision.clone(),
                previous_queue: outcome.previous_queue.clone(),
                previous_cursor: outcome.cursor_before,
                undo_action: None,
            });

        let immediate_apply = if let (Some(root_dir), Some(outcome), Some(undo)) =
            (root_dir, outcome.as_ref(), undo_entry.as_mut())
        {
            if outcome.immediate {
                let config = FsConfig::new(root_dir, destructive);
                let image = guard
                    .state()
                    .images
                    .iter()
                    .find(|image| image.id == outcome.image_id)
                    .cloned();
                image.map(|image| {
                    (
                        config,
                        image.path,
                        outcome.action.clone(),
                        image.rename_sequence,
                        undo.image_id,
                    )
                })
            } else {
                None
            }
        } else {
            None
        };
        (changed_image_id, undo_entry, immediate_apply)
    };

    if let (Some((config, image_path, action, rename_sequence, undo_image_id)), Some(undo)) =
        (immediate_apply, undo_entry.as_mut())
    {
        #[cfg(test)]
        wait_for_apply_decision_test_hook().await;
        match apply_action_with_undo(&config, &image_path, &action, rename_sequence).await {
            Ok(action) => {
                if undo.image_id == undo_image_id {
                    undo.undo_action = action;
                }
            }
            Err(err) => {
                warn!(%err, path = %image_path.display(), "Failed to apply action");
            }
        }
    }

    if let Some(undo) = undo_entry {
        let mut guard = ctx.state.write().await;
        guard.state_mut().record_undo(undo);
    }

    broadcast_navigation(&ctx).await;
    if let Some(image_id) = changed_image_id {
        patch_stack_card_if_visible(&ctx, image_id).await;
    }
}

#[cfg(test)]
async fn set_apply_decision_test_barrier(barrier: Option<Arc<tokio::sync::Barrier>>) {
    let hook = APPLY_DECISION_BEFORE_FS_BARRIER.get_or_init(|| tokio::sync::Mutex::new(None));
    let mut guard = hook.lock().await;
    *guard = barrier;
}

#[cfg(test)]
async fn wait_for_apply_decision_test_hook() {
    let hook = APPLY_DECISION_BEFORE_FS_BARRIER.get_or_init(|| tokio::sync::Mutex::new(None));
    let barrier = { hook.lock().await.clone() };
    if let Some(barrier) = barrier {
        barrier.wait().await;
    }
}

// undo helpers are handled in fs::apply_action_with_undo

async fn run_scan(ctx: &AppContext, path: PathBuf) {
    run_scan_with_root(ctx, path, None).await;
}

async fn run_scan_with_root(ctx: &AppContext, path: PathBuf, root_override: Option<PathBuf>) {
    let mut images = scan_images(&path).await;
    let root_dir = {
        let guard = ctx.state.read().await;
        root_override.unwrap_or_else(|| {
            guard
                .state()
                .root_dir
                .clone()
                .unwrap_or_else(|| path.clone())
        })
    };
    let directories = scan_directories(&path, &root_dir).await;

    let mut next_id = {
        let state = ctx.state.read().await;
        state
            .state()
            .images
            .iter()
            .map(|image| image.id)
            .max()
            .unwrap_or(0)
            + 1
    };
    for image in &mut images {
        image.id = next_id;
        image.decision = crate::domain::DecisionState::Undecided;
        image.queued_action = None;
        image.rename_sequence = None;
        next_id += 1;
    }

    let mut state = ctx.state.write().await;
    state
        .state_mut()
        .set_directory_snapshot(images, directories, Some(root_dir), Some(path));
}

async fn attempt_directory_change(ctx: &AppContext, target_path: PathBuf) {
    let queue_not_empty = {
        let mut guard = ctx.state.write().await;
        let app_state = guard.state_mut();
        app_state.close_directory_actions();
        if !app_state.queued_ids.is_empty() {
            app_state.pending_directory_path = Some(target_path.clone());
            app_state.active_modal = Some(ModalView::QueueNotEmptyConfirm);
            true
        } else {
            app_state.pending_directory_path = None;
            false
        }
    };

    if queue_not_empty {
        broadcast_patch(ctx, UiPatch::MODALS_ONLY).await;
    } else {
        run_scan(ctx, target_path).await;
        broadcast_patch(ctx, UiPatch::ALL).await;
    }
}

async fn image(State(state): State<WebState>, Path(id): Path<u64>) -> Response {
    let image_path = {
        let guard = state.ctx.state.read().await;
        guard
            .state()
            .images
            .iter()
            .find(|image| image.id == id)
            .map(|image| image.path.clone())
    };

    let Some(path) = image_path else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let bytes = match load_image_bytes(&path).await {
        Ok(bytes) => Bytes::from(bytes),
        Err(err) => {
            warn!(%err, path = %path.display(), "Failed to read image");
            return StatusCode::NOT_FOUND.into_response();
        }
    };

    bytes_response(&path, bytes)
}

async fn image_by_rel_path(State(state): State<WebState>, Path(rel): Path<String>) -> Response {
    let decoded = match urlencoding::decode(&rel) {
        Ok(value) => value.into_owned(),
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let rel_path = PathBuf::from(decoded);
    if rel_path.is_absolute()
        || rel_path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let root_dir = {
        let guard = state.ctx.state.read().await;
        guard.state().root_dir.clone()
    };
    let Some(root_dir) = root_dir else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let path = root_dir.join(rel_path);
    let bytes = match load_image_bytes(&path).await {
        Ok(bytes) => Bytes::from(bytes),
        Err(err) => {
            warn!(%err, path = %path.display(), "Failed to read image");
            return StatusCode::NOT_FOUND.into_response();
        }
    };
    bytes_response(&path, bytes)
}

async fn app_css() -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/css"),
    );
    (headers, include_str!("../../assets/app.css")).into_response()
}

async fn datastar_js() -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/javascript"),
    );
    (headers, include_str!("../../assets/datastar.js")).into_response()
}

async fn favicon_ico() -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("image/svg+xml"),
    );
    (headers, include_str!("../../assets/favicon.svg")).into_response()
}

fn bytes_response(path: &PathBuf, bytes: Bytes) -> Response {
    let mime = MimeGuess::from_path(path).first_or_octet_stream();
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    (headers, bytes).into_response()
}

async fn render_full_page(ctx: &AppContext) -> Html<String> {
    let view = build_view(ctx).await;
    let template = MainTemplate { view };
    Html(template.render().unwrap_or_default())
}

async fn events(State(state): State<WebState>) -> impl IntoResponse {
    let ctx = state.ctx.clone();
    let mut rx = ctx.sse_tx.subscribe();
    Sse::new(stream_fn(
        move |mut yielder: Yielder<Result<Event, Infallible>>| async move {
            let initial_events = build_full_resync_events(&ctx).await;
            for event in initial_events {
                yielder.yield_item(Ok(event)).await;
            }

            loop {
                match rx.recv().await {
                    Ok(event) => {
                        yielder.yield_item(Ok(event)).await;
                    }
                    Err(err) => {
                        if let Some(resync_events) = stream_recovery_events(&ctx, err).await {
                            for event in resync_events {
                                yielder.yield_item(Ok(event)).await;
                            }
                            continue;
                        }
                        break;
                    }
                }
            }
        },
    ))
    .keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(20))
            .text("keep-alive"),
    )
}

async fn broadcast_patch(ctx: &AppContext, patch: UiPatch) {
    let events = build_patch_events(ctx, patch).await;
    for event in events {
        let _ = ctx.sse_tx.send(event);
    }
}

async fn broadcast_navigation(ctx: &AppContext) {
    let patch = navigation_patch(ctx).await;
    broadcast_patch(ctx, patch).await;
}

async fn broadcast_viewer_and_signals(ctx: &AppContext) {
    broadcast_patch(ctx, UiPatch::VIEWER_AND_SIGNALS).await;
}

async fn build_full_resync_events(ctx: &AppContext) -> Vec<Event> {
    build_patch_events(ctx, UiPatch::ALL).await
}

async fn stream_recovery_events(ctx: &AppContext, err: RecvError) -> Option<Vec<Event>> {
    match err {
        RecvError::Lagged(_) => Some(build_full_resync_events(ctx).await),
        RecvError::Closed => None,
    }
}

async fn build_patch_events(ctx: &AppContext, patch: UiPatch) -> Vec<Event> {
    let view = build_view(ctx).await;
    let mut events = Vec::new();
    let rendered_ids = { ctx.rendered_stack_ids.read().await.clone() };
    let needs_viewer_swap = (view.total == 0 && !rendered_ids.is_empty())
        || (view.total > 0 && rendered_ids.is_empty());

    if patch.header {
        let html = HeaderTemplate { view: &view }.render().unwrap_or_default();
        let patch = PatchElements::new(html)
            .selector("#header")
            .mode(ElementPatchMode::Outer);
        events.push(patch.write_as_axum_sse_event());
    }

    if patch.viewer || needs_viewer_swap {
        let html = if view.total > 0 {
            ImageViewerTemplate {}.render().unwrap_or_default()
        } else {
            ImageViewerEmptyTemplate {}.render().unwrap_or_default()
        };
        let patch = PatchElements::new(html)
            .selector("#image-viewer")
            .mode(ElementPatchMode::Outer);
        events.push(patch.write_as_axum_sse_event());
    }

    if patch.sidebar {
        let html = SidebarTemplate { view: &view }.render().unwrap_or_default();
        let patch = PatchElements::new(html)
            .selector("#sidebar")
            .mode(ElementPatchMode::Outer);
        events.push(patch.write_as_axum_sse_event());
    }

    if patch.queue {
        let html = QueueSidebarTemplate { view: &view }
            .render()
            .unwrap_or_default();
        let patch = PatchElements::new(html)
            .selector("#queue-sidebar")
            .mode(ElementPatchMode::Outer);
        events.push(patch.write_as_axum_sse_event());
    }

    if patch.stack {
        if view.total > 0 {
            events.extend(
                build_stack_patch_events(ctx, &view, patch.viewer || patch.stack_reset).await,
            );
        } else {
            let mut ids_guard = ctx.rendered_stack_ids.write().await;
            ids_guard.clear();
        }
    }

    if patch.modals {
        let html = render_active_modal(&view);
        let patch = PatchElements::new(html)
            .selector("#modal")
            .mode(ElementPatchMode::Outer);
        events.push(patch.write_as_axum_sse_event());
    }

    if patch.signals {
        let patch = PatchSignals::new(counter_signals_json(&view));
        events.push(patch.write_as_axum_sse_event());
    }

    events
}

async fn build_stack_patch_events(
    ctx: &AppContext,
    view: &AppView,
    reset_rendered: bool,
) -> Vec<Event> {
    let mut events = Vec::new();
    let desired_ids: Vec<u64> = view
        .image_stack
        .cards
        .iter()
        .map(|card| card.image_id)
        .collect();
    let desired_set: HashSet<u64> = desired_ids.iter().copied().collect();

    let previous_ids = { ctx.rendered_stack_ids.read().await.clone() };
    let previous_set: HashSet<u64> = previous_ids.iter().copied().collect();

    if reset_rendered {
        let reset_stack = PatchElements::new(
            "<image-stack id=\"image-stack\" data-attr:style=\"'--stack-cursor:' + $stackCursor\"></image-stack>",
        )
        .selector("#image-stack")
        .mode(ElementPatchMode::Outer);
        events.push(reset_stack.write_as_axum_sse_event());
        for card in &view.image_stack.cards {
            let html = render_image_card(card);
            let patch = PatchElements::new(html)
                .selector("#image-stack")
                .mode(ElementPatchMode::Append);
            events.push(patch.write_as_axum_sse_event());
        }
        {
            let mut ids_guard = ctx.rendered_stack_ids.write().await;
            *ids_guard = desired_ids;
        }
        return events;
    }

    for image_id in previous_ids.iter().filter(|id| !desired_set.contains(id)) {
        let remove = PatchElements::new_remove(format!("#image-card-{image_id}"));
        events.push(remove.write_as_axum_sse_event());
    }

    let added_start: Vec<&StackCard> = view
        .image_stack
        .cards
        .iter()
        .take_while(|card| !previous_set.contains(&card.image_id))
        .collect();
    for card in added_start.iter().rev() {
        let html = render_image_card(card);
        let patch = PatchElements::new(html)
            .selector("#image-stack")
            .mode(ElementPatchMode::Prepend);
        events.push(patch.write_as_axum_sse_event());
    }

    let added_end: Vec<&StackCard> = view
        .image_stack
        .cards
        .iter()
        .rev()
        .take_while(|card| !previous_set.contains(&card.image_id))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    for card in added_end {
        let html = render_image_card(card);
        let patch = PatchElements::new(html)
            .selector("#image-stack")
            .mode(ElementPatchMode::Append);
        events.push(patch.write_as_axum_sse_event());
    }

    {
        let mut ids_guard = ctx.rendered_stack_ids.write().await;
        *ids_guard = desired_ids;
    }

    events
}

async fn patch_stack_card_if_visible(ctx: &AppContext, image_id: u64) {
    let view = build_view(ctx).await;
    let Some(card) = view
        .image_stack
        .cards
        .iter()
        .find(|card| card.image_id == image_id)
    else {
        return;
    };
    let html = render_image_card(card);
    let patch = PatchElements::new(html)
        .selector(format!("#image-card-{image_id}"))
        .mode(ElementPatchMode::Outer);
    let _ = ctx.sse_tx.send(patch.write_as_axum_sse_event());
}

fn counter_signals_json(view: &AppView) -> String {
    serde_json::json!({
        "counterLeftAction": view.left_action_count,
        "counterRightAction": view.right_action_count,
        "counterImageIndex": if view.total > 0 { view.index } else { 0 },
        "counterImageTotal": view.total,
        "counterQueueCount": view.queue_count,
        "stackCursor": view.image_stack.cursor,
        "selectedSidebarEntry": view.selected_sidebar_entry,
        "currentPath": view.current_path_label,
        "sidebarExpanded": view.sidebar_expanded,
        "queueSidebarVisible": view.queue_sidebar_visible,
        "queueSelected": view.queue_selected,
        "selectedQueueItem": view.selected_queue_item,
        "directorySelectEnabled": view.directory_select_enabled
    })
    .to_string()
}

async fn navigation_patch(ctx: &AppContext) -> UiPatch {
    let guard = ctx.state.read().await;
    let state = guard.state();
    if state.active_modal.is_some() {
        UiPatch::VIEWER_MODALS_AND_SIGNALS
    } else {
        UiPatch::VIEWER_AND_SIGNALS
    }
}

async fn build_view(ctx: &AppContext) -> AppView {
    let guard = ctx.state.read().await;
    let state = guard.state();
    let projection = state.projection();
    let total = state.order.len();
    let index = state.cursor + 1;
    let current = state.current();
    let active_modal = state.active_modal;
    let last_apply_result = state.last_apply_result.clone();
    let current_path_label = state
        .current_dir
        .as_ref()
        .and_then(|path| {
            state
                .root_dir
                .as_ref()
                .and_then(|root| path.strip_prefix(root).ok())
                .map(|relative| {
                    if relative.as_os_str().is_empty() {
                        "/".to_string()
                    } else {
                        relative.display().to_string()
                    }
                })
        })
        .unwrap_or_else(|| "No directory selected".to_string());
    let sidebar_selected_id = if state.queue_focus {
        None
    } else {
        state.selected_entry_id
    };
    let queue_selected_id = if state.queue_focus {
        state.selected_queue_image_id
    } else {
        None
    };

    let mut queue_items: Vec<QueueItem> = state
        .queued_ids
        .iter()
        .filter_map(|queued_id| state.images.iter().find(|image| image.id == *queued_id))
        .filter_map(|image| {
            let queued = image.queued_action.as_ref()?;
            Some(queue_item_from_action(
                image,
                queued,
                decision_side(image),
                state.root_dir.as_ref(),
            ))
        })
        .collect();
    for item in &mut queue_items {
        item.selected = Some(item.image_id) == queue_selected_id;
        item.peer_active = !item.selected
            && !state.queue_focus
            && Some(item.image_id) == state.current().map(|image| image.id);
    }
    let sidebar_items = state
        .nav_entries
        .iter()
        .map(|entry| {
            let item = match entry.kind {
                NavEntryKind::Directory => SidebarItem {
                    entry_id: entry.id,
                    entry_kind: "directory".to_string(),
                    status_kind: "none".to_string(),
                    action_tone: "neutral".to_string(),
                    file_label: entry.label.clone(),
                    selected: sidebar_selected_id == Some(entry.id),
                    peer_active: false,
                    is_parent_link: entry.is_parent_link,
                    path_hint: entry.rel_path.to_string_lossy().to_string(),
                },
                NavEntryKind::File => {
                    let image = state
                        .images
                        .iter()
                        .find(|image| Some(image.id) == entry.image_id)
                        .expect("nav file entry must reference image");
                    let queue_item = match &image.decision {
                        crate::domain::DecisionState::Decided { side, action } => {
                            queue_item_from_action(
                                image,
                                action,
                                Some(*side),
                                state.root_dir.as_ref(),
                            )
                        }
                        crate::domain::DecisionState::Undecided => {
                            queue_item_none(image, state.root_dir.as_ref())
                        }
                    };
                    SidebarItem {
                        entry_id: entry.id,
                        entry_kind: "file".to_string(),
                        status_kind: queue_item.status_kind,
                        action_tone: queue_item.action_tone,
                        file_label: entry.label.clone(),
                        selected: sidebar_selected_id == Some(entry.id),
                        peer_active: state.queue_focus
                            && sidebar_selected_id != Some(entry.id)
                            && state.selected_queue_image_id == Some(image.id),
                        is_parent_link: false,
                        path_hint: entry.rel_path.to_string_lossy().to_string(),
                    }
                }
            };
            item
        })
        .collect::<Vec<_>>();
    let image_stack = ImageStackProjection {
        cards: build_stack_cards_in_range(state, projection.stack_start, projection.stack_end),
        cursor: state.cursor,
    };
    AppView {
        directory_select_enabled: cfg!(feature = "tauri"),
        active_modal,
        image_stack,
        sidebar_items,
        selected_sort_mode: sort_mode_label(state.sort_mode),
        selected_sidebar_entry: state.selected_entry_id.unwrap_or(0),
        sidebar_expanded: state.sidebar_expanded,
        queue_sidebar_visible: state.queue_sidebar_visible,
        queue_selected: state.queue_is_selected(),
        selected_queue_item: state.selected_queue_image_id.unwrap_or(0),
        current_image_id: current.map(|entry| entry.id).unwrap_or(0),
        current_path_label,
        left_action_label: action_config_label(&state.action_mapping.left),
        right_action_label: action_config_label(&state.action_mapping.right),
        left_action_count: projection.left_action_count,
        right_action_count: projection.right_action_count,
        index,
        total,
        queue_count: projection.queue_count,
        apply_completed: last_apply_result.as_ref().map(|r| r.completed).unwrap_or(0),
        apply_total: last_apply_result.as_ref().map(|r| r.total).unwrap_or(0),
        apply_failed: last_apply_result.as_ref().map(|r| r.failed).unwrap_or(0),
        apply_errors: last_apply_result.map(|r| r.errors).unwrap_or_default(),
        queue_items,
    }
}

#[derive(Clone, Debug)]
pub struct AppView {
    pub directory_select_enabled: bool,
    pub active_modal: Option<ModalView>,
    pub image_stack: ImageStackProjection,
    pub sidebar_items: Vec<SidebarItem>,
    pub selected_sort_mode: String,
    pub selected_sidebar_entry: u64,
    pub sidebar_expanded: bool,
    pub queue_sidebar_visible: bool,
    pub queue_selected: bool,
    pub selected_queue_item: u64,
    pub current_image_id: u64,
    pub current_path_label: String,
    pub left_action_label: String,
    pub right_action_label: String,
    pub left_action_count: usize,
    pub right_action_count: usize,
    pub index: usize,
    pub total: usize,
    pub queue_count: usize,
    pub apply_completed: usize,
    pub apply_total: usize,
    pub apply_failed: usize,
    pub apply_errors: Vec<String>,
    pub queue_items: Vec<QueueItem>,
}

fn sort_mode_label(mode: SortMode) -> String {
    match (mode.key, mode.direction) {
        (SortKey::Alphabetical, SortDirection::Asc) => "name-asc".to_string(),
        (SortKey::Alphabetical, SortDirection::Desc) => "name-desc".to_string(),
        (SortKey::LastModified, SortDirection::Asc) => "modified-asc".to_string(),
        (SortKey::LastModified, SortDirection::Desc) => "modified-desc".to_string(),
        (SortKey::Size, SortDirection::Asc) => "size-asc".to_string(),
        (SortKey::Size, SortDirection::Desc) => "size-desc".to_string(),
        _ => "modified-asc".to_string(),
    }
}

#[derive(Clone, Debug)]
pub struct SidebarItem {
    pub entry_id: u64,
    pub entry_kind: String,
    pub status_kind: String,
    pub action_tone: String,
    pub file_label: String,
    pub selected: bool,
    pub peer_active: bool,
    pub is_parent_link: bool,
    pub path_hint: String,
}

#[derive(Clone, Debug)]
pub struct ImageStackProjection {
    pub cards: Vec<StackCard>,
    pub cursor: usize,
}

#[derive(Clone, Debug)]
pub struct QueueItem {
    pub image_id: u64,
    pub status_kind: String,
    pub action_tone: String,
    pub file_label: String,
    pub selected: bool,
    pub peer_active: bool,
}

#[derive(Clone, Debug)]
pub struct StackCard {
    pub image_id: u64,
    pub image_src: String,
    pub alignment: String,
    pub action_item: QueueItem,
    pub stack_index: usize,
}

fn decision_side(image: &ImageEntry) -> Option<DecisionSide> {
    match &image.decision {
        crate::domain::DecisionState::Decided { side, .. } => Some(*side),
        _ => None,
    }
}

fn image_alignment_for(image: &ImageEntry) -> String {
    match decision_side(image) {
        Some(DecisionSide::Left) => "left".to_string(),
        Some(DecisionSide::Right) => "right".to_string(),
        None => "center".to_string(),
    }
}

fn queue_item_for_image(image: &ImageEntry, root_dir: Option<&PathBuf>) -> QueueItem {
    match &image.decision {
        crate::domain::DecisionState::Decided { side, action } => {
            queue_item_from_action(image, action, Some(*side), root_dir)
        }
        crate::domain::DecisionState::Undecided => queue_item_none(image, root_dir),
    }
}

fn action_config_label(action: &ActionConfig) -> String {
    match action {
        ActionConfig::Keep => "Keep".to_string(),
        ActionConfig::Delete => "Delete".to_string(),
        ActionConfig::Move { target } => target.display().to_string(),
        ActionConfig::Rename { .. } => "Rename".to_string(),
        ActionConfig::MetadataEdit { .. } => "Metadata".to_string(),
    }
}

fn build_stack_cards_in_range(
    state: &crate::domain::state::AppStateInner,
    start: usize,
    end: usize,
) -> Vec<StackCard> {
    if state.order.is_empty() {
        return Vec::new();
    }
    let bounded_start = start.min(state.order.len().saturating_sub(1));
    let bounded_end = end.min(state.order.len().saturating_sub(1));
    if bounded_start > bounded_end {
        return Vec::new();
    }
    (bounded_start..=bounded_end)
        .filter_map(|pos| {
            let idx = *state.order.get(pos)?;
            let image = state.images.get(idx)?;
            let action_item = queue_item_for_image(image, state.root_dir.as_ref());
            Some(StackCard {
                image_id: image.id,
                image_src: image_src_for(image, state.root_dir.as_ref()),
                alignment: image_alignment_for(image),
                action_item,
                stack_index: pos,
            })
        })
        .collect()
}

fn image_src_for(image: &ImageEntry, root_dir: Option<&PathBuf>) -> String {
    let rel = root_dir
        .and_then(|root| image.path.strip_prefix(root).ok())
        .unwrap_or(&image.path)
        .to_string_lossy()
        .to_string();
    format!("/image/by-path/{}", urlencoding::encode(&rel))
}

fn file_label_for_image(image: &ImageEntry, root_dir: Option<&PathBuf>) -> String {
    root_dir
        .and_then(|root| image.path.strip_prefix(root).ok())
        .unwrap_or(&image.path)
        .to_string_lossy()
        .to_string()
}

fn queue_item_from_action(
    image: &ImageEntry,
    _action: &ActionConfig,
    side: Option<DecisionSide>,
    root_dir: Option<&PathBuf>,
) -> QueueItem {
    let action_tone = match side {
        Some(DecisionSide::Left) => "left".to_string(),
        Some(DecisionSide::Right) => "right".to_string(),
        None => "neutral".to_string(),
    };
    let status_kind = match side {
        Some(DecisionSide::Left) => "left".to_string(),
        Some(DecisionSide::Right) => "right".to_string(),
        None => "none".to_string(),
    };
    QueueItem {
        image_id: image.id,
        status_kind,
        action_tone,
        file_label: file_label_for_image(image, root_dir),
        selected: false,
        peer_active: false,
    }
}

fn queue_item_none(image: &ImageEntry, root_dir: Option<&PathBuf>) -> QueueItem {
    QueueItem {
        image_id: image.id,
        status_kind: "none".to_string(),
        action_tone: "neutral".to_string(),
        file_label: file_label_for_image(image, root_dir),
        selected: false,
        peer_active: false,
    }
}

#[cfg(test)]
fn preload_window_paths(
    state: &crate::domain::state::AppStateInner,
    radius: usize,
) -> std::collections::VecDeque<PathBuf> {
    if state.order.is_empty() {
        return std::collections::VecDeque::new();
    }
    let start = state.cursor.saturating_sub(radius);
    let end = (state.cursor + radius).min(state.order.len().saturating_sub(1));
    let mut window = std::collections::VecDeque::new();
    for pos in start..=end {
        if let Some(path) = state
            .order
            .get(pos)
            .and_then(|idx| state.images.get(*idx))
            .map(|entry| entry.path.clone())
        {
            window.push_back(path);
        }
    }
    window
}

#[derive(Template)]
#[template(path = "index.html")]
struct MainTemplate {
    view: AppView,
}

#[derive(Template)]
#[template(path = "elements/header.html")]
struct HeaderTemplate<'a> {
    view: &'a AppView,
}

#[derive(Template)]
#[template(path = "elements/sidebar.html")]
struct SidebarTemplate<'a> {
    view: &'a AppView,
}

#[derive(Template)]
#[template(path = "elements/queue-sidebar.html")]
struct QueueSidebarTemplate<'a> {
    view: &'a AppView,
}

#[derive(Template)]
#[template(path = "elements/image-viewer.html")]
struct ImageViewerTemplate {}

#[derive(Template)]
#[template(path = "elements/image-viewer-empty.html")]
struct ImageViewerEmptyTemplate {}

#[derive(Template)]
#[template(path = "elements/modal/confirm.html")]
struct ConfirmModalTemplate {}

#[derive(Template)]
#[template(path = "elements/modal/apply-confirm.html")]
struct ApplyConfirmModalTemplate {}

#[derive(Template)]
#[template(path = "elements/modal/queue-not-empty.html")]
struct QueueNotEmptyModalTemplate {}

#[derive(Template)]
#[template(path = "elements/modal/directory-delete-confirm.html")]
struct DirectoryDeleteConfirmModalTemplate {}

#[derive(Template)]
#[template(path = "elements/modal/help.html")]
struct HelpModalTemplate {}

#[derive(Template)]
#[template(path = "elements/modal/result.html")]
struct ResultModalTemplate<'a> {
    view: &'a AppView,
}

#[derive(Template)]
#[template(path = "elements/image-card.html")]
struct ImageCardTemplate<'a> {
    card: &'a StackCard,
}

fn render_image_card(card: &StackCard) -> String {
    ImageCardTemplate { card }.render().unwrap_or_default()
}

fn render_active_modal(view: &AppView) -> String {
    match view.active_modal {
        Some(ModalView::Queue) => "<modal-none id=\"modal\"></modal-none>".to_string(),
        Some(ModalView::ApplyConfirm) => ApplyConfirmModalTemplate {}.render().unwrap_or_default(),
        Some(ModalView::DeleteConfirm) => ConfirmModalTemplate {}.render().unwrap_or_default(),
        Some(ModalView::QueueNotEmptyConfirm) => {
            QueueNotEmptyModalTemplate {}.render().unwrap_or_default()
        }
        Some(ModalView::DirectoryDeleteConfirm) => {
            DirectoryDeleteConfirmModalTemplate {}
                .render()
                .unwrap_or_default()
        }
        Some(ModalView::Help) => HelpModalTemplate {}.render().unwrap_or_default(),
        Some(ModalView::ApplyResult) => ResultModalTemplate { view }.render().unwrap_or_default(),
        None => "<modal-none id=\"modal\"></modal-none>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppConfig, AppContext};
    use crate::domain::{
        ActionConfig, ActionMapping, AppState, DecisionSide, DecisionState, ImageMeta, ModalView,
        SortDirection, SortKey, SortMode,
    };
    use crate::domain::undo::UndoEntry;
    use axum::body::Body;
    use axum::http::Request;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use std::path::PathBuf;
    use tokio::time::{Duration, timeout};
    use tower::util::ServiceExt;

    fn sample_image(id: u64, path: &str, original_order: usize) -> ImageEntry {
        ImageEntry {
            id,
            path: PathBuf::from(path),
            original_order,
            decision: DecisionState::Undecided,
            queued_action: None,
            rename_sequence: None,
            meta: ImageMeta {
                created: None,
                modified: None,
                size: 0,
                orientation: None,
            },
        }
    }

    fn sample_state() -> crate::domain::state::AppStateInner {
        let mapping = ActionMapping {
            left: ActionConfig::Delete,
            right: ActionConfig::Keep,
        };
        let sort_mode = SortMode {
            key: SortKey::Filesystem,
            direction: SortDirection::Asc,
        };
        let mut state = crate::domain::state::AppStateInner::new(true, mapping, sort_mode);
        state.set_images(
            vec![
                sample_image(1, "/tmp/a.jpg", 0),
                sample_image(2, "/tmp/b.jpg", 1),
                sample_image(3, "/tmp/c.jpg", 2),
                sample_image(4, "/tmp/d.jpg", 3),
                sample_image(5, "/tmp/e.jpg", 4),
            ],
            Some(PathBuf::from("/tmp")),
        );
        state
    }

    fn sample_machine(queue_mode: bool) -> AppState {
        let mapping = ActionMapping {
            left: ActionConfig::Delete,
            right: ActionConfig::Keep,
        };
        let sort_mode = SortMode {
            key: SortKey::Filesystem,
            direction: SortDirection::Asc,
        };
        let mut machine = AppState::new(queue_mode, mapping, sort_mode);
        machine.transition_to_scanning();
        machine.transition_to_ready(
            vec![sample_image(1, "/tmp/a.jpg", 0)],
            Some(PathBuf::from("/tmp")),
        );
        machine.transition_to_viewing();
        machine
    }

    #[test]
    fn preload_window_is_bounded_and_ordered() {
        let mut state = sample_state();
        state.cursor = 2;
        let window = preload_window_paths(&state, 1);
        let labels: Vec<String> = window
            .iter()
            .filter_map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .collect();
        assert_eq!(labels, vec!["b.jpg", "c.jpg", "d.jpg"]);
    }

    #[test]
    fn image_card_render_contains_entry_identity_and_alignment() {
        let mut state = sample_state();
        state.images[1].decision = DecisionState::Decided {
            side: DecisionSide::Right,
            action: ActionConfig::Keep,
        };
        state.cursor = 1;
        let cards = build_stack_cards_in_range(&state, 0, 2);
        let card = cards.iter().find(|card| card.image_id == 2).unwrap();
        let html = render_image_card(card);
        assert!(html.contains("id=\"image-card-2\""));
        assert!(html.contains("class=\"align-right\""));
        assert!(html.contains("src=\"/image/by-path/b.jpg\""));
    }

    #[test]
    fn image_card_markup_is_identical_for_hydration_and_single_entry_paths() {
        let mut state = sample_state();
        state.images[2].decision = DecisionState::Decided {
            side: DecisionSide::Left,
            action: ActionConfig::Delete,
        };
        state.cursor = 2;
        let cards = build_stack_cards_in_range(&state, 1, 3);

        let hydrated_html = cards
            .iter()
            .map(render_image_card)
            .collect::<Vec<_>>()
            .join("");
        let single_entry_card = cards.iter().find(|card| card.image_id == 3).unwrap();
        let single_entry_html = render_image_card(single_entry_card);

        assert!(hydrated_html.contains(&single_entry_html));
    }

    #[test]
    fn undo_selects_the_image_that_was_undone() {
        let mut state = sample_state();
        state.select_image_by_id(2);
        let outcome = state.apply_decision(DecisionSide::Right).unwrap();
        state.record_undo(UndoEntry {
            image_id: outcome.image_id,
            previous_decision: outcome.previous_decision,
            previous_queue: outcome.previous_queue,
            previous_cursor: outcome.cursor_before,
            undo_action: None,
        });

        state.undo_last().unwrap();

        assert_eq!(state.selected_entry_id, Some(2));
        assert_eq!(state.current().map(|image| image.id), Some(2));
    }

    #[tokio::test]
    async fn startup_without_path_renders_sidebar_first_shell_without_modal() {
        let mapping = ActionMapping {
            left: ActionConfig::Delete,
            right: ActionConfig::Keep,
        };
        let sort_mode = SortMode {
            key: SortKey::Filesystem,
            direction: SortDirection::Asc,
        };
        let state = AppState::new(true, mapping, sort_mode);
        let ctx = AppContext::new(state, AppConfig::default());

        let axum::response::Html(html) = render_full_page(&ctx).await;
        assert!(html.contains("sidebar-panel"));
        assert!(html.contains("No supported images in this directory."));
        assert!(html.contains("<modal-none id=\"modal\"></modal-none>"));
    }

    #[tokio::test]
    async fn apply_shows_delete_confirmation_when_destructive_delete_enabled() {
        let mut state = sample_machine(true);
        state.state_mut().images[0].queued_action = Some(ActionConfig::Delete);

        let config = AppConfig {
            destructive_delete: true,
            ..AppConfig::default()
        };
        let ctx = AppContext::new(state, config);

        let response = cmd_apply(State(WebState { ctx: ctx.clone() }))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let guard = ctx.state.read().await;
        assert_eq!(guard.state().active_modal, Some(ModalView::DeleteConfirm));
    }

    #[tokio::test]
    async fn lagged_stream_recovery_builds_full_resync_events() {
        let ctx = AppContext::new(sample_machine(true), AppConfig::default());

        let recovery = stream_recovery_events(&ctx, RecvError::Lagged(2))
            .await
            .expect("lagged streams must resync");
        let full = build_full_resync_events(&ctx).await;

        assert!(!recovery.is_empty());
        assert_eq!(recovery.len(), full.len());
    }

    #[tokio::test]
    async fn rendered_shortcuts_use_simple_posts_for_shortcuts_and_buttons() {
        let ctx = AppContext::new(sample_machine(true), AppConfig::default());
        let axum::response::Html(html) = render_full_page(&ctx).await;

        assert!(html.contains("@get('/events', { openWhenHidden: true })"));
        assert!(html.contains("@post('/cmd/left')"));
        assert!(html.contains("@post('/cmd/next')"));
        assert!(html.contains("@post('/cmd/apply/request')"));
        assert!(html.contains("<button data-on:click=\"@post('/cmd/left')\">⬅️ Left</button>"));
        assert!(html.contains("window.piclrInitQueueSidebarList"));
        assert!(html.contains("outsideView"));
    }

    #[tokio::test]
    async fn apply_request_shows_apply_confirmation_modal() {
        let ctx = AppContext::new(sample_machine(true), AppConfig::default());
        let response = cmd_apply_request(State(WebState { ctx: ctx.clone() }))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let guard = ctx.state.read().await;
        assert_eq!(guard.state().active_modal, Some(ModalView::ApplyConfirm));
    }

    #[tokio::test]
    async fn queue_toggle_uses_sidebar_not_modal() {
        let ctx = AppContext::new(sample_machine(true), AppConfig::default());
        let _ = cmd_show_queue(State(WebState { ctx: ctx.clone() })).await;

        let axum::response::Html(html) = render_full_page(&ctx).await;
        assert!(html.contains("id=\"queue-sidebar\""));
        assert!(!html.contains("<modal-queue"));
    }

    #[tokio::test]
    async fn selected_queue_row_shows_apply_and_undo_icons() {
        let mapping = ActionMapping {
            left: ActionConfig::Delete,
            right: ActionConfig::Keep,
        };
        let sort_mode = SortMode {
            key: SortKey::Filesystem,
            direction: SortDirection::Asc,
        };
        let mut machine = AppState::new(true, mapping, sort_mode);
        machine.transition_to_scanning();
        machine.transition_to_ready(
            vec![
                sample_image(1, "/tmp/a.jpg", 0),
                sample_image(2, "/tmp/b.jpg", 1),
            ],
            Some(PathBuf::from("/tmp")),
        );
        machine.transition_to_viewing();
        machine.state_mut().apply_decision(DecisionSide::Left);
        machine.state_mut().apply_decision(DecisionSide::Right);
        machine.state_mut().toggle_queue_sidebar();
        machine.state_mut().select_queue_last();

        let ctx = AppContext::new(machine, AppConfig::default());
        let axum::response::Html(html) = render_full_page(&ctx).await;
        assert!(html.contains("/cmd/queue/apply-selected"));
        assert!(html.contains("/cmd/queue/remove-selected"));
    }

    #[tokio::test]
    async fn applying_selected_queue_item_preserves_remaining_queue() {
        let mapping = ActionMapping {
            left: ActionConfig::Delete,
            right: ActionConfig::Keep,
        };
        let sort_mode = SortMode {
            key: SortKey::Filesystem,
            direction: SortDirection::Asc,
        };
        let mut machine = AppState::new(true, mapping, sort_mode);
        machine.transition_to_scanning();
        machine.transition_to_ready(
            vec![
                sample_image(1, "/tmp/a.jpg", 0),
                sample_image(2, "/tmp/b.jpg", 1),
                sample_image(3, "/tmp/c.jpg", 2),
            ],
            Some(PathBuf::from("/tmp")),
        );
        machine.transition_to_viewing();

        machine.state_mut().apply_decision(DecisionSide::Right);
        machine.state_mut().apply_decision(DecisionSide::Right);
        machine.state_mut().apply_decision(DecisionSide::Right);
        machine.state_mut().toggle_queue_sidebar();
        machine.state_mut().select_queue_first();

        let ctx = AppContext::new(machine, AppConfig::default());
        let response = cmd_queue_apply_selected(State(WebState { ctx: ctx.clone() }))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let guard = ctx.state.read().await;
        assert_eq!(guard.state().projection().queue_count, 2);
    }

    #[tokio::test]
    async fn applying_selected_delete_does_not_leave_stale_nav_file_entry() {
        let mapping = ActionMapping {
            left: ActionConfig::Delete,
            right: ActionConfig::Keep,
        };
        let sort_mode = SortMode {
            key: SortKey::Filesystem,
            direction: SortDirection::Asc,
        };
        let mut machine = AppState::new(true, mapping, sort_mode);
        machine.transition_to_scanning();
        machine.transition_to_ready(
            vec![
                sample_image(1, "/tmp/a.jpg", 0),
                sample_image(2, "/tmp/b.jpg", 1),
            ],
            Some(PathBuf::from("/tmp")),
        );
        machine.transition_to_viewing();

        machine.state_mut().apply_decision(DecisionSide::Left);
        machine.state_mut().apply_decision(DecisionSide::Right);
        machine.state_mut().toggle_queue_sidebar();
        machine.state_mut().select_queue_first();

        let ctx = AppContext::new(machine, AppConfig::default());
        let response = cmd_queue_apply_selected(State(WebState { ctx: ctx.clone() }))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // Regression: build/render should not panic due to stale nav->image references.
        let _ = build_view(&ctx).await;
    }

    #[tokio::test]
    async fn undo_still_uses_stack_when_queue_is_selected() {
        let mapping = ActionMapping {
            left: ActionConfig::Delete,
            right: ActionConfig::Keep,
        };
        let sort_mode = SortMode {
            key: SortKey::Filesystem,
            direction: SortDirection::Asc,
        };
        let mut machine = AppState::new(true, mapping, sort_mode);
        machine.transition_to_scanning();
        machine.transition_to_ready(
            vec![
                sample_image(1, "/tmp/a.jpg", 0),
                sample_image(2, "/tmp/b.jpg", 1),
            ],
            Some(PathBuf::from("/tmp")),
        );
        machine.transition_to_viewing();

        let first_outcome = machine.state_mut().apply_decision(DecisionSide::Left).unwrap();
        machine.state_mut().record_undo(UndoEntry {
            image_id: first_outcome.image_id,
            previous_decision: first_outcome.previous_decision,
            previous_queue: first_outcome.previous_queue,
            previous_cursor: first_outcome.cursor_before,
            undo_action: None,
        });
        machine.state_mut().apply_decision(DecisionSide::Right);
        machine.state_mut().toggle_queue_sidebar();
        machine.state_mut().select_queue_last();

        let ctx = AppContext::new(machine, AppConfig::default());
        let _ = cmd_undo(State(WebState { ctx: ctx.clone() })).await;
        let guard = ctx.state.read().await;
        assert_eq!(guard.state().projection().queue_count, 1);
        assert_eq!(guard.state().undo_stack.len(), 0);
    }

    #[tokio::test]
    async fn events_endpoint_accepts_get_and_rejects_patch() {
        let ctx = AppContext::new(sample_machine(true), AppConfig::default());
        let app = router(ctx);

        let get_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get_response.status(), StatusCode::OK);

        let patch_response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(patch_response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[cfg(not(feature = "tauri"))]
    #[tokio::test]
    async fn root_select_endpoint_is_forbidden_without_tauri() {
        let ctx = AppContext::new(sample_machine(true), AppConfig::default());
        let app = router(ctx);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/cmd/sidebar/root/select")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"path":"/tmp"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn apply_decision_releases_state_lock_before_immediate_fs_work() {
        let ctx = AppContext::new(sample_machine(false), AppConfig::default());
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        set_apply_decision_test_barrier(Some(barrier.clone())).await;

        let task_ctx = ctx.clone();
        let apply_task = tokio::spawn(async move {
            apply_decision(task_ctx, DecisionSide::Left).await;
        });

        barrier.wait().await;
        let lock_result = timeout(Duration::from_millis(250), ctx.state.write()).await;
        assert!(lock_result.is_ok(), "state write lock should be available");
        drop(lock_result.unwrap());

        set_apply_decision_test_barrier(None).await;
        let _ = apply_task.await;
    }
}
