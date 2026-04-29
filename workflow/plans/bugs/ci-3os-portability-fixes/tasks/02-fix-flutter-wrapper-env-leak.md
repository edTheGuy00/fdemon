## Task: Scrub `FLUTTER_ROOT` in `test_flutter_wrapper_detection`

**Objective**: Add `#[serial]` and `std::env::remove_var("FLUTTER_ROOT")` to `test_flutter_wrapper_detection` in `locator.rs`, mirroring the pattern used by sibling tests, so the wrapper-detection strategy is reachable on CI runners that have `FLUTTER_ROOT` pre-set in their environment.

**Depends on**: None

**Estimated Time**: 0.25 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs`: Edit `test_flutter_wrapper_detection` around lines 652–666.

**Files Read (Dependencies):**
- Sibling tests in the same file that already use `#[serial]` + `std::env::remove_var("FLUTTER_ROOT")` (e.g., `test_fvm_modern_detection`, `test_flutter_root_env_beats_version_managers`). Use them as the pattern reference.

### Details

`find_flutter_sdk` walks 12 strategies in order:

- Strategy 2: reads `std::env::var_os("FLUTTER_ROOT")` and short-circuits if it points at a valid SDK.
- Strategy 9: detects a project-local `flutterw` script (the wrapper) and resolves the SDK from `<project>/.flutter/`.

GitHub's macOS runner pre-installs Flutter and sets `FLUTTER_ROOT` in the environment. The macOS CI failure on PR #38 is `assertion left == right failed; left: SdkSource::EnvironmentVariable, right: SdkSource::FlutterWrapper` — Strategy 2 wins before Strategy 9 is reached.

The fix is to scrub `FLUTTER_ROOT` (and any other Flutter env vars Strategy 2 considers — verify by reading `find_flutter_sdk` for the full list) at the start of the test, and to mark the test `#[serial]` so concurrent env-touching tests do not race.

Apply the same pattern that the codebase already uses elsewhere in the same file. Example shape (verify against actual sibling tests before committing):

```rust
#[test]
#[serial]
fn test_flutter_wrapper_detection() {
    std::env::remove_var("FLUTTER_ROOT");
    // ...rest of existing test body unchanged...
}
```

If sibling tests scrub additional env vars (e.g., `FVM_HOME`, `ASDF_DATA_DIR`, `PATH`), include them too — match the pattern exactly.

### Acceptance Criteria

1. `test_flutter_wrapper_detection` carries `#[serial]` (in addition to `#[test]`) and removes `FLUTTER_ROOT` (and any other env vars the sibling tests scrub) at the start of its body.
2. The test still asserts `result.source == SdkSource::FlutterWrapper`.
3. `cargo test -p fdemon-daemon flutter_sdk::locator::tests::test_flutter_wrapper_detection` passes on macOS even when `FLUTTER_ROOT` is set in the environment (simulate locally with `FLUTTER_ROOT=/some/valid/sdk cargo test ...`).
4. `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` exits 0.
5. `cargo test -p fdemon-daemon` passes.
6. `cargo fmt --all -- --check` is clean.
7. No other tests in `locator.rs` are modified.
8. The `serial_test` crate is already a dev-dependency (verify; it should be — sibling tests use `#[serial]`). Do not add or change Cargo dependencies.

### Testing

Reproduce the original failure locally before applying the fix:

```bash
FLUTTER_ROOT=/path/to/any/valid/sdk cargo test -p fdemon-daemon flutter_sdk::locator::tests::test_flutter_wrapper_detection
```

This should fail with `EnvironmentVariable != FlutterWrapper` before the fix and pass after.

After the fix, run the full module tests to confirm no regression:

```bash
cargo test -p fdemon-daemon flutter_sdk::locator
```

### Notes

- `std::env::remove_var` was marked unsafe-without-attribute in newer Rust editions, but the project's MSRV is 1.77.2 and the existing sibling tests use the safe form. Match what's already in the file. If Rust's safety rules force a change, the `temp_env` crate is the typical alternative — but only if the existing pattern no longer compiles.
- `#[serial]` is from the `serial_test` crate. The annotation prevents two env-touching tests from running concurrently and clobbering each other's state.

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/flutter_sdk/locator.rs` | Added `#[serial]` attribute and `std::env::remove_var("FLUTTER_ROOT")` call to `test_flutter_wrapper_detection` |

### Notable Decisions/Tradeoffs

1. **Only `FLUTTER_ROOT` scrubbed**: Strategy 2 is the only strategy that reads `FLUTTER_ROOT`. Strategies 3-8 (FVM, Puro, asdf, mise, proto) depend on project-local config files that the test does not create, so they pass through without interference. No other env vars needed scrubbing.
2. **`remove_var` placed after file setup but before `find_flutter_sdk` call**: Mirrors the exact pattern of sibling tests (`test_fvm_modern_detection`, `test_flutter_root_env_beats_version_managers`), where the env scrub happens just before the function under test is called.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` — Passed
- `cargo test -p fdemon-daemon flutter_sdk::locator::tests::test_flutter_wrapper_detection` — Passed (1 test)
- `cargo test -p fdemon-daemon` — Passed (740 tests, 3 ignored)

### Risks/Limitations

1. **Concurrent test isolation**: The `#[serial]` attribute ensures this test does not race with other env-touching tests. Without it, a concurrent test could set `FLUTTER_ROOT` between the `remove_var` call and the `find_flutter_sdk` call.
