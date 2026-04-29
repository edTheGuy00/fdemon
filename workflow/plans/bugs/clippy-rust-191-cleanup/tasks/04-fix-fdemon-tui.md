## Task: Clean up clippy warnings in `fdemon-tui`

**Objective**: Resolve all clippy warnings in `fdemon-tui` so `cargo clippy -p fdemon-tui --all-targets -- -D warnings` exits 0.

**Depends on**: None

**Estimated Time**: 1.5–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/test_utils.rs` — strip `#[test]` from doc-comment example near line 12 (`clippy::test_attr_in_doctest`).
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — convert ~26 `let mut x = T::default(); x.field = …;` patterns to struct literals (`clippy::field_reassign_with_default`).
- `crates/fdemon-tui/src/widgets/devtools/network/tests.rs` — add `#[allow(clippy::manual_is_multiple_of)]` on the test function/module containing the `if i % 2 == 0` check at line 14 (MSRV-1.77.2 cannot use `is_multiple_of`).
- `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` — fix 4 occurrences (mix of `field_reassign_with_default` and `assert_eq!(…, true)` style).
- `crates/fdemon-tui/src/widgets/header.rs` — fix one `clippy::identity_op` (`+ 0` style) by removing the no-op arithmetic.
- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` — fix 2 `field_reassign_with_default` cases.
- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` — fix the heaviest cluster (~50 `field_reassign_with_default` warnings); use struct-literal initialization throughout.
- `crates/fdemon-tui/src/widgets/search_input.rs` — fix `clippy::len_zero` (`.len() == 0` → `.is_empty()`) and one related case (2 warnings total).
- `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` — fix 2 warnings (`assert_eq!(x, false)` → `assert!(!x)` style).

**Files Read (Dependencies):**
- None.

### Warning Inventory (57 total)

| Lint | Count | Primary Files |
|------|-------|---------------|
| `clippy::field_reassign_with_default` | 50 | `target_selector.rs` (bulk), `devtools/mod.rs`, `performance/tests.rs`, `new_session_dialog/launch_context.rs` |
| `clippy::bool_assert_comparison` | 2 | `settings_panel/tests.rs` |
| `clippy::test_attr_in_doctest` | 1 | `test_utils.rs:12` |
| `clippy::manual_is_multiple_of` | 1 | `devtools/network/tests.rs:14` (**MSRV-incompatible — use `#[allow]`**) |
| `clippy::identity_op` | 1 | `header.rs` |
| `clippy::len_zero` | 1 | `search_input.rs` |
| `clippy::bool_comparison` | 1 | `search_input.rs` (`x == true` style) |

### Procedure

1. From the repo root, apply mechanical fixes:
   ```bash
   cargo clippy --fix -p fdemon-tui --all-targets --allow-dirty
   ```
   `--fix` handles most `field_reassign_with_default`, `bool_assert_comparison`, `len_zero`, `bool_comparison`, and `identity_op` cases automatically (clippy reports 6 auto-suggestions; the rest require multi-line edits which `--fix` may decline).
2. Hand-fix what remains. Walk the clippy output crate-by-crate and resolve each surviving warning:
   - **`target_selector.rs` bulk** — clippy may not auto-fix every site if the assignments are conditional. Convert remaining sites to struct literals manually:
     ```rust
     // Before
     let mut state = TargetSelectorState::default();
     state.refreshing = true;
     state.cursor = 3;

     // After
     let state = TargetSelectorState {
         refreshing: true,
         cursor: 3,
         ..Default::default()
     };
     ```
   - **`test_attr_in_doctest`** in `test_utils.rs:12` — the doc comment contains `//! #[test]` followed by a fn definition. Remove the `#[test]` line entirely so the example renders without a misleading test attribute. Verify rustdoc renders sensibly.
   - **`manual_is_multiple_of`** in `devtools/network/tests.rs:14` — **do NOT apply the suggested fix.** The `is_multiple_of` method on integers requires Rust 1.87 but the workspace MSRV is 1.77.2. Instead, add an `#[allow(clippy::manual_is_multiple_of)]` attribute on the smallest enclosing scope (function or `mod tests`). Document the MSRV reason in a one-line comment if the surrounding code is not self-explanatory.
3. Run the per-crate gate:
   ```bash
   cargo clippy -p fdemon-tui --all-targets -- -D warnings
   ```
4. Run unit tests:
   ```bash
   cargo test -p fdemon-tui
   ```
   Existing 814 unit tests must continue to pass.
5. Format:
   ```bash
   cargo fmt --all
   ```

### Acceptance Criteria

1. `cargo clippy -p fdemon-tui --all-targets -- -D warnings` exits 0.
2. `cargo test -p fdemon-tui` passes (no regressions; 814 unit tests).
3. Diff is limited to files under `crates/fdemon-tui/src/`.
4. **No `is_multiple_of` call introduced anywhere** (verify with `git grep is_multiple_of crates/fdemon-tui` — should produce only the `#[allow(clippy::manual_is_multiple_of)]` line if any).
5. Doc-comment fix in `test_utils.rs` preserves the example's intent (only the `#[test]` attribute line is removed).

### Notes

- The `field_reassign_with_default` cluster in `target_selector.rs` is the largest single-file workload. It's mechanical but voluminous — review the diff carefully to make sure no conditional assignments were collapsed into the struct literal incorrectly. Conditional cases must remain as separate assignments after the literal:
  ```rust
  let mut state = TargetSelectorState { cursor: 0, ..Default::default() };
  if condition { state.refreshing = true; }
  ```
- For `field_reassign_with_default`, clippy only fires when **all** assignments occur on consecutive lines after a `Default::default()`. Conditional or interleaved assignments don't trigger the lint and should not be merged.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-af58bdb4086fa7f7e

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/test_utils.rs` | Removed `#[test]` attribute from doc-comment example (`clippy::test_attr_in_doctest`) |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Converted 13 `field_reassign_with_default` patterns to struct literals |
| `crates/fdemon-tui/src/widgets/devtools/network/tests.rs` | Reverted auto-fix `is_multiple_of(2)` (MSRV incompatible); added `#[allow(clippy::manual_is_multiple_of)]` instead |
| `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` | Converted 2 `field_reassign_with_default` patterns to struct literals |
| `crates/fdemon-tui/src/widgets/header.rs` | Auto-fixed `clippy::len_zero` (`.len() == 0` → `.is_empty()`) and `clippy::identity_op` |
| `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | Auto-fixed `clippy::bool_comparison` and `clippy::identity_op` |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | Converted ~35 `field_reassign_with_default` patterns to struct literals (largest cluster) |
| `crates/fdemon-tui/src/widgets/search_input.rs` | Converted 1 `field_reassign_with_default` pattern to struct literal |
| `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | Auto-fixed 2 `clippy::bool_assert_comparison` (`assert_eq!(x, false)` → `assert!(!x)`) |

### Notable Decisions/Tradeoffs

1. **MSRV guard for `manual_is_multiple_of`**: The `cargo clippy --fix` auto-apply introduced `i.is_multiple_of(2)` in `network/tests.rs`, which requires Rust 1.87. This was reverted to `i % 2 == 0` with an `#[allow(clippy::manual_is_multiple_of)]` attribute, per task instructions. Pre-existing uses of `is_multiple_of` in `bars.rs` and `memory_chart/chart.rs` were already present at HEAD and not changed.
2. **Conditional assignments preserved**: In tests that set additional fields after method calls (e.g., `state.refreshing = true` after `state.set_connected_devices(...)`), only the consecutive assignments immediately after `default()` were moved into the struct literal. Post-method assignments remain separate to preserve semantics.
3. **4 pre-existing snapshot test failures**: `render::tests::snapshot_normal_mode_*` tests were already failing before this task (verified by stash test). These failures are not caused by our changes.

### Testing Performed

- `cargo clippy -p fdemon-tui --all-targets -- -D warnings` - Passed (0 warnings)
- `cargo test -p fdemon-tui` - 875 passed, 4 pre-existing snapshot failures (unrelated)
- `cargo fmt --all` - Passed

### Risks/Limitations

1. **Pre-existing snapshot failures**: 4 snapshot tests (`render::tests::snapshot_normal_mode_*`) were already failing before this task. They are not regressions introduced here.
