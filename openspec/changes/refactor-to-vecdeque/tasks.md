## 1. Baseline and characterization

- [x] 1.1 Audit current `mod.rs` queue/cache paths (including `maintain_cache_window`) and document existing invariants for ordering, bounds, and entry replacement.
- [x] 1.2 Add characterization tests that lock current externally visible behavior for queue mode, apply-at-end ordering, and window bound behavior.
- [x] 1.3 Add UI-level characterization tests (or equivalent integration checks) for image-stack full render and current single-entry update behavior.

## 2. Queue and cache-window refactor

- [x] 2.1 Replace internal queue storage with `VecDeque` while preserving existing public interfaces and command flow.
- [x] 2.2 Refactor cache-window maintenance to bounded deque transforms (insert one side, evict opposite side) and remove map-based authority for window membership.
- [x] 2.3 Ensure queue order is canonical for window derivation and apply-at-end execution order.
- [x] 2.4 Add/adjust unit tests for deterministic queue ordering, bounded eviction, and per-image queued action replacement.

## 3. Image-stack rendering and patch flow

- [x] 3.1 Define a single queue-backed projection object for image-stack rendering on the server.
- [x] 3.2 Update Askama image-stack templates to render from that projection for full-stack output.
- [x] 3.3 Implement selective `PatchElement` emission for single image-stack entries sourced from queue-derived entry data.
- [x] 3.4 Add parity tests that assert full-stack and single-entry render paths produce equivalent markup/data bindings for the same entry.

## 4. Integration, cleanup, and validation

- [x] 4.1 Run end-to-end flows covering decision changes, queue updates, apply-at-end, and UI patch synchronization.
- [x] 4.2 Remove transitional compatibility helpers and dead code from legacy map-based cache-window coordination.
- [x] 4.3 Validate no public API regressions and update internal docs/comments for new queue-as-source-of-truth semantics.
- [x] 4.4 Execute full test suite and fix regressions before implementation handoff.

## 5. CQRS and State Simplification Follow-ups

- [x] 5.1 Introduce an explicit backend queue collection (`queued_ids: VecDeque<u64>`) and stop deriving queue order from `images + order + queued_action` scans.
- [x] 5.2 Split write-model command state from read-model UI projection state, with command handlers updating domain state and projection updates feeding UI rendering.
- [x] 5.3 Route all state mutations through explicit command handlers (e.g., `Decide`, `Undo`, `ApplyQueue`, `SelectImage`) and remove ad hoc mutation paths.
- [x] 5.4 Shift from repeated full-view recomputation to incremental projection updates for counters, queue summaries, and stack-window UI state.
- [x] 5.5 Treat image scan output as a versioned immutable snapshot and reconcile decision/queue metadata by image id on rescan.
- [x] 5.6 Simplify modal state handling by replacing `view_stack` with `active_modal: Option<ModalView>` when single-modal behavior is sufficient, or enforce strict stack semantics otherwise.
