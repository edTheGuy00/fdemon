# Mouse Support Follow-Up Bugs

**Status:** Planning
**Origin:** Post-implementation bug reports against the completed mouse-support feature
**Parent feature plan:** `workflow/plans/features/mouse-support/PLAN.md` (phases 1–6 done)

This bug-fix plan addresses three polish issues discovered after merging the
mouse-support phases. Each bug is independently scoped; they share no source
files except `crates/fdemon-tui/src/terminal.rs` (bugs 2 and 3 both touch it).

---

## Bug 1 — Mode selector buttons are not clickable

### Symptom

In the New Session dialog, the Debug / Profile / Release mode buttons render
correctly but do not respond to mouse clicks. Other clickable elements in the
same dialog (devices, tabs, Launch button) work as expected.

### Root Cause

Three layered failures combine to drop the click path:

1. **`ModeSelector` never registers any click regions.**
   `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs:175-248` —
   `impl Widget for ModeSelector` uses the standard `Widget::render(self, area, buf)`
   signature with no `MouseCtx` parameter. The three per-button rects are
   computed locally at line 200 (`Layout::horizontal([Constraint::Ratio(1,3); 3]).spacing(1).split(...)`)
   but never exported.

2. **The field-row registration covers the Mode row with a single coarse region
   that emits the wrong message.**
   `launch_context.rs:1190-1217` (`register_full_layout_regions`) and
   `launch_context.rs:1223-1252` (`register_compact_layout_regions`) register
   one `MouseRect` per field row, all emitting
   `Message::NewSessionDialogFocusField { field }`. Clicking a mode button
   therefore focuses the Mode field — but never sets a mode.

3. **`Message::NewSessionDialogSetMode { mode }` exists but is a no-op stub.**
   `crates/fdemon-app/src/message.rs:548` defines the message variant.
   `crates/fdemon-app/src/handler/update.rs:1180-1185` lumps it into a
   catch-all "to be implemented" arm that returns `UpdateResult::none()`.

### Fix

| File | Change |
|------|--------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | Add a helper `register_mode_button_regions(row_rect, ctx)` that mirrors the `ModeSelector::render` layout split (label row 1, button row 3; `Layout::horizontal([Ratio(1,3); 3]).spacing(1)`) and registers three `click_at_z(rect, NewSessionDialogSetMode { mode: <Debug/Profile/Release> }, 2)` entries. Invoke it from both `register_full_layout_regions` and `register_compact_layout_regions` for the Mode row. `z_index = 2` keeps the buttons above the row-level focus region at `z = 1`, so the button click wins the hit-test. |
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | Add `pub fn handle_set_mode(state: &mut AppState, mode: FlutterMode) -> UpdateResult` modeled on `handle_mode_next` (lines 12-62). It must: (a) early-return `UpdateResult::none()` if `!is_mode_editable()`, (b) set `state.new_session_dialog_state.launch_context.mode = mode`, (c) emit `UpdateAction::AutoSaveConfig` for FDemon-source configs. Unlike the keyboard cycler, `handle_set_mode` should also set `focused_pane = LaunchContext` and `focused_field = LaunchContextField::Mode` so the click also focuses the field, matching the row-click behavior. |
| `crates/fdemon-app/src/handler/update.rs` | Remove `Message::NewSessionDialogSetMode { .. }` from the stub arm at line 1180 and add a real arm near line 1196: `Message::NewSessionDialogSetMode { mode } => new_session::handle_set_mode(state, mode)`. |

### Open Decision

When the mode is non-editable (e.g. derived from a read-only Flutter launch
config), should clicking a mode button (a) be silently ignored, (b) flash an
error toast, or (c) focus the Mode field anyway? Current keyboard cycler
silently ignores (option a). Recommend matching that behaviour for
consistency.

### Regression Tests

- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` — Unit
  test that renders a Launch Context with regions and asserts hit-tests on the
  three button rects return `NewSessionDialogSetMode { mode: Debug/Profile/Release }`,
  while a hit-test on the label row (above the buttons) still returns
  `NewSessionDialogFocusField { field: Mode }`. Covers expanded and compact
  layouts.
- `crates/fdemon-app/src/handler/new_session/launch_context.rs` — Unit tests
  for `handle_set_mode`: (a) sets mode when editable, (b) is a no-op when not
  editable, (c) returns `AutoSaveConfig` for FDemon configs, (d) returns
  `none` for VSCode-source configs.

---

## Bug 2 — Mouse cursor displays as text I-beam instead of a pointer

### Symptom

When the user hovers over the TUI, the OS-level mouse cursor shows the
text-edit (I-beam) shape because the terminal treats its window as a text
area. The user would prefer a regular arrow/pointer.

### Root Cause

The I-beam is a property of the terminal window chrome, not of the VT state
machine — enabling DECSET 1000/1002/1006 (mouse reporting) does not change
the OS pointer. The terminal continues to show the I-beam by default.

### Fix

There is a real escape sequence for this: **OSC 22**, originally from xterm,
formalized by kitty in v0.31.0, and adopted by an expanding set of terminals.

The sequence:

```
ESC ] 22 ; <shape-name> ESC \
```

Shape names map to CSS cursor values: `default`, `pointer`, `text`, `crosshair`,
`grab`, etc. To reset to the terminal default:

```
ESC ] 22 ; ESC \
```

**Terminal coverage (early 2026):**

| Terminal | OSC 22 |
|---|---|
| kitty | Full (push/pop stack) |
| xterm | Basic |
| Ghostty | Partial (default/pointer/text confirmed) |
| Foot | Yes |
| Alacritty | Opt-in via `terminal.osc22 = true` |
| WezTerm | PR open (not merged) |
| iTerm2 / Terminal.app / GNOME Terminal / Windows Terminal | No |

OSC sequences for unknown numbers are silently ignored by all conforming
terminals, so emission is fire-and-forget — no terminfo query is needed.

| File | Change |
|------|--------|
| `crates/fdemon-tui/src/terminal.rs` | In `enable_mouse_capture()` (lines 79-95), after the successful `execute!(stdout(), EnableMouseCapture)`, emit `\x1b]22;default\x1b\\` to stdout (raw write; crossterm has no helper). Wrap in `let _ = ...;` to swallow I/O errors — failure here must not abort mouse capture. In `disable_mouse_capture()` (lines 106-119), before the `DisableMouseCapture` call, emit `\x1b]22;\x1b\\` (empty shape = reset). The reset must run while the alt screen is still active and raw mode is still enabled, before `ratatui::restore()`. |

### Open Decision

The phrase "or use a custom cursor" in the report is not addressable — OSC 22
takes a fixed vocabulary of CSS cursor names. Custom bitmap cursors are not
in the protocol. Recommend documenting in `docs/KEYBINDINGS.md` (or
`MOUSE.md`) that the cursor change is best-effort and depends on terminal
support, with a link to <https://can-i-use-terminal.github.io/features/osc22.html>.

### Regression Tests

- `crates/fdemon-tui/src/terminal.rs` — Unit tests cannot observe raw stdout
  writes from `execute!`. Add a thin internal helper `pointer_shape_set(&str)`
  / `pointer_shape_reset()` that returns the exact byte sequence as a `&str`
  constant; unit-test the constants. The full I/O path is covered by manual
  testing on a supporting terminal (kitty) noted in `docs/DEVELOPMENT.md`.

---

## Bug 3 — Mouse SGR sequences leak to terminal after exit

### Symptom

After closing fdemon, garbage characters like `0;35;32m35;35;32M35;35;33M…`
appear in the terminal scrollback. The pattern (semicolon-separated triples
ending in `M` / `m`) is the SGR-extended mouse report format from DECSET
1006 — these are mouse events the terminal generated and queued while fdemon
was no longer reading them.

### Root Cause

There are **two distinct problems**, both contributing:

**Problem 3a — Buffered mouse events during async shutdown (primary cause for
normal exit).**

In `crates/fdemon-tui/src/runner.rs:56-69` (and the mirror copy at lines
148-162 in `run_with_project_and_dap`):

```rust
let result = run_loop(&mut term, &mut engine);  // exits when should_quit
engine.shutdown().await;                         // ← can take 100s of ms
terminal::disable_mouse_capture();               // ← only NOW does mouse stop
ratatui::restore();
```

Between `run_loop` returning and `disable_mouse_capture()` running,
`engine.shutdown().await` performs an async cleanup that can take significant
time (watcher stop, Flutter process termination). Mouse capture is still
active during this window. Any mouse movement the user makes — for example,
moving away from the closing window — generates SGR mouse reports that the
terminal writes to fdemon's PTY input queue. Because the event loop has
exited, those bytes accumulate in the kernel TTY buffer. After fdemon exits
and the shell becomes the foreground process, the shell reads the queued
bytes and prints them as garbage.

**Problem 3b — Panic-hook ordering puts mouse-disable AFTER ratatui-restore.**

In both runners, `terminal::install_panic_hook()` is called BEFORE
`ratatui::init()` (runner.rs:24 vs 30; lines 93 vs 123). Panic hooks compose
LIFO via the standard "take + wrap" pattern that both fdemon
(`terminal.rs:49-65`) and `ratatui::init()` use. Net effect on panic:

1. ratatui's hook fires first → `ratatui::restore()` → `LeaveAlternateScreen`
   + `disable_raw_mode`.
2. fdemon's hook fires second → `disable_mouse_capture()` → emits DECRST
   1006/1015/1003/1002/1000 to the **primary screen**, where they may render
   as visible bytes depending on the terminal's parser state.

The `terminal.rs:55-61` comment acknowledges that disable must run before
restore, but the install order makes the actual runtime sequence the
opposite.

### Fix

| File | Change |
|------|--------|
| `crates/fdemon-tui/src/runner.rs` | **Reorder teardown.** In both `run_with_project` (lines 56-69) and `run_with_project_and_dap` (lines 148-162), move `terminal::disable_mouse_capture()` to run **before** `engine.shutdown().await`. New order: `run_loop` → `disable_mouse_capture` → `drain_input(50ms)` → `engine.shutdown().await` → `ratatui::restore()`. Move `terminal::install_panic_hook()` to **after** `ratatui::init()` (line 30 → before line 34; line 123 → before line 127) so fdemon's hook wraps ratatui's and fires first on panic. |
| `crates/fdemon-tui/src/event.rs` (or a new helper) | Add `pub fn drain_input(timeout: Duration)` that calls `crossterm::event::poll(remaining)` + `crossterm::event::read()` in a loop until poll returns `false` or the cumulative time exceeds `timeout`. Discard all events. Used to consume any mouse reports that the terminal queued before `DisableMouseCapture` took effect. |
| `crates/fdemon-tui/src/terminal.rs` | No code changes for Bug 3a/b alone — but the comment block at lines 55-61 must be updated to reflect the new install order, and the doc comment on `install_panic_hook` should warn callers that it must be called after `ratatui::init()`. |

### Why Both Fixes Are Needed

- **3a alone** would handle normal exit but leave the panic path bleeding
  bytes to the primary screen.
- **3b alone** (panic-hook reorder) would fix panics but not the normal-exit
  buffered-events scenario, which matches the user's "after we close
  fdemon" wording.

### Regression Tests

- `crates/fdemon-tui/src/terminal.rs` — Tighten the existing
  `test_install_panic_hook_is_idempotent` and add a documentation comment
  asserting the install-order invariant. The actual ordering is verified
  indirectly (we cannot instrument the global panic hook chain in a unit
  test) — add an integration test that uses `serial_test::serial` and
  inspects the order in which the wrapping closures recorded a sentinel via
  a `Mutex<Vec<&str>>`.
- `crates/fdemon-tui/src/event.rs` — Unit test for `drain_input`: with no
  pending events, returns in well under the timeout; with synthetic events
  pushed via a fake event source (if available), drains all events; never
  panics on poll error.
- Manual verification on kitty (mouse-tracking-friendly): launch fdemon,
  press Q while moving the mouse rapidly, observe no garbage in the shell
  prompt. Document in `docs/DEVELOPMENT.md` under "Common Issues → Mouse
  exit leak verification".

---

## File Overlap Analysis

This bug plan touches the following files. Tasks must be designed so
wave-peer tasks have no overlap; sequential tasks may share write files.

| Task (proposed) | Files Modified (Write) | Files Read |
|---|---|---|
| **T1: Bug 1 — Add `handle_set_mode` handler** | `crates/fdemon-app/src/handler/new_session/launch_context.rs` (handler), `crates/fdemon-app/src/handler/update.rs` (route) | `message.rs` |
| **T2: Bug 1 — Register mode-button regions** | `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | T1's handler |
| **T3: Bug 2 — OSC 22 pointer shape** | `crates/fdemon-tui/src/terminal.rs` | — |
| **T4: Bug 3 — Teardown reorder + panic-hook order + `drain_input`** | `crates/fdemon-tui/src/runner.rs`, `crates/fdemon-tui/src/event.rs` (or new helper), `crates/fdemon-tui/src/terminal.rs` (doc comment) | — |
| **T5: Docs — mouse-support known limitations** | `docs/KEYBINDINGS.md` or `docs/MOUSE.md` (whichever holds mouse docs), `docs/DEVELOPMENT.md` (manual verification note) | — |

### Overlap Matrix

| | T1 | T2 | T3 | T4 | T5 |
|---|---|---|---|---|---|
| **T1** | — | depended-on by T2 | none | none | none |
| **T2** | depends on T1 | — | none | none | none |
| **T3** | none | none | — | shared write: `terminal.rs` | none |
| **T4** | none | none | shared write: `terminal.rs` | — | none |
| **T5** | none | none | none | none | — |

**Strategy:**

- **T1 → T2** sequential (T2 imports T1's new handler). Same branch.
- **T3 ↔ T4** sequential (both write `terminal.rs`). Same branch. T4 first
  (reorder is structural), then T3 (additive OSC 22 emission).
- **T1+T2 ↔ T3+T4 ↔ T5** independent — different files. Can run as
  parallel worktrees.

### Recommended Wave Layout

- **Wave A (parallel worktrees):**
  - Worktree 1: T1 then T2 (Bug 1)
  - Worktree 2: T4 then T3 (Bug 3 → Bug 2 share `terminal.rs`)
  - Worktree 3: T5 (docs)
- **Wave B:** none — single wave covers all work.

---

## Documentation Updates

No `ARCHITECTURE.md` / `CODE_STANDARDS.md` / `DEVELOPMENT.md` content changes
are required for this bug fix — none of the proposed fixes introduce new
modules, new patterns, or new build steps. The teardown-order invariant in
`runner.rs` is enforced by code comments local to the affected functions,
not by a new architectural pattern.

`docs/DEVELOPMENT.md` gets one new line in "Common Issues" pointing at the
manual mouse-exit verification — this is owned by `doc_maintainer` and goes
in a separate doc-update task (T5) if the implementor decides the
verification step is worth recording.

---

## Out of Scope

- **Per-button click feedback (hover/pressed state):** Hover requires capturing
  `MouseInput::Moved` events, which are currently dropped at the event.rs
  boundary (see `terminal.rs:80-85` comment). That's a feature, not a bug.
- **Custom bitmap mouse cursors:** OSC 22 vocabulary is fixed to CSS cursor
  names. Not addressable without protocol changes outside this project.
- **iTerm2 / Terminal.app pointer shape:** Neither supports OSC 22. The fix is
  best-effort and silently degrades to I-beam on unsupported terminals.

---

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| `drain_input` blocks longer than the 50ms timeout on a slow terminal | 🟡 Minor | Hard timeout via cumulative elapsed time; never block on a single `read()` call. |
| OSC 22 sequence corrupts output on terminals with broken OSC parsers | 🟡 Minor | OSC sequences end with ST (`ESC \`); conforming terminals discard unknown OSC numbers. If a terminal mis-parses, the worst case is a few visible bytes — a smaller leak than Bug 3 itself. |
| Panic-hook reorder breaks an existing `test_install_panic_hook_is_idempotent` | 🟡 Minor | The test only checks the `HOOK_INSTALLED` flag; the reorder does not affect it. Verify test still passes. |
| Click on Mode row in the spacer between buttons does nothing | 🔵 Nitpick | Acceptable — keyboard cycler also requires explicit focus. The row-level focus region at `z=1` still catches clicks in the label band above the buttons. |
