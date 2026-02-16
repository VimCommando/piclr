## MODIFIED Requirements

### Requirement: Optional Tauri shell
The system MUST allow running as a standalone loopback web app and MUST allow an optional Tauri shell to load the same loopback UI. For Linux desktop mode, supported top-level environments MUST include Debian-family, Fedora-family, and Arch-family distributions, and desktop execution MUST work in both X11 and Wayland sessions through Tauri's webview backend.

#### Scenario: Run without Tauri
- **WHEN** Tauri is not enabled
- **THEN** the user can open the loopback URL in a browser and use the app

#### Scenario: Run with Tauri on supported Linux families
- **WHEN** a user launches desktop mode on a supported Linux family distribution
- **THEN** the Tauri shell loads the same loopback UI used by browser mode

#### Scenario: Run with Tauri on X11 and Wayland
- **WHEN** a user launches desktop mode in either an X11 or Wayland Linux session
- **THEN** the application starts and presents a usable desktop UI
