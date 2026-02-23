# Tauri Build on Linux

This document captures the Linux prerequisites and build flow used to successfully build and bundle PICLR with Tauri v2.

## Status

- Validated in an Ubuntu 24.04 container (Distrobox) on February 16, 2026.
- Manual desktop validation completed for both X11 and Wayland sessions in this release cycle.
- Manual desktop workflow validation completed for `Ctrl+O`/titlebar `Open Location`, `F` sidebar toggle, and active-folder context switching.
- Ongoing release checks are tracked in `docs/tauri-testing-checklist.md`.
- Verified bundle outputs:
  - `.deb`
  - `.rpm`
  - `.AppImage`

## Core Toolchain

Install Rust and Cargo (if not already installed), then install Tauri CLI:

```bash
cargo install tauri-cli --version '^2'
```

Verify:

```bash
cargo tauri -V
```

## System Dependencies (Ubuntu 24.04)

The following package set was used to satisfy Tauri desktop and bundling requirements:

```bash
sudo apt-get update
sudo apt-get install -y \
  libglib2.0-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  pkg-config
```

Bundler tooling:

```bash
sudo apt-get install -y \
  patchelf \
  rpm \
  fakeroot \
  dpkg-dev \
  file \
  desktop-file-utils \
  appstream
```

Optional quality-of-life packages:

```bash
sudo apt-get install -y \
  libcanberra-gtk3-module \
  fonts-noto-color-emoji
```

- `libcanberra-gtk3-module` suppresses common GTK runtime module warnings.
- `fonts-noto-color-emoji` fixes missing emoji glyphs in UI text.

## Build and Bundle

From repo root:

```bash
cargo tauri build
```

Expected bundle outputs:

- `target/release/bundle/deb/piclr_0.1.0_amd64.deb`
- `target/release/bundle/rpm/piclr-0.1.0-1.x86_64.rpm`
- `target/release/bundle/appimage/piclr_0.1.0_amd64.AppImage`

## Install and Run (`.deb`)

```bash
sudo dpkg -i target/release/bundle/deb/piclr_0.1.0_amd64.deb
piclr /path/to/images
```

## Window Decorations (Linux)

PICLR is currently configured with `decorations: false`, so it uses an in-app custom titlebar in Tauri mode (drag region plus minimize, maximize/restore, close controls).

## Known Notes

- AppImage build may warn about missing AppStream metadata (`*.appdata.xml`). This is non-blocking for bundle generation.
- Some environments require running build/bundle commands with elevated permissions if sandbox restrictions block packaging helpers.
