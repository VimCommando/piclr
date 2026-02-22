## Context

PICLR is a local-first app where UI, server, and filesystem are co-located, typically under a Tauri wrapper. The command side intentionally returns HTTP 204 from `/cmd/*`, while read-model updates are streamed through Datastar SSE on `/events`. Recent audit findings identified correctness risks in four areas: holding state write locks across filesystem `await` calls, stream lag without deterministic resync, Datastar request auto-cancellation during rapid keyboard commands, and non-standard SSE transport method usage.

## Goals / Non-Goals

**Goals:**
- Preserve CQRS behavior where commands remain side-effecting `204` responses and UI updates flow through `/events`.
- Eliminate lock contention caused by awaiting filesystem operations while holding `AppState` write locks.
- Guarantee that lagged SSE subscribers receive a full read-model snapshot to restore consistency.
- Prevent loss of rapid keyboard command intent due to Datastar default request cancellation.
- Use `GET /events` as the canonical SSE endpoint while preserving Datastar compatibility and Tauri behavior.
- Add tests that capture stream lag recovery and command burst dispatch behavior.

**Non-Goals:**
- Introducing a separate REST read API or abandoning server-driven Datastar patching.
- Adding a client-side state manager or JavaScript framework.
- Changing action semantics (left/right decision mapping, queue mode semantics, apply behavior).
- Supporting distributed multi-host synchronization.

## Decisions

1. Split command mutation from filesystem execution in immediate-apply paths.
- Decision: capture all state needed for filesystem work while under lock, release lock, perform filesystem `await`, then reacquire lock only for minimal post-IO updates (undo entry, projections as needed).
- Rationale: avoids long write-lock holds and keeps command processing responsive even with local disk variance.
- Alternative considered: keep lock and rely on local low latency. Rejected because lock duration remains nondeterministic and can stall sequential key commands.

2. Add explicit resync-on-lag behavior for SSE consumers.
- Decision: on `broadcast::RecvError::Lagged(_)`, emit a full `UiPatch::ALL` event set to that subscriber before continuing normal incremental events.
- Rationale: maintains read-model correctness after dropped broadcast items, matching CQRS expectation that stream reflects authoritative state.
- Alternative considered: increase channel size only. Rejected because it reduces probability of lag but cannot guarantee correctness.

3. Make rapid keyboard commands non-canceling.
- Decision: set Datastar `requestCancellation: 'none'` for keyboard-triggered `@post('/cmd/*')` actions.
- Rationale: ensures each keypress command is delivered in order of dispatch without auto-aborting prior requests from the same element.
- Alternative considered: move listeners to per-button elements only. Rejected because keyboard shortcuts remain centralized and still need deterministic dispatch.

4. Standardize stream endpoint to `GET /events`.
- Decision: change router and Datastar init from PATCH to GET for the long-lived event stream endpoint.
- Rationale: aligns with SSE conventions and future infrastructure compatibility while preserving same event payload semantics.
- Alternative considered: keep PATCH due to local deployment. Rejected to avoid avoidable transport divergence.

5. Extend tests at the web boundary.
- Decision: add integration-style tests that simulate lag recovery behavior and verify command burst expectations in transport wiring.
- Rationale: these issues are behavioral and regression-prone without explicit tests.
- Alternative considered: rely on manual validation. Rejected due to fragility.

## Risks / Trade-offs

- [Risk] Increased code complexity around lock scoping in command handlers.
  - Mitigation: isolate lock-splitting logic in focused helper routines and unit tests.
- [Risk] Full resync on lag may emit a larger patch set than incremental updates.
  - Mitigation: trigger only on lag events and retain incremental patch flow otherwise.
- [Risk] Non-canceling keyboard requests may increase short bursts of command processing.
  - Mitigation: local single-user environment and lightweight command handlers keep this manageable.
- [Risk] GET migration can break if any hard-coded PATCH assumptions remain.
  - Mitigation: update router + template together and cover with endpoint wiring tests.
