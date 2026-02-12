use std::path::PathBuf;
use askama::Template;
use axum::body::Bytes as BodyBytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response, Sse};
use axum::response::sse::{Event, KeepAlive};
use axum::routing::{get, post};
use axum::Router;
use asynk_strim::{stream_fn, Yielder};
use core::convert::Infallible;
use bytes::Bytes;
use mime_guess::MimeGuess;
use tracing::warn;
use serde::Deserialize;
use datastar::prelude::{ElementPatchMode, PatchElements};
use tokio::sync::broadcast;

#[derive(Deserialize)]
struct OpenForm {
    path: String,
}

use crate::app::AppContext;
use crate::domain::{ActionConfig, DecisionSide, ImageEntry, ModalView};
use crate::fs::{apply_action, apply_action_with_undo, apply_undo_action, load_image_bytes, scan_images, FsConfig};

#[derive(Clone)]
pub struct WebState {
    pub ctx: AppContext,
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
        .route("/cmd/open", post(cmd_open))
        .route("/cmd/show-queue", post(cmd_show_queue))
        .route("/cmd/close", post(cmd_close))
        .route("/events", get(events))
        .route("/image/{id}", get(image))
        .route("/assets/datastar.js", get(datastar_js))
        .route("/assets/app.css", get(app_css))
        .with_state(state)
}

async fn index(State(state): State<WebState>) -> Html<String> {
    {
        let mut guard = state.ctx.state.write().await;
        let inner = guard.inner_mut();
        if inner.root_dir.is_none() && !inner.has_view(ModalView::OpenDirectory) {
            inner.show_view(ModalView::OpenDirectory);
        }
    }
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
    broadcast_patch(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_prev(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().prev();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_jump_next(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().jump_next_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_jump_prev(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().jump_prev_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_undo(State(state): State<WebState>) -> impl IntoResponse {
    let undo_action = {
        let mut guard = state.ctx.state.write().await;
        guard.inner_mut().undo_last().and_then(|entry| entry.undo_action)
    };

    if let Some(action) = undo_action {
        if let Err(err) = apply_undo_action(&action).await {
            warn!(%err, "Failed to undo action");
        }
    }

    let ctx = state.ctx.clone();
    broadcast_patch(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_apply(State(state): State<WebState>) -> impl IntoResponse {
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
        broadcast_patch(&ctx).await;
        return StatusCode::NO_CONTENT;
    }

    apply_queue(&state.ctx).await;
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_apply_confirm(State(state): State<WebState>) -> impl IntoResponse {
    {
        let mut guard = state.ctx.state.write().await;
        guard.inner_mut().hide_view(ModalView::DeleteConfirm);
    }
    apply_queue(&state.ctx).await;
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn apply_queue(ctx: &AppContext) {
    let (root_dir, destructive) = {
        let guard = ctx.state.read().await;
        (guard.inner().root_dir.clone(), ctx.config.destructive_delete)
    };

    let Some(root_dir) = root_dir else {
        return;
    };

    let config = FsConfig::new(root_dir, destructive);
    let mut guard = ctx.state.write().await;
    let images = guard.inner_mut().images.clone();
    for image in images {
        if let Some(action) = image.queued_action {
            if let Err(err) = apply_action(&config, &image.path, &action, image.rename_sequence).await {
                warn!(%err, path = %image.path.display(), "Failed to apply queued action");
            }
        }
    }
    guard.inner_mut().images.iter_mut().for_each(|image| image.queued_action = None);
    drop(guard);
}

async fn cmd_open(
    State(state): State<WebState>,
    body: BodyBytes,
) -> impl IntoResponse {
    let path = if body.is_empty() {
        None
    } else {
        serde_urlencoded::from_bytes::<OpenForm>(&body)
            .ok()
            .and_then(|form| {
                let value = form.path.trim().to_string();
                if value.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(value))
                }
            })
    }
    .or_else(|| state.ctx.config.initial_path.clone());
    if let Some(path) = path {
        run_scan(&state.ctx, path).await;
        let mut guard = state.ctx.state.write().await;
        guard.inner_mut().hide_view(ModalView::OpenDirectory);
    } else {
        let mut guard = state.ctx.state.write().await;
        guard.inner_mut().show_view(ModalView::OpenDirectory);
    }
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_show_queue(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().show_view(ModalView::Queue);
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn cmd_close(State(state): State<WebState>) -> impl IntoResponse {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().close_view();
    drop(guard);
    let ctx = state.ctx.clone();
    broadcast_patch(&ctx).await;
    StatusCode::NO_CONTENT
}

async fn apply_decision(ctx: AppContext, side: DecisionSide) {
    let (root_dir, destructive) = {
        let guard = ctx.state.read().await;
        (guard.inner().root_dir.clone(), ctx.config.destructive_delete)
    };

    let mut guard = ctx.state.write().await;
    let outcome = guard.inner_mut().apply_decision(side);
    let immediate = outcome.as_ref().map(|o| o.immediate).unwrap_or(false);
    let mut undo_entry = outcome.as_ref().map(|outcome| crate::domain::undo::UndoEntry {
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
    broadcast_patch(&ctx).await;
}

// undo helpers are handled in fs::apply_action_with_undo

async fn run_scan(ctx: &AppContext, path: PathBuf) {
    let mut state = ctx.state.write().await;
    state.transition_to_scanning();
    drop(state);

    let images = scan_images(&path).await;

    let mut state = ctx.state.write().await;
    state.transition_to_ready(images, Some(path));
    state.transition_to_viewing();
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

    state.ctx.cache.write().await.insert(path.clone(), bytes.clone());
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
        HeaderValue::from_str(mime.as_ref()).unwrap_or(HeaderValue::from_static("application/octet-stream")),
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
    Sse::new(stream_fn(move |mut yielder: Yielder<Result<Event, Infallible>>| async move {
        let initial = build_patch_event(&ctx).await;
        yielder.yield_item(Ok(initial)).await;

        loop {
            match rx.recv().await {
                Ok(event) => {
                    yielder.yield_item(Ok(event)).await;
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }))
    .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(20)).text("keep-alive"))
}

async fn broadcast_patch(ctx: &AppContext) {
    let event = build_patch_event(ctx).await;
    let _ = ctx.sse_tx.send(event);
}

async fn build_patch_event(ctx: &AppContext) -> Event {
    let view = build_view(ctx).await;
    let template = AppTemplate { view };
    let html = template.render().unwrap_or_default();
    let patch = PatchElements::new(html)
        .selector("#app")
        .mode(ElementPatchMode::Outer);
    patch.write_as_axum_sse_event()
}

async fn build_view(ctx: &AppContext) -> AppView {
    let guard = ctx.state.read().await;
    let inner = guard.inner();
    let total = inner.order.len();
    let index = inner.cursor + 1;
    let current = inner.current();
    if let Some(next_path) = inner.preload.next_path.clone() {
        let cache = ctx.cache.clone();
        tokio::spawn(async move {
            if cache.read().await.contains_key(&next_path) {
                return;
            }
            if let Ok(bytes) = load_image_bytes(&next_path).await {
                cache.write().await.insert(next_path, Bytes::from(bytes));
            }
        });
    }
    let queue_count = inner
        .images
        .iter()
        .filter(|image| image.queued_action.is_some())
        .count();
    let queue_items = inner
        .images
        .iter()
        .filter_map(|image| {
            let queued = image.queued_action.as_ref()?;
            queue_item_from_action(image, queued, decision_side(image), inner.root_dir.as_ref())
        })
        .collect();
    let current_action_item = current.and_then(|image| {
        let (side, action) = match &image.decision {
            crate::domain::DecisionState::Decided { side, action } => (Some(*side), action),
            _ => return None,
        };
        queue_item_from_action(image, action, side, inner.root_dir.as_ref())
    });
    let image_alignment = match decision_side_from_entry(current) {
        Some(DecisionSide::Left) => "left".to_string(),
        Some(DecisionSide::Right) => "right".to_string(),
        None => "center".to_string(),
    };
    AppView {
        has_images: current.is_some(),
        image_id: current.map(|entry| entry.id),
        image_label: current
            .map(|entry| entry.path.file_name().unwrap_or_default().to_string_lossy().to_string())
            .unwrap_or_default(),
        image_transform: current
            .and_then(|entry| orientation_transform(entry.meta.orientation))
            .unwrap_or_else(|| "".to_string()),
        image_alignment,
        current_action_item,
        index,
        total,
        queue_count,
        queue_mode: inner.queue_mode,
        show_open_modal: inner.has_view(ModalView::OpenDirectory),
        show_delete_confirm: inner.has_view(ModalView::DeleteConfirm),
        show_queue_modal: inner.has_view(ModalView::Queue),
        queue_items,
    }
}

#[derive(Clone, Debug)]
pub struct AppView {
    pub has_images: bool,
    pub image_id: Option<u64>,
    pub image_label: String,
    pub image_transform: String,
    pub image_alignment: String,
    pub current_action_item: Option<QueueItem>,
    pub index: usize,
    pub total: usize,
    pub queue_count: usize,
    pub queue_mode: bool,
    pub show_open_modal: bool,
    pub show_delete_confirm: bool,
    pub show_queue_modal: bool,
    pub queue_items: Vec<QueueItem>,
}

#[derive(Clone, Debug)]
pub struct QueueItem {
    pub arrow: String,
    pub action_label: String,
    pub action_tone: String,
    pub file_label: String,
}

fn decision_side(image: &ImageEntry) -> Option<DecisionSide> {
    match &image.decision {
        crate::domain::DecisionState::Decided { side, .. } => Some(*side),
        _ => None,
    }
}

fn decision_side_from_entry(image: Option<&ImageEntry>) -> Option<DecisionSide> {
    image.and_then(decision_side)
}

fn queue_item_from_action(
    image: &ImageEntry,
    action: &ActionConfig,
    side: Option<DecisionSide>,
    root_dir: Option<&PathBuf>,
) -> Option<QueueItem> {
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
        ActionConfig::Keep => "keep".to_string(),
        ActionConfig::Delete => "delete".to_string(),
        ActionConfig::Move { target } => format!("mv {}", target.display()),
        ActionConfig::Rename { prefix } => {
            let ext = image
                .path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("");
            let seq = image.rename_sequence.unwrap_or(0);
            if ext.is_empty() {
                format!("rename {}{:06}", prefix, seq)
            } else {
                format!("rename {}{:06}.{}", prefix, seq, ext)
            }
        }
        ActionConfig::MetadataEdit { key, value } => format!("meta {}={}", key, value),
    };
    let file_label = root_dir
        .and_then(|root| image.path.strip_prefix(root).ok())
        .unwrap_or(&image.path)
        .to_string_lossy()
        .to_string();
    Some(QueueItem {
        arrow,
        action_label,
        action_tone,
        file_label,
    })
}

fn orientation_transform(value: Option<u16>) -> Option<String> {
    let degrees = match value? {
        3 => 180,
        6 => 90,
        8 => 270,
        _ => 0,
    };
    if degrees == 0 {
        None
    } else {
        Some(format!("rotate({}deg)", degrees))
    }
}

#[derive(Template)]
#[template(path = "main.html")]
struct MainTemplate {
    view: AppView,
}

#[derive(Template)]
#[template(path = "app.html")]
struct AppTemplate {
    view: AppView,
}
