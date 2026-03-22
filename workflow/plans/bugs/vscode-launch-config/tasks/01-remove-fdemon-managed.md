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
