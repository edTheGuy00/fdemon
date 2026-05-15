# Bug: Mouse text-selection / copy in logs no longer works after mouse-support feature

**Status:** Draft — awaiting approval
**Reporter context:** user notes that dragging across log lines selects nothing, and right-click on the log view does nothing, since the mouse-support feature (`workflow/plans/features/mouse-support/PLAN.md`) shipped.

---

## TL;DR

The mouse-support feature enables crossterm's full `EnableMouseCapture` sequence, which includes DECSET `?1003h` ("any-motion" tracking). With `?1003` on, terminal emulators (macOS Terminal.app, iTerm2, Windows Terminal, etc.) hand **every** mouse motion event to fdemon, and the user's drag never reaches the terminal's native text-selection engine — Shift held or not. The PLAN's design assumed Shift+drag would always "pass through" to native selection; in practice `?1003` defeats that on virtually every modern terminal.

Two follow-on facts compound the user's experience:

1. fdemon's event-loop **drops `Moved` events at the boundary** (`event.rs`), so `?1003` buys us *zero* behavioral value — it only damages the passthrough that the PLAN was relying on.
2. Right-click is captured but unmapped, so the user perceives the right mouse button as "broken" rather than "intentionally deferred."

**Recommended fix:** stop requesting `?1003`, add a runtime toggle so users can fully suspend capture for ad-hoc native selection on terminals where Shift+drag still misbehaves, and map right-click on a log row to a copy-line action so the right button is no longer dead.

---

## Reproduction

1. From a Flutter project directory: `cargo run` (default `enable_mouse = true`).
2. Run a session so logs are visible.
3. Try to click-drag across a few log lines to select text → **nothing is highlighted, no selection is created**.
4. Right-click on a log line → **nothing happens**.
5. Press `Cmd-C` / `Ctrl-Shift-C` → clipboard is empty.

Workarounds that exist today:

- Set `[ui] enable_mouse = false` in `.fdemon/config.toml` and restart fdemon. Loses scroll wheel + every click feature.
- macOS Terminal.app only: View → "Allow Mouse Reporting" toggle (suppresses the SGR reports until toggled back; awkward per-window).

---

## Root Cause

`crates/fdemon-tui/src/terminal.rs:111` calls crossterm's `execute!(stdout(), EnableMouseCapture)`, which writes the full sequence `\x1b[?1000h \x1b[?1002h \x1b[?1003h \x1b[?1015h \x1b[?1006h`.

Mouse-capture DECSET modes (relevant subset):

| Sequence | Mode | What it tracks |
|----------|------|----------------|
| `?1000` | X10 / Normal | Press & release only |
| `?1002` | Button-event | Press, release, and motion *while a button is held* |
| `?1003` | **Any-motion** | All motion, button held or not |
| `?1015`, `?1006` | Encoding | URXVT / SGR encodings; extend column range past 223 |

When `?1003` is active, terminals route every motion event into the application's TTY stream. Modern terminals therefore suspend their own text-selection handling — including the Shift+drag override that older terminals respected when only `?1000`/`?1002` were active. macOS Terminal.app, iTerm2, Alacritty, Windows Terminal, and Ghostty all behave this way.

The PLAN explicitly assumed Shift+drag would survive ("Drag-to-select-text in the log view (terminals already pass Shift+drag through natively when mouse capture is on — that suffices)" — `workflow/plans/features/mouse-support/PLAN.md` §"Out of scope (v1)"). That assumption is incorrect under `?1003`.

The added insult: fdemon's event boundary (`crates/fdemon-tui/src/event.rs`) explicitly drops `Moved` events. So `?1003` provides **no** functional benefit today. It only delivers events that are immediately discarded — and breaks native selection in the process.

Right-click: `MouseInput::Click { button: Right, .. }` does flow through the message bus, but no `handler/mouse.rs` arm matches it for the log view. This is the explicit Phase-1 design ("right-click context menus … defer until a real use case appears"). The user's mental model — that right-click should "do something" — is the now-arrived use case.

---

## Affected Modules

### Files modified

- `crates/fdemon-tui/src/terminal.rs` — replace `EnableMouseCapture` / `DisableMouseCapture` with a hand-written DECSET set that omits `?1003`; provide a `set_mouse_capture(enabled: bool)` runtime helper.
- `crates/fdemon-tui/src/event.rs` — pass `MouseEventKind::Moved` through to a `None` early-return without conversion (already the case; just confirm and add a test asserting `?1003`-class motion is never observed in practice).
- `crates/fdemon-app/src/handler/mouse.rs` — add a Normal-mode hit-test branch for `MouseButton::Right` on log-view rows → emit a new `Message::CopyLogEntryToClipboard { entry_id }`.
- `crates/fdemon-app/src/message.rs` — add `Message::CopyLogEntryToClipboard { entry_id: LogEntryId }`, `Message::ToggleMouseCapture`.
- `crates/fdemon-app/src/handler/update.rs` — handle the new messages.
- `crates/fdemon-app/src/state.rs` — add `mouse_capture_active: bool` (mirrors the actual terminal-side flag, defaulting to `settings.ui.enable_mouse`) so the UI footer can show the current state.
- `crates/fdemon-app/src/handler/keys.rs` — bind a keyboard shortcut (proposed: `Ctrl+M`) → `Message::ToggleMouseCapture` so users can suspend capture without leaving the app.
- `crates/fdemon-tui/src/runner.rs` — observe `Message::ToggleMouseCapture` results and call `terminal::set_mouse_capture(false|true)` accordingly. (Plumbed via `UpdateAction::SetMouseCapture(bool)`.)
- `crates/fdemon-app/src/handler/update.rs` (action emission) — emit `UpdateAction::SetMouseCapture(bool)` so the side-effect crosses the TEA boundary cleanly.
- `crates/fdemon-tui/src/widgets/header.rs` *(or status-bar widget)* — show a small `[mouse: off]` / `[mouse: on]` indicator so the toggle is discoverable and the current state is visible.
- Clipboard write: introduce a thin `fdemon-tui` (or `fdemon-app/services/clipboard.rs`) wrapper around `arboard` (or `copypasta`) — keep it behind a trait so tests can inject a `MemoryClipboard`.

### Documentation

- `docs/MOUSE.md` — replace "Shift+drag passthrough suffices" with the truth: native selection requires the toggle (or `enable_mouse = false`). Document the new `Ctrl+M` toggle, right-click-to-copy action, and the status indicator. Update the Future Work list (remove "Drag-to-select log lines" if right-click-copy + toggle covers the use case, or keep but reprioritize).
- `docs/KEYBINDINGS.md` — add the `Ctrl+M` binding row.
- `docs/CONFIGURATION.md` — clarify that `[ui] enable_mouse` is the *initial* state; the toggle alters runtime state and is not persisted.
- `docs/ARCHITECTURE.md` — note the new `UpdateAction::SetMouseCapture` side-effect channel (doc_maintainer task).
- `docs/CODE_STANDARDS.md` — no changes (existing patterns cover the additions).
- `workflow/plans/features/mouse-support/PLAN.md` — append a "Bugfix follow-up" note pointing at this BUG.md so future readers don't repeat the `?1003` mistake.

---

## Design

### Fix 1 — Drop `?1003` from the capture set (root cause)

Replace crossterm's `EnableMouseCapture` with a hand-written escape-sequence write that emits **only**:

```
\x1b[?1000h \x1b[?1002h \x1b[?1006h
```

- `?1000` + `?1002` → press, release, and drag-with-button-held are all reported. Sufficient for every click + scroll-wheel feature already shipped.
- `?1006` → SGR-encoded reports (extended column range; supported by every terminal that supports `?1002`).
- We drop `?1003` (no consumer) and `?1015` (URXVT encoding fallback, redundant with `?1006`).

Disable path mirrors: emit `\x1b[?1006l \x1b[?1002l \x1b[?1000l` (reverse order matches enable; `?1015` is not enabled so does not need disabling — the existing `MOUSE_CAPTURE_ON` guard still protects against disable-without-enable on Windows).

This single change is expected to restore Shift+drag passthrough on macOS Terminal.app, iTerm2, kitty, Alacritty, Ghostty, Wezterm, Windows Terminal, and GNOME Terminal. (Behavior verified by the spec, not yet by manual testing — see Manual-test Matrix below.)

**Why hand-written rather than a new crossterm helper:** crossterm 0.29's `EnableMouseCapture` is a single opaque command that always sends all five. Issue [crossterm-rs/crossterm#947](https://github.com/crossterm-rs/crossterm/issues/947) tracks splitting them, unresolved upstream. Hand-writing the three DECSETs is six lines and keeps us off a fork.

### Fix 2 — Runtime toggle (escape hatch when Shift+drag still fights us)

Even with `?1003` dropped, some terminals (older xterm builds, some tmux configurations, some Linux distros' GNOME-Terminal) still suppress Shift+drag while `?1002` is on. For these:

- `Ctrl+M` toggles `terminal::set_mouse_capture(active)`. State is in-process only — restart returns to `settings.ui.enable_mouse`'s default.
- Status indicator on the header / status bar: `[mouse]` (on) vs `[mouse-off]` (off) — short enough to fit in 80-col terminals.
- While off: all `MouseEvent`s simply don't reach fdemon at all (the terminal handles them), so wheel scroll goes to terminal scrollback, native selection works, right-click does the terminal's default. Toggle back on with `Ctrl+M` to resume fdemon's mouse features.

This makes the feature genuinely opt-out at runtime, not just at config-file edit time.

> **Key-binding caveat:** `Ctrl+M` is the same byte as `Enter` (`\r`) in most terminals. If we cannot reliably disambiguate, fall back to a different chord — proposed alternatives: `Alt+m`, `Ctrl+\`, or a leader sequence. To confirm before locking in the choice. **Open question (Q1)** in the section below.

### Fix 3 — Right-click on a log row copies its line

Right-click is the universally-expected "I want to do something with this specific row" gesture. Rather than build a context menu (PLAN.md defers them), bind right-click directly to the single most useful action:

- `MouseInput::Click { button: Right, x, y, .. }` over a log row → `Message::CopyLogEntryToClipboard { entry_id }`.
- Handler resolves `entry_id` → reads the entry's rendered text → writes to clipboard via a `Clipboard` trait.
- Flash a 1-second status-bar toast: `Copied: <truncated 60-char preview>` so the action is visible.

This delivers obvious mouse value without committing to context-menu infrastructure. If/when a real menu is needed, right-click can be repurposed; the toast keeps current users informed.

Clipboard library choice: `arboard` (already a transitive dep of many Rust TUI projects; cross-platform, no system-clipboard daemon needed on Linux/Wayland for paste — and we only need write here). Wrap behind a `Clipboard` trait so tests can swap in an in-memory implementation.

### Why not implement drag-to-select inside the TUI

A drag-to-select implementation (custom highlight + clipboard write on release) was considered and rejected for v1 of this bug fix:

- High complexity: line-wrap-aware selection across wrapped soft-wrap lines, scroll-while-dragging support, terminal-cell→string-offset mapping, edge cases around stack-trace toggles mid-drag, multi-line span selection.
- Right-click-copy-line covers ~80% of the "I want to share this log line" use case. Shift+drag (now working) covers the rest for arbitrary substrings.
- Listed in the PLAN's Future Enhancements; remains there.

---

## Edge Cases & Risks

### `?1003` was load-bearing for some future feature
- **Risk**: Some future planned feature might rely on motion-without-button.
- **Mitigation**: None planned that we can find. If one arrives, the call site is a single function — we re-enable `?1003` there. The TODO marker in the existing `enable_mouse_capture` doc-comment ("when `Moved` events become useful in a future phase") makes this explicit.

### Toggle-binding collision with Enter
- **Risk**: `Ctrl+M` is `\r` in the TTY input stream. On terminals that don't send the kitty/CSI-u extended encoding, fdemon cannot distinguish `Ctrl+M` from `Enter`.
- **Mitigation**: Use a different chord, or detect the kitty-keyboard protocol bit and only bind under it. Decision pending Q1 below.

### Right-click on non-log surfaces
- **Risk**: Users will start expecting right-click everywhere.
- **Mitigation**: Document the scope: right-click currently only acts on log rows. Other surfaces emit a brief toast `Right-click: copy is only available on log rows` (debounced) or stay silent. Lean toward "silent" to keep the toast useful when it does appear. Open question Q2.

### Clipboard library failure on headless Linux CI
- **Risk**: `arboard` requires an X11/Wayland connection. CI tests must use the mock impl.
- **Mitigation**: Trait abstraction; `tests` use `MemoryClipboard`; only the runner instantiates the real `arboard::Clipboard`. Verified by running the workspace test suite headlessly in the implementor task.

### Toggle state drift between TEA model and terminal reality
- **Risk**: `set_mouse_capture(true)` could fail mid-flight (e.g., transient stdout error); the TEA model would believe capture is on while the terminal has nothing capturing.
- **Mitigation**: The runner returns the actual outcome via a follow-up `Message::MouseCaptureChanged(bool)`. The TEA `mouse_capture_active` field reflects observed state, not intent. Errors are toasted.

### Windows conhost / `?1003`-required terminals
- **Risk**: Some legacy Windows builds reportedly require `?1003` to deliver any mouse events at all (unconfirmed; folklore).
- **Mitigation**: Verify on Windows Terminal + conhost in the manual-test matrix. If true, gate the `?1003` decision behind a platform check. We strongly suspect modern Windows Terminal works fine with `?1002` alone (it's the spec-compliant subset).

---

## Manual-Test Matrix (Pre-merge)

| Terminal | Plain drag selects? | Shift+drag selects? | Right-click copies row? | `Ctrl+M` toggle works? |
|---|---|---|---|---|
| macOS Terminal.app | (native) | ✓ expected | ✓ | ✓ |
| iTerm2 | (native) | ✓ expected | ✓ | ✓ |
| kitty | (native) | ✓ expected | ✓ | ✓ |
| Alacritty | (native) | ✓ expected | ✓ | ✓ |
| Ghostty | (native) | ✓ expected | ✓ | ✓ |
| Wezterm | (native) | ✓ expected | ✓ | ✓ |
| Windows Terminal | (native) | ✓ expected | ✓ | ✓ |
| GNOME Terminal | (native) | ✓ expected | ✓ | ✓ |

"(native)" = without holding Shift, the terminal does its default — usually no selection while capture is on, but Shift+drag must work.

---

## Resolved Decisions

- **Toggle binding:** `Alt+m`. Documented caveat: terminals that interpret Alt as a Meta prefix may instead deliver `Esc` then `m`; we accept either form. If a terminal swallows Alt entirely, users fall back to editing `config.toml`.
- **Right-click outside a log row:** push a one-shot, dedup-by-text toast: `Right-click copies log lines; nothing to copy here.` (Uses the existing `AppState::push_toast` + `ToastLevel::Info` infrastructure at `state.rs:1271`.)
- **Status indicator:** rendered in the existing bottom metadata bar via `widgets::log_view::StatusInfo` (`crates/fdemon-tui/src/widgets/log_view/mod.rs:37`). New field on `StatusInfo` consumed when present; absent ⇒ no badge rendered.
- **Persistence:** the `Alt+m` toggle is **in-process only**. `[ui] enable_mouse` in `config.toml` is the sole persistent setting; the runtime toggle never writes it back. On restart, capture returns to the configured value.

---

## Success Criteria

- [ ] On macOS Terminal.app, iTerm2, Alacritty, Ghostty, kitty: Shift+drag selects log text natively while fdemon is running with `enable_mouse = true`.
- [ ] Right-click on any log row copies the row's full text to the system clipboard; status-bar toast confirms.
- [ ] `Ctrl+M` (or chosen chord) toggles mouse capture without restarting fdemon; status indicator reflects the current state; toggle is logged for debugging.
- [ ] All existing mouse features (scroll wheel, click `[r]`, click tabs, double-click stack-trace) still work after the fix.
- [ ] `cargo test --workspace` passes; new tests cover: (a) capture sequence excludes `?1003`, (b) right-click → clipboard write via mock, (c) toggle updates state, (d) status indicator renders both states.
- [ ] `docs/MOUSE.md` rewritten to match the new reality; PLAN.md cross-references this BUG.md.

---

## References

- Existing capture call site: `crates/fdemon-tui/src/terminal.rs:111` (`enable_mouse_capture`).
- Right-click drop site: `crates/fdemon-app/src/handler/mouse.rs` (no `Right` arm in `Normal` mode).
- PLAN.md "Out of scope (v1)" passage that this fix supersedes: `workflow/plans/features/mouse-support/PLAN.md:165–170`.
- DECSET reference: <https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Mouse-Tracking>
- crossterm split-capture-modes issue: <https://github.com/crossterm-rs/crossterm/issues/947>
- arboard crate: <https://docs.rs/arboard/>
