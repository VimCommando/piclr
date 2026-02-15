## Queue/Cache Audit (`mod.rs`)

### Observed invariants before refactor

- Queue mode stores at most one queued action per image via `ImageEntry.queued_action: Option<ActionConfig>`.
- Re-deciding an image replaces that image's queued action, not an additional queue entry.
- Apply-at-end previously iterated `images` storage order and cleared all queued actions afterward.
- Stack visibility is driven by `order` + `cursor` windowing, with a bounded radius around the cursor.
- Single-entry stack updates are emitted via `PatchElements::Outer` against `#image-card-{id}` when a changed entry is visible.

### Refactor targets aligned to invariants

- Keep one queued action per image while making queue execution order canonical from `order`.
- Keep bounded stack window behavior while simplifying cache-window maintenance.
- Keep selective single-entry patch behavior, reusing the same card rendering path as full-stack patches.
