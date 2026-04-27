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

**Status:** Not Started
**Branch:** _to be filled by implementor_

### Files Modified

| File | Changes |
|------|---------|
| _tbd_ | _tbd_ |

### Notable Decisions/Tradeoffs

_tbd_

### Testing Performed

- `cargo clippy -p fdemon-core --all-targets -- -D warnings` — _tbd_
- `cargo test -p fdemon-core` — _tbd_
- `cargo fmt --all -- --check` — _tbd_

### Risks/Limitations

_tbd_
