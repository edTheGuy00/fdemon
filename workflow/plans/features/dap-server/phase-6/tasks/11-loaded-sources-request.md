## Task: Implement loadedSources Request

**Objective**: Add the `loadedSources` DAP request handler that returns all loaded Dart scripts as DAP `Source` objects. This enables the "Loaded Scripts" panel in IDEs for navigating all available source files.

**Depends on**: 02-expand-backend-trait

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Add `loadedSources` to dispatch table with handler
- `crates/fdemon-dap/src/protocol/types.rs`: Add `supports_loaded_sources_request: Some(true)` to `fdemon_defaults()`

### Details

#### Handler implementation:

```rust
async fn handle_loaded_sources(&mut self, request: &DapRequest) -> DapResponse {
    // Get any paused or active isolate
    let isolate_id = self.most_recent_isolate_id()
        .ok_or("No isolate available")?;

    // Call existing backend method (already in DebugBackend trait)
    let scripts_response = self.backend.get_scripts(&isolate_id).await?;

    let scripts = scripts_response.get("scripts")
        .and_then(|s| s.as_array())
        .unwrap_or(&vec![]);

    let sources: Vec<DapSource> = scripts.iter()
        .filter_map(|script| {
            let uri = script.get("uri")?.as_str()?;
            let script_id = script.get("id")?.as_str()?;

            // Filter out internal/generated scripts
            if uri.starts_with("eval:") || uri.contains("dart:_") {
                return None;
            }

            // Use extract_source_with_store for consistent source resolution
            let source = build_source_from_uri(
                uri,
                script_id,
                &mut self.source_reference_store,
                &isolate_id,
                self.project_root.as_deref(),
            );
            Some(source)
        })
        .collect();

    DapResponse::success(request, json!({ "sources": sources }))
}
```

#### Source categorization:

| URI prefix | Treatment |
|---|---|
| `file://` | Path resolution via `dart_uri_to_path` |
| `package:` | Resolve via `resolve_package_uri` or assign `sourceReference` |
| `dart:` | Assign `sourceReference`, `presentationHint: "deemphasize"` |
| `org-dartlang-sdk:` | Assign `sourceReference`, `presentationHint: "deemphasize"` |
| `eval:source` | Filter out (generated) |
| `dart:_internal` etc. | Filter out (internal) |

#### Reuse `extract_source_with_store` pattern:

Factor out the URI → `DapSource` conversion from `extract_source_with_store` so both `handle_stack_trace` and `handle_loaded_sources` share the same logic. Or create a simpler `uri_to_source` helper.

### Acceptance Criteria

1. `loadedSources` returns all user-visible scripts as `Source` objects
2. SDK sources have `sourceReference > 0` and `presentationHint: "deemphasize"`
3. Package sources resolve to local paths when possible
4. Internal/generated scripts are filtered out
5. `supportsLoadedSourcesRequest: true` in capabilities
6. 6+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_loaded_sources_returns_scripts() {
    // MockBackend: get_scripts returns scripts: [{uri: "file:///app/lib/main.dart", id: "scripts/1"}, ...]
    // Verify response has sources array with resolved paths
}

#[tokio::test]
async fn test_loaded_sources_filters_internal() {
    // Scripts include "dart:_internal", "eval:source/1"
    // Verify these are NOT in the response
}

#[tokio::test]
async fn test_loaded_sources_deemphasizes_sdk() {
    // Scripts include "dart:core"
    // Verify presentationHint == "deemphasize" and sourceReference > 0
}
```

### Notes

- `backend.get_scripts()` is already defined in the `DebugBackend` trait and implemented in `VmServiceBackend` — it just hasn't been called from any handler until now.
- The `loadedSources` request does not take a thread ID — scripts are global to the isolate. Use the most recently active isolate.
- This is a "should-have" feature that the Dart DDS adapter doesn't implement. It enables the "Loaded Scripts" explorer in VS Code.
