## Why

The current queue/cache flow appears over-engineered for a sequence-oriented problem, with maintenance logic that is harder to reason about than necessary. Simplifying around deque semantics now should reduce complexity, improve maintainability, and make rendering/data flow more direct.

## What Changes

- Refactor the action queue internals to use `VecDeque` as the primary storage type.
- Simplify cache-window behavior to deque-native operations instead of map-based coordination where queue ordering is the source of truth.
- Align image-stack rendering to consume a single queue-backed object via template rendering, with selective `PatchElement` updates for individual entries.
- Preserve external behavior while reducing internal moving parts and improving readability in queue/cache code paths.
- Add or update tests to verify ordering, window maintenance, and UI update behavior parity after simplification.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `action-queue`: Update requirements to define queue/window behavior in terms of deque operations and ordered queue state as the primary source of truth.
- `local-web-ui`: Update requirements for image-stack rendering and selective per-entry patch updates sourced from queue state.

## Impact

- Affected code: queue implementation modules and related tests in the Rust codebase.
- Affected code: queue/cache logic (including current cache-window maintenance), image-stack render/update paths, and related tests.
- APIs: no intended public API changes.
- Dependencies: no new external dependencies expected.
- Systems: lowers operational complexity and should improve predictability of queue-backed processing and UI synchronization.
