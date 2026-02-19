## Task: Refactor VmServiceClient Ownership in ObjectGroupManager/WidgetInspector

**Objective**: Remove the owned `VmServiceClient` from `ObjectGroupManager` and thread `&VmServiceClient` through method parameters instead, eliminating the dual-client pattern and uncompilable doc examples.

**Depends on**: 01-split-extensions-submodules

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` — Primary refactor target
- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` — Update re-exports if signatures change

### Details

#### Problem

`ObjectGroupManager` takes ownership of a `VmServiceClient` (which is NOT `Clone` — it contains an `mpsc::Receiver`). Meanwhile, `WidgetInspector` methods also accept `&VmServiceClient` as a parameter, creating a dual-client pattern where:
- Group lifecycle (dispose) uses the **owned** client
- Data fetching uses the **parameter** client
- `dispose_all` takes a `_client: &VmServiceClient` parameter that is **completely unused**
- Doc comments say "cloned" but `VmServiceClient` has no `Clone` impl

Since `call_extension` takes `&self`, no ownership is needed — borrowing is sufficient.

#### Refactored API

**ObjectGroupManager** — remove `client` field, add `client` parameter to methods that make RPCs:

```rust
pub struct ObjectGroupManager {
    // client field REMOVED
    isolate_id: String,
    active_group: Option<String>,
    group_counter: u32,
}

impl ObjectGroupManager {
    pub fn new(isolate_id: String) -> Self {
        Self {
            isolate_id,
            active_group: None,
            group_counter: 0,
        }
    }

    pub async fn create_group(
        &mut self,
        client: &super::super::client::VmServiceClient,
    ) -> Result<String> {
        if let Some(old) = self.active_group.take() {
            // Note: Task 03 will improve the error handling here.
            // For now, keep the existing behavior (propagate via ?).
            self.dispose_group(client, &old).await?;
        }
        self.group_counter += 1;
        let name = format!("fdemon-inspector-{}", self.group_counter);
        self.active_group = Some(name.clone());
        Ok(name)
    }

    pub async fn dispose_group(
        &self,
        client: &super::super::client::VmServiceClient,
        group_name: &str,
    ) -> Result<()> {
        let mut args = HashMap::new();
        args.insert("objectGroup".to_string(), group_name.to_string());
        match client.call_extension(ext::DISPOSE_GROUP, &self.isolate_id, Some(args)).await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::debug!("ObjectGroupManager: failed to dispose group '{}': {}", group_name, e);
                Err(e)
            }
        }
    }

    pub async fn dispose_all(
        &mut self,
        client: &super::super::client::VmServiceClient,
    ) -> Result<()> {
        if let Some(group) = self.active_group.take() {
            self.dispose_group(client, &group).await?;
        }
        Ok(())
    }

    // active_group() and group_counter() unchanged (no client needed)
}
```

**WidgetInspector** — remove client from constructor, use the parameter client consistently:

```rust
pub struct WidgetInspector {
    object_group: ObjectGroupManager,
    isolate_id: String,
}

impl WidgetInspector {
    pub fn new(isolate_id: String) -> Self {
        let object_group = ObjectGroupManager::new(isolate_id.clone());
        Self { object_group, isolate_id }
    }

    pub async fn fetch_tree(
        &mut self,
        client: &super::super::client::VmServiceClient,
    ) -> Result<DiagnosticsNode> {
        let group = self.object_group.create_group(client).await?;
        get_root_widget_tree(client, &self.isolate_id, &group).await
    }

    // fetch_details, fetch_selected — unchanged (already take &client)

    pub async fn dispose(
        &mut self,
        client: &super::super::client::VmServiceClient,
    ) -> Result<()> {
        self.object_group.dispose_all(client).await
    }
}
```

#### Doc Comment Fixes

- Remove all references to "cloned" in `ObjectGroupManager::new` and `WidgetInspector::new` doc comments
- Update the doc example on `ObjectGroupManager::new` (line ~184) to show the new API without `.clone()`
- Update `WidgetInspector` doc comments to note that client is borrowed, not owned

### Acceptance Criteria

1. `ObjectGroupManager` has no `client` field
2. `ObjectGroupManager::new` takes only `isolate_id: String` (no `VmServiceClient` param)
3. `create_group`, `dispose_group`, and `dispose_all` all accept `client: &VmServiceClient` as a parameter
4. `dispose_all` has NO unused `_client` parameter — the `client` is actively used
5. `WidgetInspector::new` takes only `isolate_id: String` (no `VmServiceClient` param)
6. All `WidgetInspector` methods use the same `client` parameter — no dual-client pattern
7. Doc comments are accurate (no "cloned" references, doc examples compile conceptually)
8. All existing tests pass
9. `cargo fmt --all` clean
10. `cargo clippy --workspace -- -D warnings` clean

### Testing

No new tests needed for the refactor itself (same behavior, different API). However:

- Verify all existing `ObjectGroupManager` tests still pass
- Verify all existing `WidgetInspector` tests still pass
- If there are tests that construct `ObjectGroupManager::new(client, isolate_id)`, update them to `ObjectGroupManager::new(isolate_id)` and pass `&client` to each method call

```bash
cargo fmt --all && cargo check --workspace && cargo test --lib && cargo clippy --workspace -- -D warnings
```

### Notes

- **Zero external callers exist** — both `ObjectGroupManager::new` and `WidgetInspector::new` have no call sites outside `extensions.rs`. The refactor is isolated.
- The `VmServiceClient` path will depend on the split done in task 01. Use whatever path convention task 01 establishes (likely a type alias or re-export in `extensions/mod.rs`).
- This task resolves Review Issue #1 (Critical: VmServiceClient ownership) and Issue #4 (Major: unused `_client` parameter).

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` | Removed `client` field from `ObjectGroupManager`; changed `new` to take only `isolate_id: String`; added `client: &VmServiceClient` parameter to `create_group`, `dispose_group`, and `dispose_all`; changed `WidgetInspector::new` to take only `isolate_id: String`; updated `fetch_tree` to pass `client` to `create_group`; fixed `dispose_all` to actively use its `client` parameter (was `_client`); updated all doc comments to remove "cloned" references |

### Notable Decisions/Tradeoffs

1. **No changes to `extensions/mod.rs`**: The re-export `pub(super) use super::client::VmServiceClient` established by task 01 is unchanged. The `VmServiceClient` type alias is already available in `inspector.rs` via `use super::VmServiceClient`, so no additional re-export changes were needed.
2. **`dispose_group` signature order**: The `client` parameter is placed before `group_name` to match the task spec and keep consistency with `create_group` and `dispose_all`.

### Testing Performed

- `cargo fmt --all` - Passed (no changes needed)
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-daemon --lib` - Passed (305 tests, 0 failures)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None**: Zero external callers of `ObjectGroupManager::new` or `WidgetInspector::new` exist outside `inspector.rs`. The refactor is fully isolated to that file.
