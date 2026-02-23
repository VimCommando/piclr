## Context

The project already introduced a persistent file-navigation sidebar (`sidebar-file-navigation`) to replace modal-driven navigation. The queue experience still relies on a modal interaction model, creating an inconsistent UI pattern and extra context switches while reviewing images. This change aligns queue interaction with the existing sidebar-first workflow and expands keyboard editing semantics for long queues.

Constraints:
- Queue UI must remain inside the main app frame between header and footer.
- Existing queue semantics (one action per image, ordered execution) remain authoritative.
- Keyboard interactions must stay predictable with current action bindings.

## Goals / Non-Goals

**Goals:**
- Replace queue modal with a slide-out sidebar list that can be toggled from keyboard and UI icon.
- Place queue action controls at the top of the queue list, mirroring file-list control placement.
- Provide efficient keyboard editing for long queues (scroll follow, Home/End, contextual undo, action override).
- Make selected queue-item actions explicit via apply/undo icons.

**Non-Goals:**
- Changing core apply-at-end ordering semantics.
- Introducing multi-select queue editing.
- Redesigning header/footer layout or global hotkey architecture beyond queue interactions.

## Decisions

- Use in-layout sidebar composition instead of modal overlay.
Rationale: keeps queue visible alongside current image context and aligns with existing sidebar architecture.
Alternative considered: keep modal and add shortcuts; rejected because it preserves context switching and inconsistent layout model.

- Treat queue selection as a first-class focus mode with dedicated key semantics.
Rationale: requirements include different `u` behavior when queue is selected vs not selected, plus Home/End and Left/Right override behavior tied to queue selection.
Alternative considered: global keys only; rejected because behavior would be ambiguous without explicit queue focus context.

- Render selected-row affordances (apply/undo icons) inline on the selected item only.
Rationale: reduces visual noise while still making per-item actions discoverable.
Alternative considered: always-visible per-row controls; rejected due to density and clutter in long lists.

- Follow-selection scrolling is automatic when selection changes.
Rationale: long queue usability depends on keeping active selection visible without manual scrolling.
Alternative considered: no auto-scroll with optional jump key only; rejected because it fails continuous keyboard navigation ergonomics.

## Risks / Trade-offs

- [Keybinding conflicts] Queue-specific Left/Right and `u` semantics may conflict with existing global actions if focus state is not explicit.
  → Mitigation: gate behavior by queue-selected state and preserve existing stack undo when queue is not selected.

- [UI state complexity] Queue visibility and selection state can desynchronize under rapid input.
  → Mitigation: centralize queue UI state transitions and drive rendering from single reactive source of truth.

- [Regression in undo behavior] Users may expect old undo behavior everywhere.
  → Mitigation: keep stack-pop behavior unchanged outside queue selection and document contextual behavior in UI/help text.

- [Long-list performance] Auto-scroll on every selection move could cause jitter.
  → Mitigation: scroll only when selected row leaves viewport bounds and use nearest-edge alignment.
