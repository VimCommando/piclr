![PICLR icon](icons/icon.png)

# PICLR

PICLR (**PIC**ture **L**eft/**R**ight) lets you rapidly sort through large folders of images with simple `left` and `right` actions.

## Install

### Build from a git clone

```bash
git clone <your-fork-or-this-repo-url> piclr
cd piclr
cargo install --path .
```

Run as local web service:

```bash
piclr /path/to/images
```

This will log the URL for the local web service.

Run in the Tauri desktop shell:

```bash
cargo run --features tauri -- /path/to/images
```

### Install from crates.io

If/when the crate is published:

```bash
cargo install piclr
```

Then run:

```bash
piclr /path/to/images
```

### Install with Homebrew

If/when a formula is published:

```bash
brew install piclr
```

Then run:

```bash
piclr /path/to/images
```

### Runtime note: app vs web service

- Running with `--features tauri` launches PICLR as a desktop app.
- In Tauri mode, open/change root location from the titlebar menu (`Open Location`) or `Ctrl+O`.
- Running as the web service (`piclr ...` without Tauri) does not support changing folders from inside the UI. Choose the folder when launching the command.

### Platform Build Docs

- Linux: `docs/tauri-build-linux.md`
- macOS: `docs/tauri-build-macos.md`
- Windows: `docs/tauri-build-windows.md`

### Linux Tauri Desktop Support

Top-level Linux support targets for desktop mode are:

- Debian-family distributions
- Fedora-family distributions
- Arch-family distributions

PICLR desktop mode is expected to run in both X11 and Wayland sessions through Tauri's webview backend.

#### Linux prerequisites

Install desktop runtime dependencies before launching with `--features tauri`:

- Debian-family:
  ```bash
  sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0
  ```
- Fedora-family:
  ```bash
  sudo dnf install webkit2gtk4.1 gtk3
  ```
- Arch-family:
  ```bash
  sudo pacman -S webkit2gtk gtk3
  ```

Build and run:

```bash
cargo run --features tauri -- /path/to/images
```

With `decorations: false`, PICLR renders an in-app Tauri-only titlebar with drag, minimize, maximize/restore, and close controls.

#### Linux troubleshooting

- If startup reports that no display session is detected, launch from an X11/Wayland desktop session where `DISPLAY` or `WAYLAND_DISPLAY` is set.
- If startup reports missing WebKitGTK or GTK3 libraries, install the distro-family packages listed above and retry.
- If `Open Location` (menu or `Ctrl+O`) does not open a folder picker, verify you are running desktop mode (`--features tauri`) and not web-service mode.

#### Linux verification checklist

- Desktop shell starts and loads loopback UI in X11 session.
- Desktop shell starts and loads loopback UI in Wayland session.
- Titlebar menu `Open Location` or `Ctrl+O` opens native folder picker and selected folder replaces the active run context.

## How to use PICLR

Start PICLR with an image directory:

```bash
piclr /path/to/images
```

If no port is provided, PICLR binds to an available loopback port and logs the URL.

### Core workflow

1. Move through images.
2. Assign left/right decisions.
3. Undo if needed.
4. Apply queued actions when ready.

By default:

- Left action: move image to `trash/`
- Right action: move image to `keep/`

### Controls

#### Navigation

- `↑` / `K`: previous image
- `↓` / `J`: next image
- `Shift+↑` / `Shift+K`: previous undecided
- `Shift+↓` / `Shift+J`: next undecided

#### Decisions and actions

- `←` / `H`: apply left action
- `→` / `L`: apply right action
- `U`: undo last command
- `Shift+A`: apply queued actions

#### Modals and utility

- `Q`: show queued actions
- `F`: show/hide files sidebar
- `?`: show help
- `Esc`: close top modal
- `Ctrl+O`: open location (desktop app mode only)

You can also use the footer/header buttons for the same commands.

## DATA Stack Example

I'm using PICLR as a full end-to-end example of the [DATA
Stack](docs/data-stack.md):

- **Datastar** - Event-driven hypermedia interface
- **Askama** - Compile-time HTML templating
- **Tauri** - Native desktop application wrapper
- **Axum** - Bridging HTTP interface and state management

## Built with OpenAI Codex and OpenSpec

PICLR was developed with Codex as an exercise to push its limits in an emerging
technology stack.

It also leverages the
[OpenSpec](https://github.com/Fission-AI/OpenSpec/) framework for AI-assisted
development.

## License

MIT. See `LICENSE`.
