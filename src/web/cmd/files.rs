use std::path::PathBuf;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use datastar::axum::ReadSignals;
use serde::Deserialize;
use tracing::warn;

use crate::domain::{ModalView, SortDirection, SortKey, SortMode};

use super::super::{
    UiPatch, WebState, apply_queue, attempt_directory_change, broadcast_patch,
    broadcast_viewer_and_signals, run_scan, run_scan_with_root,
};

#[derive(Deserialize)]
pub(crate) struct RenameDirectorySignals {
    #[serde(alias = "directoryName", default)]
    directory_name: String,
}

#[derive(Deserialize)]
pub(crate) struct RootDirectorySignals {
    #[serde(default)]
    path: String,
}

pub(crate) async fn toggle(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().toggle_sidebar();
    guard.state_mut().close_directory_actions();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_viewer_and_signals(&ctx).await;
    StatusCode::NO_CONTENT
}

pub(crate) async fn root_select(
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

pub(crate) async fn open(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn sort(
    State(state): State<WebState>,
    Path(mode): Path<String>,
) -> impl IntoResponse {
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

pub(crate) async fn open_parent(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn change_directory_cancel(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn change_directory_apply(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn change_directory_clear(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn rename(
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

pub(crate) async fn delete_request(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn delete_confirm(State(state): State<WebState>) -> impl IntoResponse {
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

pub(crate) async fn open_entry(
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
