## Context

PICLR currently runs in a Tauri shell over a loopback-served UI. To avoid forced client-side decorations in some Linux/Wayland setups, the main window is now configured as undecorated (`decorations: false`). That resolves unwanted toolkit-drawn title bars but removes native window controls and frame affordances.

The UI stack already uses Askama templates and Datastar attributes with minimal client-managed state. This change must preserve existing web-service behavior while introducing Tauri-only chrome affordances in desktop mode.

## Goals / Non-Goals

**Goals:**
- Restore close/minimize/maximize behavior in undecorated Tauri windows.
- Provide a draggable title region so users can reposition the window.
- Provide visible border/padding/frame affordances for undecorated mode.
- Keep browser mode unchanged and avoid introducing Tauri dependencies into non-Tauri execution paths.

**Non-Goals:**
- Re-implement full platform-native theming or menu bars.
- Replace all keyboard/window-manager shortcuts.
- Add deep window state persistence beyond current app behavior.

## Decisions

1. Add a top-level `<window-chrome>` wrapper in the page template, rendered only when running in Tauri mode.
- Rationale: keeps desktop-specific UI isolated and prevents regressions in pure web mode.
- Alternative considered: always render chrome and hide via CSS/runtime checks. Rejected because it risks DOM/event complexity and accidental exposure in web mode.

2. Expose explicit Tauri commands for `minimize`, `toggle_maximize`, `close`, and drag-start behavior.
- Rationale: command-based integration preserves server-driven UI while delegating native window operations to Tauri.
- Alternative considered: JS-only plugin calls from static assets. Rejected because current architecture favors explicit command surfaces and testable command handlers.

3. Add lightweight chrome styles (padding, border, title row, controls) in existing frontend CSS.
- Rationale: minimal visual system change with clear boundary around window controls.
- Alternative considered: separate theme package/component framework. Rejected as unnecessary scope for this targeted behavior.

4. Keep undecorated window mode enabled in Tauri config and rely on in-app chrome for controls.
- Rationale: undecorated mode is the stable workaround for Wayland CSD behavior observed in this environment.
- Alternative considered: runtime toggling between decorated/undecorated based on display server. Rejected for complexity and inconsistent compositor behavior.

## Risks / Trade-offs

- [Risk] Custom chrome may feel less native than WM decorations. -> Mitigation: keep visual treatment simple and consistent with app styling.
- [Risk] Drag behavior can be compositor-sensitive. -> Mitigation: implement drag via Tauri-supported window drag API and validate on both X11 and Wayland.
- [Risk] Window controls could render in web mode by mistake. -> Mitigation: gate rendering on explicit Tauri runtime signal and include tests for both modes.
- [Risk] Accessibility regressions for custom controls. -> Mitigation: use semantic buttons, labels, and keyboard-focus-visible styles.

## Migration Plan

1. Add Tauri runtime signal to initial page model/render context.
2. Add chrome wrapper template and controls behind the Tauri gate.
3. Add Tauri command handlers for minimize/maximize/close/drag and connect UI control events.
4. Keep `decorations: false` in Tauri config and update docs for custom chrome behavior.
5. Validate flows in Wayland and X11 and run regression tests for web mode.

Rollback:
- Re-enable native decorations by setting `decorations: true` and remove the chrome wrapper/commands if severe issues are found.

## Open Questions

- Should maximize use a toggle button state indicator (maximize vs restore icon) in v1, or is a single toggle affordance acceptable?
- Should the custom border thickness vary by platform density/theme, or remain fixed for consistency?
