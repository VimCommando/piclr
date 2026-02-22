## Why

PICLR runs as a co-located loopback app, but command/event consistency still relies on robust local ordering and stream recovery behavior. The current implementation can lose rapid command intents and can leave the UI stale after stream lag, which conflicts with the expected CQRS model where commands return `204` and state changes are reflected through `/events`.

## What Changes

- Ensure command handlers do not hold the write lock across filesystem `await` points so UI command processing is not blocked during local IO.
- Add deterministic recovery behavior when SSE consumers lag, including a full read-model resync event set to restore UI consistency.
- Harden frontend Datastar command dispatch so rapid keyboard command bursts are not dropped by request auto-cancellation.
- Standardize the SSE endpoint on `GET /events` while keeping existing Datastar stream semantics and Tauri compatibility.
- Add focused tests for SSE lag recovery and command burst behavior expectations.

## Capabilities

### New Capabilities
- `local-cqrs-stream-recovery`: Defines read-model resync guarantees when the `/events` stream falls behind or reconnects.

### Modified Capabilities
- `local-web-ui`: Update command/event transport requirements for `GET /events`, command delivery behavior, and stream consistency guarantees.
- `image-review`: Update keyboard command handling requirements to preserve rapid key intent dispatch in Datastar-driven interactions.

## Impact

- Affected code: `/Users/reno/Development/piclr/src/web/mod.rs`, `/Users/reno/Development/piclr/src/app.rs`, `/Users/reno/Development/piclr/templates/index.html`.
- API surface: local command/read-model endpoints (`/cmd/*`, `/events`) and Datastar event wiring.
- Testing: add or expand web-level tests for stream lag resync and keyboard burst command behavior.
- Dependencies: no new external dependencies required.
