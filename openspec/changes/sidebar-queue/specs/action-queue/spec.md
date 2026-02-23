## MODIFIED Requirements

### Requirement: Queue mode with one action per image
The system MUST support an optional queue mode where each image has at most one queued action, and a new decision replaces any prior queued action for that image. The queue representation MUST be an ordered list used as the source of truth for execution order and queue selection behavior. When queue mode is enabled, the selected queue item MUST support keyboard-first editing, including Home/End jumps, follow-selection scrolling for long lists, and Left/Right overrides that replace the selected item's action with the currently configured left/right action.

#### Scenario: Replace a queued action
- **WHEN** queue mode is enabled and the user changes an image decision
- **THEN** the previous queued action for that image is replaced by the new action

#### Scenario: Home and End jump queue selection
- **WHEN** the queue list is selected and the user presses Home or End
- **THEN** selection jumps to the first or last queue item respectively

#### Scenario: Selected item stays visible during keyboard navigation
- **WHEN** the queue list contains more items than fit in the viewport and queue selection changes
- **THEN** the queue scroll position updates to keep the selected item visible

#### Scenario: Left and Right override selected queued action
- **WHEN** the queue list is selected and the user presses Left or Right on a selected queue item
- **THEN** the selected queue item is updated to the currently configured Left or Right action

### Requirement: Undo last command
The system MUST support undoing the most recent command, reversing its effect on image decision state and queue/application. When queue selection is active, pressing `u` MUST remove the selected queue item instead of popping from the global action stack. When queue selection is not active, pressing `u` MUST pop the most recent item from the action stack.

#### Scenario: Undo removes selected queue item in queue focus
- **WHEN** the queue list is selected and the user presses `u`
- **THEN** the selected queue item is removed from the queue list

#### Scenario: Undo pops action stack outside queue focus
- **WHEN** the queue list is not selected and the user presses `u`
- **THEN** the most recent action-stack item is popped and undone
