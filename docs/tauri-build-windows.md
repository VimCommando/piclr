# Tauri Build on Windows

This document tracks Windows prerequisites and build steps for PICLR with Tauri v2.

## Status

- Not yet validated end-to-end in this project workspace.
- Treat this as a baseline checklist until a full local Windows build run is recorded.

## Target Support

- Minimum supported Windows version: **Windows 10**.
- Windows 11 is expected to work but is not the minimum compatibility baseline.

## Core Toolchain

Install:

- Rust (MSVC toolchain)
- Microsoft Visual Studio C++ Build Tools (or full Visual Studio with Desktop C++ workload)
- WebView2 runtime (usually preinstalled on modern Windows; install manually if missing)

Install Tauri CLI:

```powershell
cargo install tauri-cli --version '^2'
```

Verify:

```powershell
cargo tauri -V
```

## Build and Bundle

From repo root (PowerShell):

```powershell
cargo tauri build
```

Expected output type on Windows typically includes:

- `.msi`
- NSIS installer (`.exe`) if configured

## Notes

- Ensure Rust is using an MSVC target (not GNU) for standard Tauri Windows builds.
- If packaging fails, confirm Build Tools install includes:
  - MSVC compiler toolset
  - Windows SDK
  - CMake/Ninja components (recommended for some native crates)
- If WebView initialization fails at runtime, install or repair Microsoft Edge WebView2 Runtime.
