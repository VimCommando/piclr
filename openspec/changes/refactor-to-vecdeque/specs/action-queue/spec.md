## MODIFIED Requirements

### Requirement: Queue mode with one action per image
The system MUST support an optional queue mode where each image has at most one queued action, and a new decision replaces any prior queued action for that image. The queue/window representation for queued items MUST use ordered deque semantics, and queue order MUST be the source of truth for window membership and eviction order.

#### Scenario: Replace a queued action
- **WHEN** queue mode is enabled and the user changes an image decision
- **THEN** the previous queued action for that image is replaced by the new action

#### Scenario: Window updates use deque ordering
- **WHEN** a queue-window update adds an item at one end and the window exceeds its maximum size
- **THEN** the system evicts one item from the opposite end and preserves deterministic ordering in the remaining window

### Requirement: Apply all queued actions at end
The system MUST apply all queued actions when the user triggers the apply-at-end operation. The execution order MUST follow the current queue order at apply time.

#### Scenario: Apply queued actions
- **WHEN** the user triggers apply-at-end
- **THEN** all queued actions are executed and the queue is cleared

#### Scenario: Apply order follows queue order
- **WHEN** queued actions exist in a specific queue order
- **THEN** apply-at-end executes actions in that same order

## ADDED Requirements

### Requirement: Queue-backed cache window state
The system MUST maintain cache-window state from a single ordered queue representation and MUST NOT require a separate map-based authority to reconcile window membership.

#### Scenario: Derive window from queue
- **WHEN** the queue state is updated
- **THEN** the cache window is derived directly from queue order and configured window bounds
