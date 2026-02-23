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

### 4.1 Open Location (`Ctrl+O`)

Steps:
1. Launch desktop mode.
2. Press `Ctrl+O`.
3. In the native folder picker, select a different directory.

Expected:
- Native folder picker opens.
- Selecting a directory updates active root/current directory.
- Sidebar and image list refresh to the selected location.

Actual:
- Dialog behavior:
- Root update behavior:
- Refresh behavior:

### 4.2 Files Sidebar Toggle (`F`)

Steps:
1. Launch desktop mode.
2. Press `F`.
3. Press `F` again.

Expected:
- Files sidebar visibility toggles open/closed.
- No native folder dialog opens from `F`.

Actual:
- Toggle behavior:
- Dialog behavior:

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

| Session | Startup OK | Ctrl+O Open Location OK | Root Switch Refresh OK | F Toggle Files Sidebar OK | Notes |
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
