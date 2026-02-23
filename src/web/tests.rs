
use super::*;
use crate::app::{AppConfig, AppContext};
use crate::domain::undo::UndoEntry;
use crate::domain::{
    ActionConfig, ActionMapping, AppState, DecisionSide, DecisionState, ImageMeta, ModalView,
    SortDirection, SortKey, SortMode,
};
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

    let response = cmd::queue::apply(State(WebState { ctx: ctx.clone() }))
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
    let response = cmd::queue::apply_request(State(WebState { ctx: ctx.clone() }))
        .await
        .into_response();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let guard = ctx.state.read().await;
    assert_eq!(guard.state().active_modal, Some(ModalView::ApplyConfirm));
}

#[tokio::test]
async fn queue_toggle_uses_sidebar_not_modal() {
    let ctx = AppContext::new(sample_machine(true), AppConfig::default());
    let _ = cmd::queue::show(State(WebState { ctx: ctx.clone() })).await;

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
    let response = cmd::queue::apply_selected(State(WebState { ctx: ctx.clone() }))
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
    let response = cmd::queue::apply_selected(State(WebState { ctx: ctx.clone() }))
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

    let first_outcome = machine
        .state_mut()
        .apply_decision(DecisionSide::Left)
        .unwrap();
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
    let _ = cmd::undo(State(WebState { ctx: ctx.clone() })).await;
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
