# Mouse Interaction Reference

Flutter Demon supports mouse interaction in the terminal when `[ui] enable_mouse = true`
(the default). This document describes how the wheel scrolls each UI mode, the modifier
keys that change scroll behavior, and the platform caveats.

For the on/off setting, see `[ui] enable_mouse` in [CONFIGURATION.md](CONFIGURATION.md).

---

## Scroll Behavior by UI Mode

Wheel events route to the focused surface based on the current `UiMode`. There is **no
coordinate-based routing** — scrolling anywhere in the terminal scrolls the focused
surface (the log view, the settings list, the active DevTools panel, etc.). See
[Coordinate-Free Routing](#coordinate-free-routing) for details.

| Mode | Plain Wheel | Shift+Wheel |
|------|-------------|-------------|
| Normal (logs) | Scroll log line up/down | Page log up/down |
| Normal (tag-filter open) | Move tag-filter selection | Move tag-filter selection |
| LinkHighlight | Scroll log line up/down | Page log up/down |
| DevTools — Inspector | Tree row up/down | Tree row up/down (single-step; no page analogue) |
| DevTools — Performance | (no-op — use keyboard ← / → for frame navigation) | (no-op) |
| DevTools — Network | Request list up/down | Page request list up/down |
| DevTools — Network (filter input active) | (no-op — text input mode) | (no-op) |
| Settings (main list) | Item selection up/down | Item selection up/down |
| Settings (inline editing) | (no-op — text input mode) | (no-op) |
| Settings (dart-defines modal — list pane) | Dart-define selection up/down | Dart-define selection up/down |
| Settings (dart-defines modal — edit pane) | (no-op — text input mode) | (no-op) |
| Settings (extra-args modal) | Extra-args selection up/down | Extra-args selection up/down |
| NewSessionDialog / Startup (target-selector pane) | Device selection up/down | Device selection up/down |
| NewSessionDialog / Startup (launch-context pane) | Field focus prev/next | Field focus prev/next |
| NewSessionDialog (fuzzy modal open) | Fuzzy selection up/down | Fuzzy selection up/down |
| NewSessionDialog (dart-defines modal open) | Dart-define selection up/down | Dart-define selection up/down |
| FlutterVersion | Version selection up/down | Version selection up/down |
| SearchInput, Confirm, Loading, EmulatorSelector | (no-op — text input or system modal) | (no-op) |

### Modifier Key Rules

The exact behavior of `Ctrl`, `Alt`, and `Shift` modifiers depends on the mode:

**Modes that honor `Shift+Wheel` for page-step scrolling** (Normal, LinkHighlight,
DevTools/Network):

- `Shift+Wheel` → page up/page down
- `Ctrl+Wheel`, `Alt+Wheel`, `Ctrl+Shift+Wheel`, `Alt+Shift+Wheel` → **no-op**

This intentionally avoids conflict with common terminal-level `Ctrl+Wheel` font-zoom
bindings.

**Modes that ignore modifiers** (Settings, NewSessionDialog, FlutterVersion):

- All modifier combinations produce the same single-step navigation as plain wheel.
- There is no page-step analogue in these modes.

**DevTools — Inspector** (special case):

- Plain wheel, `Shift+Wheel` → single-step tree row navigation (no page-step analogue)
- `Ctrl+Wheel` alone, `Alt+Wheel` alone → no-op
- `Ctrl+Shift+Wheel`, `Alt+Shift+Wheel` → single-step navigation (Shift is prioritized)

> **Note on Inspector modifier behavior:** The Inspector's handling of
> `Shift+Ctrl+Wheel` (single-step rather than no-op) is a deliberate exception to the
> no-op rule used in Normal/Network modes. Because the Inspector has no page-step
> navigation, Shift-held scrolls fall through to single-step rather than becoming dead
> input.

### Horizontal Scroll

`ScrollDir::Left` and `ScrollDir::Right` (touchpad horizontal scroll) are **no-ops in
all modes**. Future phases may map horizontal scroll to log timeline panning or a
DevTools secondary-axis navigation.

---

## Coordinate-Free Routing

Wheel events are routed by `UiMode` only — the cursor position `(x, y)` does not affect
which surface receives the scroll. This means scrolling while hovering over the header,
status bar, or session tabs still scrolls the focused surface (e.g., the log view in
`Normal` mode).

This is a deliberate v1 simplification. A future phase will introduce region-based
hit-testing, at which point scroll routing may also become coordinate-aware.

**Practical implication:** If you hover over a session tab and scroll, the log view
scrolls — the tab strip does not change. Use the keyboard (`1`–`9` to jump to a session,
`[` / `]` to cycle) to switch sessions.

---

## Platform Caveats

### Windows 11 — Shift modifier dropped on mouse events

Crossterm issue [#986](https://github.com/crossterm-rs/crossterm/issues/986) documents
that Windows 11 (running under modern Windows Terminal or conhost) drops the Shift
modifier on mouse events before crossterm can read them. The practical impact:

- `Shift+Wheel` degrades to plain wheel in Normal, LinkHighlight, and DevTools/Network.
- Page-step scrolling via the wheel is therefore unavailable on Windows 11.
- Workaround: use the keyboard `PageUp`/`PageDown` keys for page-step navigation —
  these are unaffected by the Shift-drop bug.

Other platforms (macOS, Linux, older Windows builds) are not affected.

### Legacy Windows conhost — mouse capture silently ignored

If your terminal is the legacy `conhost.exe` shipped before Windows 10, mouse capture
escape sequences are silently ignored and the wheel is never delivered to fdemon. Wheel
events instead fall through to the host terminal's scrollback (which is often the desired
behavior anyway when capture does not work).

Set `enable_mouse = false` in `.fdemon/config.toml` to opt out cleanly and avoid any
side effects from sending capture sequences to a terminal that ignores them.

---

## Disabling Mouse Capture

If you prefer wheel events to drive your terminal's native scrollback, or if you are on
legacy Windows conhost, disable mouse capture:

```toml
[ui]
enable_mouse = false
```

Restart fdemon after changing this setting. See `[ui] enable_mouse` in
[CONFIGURATION.md](CONFIGURATION.md) for the full setting reference including the "When
to disable mouse capture" callout.

---

## Future Work

- Coordinate-aware click handling (region registry, header shortcuts, log-row clicks)
- Drag-to-select for log lines
- Horizontal-scroll consumers (log timeline panning, DevTools secondary axis)
- First-launch hint for users who did not realize mouse capture is active
