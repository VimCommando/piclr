## MODIFIED Requirements

### Requirement: Queue mode with one action per image
The system MUST support an optional queue mode where each image has at most one queued action, and a new decision replaces any prior queued action for that image. The queue representation MUST be an ordered list used as the source of truth for execution order and queue selection behavior. When queue mode is enabled, the selected queue item MUST support keyboard-first editing, including Home/End jumps, follow-selection scrolling for long lists, Shift+Up/Down selection movement, and Shift-modified per-item actions (Shift+Right apply selected with confirmation, Shift+Left remove selected).

#### Scenario: Replace a queued action
- **WHEN** queue mode is enabled and the user changes an image decision
- **THEN** the previous queued action for that image is replaced by the new action

#### Scenario: Home and End jump queue selection
- **WHEN** the queue list is selected and the user presses Home or End
- **THEN** selection jumps to the first or last queue item respectively

#### Scenario: Selected item stays visible during keyboard navigation
- **WHEN** the queue list contains more items than fit in the viewport and queue selection changes
- **THEN** the queue scroll position updates to keep the selected item visible

#### Scenario: Shift+Up and Shift+Down move queue selection
- **WHEN** the user presses Shift+Up or Shift+Down
- **THEN** queue selection moves to the previous or next queue item

#### Scenario: Shift+Right applies selected queued action
- **WHEN** a queue item is selected and the user presses Shift+Right
- **THEN** the app asks for confirmation and applies only the selected queued action

#### Scenario: Shift+Left removes selected queued action
- **WHEN** a queue item is selected and the user presses Shift+Left
- **THEN** the selected queue item is removed from the queue list

### Requirement: Undo last command
The system MUST support undoing the most recent command, reversing its effect on image decision state and queue/application. Pressing `u` MUST pop the most recent item from the global action stack regardless of queue selection state.

#### Scenario: Undo pops action stack
- **WHEN** the user presses `u`
- **THEN** the most recent action-stack item is popped and undone
