use crate::app::AppContext;
use crate::domain::{
    ActionConfig, DecisionSide, ImageEntry, ModalView, NavEntryKind, SortDirection, SortKey,
    SortMode,
};
use crate::fs::{
    FsConfig, apply_action, apply_action_with_undo, load_image_bytes, scan_directories, scan_images,
};
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
use datastar::prelude::{ElementPatchMode, PatchElements, PatchSignals};
use mime_guess::MimeGuess;
use std::collections::HashSet;
use std::path::{Component, PathBuf};
#[cfg(test)]
use std::sync::{Arc, OnceLock};
use tokio::sync::broadcast::error::RecvError;
use tracing::warn;

mod cmd;

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
        .route("/assets/app.css", get(app_css))
        .route("/assets/datastar.js", get(datastar_js))
        .route("/cmd/apply", post(cmd::queue::apply))
        .route("/cmd/apply-confirm", post(cmd::queue::apply_confirm))
        .route("/cmd/apply/request", post(cmd::queue::apply_request))
        .route("/cmd/close", post(cmd::close))
        .route("/cmd/end", post(cmd::end))
        .route("/cmd/help", post(cmd::help))
        .route("/cmd/home", post(cmd::home))
        .route("/cmd/image/left", post(cmd::image::left))
        .route("/cmd/image/right", post(cmd::image::right))
        .route("/cmd/jump-next", post(cmd::jump_next))
        .route("/cmd/jump-prev", post(cmd::jump_prev))
        .route("/cmd/left", post(cmd::left))
        .route("/cmd/next", post(cmd::next))
        .route("/cmd/prev", post(cmd::prev))
        .route(
            "/cmd/queue/apply-selected",
            post(cmd::queue::apply_selected),
        )
        .route("/cmd/queue/next", post(cmd::queue::next))
        .route("/cmd/queue/prev", post(cmd::queue::prev))
        .route(
            "/cmd/queue/remove-selected",
            post(cmd::queue::remove_selected),
        )
        .route("/cmd/queue/reset", post(cmd::queue::reset))
        .route("/cmd/queue/select/{id}", post(cmd::queue::select))
        .route("/cmd/queue/show", post(cmd::queue::show))
        .route("/cmd/right", post(cmd::right))
        .route("/cmd/select/{id}", post(cmd::select))
        .route(
            "/cmd/sidebar/change-directory/apply",
            post(cmd::files::change_directory_apply),
        )
        .route(
            "/cmd/sidebar/change-directory/cancel",
            post(cmd::files::change_directory_cancel),
        )
        .route(
            "/cmd/sidebar/change-directory/clear",
            post(cmd::files::change_directory_clear),
        )
        .route("/cmd/sidebar/delete", post(cmd::files::delete_request))
        .route(
            "/cmd/sidebar/delete/confirm",
            post(cmd::files::delete_confirm),
        )
        .route("/cmd/sidebar/open", post(cmd::files::open))
        .route(
            "/cmd/sidebar/open-entry/{id}",
            post(cmd::files::open_entry),
        )
        .route("/cmd/sidebar/open-parent", post(cmd::files::open_parent))
        .route("/cmd/sidebar/rename", patch(cmd::files::rename))
        .route("/cmd/sidebar/root/select", post(cmd::files::root_select))
        .route("/cmd/sidebar/sort/{mode}", post(cmd::files::sort))
        .route("/cmd/sidebar/toggle", post(cmd::files::toggle))
        .route("/cmd/undo", post(cmd::undo))
        .route("/events", get(events))
        .route("/favicon.ico", get(favicon_ico))
        .route("/image/by-path/{rel}", get(image_by_rel_path))
        .route("/image/{id}", get(image))
        .with_state(state)
}

async fn index(State(state): State<WebState>) -> Html<String> {
    let ctx = state.ctx.clone();
    render_full_page(&ctx).await
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
        || rel_path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
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

    let queue_items = build_queue_items(state, queue_selected_id);
    let sidebar_items = build_sidebar_items(state, sidebar_selected_id);
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

fn build_queue_items(
    state: &crate::domain::state::AppStateInner,
    queue_selected_id: Option<u64>,
) -> Vec<QueueItem> {
    let current_id = state.current().map(|image| image.id);
    let mut queue_items: Vec<QueueItem> = state
        .queued_ids
        .iter()
        .filter_map(|queued_id| state.images.iter().find(|image| image.id == *queued_id))
        .filter_map(|image| {
            image.queued_action.as_ref()?;
            Some(queue_item_from_action(
                image,
                decision_side(image),
                state.root_dir.as_ref(),
            ))
        })
        .collect();
    for item in &mut queue_items {
        item.selected = Some(item.image_id) == queue_selected_id;
        item.peer_active =
            !item.selected && !state.queue_focus && Some(item.image_id) == current_id;
    }
    queue_items
}

fn build_sidebar_items(
    state: &crate::domain::state::AppStateInner,
    sidebar_selected_id: Option<u64>,
) -> Vec<SidebarItem> {
    state
        .nav_entries
        .iter()
        .map(|entry| match entry.kind {
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
                    crate::domain::DecisionState::Decided { side, .. } => {
                        queue_item_from_action(image, Some(*side), state.root_dir.as_ref())
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
        })
        .collect::<Vec<_>>()
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
        crate::domain::DecisionState::Decided { side, .. } => {
            queue_item_from_action(image, Some(*side), root_dir)
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
        Some(ModalView::DirectoryDeleteConfirm) => DirectoryDeleteConfirmModalTemplate {}
            .render()
            .unwrap_or_default(),
        Some(ModalView::Help) => HelpModalTemplate {}.render().unwrap_or_default(),
        Some(ModalView::ApplyResult) => ResultModalTemplate { view }.render().unwrap_or_default(),
        None => "<modal-none id=\"modal\"></modal-none>".to_string(),
    }
}

#[cfg(test)]
mod tests;
