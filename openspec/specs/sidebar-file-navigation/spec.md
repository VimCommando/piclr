## Purpose
Root-scoped sidebar file navigation for directory-aware image review.

## Requirements

### Requirement: Root-scoped filesystem boundary
The system MUST enforce a root boundary for UI-driven filesystem navigation and actions. In web-only mode, the launch path provided to `piclr` is the immutable root. In Tauri desktop mode, selecting a new root directory from the sidebar root control updates the active root boundary. The UI MUST NOT allow traversal or mutation above the active root.

#### Scenario: Reject navigation above root
- **WHEN** a user action attempts to move to a parent path above the launch root
- **THEN** the command is rejected and the current working directory remains unchanged

#### Scenario: Tauri root reselection updates active root boundary
- **WHEN** the app is running in Tauri mode and the user selects a new root directory from the sidebar root control
- **THEN** the active root boundary and current working directory are updated to the selected directory and sidebar/image projections are rebuilt from that new root

### Requirement: Server-populated sidebar listing
The system MUST populate the sidebar from server-side filesystem reads for the current working directory and MUST include directory entries plus only files with supported image extensions.

#### Scenario: Sidebar shows directories and supported image files
- **WHEN** the server lists entries for the current working directory
- **THEN** the sidebar includes child directories and supported image files only

### Requirement: File-only image-viewer content
The system MUST exclude directory entries from the image-viewer and review list, even when directories are visible in the sidebar.

#### Scenario: Directory excluded from image-viewer
- **WHEN** the current working directory contains subdirectories and image files
- **THEN** the image-viewer list contains only supported image files

### Requirement: Sidebar decision-state decorations
The system MUST render existing decision-state decorations (left, right, none glyphs plus highlight/color coding) directly on sidebar file rows using the same semantics as the current file/image list.

#### Scenario: Sidebar row reflects decision state
- **WHEN** a file has a left, right, or undecided decision state
- **THEN** the corresponding sidebar row displays the matching glyph and color/highlight treatment

### Requirement: Directory keyboard navigation semantics
When the selected sidebar entry is a directory, the system MUST interpret directional action keys as directory navigation commands instead of image decision commands.

#### Scenario: Navigate into selected directory
- **WHEN** the selected sidebar row is a directory and the user presses the configured right action key
- **THEN** the working directory changes to that directory and the sidebar/image lists refresh for the new working directory

#### Scenario: Navigate to parent directory
- **WHEN** the selected sidebar row is a directory and the user presses the configured left action key
- **THEN** the working directory changes to its parent unless the current directory is the root

### Requirement: Directory actions on selected row
The system MUST expose directory actions (`open`, `rename`, and `delete`) inline on the selected directory row. Pressing Enter on a selected directory row MUST trigger `open`.

#### Scenario: Selected directory row exposes actions
- **WHEN** a directory row is selected and the user presses Enter
- **THEN** the selected row provides `open`, `rename`, and `delete` actions and Enter triggers `open`

### Requirement: Directory open command updates working directory
The `open` directory action MUST set the selected directory as the current working directory and update sidebar and image-viewer data from that new location.

#### Scenario: Open action changes working directory
- **WHEN** the user selects `open` from the selected directory row actions (or presses Enter on the selected directory row)
- **THEN** the server updates current working directory and emits refreshed sidebar and image-viewer projections

### Requirement: Files sidebar toggle shortcut
The system MUST bind `F` to sidebar expand/collapse behavior.

#### Scenario: F toggles sidebar
- **WHEN** the user presses `F`
- **THEN** the sidebar visibility toggles

### Requirement: Ctrl+O opens root-location picker in desktop mode
The system MUST bind `Ctrl+O` to root-location picker behavior in desktop mode and MUST NOT reuse that shortcut for sidebar toggling.

#### Scenario: Ctrl+O opens location picker
- **WHEN** the user presses `Ctrl+O` in desktop mode
- **THEN** the native location picker opens and selecting a directory updates active root/current directory

### Requirement: Remove image/file list modal
The system MUST remove the legacy image/file list modal and perform file/directory navigation through the sidebar.

#### Scenario: Image/file list modal is unavailable
- **WHEN** the user requests file navigation from the UI
- **THEN** navigation is provided by the sidebar and no image/file list modal is opened
