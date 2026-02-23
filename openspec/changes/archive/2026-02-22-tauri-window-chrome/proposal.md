## Why

On Wayland, disabling native decorations is the only reliable way to avoid GTK client-side decorations in this app, but that removes close/minimize/maximize controls and visual frame affordances. We need a Tauri-only in-app chrome so undecorated windows remain usable and discoverable.

## What Changes

- Add a Tauri-only window chrome wrapper rendered above existing app content when running in desktop mode.
- Provide window controls in the custom chrome for minimize, maximize/restore toggle, and close.
- Provide a draggable title area so the undecorated window can be moved.
- Add subtle in-app border/padding styling to replace lost native frame affordances when decorations are disabled.
- Keep browser/web-service mode unchanged with no custom window chrome.

## Capabilities

### New Capabilities
- `tauri-window-chrome`: Tauri-only custom chrome behavior and controls for undecorated desktop windows.

### Modified Capabilities
- `local-web-ui`: Add runtime behavior that conditionally renders desktop chrome only in Tauri mode while preserving web-mode UI behavior.

## Impact

- Affected specs:
  - New: `openspec/changes/tauri-window-chrome/specs/tauri-window-chrome/spec.md`
  - Delta: `openspec/changes/tauri-window-chrome/specs/local-web-ui/spec.md`
- Affected frontend templates/styles for a top-level wrapper and control row.
- Affected Tauri command surface to expose minimize/maximize/close and drag operations.
- Affected keyboard/mouse interaction handling for draggable titlebar and control buttons.
