# Task 01: Remove `fdemon-managed` Field and Dead Constant

## Objective

Remove the invalid `"fdemon-managed": true` field from generated VS Code/Neovim launch.json entries and the unused `FDEMON_MARKER_FIELD` constant. This eliminates VS Code's "Property fdemon-managed is not allowed" validation warning.

## Context

- `fdemon_entry()` in `vscode.rs` includes `"fdemon-managed": true` in the generated JSON
- The Dart extension validates launch.json against a schema that rejects unknown fields
- The merge logic uses `"name": "Flutter (fdemon)"` as the match key — `fdemon-managed` is never read
- `FDEMON_MARKER_FIELD` in `merge.rs` is `#[allow(dead_code)]` and unreferenced
- Neovim inherits this via delegation to `VSCodeGenerator`

## Acceptance Criteria

- [ ] `fdemon_entry()` no longer includes `"fdemon-managed": true`
- [ ] `FDEMON_MARKER_FIELD` constant removed from `merge.rs`
- [ ] `#[allow(dead_code)]` annotation removed
- [ ] Neovim config no longer contains the field (automatic — delegates to VSCode)
- [ ] Merge-by-name still works correctly (existing tests pass)
- [ ] All tests in `vscode.rs`, `merge.rs`, `neovim.rs` pass
- [ ] Any test assertions checking for `fdemon-managed` updated

## Implementation Steps

1. **`crates/fdemon-app/src/ide_config/vscode.rs`**:
   - Remove `"fdemon-managed": true` from the `json!({})` block in `fdemon_entry()`

2. **`crates/fdemon-app/src/ide_config/merge.rs`**:
   - Remove the `FDEMON_MARKER_FIELD` constant and its `#[allow(dead_code)]` annotation
   - Keep `FDEMON_CONFIG_NAME` — it is actively used

3. **Update tests**:
   - Search for test assertions that check for `"fdemon-managed"` or `FDEMON_MARKER_FIELD`
   - Update or remove those assertions

## Estimated Time

10 minutes

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/vscode.rs` | Removed `"fdemon-managed": true` from `fdemon_entry()`; updated test assertion from `assert_eq!(configs[0]["fdemon-managed"], true)` to `assert!(cfg.get("fdemon-managed").is_none())` |
| `crates/fdemon-app/src/ide_config/merge.rs` | Removed `FDEMON_MARKER_FIELD` constant and its `#[allow(dead_code)]` annotation; updated `test_constants_have_expected_values` to only assert `FDEMON_CONFIG_NAME` |
| `crates/fdemon-app/src/ide_config/neovim.rs` | Updated test assertion from `assert_eq!(cfg["fdemon-managed"], true)` to `assert!(cfg.get("fdemon-managed").is_none())` |

### Notable Decisions/Tradeoffs

1. **Test input data preserved**: Test strings that include `"fdemon-managed": true` in existing-config inputs (merge scenarios) were left as-is — they simulate old user configs being migrated, and the merge replaces the entire entry with a fresh `fdemon_entry()` that no longer contains the field. The assertions only verify `debugServer` and `name`, so they remain correct.
2. **Pre-existing daemon compile error**: `fdemon-daemon` has a pre-existing E0063 error (`missing field ws_uri` in `VmRequestHandle::new_for_test`) that blocks `cargo test` for all dependent crates. This is unrelated to this task. `cargo check -p fdemon-app` passes cleanly.

### Testing Performed

- `cargo check -p fdemon-app` - Passed (no errors or warnings from our changes)
- `cargo test -p fdemon-app -- ide_config` - Blocked by pre-existing `fdemon-daemon` compile error (unrelated to this task)

### Risks/Limitations

1. **Pre-existing daemon error**: The `fdemon-daemon` crate has a compile error in `vm_service/client.rs:177` that prevents running the `fdemon-app` test suite. This pre-dates this task (verified via `git status` — only our 3 files are modified). Full test validation requires that error to be fixed first.
