## Context

`piclr` is a local, single-user image triage tool. The UI is a minimal web app served from an Axum loopback server, with Askama templates and Datastar for server-driven updates. The app must support keyboard and click-based left/right decisions, safe delete, and optional batch application of queued actions. Tauri is optional and should wrap the loopback web app when enabled, but the server remains the source of truth.

State is in-memory for each run. The frontend should be as stateless as possible, relying on Datastar event streams and server-side projections. The domain should be modeled with explicit state machines and type-state to avoid invalid transitions.

## Goals / Non-Goals

**Goals:**
- Present images in a deterministic order with configurable sorting and navigation.
- Support left/right decisions bound to actions such as delete, keep, move, rename, and metadata edits.
- Provide an optional queue mode: track one action per image and apply all at the end.
- Implement undo for the last command only.
- Keep the frontend minimal (Askama + Datastar, embedded JS).
- Keep runtime local and single-user, with no persistent state.

**Non-Goals:**
- Multi-user sessions, remote access, or collaborative sorting.
- Persistent storage of progress across runs.
- Complex undo stacks beyond “reverse last command”.

## Decisions

**Loopback-first architecture**
- Decision: The Axum server runs locally on loopback, and the UI connects via HTTP. Tauri is optional and loads the same loopback URL.
- Alternatives: Pure Tauri protocol or embedded handlers.
- Rationale: Keeps the server the canonical source of truth and makes Tauri optional.

**Server-driven UI with CQRS**
- Decision: UI emits commands (left/right/undo/navigation) to Axum; Axum updates state and streams Datastar patches.
- Alternatives: client-side state in JS or SPA framework.
- Rationale: Minimize frontend logic and keep the state machine in Rust.

**State machines and type-state**
- Decision: Model per-image and global app state transitions explicitly, enforcing valid transitions at compile time where feasible.
- Alternatives: ad-hoc state flags or a single mutable struct.
- Rationale: Avoid invalid combinations and make command handling predictable.

**Queue mode semantics**
- Decision: Global queue mode. Each image holds at most one queued action; a new decision replaces the previous action for that image. Apply-at-end executes all queued actions together.
- Alternatives: append-only action log or per-image queues.
- Rationale: Keeps behavior simple and matches user expectation for binary decisions.

**Safe delete with double confirmation**
- Decision: Delete actions move files to a `trash/` subdirectory by default and require explicit confirmation before permanent deletion when destructive deletion is enabled.
- Alternatives: immediate permanent delete or OS trash integration only.
- Rationale: Prevents irreversible mistakes in a fast-paced workflow.

**Sorting and navigation**
- Decision: Default filesystem order, with optional sorting by created_at, last_modified, or alphabetical, each asc/desc. Up/Down navigates without changing state; Left/Right applies actions.
- Decision: Ctrl+Up and Ctrl+Down jump to the previous/next undecided image.
- Alternatives: skip-only undecided items or random shuffle.
- Rationale: Predictable navigation and reproducible ordering.

**Startup path selection**
- Decision: If a CLI path is provided, start scanning there; otherwise present a directory selection modal. Ctrl+O opens a new directory and replaces the current run.
- Alternatives: CLI-only path selection.
- Rationale: Supports both terminal-driven and UI-driven workflows.

**Created-at interpretation**
- Decision: Use platform-native creation time when available; otherwise fall back to metadata that best reflects creation (e.g., birthtime on macOS, creation time on Windows, or last_modified on platforms lacking creation time).
- Alternatives: require EXIF timestamps or add a separate indexing step.
- Rationale: Targeting modern desktop OSes with standard filesystems; prioritize practical availability over strict semantic purity.

**Configurable action mapping**
- Decision: Left/right actions are configurable, supporting delete, keep, move, rename, and metadata edit, with an extensible action registry for future additions.
- Alternatives: fixed delete/keep mapping only.
- Rationale: Enables flexible workflows while keeping the UI minimal.

**Rename and move defaults**
- Decision: Rename uses a simple `prefix-number.ext` pattern with an auto-incrementing sequence; move targets are user-defined and default to paths relative to the current directory.
- Alternatives: arbitrary rename templates or absolute-only targets.
- Rationale: Keeps v1 simple while enabling common organization workflows.

**Image format scope and orientation**
- Decision: Support web-compatible formats (jpg/jpeg, png, gif, webp, heic) and honor EXIF orientation for display.
- Alternatives: limit to a subset or ignore orientation metadata.
- Rationale: Matches modern photo libraries and avoids confusing rotations.

**Metadata edits**
- Decision: Support a metadata edit action that updates metadata fields without changing image formats.
- Alternatives: read-only metadata or external editing tools.
- Rationale: Enables lightweight tagging and corrections during sorting.

**Component Breakdown**

```
┌───────────────────────────┐
│        UI Layer           │
│ Askama + Datastar events  │
└────────────┬──────────────┘
             │ commands / patches
┌────────────▼──────────────┐
│        Axum API           │
│ CQRS: commands + views    │
└────────────┬──────────────┘
             │
┌────────────▼──────────────┐
│     Domain Core           │
│ State machines + types    │
└────────────┬──────────────┘
             │
┌────────────▼──────────────┐
│   Filesystem Adapter      │
│ Scan, move, trash, apply  │
└───────────────────────────┘
```

- **UI Layer**: Askama templates with `data-*` attributes; Datastar handles event stream updates; minimal JS.
- **Axum API**: Command endpoints for user actions; projection endpoints / event stream for UI updates.
- **Domain Core**: In-memory data structures; per-image state machine; global app state and undo stack.
- **Filesystem Adapter**: Abstracted operations (scan, move, safe delete, apply queue).
- **Optional Shell**: Tauri wrapper for local web app; no dependency on Tauri for core runtime.

## Risks / Trade-offs

- **Image decoding cost** → Use lazy loading and/or background prefetch; avoid decoding entire directory at once.
- **Large directories** → Memory pressure from metadata and undo stack; mitigate with lightweight structs and bounded history.
- **Safe delete consistency across platforms** → Use a local trash/ folder by default; document optional destructive delete behavior.
- **No persistence** → Users lose progress on restart; offset with clear warning and optional “apply at end” flow.
- **Loopback port management** → Handle port collisions with ephemeral port selection and readiness checks.

## Migration Plan

- No data migration required. Initial rollout is local only.
- If Tauri is enabled, add a wrapper that points at the loopback server once it is ready.
- Optional future step: add persistence as a new capability if requirements change.

## Open Questions

None.
