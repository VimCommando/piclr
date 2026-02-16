# Linux Tauri Validation Template

Use this template to validate PICLR desktop mode on Linux targets.
Keep each run result in version control (or CI artifacts) so regressions are traceable.

## 1. Run Metadata

- Date:
- Commit/branch:
- Tester:
- Host distro/family:
- Session type: `X11` / `Wayland`
- Kernel:
- Rust version (`rustc -V`):
- Cargo version (`cargo -V`):

## 2. Prerequisite Check

Run:

```bash
pkg-config --modversion glib-2.0 gtk+-3.0 webkit2gtk-4.1
```

Expected:
- All three versions are returned.

Actual:
- glib-2.0:
- gtk+-3.0:
- webkit2gtk-4.1:

## 3. Build and Startup

Run:

```bash
cargo check --features tauri
cargo run --features tauri -- /path/to/images
```

Expected:
- Build succeeds.
- Desktop window opens and loads loopback UI.
- No fatal startup error about missing Linux prerequisites.

Actual:
- Build:
- Startup:
- Logs/errors:

## 4. Functional Desktop Validation

### 4.1 Folder Picker (`Ctrl+O`)

Steps:
1. Launch desktop mode.
2. Press `Ctrl+O`.
3. Select a different image directory.

Expected:
- Native folder dialog opens.
- Selection closes dialog and updates active directory in UI.

Actual:
- Dialog behavior:
- Directory update behavior:

### 4.2 Folder Switch via UI Buttons

Steps:
1. Click header folder button.
2. Click footer `Open` button.

Expected:
- Both paths mirror `Ctrl+O` behavior.
- Active run context switches and image list refreshes.

Actual:
- Header button:
- Footer button:

### 4.3 Basic Usability Smoke

Steps:
1. Navigate images (`J/K`, arrow keys).
2. Apply decisions (`H/L`, arrow keys).
3. Open queue/files/help modals.

Expected:
- UI remains responsive.
- No hard failure or broken loopback connection.

Actual:
- Navigation:
- Decisions:
- Modals:

## 5. Session-Specific Matrix

Fill this table for each validated session.

| Session | Startup OK | Ctrl+O OK | Header/Footer Open OK | Folder Switch Refresh OK | Notes |
|---|---|---|---|---|---|
| X11 |  |  |  |  |  |
| Wayland |  |  |  |  |  |

## 6. Automation Notes

Use this section to record scripts or CI jobs that can automate partial checks.

- Headless build check command:
  - `cargo check --features tauri`
- Unit/integration test command:
  - `cargo test`
- Candidate GUI automation command(s):
  - `<add script path here>`
- Known non-automatable items:
  - Native folder dialog interaction may require manual or specialized desktop automation harness.

## 7. Result

- Overall status: `PASS` / `FAIL` / `PARTIAL`
- Blocking issues:
  - `<issue list>`
- Follow-up tasks:
  - `<task list>`
