# Plan: Mouse Support

## TL;DR

Add opt-in mouse interaction to the fdemon TUI without sacrificing keyboard-first ergonomics. Crossterm `MouseEvent`s convert to an abstract `MouseInput` enum at the `fdemon-tui` boundary (mirroring how `InputKey` works for keyboards) and flow through the existing TEA message bus as `Message::Mouse(MouseInput)`. Widgets register clickable rectangles into a per-frame `MouseRegions` registry on `AppState` during `view()`; the handler layer hit-tests against that registry to translate clicks into the same `Message`s the keybindings already emit. Scroll-wheel support comes first (highest value, no hit-testing required), header/tab clicks next, then log view, DevTools, dialogs, and the tag-filter overlay. Mouse capture is gated behind a new `[ui] enable_mouse` setting (default `true`) and is properly torn down on shutdown and panic so the user's terminal is never left in a broken state.

---

## Background

`docs/IDEAS.md` lists Mouse Support as a deferred Medium-complexity, Low-priority feature. The core arguments to defer were (a) terminal power users prefer keyboard, (b) significant complexity in event handling, (c) inconsistent terminal mouse support. The project has matured since: many Ratatui apps now ship mouse support without disrupting keyboard workflows, the TEA architecture in fdemon already routes input through a single `Message` enum that is straightforward to extend, and the responsive-layout work has produced a clean `Cell<usize>` render-hint pattern (CODE_STANDARDS.md, Principle 3) that we can reuse for hit-testing.

Mouse adds three concrete user wins:

1. **Scroll wheel in the log view** — by far the most-requested ergonomic gain; today users must reach for `j`/`k` or arrows.
2. **Clickable header shortcuts** — the header already shows `[r] Run [R] Restart [x] Stop [d] Debug [D] DAP [q] Quit`; making those bracketed labels clickable is discoverable and zero-cognitive-cost.
3. **Clickable session tabs** — when 3+ sessions are open, hunting for `Tab`/`1-9` is slower than a click.

We will *not* attempt to make every cell clickable. Mouse is purely additive over the existing keymap; everything must remain reachable via keyboard.

---

## Affected Modules

### New files

- `crates/fdemon-app/src/input_mouse.rs` — **NEW** `MouseInput` abstract enum (mirrors `input_key.rs`).
- `crates/fdemon-app/src/mouse_regions.rs` — **NEW** `MouseRegions` registry, `MouseAction` enum, `Cell<MouseRegions>` storage, hit-test helpers.
- `crates/fdemon-app/src/handler/mouse.rs` — **NEW** `handle_mouse(state, input) -> Option<Message>` per-mode dispatcher (parallels `handler/keys.rs`).
- `docs/MOUSE.md` — **NEW** user-facing docs (interaction map, terminal compatibility, opt-out instructions).

### Modified files

- `crates/fdemon-app/src/lib.rs` — re-export `MouseInput`, `MouseRegions`, `MouseAction`.
- `crates/fdemon-app/src/message.rs` — add `Message::Mouse(MouseInput)` variant.
- `crates/fdemon-app/src/handler/update.rs` — dispatch `Message::Mouse` to `handler::mouse::handle_mouse`.
- `crates/fdemon-app/src/handler/keys.rs` — extract per-mode `is_busy`/quit-gating helpers shared with `handler/mouse.rs`.
- `crates/fdemon-app/src/handler/mod.rs` — register the new `mouse` submodule.
- `crates/fdemon-app/src/state.rs` — add `mouse_regions: Cell<MouseRegions>` field; reset it at the start of each `view()`.
- `crates/fdemon-app/src/config/types.rs` — add `enable_mouse: bool` to `UiSettings` (default `true`).
- `crates/fdemon-app/src/settings_items.rs` — surface the new setting in the UI Settings tab.
- `crates/fdemon-tui/src/event.rs` — handle `Event::Mouse`, convert to `MouseInput`, emit `Message::Mouse`.
- `crates/fdemon-tui/src/terminal.rs` — `enable_mouse_capture()` / `disable_mouse_capture()` helpers; updated panic hook that disables mouse before `ratatui::restore()`; tracks an "enabled" flag to avoid the crossterm #613 Windows panic on disable-without-enable.
- `crates/fdemon-tui/src/runner.rs` — call `enable_mouse_capture()` after `ratatui::init()` when `settings.ui.enable_mouse`; call `disable_mouse_capture()` before `ratatui::restore()` in every exit path.
- `crates/fdemon-tui/src/widgets/header.rs` — record bracketed-shortcut rects into the registry during render.
- `crates/fdemon-tui/src/widgets/tabs.rs` — record per-tab rects into the registry.
- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — record the log-area rect (for scroll wheel + click-to-focus / toggle-stack-trace).
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — record sub-tab rects (Inspector / Performance / Network).
- `crates/fdemon-tui/src/widgets/devtools/{inspector,performance,network}/*` — record panel-internal rects (tree row, frame bar, request row, detail sub-tabs).
- `crates/fdemon-tui/src/widgets/new_session_dialog/*` — record device row, pane, and field rects.
- `crates/fdemon-tui/src/widgets/confirm_dialog.rs` — record Yes/No button rects.
- `crates/fdemon-tui/src/widgets/tag_filter.rs` — record per-tag-row rects.

### Documentation

- `docs/ARCHITECTURE.md` — document the `MouseInput` → `MouseRegions` → `handle_mouse` flow as a peer to the key flow (doc_maintainer task).
- `docs/CODE_STANDARDS.md` — add the Region Registry pattern as a counterpart to the `Cell<usize>` render-hint (doc_maintainer task).
- `docs/CONFIGURATION.md` — document `[ui] enable_mouse`.
- `docs/KEYBINDINGS.md` — cross-link to the new `MOUSE.md`.
- `docs/IDEAS.md` — strike "Mouse Support" from the Deferred Features list.

---

## Design

### Two abstractions, one bus

Keyboard input already follows a clean two-step boundary conversion:

```
crossterm::event::KeyEvent
    → InputKey                 (fdemon-app, terminal-library agnostic)
    → Message::Key(InputKey)
    → handler::keys::handle_key (per-UiMode dispatcher)
    → Message::<concrete action>
```

Mouse will mirror this exactly:

```
crossterm::event::MouseEvent
    → MouseInput               (fdemon-app, terminal-library agnostic)
    → Message::Mouse(MouseInput)
    → handler::mouse::handle_mouse (per-UiMode dispatcher)
    → Message::<concrete action>
```

This preserves the architectural invariant that `fdemon-app` does not depend on `crossterm`, and lets future non-TUI consumers (a GUI front-end, an MCP server) deliver mouse events the same way they deliver key events.

### `MouseInput` shape

A small, stable enum carrying coordinates and the modifier set. Uniform `(x, y)` lets the per-mode dispatcher consult the region registry consistently:

```rust
pub enum MouseInput {
    /// Primary button pressed at (x, y).
    Click { x: u16, y: u16, button: MouseButton, modifiers: KeyModSet },
    /// Primary button released at (x, y).
    Release { x: u16, y: u16, button: MouseButton, modifiers: KeyModSet },
    /// Drag motion at (x, y) with `button` held.
    Drag { x: u16, y: u16, button: MouseButton, modifiers: KeyModSet },
    /// Wheel scroll at (x, y).
    Scroll { x: u16, y: u16, direction: ScrollDir, modifiers: KeyModSet },
}
```

We deliberately drop `Moved` (mouse motion without a button) at the TUI boundary — it produces high-volume events with no current consumer and would force every render to mouse-rate.

`KeyModSet` is a small bitset wrapping shift/ctrl/alt — independent of crossterm so `fdemon-app` stays clean.

### `MouseRegions` registry

```rust
pub struct MouseRegions {
    entries: Vec<MouseRegionEntry>,
}

struct MouseRegionEntry {
    rect: Rect,             // re-exported from ratatui via fdemon-core OR a local Rect type
    on_click: MouseAction,  // what to do on left-click in this rect
    on_scroll: Option<MouseAction>, // optional scroll handler
    z_index: u8,            // for overlay precedence (modal > base)
}

pub enum MouseAction {
    /// Emit a single Message verbatim.
    Emit(Message),
    /// Emit Message with the click coordinate substituted in.
    /// Used for log-view clicks where the row determines which entry was clicked.
    EmitWithCoord(fn(u16, u16) -> Message),
}
```

Stored on `AppState` as `Cell<MouseRegions>` (same exception class as the existing `Cell<usize>` render-hint feedback in CODE_STANDARDS.md Principle 3). Each `view()` call:

1. Takes the registry out (`Cell::take`), clears it, returns a `MouseRegionsBuilder` to widgets.
2. Widgets push entries during render via `builder.click(rect, action)` / `builder.scroll(rect, action)`.
3. After render, the populated registry is put back via `Cell::set`.

When `Message::Mouse` arrives, `handle_mouse` reads the registry (without taking it — `Cell::take` + put-back to keep `&AppState` semantics), iterates highest-z first, and returns the first match.

### Region precedence (z-index)

Modal layers (NewSessionDialog, ConfirmDialog, TagFilter overlay, LinkHighlight badge, search overlay) record their regions with `z_index = 1`; everything else uses `0`. Hit-testing iterates highest-z first and stops at the first match. This keeps modal correctness without forcing widgets to know the global UI mode.

### Coordinate-aware actions

Most clicks map cleanly to existing `Message`s — `[r] Run` → `Message::HotReload`. A few need the click coordinate:

- **Log-view row click**: which entry was clicked depends on `y - logs.y`. This becomes a new `Message::FocusLogEntryAtRow { row: u16 }` (see Phase 4); the registry stores the relative row mapping.
- **Frame chart bar click**: `Message::SelectFrameAtX { x: u16 }`.
- **Network table row click**: `Message::SelectNetworkRowAt { row: u16 }`.

These coordinate-dependent messages are new pure handlers; they do not require new TEA actions, just routing to existing per-session state setters.

### Mouse capture lifecycle

- **Enable**: After `ratatui::init()` in each runner entry path *(`run_with_project`, `run_with_project_and_dap`, `run`, `selector::run`)*, call `terminal::enable_mouse_capture()` if `settings.ui.enable_mouse`. The function records that capture is on into a `static AtomicBool` — required to dodge the crossterm #613 panic on Windows when `DisableMouseCapture` runs without a prior enable.
- **Disable**: Always before `ratatui::restore()` in normal exit paths.
- **Panic hook**: Updated to `disable_mouse_capture()` then `ratatui::restore()`. The `AtomicBool` guard makes this safe to invoke even if the panic hits before mouse was enabled.
- **Setting toggled at runtime**: Out of scope for v1. The setting is read once at startup. (Settings panel will show a dialog hint that a restart is required, mirroring how `theme` already works.)

### Out of scope (v1)

- Drag-to-select-text in the log view (terminals already pass Shift+drag through natively when mouse capture is on — that suffices).
- Mouse hover tooltips (no native support; would require ticker-driven repaints).
- Touchpad horizontal scroll (`ScrollLeft`/`ScrollRight`) — collected at the boundary but currently no consumer; routes to no-op.
- Right-click context menus (no widget designed for them; defer until a real use case appears).
- Click-and-drag scrollbar handle dragging (Ratatui scrollbars are render-only; would need substantial state work).

---

## Development Phases

### Phase 1: Foundation — plumbing + opt-out + safe lifecycle

**Goal**: Mouse events flow from the terminal into the TEA loop and are silently consumed; users can disable the entire feature; the terminal is never left in a broken state on crash.

**Steps**

1. **Abstract input + message wiring**
   - Add `crates/fdemon-app/src/input_mouse.rs` with `MouseInput`, `MouseButton`, `ScrollDir`, `KeyModSet`.
   - Add `Message::Mouse(MouseInput)` to `message.rs`.
   - Add a no-op `handler::mouse::handle_mouse` returning `None` for all UI modes.
   - Wire `Message::Mouse` in `handler::update::update` to call `handle_mouse`.

2. **Terminal-side conversion**
   - Extend `crates/fdemon-tui/src/event.rs::poll()` to handle `Event::Mouse(_)`, drop `Moved`, convert the rest, emit `Message::Mouse`.
   - Add `key_modifiers_to_set()` helper.

3. **Mouse-capture lifecycle**
   - In `terminal.rs`, add `enable_mouse_capture()` / `disable_mouse_capture()` backed by `static MOUSE_CAPTURE_ON: AtomicBool` so `disable` is a no-op if `enable` was never called (guards crossterm issue #613 on Windows).
   - Update `install_panic_hook()` to call `disable_mouse_capture()` before `ratatui::restore()`.
   - Each `runner.rs` entry path: after `ratatui::init()`, call `enable_mouse_capture()` iff `settings.ui.enable_mouse`. Before each `ratatui::restore()`, call `disable_mouse_capture()`.

4. **Configuration**
   - Add `enable_mouse: bool` (default `true`) to `UiSettings`. Add a `default_true_for_mouse()` function so an explicit `enable_mouse = false` in `config.toml` is respected.
   - Surface in `settings_items.rs` UI Settings tab with a "Restart required" tooltip-style description.

5. **Tests**
   - Unit-test the crossterm `MouseEvent` → `MouseInput` conversion table (one test per `MouseEventKind` variant).
   - Unit-test that `Moved` events drop to `None`.
   - Unit-test the `AtomicBool` guard: calling `disable_mouse_capture()` without a prior `enable` is a no-op and does not error.
   - Unit-test that `update()` consumes `Message::Mouse` without changing state when `handle_mouse` returns `None`.

**Milestone**: A user can scroll the wheel inside fdemon and nothing changes — no crashes, no terminal corruption on Ctrl-C, and `enable_mouse = false` truly disables the capture.

---

### Phase 2: Scroll wheel — biggest single ergonomic win

**Goal**: Wheel scrolls the log view, settings list, DevTools panels, and dialogs as users would expect, with no hit-testing required (scroll always routes by current `UiMode`).

**Steps**

1. **Per-mode scroll dispatch in `handle_mouse`**
   - In `Normal`: `ScrollDir::Up` → `Message::ScrollUp`, `Down` → `Message::ScrollDown`. Hold `Shift` for page scroll.
   - In `DevTools` Inspector: `InspectorNav::Up`/`Down`.
   - In `DevTools` Performance: no-op (frame timeline is keyboard-arrow only).
   - In `DevTools` Network: `NetworkNav::Up`/`Down`. Shift-scroll → `PageUp`/`PageDown`.
   - In `Settings`: `SettingsPrevItem` / `SettingsNextItem`.
   - In `NewSessionDialog`: `NewSessionDialogUp` / `NewSessionDialogDown`.
   - In `LinkHighlight`: `ScrollUp`/`ScrollDown` (already supported per existing keybindings).
   - In `FlutterVersion`: corresponding nav messages.
   - In `SearchInput`/`ConfirmDialog`/`Loading`/`EmulatorSelector`: no-op (consume silently).

2. **Tests**
   - Per-mode unit tests: assert that `MouseInput::Scroll { direction: Up, .. }` produces the expected `Message` for each `UiMode`.
   - Test that scroll events outside the log area still scroll (we intentionally don't gate on coordinate for v1 — scroll is a global "scroll the focused thing" gesture).

**Milestone**: A user can drop into fdemon, run a Flutter session, and scroll through logs, settings, and DevTools panels with the wheel.

---

### Phase 3: Region registry + clickable header & tabs

**Goal**: Bracketed shortcuts in the header (`[r] [R] [x] [d] [D] [q]`) and session tabs become clickable.

**Steps**

1. **`MouseRegions` infrastructure**
   - Implement `mouse_regions.rs` (registry, builder, hit-test).
   - Add `mouse_regions: Cell<MouseRegions>` to `AppState`.
   - In `render::view`, take/clear/return-builder at the start; put back at the end.
   - Plumb a `&mut MouseRegionsBuilder` through to widgets via a `MouseCtx<'a>` parameter or a new `Widget`-extension trait. (Implementation choice: deferred to task design — depends on whether we make the builder accessible via `AppState` directly or thread it as a parameter. Recommendation: thread it — keeps render functions explicit about side effects.)

2. **Header shortcuts**
   - In `widgets/header.rs::render_title_row`, record each `[x]`-bracketed segment as a separate region paired with its `Message` (HotReload, HotRestart, StopApp, EnterDevToolsMode, ToggleDapServer, RequestQuit). Register only the bracket+letter cells, not the trailing label text (so accidental drags don't fire actions).

3. **Session tabs**
   - In `widgets/tabs.rs`, record each tab rect with `Message::SelectSessionByIndex(idx)`. Register middle-click → `CloseCurrentSessionAt(idx)` (new variant; closes the clicked session, not necessarily the current one).
   - For the single-session compact header, the device pill is also clickable → `Message::OpenNewSessionDialog` (so the user can quickly switch). Out of scope: device pill click in multi-session mode (no obvious action).

4. **Hit-test in `handle_mouse_normal`**
   - On `MouseInput::Click { button: Left, x, y, .. }` in `UiMode::Normal`, query `state.mouse_regions` for `(x, y)`. Return the matching region's `Message::Emit(...)`.

5. **Tests**
   - Snapshot test on header render at 80×24: confirm the registry has exactly six clickable regions covering the bracketed shortcuts.
   - Unit test on tab regions across 1, 3, and 9 sessions.
   - Unit test that left-click on `r`'s rect produces `Message::HotReload` only when not busy (mirroring the keymap gate).

**Milestone**: User can click `[r]` in the header to hot reload, click a session tab to switch, middle-click a tab to close it.

---

### Phase 4: Log view & DevTools panel-internal clicks

**Goal**: Click a log entry to focus / toggle stack trace; click a DevTools sub-tab; click a frame bar; click a network request row.

**Steps**

1. **Log-view click handlers**
   - In `widgets/log_view/mod.rs`, register one region covering the log-list area with `MouseAction::EmitWithCoord(|x, y| Message::FocusLogEntryAtRow { row: y - origin_y })`.
   - Add `Message::FocusLogEntryAtRow` and a handler that maps the row to an entry index using the existing `LogViewState::offset` and the per-line height map (already computed by the renderer for wrap mode).
   - On *double-click* (detected via timestamp + position match), emit `Message::ToggleStackTrace`. (Double-click detection: implement in `handle_mouse` with a `last_click_at: Instant` field on a small `MouseClickState` struct on `AppState`.)
   - Vertical scrollbar: not interactive in v1; users use wheel.

2. **DevTools sub-tab clicks**
   - In `widgets/devtools/mod.rs::render_tab_bar`, register each sub-tab rect → `Message::SwitchDevToolsPanel(panel)`.

3. **Inspector tree row clicks**
   - In `tree_panel.rs`, register one region per visible tree row → `Message::InspectorClickRow { row: u16 }`. Handler maps row → node id, sets selection, fetches layout data.
   - Click on a row's expansion glyph (`▶`/`▼`) → `Message::InspectorToggleExpansion`.

4. **Performance frame bars**
   - In `frame_chart/bars.rs`, register the bar area → `MouseAction::EmitWithCoord(|x, _| Message::PerformanceClickBar { x })`. Handler maps x → frame index using the existing bar-position table.

5. **Network request rows**
   - In `request_table.rs`, register one region per visible row → `Message::NetworkClickRow { row: u16 }`. Click on already-selected row → refetch (already existing behavior on Enter).
   - Detail sub-tabs (g/h/q/s/t) become clickable → `Message::NetworkSwitchDetailTab(tab)`.

6. **Tests**
   - Snapshot test: log-view registry contains exactly one row-click region after render at 80×24 with N entries.
   - Unit test for double-click detection (within 400 ms, within 1 cell of previous click → toggle).
   - Per-DevTools-panel registry tests.

**Milestone**: Click any panel surface and it does the natural thing — select a frame, expand a widget, focus a log entry.

---

### Phase 5: Dialogs & overlays

**Goal**: NewSessionDialog, ConfirmDialog, TagFilter overlay, and LinkHighlight badges become clickable.

**Steps**

1. **NewSessionDialog**
   - Register Connected/Bootable tab buttons.
   - Register each device row → `Message::NewSessionDialogDeviceUp/Down + NewSessionDialogConfirm` (set selected index then confirm).
   - Register Configuration / Mode / Flavor / Dart Defines fields → `NewSessionDialogFieldActivate` after focusing them.
   - Register the Launch button → `NewSessionDialogLaunch`.
   - Inside fuzzy modals: register each result row → select-and-confirm.

2. **ConfirmDialog**
   - Register Yes/No buttons → `ConfirmQuit`/`CancelQuit`.

3. **TagFilter overlay**
   - Register each tag row → `Message::TagFilterMoveToIndex(i) + TagFilterToggleSelected`.
   - Register the `[a] all`/`[n] none` action labels.

4. **LinkHighlight**
   - Register each link badge rect → `Message::SelectLink(c)` for the corresponding shortcut.

5. **Settings panel**
   - Register tab headers → `SettingsGotoTab(i)`.
   - Register each setting row → `SettingsSelectIndex(i) + SettingsToggleEdit` (single click selects, double-click activates).

6. **Tests**
   - Per-dialog snapshot test on the registry contents.
   - Click-precedence test: when NewSessionDialog is open, header regions must NOT match (modal z-index wins).

**Milestone**: Every visible UI surface that has a keyboard activator also responds to clicks. Mouse is fully usable (but never required).

---

### Phase 6: Documentation & polish

**Goal**: Users discover the feature; the architecture is documented; the deferred-features doc is updated.

**Steps** *(routed to `doc_maintainer` for ARCHITECTURE.md / CODE_STANDARDS.md)*

1. **Create `docs/MOUSE.md`** — interaction map mirroring `KEYBINDINGS.md` structure. Compatibility section calling out: macOS Terminal.app mouse-reporting toggle, legacy Windows conhost limitation, Shift+drag passthrough for native text selection.
2. **Update `docs/ARCHITECTURE.md`** — new "Input Subsystem" or extension to existing input section: `InputKey`/`MouseInput` parallel, `MouseRegions` registry, hit-test precedence.
3. **Update `docs/CODE_STANDARDS.md`** — add a "Region Registry Pattern" subsection under Responsive Layout Guidelines, citing the existing `Cell<usize>` render-hint exception as precedent.
4. **Update `docs/CONFIGURATION.md`** — `[ui] enable_mouse` documentation.
5. **Update `docs/KEYBINDINGS.md`** — top-of-file note: "Mouse interactions are documented in MOUSE.md."
6. **Update `docs/IDEAS.md`** — remove "Mouse Support" from Deferred Features.

**Milestone**: Anyone reading the docs in isolation understands what mouse can do, how to disable it, and how the implementation is structured.

---

## Edge Cases & Risks

### Terminal compatibility variance
- **Risk**: macOS Terminal.app users must toggle View → Allow Mouse Reporting; legacy conhost on older Windows doesn't deliver mouse events at all (microsoft/terminal#7376). Users on those terminals will see fdemon "ignore" their clicks.
- **Mitigation**: `docs/MOUSE.md` calls out the macOS toggle and recommends Windows Terminal on Windows. The feature is opt-out via `enable_mouse = false` for users whose terminal is broken. We do not attempt to detect-and-warn — far too much heuristic guesswork for the value.

### Disable-without-enable panic on Windows (crossterm #613)
- **Risk**: Calling `DisableMouseCapture` without a successful prior `EnableMouseCapture` panics with `TryFromIntError` on Windows.
- **Mitigation**: `static AtomicBool MOUSE_CAPTURE_ON` flag guards `disable_mouse_capture()` so it is a no-op when not active. Verified with a unit test.

### Shift modifier silently dropped on Windows 11 (crossterm #986)
- **Risk**: Shift+click on Windows 11 + Windows Terminal does not include the Shift modifier in the event. Our Shift-page-scroll feature may not work there.
- **Mitigation**: Acceptable degradation — the feature is additive, not load-bearing. Documented in `docs/MOUSE.md`. Users keep the keyboard PageUp/PageDown.

### `Cell<MouseRegions>` purity exception
- **Risk**: TEA purists may object to render-time mutation of registry state, even via `Cell`.
- **Mitigation**: This is the same exception class as the existing `Cell<usize>` render-hint feedback (CODE_STANDARDS.md, Principle 3, "TEA exception note"). We document it explicitly with the same `// EXCEPTION:` comment style at every call site, and limit the mutation to a single non-business-logic value.

### High-volume mouse events flooding the channel
- **Risk**: `Moved` and rapid `Drag` events from a touchpad scroll could overrun the message channel.
- **Mitigation**: Drop `Moved` at the TUI boundary (never converted). Drag is preserved (Phase 1), but no consumer wired up in v1; it is a no-op. If a future feature needs drag, we will add a coalescing layer in `event.rs`.

### Region registry allocation churn
- **Risk**: Reallocating `Vec<MouseRegionEntry>` every frame at 20 FPS could be a hotspot.
- **Mitigation**: Reuse the underlying `Vec` via `Cell::take` + `Vec::clear` (not drop/reallocate). Pre-size to ~32 entries — covers worst-case header + 9 tabs + 9 device rows + 6 settings rows + buffer. Verified with a benchmark in Phase 6.

### Click on bracketed shortcut while session is busy
- **Risk**: Clicking `[r]` during a reload should respect the same `is_busy` gate as the keypress, not silently succeed.
- **Mitigation**: `handle_mouse` consults the same `is_busy` and `tag_filter_visible` gates as `handle_key`. Tested per phase.

### `clear_logs` collision (`c` key) and click misroute
- **Risk**: `c` key clears logs and is bound at the global level; if we accidentally route a wheel-click to a `c`-equivalent message, we'd nuke logs unintentionally.
- **Mitigation**: We never map a click to `Message::ClearLogs`. `ClearLogs` remains keyboard-only.

### Bind drift over time
- **Risk**: Future changes to header copy ("[r] Run" → "[r] Reload") could desynchronize the rect-recording loop from the rendered text.
- **Mitigation**: Region recording lives inside the same render function that emits the spans, indexed off the same source-of-truth literal. Snapshot tests on the registry catch drift.

---

## Configuration Additions

```toml
# .fdemon/config.toml

[ui]
# Enable mouse interactions in the TUI: clickable header shortcuts, session
# tabs, log view, DevTools panels, and dialogs. Scroll wheel always works
# when enabled. Defaults to true.
#
# Set to false if your terminal handles mouse reporting poorly or you prefer
# Shift-free native text selection (some terminals reserve Shift+drag for
# native selection when mouse capture is on; this setting disables capture
# entirely).
#
# Note: Changes take effect on restart.
enable_mouse = true
```

---

## Mouse Interaction Summary

| Surface | Interaction | Effect |
|---------|-------------|--------|
| Anywhere | Wheel up/down | Scroll the focused list / log / panel |
| Anywhere | Shift+wheel | Page scroll (Linux/macOS only — see #986) |
| Header | Click `[r]` / `[R]` / `[x]` / `[d]` / `[D]` / `[q]` | Hot reload / restart / stop / DevTools / DAP / quit |
| Header | Click device pill (single-session) | Open NewSessionDialog |
| Tabs | Left-click tab | Select session |
| Tabs | Middle-click tab | Close that session |
| Log view | Left-click row | Focus the entry |
| Log view | Double-click row | Toggle stack trace expansion |
| DevTools | Left-click sub-tab | Switch panel |
| Inspector | Click row | Select node |
| Inspector | Click expansion glyph | Toggle expand/collapse |
| Performance | Click frame bar | Select frame |
| Network | Click row | Select / refetch request |
| Network | Click detail sub-tab | Switch detail view |
| NewSessionDialog | Click device | Select |
| NewSessionDialog | Click Launch | Launch |
| ConfirmDialog | Click Yes/No | Confirm/cancel |
| TagFilter | Click tag row | Toggle visibility |
| LinkHighlight | Click badge | Open link |
| Settings | Click row | Select |
| Settings | Double-click row | Edit |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `cargo test --workspace` includes ≥ 8 new tests covering input conversion and lifecycle guards
- [ ] Mouse events flow into the engine and are silently consumed (no behavior change)
- [ ] `enable_mouse = false` truly disables capture (verified by checking no escape sequences are written)
- [ ] Ctrl+C and panic in any code path leave the terminal usable (manual test on macOS + Linux)
- [ ] No clippy warnings, no fmt diff, all OS runners green in CI

### Phase 2 Complete When:
- [ ] Scroll wheel scrolls log view, settings, DevTools panels, NewSessionDialog
- [ ] Shift+wheel does page scroll where the modifier survives
- [ ] Per-mode unit tests assert correct message routing for every `UiMode`

### Phase 3 Complete When:
- [ ] Clicking `[r]` triggers hot reload (with `is_busy` gate respected)
- [ ] Clicking session tabs selects them; middle-click closes them
- [ ] Modal regions take precedence over base regions in hit-tests
- [ ] Snapshot tests cover header and tabs registries

### Phase 4 Complete When:
- [ ] Single click in log view focuses entry; double-click toggles stack trace
- [ ] DevTools sub-tab click switches panels
- [ ] Frame bar / network row / inspector tree click work end-to-end

### Phase 5 Complete When:
- [ ] Every dialog and overlay responds to clicks where a keyboard activator exists
- [ ] Mouse-only walk-through of full launch flow (open dialog → select device → launch → click reload → click DevTools → click frame → quit) succeeds

### Phase 6 Complete When:
- [ ] `docs/MOUSE.md` exists with interaction map and compatibility notes
- [ ] `docs/ARCHITECTURE.md` documents the registry pattern
- [ ] `docs/CODE_STANDARDS.md` documents the registry as a render-hint exception
- [ ] `docs/CONFIGURATION.md` documents the new setting
- [ ] `docs/IDEAS.md` no longer lists Mouse Support as deferred

---

## Future Enhancements

- Drag-to-select log lines (would require leaving capture off during drag, then re-enabling — feasible but fiddly).
- Drag-to-resize panel splits (DevTools Inspector vs Layout).
- Hover tooltips on truncated device names / status icons.
- Mouse support for the project selector at startup (`selector.rs`).
- Right-click context menus on log entries (copy line, jump to file, filter to source).

---

## References

- Crossterm 0.29 [`MouseEvent`](https://docs.rs/crossterm/0.29.0/crossterm/event/struct.MouseEvent.html), [`MouseEventKind`](https://docs.rs/crossterm/0.29.0/crossterm/event/enum.MouseEventKind.html)
- Crossterm [`EnableMouseCapture`](https://docs.rs/crossterm/latest/crossterm/event/struct.EnableMouseCapture.html) — sends `?1000h ?1002h ?1003h ?1015h ?1006h`; SGR 1006 lifts the 223-column cap.
- Ratatui 0.30 [`init`](https://docs.rs/ratatui/0.30.0/ratatui/fn.init.html) / [`restore`](https://docs.rs/ratatui/0.30.0/ratatui/fn.restore.html) — neither touches mouse capture.
- Ratatui [Mouse Capture Concepts](https://ratatui.rs/concepts/backends/mouse-capture/), [Panic Hooks Recipe](https://ratatui.rs/recipes/apps/panic-hooks/)
- Crossterm issue [#613](https://github.com/crossterm-rs/crossterm/issues/613) — disable-without-enable panic on Windows; we guard with `AtomicBool`.
- Crossterm issue [#986](https://github.com/crossterm-rs/crossterm/issues/986) — Shift modifier dropped on Windows 11 mouse events; documented as a known degradation.
- `docs/CODE_STANDARDS.md` Principle 3 — `Cell<usize>` render-hint exception, the existing precedent for the `Cell<MouseRegions>` pattern.
- `docs/ARCHITECTURE.md` "TEA Message Flow (via Engine)" — the message bus that `Message::Mouse` plugs into.
- `docs/IDEAS.md` §2 "Mouse Support" — original deferral rationale.
