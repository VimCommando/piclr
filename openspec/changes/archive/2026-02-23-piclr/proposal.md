## Why

We need a lightweight, local tool to rapidly triage large photo folders with simple left/right choices, without heavy asset managers or persistent databases.

## What Changes

- Introduce a new Rust application named `piclr` that scans a directory and presents images one-by-one for left/right decisions.
- Provide keyboard (left/right/up/down) and click zones to apply actions or navigate without changes.
- Support configurable sorting (created_at, last_modified, alphabetical) with ascending/descending order.
- Implement safe delete and an optional queue mode to apply actions at the end.
- Offer an optional Tauri shell while keeping the loopback web app as the primary runtime.

## Capabilities

### New Capabilities
- `image-review`: Present images sequentially with navigation controls and ordering options.
- `action-queue`: Track per-image decisions, support undo-last, and optionally apply actions in batch.
- `local-web-ui`: Serve a minimal SSR UI with Datastar-driven updates and optional Tauri shell.

### Modified Capabilities
- (none)

## Impact

- New Rust binaries and modules for the local web service, UI rendering, and filesystem actions.
- New dependencies: tokio, axum, askama, datastar.js (embedded), optional tauri runtime.
- Local loopback networking to connect the UI to the server.
