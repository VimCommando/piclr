## 1. Runtime and command surface

- [x] 1.1 Add a runtime flag in server-rendered page context indicating whether the app is running in Tauri mode.
- [x] 1.2 Add Tauri command handlers for `minimize`, `toggle_maximize`, and `close` window actions.
- [x] 1.3 Add a Tauri command handler for titlebar drag initiation and ensure it is callable from the frontend.

## 2. Template and UI composition

- [x] 2.1 Add a top-level window-chrome wrapper template that is rendered only when Tauri mode is enabled.
- [x] 2.2 Add chrome controls (title, minimize, maximize/restore, close) and bind them to Datastar actions that invoke Tauri commands.
- [x] 2.3 Ensure existing image viewer, sidebar, and modal content renders unchanged beneath the chrome wrapper in both hydration and patch paths.

## 3. Styling and interaction behavior

- [x] 3.1 Add custom chrome and frame-affordance styles (padding/border/title row) for undecorated windows.
- [x] 3.2 Add hover/focus/active styles and accessible labels for window control buttons.
- [x] 3.3 Validate drag region behavior does not interfere with existing interactive controls.

## 4. Verification and documentation

- [x] 4.1 Add/adjust tests for Tauri-mode chrome presence and web-mode chrome absence.
- [x] 4.2 Verify minimize/maximize/close/drag behaviors in a Linux desktop run with `--features tauri`.
- [x] 4.3 Update Linux/Tauri docs to describe custom chrome behavior with `decorations: false`.
