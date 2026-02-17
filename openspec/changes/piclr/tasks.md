## 1. Project Setup

- [x] 1.1 Add core dependencies (tokio, axum, askama, datastar asset pipeline) to Cargo.toml
- [x] 1.2 Add optional Tauri feature flags and build wiring
- [x] 1.3 Create module layout for domain, web, and filesystem adapters

## 2. Domain Core

- [x] 2.1 Define global app state machine (init, scanning, ready, viewing, applying, done)
- [x] 2.2 Define per-image state machine with type-state and decision states
- [x] 2.3 Implement action mapping registry (delete, keep, move, rename, metadata edit) with extensibility
- [x] 2.4 Implement queue mode behavior (one action per image, replacement on change)
- [x] 2.5 Implement undo-last command stack and reversal logic
- [x] 2.6 Implement sorting strategies and navigation (up/down, ctrl+up/down)
- [x] 2.7 Implement preload strategy for next image

## 3. Filesystem Adapter

- [x] 3.1 Implement directory scan for supported image files
- [x] 3.2 Implement safe delete (trash) and permanent delete after confirmation
- [x] 3.3 Implement move and rename operations used by actions
- [x] 3.4 Implement metadata read/update operations for metadata edit action
- [x] 3.5 Implement apply-at-end execution for queued actions

## 4. Axum Web Service

- [x] 4.1 Implement loopback server startup with ephemeral port selection
- [x] 4.2 Add command endpoints for left/right, undo, navigation, and apply-at-end
- [x] 4.3 Add projection endpoints or Datastar event stream for UI updates
- [x] 4.4 Embed and serve datastar.js as a static asset

## 5. UI Templates

- [x] 5.1 Create Askama templates for the main viewing page
- [x] 5.2 Add Datastar-compatible `data-*` attributes for commands and updates
- [x] 5.3 Implement delete confirmation modal for apply-at-end (default No)

## 6. Optional Tauri Shell

- [x] 6.1 Add Tauri wrapper that opens the loopback URL
- [x] 6.2 Ensure graceful shutdown and server readiness before UI load

## 7. Verification

- [x] 7.1 Add lightweight tests for state machine transitions and queue behavior
- [x] 7.2 Manually verify navigation, sorting, and delete confirmation flows
