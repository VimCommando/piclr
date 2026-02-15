## Context

The current action queue and cache-window maintenance logic in `mod.rs` introduces more state coordination than needed for an ordered sequence. Queue-window behavior is currently harder to verify and reason about because the implementation spreads responsibility across multiple structures.

This change keeps behavior stable while simplifying internals around deque semantics. The queue order becomes the canonical state for both action processing and image-stack UI updates. The proposal also expands scope to align local web UI rendering with this model: render from one queue-backed object and patch single entries selectively.

Constraints:
- Preserve externally visible behavior and API contracts.
- Avoid introducing new dependencies.
- Maintain parity for ordering and update semantics under existing tests and expected UI flows.

## Goals / Non-Goals

**Goals:**
- Replace queue internals with `VecDeque` and define queue/window maintenance using native deque operations.
- Remove map-based cache-window coordination where queue ordering already defines correct behavior.
- Ensure image-stack rendering is driven by a single queue-backed object.
- Support selective `PatchElement` updates for single image-stack entries without full re-render.
- Improve readability and testability of queue/cache + UI synchronization logic.

**Non-Goals:**
- Changing public API shape for queue consumers.
- Redesigning the entire UI template system beyond queue/image-stack paths.
- Introducing cross-process caching or persistence changes.
- Broad performance tuning outside queue/cache and related rendering flows.

## Decisions

1. Queue is canonical and uses `VecDeque`
Rationale:
- Queue operations needed by cache-window maintenance are naturally represented as `push_front`/`pop_back` or `push_back`/`pop_front`.
- `VecDeque` avoids front-removal penalties and removes the need for index-shifting logic associated with `Vec`.

Alternatives considered:
- Keep `Vec` with manual window management: rejected due to complexity and poorer ergonomics for front operations.
- Keep hash map as cache authority plus queue as index: rejected because it duplicates state authority and increases sync risk.

2. Cache-window maintenance is expressed as bounded deque transforms
Rationale:
- Window updates become deterministic: insert on one side, evict on the opposite side when capacity is exceeded.
- Fewer mutable structures reduce edge cases and simplify invariants.

Alternatives considered:
- Retain `maintain_cache_window` with map reconciliation steps: rejected; complexity remains high for the same behavior.
- Build a custom ring buffer abstraction: rejected for now; `VecDeque` already provides required semantics.

3. Image-stack render source is a single queue-backed view model
Rationale:
- Rendering from one object lowers coupling between data assembly and template rendering.
- Prevents drift between queue state and rendered stack representation.

Alternatives considered:
- Keep multi-source render inputs (queue + cache map + derived fragments): rejected due to synchronization complexity.

4. Selective `PatchElement` updates target single entries pulled from queue state
Rationale:
- Enables incremental UI updates for entry-level changes.
- Preserves responsiveness while avoiding full image-stack re-renders for small mutations.

Alternatives considered:
- Always full re-render of image-stack: rejected due to unnecessary work for small updates.
- Fine-grained patching sourced from non-queue cache structures: rejected because queue should remain source of truth.

5. Behavior parity is enforced with focused tests
Rationale:
- Refactor changes internal mechanics; tests must guard against ordering regressions and UI drift.
- Priority cases: push/pop ordering, bounded-window eviction, and selective patch correctness.

Alternatives considered:
- Rely on existing tests only: rejected because current coverage may not encode deque-specific invariants explicitly.

## Risks / Trade-offs

- [Risk] Hidden assumptions in existing `maintain_cache_window` behavior are lost during simplification.
  -> Mitigation: capture current behavior with characterization tests before final deletion/refactor of old logic.

- [Risk] Queue-driven source-of-truth may expose previously masked ordering bugs in UI updates.
  -> Mitigation: add integration tests validating queue mutation -> rendered output -> patch update sequence.

- [Risk] Selective patch paths can diverge from full-render output.
  -> Mitigation: enforce shared entry rendering helpers and parity tests between full render and single-entry patch output.

- [Trade-off] Removing map-based coordination may reduce ad hoc lookup convenience.
  -> Mitigation: use queue-index helpers only where required, keep lookups localized, and avoid reintroducing dual authority.

- [Trade-off] Initial migration introduces temporary code churn across queue and UI modules.
  -> Mitigation: stage changes behind internal adapter functions and remove transitional shims after test parity.

## Migration Plan

1. Baseline and characterize current behavior with tests around queue ordering, window bounds, and image-stack update patterns.
2. Introduce `VecDeque`-backed queue internals behind existing interfaces.
3. Refactor cache-window logic to bounded deque operations and remove map-based authority from this path.
4. Update image-stack render path to consume a single queue-backed view model.
5. Implement selective `PatchElement` updates for single entries sourced from queue state.
6. Run parity tests and regressions; remove temporary compatibility helpers.
7. Rollback strategy: if regressions surface, revert to pre-refactor queue/cache implementation commit while preserving added characterization tests.

## Open Questions

- Should queue-window direction be standardized as `push_front/pop_back` or `push_back/pop_front`, or does it vary by call site?
- Are there call sites relying on implicit O(1) key-based lookup from current map logic that need explicit replacement helpers?
- What is the minimal stable entry identity needed for selective `PatchElement` updates to avoid stale DOM targets?
