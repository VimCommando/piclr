use std::path::PathBuf;

use piclr::domain::{
    ActionConfig, ActionMapping, AppState, DecisionSide, DecisionState, ImageEntry, ImageMeta,
    SortDirection, SortKey, SortMode,
};

fn sample_image(id: u64, path: &str, order: usize) -> ImageEntry {
    ImageEntry {
        id,
        path: PathBuf::from(path),
        original_order: order,
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

#[test]
fn state_machine_transitions() {
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
    machine.transition_to_ready(vec![sample_image(1, "a.jpg", 0)], Some(PathBuf::from(".")));
    machine.transition_to_viewing();
    machine.transition_to_applying();
    machine.transition_to_done();
}

#[test]
fn queue_mode_sets_queued_action() {
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
    machine.transition_to_ready(vec![sample_image(1, "a.jpg", 0)], Some(PathBuf::from(".")));
    machine.transition_to_viewing();

    let outcome = machine.state_mut().apply_decision(DecisionSide::Left);
    assert!(outcome.is_some());
    let entry = machine.state().images.first().unwrap();
    assert!(entry.queued_action.is_some());
}

#[test]
fn queued_ids_follow_sort_order_not_decision_time() {
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
            sample_image(1, "a.jpg", 0),
            sample_image(2, "b.jpg", 1),
            sample_image(3, "c.jpg", 2),
        ],
        Some(PathBuf::from(".")),
    );
    machine.transition_to_viewing();

    assert!(machine.state_mut().select_image_by_id(2));
    machine.state_mut().apply_decision(DecisionSide::Right);
    assert!(machine.state_mut().select_image_by_id(1));
    machine.state_mut().apply_decision(DecisionSide::Left);

    assert_eq!(
        machine
            .state()
            .queued_ids
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn changing_decision_replaces_queued_action_for_image() {
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
        vec![sample_image(1, "a.jpg", 0), sample_image(2, "b.jpg", 1)],
        Some(PathBuf::from(".")),
    );
    machine.transition_to_viewing();

    machine.state_mut().apply_decision(DecisionSide::Left);
    assert!(machine.state_mut().select_image_by_id(1));
    machine.state_mut().apply_decision(DecisionSide::Right);

    assert_eq!(
        machine
            .state()
            .queued_ids
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![1]
    );

    let first = machine
        .state()
        .images
        .iter()
        .find(|image| image.id == 1)
        .unwrap();
    match &first.queued_action {
        Some(ActionConfig::Keep) => {}
        _ => panic!("expected queued keep action after replacement"),
    }
}

#[test]
fn projection_and_invariants_stay_consistent_after_commands() {
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
            sample_image(1, "a.jpg", 0),
            sample_image(2, "b.jpg", 1),
            sample_image(3, "c.jpg", 2),
        ],
        Some(PathBuf::from(".")),
    );
    machine.transition_to_viewing();

    machine.state_mut().apply_decision(DecisionSide::Left);
    machine.state_mut().apply_decision(DecisionSide::Right);

    let projection = machine.state().projection();
    assert_eq!(projection.left_action_count, 1);
    assert_eq!(projection.right_action_count, 1);
    assert_eq!(projection.queue_count, 2);
}

#[test]
fn apply_actions_follow_queue_order() {
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
            sample_image(1, "a.jpg", 0),
            sample_image(2, "b.jpg", 1),
            sample_image(3, "c.jpg", 2),
        ],
        Some(PathBuf::from(".")),
    );
    machine.transition_to_viewing();

    assert!(machine.state_mut().select_image_by_id(3));
    machine.state_mut().apply_decision(DecisionSide::Right);
    assert!(machine.state_mut().select_image_by_id(1));
    machine.state_mut().apply_decision(DecisionSide::Left);
    assert!(machine.state_mut().select_image_by_id(2));
    machine.state_mut().apply_decision(DecisionSide::Right);

    let queued = machine.state().queued_actions_for_apply();
    let order: Vec<String> = queued
        .iter()
        .map(|(path, _, _)| path.to_string_lossy().to_string())
        .collect();

    assert_eq!(order, vec!["a.jpg", "b.jpg", "c.jpg"]);
}

#[test]
fn queue_sidebar_selection_and_override_work() {
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
            sample_image(1, "a.jpg", 0),
            sample_image(2, "b.jpg", 1),
            sample_image(3, "c.jpg", 2),
        ],
        Some(PathBuf::from(".")),
    );
    machine.transition_to_viewing();

    machine.state_mut().apply_decision(DecisionSide::Right);
    machine.state_mut().apply_decision(DecisionSide::Left);
    machine.state_mut().apply_decision(DecisionSide::Right);

    machine.state_mut().toggle_queue_sidebar();
    assert!(machine.state().queue_sidebar_visible);
    assert!(machine.state().queue_is_selected());

    assert!(machine.state_mut().select_queue_first());
    assert_eq!(machine.state().selected_queue_image_id, Some(1));
    assert!(machine.state_mut().select_queue_last());
    assert_eq!(machine.state().selected_queue_image_id, Some(3));

    assert!(
        machine
            .state_mut()
            .override_selected_queue_action(DecisionSide::Left)
    );
    let entry = machine
        .state()
        .images
        .iter()
        .find(|image| image.id == 3)
        .unwrap();
    match &entry.queued_action {
        Some(ActionConfig::Delete) => {}
        _ => panic!("expected selected queue action to be overridden to Delete"),
    }
}

#[test]
fn removing_selected_queue_item_updates_selection() {
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
        vec![sample_image(1, "a.jpg", 0), sample_image(2, "b.jpg", 1)],
        Some(PathBuf::from(".")),
    );
    machine.transition_to_viewing();

    machine.state_mut().apply_decision(DecisionSide::Left);
    machine.state_mut().apply_decision(DecisionSide::Right);
    machine.state_mut().toggle_queue_sidebar();
    assert!(machine.state_mut().select_queue_last());
    assert_eq!(machine.state().selected_queue_image_id, Some(2));

    let removed = machine.state_mut().remove_selected_queue_item();
    assert_eq!(removed, Some(2));
    assert_eq!(machine.state().projection().queue_count, 1);
    assert_eq!(machine.state().selected_queue_image_id, Some(1));
}

#[test]
fn closing_file_sidebar_keeps_focus_on_queue_when_queue_is_visible() {
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
        vec![sample_image(1, "a.jpg", 0), sample_image(2, "b.jpg", 1)],
        Some(PathBuf::from(".")),
    );
    machine.transition_to_viewing();

    machine.state_mut().toggle_sidebar();
    machine.state_mut().apply_decision(DecisionSide::Left);
    machine.state_mut().toggle_queue_sidebar();
    assert!(machine.state().queue_is_selected());

    // Simulate user selecting files while both are visible.
    machine.state_mut().queue_focus = false;
    assert!(!machine.state().queue_is_selected());

    // Closing file sidebar should hand focus back to queue selection.
    machine.state_mut().toggle_sidebar();
    assert!(!machine.state().sidebar_expanded);
    assert!(machine.state().queue_is_selected());
}
