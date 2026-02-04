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
