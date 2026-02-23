use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use tracing::warn;

use crate::domain::{DecisionSide, ModalView};
use crate::fs::apply_undo_action;

use super::{
    UiPatch, WebState, apply_decision, attempt_directory_change, broadcast_navigation,
    broadcast_patch, broadcast_viewer_and_signals, patch_stack_card_if_visible,
};

pub mod files;
pub mod image;
pub mod queue;

pub(crate) async fn left(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn right(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn next(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn prev(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn jump_next(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().jump_next_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    StatusCode::NO_CONTENT
}

pub(crate) async fn jump_prev(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().jump_prev_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    StatusCode::NO_CONTENT
}

pub(crate) async fn home(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn end(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn undo(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn help(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn select(
    State(state): State<WebState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
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

pub(crate) async fn close(State(state): State<WebState>) -> impl IntoResponse {
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
