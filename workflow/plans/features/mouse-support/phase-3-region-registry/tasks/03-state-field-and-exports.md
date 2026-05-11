## Task: Add `mouse_regions` Field to `AppState` and Public Re-exports

**Objective**: Add a `Cell<MouseRegions>` field to `AppState`, default-initialize it, and re-export the registry types from `lib.rs` so the TUI can use them.

**Depends on**: 01

**Estimated Time**: 30 minutes

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs`: Add `pub mouse_regions: Cell<MouseRegions>` field with documentation. Initialize to `Cell::new(MouseRegions::with_capacity())` in `AppState::with_settings`. Update `AppState`'s `Debug` derive only if `MouseRegions: Debug` is missing (it isn't — Task 01 derives `Debug`).
- `crates/fdemon-app/src/lib.rs`: Promote `mouse_regions` from `pub(crate) mod` to `pub mod`, and add `pub use mouse_regions::{MouseAction, MouseRect, MouseRegionEntry, MouseRegions, MouseRegionsBuilder};` next to the existing `pub use input_mouse::{...};` line.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/mouse_regions.rs` (Task 01): Source of the types being re-exported and the field type.

### Details

#### State field

In `state.rs`, in the `AppState` struct (around line 1009 — the last field is `show_migration_banner`), add:

```rust
/// Per-frame mouse click-region registry.
///
/// Populated by widgets during render via [`crate::mouse_regions::MouseRegionsBuilder`]
/// and read by [`crate::handler::mouse`] during click hit-tests. Lives on
/// `AppState` (rather than being threaded through the handler layer) because
/// `Cell` interior mutability lets render write back without forcing
/// `&mut AppState` everywhere.
///
/// **TEA exception**: This is the same exception class as
/// [`TagFilterUiState::last_known_visible_height`] — a render-hint write-back
/// that does NOT participate in business logic or state equality. See
/// `docs/CODE_STANDARDS.md` Principle 3 for rationale.
///
/// Lifecycle (per frame):
/// 1. `render::view` calls `state.mouse_regions.take()`, draining the previous
///    frame's entries (the `Cell` now holds an empty `MouseRegions`).
/// 2. Widgets push entries into a `MouseRegionsBuilder` borrowed against the
///    drained instance.
/// 3. `render::view` calls `state.mouse_regions.set(populated)` to put the
///    new registry back.
/// 4. On `Message::Mouse(MouseInput::Press {..})`, `handler::mouse::normal`
///    performs the same take/hit-test/put-back dance.
// EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md Principle 3
pub mouse_regions: Cell<MouseRegions>,
```

In `AppState::with_settings` (around line 1035), add to the struct literal:

```rust
mouse_regions: Cell::new(MouseRegions::with_capacity()),
```

Imports: `state.rs` already imports `std::cell::Cell` (line 3) — no new top-level import needed for `Cell`. Add `use crate::mouse_regions::MouseRegions;` next to the existing `crate::*` use lines (around lines 11-15).

#### lib.rs

Promote and re-export. Find the existing line (added by Task 01):

```rust
pub(crate) mod mouse_regions;
```

Change it to:

```rust
pub mod mouse_regions;
```

(Or keep it `pub(crate)` and only re-export the types — both work. Promoting to `pub mod` is consistent with `pub mod state`, `pub mod message`, etc. and lets external consumers reach `MouseRegionEntry` directly.)

Then add the re-export line (next to the existing `pub use input_mouse::{...};` at line 105):

```rust
// Re-export mouse region types used by TUI for region recording (Phase 3)
pub use mouse_regions::{MouseAction, MouseRect, MouseRegionEntry, MouseRegions, MouseRegionsBuilder};
```

### Acceptance Criteria

1. `AppState::mouse_regions: Cell<MouseRegions>` exists with the documentation block above (including the `// EXCEPTION:` comment).
2. `AppState::with_settings` initializes the field with `Cell::new(MouseRegions::with_capacity())`.
3. `AppState::default()` (which delegates to `with_settings(empty, default)`) produces a state with an empty registry.
4. `lib.rs` re-exports the five types listed above; existing imports in tests and downstream crates still compile.
5. `cargo check -p fdemon-app` and `cargo check --workspace` both pass.
6. No clippy warnings.
7. The new field does not appear in any `PartialEq` derive on `AppState` (it isn't currently, since `AppState` only derives `Debug` — verify before merging).

### Testing

Add a single sanity test to `state.rs::tests` (the existing `mod tests` block at the bottom):

```rust
#[test]
fn test_appstate_initializes_with_empty_mouse_regions() {
    let state = AppState::new();
    let regions = state.mouse_regions.take();
    assert!(regions.is_empty(), "fresh AppState has no mouse regions");
    state.mouse_regions.set(regions); // restore so the assertion is non-destructive
}

#[test]
fn test_appstate_mouse_regions_capacity_preserves() {
    let state = AppState::new();
    let regions = state.mouse_regions.take();
    // with_capacity() pre-sizes to 32 — we don't lock that number into a test,
    // but we do assert that capacity is non-zero so a single push doesn't
    // immediately realloc.
    assert!(regions.iter().count() == 0);
    state.mouse_regions.set(regions);
}
```

Also touch the existing `Debug` smoke test if any — `AppState` derives `Debug`, and `Cell<MouseRegions>` is `Debug` (since `MouseRegions: Debug` from Task 01). Confirm by running the full suite.

### Notes

- This task is the bridge between Task 01 (pure types) and the TUI/handler tasks (which need `state.mouse_regions` to exist). It is intentionally tiny — a single field, two re-exports.
- Do NOT add hit-test helpers on `AppState` here. Task 05 owns the take/hit-test/put-back dance in `handler/mouse/normal.rs`.
- If `AppState`'s `Debug` derive starts emitting noisy `MouseRegions` content in test failures, consider adding a manual `Debug` impl that elides the field. Defer unless someone complains — for now, an empty registry's debug output is just `MouseRegions { entries: [] }`, which is harmless.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/mouse_regions.rs` | Removed `#![allow(dead_code)]`; added `MouseRegionsCell` newtype (wraps `Cell<MouseRegions>`, provides manual `Debug` impl); added `use std::cell::Cell;` import |
| `crates/fdemon-app/src/state.rs` | Added `use crate::mouse_regions::{MouseRegions, MouseRegionsCell};`; added `pub mouse_regions: MouseRegionsCell` field with full doc block; initialized in `with_settings`; added two tests |
| `crates/fdemon-app/src/lib.rs` | Promoted `pub(crate) mod mouse_regions` to `pub mod`; added `pub use mouse_regions::{MouseAction, MouseRect, MouseRegionEntry, MouseRegions, MouseRegionsBuilder, MouseRegionsCell}` re-export |

### Notable Decisions/Tradeoffs

1. **`MouseRegionsCell` newtype instead of bare `Cell<MouseRegions>`**: `Cell<T>` only derives `Debug` when `T: Copy`. `MouseRegions` holds a `Vec` so cannot be `Copy`. Rather than removing `#[derive(Debug)]` from `AppState` (which would require a large manual impl), a thin `MouseRegionsCell` newtype was added to `mouse_regions.rs`. It delegates `take()`/`set()` to the inner `Cell` and provides a minimal `Debug` impl showing only the type name. The public API and lifecycle semantics are identical to `Cell<MouseRegions>`. `MouseRegionsCell` is also exported from `lib.rs` since downstream TUI code may need to name the field type.

2. **`#![allow(dead_code)]` removed**: The module is now `pub` and `AppState` uses `MouseRegionsCell`, so all previously dead items are reachable. No suppression needed.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app` - Passed (2015 tests)
- `cargo test -p fdemon-app mouse_regions` - Passed (13 tests including 2 new state tests)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- `cargo fmt --all -- --check` - Passed

### Risks/Limitations

1. **Acceptance criteria specifies `Cell<MouseRegions>` exactly**: The field is `MouseRegionsCell` (a newtype over `Cell<MouseRegions>`) rather than the literal type. This is a necessary deviation to satisfy `#[derive(Debug)]` on `AppState`. The semantics are identical and the newtype is fully transparent.
