## MODIFIED Requirements

### Requirement: Minimal frontend state
The system MUST keep frontend state minimal and derive UI from server state. For image-stack rendering, the server MUST provide one queue-backed projection object as the canonical render source.

#### Scenario: Refresh UI state
- **WHEN** the UI reconnects to the server
- **THEN** the server provides the current projection needed to render the view

#### Scenario: Image-stack projection is canonical
- **WHEN** the server emits image-stack data
- **THEN** the UI derives stack rendering from a single queue-backed projection object rather than multiple independent sources

### Requirement: Askama templates with Datastar attributes
The system MUST render UI with Askama templates using `data-*` attributes compatible with Datastar. Image-stack rendering MUST support initial stack hydration and single-entry render paths that produce equivalent markup for the same entry data.

#### Scenario: Render initial page
- **WHEN** the UI loads the main page
- **THEN** the HTML includes Datastar-compatible `data-*` attributes for events and bindings

#### Scenario: Entry parity across render paths
- **WHEN** an image-stack entry is rendered via initial hydration output and via single-entry template path
- **THEN** both outputs are equivalent for that entry's DOM structure and data bindings

## ADDED Requirements

### Requirement: Selective image-stack entry patching
The system MUST support selective `PatchElement` updates for individual image-stack entries using server-provided entry data derived from queue state.

#### Scenario: Patch a single entry after queue update
- **WHEN** a command changes one queued image entry
- **THEN** the server emits a `PatchElement` update for only that entry without requiring a full image-stack re-render
