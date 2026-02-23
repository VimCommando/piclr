## Context

`piclr` currently relies on `Ctrl+O` opening a native file picker and a modal image/file list for navigation. This creates two problems for review throughput: users lose persistent filesystem context, and navigation logic is split between modal interactions and core image review controls.

This change introduces a server-driven left sidebar as the primary file/directory navigator. The launch path (`piclr <path>`) defines the initial root boundary for UI-driven navigation and file operations. In web-only mode this root remains immutable for the run; in Tauri desktop mode the user can pick a new root from the sidebar root control, which resets the active root boundary. All user actions from the UI are interpreted as root-relative paths, and the UI must never allow traversal above the active root.

The change touches both server read model/commands and frontend keyboard interaction semantics. Existing state visualization (left/right/none glyphs and color coding) should be preserved and moved onto sidebar rows.

## Goals / Non-Goals

**Goals:**
- Replace `Ctrl+O` native picker flow with a sidebar expand/collapse toggle.
- Present a root-scoped directory view that includes directories and supported image files only.
- Keep image viewer content file-only (never directories).
- Support directory keyboard behaviors:
  - Left/right keys navigate out/in of directory context
  - Enter on a selected directory triggers `open`; selected directory rows expose inline actions (`open`, `rename`, `delete`)
  - Directory `open` updates current working directory and image list
- Apply existing decision-state glyph/highlight/color behavior directly in sidebar rows.
- Remove the old image viewer modal.

**Non-Goals:**
- Allow navigation above launch root.
- Introduce recursive tree expansion for all descendants at once (single working-directory model is sufficient).
- Change decision semantics (left/right/none meaning remains as already defined).
- Add non-image file support in the image viewer.

## Decisions

### 1) Root-anchored path model
- Decision: Store `root_dir` (active root boundary) and `cwd_rel` (relative to root) in server state; derive absolute paths only server-side.
- Rationale: Prevents path traversal and keeps command/query contracts stable across UI interactions.
- Alternatives considered:
  - Keep absolute paths in UI/state. Rejected: makes boundary enforcement error-prone.
  - Track only absolute cwd. Rejected: harder to validate root-relative operations and stream clean relative paths to UI.

### 2) Server-authoritative sidebar listing
- Decision: Sidebar rows are produced from server filesystem reads for current directory; UI does not perform client-side directory reads.
- Rationale: Fits existing CQRS/Datastar pattern and centralizes extension filtering plus root-bound checks.
- Alternatives considered:
  - Client-side listing via browser APIs. Rejected: inconsistent availability and weak control/security model.

### 3) Unified row model for directory + image entries
- Decision: Introduce one sidebar row projection with `entry_type` (`directory`|`file`), display name, relative path, and decision state decoration fields.
- Rationale: Keeps rendering and patching simple while allowing directory-specific commands.
- Alternatives considered:
  - Separate lists/components for directories and files. Rejected: duplicates selection and keyboard handling.

### 4) Directory-specific key handling
- Decision: Keyboard dispatcher resolves behavior by selected row type:
  - on file row, keep existing left/right decision behavior
  - on directory row, left/right map to navigate out/in directory context
  - Enter on directory triggers `open`; selected directory rows expose inline action controls (open/rename/delete)
- Rationale: Preserves muscle memory while adding directory navigation without extra modes.
- Alternatives considered:
  - New dedicated keys for directory navigation. Rejected: increases cognitive load.

### 5) Modal removal and sidebar as single navigation surface
- Decision: Remove image list modal and drive selection/actions directly from sidebar.
- Rationale: Eliminates duplicate UI state and aligns with persistent context objective.
- Alternatives considered:
  - Keep modal as fallback. Rejected: duplicates behavior and increases maintenance/test scope.

### 6) Directory operations are root-relative commands
- Decision: Add/extend command endpoints for directory open/rename/delete accepting root-relative paths; server validates descendant-of-root before applying.
- Rationale: Keeps all mutating actions behind server validation and audit-friendly command path.
- Alternatives considered:
  - Mutate local state first, then best-effort filesystem op. Rejected: risks state drift on failure.

## Risks / Trade-offs

- [Risk] Ambiguous left/right semantics on directory rows could confuse users initially.
  - Mitigation: Add clear row-type affordance and action hints in sidebar UI.
- [Risk] Extra filesystem reads when navigating directories may affect responsiveness on large folders.
  - Mitigation: Restrict to current directory, filter early by supported extensions, and patch only changed sidebar regions.
- [Risk] Rename/delete failures (permissions, race with external changes) can desync expectations.
  - Mitigation: Server returns command errors and emits reconciled listing snapshot after failed mutations.
- [Risk] Removing modal may break existing keyboard tests and interaction assumptions.
  - Mitigation: Update integration tests around `Ctrl+O`, selection, and row-type key dispatch.

## Migration Plan

1. Add server state for `root_dir` and root-relative `cwd` plus guard helpers (`resolve_child_path`, `ensure_within_root`).
2. Implement sidebar listing query/projection (directories + supported files + existing decision decorations).
3. Add/update commands for directory open/rename/delete and emit read-model updates after each command.
4. Replace `Ctrl+O` binding behavior to toggle sidebar visibility; remove native picker invocation path.
5. Update key dispatcher for row-type-aware left/right/Enter behavior.
6. Remove image viewer modal templates/handlers and route interactions through sidebar.
7. Add/adjust tests:
   - cannot navigate above root
   - directories excluded from image-viewer
   - directory key behavior and selected-row action flow
   - preserved glyph/color/highlight semantics on file rows
8. Rollback strategy: if regressions appear, restore previous `Ctrl+O` picker path behind a temporary feature flag while keeping root guard code intact.

## Open Questions

- Should left/right on a directory always map to parent/child navigation, or should one key open actions when no child context exists?
- For directory delete, do we require empty directory only, or recursive delete with confirmation?
- When renaming a directory containing reviewed images, should decision state remap to new paths automatically or reset for moved entries?
- Should sidebar expansion state persist across app restart, or reset to collapsed each run?
