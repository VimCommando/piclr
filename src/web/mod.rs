use askama::Template;
use asynk_strim::{Yielder, stream_fn};
use axum::Router;
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
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::warn;

use crate::app::AppContext;
use crate::domain::{ActionConfig, DecisionSide, ImageEntry, ModalView};
use crate::fs::{
    FsConfig, apply_action, apply_action_with_undo, apply_undo_action, load_image_bytes,
    scan_images,
};

#[derive(Clone)]
pub struct WebState {
    pub ctx: AppContext,
}

#[derive(Clone, Copy)]
struct UiPatch {
    header: bool,
    viewer: bool,
    modals: bool,
    signals: bool,
}

impl UiPatch {
    const ALL: Self = Self {
        header: true,
        viewer: true,
        modals: true,
        signals: true,
    };

    const VIEWER_AND_SIGNALS: Self = Self {
        header: false,
        viewer: true,
        modals: false,
        signals: true,
    };

    const MODALS_ONLY: Self = Self {
        header: false,
        viewer: false,
        modals: true,
        signals: false,
    };

    const VIEWER_MODALS_AND_SIGNALS: Self = Self {
        header: false,
        viewer: true,
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
        .route("/cmd/next", post(cmd_next))
        .route("/cmd/prev", post(cmd_prev))
        .route("/cmd/jump-next", post(cmd_jump_next))
        .route("/cmd/jump-prev", post(cmd_jump_prev))
        .route("/cmd/undo", post(cmd_undo))
        .route("/cmd/apply", post(cmd_apply))
        .route("/cmd/apply-confirm", post(cmd_apply_confirm))
        .route("/cmd/update-directory", patch(cmd_update_directory))
        .route("/cmd/show-queue", post(cmd_show_queue))
        .route("/cmd/show-files", post(cmd_show_files))
        .route("/cmd/help", post(cmd_help))
        .route("/cmd/select/{id}", post(cmd_select))
        .route("/cmd/close", post(cmd_close))
        .route("/events", patch(events))
        .route("/image/{id}", get(image))
        .route("/assets/datastar.js", get(datastar_js))
        .route("/assets/app.css", get(app_css))
        .with_state(state)
}

async fn index(State(state): State<WebState>) -> Html<String> {
    let ctx = state.ctx.clone();
    render_full_page(&ctx).await
}

async fn cmd_left(State(state): State<WebState>) -> impl IntoResponse {
    let ctx = state.ctx.clone();
    apply_decision(ctx, DecisionSide::Left).await;
    StatusCode::NO_CONTENT
}

async fn cmd_right(State(state): State<WebState>) -> impl IntoResponse {
    let ctx = state.ctx.clone();
    apply_decision(ctx, DecisionSide::Right).await;
    StatusCode::NO_CONTENT
}

async fn cmd_next(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().next();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
    StatusCode::NO_CONTENT
}

async fn cmd_prev(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().prev();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
    StatusCode::NO_CONTENT
}

async fn cmd_jump_next(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().jump_next_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
    StatusCode::NO_CONTENT
}

async fn cmd_jump_prev(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().jump_prev_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
    StatusCode::NO_CONTENT
}

async fn cmd_undo(State(state): State<WebState>) -> impl IntoResponse {
    let undo_action = {
        let mut guard = state.ctx.state.write().await;
        guard
            .inner_mut()
            .undo_last()
            .and_then(|entry| entry.undo_action)
    };

    if let Some(action) = undo_action {
        if let Err(err) = apply_undo_action(&action).await {
            warn!(%err, "Failed to undo action");
        }
    }

    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
    StatusCode::NO_CONTENT
}

async fn cmd_apply(State(state): State<WebState>) -> impl IntoResponse {
    {
        let mut guard = state.ctx.state.write().await;
        guard.inner_mut().hide_view(ModalView::Queue);
    }

    let needs_confirm = {
        let guard = state.ctx.state.read().await;
        let has_delete = guard
            .inner()
            .images
            .iter()
            .any(|image| matches!(image.queued_action, Some(ActionConfig::Delete)));
        state.ctx.config.destructive_delete && has_delete
    };

    if needs_confirm {
        let mut guard = state.ctx.state.write().await;
        guard.inner_mut().show_view(ModalView::DeleteConfirm);
        drop(guard);
        let ctx = state.ctx.clone();
        broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
        return StatusCode::NO_CONTENT;
    }

    let summary = apply_queue(&state.ctx).await;
    refresh_images_after_apply(&state.ctx).await;
    {
        let mut guard = state.ctx.state.write().await;
        let inner = guard.inner_mut();
        inner.last_apply_result = Some(crate::domain::state::ApplyResultSummary {
            completed: summary.completed,
            total: summary.total,
            failed: summary.failed,
            errors: summary.errors,
        });
        inner.show_view(ModalView::ApplyResult);
    }
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::VIEWER_MODALS_AND_SIGNALS).await;
    StatusCode::NO_CONTENT
}

async fn cmd_apply_confirm(State(state): State<WebState>) -> impl IntoResponse {
    {
        let mut guard = state.ctx.state.write().await;
        guard.inner_mut().hide_view(ModalView::DeleteConfirm);
        guard.inner_mut().hide_view(ModalView::Queue);
    }
    let summary = apply_queue(&state.ctx).await;
    refresh_images_after_apply(&state.ctx).await;
    {
        let mut guard = state.ctx.state.write().await;
        let inner = guard.inner_mut();
        inner.last_apply_result = Some(crate::domain::state::ApplyResultSummary {
            completed: summary.completed,
            total: summary.total,
            failed: summary.failed,
            errors: summary.errors,
        });
        inner.show_view(ModalView::ApplyResult);
    }
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::VIEWER_MODALS_AND_SIGNALS).await;
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
            guard.inner().root_dir.clone(),
            ctx.config.destructive_delete,
        )
    };

    let Some(root_dir) = root_dir else {
        return ApplySummary::default();
    };

    let config = FsConfig::new(root_dir, destructive);
    let mut guard = ctx.state.write().await;
    let images = guard.inner_mut().images.clone();
    let mut summary = ApplySummary::default();
    for image in images {
        if let Some(action) = image.queued_action {
            summary.total += 1;
            if let Err(err) =
                apply_action(&config, &image.path, &action, image.rename_sequence).await
            {
                summary.failed += 1;
                summary
                    .errors
                    .push(format!("{}: {}", image.path.display(), err));
                warn!(%err, path = %image.path.display(), "Failed to apply queued action");
            } else {
                summary.completed += 1;
            }
        }
    }
    guard
        .inner_mut()
        .images
        .iter_mut()
        .for_each(|image| image.queued_action = None);
    drop(guard);
    summary
}

async fn refresh_images_after_apply(ctx: &AppContext) {
    let root_dir = {
        let guard = ctx.state.read().await;
        guard.inner().root_dir.clone()
    };
    if let Some(path) = root_dir {
        run_scan(ctx, path).await;
    }
}

#[derive(Deserialize)]
struct UpdateDirectorySignals {
    #[serde(alias = "directoryPath")]
    directory_path: Option<String>,
}

async fn cmd_update_directory(
    State(state): State<WebState>,
    ReadSignals(signals): ReadSignals<UpdateDirectorySignals>,
) -> impl IntoResponse {
    let path = signals
        .directory_path
        .and_then(|path| {
            let trimmed = path.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(PathBuf::from(trimmed))
            }
        })
        .or_else(|| state.ctx.config.initial_path.clone());

    let Some(path) = path else {
        return StatusCode::NO_CONTENT;
    };

    run_scan(&state.ctx, path).await;
    {
        let mut guard = state.ctx.state.write().await;
        guard.inner_mut().view_stack.clear();
    }
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::VIEWER_MODALS_AND_SIGNALS).await;
    StatusCode::NO_CONTENT
}

async fn cmd_show_queue(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let inner = guard.inner_mut();
    if inner.view_stack.last().copied() == Some(ModalView::Queue) {
        inner.close_view();
    } else {
        inner.show_view(ModalView::Queue);
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

async fn cmd_show_files(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let inner = guard.inner_mut();
    if inner.view_stack.last().copied() == Some(ModalView::Files) {
        inner.close_view();
    } else {
        inner.show_view(ModalView::Files);
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

async fn cmd_help(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let inner = guard.inner_mut();
    if inner.view_stack.last().copied() == Some(ModalView::Help) {
        inner.close_view();
    } else {
        inner.show_view(ModalView::Help);
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

async fn cmd_select(State(state): State<WebState>, Path(id): Path<u64>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let selected = guard.inner_mut().select_image_by_id(id);
    if selected {
        guard.inner_mut().hide_view(ModalView::Queue);
        guard.inner_mut().hide_view(ModalView::Files);
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

async fn cmd_close(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().close_view();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

async fn apply_decision(ctx: AppContext, side: DecisionSide) {
    let (root_dir, destructive) = {
        let guard = ctx.state.read().await;
        (
            guard.inner().root_dir.clone(),
            ctx.config.destructive_delete,
        )
    };

    let mut guard = ctx.state.write().await;
    let outcome = guard.inner_mut().apply_decision(side);
    let immediate = outcome.as_ref().map(|o| o.immediate).unwrap_or(false);
    let mut undo_entry = outcome
        .as_ref()
        .map(|outcome| crate::domain::undo::UndoEntry {
            image_id: outcome.image_id,
            previous_decision: outcome.previous_decision.clone(),
            previous_queue: outcome.previous_queue.clone(),
            previous_cursor: outcome.cursor_before,
            undo_action: None,
        });

    if immediate {
        if let (Some(root_dir), Some(outcome), Some(undo)) =
            (root_dir, outcome.as_ref(), undo_entry.as_mut())
        {
            let config = FsConfig::new(root_dir, destructive);
            let image = guard
                .inner()
                .images
                .iter()
                .find(|image| image.id == outcome.image_id)
                .cloned();
            if let Some(image) = image {
                match apply_action_with_undo(
                    &config,
                    &image.path,
                    &outcome.action,
                    image.rename_sequence,
                )
                .await
                {
                    Ok(action) => {
                        undo.undo_action = action;
                    }
                    Err(err) => {
                        warn!(%err, path = %image.path.display(), "Failed to apply action");
                    }
                }
            }
        }
    }

    if let Some(undo) = undo_entry {
        guard.inner_mut().record_undo(undo);
    }

    drop(guard);
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
}

// undo helpers are handled in fs::apply_action_with_undo

async fn run_scan(ctx: &AppContext, path: PathBuf) {
    let images = scan_images(&path).await;

    let mut state = ctx.state.write().await;
    state.inner_mut().set_images(images, Some(path));
}

async fn image(State(state): State<WebState>, Path(id): Path<u64>) -> Response {
    let image_path = {
        let guard = state.ctx.state.read().await;
        guard
            .inner()
            .images
            .iter()
            .find(|image| image.id == id)
            .map(|image| image.path.clone())
    };

    let Some(path) = image_path else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if let Some(bytes) = state.ctx.cache.read().await.get(&path).cloned() {
        return bytes_response(&path, bytes);
    }

    let bytes = match load_image_bytes(&path).await {
        Ok(bytes) => Bytes::from(bytes),
        Err(err) => {
            warn!(%err, path = %path.display(), "Failed to read image");
            return StatusCode::NOT_FOUND.into_response();
        }
    };

    state
        .ctx
        .cache
        .write()
        .await
        .insert(path.clone(), bytes.clone());
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

fn bytes_response(path: &PathBuf, bytes: Bytes) -> Response {
    let mime = MimeGuess::from_path(path).first_or_octet_stream();
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, max-age=0, must-revalidate"),
    );
    headers.insert(
        axum::http::header::PRAGMA,
        HeaderValue::from_static("no-cache"),
    );
    headers.insert(axum::http::header::EXPIRES, HeaderValue::from_static("0"));
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
            let initial_events = build_patch_events(&ctx, UiPatch::ALL).await;
            for event in initial_events {
                yielder.yield_item(Ok(event)).await;
            }

            loop {
                match rx.recv().await {
                    Ok(event) => {
                        yielder.yield_item(Ok(event)).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
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

async fn build_patch_events(ctx: &AppContext, patch: UiPatch) -> Vec<Event> {
    let view = build_view(ctx).await;
    let mut events = Vec::new();

    if patch.header {
        let html = ActionHeaderTemplate { view: &view }
            .render()
            .unwrap_or_default();
        let patch = PatchElements::new(html)
            .selector("#action-header")
            .mode(ElementPatchMode::Outer);
        events.push(patch.write_as_axum_sse_event());
    }

    if patch.viewer {
        let html = ViewerRegionTemplate { view: &view }
            .render()
            .unwrap_or_default();
        let patch = PatchElements::new(html)
            .selector("#viewer-region")
            .mode(ElementPatchMode::Outer);
        events.push(patch.write_as_axum_sse_event());
    }

    if patch.modals {
        let html = ModalRegionTemplate { view: &view }
            .render()
            .unwrap_or_default();
        let patch = PatchElements::new(html)
            .selector("#modal-region")
            .mode(ElementPatchMode::Outer);
        events.push(patch.write_as_axum_sse_event());
    }

    if patch.signals {
        let patch = PatchSignals::new(counter_signals_json(&view));
        events.push(patch.write_as_axum_sse_event());
    }

    events
}

fn counter_signals_json(view: &AppView) -> String {
    serde_json::json!({
        "counterLeftAction": view.left_action_count,
        "counterRightAction": view.right_action_count,
        "counterImageIndex": if view.total > 0 { view.index } else { 0 },
        "counterImageTotal": view.total,
        "counterQueueCount": view.queue_count,
        "currentPath": view.current_path_label,
        "directorySelectEnabled": view.directory_select_enabled
    })
    .to_string()
}

async fn navigation_patch(ctx: &AppContext) -> UiPatch {
    let guard = ctx.state.read().await;
    let inner = guard.inner();
    if inner.has_view(ModalView::Queue) || inner.has_view(ModalView::Files) {
        UiPatch::VIEWER_MODALS_AND_SIGNALS
    } else {
        UiPatch::VIEWER_AND_SIGNALS
    }
}

async fn build_view(ctx: &AppContext) -> AppView {
    let guard = ctx.state.read().await;
    let inner = guard.inner();
    let total = inner.order.len();
    let index = inner.cursor + 1;
    let current = inner.current();
    let window_paths = preload_window_paths(inner, 5);
    let cache = ctx.cache.clone();
    tokio::spawn(async move {
        maintain_cache_window(cache, window_paths).await;
    });
    let mut queue_count = 0usize;
    let mut left_action_count = 0usize;
    let mut right_action_count = 0usize;
    for image in &inner.images {
        if image.queued_action.is_some() {
            queue_count += 1;
        }
        match decision_side(image) {
            Some(DecisionSide::Left) => left_action_count += 1,
            Some(DecisionSide::Right) => right_action_count += 1,
            None => {}
        }
    }
    let file_count = inner.images.len();
    let show_queue_modal = inner.has_view(ModalView::Queue);
    let show_files_modal = inner.has_view(ModalView::Files);
    let show_help_modal = inner.has_view(ModalView::Help);
    let show_apply_result = inner.has_view(ModalView::ApplyResult);
    let show_delete_confirm = inner.has_view(ModalView::DeleteConfirm);
    let queue_modal_z = modal_layer(&inner.view_stack, ModalView::Queue);
    let files_modal_z = modal_layer(&inner.view_stack, ModalView::Files);
    let help_modal_z = modal_layer(&inner.view_stack, ModalView::Help);
    let apply_result_z = modal_layer(&inner.view_stack, ModalView::ApplyResult);
    let delete_modal_z = modal_layer(&inner.view_stack, ModalView::DeleteConfirm);
    let last_apply_result = inner.last_apply_result.clone();
    let current_path_label = inner
        .root_dir
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "No directory selected".to_string());
    let mut queue_items: Vec<QueueItem> = Vec::new();
    let mut queue_has_current = false;
    if show_queue_modal {
        let current_id = current.map(|e| e.id).unwrap_or(0);
        queue_items = inner
            .images
            .iter()
            .filter_map(|image| {
                let queued = image.queued_action.as_ref()?;
                if image.id == current_id {
                    queue_has_current = true;
                }
                Some(queue_item_from_action(
                    image,
                    queued,
                    decision_side(image),
                    inner.root_dir.as_ref(),
                ))
            })
            .collect();
    }
    let mut queue_insert_before_id: Option<u64> = None;
    let mut queue_insert_at_end = false;
    if show_queue_modal && !queue_items.is_empty() && !queue_has_current {
        if let Some(current_id) = current.map(|entry| entry.id) {
            let current_pos = inner.images.iter().position(|image| image.id == current_id);
            if let Some(current_pos) = current_pos {
                if let Some(before_id) = inner
                    .images
                    .iter()
                    .enumerate()
                    .filter(|(_, image)| image.queued_action.is_some())
                    .find(|(idx, _)| *idx > current_pos)
                    .map(|(_, image)| image.id)
                {
                    queue_insert_before_id = Some(before_id);
                } else {
                    queue_insert_at_end = true;
                }
            }
        }
    }
    if let Some(before_id) = queue_insert_before_id {
        queue_items
            .iter_mut()
            .filter(|item| item.image_id == before_id)
            .for_each(|item| item.is_insert_before = true);
    }
    let file_items = if show_files_modal {
        inner
            .images
            .iter()
            .map(|image| match &image.decision {
                crate::domain::DecisionState::Decided { side, action } => {
                    queue_item_from_action(image, action, Some(*side), inner.root_dir.as_ref())
                }
                crate::domain::DecisionState::Undecided => {
                    queue_item_none(image, inner.root_dir.as_ref())
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let stack_cards = build_stack_cards(inner, 5);
    let nav_direction = match inner.nav_direction {
        Some(crate::domain::state::NavDirection::Up) => "up".to_string(),
        Some(crate::domain::state::NavDirection::Down) => "down".to_string(),
        None => "none".to_string(),
    };
    AppView {
        has_images: current.is_some(),
        directory_select_enabled: cfg!(feature = "tauri"),
        stack_cards,
        nav_direction,
        nav_tick: inner.nav_tick,
        current_image_id: current.map(|entry| entry.id).unwrap_or(0),
        current_path_label,
        left_action_label: action_config_label(&inner.action_mapping.left),
        right_action_label: action_config_label(&inner.action_mapping.right),
        left_action_count,
        right_action_count,
        index,
        total,
        queue_count,
        file_count,
        queue_mode: inner.queue_mode,
        show_delete_confirm,
        show_queue_modal,
        show_files_modal,
        show_help_modal,
        show_apply_result,
        delete_modal_z,
        queue_modal_z,
        files_modal_z,
        help_modal_z,
        apply_result_z,
        apply_completed: last_apply_result.as_ref().map(|r| r.completed).unwrap_or(0),
        apply_total: last_apply_result.as_ref().map(|r| r.total).unwrap_or(0),
        apply_failed: last_apply_result.as_ref().map(|r| r.failed).unwrap_or(0),
        apply_errors: last_apply_result.map(|r| r.errors).unwrap_or_default(),
        queue_items,
        file_items,
        queue_insert_before_id,
        queue_insert_at_end,
    }
}

#[derive(Clone, Debug)]
pub struct AppView {
    pub has_images: bool,
    pub directory_select_enabled: bool,
    pub stack_cards: Vec<StackCard>,
    pub nav_direction: String,
    pub nav_tick: u64,
    pub current_image_id: u64,
    pub current_path_label: String,
    pub left_action_label: String,
    pub right_action_label: String,
    pub left_action_count: usize,
    pub right_action_count: usize,
    pub index: usize,
    pub total: usize,
    pub queue_count: usize,
    pub file_count: usize,
    pub queue_mode: bool,
    pub show_delete_confirm: bool,
    pub show_queue_modal: bool,
    pub show_files_modal: bool,
    pub show_help_modal: bool,
    pub show_apply_result: bool,
    pub delete_modal_z: usize,
    pub queue_modal_z: usize,
    pub files_modal_z: usize,
    pub help_modal_z: usize,
    pub apply_result_z: usize,
    pub apply_completed: usize,
    pub apply_total: usize,
    pub apply_failed: usize,
    pub apply_errors: Vec<String>,
    pub queue_items: Vec<QueueItem>,
    pub file_items: Vec<QueueItem>,
    pub queue_insert_before_id: Option<u64>,
    pub queue_insert_at_end: bool,
}

#[derive(Clone, Debug)]
pub struct QueueItem {
    pub image_id: u64,
    pub arrow: String,
    pub action_label: String,
    pub action_tone: String,
    pub file_label: String,
    pub is_insert_before: bool,
}

#[derive(Clone, Debug)]
pub struct StackCard {
    pub image_id: u64,
    pub image_src: String,
    pub alignment: String,
    pub action_item: QueueItem,
    pub top_percent: f32,
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

fn build_stack_cards(inner: &crate::domain::state::AppStateInner, radius: usize) -> Vec<StackCard> {
    if inner.order.is_empty() {
        return Vec::new();
    }
    let start = inner.cursor.saturating_sub(radius);
    let end = (inner.cursor + radius).min(inner.order.len().saturating_sub(1));
    (start..=end)
        .filter_map(|pos| {
            let idx = *inner.order.get(pos)?;
            let image = inner.images.get(idx)?;
            let offset = pos as isize - inner.cursor as isize;
            let top_percent = 16.0 + (offset as f32 * 70.0);
            let action_item = queue_item_for_image(image, inner.root_dir.as_ref());
            let cache_key = query_encode_component(&action_item.file_label);
            Some(StackCard {
                image_id: image.id,
                image_src: format!("/image/{}?k={}", image.id, cache_key),
                alignment: image_alignment_for(image),
                action_item,
                top_percent,
            })
        })
        .collect()
}

fn query_encode_component(value: &str) -> String {
    let encoded = serde_urlencoded::to_string([("k", value)]).unwrap_or_default();
    encoded
        .strip_prefix("k=")
        .unwrap_or(encoded.as_str())
        .to_string()
}

fn queue_item_from_action(
    image: &ImageEntry,
    action: &ActionConfig,
    side: Option<DecisionSide>,
    root_dir: Option<&PathBuf>,
) -> QueueItem {
    let action_tone = match side {
        Some(DecisionSide::Left) => "left".to_string(),
        Some(DecisionSide::Right) => "right".to_string(),
        None => "neutral".to_string(),
    };
    let arrow = match side {
        Some(DecisionSide::Left) => "←".to_string(),
        Some(DecisionSide::Right) => "→".to_string(),
        None => "•".to_string(),
    };
    let action_label = match action {
        ActionConfig::Keep => "Keep".to_string(),
        ActionConfig::Delete => "Delete".to_string(),
        ActionConfig::Move { target } => target.display().to_string(),
        ActionConfig::Rename { prefix } => {
            let ext = image
                .path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("");
            let seq = image.rename_sequence.unwrap_or(0);
            if ext.is_empty() {
                format!("Rename {}{:06}", prefix, seq)
            } else {
                format!("Rename {}{:06}.{}", prefix, seq, ext)
            }
        }
        ActionConfig::MetadataEdit { key, value } => format!("Metadata {}={}", key, value),
    };
    let file_label = root_dir
        .and_then(|root| image.path.strip_prefix(root).ok())
        .unwrap_or(&image.path)
        .to_string_lossy()
        .to_string();
    QueueItem {
        image_id: image.id,
        arrow,
        action_label,
        action_tone,
        file_label,
        is_insert_before: false,
    }
}

fn queue_item_none(image: &ImageEntry, root_dir: Option<&PathBuf>) -> QueueItem {
    let file_label = root_dir
        .and_then(|root| image.path.strip_prefix(root).ok())
        .unwrap_or(&image.path)
        .to_string_lossy()
        .to_string();
    QueueItem {
        image_id: image.id,
        arrow: "•".to_string(),
        action_label: "None".to_string(),
        action_tone: "neutral".to_string(),
        file_label,
        is_insert_before: false,
    }
}

fn preload_window_paths(
    inner: &crate::domain::state::AppStateInner,
    radius: usize,
) -> Vec<PathBuf> {
    if inner.order.is_empty() {
        return Vec::new();
    }
    let start = inner.cursor.saturating_sub(radius);
    let end = (inner.cursor + radius).min(inner.order.len().saturating_sub(1));
    (start..=end)
        .filter_map(|pos| {
            inner
                .order
                .get(pos)
                .and_then(|idx| inner.images.get(*idx))
                .map(|entry| entry.path.clone())
        })
        .collect()
}

async fn maintain_cache_window(
    cache: Arc<RwLock<HashMap<PathBuf, Bytes>>>,
    desired_paths: Vec<PathBuf>,
) {
    let keep: HashSet<PathBuf> = desired_paths.iter().cloned().collect();
    {
        let mut guard = cache.write().await;
        guard.retain(|path, _| keep.contains(path));
    }
    for path in desired_paths {
        let exists = {
            let guard = cache.read().await;
            guard.contains_key(&path)
        };
        if exists {
            continue;
        }
        if let Ok(bytes) = load_image_bytes(&path).await {
            cache.write().await.insert(path, Bytes::from(bytes));
        }
    }
}

fn modal_layer(stack: &[ModalView], view: ModalView) -> usize {
    let base = 1000usize;
    stack
        .iter()
        .position(|entry| *entry == view)
        .map(|idx| base + idx)
        .unwrap_or(0)
}

#[derive(Template)]
#[template(path = "index.html")]
struct MainTemplate {
    view: AppView,
}

#[derive(Template)]
#[template(path = "partials/header.html")]
struct ActionHeaderTemplate<'a> {
    view: &'a AppView,
}

#[derive(Template)]
#[template(path = "partials/viewer.html")]
struct ViewerRegionTemplate<'a> {
    view: &'a AppView,
}

#[derive(Template)]
#[template(path = "partials/modal.html")]
struct ModalRegionTemplate<'a> {
    view: &'a AppView,
}
