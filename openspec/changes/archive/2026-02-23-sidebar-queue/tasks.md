## 1. Queue Sidebar UI Structure

- [x] 1.1 Replace queue modal rendering with an in-layout slide-out sidebar component mounted between header and footer
- [x] 1.2 Add queue visibility toggles for `q` key and queue icon click, wired to a shared queue-visible state
- [x] 1.3 Move queue action controls to the top section of the queue list to mirror file-list control placement
- [x] 1.4 Render apply and undo icons on the currently selected queue row only

## 2. Queue Selection and Keyboard Behavior

- [x] 2.1 Introduce explicit queue-selected focus state to gate queue-specific key behavior
- [x] 2.2 Implement Home/End key handling to jump queue selection to first/last item
- [x] 2.3 Implement Shift-based queue editing shortcuts: Shift+Up/Down move queue selection, Shift+Right applies selected item with confirmation, Shift+Left removes selected item
- [x] 2.4 Keep `u` behavior global: pressing `u` pops from the action stack regardless of queue selection

## 3. Scrolling and Long-List Usability

- [x] 3.1 Add follow-selection scrolling so selected queue item is kept visible during keyboard navigation
- [x] 3.2 Ensure scroll logic only repositions when selected row leaves viewport bounds to avoid jitter

## 4. Validation and Regression Coverage

- [x] 4.1 Add or update tests for queue sidebar visibility toggles (`q` and queue icon) and non-modal rendering
- [x] 4.2 Add or update tests for Home/End jumps, Shift-based queue actions, and selected-row apply/undo affordances
- [x] 4.3 Add or update tests for global `u` stack-pop behavior and queue removal via explicit queue action controls
- [x] 4.4 Add or update tests for follow-selection scrolling behavior on long queue lists
