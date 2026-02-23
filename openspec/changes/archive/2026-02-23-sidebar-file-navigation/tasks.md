## 1. Root Path Model And Filesystem Guards

- [x] 1.1 Add `root_dir` and root-relative working-directory state fields to the server read/write model initialization path.
- [x] 1.2 Implement shared path guard helpers to resolve root-relative child paths and reject traversal above launch root.
- [x] 1.3 Update command handlers to use root guard helpers for all directory navigation and mutation operations.

## 2. Sidebar Listing Query And Projection

- [x] 2.1 Implement server-side current-directory listing that returns directories plus only supported image files.
- [x] 2.2 Extend read-model/sidebar projection with row type, root-relative path, and existing decision-state decoration fields.
- [x] 2.3 Ensure image-viewer/review list derivation excludes directory rows in all listing and refresh paths.

## 3. Directory Commands And Stream Updates

- [x] 3.1 Add command endpoint/handler support for directory `open` with root-relative validation and working-directory update.
- [x] 3.2 Add command endpoint/handler support for directory `rename` with root-relative validation and filesystem error propagation.
- [x] 3.3 Add command endpoint/handler support for directory `delete` with root-relative validation and deterministic refresh behavior.
- [x] 3.4 Emit Datastar patch/update events to refresh sidebar and image-viewer projections after directory commands.

## 4. Keyboard Routing And Interaction Semantics

- [x] 4.1 Change `Ctrl+O` binding to toggle sidebar visibility and remove native file picker invocation from that path.
- [x] 4.2 Update key dispatch logic to branch on selected row type so directory rows use left/right navigation semantics.
- [x] 4.3 Implement selected-directory row actions (`open`, `rename`, `delete`) with Enter defaulting to `open`.
- [x] 4.4 Preserve existing left/right decision behavior for file rows while keeping directory row directional semantics.

## 5. UI Template Refactor And Modal Removal

- [x] 5.1 Add/adjust Askama templates for sidebar row rendering with parity between initial hydration and patch updates.
- [x] 5.2 Render left/right/none glyphs and existing highlight/color coding directly on sidebar file rows.
- [x] 5.3 Remove legacy image/file list modal templates, handlers, and entry points.
- [x] 5.4 Update frontend event wiring so file/directory navigation is performed through the sidebar surface.

## 6. Validation And Regression Coverage

- [x] 6.1 Add tests for root-boundary enforcement, including rejection of parent navigation above root.
- [x] 6.2 Add tests confirming sidebar listing includes directories + supported files and image-viewer excludes directories.
- [x] 6.3 Add tests for directory keyboard behaviors (left/right on directory, Enter open behavior, open updates working directory).
- [x] 6.4 Add tests validating `Ctrl+O` toggles sidebar and no native picker/image-list modal is invoked.
- [x] 6.5 Add or update snapshot/integration coverage for sidebar row decision glyph/color/highlight rendering parity.
