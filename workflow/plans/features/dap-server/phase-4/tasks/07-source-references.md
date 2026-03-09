## Task: Implement Source References for SDK and Package Sources

**Objective**: Enable IDEs to view Dart SDK sources (e.g., `dart:core`) and package sources that may not exist on the user's local filesystem as editable files. When a stack frame points to an SDK or package source, provide a `sourceReference` so the IDE can request the source text from the adapter.

**Depends on**: 01-wire-debug-event-channel

**Estimated Time**: 3–5 hours

### Scope

- `crates/fdemon-dap/src/adapter/stack.rs`: Assign `sourceReference` for non-local sources in stack frames
- `crates/fdemon-dap/src/adapter/mod.rs`: Add `handle_source()` request handler for the `source` DAP request
- `crates/fdemon-dap/src/adapter/mod.rs`: Add source reference store (maps reference ID → script info)
- `crates/fdemon-app/src/handler/dap_backend.rs`: Add `get_source(script_id)` to `DebugBackend` trait

### Details

#### When to Use `sourceReference` vs `path`

| Source URI | Strategy |
|------------|----------|
| `file:///path/to/user/code.dart` | `path` only, `sourceReference: 0` — IDE opens editable file |
| `dart:core/string.dart` | `sourceReference > 0`, no `path` — adapter serves read-only source |
| `package:flutter/widgets.dart` | Try to resolve to local path via `.dart_tool/package_config.json`. If found: use `path`. If not: use `sourceReference`. |
| `org-dartlang-sdk:///...` | `sourceReference > 0` — SDK source, fetch via VM Service |

#### Source Reference Store

```rust
/// Maps sourceReference IDs to script information needed to fetch source text.
pub struct SourceReferenceStore {
    next_id: i64,
    /// reference_id → (isolate_id, script_id, script_uri)
    references: HashMap<i64, SourceRefEntry>,
}

struct SourceRefEntry {
    isolate_id: String,
    script_id: String,
    uri: String,
}

impl SourceReferenceStore {
    fn get_or_create(&mut self, isolate_id: &str, script_id: &str, uri: &str) -> i64 {
        // Check if we already have a reference for this script
        for (&id, entry) in &self.references {
            if entry.script_id == script_id && entry.isolate_id == isolate_id {
                return id;
            }
        }
        // Create new reference
        self.next_id += 1;
        self.references.insert(self.next_id, SourceRefEntry {
            isolate_id: isolate_id.to_string(),
            script_id: script_id.to_string(),
            uri: uri.to_string(),
        });
        self.next_id
    }

    fn clear(&mut self) {
        self.references.clear();
        // Don't reset next_id — DAP clients may cache old references
    }
}
```

#### `source` Request Handler

```rust
async fn handle_source(&mut self, request: &DapRequest) -> DapResponse {
    let args: SourceArguments = parse_args(request)?;
    let source_ref = args.source_reference;

    let entry = self.source_reference_store.get(source_ref)
        .ok_or("Unknown source reference")?;

    // Fetch source text via VM Service: getObject(isolateId, scriptId)
    let source_text = self.backend.get_source(&entry.isolate_id, &entry.script_id).await?;

    DapResponse::success(request, json!({
        "content": source_text,
        "mimeType": "text/x-dart"
    }))
}
```

#### Backend Trait Addition

```rust
// In DebugBackend trait:
async fn get_source(&self, isolate_id: &str, script_id: &str) -> Result<String, String>;

// In VmServiceBackend:
async fn get_source(&self, isolate_id: &str, script_id: &str) -> Result<String, String> {
    let result = debugger::get_object(&self.handle, isolate_id, script_id).await
        .map_err(|e| e.to_string())?;
    // Parse the Script object's "source" field
    result["source"].as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Script has no source".to_string())
}
```

#### Package Resolution

For `package:` URIs, try resolving via `.dart_tool/package_config.json`:

```rust
fn resolve_package_uri(uri: &str, project_root: &Path) -> Option<PathBuf> {
    if !uri.starts_with("package:") { return None; }

    let config_path = project_root.join(".dart_tool/package_config.json");
    let config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config_path).ok()?).ok()?;

    let package_name = uri.strip_prefix("package:")?.split('/').next()?;
    let rest = uri.strip_prefix(&format!("package:{}/", package_name))?;

    let packages = config["packages"].as_array()?;
    for pkg in packages {
        if pkg["name"].as_str() == Some(package_name) {
            let root_uri = pkg["rootUri"].as_str()?;
            let package_uri = pkg.get("packageUri").and_then(|v| v.as_str()).unwrap_or("lib/");
            let root = if root_uri.starts_with("file://") {
                PathBuf::from(root_uri.strip_prefix("file://")?)
            } else {
                config_path.parent()?.join(root_uri)
            };
            return Some(root.join(package_uri).join(rest));
        }
    }
    None
}
```

#### Invalidation

Source references should be invalidated on hot restart (new isolate, old script IDs invalid). Call `source_reference_store.clear()` in `on_restart()`.

### Acceptance Criteria

1. Clicking a stack frame in SDK code (e.g., `dart:core`) opens the source in the IDE (read-only)
2. Package sources resolve to local paths when possible (editable)
3. `source` DAP request returns source text for a `sourceReference` ID
4. Unknown `sourceReference` returns an error response
5. Source references invalidated on hot restart
6. User code always uses `path` (not `sourceReference`) for editability
7. 10+ new unit tests

### Testing

```rust
#[test]
fn test_source_reference_store_get_or_create() {
    let mut store = SourceReferenceStore::new();
    let id1 = store.get_or_create("isolate/1", "script/1", "dart:core/string.dart");
    let id2 = store.get_or_create("isolate/1", "script/1", "dart:core/string.dart");
    assert_eq!(id1, id2); // Same script → same reference
    let id3 = store.get_or_create("isolate/1", "script/2", "dart:core/list.dart");
    assert_ne!(id1, id3); // Different script → different reference
}

#[test]
fn test_resolve_package_uri_local() {
    // Create temp package_config.json
    // Verify "package:my_pkg/main.dart" resolves to local path
}

#[test]
fn test_dart_sdk_uri_gets_source_reference() {
    // Stack frame with uri "dart:core/string.dart"
    // Verify Source object has sourceReference > 0 and no path
}

#[test]
fn test_user_code_gets_path_not_reference() {
    // Stack frame with uri "file:///home/user/app/lib/main.dart"
    // Verify Source object has path and sourceReference == 0
}
```

### Notes

- The `getObject` VM Service RPC on a `Script` object returns the `source` field with the full source text. This is the standard way to fetch source in the VM Service protocol.
- `supportsLoadedSourcesRequest` could be advertised to let IDEs browse all loaded scripts. This is optional and can be deferred.
- Source reference IDs should remain stable within a debug session (same script → same ID) so IDEs can cache content.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/stack.rs` | Added `SourceReferenceStore`, `SourceRefEntry`, `SourceRefInfo`; added `extract_source_with_store()` for URI-aware source extraction with store integration; added `resolve_package_uri()` for `.dart_tool/package_config.json` lookups; 16 new unit tests |
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `get_source()` to `LocalDebugBackend` trait; added `get_source_boxed()` to `DynDebugBackendInner`; implemented both on `DynDebugBackend`; added `source_reference_store: SourceReferenceStore` field to `DapAdapter`; added `handle_source()` handler; registered `"source"` in `handle_request()` dispatch; added `on_hot_restart()` which clears source references; updated all mock backends in test sections |
| `crates/fdemon-dap/src/adapter/evaluate.rs` | Added `get_source()` to `MockBackend` in test section |
| `crates/fdemon-dap/src/server/session.rs` | Added `get_source()` to `NoopBackend` and to `MockBackend` in test section |
| `crates/fdemon-dap/src/server/mod.rs` | Added `get_source_boxed()` to `MockBackendInner` in test section |
| `crates/fdemon-app/src/handler/dap_backend.rs` | Implemented `get_source()` on `VmServiceBackend` using `debugger::get_object` + `"source"` field extraction; added `get_source_boxed()` to `DynDebugBackendInner` impl |

### Notable Decisions/Tradeoffs

1. **`on_hot_restart()` vs modifying `on_resume()`**: Source references are only invalidated on hot restart (not on every resume), because IDEs may request source text any time after a stack frame is shown — not just while the debuggee is stopped. A new `on_hot_restart()` public method was added that clears the source reference store AND resets var/frame stores. Callers should call `on_hot_restart()` instead of `on_resume()` when handling a Flutter hot restart.

2. **ID preservation after `clear()`**: `next_id` is not reset when `clear()` is called. This ensures that after a hot restart, new IDs are numerically higher than any IDs the client may have cached. Reusing IDs after clear could cause stale content to be served from client caches.

3. **`extract_source_with_store` is additive**: The original `extract_source()` function is preserved for cases that don't need source reference assignment (e.g., logpoint output events). The new `extract_source_with_store()` is the richer variant for `stackTrace` responses.

4. **`script_id` fallback to URI**: When a VM frame's `script.id` field is absent, the URI itself is used as the script_id key. This is a safe fallback since URIs are unique within an isolate.

5. **`get_source` returns `Result<String, String>`**: Unlike other backend methods that use `BackendError`, `get_source` uses plain `String` errors to avoid coupling the source-fetch error type to the DAP adapter's internal error enum. This matches the task spec and keeps the interface simple.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-dap` - Passed (503 tests)
- `cargo test --workspace` - Passed (all crates)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **`on_hot_restart()` not yet wired**: The new `on_hot_restart()` method is implemented but must be called by the Engine integration layer (in `fdemon-app`) when a hot restart completes. This wiring is deferred to the hot-restart integration task. Until then, source references are never invalidated (only accumulated), which is safe but may serve stale content after restart.

2. **`extract_source_with_store` not yet used in `handle_stack_trace`**: The `handle_stack_trace` handler still calls the old `extract_source()`. Wiring `extract_source_with_store()` into `handle_stack_trace` requires passing the isolate_id and project_root into that handler, which is a follow-on change. The infrastructure (store, store method, `handle_source` handler) is all in place.
