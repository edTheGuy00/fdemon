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

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/devices.rs` | Replace `groups.get("Windows").is_none()` with `!groups.contains_key("Windows")` |
| `crates/fdemon-daemon/src/native_logs/custom.rs` | Convert three `loop { match ... }` blocks to `while let Ok(Some(event)) = ...` form |
| `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` | Indent two doc-comment continuation lines to 4 spaces so they render as list item continuations |
| `crates/fdemon-daemon/src/vm_service/extensions/overlays.rs` | Replace `Default::default()` + reassignment with struct literal `DebugOverlayState { repaint_rainbow: Some(true), ..Default::default() }` |
| `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` | Auto-fixed by `cargo clippy --fix`: 3x `assert_eq!(x, true/false)` → `assert!(x)` / `assert!(!x)` |

### Notable Decisions/Tradeoffs

1. **Auto-fix first**: Ran `cargo clippy --fix --allow-dirty` which resolved the `bool_assert_comparison` lint automatically. The other lints required manual edits.
2. **while_let_loop**: All three loops had symmetric `Ok(Some(event)) => push` + `_ => break` patterns, making them safe to convert to `while let Ok(Some(event)) = ...`. No side effects in the non-Some arms were lost.
3. **doc_lazy_continuation**: The original code had two continuation lines with only 2-space indent (`///   `) but needed 4-space indent (`///     `) to be recognized as continuation of the bullet list items at `///   -`. The fix is purely cosmetic and preserves the existing prose.

### Testing Performed

- `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` — Passed (exit 0)
- `cargo test -p fdemon-daemon` — Passed (740 tests, 3 ignored, 0 failed)
- `cargo fmt --all` — Applied (no formatting issues)

### Risks/Limitations

1. **Test count**: Task stated 527 unit tests but actual count is 740 — the test suite has grown since the task was written. All tests pass, no regressions.
