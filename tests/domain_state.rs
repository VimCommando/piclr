use std::path::PathBuf;

use piclr::domain::{
    ActionConfig, ActionMapping, AppStateMachine, DecisionSide, DecisionState, ImageEntry,
    ImageMeta, SortDirection, SortKey, SortMode,
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
    let mut machine = AppStateMachine::new(true, mapping, sort_mode);
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
    let mut machine = AppStateMachine::new(true, mapping, sort_mode);
    machine.transition_to_scanning();
    machine.transition_to_ready(vec![sample_image(1, "a.jpg", 0)], Some(PathBuf::from(".")));
    machine.transition_to_viewing();

    let outcome = machine.inner_mut().apply_decision(DecisionSide::Left);
    assert!(outcome.is_some());
    let entry = machine.inner().images.first().unwrap();
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
    let mut machine = AppStateMachine::new(true, mapping, sort_mode);
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

    assert!(machine.inner_mut().select_image_by_id(2));
    machine.inner_mut().apply_decision(DecisionSide::Right);
    assert!(machine.inner_mut().select_image_by_id(1));
    machine.inner_mut().apply_decision(DecisionSide::Left);

    let queued_in_order: Vec<u64> = machine
        .inner()
        .order
        .iter()
        .filter_map(|idx| machine.inner().images.get(*idx))
        .filter(|image| image.queued_action.is_some())
        .map(|image| image.id)
        .collect();
    assert_eq!(queued_in_order, vec![1, 2]);
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
    let mut machine = AppStateMachine::new(true, mapping, sort_mode);
    machine.transition_to_scanning();
    machine.transition_to_ready(
        vec![sample_image(1, "a.jpg", 0), sample_image(2, "b.jpg", 1)],
        Some(PathBuf::from(".")),
    );
    machine.transition_to_viewing();

    machine.inner_mut().apply_decision(DecisionSide::Left);
    assert!(machine.inner_mut().select_image_by_id(1));
    machine.inner_mut().apply_decision(DecisionSide::Right);

    let queued_in_order: Vec<u64> = machine
        .inner()
        .order
        .iter()
        .filter_map(|idx| machine.inner().images.get(*idx))
        .filter(|image| image.queued_action.is_some())
        .map(|image| image.id)
        .collect();
    assert_eq!(queued_in_order, vec![1]);

    let first = machine
        .inner()
        .images
        .iter()
        .find(|image| image.id == 1)
        .unwrap();
    match &first.queued_action {
        Some(ActionConfig::Keep) => {}
        _ => panic!("expected queued keep action after replacement"),
    }
}
