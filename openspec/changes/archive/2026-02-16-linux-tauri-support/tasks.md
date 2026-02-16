## 1. Linux Support Baseline and Dependency Wiring

- [x] 1.1 Define Debian-family, Fedora-family, and Arch-family Linux support scope in user-facing docs
- [x] 1.2 Verify and document Linux build/runtime prerequisites for Tauri mode (including webview-related packages)
- [x] 1.3 Ensure Cargo/Tauri feature wiring for desktop mode remains optional and buildable on Linux

## 2. Linux Runtime Checks and Error Diagnostics

- [x] 2.1 Add Linux desktop startup prerequisite checks in Tauri bootstrap paths
- [x] 2.2 Implement actionable runtime error messages for missing or incompatible Linux prerequisites
- [x] 2.3 Ensure diagnostic output references concrete remediation steps aligned with distro-family docs

## 3. Linux Desktop Mode Behavior Parity

- [x] 3.1 Verify loopback UI startup path in Linux Tauri shell remains consistent with browser mode semantics
- [x] 3.2 Validate folder picker invocation (`Ctrl+O`) works in Linux desktop mode
- [x] 3.3 Validate folder switching replaces active run context and refreshes UI correctly on Linux

## 4. X11 and Wayland Validation

- [x] 4.1 Validate Linux desktop startup and usability in an X11 session
- [x] 4.2 Validate Linux desktop startup and usability in a Wayland session
- [x] 4.3 Capture known environment caveats and expected behavior differences, if any, in docs

## 5. Documentation and Verification

- [x] 5.1 Update README with Linux desktop prerequisites and troubleshooting for Debian/Fedora/Arch families
- [x] 5.2 Add or update tests/checks for Linux-specific startup error handling paths where feasible
- [x] 5.3 Run end-to-end manual verification for Linux desktop workflows and record results
