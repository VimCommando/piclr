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
The system MUST accept command requests from the UI and respond with server-driven UI updates.

#### Scenario: Command updates UI
- **WHEN** the user submits a left decision command
- **THEN** the server updates state and returns UI updates via the Datastar stream

### Requirement: Minimal frontend state
The system MUST keep frontend state minimal and derive UI from server state.

#### Scenario: Refresh UI state
- **WHEN** the UI reconnects to the server
- **THEN** the server provides the current projection needed to render the view

### Requirement: Askama templates with Datastar attributes
The system MUST render UI with Askama templates using `data-*` attributes compatible with Datastar.

#### Scenario: Render initial page
- **WHEN** the UI loads the main page
- **THEN** the HTML includes Datastar-compatible `data-*` attributes for events and bindings

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
