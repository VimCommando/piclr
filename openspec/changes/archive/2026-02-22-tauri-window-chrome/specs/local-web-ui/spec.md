## MODIFIED Requirements

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

### Requirement: Optional Tauri shell
The system MUST allow running as a standalone loopback web app and MUST allow an optional Tauri shell to load the same loopback UI. When running in Tauri mode with undecorated windows, the UI MUST expose desktop window controls and drag affordances via in-app chrome while web mode remains unchanged.

#### Scenario: Run without Tauri
- **WHEN** Tauri is not enabled
- **THEN** the user can open the loopback URL in a browser and use the app

#### Scenario: Tauri mode exposes in-app window controls
- **WHEN** Tauri is enabled and window decorations are disabled
- **THEN** the UI presents in-app controls for minimize, maximize/restore, and close plus a drag-capable title region
