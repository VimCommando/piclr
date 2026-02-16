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
use tokio::sync::broadcast;
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
    stack: bool,
    stack_reset: bool,
    modals: bool,
    signals: bool,
}

impl UiPatch {
    const ALL: Self = Self {
        header: true,
        viewer: true,
        stack: true,
        stack_reset: true,
        modals: true,
        signals: true,
    };

    const VIEWER_AND_SIGNALS: Self = Self {
        header: false,
        viewer: false,
        stack: true,
        stack_reset: false,
        modals: false,
        signals: true,
    };

    const MODALS_ONLY: Self = Self {
        header: false,
        viewer: false,
        stack: false,
        stack_reset: false,
        modals: true,
        signals: false,
    };

    const VIEWER_MODALS_AND_SIGNALS: Self = Self {
        header: false,
        viewer: false,
        stack: true,
        stack_reset: false,
        modals: true,
        signals: true,
    };

    const RESET_QUEUE: Self = Self {
        header: false,
        viewer: false,
        stack: true,
        stack_reset: true,
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
        .route("/cmd/queue/reset", post(cmd_reset_queue))
        .route("/cmd/update-directory", patch(cmd_update_directory))
        .route("/cmd/queue/show", post(cmd_show_queue))
        .route("/cmd/files/show", post(cmd_show_files))
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
    guard.state_mut().next();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
    StatusCode::NO_CONTENT
}

async fn cmd_prev(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().prev();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
    StatusCode::NO_CONTENT
}

async fn cmd_jump_next(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().jump_next_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
    StatusCode::NO_CONTENT
}

async fn cmd_jump_prev(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.state_mut().jump_prev_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
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
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
    if undone_image_id != 0 {
        patch_stack_card_if_visible(&ctx, undone_image_id).await;
    }
    StatusCode::NO_CONTENT
}

async fn cmd_apply(State(state): State<WebState>) -> impl IntoResponse {
    {
        let mut guard = state.ctx.state.write().await;
        guard.state_mut().hide_view(ModalView::Queue);
    }

    let needs_confirm = {
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
    refresh_images_after_apply(&state.ctx).await;
    {
        let mut guard = state.ctx.state.write().await;
        let state = guard.state_mut();
        state.last_apply_result = Some(crate::domain::state::ApplyResultSummary {
            completed: summary.completed,
            total: summary.total,
            failed: summary.failed,
            errors: summary.errors,
        });
        state.show_view(ModalView::ApplyResult);
    }
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::VIEWER_MODALS_AND_SIGNALS).await;
    StatusCode::NO_CONTENT
}

async fn cmd_apply_confirm(State(state): State<WebState>) -> impl IntoResponse {
    {
        let mut guard = state.ctx.state.write().await;
        guard.state_mut().hide_view(ModalView::DeleteConfirm);
        guard.state_mut().hide_view(ModalView::Queue);
    }
    let summary = apply_queue(&state.ctx).await;
    refresh_images_after_apply(&state.ctx).await;
    {
        let mut guard = state.ctx.state.write().await;
        let state = guard.state_mut();
        state.last_apply_result = Some(crate::domain::state::ApplyResultSummary {
            completed: summary.completed,
            total: summary.total,
            failed: summary.failed,
            errors: summary.errors,
        });
        state.show_view(ModalView::ApplyResult);
    }
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::VIEWER_MODALS_AND_SIGNALS).await;
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

async fn refresh_images_after_apply(ctx: &AppContext) {
    let root_dir = {
        let guard = ctx.state.read().await;
        guard.state().root_dir.clone()
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
        guard.state_mut().active_modal = None;
    }
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::VIEWER_MODALS_AND_SIGNALS).await;
    StatusCode::NO_CONTENT
}

async fn cmd_show_queue(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    if app_state.active_modal == Some(ModalView::Queue) {
        app_state.close_view();
    } else {
        app_state.show_view(ModalView::Queue);
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

async fn cmd_show_files(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
    if app_state.active_modal == Some(ModalView::Files) {
        app_state.close_view();
    } else {
        app_state.show_view(ModalView::Files);
    }
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx, UiPatch::MODALS_ONLY).await;
    StatusCode::NO_CONTENT
}

async fn cmd_help(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    let app_state = guard.state_mut();
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
    let selected = guard.state_mut().select_image_by_id(id);
    if selected {
        guard.state_mut().hide_view(ModalView::Queue);
        guard.state_mut().hide_view(ModalView::Files);
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
    guard.state_mut().close_view();
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

    let mut guard = ctx.state.write().await;
    let outcome = guard.state_mut().apply_decision(side);
    let immediate = outcome.as_ref().map(|o| o.immediate).unwrap_or(false);
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

    if immediate {
        if let (Some(root_dir), Some(outcome), Some(undo)) =
            (root_dir, outcome.as_ref(), undo_entry.as_mut())
        {
            let config = FsConfig::new(root_dir, destructive);
            let image = guard
                .state()
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
        guard.state_mut().record_undo(undo);
    }

    drop(guard);
    broadcast_patch(&ctx, navigation_patch(&ctx).await).await;
    if let Some(image_id) = changed_image_id {
        patch_stack_card_if_visible(&ctx, image_id).await;
    }
}

// undo helpers are handled in fs::apply_action_with_undo

async fn run_scan(ctx: &AppContext, path: PathBuf) {
    let mut images = scan_images(&path).await;

    let (existing_ids_by_path, existing_state_by_id) = {
        let state = ctx.state.read().await;
        let ids = state
            .state()
            .images
            .iter()
            .map(|image| (image.path.clone(), image.id))
            .collect::<HashMap<PathBuf, u64>>();
        let existing = state
            .state()
            .images
            .iter()
            .map(|image| {
                (
                    image.id,
                    (
                        image.decision.clone(),
                        image.queued_action.clone(),
                        image.rename_sequence,
                    ),
                )
            })
            .collect::<HashMap<u64, (crate::domain::DecisionState, Option<ActionConfig>, Option<u64>)>>();
        (ids, existing)
    };
    let mut next_id = existing_ids_by_path.values().copied().max().unwrap_or(0) + 1;
    for image in &mut images {
        if let Some(existing_id) = existing_ids_by_path.get(&image.path).copied() {
            image.id = existing_id;
            if let Some((decision, queued_action, rename_sequence)) = existing_state_by_id.get(&existing_id) {
                image.decision = decision.clone();
                image.queued_action = queued_action.clone();
                image.rename_sequence = *rename_sequence;
            }
        } else {
            image.id = next_id;
            next_id += 1;
        }
    }

    let mut state = ctx.state.write().await;
    state.state_mut().set_images(images, Some(path));
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
        let html = HeaderTemplate { view: &view }.render().unwrap_or_default();
        let patch = PatchElements::new(html)
            .selector("#header")
            .mode(ElementPatchMode::Outer);
        events.push(patch.write_as_axum_sse_event());
    }

    if patch.viewer {
        let html = ImageViewerTemplate {}.render().unwrap_or_default();
        let patch = PatchElements::new(html)
            .selector("#image-viewer")
            .mode(ElementPatchMode::Outer);
        events.push(patch.write_as_axum_sse_event());
    }

    if patch.stack {
        events
            .extend(build_stack_patch_events(ctx, &view, patch.viewer || patch.stack_reset).await);
    }

    if patch.modals {
        let html = ModalTemplate { view: &view }.render().unwrap_or_default();
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
        for image_id in &previous_ids {
            let remove = PatchElements::new_remove(format!("#image-card-{image_id}"));
            events.push(remove.write_as_axum_sse_event());
        }
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
        "currentPath": view.current_path_label,
        "directorySelectEnabled": view.directory_select_enabled
    })
    .to_string()
}

async fn navigation_patch(ctx: &AppContext) -> UiPatch {
    let guard = ctx.state.read().await;
    let state = guard.state();
    if state.has_view(ModalView::Queue) || state.has_view(ModalView::Files) {
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
    let file_count = state.images.len();
    let show_queue_modal = state.has_view(ModalView::Queue);
    let show_files_modal = state.has_view(ModalView::Files);
    let show_help_modal = state.has_view(ModalView::Help);
    let show_apply_result = state.has_view(ModalView::ApplyResult);
    let show_delete_confirm = state.has_view(ModalView::DeleteConfirm);
    let queue_modal_z = modal_layer(state.active_modal, ModalView::Queue);
    let files_modal_z = modal_layer(state.active_modal, ModalView::Files);
    let help_modal_z = modal_layer(state.active_modal, ModalView::Help);
    let apply_result_z = modal_layer(state.active_modal, ModalView::ApplyResult);
    let delete_modal_z = modal_layer(state.active_modal, ModalView::DeleteConfirm);
    let last_apply_result = state.last_apply_result.clone();
    let current_path_label = state
        .root_dir
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "No directory selected".to_string());
    let mut queue_items: Vec<QueueItem> = Vec::new();
    let mut queue_has_current = false;
    if show_queue_modal {
        let current_id = current.map(|e| e.id).unwrap_or(0);
        queue_items = state
            .queued_ids
            .iter()
            .filter_map(|queued_id| state.images.iter().find(|image| image.id == *queued_id))
            .filter_map(|image| {
                let queued = image.queued_action.as_ref()?;
                if image.id == current_id {
                    queue_has_current = true;
                }
                Some(queue_item_from_action(
                    image,
                    queued,
                    decision_side(image),
                    state.root_dir.as_ref(),
                ))
            })
            .collect();
    }
    let mut queue_insert_before_id: Option<u64> = None;
    let mut queue_insert_at_end = false;
    if show_queue_modal && !queue_items.is_empty() && !queue_has_current {
        if let Some(current_id) = current.map(|entry| entry.id) {
            let current_pos = state
                .order
                .iter()
                .position(|idx| state.images.get(*idx).map(|image| image.id) == Some(current_id));
            if let Some(current_pos) = current_pos {
                let order_position_by_id = state
                    .queued_ids
                    .iter()
                    .filter_map(|queued_id| {
                        state
                            .images
                            .iter()
                            .find(|image| image.id == *queued_id)
                            .and_then(|image| {
                                state
                                    .order
                                    .iter()
                                    .position(|idx| {
                                        state.images.get(*idx).map(|candidate| candidate.id)
                                            == Some(image.id)
                                    })
                                    .map(|order_pos| (image.id, order_pos))
                            })
                    })
                    .collect::<Vec<_>>();
                if let Some((before_id, _)) = order_position_by_id
                    .iter()
                    .find(|(_, order_pos)| *order_pos > current_pos)
                {
                    queue_insert_before_id = Some(*before_id);
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
        state
            .images
            .iter()
            .map(|image| match &image.decision {
                crate::domain::DecisionState::Decided { side, action } => {
                    queue_item_from_action(image, action, Some(*side), state.root_dir.as_ref())
                }
                crate::domain::DecisionState::Undecided => {
                    queue_item_none(image, state.root_dir.as_ref())
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let image_stack = ImageStackProjection {
        cards: build_stack_cards_in_range(state, projection.stack_start, projection.stack_end),
        cursor: state.cursor,
    };
    AppView {
        directory_select_enabled: cfg!(feature = "tauri"),
        image_stack,
        current_image_id: current.map(|entry| entry.id).unwrap_or(0),
        current_path_label,
        left_action_label: action_config_label(&state.action_mapping.left),
        right_action_label: action_config_label(&state.action_mapping.right),
        left_action_count: projection.left_action_count,
        right_action_count: projection.right_action_count,
        index,
        total,
        queue_count: projection.queue_count,
        file_count,
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
    pub directory_select_enabled: bool,
    pub image_stack: ImageStackProjection,
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
pub struct ImageStackProjection {
    pub cards: Vec<StackCard>,
    pub cursor: usize,
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
                image_src: format!("/image/{}", image.id),
                alignment: image_alignment_for(image),
                action_item,
                stack_index: pos,
            })
        })
        .collect()
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

fn modal_layer(active_modal: Option<ModalView>, view: ModalView) -> usize {
    let base = 1000usize;
    if active_modal == Some(view) { base } else { 0 }
}

#[derive(Template)]
#[template(path = "index.html")]
struct MainTemplate {
    view: AppView,
}

#[derive(Template)]
#[template(path = "partials/header.html")]
struct HeaderTemplate<'a> {
    view: &'a AppView,
}

#[derive(Template)]
#[template(path = "partials/viewer.html")]
struct ImageViewerTemplate {}

#[derive(Template)]
#[template(path = "partials/modal.html")]
struct ModalTemplate<'a> {
    view: &'a AppView,
}

#[derive(Template)]
#[template(path = "partials/image_card.html")]
struct ImageCardTemplate<'a> {
    card: &'a StackCard,
}

fn render_image_card(card: &StackCard) -> String {
    ImageCardTemplate { card }.render().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ActionConfig, ActionMapping, DecisionState, ImageMeta, SortDirection, SortKey, SortMode,
    };
    use std::path::PathBuf;

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
        assert!(html.contains("src=\"/image/2\""));
    }
}
