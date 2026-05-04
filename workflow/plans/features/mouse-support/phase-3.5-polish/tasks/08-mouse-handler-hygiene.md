# Task 08: Mouse handler hygiene — lift `tag_filter_visible` + doc `handle_scroll`

**Status:** Not Started
**Estimated Hours:** 0.5h
**Depends On:** —
**Crate / Area:** `fdemon-app`

## Goal

Discharge two Phase-3 review findings that both touch `crates/fdemon-app/src/handler/mouse/`:

1. **Lift `tag_filter_visible` early-return into the dispatcher** (review item 10): Currently `normal::handle_press` checks `state.tag_filter_visible` at lines 33–35 and returns `None` early. This is a cross-mode concern — Phase 4/5 will add `handle_press` impls for DevTools, Settings, and dialog modes, and each will have to remember the gate. Lift it to `mouse::handle_press` (the dispatcher in `handler/mouse/mod.rs`) so all per-mode handlers inherit it for free, mirroring the keyboard handler's `keys.rs:105-126` intercept structure.

2. **Add a `///` doc comment to `handle_scroll`** (review item 9): `pub(super) fn handle_scroll` at line 75 of `normal.rs` has no doc comment, while the sibling `handle_press` has a thorough one. Per `docs/CODE_STANDARDS.md`, even `pub(super)` items deserve docs. Mirror the level of detail from `handle_press`.

Bundling these into one task because both touch `handler/mouse/normal.rs` — splitting would create a same-file sequential pair with no parallelism win.

## Files Modified (Write)

- `crates/fdemon-app/src/handler/mouse/mod.rs`
- `crates/fdemon-app/src/handler/mouse/normal.rs`

## Files Read

- `crates/fdemon-app/src/handler/keys.rs` — confirm the parallel `tag_filter_visible` intercept pattern (around lines 105–126) to cite in a comment

## Implementation Steps

### Part A — Lift `tag_filter_visible` to the dispatcher

1. **In `handler/mouse/mod.rs::handle_press`,** add a `tag_filter_visible` early-return *before* the per-mode dispatch:
   ```rust
   pub(crate) fn handle_press(
       state: &AppState,
       x: u16,
       y: u16,
       button: MouseButton,
       mods: KeyModSet,
   ) -> Option<Message> {
       // Tag-filter overlay intercepts all input regardless of underlying UiMode.
       // Mirrors the keyboard handler at `handler/keys.rs:105-126`.
       if state.tag_filter_visible {
           return None;
       }

       match state.ui_mode {
           UiMode::Normal => normal::handle_press(state, x, y, button, mods),
           // Phase 5 wires DevTools/Settings/dialog modes; for now, no-op.
           _ => None,
       }
   }
   ```

2. **In `handler/mouse/normal.rs::handle_press`,** remove the now-redundant early-return at lines 33–35:
   ```rust
   // Tag-filter overlay intercepts all input
   if state.tag_filter_visible {
       return None;
   }
   ```
   Replace with a comment noting the new location:
   ```rust
   // tag_filter_visible is gated at the dispatcher (handler/mouse/mod.rs::handle_press).
   ```

3. **Update or remove tests that explicitly verified the per-mode `tag_filter_visible` short-circuit.** The test `press_when_tag_filter_visible_is_no_op` in `normal.rs` still passes through the dispatcher — but it currently calls `normal::handle_press` directly, bypassing the dispatcher. Either:
   - Re-target the test to call `mouse::handle_press` (the dispatcher entry point) so the new gate is exercised, OR
   - Add a *new* test in `handler/mouse/mod.rs` (or wherever dispatcher tests live) that asserts `mouse::handle_press` returns `None` when `tag_filter_visible == true` regardless of `ui_mode`, AND keep the per-mode test as a redundant safeguard.

   Prefer option (a) — the dispatcher is now the canonical gate.

### Part B — Add `handle_scroll` doc comment

4. **Add a `///` doc block above `pub(super) fn handle_scroll`** at `normal.rs:75`. Mirror the level of detail in the existing `handle_press` doc:
   - One-line summary of what `handle_scroll` does.
   - Notes on inputs (`x`, `y`, `direction`, `mods`).
   - Notes on which modes return `None` (currently: tag-filter visible, plus any per-`UiMode` short-circuits).
   - Reference the parallel `keys.rs` scroll/page handler if there is one.
   - Cross-link to `mouse::handle_scroll` (the dispatcher entry point) if applicable.

   Example structure (adjust to match the actual function body):
   ```rust
   /// Handle a mouse scroll event in `UiMode::Normal`.
   ///
   /// Routes vertical wheel deltas to:
   /// - `Message::ScrollUpPage` / `ScrollDownPage` if `Shift` is held (paged scroll).
   /// - `Message::ScrollUp` / `ScrollDown` otherwise (line scroll).
   ///
   /// Returns `None` when no Normal-mode scroll target is active (e.g., focused widget
   /// does not consume scroll). Modifier filtering matches the keyboard handler at
   /// `handler/keys.rs:<lines>` to keep `Shift+Wheel` consistent across input modalities.
   pub(super) fn handle_scroll(...) -> Option<Message> { ... }
   ```

## Acceptance Criteria

- [ ] `mouse::handle_press` (dispatcher) returns `None` early when `state.tag_filter_visible == true`, before consulting `ui_mode`
- [ ] `normal::handle_press` no longer contains a `tag_filter_visible` early-return; a comment in its place points to the dispatcher
- [ ] `handle_scroll` in `normal.rs` carries a `///` doc block mirroring the level of detail in `handle_press`'s doc
- [ ] At least one test exists asserting `mouse::handle_press` returns `None` for `tag_filter_visible == true` (regardless of `ui_mode`) — either a re-targeted version of the existing `press_when_tag_filter_visible_is_no_op` or a new dispatcher-level test
- [ ] `cargo test -p fdemon-app handler::mouse` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes

## Notes

- This task changes the *location* of an existing gate, not its behavior. The integration test `view_header_regions_present_in_settings_mode_because_header_always_renders` and the manual smoke-test bullets in Phase 3's TASKS.md should still pass unchanged.
- Future per-mode handlers (Phase 4/5) will benefit because they no longer need to remember to repeat the tag-filter check.
- Do not lift the *busy gate* (`HotReload`/`HotRestart`/`StopApp` short-circuit) into the dispatcher — that gate is per-message, not per-mode, and lives correctly inside `normal::handle_press` after the registry hit-test.
- Be careful when re-targeting the existing test: the dispatcher takes `&AppState` while `normal::handle_press` may take a slightly different signature. Match the public dispatcher signature.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/mod.rs` | Added `tag_filter_visible` early-return to `handle_press` dispatcher before the `ui_mode` match; added dispatcher-level test `dispatcher_press_tag_filter_visible_is_no_op` covering all 11 UiMode variants |
| `crates/fdemon-app/src/handler/mouse/normal.rs` | Removed redundant `tag_filter_visible` early-return from `handle_press`; replaced with a comment pointing to the dispatcher; added 24-line `///` doc block to `handle_scroll`; replaced `press_when_tag_filter_visible_is_no_op` test with a relocation note |

### Notable Decisions/Tradeoffs

1. **Test relocation strategy**: Chose option (a) from the task spec — re-targeted the existing test to the dispatcher level (`mod.rs`) rather than keeping a redundant normal-level test. The normal.rs test body was replaced with a comment noting the relocation to keep the test count accurate and avoid testing a gate that no longer exists at that level.

2. **Dispatcher test covers all modes**: The new `dispatcher_press_tag_filter_visible_is_no_op` test iterates all 11 `UiMode` variants (not just `Normal`) to confirm the dispatcher gate is truly mode-agnostic.

3. **`handle_scroll` doc**: Documented three routing branches (tag-filter nav, page scroll, line scroll), cross-linked to `handler/keys.rs:265-270` for the keyboard parallels, and noted the horizontal-wheel no-op behavior and the Ctrl/Alt intent.

### Testing Performed

- `cargo test -p fdemon-app handler::mouse` — Passed (68 tests, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo fmt --all -- --check` — Passed
