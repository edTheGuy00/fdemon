## Task: Clean up clippy warnings in `flutter-demon` integration tests

**Objective**: Resolve all clippy warnings in the workspace-root integration tests so `cargo clippy --test sdk_detection -- -D warnings` exits 0 and contributes zero warnings to the workspace lint.

**Depends on**: None

**Estimated Time**: 0.25 hours

### Scope

**Files Modified (Write):**
- `tests/sdk_detection/docker_helpers.rs` — remove unneeded `mut` on two `let` bindings (~lines 135, 327).
- `tests/sdk_detection/tier1_detection_chain.rs` — convert one `match` with a single arm to `if let` (~line 990).
- `tests/sdk_detection/tier2_headless.rs` — replace `.map_or(false, …)` with `.is_some_and(…)` (~line 266).

**Files Read (Dependencies):**
- None.

### Warning Inventory (4 total)

| Lint | Count | Location |
|------|-------|----------|
| `variable does not need to be mutable` | 2 | `docker_helpers.rs:135, 327` |
| `clippy::single_match` | 1 | `tier1_detection_chain.rs:990` |
| `clippy::unnecessary_map_or` | 1 | `tier2_headless.rs:266` |

### Procedure

1. From the repo root, apply mechanical fixes for the integration-test target:
   ```bash
   cargo clippy --fix --test sdk_detection --allow-dirty
   ```
   Clippy reports 3 of 4 warnings as auto-fixable.
2. Hand-fix the remaining `clippy::single_match` if `--fix` skipped it. Pattern:
   ```rust
   // Before
   match value {
       Some(x) => do_thing(x),
       _ => {}
   }

   // After
   if let Some(x) = value {
       do_thing(x);
   }
   ```
3. Verify the integration-test target compiles cleanly (the warnings live in this binary's test target, not in any library crate):
   ```bash
   cargo clippy --test sdk_detection -- -D warnings
   ```
   Also confirm the broader `--all-targets` view stays clean for this crate:
   ```bash
   cargo clippy -p flutter-demon --all-targets -- -D warnings
   ```
4. Run the integration tests (most are `#[ignore]`'d; the non-ignored subset must still pass):
   ```bash
   cargo test --test sdk_detection
   ```
5. Format:
   ```bash
   cargo fmt --all
   ```

### Acceptance Criteria

1. `cargo clippy --test sdk_detection -- -D warnings` exits 0.
2. `cargo clippy -p flutter-demon --all-targets -- -D warnings` exits 0.
3. `cargo test --test sdk_detection` passes its non-ignored cases.
4. Diff is limited to the three files under `tests/sdk_detection/`.

### Notes

- `is_some_and` is stabilized in Rust 1.70 — well within MSRV 1.77.2.
- These tests are part of the binary crate's integration test suite; they don't ship in any library crate.

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

- `cargo clippy --test sdk_detection -- -D warnings` — _tbd_
- `cargo test --test sdk_detection` — _tbd_
- `cargo fmt --all -- --check` — _tbd_

### Risks/Limitations

_tbd_
