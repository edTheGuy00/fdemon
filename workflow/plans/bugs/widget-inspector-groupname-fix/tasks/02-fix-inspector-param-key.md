## Task: Fix parameter key in `get_root_widget_tree` (inspector.rs)

**Objective**: Fix the `get_root_widget_tree` function in the daemon crate's inspector module to use `groupName` instead of `objectGroup` for consistency, even though this code path is currently unused by the app layer.

**Depends on**: None

**Estimated Time**: 10 minutes

### Scope

- `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs`: Fix `get_root_widget_tree` function (line ~178)

### Details

The `get_root_widget_tree` function (line 171) is the lower-level wrapper in the daemon crate. The app layer (`actions.rs`) calls the VM Service directly rather than through this wrapper, but both have the same bug. This task fixes the daemon-side copy for consistency and correctness.

**Change 1 — Parameter key (line 178):**

```rust
// Before:
newer_args.insert("objectGroup".to_string(), object_group.to_string());

// After:
newer_args.insert("groupName".to_string(), object_group.to_string());
```

**Change 2 — `withPreviews` value (line 180):**

```rust
// Before:
newer_args.insert("withPreviews".to_string(), "false".to_string());

// After:
newer_args.insert("withPreviews".to_string(), "true".to_string());
```

**Do NOT change:**
- The `older_args` fallback block (line 198) — `getRootWidgetSummaryTree` correctly uses `objectGroup`
- `get_details_subtree` (line 240) — uses `objectGroup` correctly (that extension uses `_registerObjectGroupServiceExtension`)
- `get_selected_widget` (line 267) — uses `objectGroup` correctly (same registration style)
- `ObjectGroupManager::dispose_group` (line 107) — uses `objectGroup` correctly (`disposeGroup` uses `_registerObjectGroupServiceExtension`)

### Acceptance Criteria

1. `get_root_widget_tree` params contain `groupName` (not `objectGroup`) for the newer API
2. `get_root_widget_tree` params contain `withPreviews: "true"`
3. Legacy `getRootWidgetSummaryTree` fallback still uses `objectGroup`
4. `get_details_subtree`, `get_selected_widget`, `dispose_group` unchanged
5. `cargo check -p fdemon-daemon` compiles
6. `cargo test -p fdemon-daemon` passes
7. `cargo clippy -p fdemon-daemon` no warnings

### Testing

No new tests needed — the function requires a live WebSocket connection to test. The existing unit tests for `ObjectGroupManager` and `parse_diagnostics_node_response` are unaffected.

### Notes

- This function is currently unused by the app layer (app uses `VmRequestHandle::call_extension` directly in `actions.rs`), but it's the public API for the daemon crate and could be used by future consumers
- The `WidgetInspector::fetch_tree` high-level API (line 337) delegates to this function, so fixing it here ensures any caller of the public API gets correct behavior
- The `ObjectGroupManager` doc comment (line 46) already shows the correct `groupName` usage in its example — the code just wasn't consistent with the docs

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` | Line 178: `"objectGroup"` → `"groupName"` in `newer_args`; Line 180: `"withPreviews"` value `"false"` → `"true"` in `newer_args` |

### Notable Decisions/Tradeoffs

1. **Minimal scope**: Only the two specified lines in `get_root_widget_tree`'s `newer_args` block were changed. The `older_args` fallback block (line 198), `get_details_subtree`, `get_selected_widget`, and `dispose_group` were all left untouched as required.

### Testing Performed

- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon` - Passed (375 passed; 0 failed; 3 ignored)
- `cargo clippy -p fdemon-daemon` - Passed (no warnings)

### Risks/Limitations

1. **Live WebSocket required**: The changed function cannot be exercised in unit tests without a running Flutter process. Correctness is validated by code review against the Flutter 3.22+ `getRootWidgetTree` API contract.
