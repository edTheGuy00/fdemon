## Task: Fix Error Handling in Extension Functions

**Objective**: Fix three error handling issues: (1) silent error swallowing in `get_root_widget_tree` fallback, (2) state loss in `create_group` on dispose failure, (3) silent child skipping in `extract_layout_tree`.

**Depends on**: 01-split-extensions-submodules

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` — Fix `get_root_widget_tree` fallback and `create_group` state loss
- `crates/fdemon-daemon/src/vm_service/extensions/layout.rs` — Fix `extract_layout_tree` silent skip
- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` — May need to update `parse_optional_diagnostics_node_response`

### Details

#### Fix 1: `get_root_widget_tree` — Narrow the Fallback (Review Issue #3)

**Current code** (after split, in `inspector.rs`):
```rust
match result {
    Ok(value) => parse_diagnostics_node_response(&value),
    Err(_) => {
        // Fallback: try the older getRootWidgetSummaryTree API.
        // ...
    }
}
```

**Problem**: `Err(_)` catches ALL errors including transport errors (channel closed, connection lost). These should propagate immediately, not trigger a redundant fallback.

**Fix**: Check whether the error indicates "extension not available" before falling back. The `is_extension_not_available` function already exists in `mod.rs` but operates on `VmServiceError` structs from JSON-RPC responses, not on the `Error` enum. We need to discriminate at the `Error` level.

```rust
match result {
    Ok(value) => parse_diagnostics_node_response(&value),
    Err(e) => {
        // Only fall back if the newer API is not registered on this Flutter version.
        // Transport/channel errors propagate immediately.
        if matches!(&e, Error::Protocol(_)) {
            tracing::debug!(
                "getRootWidgetTree not available, falling back to getRootWidgetSummaryTree: {e}"
            );
            // Fallback: try the older getRootWidgetSummaryTree API.
            let fallback_result = client.call_extension(
                ext::GET_ROOT_WIDGET_SUMMARY_TREE,
                isolate_id,
                Some(args),
            ).await?;
            parse_diagnostics_node_response(&fallback_result)
        } else {
            Err(e)
        }
    }
}
```

**Rationale**: `Error::Protocol` is the variant used for JSON-RPC level errors (method not found, extension not available). Other variants like `Error::ChannelClosed`, `Error::Daemon`, etc. indicate transport-level failures that will not be resolved by retrying a different method.

#### Fix 2: `create_group` — Graceful Dispose Failure (Review Issue #5)

**Current code** (after split, in `inspector.rs`):
```rust
pub async fn create_group(&mut self) -> Result<String> {
    if let Some(old) = self.active_group.take() {
        self.dispose_group(&old).await?;  // <-- on failure, old group name is lost
    }
    // ...
}
```

**Problem**: `take()` removes the old group name from state BEFORE `dispose_group()`. If dispose fails:
- `active_group` is `None` (old name lost)
- No new group was created (early return via `?`)
- The old group is leaked on the Flutter side

**Fix**: Log the dispose failure and proceed with creating the new group. This matches the doc comment ("non-fatal in most cases"):

```rust
pub async fn create_group(
    &mut self,
    client: &VmServiceClient,  // after task 02 refactor
) -> Result<String> {
    if let Some(old) = self.active_group.take() {
        if let Err(e) = self.dispose_group(client, &old).await {
            tracing::warn!(
                "Failed to dispose object group '{}', proceeding with new group: {e}",
                old
            );
            // Old group is leaked on the Flutter side, but new work can continue.
        }
    }
    self.group_counter += 1;
    let name = format!("fdemon-inspector-{}", self.group_counter);
    self.active_group = Some(name.clone());
    Ok(name)
}
```

**Note**: If task 02 (client ownership refactor) is not yet complete, adjust the signature to match the current API. The logic change is the same either way.

#### Fix 3: `extract_layout_tree` — Log Skipped Children (Review Issue #10)

**Current code** (after split, in `layout.rs`):
```rust
if let Some(children) = result_value.get("children").and_then(|c| c.as_array()) {
    for child_json in children {
        if let Ok(child_node) = serde_json::from_value::<DiagnosticsNode>(child_json.clone()) {
            layouts.push(extract_layout_info(&child_node, child_json));
        }
    }
}
```

**Fix**: Add a `tracing::warn!` for skipped children:

```rust
if let Some(children) = result_value.get("children").and_then(|c| c.as_array()) {
    for (i, child_json) in children.iter().enumerate() {
        match serde_json::from_value::<DiagnosticsNode>(child_json.clone()) {
            Ok(child_node) => {
                layouts.push(extract_layout_info(&child_node, child_json));
            }
            Err(e) => {
                tracing::warn!(
                    "Skipping layout child {}: failed to parse DiagnosticsNode: {e}",
                    i
                );
            }
        }
    }
}
```

#### Fix 4: `parse_optional_diagnostics_node_response` — Use `node_value` Consistently (Review Issue #7)

**Current code** (in `mod.rs`):
```rust
pub fn parse_optional_diagnostics_node_response(value: &Value) -> Result<Option<DiagnosticsNode>> {
    let node_value = value.get("result").unwrap_or(value);
    if node_value.is_null() {
        return Ok(None);
    }
    parse_diagnostics_node_response(value).map(Some)  // <-- passes `value`, not `node_value`
}
```

**Fix**: Use `node_value` directly instead of delegating:

```rust
pub fn parse_optional_diagnostics_node_response(value: &Value) -> Result<Option<DiagnosticsNode>> {
    let node_value = value.get("result").unwrap_or(value);
    if node_value.is_null() {
        return Ok(None);
    }
    serde_json::from_value(node_value.clone())
        .map(Some)
        .map_err(|e| Error::protocol(format!("failed to parse DiagnosticsNode: {e}")))
}
```

### Acceptance Criteria

1. `get_root_widget_tree` only falls back on `Error::Protocol` errors; transport errors propagate immediately
2. The discarded error in the fallback path is logged at `debug` level
3. `create_group` succeeds even when old group dispose fails; a warning is logged
4. `extract_layout_tree` logs a `warn!` when a child fails deserialization
5. `parse_optional_diagnostics_node_response` uses `node_value` consistently (no redundant double-extraction)
6. All existing tests pass
7. New tests cover the error-path changes

### Testing

**New tests needed:**

```rust
// In inspector.rs tests:

#[test]
fn test_create_group_proceeds_after_dispose_failure() {
    // Verify that create_group increments counter and sets active_group
    // even if dispose_group would fail.
    // (Unit test: verify state transitions without async client)
}

// In layout.rs tests:

#[test]
fn test_extract_layout_tree_logs_warning_on_invalid_child() {
    // Create a JSON with a valid root but an invalid child (missing "description")
    // Verify the function returns layouts for the valid nodes only
    // (The tracing::warn is a side effect; test the return value behavior)
}
```

Verification:
```bash
cargo fmt --all && cargo check --workspace && cargo test --lib && cargo clippy --workspace -- -D warnings
```

### Notes

- Fix 1 and Fix 2 are in `inspector.rs`; Fix 3 is in `layout.rs`; Fix 4 is in `mod.rs`. After the task-01 split, these are separate files with no contention.
- If task 02 (client ownership refactor) runs concurrently, the `create_group` signature may differ. Coordinate with task 02 on the final `create_group` method signature.
- The `matches!(&e, Error::Protocol(_))` check in Fix 1 may need adjustment based on the actual `Error` enum variants. Check `fdemon-core/src/error.rs` for the exact variant names.
