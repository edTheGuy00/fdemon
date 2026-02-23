## Task: Verify `disposeGroup` and other extensions still use `objectGroup`

**Objective**: Audit all inspector extension call sites to confirm that only `getRootWidgetTree` was changed to `groupName`, and all other extensions (`disposeGroup`, `getDetailsSubtree`, `getSelectedWidget`, `getRootWidgetSummaryTree`) still correctly use `objectGroup`. Run the full workspace verification suite.

**Depends on**: 01-fix-actions-param-key, 02-fix-inspector-param-key

**Estimated Time**: 15 minutes

### Scope

Audit (read-only) + full workspace verification:

- `crates/fdemon-app/src/actions.rs` — verify all `objectGroup` usages in non-`getRootWidgetTree` calls
- `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` — verify all `objectGroup` usages in non-`getRootWidgetTree` functions
- `crates/fdemon-daemon/src/vm_service/extensions/layout.rs` — verify `groupName` usage is correct (already was)

### Details

**Expected parameter key usage after the fix:**

| Call Site | Extension | Expected Key |
|-----------|-----------|--------------|
| `actions.rs` `try_fetch_widget_tree` newer API | `getRootWidgetTree` | `groupName` |
| `actions.rs` `try_fetch_widget_tree` older API | `getRootWidgetSummaryTree` | `objectGroup` |
| `actions.rs` `spawn_fetch_widget_tree` dispose | `disposeGroup` | `objectGroup` |
| `actions.rs` `spawn_fetch_layout_data` dispose | `disposeGroup` | `objectGroup` |
| `actions.rs` `spawn_fetch_layout_data` fetch | `getLayoutExplorerNode` | `groupName` |
| `actions.rs` `spawn_dispose_devtools_groups` | `disposeGroup` | `objectGroup` |
| `inspector.rs` `get_root_widget_tree` newer API | `getRootWidgetTree` | `groupName` |
| `inspector.rs` `get_root_widget_tree` older API | `getRootWidgetSummaryTree` | `objectGroup` |
| `inspector.rs` `get_details_subtree` | `getDetailsSubtree` | `objectGroup` |
| `inspector.rs` `get_selected_widget` | `getSelectedWidget` | `objectGroup` |
| `inspector.rs` `ObjectGroupManager::dispose_group` | `disposeGroup` | `objectGroup` |
| `layout.rs` `get_layout_node` | `getLayoutExplorerNode` | `groupName` |
| `layout.rs` `fetch_layout_data` | `getLayoutExplorerNode` | `groupName` |

**Verification steps:**

1. Search for all `"objectGroup"` and `"groupName"` string literals in `crates/` — confirm each usage matches the table above
2. Run `cargo fmt --all`
3. Run `cargo check --workspace`
4. Run `cargo test --workspace`
5. Run `cargo clippy --workspace`

### Acceptance Criteria

1. All `objectGroup` / `groupName` usages match the expected table
2. `cargo fmt --all` — no changes needed
3. `cargo check --workspace` — passes
4. `cargo test --workspace` — all tests pass
5. `cargo clippy --workspace` — no new warnings

### Testing

This task is primarily an audit and verification step. No new code or tests to write.

### Notes

- The key distinction: `getRootWidgetTree` (Flutter 3.22+) uses raw `registerServiceExtension` which expects `groupName`. All other inspector extensions use `_registerObjectGroupServiceExtension` helper which wraps the callback and maps `objectGroup`.
- `getLayoutExplorerNode` also uses `groupName` because it was registered via `registerServiceExtension` (same pattern as `getRootWidgetTree`)
- If any additional call sites are found that should use `groupName`, flag them and create follow-up tasks

---

## Completion Summary

**Status:** Not started
