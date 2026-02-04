use std::path::PathBuf;
use askama::Template;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use bytes::Bytes;
use mime_guess::MimeGuess;
use tracing::warn;
use serde::Deserialize;

use crate::app::AppContext;
use crate::domain::{ActionConfig, DecisionSide};
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
        .route("/cmd/apply-cancel", post(cmd_apply_cancel))
        .route("/cmd/open", post(cmd_open))
        .route("/image/:id", get(image))
        .route("/assets/datastar.js", get(datastar_js))
        .route("/assets/app.css", get(app_css))
        .with_state(state)
}

async fn index(State(state): State<WebState>) -> Html<String> {
    let ctx = state.ctx.clone();
    render_full_page(&ctx).await
}

async fn cmd_left(State(state): State<WebState>) -> Html<String> {
    let ctx = state.ctx.clone();
    apply_decision(&ctx, DecisionSide::Left).await
}

async fn cmd_right(State(state): State<WebState>) -> Html<String> {
    let ctx = state.ctx.clone();
    apply_decision(&ctx, DecisionSide::Right).await
}

async fn cmd_next(State(state): State<WebState>) -> Html<String> {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().next();
    drop(guard);
    let ctx = state.ctx.clone();
    render_partial(&ctx).await
}

async fn cmd_prev(State(state): State<WebState>) -> Html<String> {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().prev();
    drop(guard);
    let ctx = state.ctx.clone();
    render_partial(&ctx).await
}

async fn cmd_jump_next(State(state): State<WebState>) -> Html<String> {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().jump_next_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    render_partial(&ctx).await
}

async fn cmd_jump_prev(State(state): State<WebState>) -> Html<String> {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().jump_prev_undecided();
    drop(guard);
    let ctx = state.ctx.clone();
    render_partial(&ctx).await
}

async fn cmd_undo(State(state): State<WebState>) -> Html<String> {
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
    render_partial(&ctx).await
}

async fn cmd_apply(State(state): State<WebState>) -> Html<String> {
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
        guard.inner_mut().pending_delete_confirm = true;
        drop(guard);
        let ctx = state.ctx.clone();
        return render_partial(&ctx).await;
    }

    apply_queue(&state.ctx).await;
    let ctx = state.ctx.clone();
    render_partial(&ctx).await
}

async fn cmd_apply_confirm(State(state): State<WebState>) -> Html<String> {
    {
        let mut guard = state.ctx.state.write().await;
        guard.inner_mut().pending_delete_confirm = false;
    }
    apply_queue(&state.ctx).await;
    let ctx = state.ctx.clone();
    render_partial(&ctx).await
}

async fn cmd_apply_cancel(State(state): State<WebState>) -> Html<String> {
    let mut guard = state.ctx.state.write().await;
    guard.inner_mut().pending_delete_confirm = false;
    drop(guard);
    let ctx = state.ctx.clone();
    render_partial(&ctx).await
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

#[derive(Deserialize)]
struct OpenForm {
    path: String,
}

async fn cmd_open(
    State(state): State<WebState>,
    form: Option<Form<OpenForm>>,
) -> Html<String> {
    let path = form
        .and_then(|form| {
            let value = form.0.path.trim().to_string();
            if value.is_empty() {
                None
            } else {
                Some(PathBuf::from(value))
            }
        })
        .or_else(|| state.ctx.config.initial_path.clone());
    if let Some(path) = path {
        run_scan(&state.ctx, path).await;
    } else {
        let mut guard = state.ctx.state.write().await;
        guard.inner_mut().images.clear();
        guard.inner_mut().order.clear();
        guard.inner_mut().cursor = 0;
        guard.inner_mut().root_dir = None;
    }
    let ctx = state.ctx.clone();
    render_partial(&ctx).await
}

async fn apply_decision(ctx: &AppContext, side: DecisionSide) -> Html<String> {
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
    render_partial(ctx).await
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

async fn render_partial(ctx: &AppContext) -> Html<String> {
    let view = build_view(ctx).await;
    let template = AppTemplate { view };
    Html(template.render().unwrap_or_default())
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
    let has_root = inner.root_dir.is_some();
    AppView {
        has_images: current.is_some(),
        image_id: current.map(|entry| entry.id),
        image_label: current
            .map(|entry| entry.path.file_name().unwrap_or_default().to_string_lossy().to_string())
            .unwrap_or_default(),
        image_transform: current
            .and_then(|entry| orientation_transform(entry.meta.orientation))
            .unwrap_or_else(|| "".to_string()),
        index,
        total,
        queue_count,
        queue_mode: inner.queue_mode,
        show_open_modal: !has_root,
        show_delete_confirm: inner.pending_delete_confirm,
    }
}

#[derive(Clone, Debug)]
pub struct AppView {
    pub has_images: bool,
    pub image_id: Option<u64>,
    pub image_label: String,
    pub image_transform: String,
    pub index: usize,
    pub total: usize,
    pub queue_count: usize,
    pub queue_mode: bool,
    pub show_open_modal: bool,
    pub show_delete_confirm: bool,
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
