## Purpose
TBD
## Requirements
### Requirement: Loopback web service
The system MUST serve the UI from a local loopback HTTP server.

#### Scenario: Start local server
- **WHEN** the application starts
- **THEN** a loopback HTTP endpoint is available for the UI

### Requirement: CLI path argument
The system MUST accept an optional directory path when invoked from the CLI and start the UI on that path.

#### Scenario: Start with path argument
- **WHEN** the user runs `piclr ~/images`
- **THEN** the initial scan uses `~/images`

### Requirement: Server-driven UI with CQRS
The system MUST accept command requests from the UI and respond with server-driven UI updates. Command endpoints under `/cmd/*` MUST return HTTP 204 for accepted commands, and read-model updates MUST be delivered through the Datastar stream endpoint at `GET /events`.

#### Scenario: Command updates UI
- **WHEN** the user submits a left decision command
- **THEN** the server updates state, responds with HTTP 204, and emits corresponding UI updates via the Datastar stream

#### Scenario: Stream endpoint uses GET
- **WHEN** the UI initializes its Datastar event stream connection
- **THEN** it opens `GET /events` and receives server-sent patch events

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

### Requirement: Embedded Datastar script
The system MUST serve Datastar as an embedded static asset from the local server.

#### Scenario: Load Datastar
- **WHEN** the UI loads in the browser or Tauri webview
- **THEN** datastar.js is served locally and initializes successfully

### Requirement: Optional Tauri shell
The system MUST allow running as a standalone loopback web app and MUST allow an optional Tauri shell to load the same loopback UI.

#### Scenario: Run without Tauri
- **WHEN** Tauri is not enabled
- **THEN** the user can open the loopback URL in a browser and use the app

### Requirement: Directory selection when no path is provided
The system MUST prompt for a directory when started without a CLI path argument.

#### Scenario: Start without path argument
- **WHEN** the user runs `piclr` with no arguments
- **THEN** the UI presents a directory selection modal before scanning

### Requirement: Selective image-stack entry patching
The system MUST support selective `PatchElement` updates for individual image-stack entries using server-provided entry data derived from queue state.

#### Scenario: Patch a single entry after queue update
- **WHEN** a command changes one queued image entry
- **THEN** the server emits a `PatchElement` update for only that entry without requiring a full image-stack re-render
