## Why

The current `Ctrl+O` native picker and modal image list interrupt review flow and hide filesystem context needed for fast triage. We need an always-available, root-scoped sidebar navigator so users can move through directories and act on files without leaving the main workflow.

## What Changes

- Replace the native file picker workflow on `Ctrl+O` with a left-side expand/collapse file navigation panel.
- Define a root directory boundary from the launch path (`piclr <path>`) and enforce all UI-driven navigation/actions as relative to that root. In Tauri desktop mode, allow selecting a new root from the sidebar root control, which resets the active root boundary.
- Populate the sidebar list from server-side filesystem reads, showing directories plus only files with supported image extensions.
- Remove the existing image viewer modal and place action glyphs (left/right/none), highlighting, and color coding directly in the sidebar list.
- Add directory interactions in the sidebar:
  - left/right action keys navigate into or out of directories
  - Enter on a directory opens actions (open, rename, delete)
  - `open` changes working directory and refreshes the active image list
- Ensure directories never appear in the `image-viewer`; only supported files are included there.

## Capabilities

### New Capabilities
- `sidebar-file-navigation`: Root-scoped server-driven directory/file sidebar with expand/collapse, directory actions, and keyboard navigation semantics for directories.

### Modified Capabilities
- `image-review`: Change `Ctrl+O` behavior from native picker to sidebar toggle, update navigation semantics when selection is a directory, and constrain image-viewer content to files only.
- `local-web-ui`: Remove startup/interaction dependency on native directory picker modal in favor of server-driven root-aware navigation interactions and sidebar rendering updates.

## Impact

- Affected code:
  - frontend templates/components for left sidebar list, row rendering, and selected-row directory action controls
  - keyboard command routing for `Ctrl+O`, left/right, and Enter on directory rows
  - server command/query handlers that enumerate root-relative filesystem entries and enforce root boundary
  - read-model shape and stream patches for sidebar entries, selection state, and directory context
- APIs/endpoints:
  - command endpoints for directory open/rename/delete and root-relative navigation
  - query/read-model updates to include mixed directory/file sidebar entries with action state decorations
- Dependencies/systems:
  - filesystem operations for list, rename, delete constrained to launch root
  - removal of native file picker dependency from runtime interaction path
