# Tauri Testing Checklist

Use this checklist for release validation of desktop behavior.

## Automated Checks

- [ ] `cargo fmt --check`
- [ ] `cargo check`
- [ ] `cargo check --features tauri`
- [ ] `cargo test`
- [ ] `cargo test --features tauri tauri_render_includes_custom_window_chrome -- --nocapture`

## Manual Checks (Desktop Session)

- [ ] Launch with `cargo run --features tauri -- /path/to/images`
- [ ] Verify custom titlebar renders and drag works.
- [ ] Verify double-click titlebar toggles maximize/restore.
- [ ] Verify minimize, maximize/restore, and close controls work.
- [ ] Verify window menu opens and closes on outside click.
- [ ] Verify `Open Location` from menu opens native picker and switches root/current directory.
- [ ] Verify `Ctrl+O` performs the same open-location action.
- [ ] Verify `F` toggles files sidebar visibility (and does not open picker).
- [ ] Verify queue/help interactions still work (`Q`, `?`, `Esc`).
- [ ] Verify key help modal matches current keymap text.

## Notes

- Record X11 and Wayland results separately when both are supported.
- If manual behavior differs by platform/compositor, document exact environment details.
