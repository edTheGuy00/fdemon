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

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/stack.rs` | Added `build_source_from_uri` free function: URI+script_id → `DapSource` with correct strategy for file://, package:, dart:, org-dartlang-sdk: |
| `crates/fdemon-dap/src/adapter/mod.rs` | Exported `build_source_from_uri` from the module's public API |
| `crates/fdemon-dap/src/adapter/handlers.rs` | Added `"loadedSources"` to dispatch table; added `handle_loaded_sources` method; imported `build_source_from_uri` |
| `crates/fdemon-dap/src/protocol/types.rs` | Set `supports_loaded_sources_request: Some(true)` in `fdemon_defaults()`; updated existing test that asserted `is_none()` |
| `crates/fdemon-dap/src/adapter/tests/loaded_sources.rs` | New file: 9 unit tests covering all acceptance criteria |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Added `mod loaded_sources;` |

### Notable Decisions/Tradeoffs

1. **`build_source_from_uri` vs reusing `extract_source_with_store`**: The existing `extract_source_with_store` takes a VM frame JSON object and extracts URI + script_id from it. For `loadedSources`, scripts are available directly as `{uri, id}` pairs (not nested inside frames). A new `build_source_from_uri(uri, script_id, store, isolate_id, project_root)` function was added to `stack.rs` with the same logic, keeping the original function untouched.

2. **Isolate selection**: The task says "use the most recently active isolate." The adapter doesn't have a `most_recent_isolate_id()` method, so the handler tries `most_recent_paused_isolate()` first (a paused isolate has a valid script list) and falls back to `primary_isolate_id()` (the first registered isolate). This matches the intent without requiring new state.

3. **`project_root` is `None`**: The `DapAdapter` struct doesn't store a `project_root`, same as `handle_stack_trace`. Package URIs without a resolvable path get a `sourceReference` instead, which is handled correctly by the `source` request handler.

### Testing Performed

- `cargo check -p fdemon-dap` — Passed
- `cargo test -p fdemon-dap loaded_sources` — Passed (9 tests)
- `cargo test --workspace` — Passed (all 709 fdemon-dap tests, 3,863+ total)
- `cargo clippy -p fdemon-dap -- -D warnings` — Passed
- `cargo fmt --all` — Applied (no functional changes)

### Risks/Limitations

1. **No project root**: Package URIs are never resolved to local paths in the `loadedSources` response since `DapAdapter` doesn't hold `project_root`. This is consistent with how `handle_stack_trace` works. IDEs will request source text via the `source` request using the assigned `sourceReference`.
2. **All-or-nothing filtering**: Internal scripts are filtered by simple string matching (`eval:`, `dart:_`). If the Dart SDK introduces new internal URI patterns, they would appear in the loaded scripts list until explicitly filtered.
