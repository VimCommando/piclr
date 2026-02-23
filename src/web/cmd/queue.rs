use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use tracing::warn;

use crate::domain::{ActionConfig, ModalView};
use crate::fs::{FsConfig, apply_action};

use super::super::{
    UiPatch, WebState, apply_queue, broadcast_navigation, broadcast_patch,
    broadcast_viewer_and_signals, finalize_apply_result,
};

pub(crate) async fn apply(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn apply_request(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    app_state.close_directory_actions();
    app_state.show_view(ModalView::ApplyConfirm);
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

pub(crate) async fn apply_confirm(State(state): State<WebState>) -> impl IntoResponse {
    {
        let mut guard = state.ctx.state.write().await;
        guard.state_mut().hide_view(ModalView::DeleteConfirm);
    }
    let summary = apply_queue(&state.ctx).await;
    finalize_apply_result(&state.ctx, summary).await;
    StatusCode::NO_CONTENT
}

pub(crate) async fn reset(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().reset_queue_state();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::RESET_QUEUE).await;
    StatusCode::NO_CONTENT
}

pub(crate) async fn prev(State(state): State<WebState>) -> impl IntoResponse {
    move_selection(&state, false).await
}

pub(crate) async fn next(State(state): State<WebState>) -> impl IntoResponse {
    move_selection(&state, true).await
}

pub(crate) async fn select(
    State(state): State<WebState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    let selected = activate_item_selection(app_state, id);
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    if selected {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

pub(crate) async fn apply_selected(State(state): State<WebState>) -> impl IntoResponse {
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
    if let Err(err) = apply_action(&config, &path, &action, rename_sequence).await {
        warn!(%err, path = %path.display(), "Failed to apply selected queued action");
        return StatusCode::NO_CONTENT;
    }

    {
        let mut guard = state.ctx.state.write().await;
        let app_state = guard.state_mut();
        app_state.apply_selected_queue_action_result(image_id, &action);
    }
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::ALL).await;
    StatusCode::NO_CONTENT
}

pub(crate) async fn remove_selected(State(state): State<WebState>) -> impl IntoResponse {
    let removed = {
        let mut guard = state.ctx.state.write().await;
        guard.state_mut().remove_selected_queue_item()
    };
    let Some(image_id) = removed else {
        return StatusCode::NO_CONTENT;
    };
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    super::super::patch_stack_card_if_visible(&ctx, image_id).await;
    StatusCode::NO_CONTENT
}

pub(crate) async fn show(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    app_state.close_directory_actions();
    app_state.toggle_queue_sidebar();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_viewer_and_signals(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn move_selection(state: &WebState, forward: bool) -> StatusCode {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    let moved = if forward {
        app_state.select_queue_next()
    } else {
        app_state.select_queue_prev()
    };
    if moved {
        if let Some(selected) = app_state.selected_queue_image_id {
            activate_item_selection(app_state, selected);
        }
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_navigation(&ctx).await;
    StatusCode::NO_CONTENT
}

fn activate_item_selection(
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
