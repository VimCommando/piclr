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
The system MUST accept command requests from the UI and respond with server-driven UI updates. Command endpoints under `/cmd/*` MUST return HTTP 204 for accepted commands, and read-model updates MUST be delivered through the Datastar stream endpoint at `GET /events`. Directory navigation and directory actions (open, rename, delete) MUST be processed as server-validated root-relative commands.

#### Scenario: Command updates UI
- **WHEN** the user submits a left decision command
- **THEN** the server updates state, responds with HTTP 204, and emits corresponding UI updates via the Datastar stream

#### Scenario: Directory command updates UI
- **WHEN** the user submits a directory `open`, `rename`, or `delete` command from the sidebar
- **THEN** the server validates the target as a descendant of root, applies the command, responds with HTTP 204, and emits updated sidebar/image projections

#### Scenario: Stream endpoint uses GET
- **WHEN** the UI initializes its Datastar event stream connection
- **THEN** it opens `GET /events` and receives server-sent patch events

### Requirement: Minimal frontend state
The system MUST keep frontend state minimal and derive UI from server state. For image-stack rendering, the server MUST provide one queue-backed projection object as the canonical render source. For filesystem navigation, the server MUST provide a canonical sidebar projection that includes row type, root-relative path, and decision decoration metadata.

#### Scenario: Refresh UI state
- **WHEN** the UI reconnects to the server
- **THEN** the server provides the current projection needed to render the view

#### Scenario: Image-stack projection is canonical
- **WHEN** the server emits image-stack data
- **THEN** the UI derives stack rendering from a single queue-backed projection object rather than multiple independent sources

#### Scenario: Sidebar projection is canonical
- **WHEN** the server emits sidebar file-navigation data
- **THEN** the UI renders directory/file rows and decorations from that server projection without client-side filesystem derivation

### Requirement: Askama templates with Datastar attributes
The system MUST render UI with Askama templates using `data-*` attributes compatible with Datastar. Image-stack rendering MUST support initial stack hydration and single-entry render paths that produce equivalent markup for the same entry data. Sidebar row rendering MUST provide equivalent markup between initial hydration and patch updates and MUST replace the legacy image/file list modal as the navigation surface. In Tauri mode, the template layout MUST support a top-level desktop chrome wrapper while preserving equivalent render behavior for existing image stack and sidebar content.

#### Scenario: Render initial page
- **WHEN** the UI loads the main page
- **THEN** the HTML includes Datastar-compatible `data-*` attributes for events and bindings

#### Scenario: Entry parity across render paths
- **WHEN** an image-stack entry is rendered via initial hydration output and via single-entry template path
- **THEN** both outputs are equivalent for that entry's DOM structure and data bindings

#### Scenario: Sidebar row parity across render paths
- **WHEN** a sidebar row is rendered via initial hydration output and via patch update output
- **THEN** both outputs are equivalent for that row's DOM structure, data bindings, and decision decoration hooks

#### Scenario: Sidebar replaces image/file list modal
- **WHEN** the user accesses file navigation interactions
- **THEN** the UI uses the sidebar surface and does not render the legacy image/file list modal

### Requirement: Embedded Datastar script
The system MUST serve Datastar as an embedded static asset from the local server.

#### Scenario: Load Datastar
- **WHEN** the UI loads in the browser or Tauri webview
- **THEN** datastar.js is served locally and initializes successfully

### Requirement: Optional Tauri shell
The system MUST allow running as a standalone loopback web app and MUST allow an optional Tauri shell to load the same loopback UI. When running in Tauri mode with undecorated windows, the UI MUST expose desktop window controls and drag affordances via in-app chrome while web mode remains unchanged.

#### Scenario: Run without Tauri
- **WHEN** Tauri is not enabled
- **THEN** the user can open the loopback URL in a browser and use the app

#### Scenario: Tauri mode exposes in-app window controls
- **WHEN** Tauri is enabled and window decorations are disabled
- **THEN** the UI presents in-app controls for minimize, maximize/restore, and close plus a drag-capable title region

### Requirement: Empty-state no-images message
The system MUST present a clear empty-state message when no images are available in the currently selected directory.

#### Scenario: No images available
- **WHEN** the selected directory has zero supported images
- **THEN** the main image viewer shows a centered empty-state message indicating there are no supported images in the current directory

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
