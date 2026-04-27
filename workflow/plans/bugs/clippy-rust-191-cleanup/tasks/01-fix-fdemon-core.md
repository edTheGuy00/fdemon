## Task: Clean up clippy warnings in `fdemon-core`

**Objective**: Resolve all clippy warnings in `fdemon-core` so `cargo clippy -p fdemon-core --all-targets -- -D warnings` exits 0.

**Depends on**: None

**Estimated Time**: 0.25 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/ansi.rs`: Replace two `vec![…]` array literals with plain array `[…]` (test scope, ~lines 461 and 470).

**Files Read (Dependencies):**
- None.

### Warning Inventory (2 total)

| Lint | Location |
|------|----------|
| `clippy::useless_vec` | `src/ansi.rs:461` |
| `clippy::useless_vec` | `src/ansi.rs:470` |

Both occurrences are inside test code where a `Vec<&str>` literal is constructed, iterated, and dropped. A `[&str; N]` array works identically.

### Procedure

1. From the repo root, apply mechanical fixes:
   ```bash
   cargo clippy --fix -p fdemon-core --all-targets --allow-dirty
   ```
2. Inspect the diff (`git diff crates/fdemon-core/`) — only `ansi.rs` should change.
3. Run the per-crate gate:
   ```bash
   cargo clippy -p fdemon-core --all-targets -- -D warnings
   ```
   Must exit 0 with no warnings.
4. Run unit tests:
   ```bash
   cargo test -p fdemon-core
   ```
5. Format:
   ```bash
   cargo fmt --all
   ```

### Acceptance Criteria

1. `cargo clippy -p fdemon-core --all-targets -- -D warnings` exits 0.
2. `cargo test -p fdemon-core` passes (no regressions; previously-counted 357 unit tests still pass).
3. Diff is limited to `crates/fdemon-core/src/ansi.rs`.
4. No behavior change — only `vec![…]` → `[…]` in two fixture/test arrays.

### Notes

- This task is the smallest in the wave; useful as a smoke-test for the per-crate fix recipe before tackling larger crates.
- No MSRV concerns; both fixes are pure syntax changes.

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/ansi.rs` | Replaced two `vec![...]` array literals with plain `[...]` arrays in test fixtures (lines 461 and 470 of `test_strip_flutter_machine_mode_full_block`). |

### Notable Decisions/Tradeoffs

1. **Reverted unrelated `cargo fmt` change**: `cargo fmt --all` also reformatted a line in `crates/fdemon-daemon/src/flutter_sdk/locator.rs`. That change is outside this task's scope (no clippy warning there), so it was reverted to keep the diff limited to `ansi.rs` as required by acceptance criteria.

### Testing Performed

- `cargo clippy -p fdemon-core --all-targets -- -D warnings` — Passed (0 warnings, exit 0)
- `cargo test -p fdemon-core` — Passed (372 unit tests + 5 doc tests, 0 failures)
- `cargo fmt --all` — Passed (no formatting changes needed in `ansi.rs`)

### Risks/Limitations

1. **None**: Pure syntax change (`vec![...]` → `[...]`) with no behavior difference. Both types implement `IntoIterator` and `iter()` identically in this context.
