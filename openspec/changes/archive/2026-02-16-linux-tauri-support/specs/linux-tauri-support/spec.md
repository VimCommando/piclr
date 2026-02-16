## ADDED Requirements

### Requirement: Linux distro family support targets
The system MUST define Linux desktop support targets for Tauri mode as Debian-family, Fedora-family, and Arch-family distributions.

#### Scenario: Supported Linux target families are documented
- **WHEN** a user checks Linux desktop support guidance
- **THEN** documentation names Debian-family, Fedora-family, and Arch-family as top-level supported targets

### Requirement: Linux display server compatibility in Tauri mode
The system MUST support running the Tauri desktop shell on Linux environments using either X11 or Wayland through the webview stack used by Tauri.

#### Scenario: Start desktop shell on X11
- **WHEN** a Linux user launches `piclr` with `--features tauri` in an X11 session
- **THEN** the desktop window loads the loopback UI and remains usable for image review actions

#### Scenario: Start desktop shell on Wayland
- **WHEN** a Linux user launches `piclr` with `--features tauri` in a Wayland session
- **THEN** the desktop window loads the loopback UI and remains usable for image review actions

### Requirement: Linux desktop-mode workflow parity
The system MUST provide Linux parity for desktop-only workflows, including folder picker invocation and switching to a newly selected folder.

#### Scenario: Open folder picker on Linux
- **WHEN** the user triggers folder selection from Linux desktop mode (for example `Ctrl+O`)
- **THEN** a native folder selection dialog is shown

#### Scenario: Switch active folder on Linux desktop mode
- **WHEN** the user selects a different folder in Linux desktop mode
- **THEN** the application replaces the current run context with the newly selected folder and updates the UI for that folder

### Requirement: Linux prerequisite failure diagnostics
The system MUST provide actionable runtime error guidance when Linux Tauri prerequisites are missing or incompatible.

#### Scenario: Missing Linux prerequisite at startup
- **WHEN** desktop mode startup fails due to missing or incompatible Linux runtime prerequisites
- **THEN** the user is shown a clear error message describing the failure and what dependencies to install or verify

### Requirement: Linux setup documentation
The system MUST document Linux desktop prerequisites and troubleshooting for supported distro families.

#### Scenario: User follows Linux setup documentation
- **WHEN** a user follows Linux setup instructions for Debian-family, Fedora-family, or Arch-family distributions
- **THEN** they can identify required dependencies and troubleshooting steps before launching desktop mode
