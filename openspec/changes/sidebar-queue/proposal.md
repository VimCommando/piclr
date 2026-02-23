## Why

The queue currently uses a modal that interrupts navigation flow and diverges from the sidebar interaction model already used for file navigation. Moving queue interactions into an in-layout sidebar makes action review faster, keeps context visible, and enables keyboard-first queue editing for long sessions.

## What Changes

- Replace the queue modal with a slide-out queue sidebar list that sits between header and footer.
- Toggle queue sidebar visibility with `q` and by clicking the queue icon.
- Move queue action controls to the top of the queue list, matching the file-list control pattern.
- Show "apply" and "undo" icons on the selected queue item for clear per-item action affordances.
- Keep the selected queue item in view by auto-scrolling on long lists.
- Add Home/End behavior to jump queue selection to first/last item.
- Allow Left/Right actions to override the current action on the selected queue item.
- Update undo behavior:
- `u` when queue is selected removes the selected queue item.
- `u` when queue is not selected pops items from the action stack.

## Capabilities

### New Capabilities
- `queue-sidebar-layout`: Queue is presented and managed as an in-layout slide-out sidebar instead of a modal.

### Modified Capabilities
- `action-queue`: Keyboard and per-item queue behaviors are expanded to support sidebar selection, scroll-follow, home/end navigation, action override, and context-sensitive undo.

## Impact

- Affected UI components for queue rendering, queue controls, and keyboard shortcut handling.
- Interaction/state management updates for queue visibility, queue focus/selection state, and undo semantics.
- Queue navigation behavior changes that may require updates to existing tests around action queue input handling.
