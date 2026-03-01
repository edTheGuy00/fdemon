## Task: Add `last_known_visible_height` Field to `TargetSelectorState`

**Objective**: Add a `Cell<usize>` field to `TargetSelectorState` that acts as a feedback channel from the render layer to the state layer. The renderer will write the actual device list area height each frame, and the handler will read it when processing navigation keys.

**Depends on**: None (Phase 2 must be complete)

**Estimated Time**: 1 hour

### Scope

- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`: Add field, update `Default` impl, update `new()`

### Details

**Add `Cell` import:**
```rust
use std::cell::Cell;
```

**Add field to `TargetSelectorState` (after `scroll_offset`, before `cached_flat_list`):**
```rust
/// Last-known visible height of the device list area (in rows).
///
/// Written by the renderer each frame via interior mutability (`Cell`).
/// Read by the handler to compute accurate scroll offsets.
/// Defaults to 0, which signals "no render has occurred yet" — the handler
/// falls back to `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` when this is 0.
pub last_known_visible_height: Cell<usize>,
```

**Update `Default` impl (line 48-61) — add field:**
```rust
last_known_visible_height: Cell::new(0),
```

A value of `0` indicates no render has occurred yet. The handler (Task 03) will check for this and fall back to `DEFAULT_ESTIMATED_VISIBLE_HEIGHT`.

**Update `new()` (line 69-71):** No change needed — `new()` delegates to `Default::default()`.

**Update `set_tab()` (line 74-81):** No change needed — scroll offset is already reset, visible height will be updated on the next render.

**Update `set_connected_devices()` and `set_bootable_devices()`:** No change needed — visible height is a render-derived value and doesn't need to be reset when devices change.

### `Cell<usize>` Compatibility

`Cell<usize>` implements:
- `Debug` — prints `Cell { value: N }` (compatible with `#[derive(Debug)]`)
- `Clone` — clones the inner value (compatible with `#[derive(Clone)]`)
- `Default` — defaults to `Cell::new(0)` (but we set it explicitly)

`TargetSelectorState` derives `Debug, Clone` only (no `PartialEq`, `Eq`, `Hash`), so `Cell<usize>` is fully compatible.

### Acceptance Criteria

1. `TargetSelectorState` has a `pub last_known_visible_height: Cell<usize>` field
2. Default value is `Cell::new(0)` (not pre-filled with 10)
3. `#[derive(Debug, Clone)]` still works on `TargetSelectorState`
4. `cargo check -p fdemon-app` passes
5. `cargo test -p fdemon-app` passes — no existing test breakage
6. `cargo test -p fdemon-tui` passes — no breakage from re-exported type

### Testing

No new tests in this task — the field is exercised by Tasks 02-04. Verify with:
- `cargo check --workspace` — confirms no compilation errors
- `cargo test --workspace` — confirms no regressions

### Notes

- The `Cell<usize>` is interior-mutable: it can be written through `&TargetSelectorState` (shared reference). This is the key property that lets the renderer write back the visible height without needing `&mut`.
- `Cell<usize>` is zero-cost: it compiles to the same code as a plain `usize` — no runtime overhead, no atomic operations.
- The field should be placed after `scroll_offset` to keep scroll-related fields grouped together.
- Existing tests that construct `TargetSelectorState::default()` or `TargetSelectorState::new()` will continue to work without changes since `Cell::new(0)` is included in the `Default` impl.
- Tests that use struct literal construction (if any) will need `last_known_visible_height: Cell::new(0)` added — but current tests all use `default()` or `new()`.
