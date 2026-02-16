# Tauri Build on macOS

This document tracks macOS prerequisites and build steps for PICLR with Tauri v2.

## Status

- Validated end-to-end by project testing.
- macOS validation was completed before Linux validation in this release cycle.

## Core Toolchain

Install Xcode Command Line Tools:

```bash
xcode-select --install
```

Install Rust and Tauri CLI:

```bash
rustup toolchain install stable
rustup default stable
cargo install tauri-cli --version '^2'
```

Verify:

```bash
cargo tauri -V
```

## Recommended System Packages

Homebrew (optional but common):

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

Potentially useful for assets and packaging workflows:

```bash
brew install pkg-config
```

## Build and Bundle

From repo root:

```bash
cargo tauri build
```

Expected output type on macOS typically includes:

- `.app` bundle
- `.dmg` installer (if enabled in Tauri bundle config)

## Notes

- Installing `cargo-tauri` does not replace Xcode CLT requirements; both are needed for Rust/native build toolchain support on macOS.
- If signing/notarization is required, add Apple Developer certificate and notarization credentials to your CI/local signing flow.
