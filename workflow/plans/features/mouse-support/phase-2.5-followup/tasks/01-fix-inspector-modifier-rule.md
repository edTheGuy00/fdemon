## Task: Fix Inspector modifier rule to match peer handlers

**Objective**: Replace the `handle_inspector_scroll` modifier guard in `crates/fdemon-app/src/handler/mouse/devtools.rs` so `Shift+Ctrl+wheel` and `Shift+Alt+wheel` return `None` (matching `normal.rs`, `link_highlight.rs`, and `handle_network_scroll`). Also update the file's module doc and the inline comment to reflect the new uniform rule.

**Depends on**: None

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/devtools.rs`: Change the `handle_inspector_scroll` modifier guard from `if !mods.shift && (mods.ctrl || mods.alt)` to `if mods.ctrl || mods.alt`. Update the inline comment to drop the "small UX win for shift-held scrolls" hedge. Update the module-level `//!` doc to drop the implicit assumption that Shift+modifier still navigates. Add a unit test asserting `Shift+Ctrl+wheel` returns `None` for Inspector.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/keys.rs`: Reference for keyboard parity — Inspector keyboard handler at `keys.rs:537-548` does not bind any Shift+modifier combo to navigation, so this change brings mouse into alignment with keyboard behavior.
- `crates/fdemon-app/src/handler/mouse/normal.rs`: Reference for the prevailing modifier discipline (`is_shift_only` + `mods.ctrl || mods.alt → None`).

### Details

Today the guard reads:

```rust
fn handle_inspector_scroll(dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    // Inspector has no page-step navigation — Shift+wheel falls back to a
    // single-step move rather than no-op (small UX win for shift-held scrolls).
    // Ctrl/Alt with no Shift returns None as in normal mode.
    if !mods.shift && (mods.ctrl || mods.alt) {
        return None;
    }
    match dir {
        ScrollDir::Up => Some(Message::DevToolsInspectorNavigate(InspectorNav::Up)),
        ScrollDir::Down => Some(Message::DevToolsInspectorNavigate(InspectorNav::Down)),
        ScrollDir::Left | ScrollDir::Right => None,
    }
}
```

This means `Shift+Ctrl+wheel` (and `Shift+Alt+wheel`) produce `Some(InspectorNavigate(Up/Down))` because the guard's `!mods.shift` short-circuits when Shift is held. Every other handler in the phase rejects those combos via the `is_shift_only()` discipline declared in `TASKS.md` for the parent phase.

Replace with a uniform rule:

```rust
fn handle_inspector_scroll(dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    // Inspector has no page-step navigation — there is no `InspectorNav::PageUp`
    // analogue. Any modifier combination (including Shift, Ctrl, Alt) returns
    // None for parity with normal.rs / link_highlight.rs / handle_network_scroll.
    if mods.shift || mods.ctrl || mods.alt {
        return None;
    }
    match dir {
        ScrollDir::Up => Some(Message::DevToolsInspectorNavigate(InspectorNav::Up)),
        ScrollDir::Down => Some(Message::DevToolsInspectorNavigate(InspectorNav::Down)),
        ScrollDir::Left | ScrollDir::Right => None,
    }
}
```

The rejection of `Shift`-alone is a behavior change from Phase 2: a user holding Shift while wheeling in Inspector will now get nothing rather than single-step navigation. Per the review, this is the simpler and more discoverable rule — and the lost behavior is genuinely tiny.

Also update the module `//!` doc at the top of the file. Today it says:

```rust
//! - Inspector → tree row navigation (Up/Down only; no page step)
```

Change to:

```rust
//! - Inspector → tree row navigation (Up/Down with no modifiers; any modifier
//!   returns None because there is no page-step analogue for the inspector tree)
```

### Acceptance Criteria

1. `handle_inspector_scroll(ScrollDir::Up, KeyModSet::new(true, false, false))` (Shift-only) returns `None`.
2. `handle_inspector_scroll(ScrollDir::Up, KeyModSet::new(true, true, false))` (Shift+Ctrl) returns `None`.
3. `handle_inspector_scroll(ScrollDir::Up, KeyModSet::new(true, false, true))` (Shift+Alt) returns `None`.
4. `handle_inspector_scroll(ScrollDir::Up, KeyModSet::new(false, true, false))` (Ctrl-only) returns `None` (unchanged from before).
5. `handle_inspector_scroll(ScrollDir::Up, KeyModSet::NONE)` returns `Some(DevToolsInspectorNavigate(InspectorNav::Up))` (unchanged from before).
6. The module `//!` doc for `devtools.rs` accurately describes the new rule (any modifier on Inspector → `None`).
7. `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

Add a new test alongside the existing Inspector tests in `devtools.rs::tests`:

```rust
#[test]
fn inspector_any_modifier_combination_returns_none() {
    let s = state_with_panel(DevToolsPanel::Inspector);
    let combos = [
        KeyModSet::new(true, false, false),  // Shift
        KeyModSet::new(false, true, false),  // Ctrl
        KeyModSet::new(false, false, true),  // Alt
        KeyModSet::new(true, true, false),   // Shift+Ctrl
        KeyModSet::new(true, false, true),   // Shift+Alt
        KeyModSet::new(false, true, true),   // Ctrl+Alt
        KeyModSet::new(true, true, true),    // Shift+Ctrl+Alt
    ];
    for mods in combos {
        for dir in [ScrollDir::Up, ScrollDir::Down] {
            assert!(
                handle_scroll(&s, dir, mods).is_none(),
                "expected None for Inspector + {:?} + {:?}",
                dir,
                mods
            );
        }
    }
}
```

The pre-existing `ctrl_or_alt_only_is_no_op_in_inspector_and_network` test already covers Ctrl-only and Alt-only; the new test extends coverage to every Shift-bearing combination.

Run:

```bash
cargo test -p fdemon-app handler::mouse::devtools
cargo test --workspace
```

### Notes

- **This is a behavior change for users.** Pre-Phase-2.5: Shift+wheel in Inspector navigated single-step. Post-Phase-2.5: Shift+wheel in Inspector does nothing. The user can still press `j`/`k` or `Up`/`Down` on the keyboard to navigate, so functionality is not lost.
- **Why now and not Phase 2.** Phase 2 shipped with the asymmetric rule because each implementor wrote their submodule independently and the plan's `is_shift_only` discipline was not enforced uniformly. The review caught it; Phase 2.5 fixes it before Phase 3 builds on this foundation.
- **DO NOT touch `mod.rs`.** Per the Phase 2.5 plan, `mod.rs` is exclusively owned by Task 04. The existing positive-assertion test in `mod.rs` (`test_devtools_scroll_routes_to_inspector_nav`) uses `make_scroll_up()` with no modifiers, so it still passes after this change.
- **DO NOT touch `tests.rs`.** The integration test `mouse_scroll_devtools_performance_shift_up_produces_none` is for Performance panel, not Inspector. Inspector integration tests use no modifiers and are unaffected.
- **Follow-up consideration.** If a future phase adds `InspectorNav::PageUp/PageDown`, this guard becomes too aggressive — Shift would then be a legitimate page-step trigger. At that point split the rule like `handle_network_scroll` does. Out of scope for Phase 2.5.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/devtools.rs` | Changed `handle_inspector_scroll` guard from `if !mods.shift && (mods.ctrl \|\| mods.alt)` to `if mods.shift \|\| mods.ctrl \|\| mods.alt`. Updated inline comment and module `//!` doc. Added `inspector_any_modifier_combination_returns_none` test. |

### Notable Decisions/Tradeoffs

1. **Uniform modifier discipline**: The guard is now a single `||` expression matching every other handler (normal.rs, link_highlight.rs, handle_network_scroll). Shift-only is now a no-op for Inspector; prior behavior (single-step on Shift+wheel) was a Phase 2 divergence caught in review.
2. **Comment spacing**: The task's example used two spaces before inline comments (`// Shift`), but `cargo fmt` normalizes to one space. Used single-space alignment to pass the formatter.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test -p fdemon-app handler::mouse::devtools` — Passed (9 tests)
- `cargo test --workspace` — Passed (all suites green)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Behavior change for users:** Shift+wheel in Inspector no longer single-steps the tree. Documented as intentional in the new module doc. Users retain keyboard `j`/`k`/`Up`/`Down` navigation.
