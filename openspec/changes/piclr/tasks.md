## 1. Project Setup

- [ ] 1.1 Add core dependencies (tokio, axum, askama, datastar asset pipeline) to Cargo.toml
- [ ] 1.2 Add optional Tauri feature flags and build wiring
- [ ] 1.3 Create module layout for domain, web, and filesystem adapters

## 2. Domain Core

- [ ] 2.1 Define global app state machine (init, scanning, ready, viewing, applying, done)
- [ ] 2.2 Define per-image state machine with type-state and decision states
- [ ] 2.3 Implement action mapping registry (delete, keep, move, rename, metadata edit) with extensibility
- [ ] 2.4 Implement queue mode behavior (one action per image, replacement on change)
- [ ] 2.5 Implement undo-last command stack and reversal logic
- [ ] 2.6 Implement sorting strategies and navigation (up/down, ctrl+up/down)
- [ ] 2.7 Implement preload strategy for next image

## 3. Filesystem Adapter

- [ ] 3.1 Implement directory scan for supported image files
- [ ] 3.2 Implement safe delete (trash) and permanent delete after confirmation
- [ ] 3.3 Implement move and rename operations used by actions
- [ ] 3.4 Implement metadata read/update operations for metadata edit action
- [ ] 3.5 Implement apply-at-end execution for queued actions

## 4. Axum Web Service

- [ ] 4.1 Implement loopback server startup with ephemeral port selection
- [ ] 4.2 Add command endpoints for left/right, undo, navigation, and apply-at-end
- [ ] 4.3 Add projection endpoints or Datastar event stream for UI updates
- [ ] 4.4 Embed and serve datastar.js as a static asset

## 5. UI Templates

- [ ] 5.1 Create Askama templates for the main viewing page
- [ ] 5.2 Add Datastar-compatible `data-*` attributes for commands and updates
- [ ] 5.3 Implement delete confirmation modal for apply-at-end (default No)

## 6. Optional Tauri Shell

- [ ] 6.1 Add Tauri wrapper that opens the loopback URL
- [ ] 6.2 Ensure graceful shutdown and server readiness before UI load

## 7. Verification

- [ ] 7.1 Add lightweight tests for state machine transitions and queue behavior
- [ ] 7.2 Manually verify navigation, sorting, and delete confirmation flows
