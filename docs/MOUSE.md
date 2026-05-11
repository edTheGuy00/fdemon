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

This is a deliberate simplification. Scroll routing is coordinate-free; only **click**
events use the per-frame region registry for coordinate-based hit-testing (see Phases 3
and 4 below).

**Practical implication:** If you hover over a session tab and scroll, the log view
scrolls — the tab strip does not change. Use the keyboard (`1`–`9` to jump to a session,
`[` / `]` to cycle) to switch sessions, or left-click a tab to select it.

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

### Pointer shape (OSC 22)

While the TUI is active, fdemon requests the `default` (arrow) OS-level pointer shape
via the OSC 22 escape sequence, and resets it on exit. This keeps the cursor from
staying as a text I-beam while hovering over buttons and clickable regions.

OSC 22 support is best-effort and depends on your terminal emulator:

| Terminal | OSC 22 Support |
|----------|---------------|
| kitty | Supported |
| Ghostty | Supported |
| Foot | Supported |
| xterm | Supported |
| Alacritty | Requires `terminal.osc22 = true` in Alacritty config |
| iTerm2 | Silently ignored — I-beam remains |
| macOS Terminal.app | Silently ignored — I-beam remains |
| Windows Terminal | Silently ignored — I-beam remains |
| GNOME Terminal | Silently ignored — I-beam remains |

Terminals that do not support OSC 22 silently ignore the escape sequence; there is no
functional regression, only a cosmetic one (the pointer shape stays as an I-beam).

- Pointer-shapes reference: <https://sw.kovidgoyal.net/kitty/pointer-shapes/>
- Terminal compatibility table: <https://can-i-use-terminal.github.io/features/osc22.html>

### Compact NewSessionDialog — mouse not available at narrow widths

When the terminal is between 40–69 columns wide and 20–21 rows tall, the New Session
Dialog falls back to a compact-vertical layout that does not register device-row click
regions. In this size range fdemon shows a small hint line (e.g. `"Resize for mouse"`);
device selection remains fully functional via the keyboard. Resize the terminal wider
than 70 columns to restore mouse coverage.

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

## Phase 3: Click Surfaces — Header and Session Tabs

Click support for the header and session tabs was added in Phase 3.

### Header Shortcuts

Bracketed shortcuts in the title bar are clickable. Clicking `[r]`, `[R]`, `[x]`,
`[d]`, `[D]`, or `[q]` fires the same action as the corresponding key, subject to
the same `is_busy` gate (e.g., `[r]` is a no-op during a hot-reload in progress).

### Session Tabs

- **Left-click a tab**: switches to that session.
- **Middle-click a tab**: closes that session.
- **Click the device pill** (single-session compact header): opens the New Session
  dialog so you can add or switch devices.

---

## Phase 4: Click Behavior

### Log View

- **Single click on a log row**: no visible action. The row is registered for
  double-click detection but is not scrolled or highlighted.
- **Double click on the same row within 400 ms**: toggles the entry's stack trace
  expansion (if the entry has a stack trace).
- **Double click on a different row within 400 ms**: treated as two separate single
  clicks; no toggle.
- **Double click on the same row after a session switch**: treated as a fresh single
  click (the previous click stamp is cleared on session change).

### DevTools Sub-tab Bar

- Click `[i] Inspector` / `[p] Performance` / `[n] Network` to switch the active
  panel. Equivalent to pressing `i` / `p` / `n` keys.

### Inspector Tree

- Click a tree row to select it (equivalent to `↑`/`↓` keyboard navigation).
- Click the `▶`/`▼` glyph at the row's left edge to expand or collapse the node
  (equivalent to `→`/`←` keyboard expand/collapse).
- Both clicks dispatch a layout fetch under the same debounce and cache rules as
  keyboard navigation.

### Performance Frame Chart

- Click a frame bar in the chart to select it. Equivalent to `Tab`/`Shift+Tab` in
  the frames view.
- Clicking outside any frame bar (e.g., on the budget-line area) is a no-op.

### Network Table

- Click a row in the request table to select it; details appear in the side panel
  (or below in narrow mode).
- Click `[g]` / `[h]` / `[q]` / `[s]` / `[t]` in the detail-tab bar to switch
  detail tabs.

### Network Filter Input Mode

- When typing in the network filter input, clicks in the table area are suppressed
  (the user is typing).
- **Exception:** clicks on the DevTools sub-tab bar (`[i]`/`[p]`/`[n]`) still work
  — they switch panels AND exit filter input mode. This prevents a mouse-only user
  from being trapped in the filter.

---

## Phase 5: Dialogs and Overlays

Phase 5 extended the per-frame region registry to cover every remaining clickable
surface. After Phase 5, every visible UI element that has a keyboard activator also
responds to left-click.

### NewSessionDialog

- **Click `[1] Connected` / `[2] Bootable` tab headers** → switches the device list to
  the corresponding tab. Equivalent to `1` / `2` keyboard shortcuts.
- **Click a device row** → selects that device (single click sets the selection).
  Then click `Launch` to start the session.
- **Click a launch-context field** (`Configuration` / `Mode` / `Flavor` / `Entry Point`
  / `Dart Defines`) → focuses the field and activates it, mirroring the keyboard
  `Enter` press on that field.
- **Click `Launch` button** → launches the selected Flutter session.
- **Inside the fuzzy modal, click a visible result row** → selects and confirms the
  entry in a single click (equivalent to `↑`/`↓` + `Enter`).
- **The dart-defines modal inside `NewSessionDialog` is keyboard-only in v1.** No
  clickable rows are registered for the dart-defines sub-modal.

### ConfirmDialog

- **Click `[y] Yes`** → emits the action stored at the Yes button's index in
  `state.confirm_dialog_state.actions` (typically `ConfirmQuit`, but the registry
  reads from state so all confirm dialogs — quit, unsaved-settings, etc. — are
  clickable generically).
- **Click `[n] No`** → emits the corresponding No action (typically `CancelQuit`).
- The clickable rect covers the bracket + label only (`[y] Yes` / `[n] No`).
  Clicks elsewhere on the modal are no-ops.

### TagFilter Overlay (open with `T`)

- **Click a tag row** → sets the selected index **and** toggles the tag's visibility
  in a single click. There is no separate "select then toggle" two-click flow —
  one click both navigates and toggles.
- **Click `[a] All`** → shows all tags (equivalent to the `a` keyboard shortcut).
- **Click `[n] None`** → hides all tags (equivalent to the `n` keyboard shortcut).

### LinkHighlight Badges (visible after `Shift+L`)

- **Click a badge `[<char>]`** → emits `Message::SelectLink(<char>)`, following the
  link associated with that character. Equivalent to pressing the character key.
- The clickable rect is exactly the three-cell badge span (`[`, character, `]`).
  Clicks on the adjacent link text are not clickable in v1 — intentionally narrow
  to prevent accidental activation during scroll gestures.

### Settings Panel

- **Click a tab header** (`1. PROJECT` / `2. USER` / `3. LAUNCH` / `4. VSCODE`) →
  switches to that settings tab. Equivalent to the `1`–`4` keyboard shortcuts.
- **Click a setting row** → selects it (sets `selected_index`). Single click does not
  enter edit mode.
- **Double-click the same row within 400 ms** → enters edit mode (equivalent to
  `Enter`). This mirrors the Phase 4 log-view double-click pattern.
- **The Settings dart-defines and extra-args sub-modals are keyboard-only in v1.**

---

## Modal Precedence and Sub-Modal Gates

When a modal is open (`NewSessionDialog`, `ConfirmDialog`, `TagFilter`, `FlutterVersion`,
`Settings`, `LinkHighlight`), the renderer does not register base-UI click regions
(header brackets, log-view rows, session tabs) for the underlying surface. Clicks that
land outside the modal's own rects are silently dropped — they do **not** activate the
underlying base-UI region. This guarantees, for example, that clicking on header `[r]`
while a `ConfirmDialog` is shown does not fire a hot reload.

The z-index convention is:

| z-index | Layer |
|---------|-------|
| 0 | Base UI (header, tabs, log view, DevTools panels) |
| 1 | Primary modals (NewSessionDialog, ConfirmDialog, TagFilter, FlutterVersion) |
| 2 | Sub-modals layered atop a primary modal (NewSessionDialog fuzzy modal) |

**Sub-modal gates** narrow this further for Settings: when a dart-defines or extra-args
sub-modal is open inside `Settings`, `settings::handle_press` returns `None` for any
click, preventing leaks to the underlying Settings rows. The Settings panel does not
change `UiMode` when sub-modals open (they render on top), so the renderer-level modal
gate cannot cover them — the explicit gate inside the Settings press dispatcher closes
the gap.

---

## Future Work

- Drag-to-select for log lines.
- Drag-to-resize panel splits.
- Hover tooltips.
- Project-selector mouse support.
- Right-click context menus.
- Horizontal-scroll consumers (log timeline panning, DevTools secondary-axis navigation).

