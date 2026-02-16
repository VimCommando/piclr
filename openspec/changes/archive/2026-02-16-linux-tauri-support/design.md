## Context

PICLR already runs as a loopback web service and optionally as a Tauri desktop shell. The desktop mode is valuable on Linux because it enables native folder selection and in-app folder switching, but Linux support is currently implicit rather than explicitly specified. This change defines Linux support boundaries and implementation expectations without altering the core loopback architecture.

Current constraints:
- Core behavior must remain identical between browser mode and Tauri mode.
- Tauri remains optional (`--features tauri`) and must not become a hard runtime dependency.
- Linux environments vary (Wayland/X11, distro-provided WebKitGTK versions, missing packaging tools), so behavior and failure modes must be explicit.

## Goals / Non-Goals

**Goals:**
- Define clear Linux requirements for building, running, and packaging the Tauri shell.
- Ensure Linux desktop mode consistently supports startup, folder selection, and folder switching.
- Keep platform-specific concerns isolated to Tauri shell and startup wiring.
- Improve diagnostics and documentation when Linux prerequisites are missing.

**Non-Goals:**
- Re-architecting the loopback server, domain state, or Datastar/Askama rendering model.
- Adding new cross-platform desktop features unrelated to Linux support.
- Changing browser-only mode behavior.
- Supporting every Linux packaging ecosystem in this change.

## Decisions

**Linux support is an explicit capability, not an incidental side effect**
- Decision: Introduce a dedicated `linux-tauri-support` capability and spec to codify required behavior.
- Alternative considered: only update README/docs without specs.
- Rationale: Behavioral expectations (startup reliability, folder dialogs, failure messaging) need testable requirements.

**Preserve loopback-first runtime for Linux desktop mode**
- Decision: Linux Tauri continues loading the same local Axum loopback UI used by browser mode.
- Alternative considered: Linux-specific embedded protocol path.
- Rationale: avoids platform divergence and keeps server state as the single source of truth.

**Constrain platform branching to shell/bootstrap boundaries**
- Decision: Linux-specific conditionals live in Tauri bootstrap and shell integration paths, not domain logic.
- Alternative considered: broader `cfg(target_os = "linux")` branches across app modules.
- Rationale: reduces maintenance risk and prevents behavioral drift in core features.

**Fail fast with actionable diagnostics for missing Linux prerequisites**
- Decision: detect and surface clear guidance when Tauri runtime prerequisites are unavailable on Linux.
- Alternative considered: generic startup failure or opaque panic output.
- Rationale: Linux setup variance makes explicit error messaging necessary for supportability.

**Treat Linux desktop flows as parity requirements**
- Decision: folder picker (`Ctrl+O`) and folder switching must function on Linux with the same user-visible semantics as other desktop targets.
- Alternative considered: best-effort Linux behavior with weaker guarantees.
- Rationale: these are core desktop-mode workflows and should not be platform-optional.

**Scope packaging expectations to supported baseline outputs**
- Decision: define required Linux packaging/build outputs for this repo’s current Tauri configuration, and defer broad format expansion to future changes.
- Alternative considered: immediate support for every Linux package format/toolchain.
- Rationale: keeps the change deliverable while still making Linux support concrete.

## Risks / Trade-offs

- [Linux distro dependency fragmentation] -> Mitigation: document required runtime/build dependencies and include troubleshooting guidance.
- [Wayland/X11 or WebKitGTK environment differences] -> Mitigation: define expected behavior in requirements and validate on representative Linux setups.
- [Overly broad Linux support claims] -> Mitigation: explicitly state support boundaries in specs and docs.
- [Additional CI/runtime matrix cost] -> Mitigation: start with targeted Linux verification focused on Tauri startup and desktop-only flows.

## Migration Plan

1. Add/modify specs for `linux-tauri-support` and `local-web-ui` to formalize Linux desktop requirements.
2. Implement Linux-focused Tauri startup and error-handling adjustments in shell/bootstrap paths.
3. Update README/Linux docs with prerequisite and troubleshooting sections.
4. Validate Linux desktop flows and bundling/build expectations, then finalize tasks.

Rollback strategy:
- If Linux-specific shell changes regress startup, disable or gate those changes while retaining spec/docs artifacts for iterative follow-up.

## Open Questions

- Which Linux distributions/windowing combinations will be considered the initial supported baseline?
- Should Linux prerequisite checks be compile-time guidance only, runtime checks, or both?
