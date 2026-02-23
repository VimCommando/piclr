## ADDED Requirements

### Requirement: Tauri-only custom window chrome
The system MUST render a custom window chrome container when running in Tauri desktop mode with undecorated windows, and MUST NOT render that chrome in browser-only mode.

#### Scenario: Tauri mode renders window chrome
- **WHEN** the app is launched with Tauri enabled
- **THEN** the top-level UI includes a custom window chrome container above the main app content

#### Scenario: Web mode does not render window chrome
- **WHEN** the app is launched without Tauri
- **THEN** the custom window chrome container is absent and the existing web UI layout remains unchanged

### Requirement: Custom chrome window controls
The system MUST provide close, minimize, and maximize/restore controls in the custom window chrome and MUST map each control to the corresponding native Tauri window action.

#### Scenario: Minimize from custom chrome
- **WHEN** the user activates the custom minimize control
- **THEN** the Tauri window is minimized

#### Scenario: Toggle maximize or restore from custom chrome
- **WHEN** the user activates the custom maximize control
- **THEN** the Tauri window toggles between maximized and restored states

#### Scenario: Close from custom chrome
- **WHEN** the user activates the custom close control
- **THEN** the Tauri window closes

### Requirement: Draggable custom title region
The system MUST provide a draggable title region in the custom window chrome so users can reposition undecorated windows.

#### Scenario: Drag window from title region
- **WHEN** the user press-drags the custom title region in Tauri mode
- **THEN** the window begins native drag movement

### Requirement: Visible frame affordance for undecorated mode
The system MUST provide a visible frame affordance (border or equivalent padding/outline treatment) around content when native decorations are disabled.

#### Scenario: Undecorated window remains visually bounded
- **WHEN** the app runs with native decorations disabled
- **THEN** the UI shows a consistent visual frame around the app content
