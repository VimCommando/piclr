## Why

PICLR already supports a Tauri desktop shell, but Linux usage is not clearly specified or consistently supported for building, bundling, and running the desktop app. Defining Linux-specific support now reduces setup friction and makes desktop usage reliable for Linux users.

## What Changes

- Define Linux runtime and packaging expectations for running PICLR with the Tauri shell.
- Add explicit requirements for Linux-compatible startup and desktop integration behavior.
- Document Linux-specific setup and troubleshooting guidance in project docs.
- Validate that existing desktop-only flows (folder picker and folder switching) behave correctly on Linux.

## Capabilities

### New Capabilities
- `linux-tauri-support`: Define and validate Linux support requirements for the Tauri desktop mode, including runtime prerequisites, app startup behavior, and packaging expectations.

### Modified Capabilities
- `local-web-ui`: Clarify optional Tauri shell requirements so Linux desktop behavior is explicitly covered for the same loopback UI workflow.

## Impact

- Affected code: Tauri shell integration paths (`src/tauri_shell.rs`, startup wiring in `src/main.rs` and related modules), plus any Linux-specific gating needed for desktop behaviors.
- Affected config: `tauri.conf.json`, Cargo feature/dependency setup, and any build/bundle scripts tied to Linux packaging.
- Affected docs: `README.md` installation and run instructions for Linux desktop mode.
- Affected systems: local Linux development and CI validation for `--features tauri` execution.
