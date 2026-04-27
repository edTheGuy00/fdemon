## Task: Clean up clippy warnings in `fdemon-daemon`

**Objective**: Resolve all clippy warnings in `fdemon-daemon` so `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` exits 0.

**Depends on**: None

**Estimated Time**: 0.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/devices.rs` — replace `map.get("Windows").is_none()` with `!map.contains_key("Windows")` (~line 654).
- `crates/fdemon-daemon/src/native_logs/custom.rs` — convert three `loop { match … }` blocks to `while let` (~lines 450, 491, 719).
- `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` — fix doc-comment list indentation (~lines 450–451).
- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` — replace three `assert_eq!(x, true)` / `assert_eq!(x, false)` calls with `assert!(x)` / `assert!(!x)` (~lines 308, 314, 340).
- `crates/fdemon-daemon/src/vm_service/extensions/overlays.rs` — convert `let mut state = DebugOverlayState::default(); state.repaint_rainbow = Some(true);` (~line 223–224) into a struct-literal `DebugOverlayState { repaint_rainbow: Some(true), ..Default::default() }`.

**Files Read (Dependencies):**
- None.

### Warning Inventory (10 total)

| Lint | Count | Locations |
|------|-------|-----------|
| `clippy::unnecessary_get_then_check` | 1 | `devices.rs:654` |
| `clippy::while_let_loop` | 3 | `native_logs/custom.rs:450, 491, 719` |
| `clippy::doc_lazy_continuation` | 2 | `vm_service/extensions/inspector.rs:450, 451` (one warning, two highlights) |
| `clippy::field_reassign_with_default` | 1 | `vm_service/extensions/overlays.rs:224` |
| `clippy::bool_assert_comparison` | 3 | `vm_service/extensions/mod.rs:308, 314, 340` |

### Procedure

1. From the repo root, apply mechanical fixes:
   ```bash
   cargo clippy --fix -p fdemon-daemon --all-targets --allow-dirty
   ```
   `--fix` will resolve `bool_assert_comparison`, `unnecessary_get_then_check`, and `while_let_loop` automatically.
2. Hand-fix the remaining cases clippy can't auto-rewrite:
   - **`field_reassign_with_default`** in `overlays.rs`: rewrite the `Default::default()` + reassignment block as a struct literal, e.g.
     ```rust
     let state = DebugOverlayState {
         repaint_rainbow: Some(true),
         ..Default::default()
     };
     ```
   - **`doc_lazy_continuation`** in `inspector.rs`: indent the wrapped doc-comment list lines so they align with the bullet marker. Inspect the doc block around line 450 and add the necessary leading spaces so the markdown renders as one list item rather than two.
3. Run the per-crate gate:
   ```bash
   cargo clippy -p fdemon-daemon --all-targets -- -D warnings
   ```
4. Run unit tests:
   ```bash
   cargo test -p fdemon-daemon
   ```
   Must remain at the existing 527 unit-test count (no regressions).
5. Format:
   ```bash
   cargo fmt --all
   ```

### Acceptance Criteria

1. `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` exits 0.
2. `cargo test -p fdemon-daemon` passes (no regressions).
3. Diff is limited to the five files listed above under `crates/fdemon-daemon/src/`.
4. No behavior changes; the doc-comment fix preserves the existing prose, only the indentation changes.

### Notes

- The `while_let_loop` fix is mechanical but each loop body should be re-read to confirm the implicit `break` paths line up with the new `while let` shape. If the original `match` had side effects in the non-`Some` arms, preserve them.
- The doc-comment fix only affects rustdoc rendering; it does not change runtime behavior.

---

## Completion Summary

**Status:** Not Started
**Branch:** _to be filled by implementor_

### Files Modified

| File | Changes |
|------|---------|
| _tbd_ | _tbd_ |

### Notable Decisions/Tradeoffs

_tbd_

### Testing Performed

- `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` — _tbd_
- `cargo test -p fdemon-daemon` — _tbd_
- `cargo fmt --all -- --check` — _tbd_

### Risks/Limitations

_tbd_
