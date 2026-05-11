## Task: Mouse handler directory restructure + scroll dispatcher skeleton

**Objective**: Convert `crates/fdemon-app/src/handler/mouse.rs` into a `handler/mouse/` directory module with one stub submodule per UI-mode group, wire a per-`UiMode` scroll dispatcher in `mod.rs`, and add `KeyModSet::is_shift_only()` to `input_mouse.rs`. After this task, every Phase 2 follow-up can fill in its own submodule file in parallel.

**Depends on**: None (Phase 1.5 prerequisite — see TASKS.md "Prerequisites" section)

**Estimated Time**: 1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mod.rs` — declare the new `mouse` module as a directory module (no functional change beyond removing the file-module declaration if the path resolution differs).
- `crates/fdemon-app/src/handler/mouse.rs` — **DELETE** (content moves into `handler/mouse/mod.rs`).
- `crates/fdemon-app/src/handler/mouse/mod.rs` — **NEW** Top-level entry. Defines `pub fn handle_mouse(state, input) -> Option<Message>` matching on `MouseInput` variant; for `Scroll`, dispatches to `handle_scroll(state, dir, mods)` which matches on `state.ui_mode` and delegates to the appropriate submodule. For `Press` / `Release` / `Drag`, returns `None` (Phase 3+ wires those). Re-exports `pub(super)` no-op test helpers from previous file.
- `crates/fdemon-app/src/handler/mouse/normal.rs` — **NEW** stub. `pub(super) fn handle_scroll(state: &AppState, dir: ScrollDir, mods: KeyModSet) -> Option<Message>` returning `None`.
- `crates/fdemon-app/src/handler/mouse/devtools.rs` — **NEW** stub (same signature, returns `None`).
- `crates/fdemon-app/src/handler/mouse/settings.rs` — **NEW** stub (same).
- `crates/fdemon-app/src/handler/mouse/new_session.rs` — **NEW** stub (same).
- `crates/fdemon-app/src/handler/mouse/link_highlight.rs` — **NEW** stub (same).
- `crates/fdemon-app/src/handler/mouse/flutter_version.rs` — **NEW** stub (same).
- `crates/fdemon-app/src/input_mouse.rs` — Add `pub fn is_shift_only(self) -> bool` on `KeyModSet` returning `self.shift && !self.ctrl && !self.alt`. Add a unit test.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `UiMode` enum variants (line 23-59).
- `crates/fdemon-app/src/handler/keys.rs` — Reference dispatcher pattern at line 9-22 (`handle_key` matches `state.ui_mode` and dispatches to per-mode helpers).

### Details

**Step 1 — Move `mouse.rs` content into `mouse/mod.rs`.**

Today the file lives at `crates/fdemon-app/src/handler/mouse.rs`. After this task it lives at `crates/fdemon-app/src/handler/mouse/mod.rs`. The existing `pub fn handle_mouse(state, input) -> Option<Message>` and its no-op tests move verbatim into `mod.rs`, then `handle_mouse` is rewritten as a thin dispatcher:

```rust
//! Mouse event handlers for different UI modes.
//!
//! Mirrors [`crate::handler::keys`] — converts a [`MouseInput`] into a
//! concrete [`Message`] based on the current [`UiMode`]. Phase 2 wires
//! per-mode scroll routing; Phase 3+ adds click hit-testing.

mod devtools;
mod flutter_version;
mod link_highlight;
mod new_session;
mod normal;
mod settings;

use crate::input_mouse::{KeyModSet, MouseInput, ScrollDir};
use crate::message::Message;
use crate::state::{AppState, UiMode};

/// Convert a mouse event to a follow-up message based on the current UI mode.
///
/// In Phase 2 only [`MouseInput::Scroll`] produces messages; the press,
/// release, and drag variants are reserved for Phase 3+ click hit-testing
/// and currently return `None` for every mode.
pub fn handle_mouse(state: &AppState, input: MouseInput) -> Option<Message> {
    match input {
        MouseInput::Scroll {
            direction,
            modifiers,
            ..
        } => handle_scroll(state, direction, modifiers),
        // Phase 3+ wires button-press dispatch (region hit-testing).
        MouseInput::Press { .. } | MouseInput::Release { .. } | MouseInput::Drag { .. } => None,
    }
}

fn handle_scroll(state: &AppState, dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    match state.ui_mode {
        UiMode::Normal => normal::handle_scroll(state, dir, mods),
        UiMode::DevTools => devtools::handle_scroll(state, dir, mods),
        UiMode::Settings => settings::handle_scroll(state, dir, mods),
        UiMode::Startup | UiMode::NewSessionDialog => {
            new_session::handle_scroll(state, dir, mods)
        }
        UiMode::LinkHighlight => link_highlight::handle_scroll(state, dir, mods),
        UiMode::FlutterVersion => flutter_version::handle_scroll(state, dir, mods),
        // Modes with no scrollable surface — explicitly no-op.
        UiMode::SearchInput
        | UiMode::ConfirmDialog
        | UiMode::EmulatorSelector
        | UiMode::Loading => None,
    }
}

#[cfg(test)]
mod tests {
    // ... existing no-op tests for Press/Release/Drag in every mode move here.
    // These continue to pass because non-Scroll variants always return None.
}
```

**Step 2 — Create six stub submodules.**

Each file follows the same shape:

```rust
//! Scroll routing for `<UiMode variant>`.
//!
//! Phase 2 task <NN>-<slug> populates the body. The stub returns `None`
//! so the dispatcher compiles and tests stay green between waves.

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::state::AppState;

pub(super) fn handle_scroll(
    _state: &AppState,
    _dir: ScrollDir,
    _mods: KeyModSet,
) -> Option<Message> {
    None
}
```

Use `_`-prefixed parameters in the stub so clippy is happy with `-D warnings`.

**Step 3 — Add `KeyModSet::is_shift_only()`.**

In `crates/fdemon-app/src/input_mouse.rs`, append to `impl KeyModSet`:

```rust
/// Returns `true` when only the Shift modifier is held (no Ctrl, no Alt).
///
/// Used by mouse scroll handlers to detect Shift+wheel for page-scroll
/// without false-firing when Ctrl or Alt is also held.
pub const fn is_shift_only(self) -> bool {
    self.shift && !self.ctrl && !self.alt
}
```

Add a unit test alongside the existing `KeyModSet` tests:

```rust
#[test]
fn test_is_shift_only_distinguishes_pure_shift_from_combos() {
    assert!(KeyModSet::new(true, false, false).is_shift_only());
    assert!(!KeyModSet::new(false, false, false).is_shift_only());
    assert!(!KeyModSet::new(true, true, false).is_shift_only());
    assert!(!KeyModSet::new(true, false, true).is_shift_only());
    assert!(!KeyModSet::new(true, true, true).is_shift_only());
}
```

**Step 4 — Update `handler/mod.rs` if needed.**

The current declaration is `pub(crate) mod mouse;`. With the directory layout this stays unchanged — Rust resolves both `mouse.rs` and `mouse/mod.rs` from the same `mod mouse;` declaration. Verify no other file imports `handler::mouse::<private>` symbols; if any does, retain or re-export them through `mod.rs`.

### Acceptance Criteria

1. `crates/fdemon-app/src/handler/mouse.rs` no longer exists.
2. `crates/fdemon-app/src/handler/mouse/` exists as a directory containing `mod.rs` and six submodule stubs.
3. `handle_mouse(state, MouseInput::Scroll { ... })` returns `None` for every `UiMode` variant (because every submodule stub returns `None`).
4. `handle_mouse(state, MouseInput::Press|Release|Drag { ... })` returns `None` for every `UiMode` variant.
5. The existing no-op tests from the old `mouse.rs` (test_press_no_op_in_every_mode, test_scroll_no_op_in_every_mode — adjusted for the rename to `Press`) still pass after the move.
6. `KeyModSet::is_shift_only()` exists and the new unit test passes.
7. `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` all pass.
8. No production code outside `handler/mouse/` changed (apart from `input_mouse.rs` for the helper).

### Testing

```bash
cargo test -p fdemon-app input_mouse::tests::is_shift_only
cargo test -p fdemon-app handler::mouse
cargo test -p fdemon-app  # full crate sanity
cargo clippy -p fdemon-app --all-targets -- -D warnings
```

The pre-existing no-op tests must keep passing without modification beyond renaming `Click` → `Press` (which Phase 1.5 Task 01 has already done).

### Notes

- **No behavior change for users.** The dispatcher and stubs all return `None`; this task is pure refactor + helper addition.
- **Why six submodules and not one per `UiMode`?** Three modes (`SearchInput`, `ConfirmDialog`, `EmulatorSelector`, `Loading`) have nothing to scroll and never will — inlining their no-ops in the dispatcher is shorter and clearer than six empty files. Two modes (`Startup` and `NewSessionDialog`) share one handler because `handle_key_new_session_dialog` already serves both (`keys.rs:11`).
- **Why `pub(super) fn handle_scroll`?** Submodule helpers are crate-private and called only from the parent `mod.rs`. `pub(super)` is the tightest scope that compiles.
- **`MouseInput::Press` vs `Click`.** Phase 1.5 Task 01 renames `Click` → `Press`. This task assumes the rename has landed. If it has not, substitute `Click` everywhere `Press` appears and add a TODO comment to switch on Phase 1.5 merge.
- **`is_shift_only` alternatives considered.** Inlining the bool expression three times (once per consuming mode) was rejected for drift risk; making it a free function in `input_mouse.rs` was rejected because methods on the type read more naturally at the call site. `const` was chosen so the helper can be evaluated in match guards if needed in future phases.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse.rs` | Deleted — content moved to directory module |
| `crates/fdemon-app/src/handler/mouse/mod.rs` | New — top-level dispatcher matching on MouseInput variant; scroll delegates to handle_scroll which dispatches by UiMode; existing no-op tests moved here |
| `crates/fdemon-app/src/handler/mouse/normal.rs` | New — stub returning None |
| `crates/fdemon-app/src/handler/mouse/devtools.rs` | New — stub returning None |
| `crates/fdemon-app/src/handler/mouse/settings.rs` | New — stub returning None |
| `crates/fdemon-app/src/handler/mouse/new_session.rs` | New — stub returning None (serves Startup + NewSessionDialog) |
| `crates/fdemon-app/src/handler/mouse/link_highlight.rs` | New — stub returning None |
| `crates/fdemon-app/src/handler/mouse/flutter_version.rs` | New — stub returning None |
| `crates/fdemon-app/src/input_mouse.rs` | Added `KeyModSet::is_shift_only() -> bool` const method and unit test |

### Notable Decisions/Tradeoffs

1. **Rustfmt single-line match arm**: The `Startup | NewSessionDialog` multi-pattern arm was formatted as a single line by rustfmt (without a block), matching the style of the surrounding arms.
2. **handler/mod.rs unchanged**: The `pub(crate) mod mouse;` declaration is identical for both `mouse.rs` and `mouse/mod.rs` — Rust resolves both paths from the same declaration, so no change was needed.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app is_shift_only` - Passed (1 test)
- `cargo test -p fdemon-app handler::mouse` - Passed (2 tests)
- `cargo test --workspace` - Passed (all tests across all crates)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **No behavior change**: All stubs return None; this is a pure structural refactor + helper addition. Phase 2 follow-up tasks will populate each submodule.
