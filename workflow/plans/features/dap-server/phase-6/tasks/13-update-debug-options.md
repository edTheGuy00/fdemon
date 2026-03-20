## Task: Implement updateDebugOptions Custom Request

**Objective**: Add the `updateDebugOptions` custom DAP request handler that toggles `debugSdkLibraries` and `debugExternalPackageLibraries` settings. This controls whether stepping enters SDK/framework code, and whether SDK frames are shown as debuggable in the stack trace.

**Depends on**: 02-expand-backend-trait

**Estimated Time**: 4–6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Add `updateDebugOptions` to the dispatch table
- `crates/fdemon-dap/src/adapter/mod.rs`: Add `debug_sdk_libraries` and `debug_external_package_libraries` fields to `DapAdapter`
- `crates/fdemon-dap/src/adapter/events.rs`: Re-apply library debuggability on `IsolateRunnable` events

### Details

#### Handler:

```rust
async fn handle_update_debug_options(&mut self, request: &DapRequest) -> DapResponse {
    let args = request.arguments.as_ref().ok_or("Missing arguments")?;

    if let Some(debug_sdk) = args.get("debugSdkLibraries").and_then(|v| v.as_bool()) {
        self.debug_sdk_libraries = debug_sdk;
    }
    if let Some(debug_external) = args.get("debugExternalPackageLibraries").and_then(|v| v.as_bool()) {
        self.debug_external_package_libraries = debug_external;
    }

    // Apply to all current isolates
    for isolate_id in self.thread_map.all_isolate_ids() {
        self.apply_library_debuggability(&isolate_id).await?;
    }

    DapResponse::success(request, json!({}))
}
```

#### Library classification:

```rust
async fn apply_library_debuggability(&self, isolate_id: &str) -> Result<(), String> {
    let isolate = self.backend.get_isolate(isolate_id).await?;
    let libraries = isolate.get("libraries")
        .and_then(|l| l.as_array())
        .ok_or("No libraries in isolate")?;

    for lib in libraries {
        let lib_id = lib.get("id").and_then(|i| i.as_str()).unwrap_or("");
        let uri = lib.get("uri").and_then(|u| u.as_str()).unwrap_or("");

        let is_debuggable = if uri.starts_with("dart:") {
            self.debug_sdk_libraries
        } else if uri.starts_with("package:") && !self.is_app_package(uri) {
            self.debug_external_package_libraries
        } else {
            true  // App code is always debuggable
        };

        self.backend.set_library_debuggable(isolate_id, lib_id, is_debuggable).await
            .unwrap_or_else(|e| tracing::warn!("Failed to set library debuggability: {}", e));
    }
    Ok(())
}
```

#### App package detection:

An app package is one whose URI matches `package:<project_name>/`. The project name comes from `pubspec.yaml`'s `name` field, or from the project directory name as fallback. Store this on `DapAdapter` initialization.

#### Re-apply on `IsolateRunnable`:

In `events.rs`, when handling `IsolateRunnable` events (new isolates from hot restart), call `apply_library_debuggability` before resuming the isolate. Library debuggability MUST be set BEFORE breakpoints are applied.

Order on `IsolateRunnable`:
1. `setLibraryDebuggable` for all libraries
2. `setIsolatePauseMode` (exception mode)
3. Apply all `desired_breakpoints`
4. Resume isolate

#### Settings initialization:

Set default values from the `attach` request arguments:
```rust
// In handle_attach:
self.debug_sdk_libraries = args.get("debugSdkLibraries")
    .and_then(|v| v.as_bool()).unwrap_or(false);
self.debug_external_package_libraries = args.get("debugExternalPackageLibraries")
    .and_then(|v| v.as_bool()).unwrap_or(false);
```

### Acceptance Criteria

1. `updateDebugOptions` toggles SDK library stepping for all isolates
2. `updateDebugOptions` toggles external package library stepping
3. New isolates (from hot restart) inherit the current settings
4. Library debuggability is set BEFORE breakpoints on `IsolateRunnable`
5. App code libraries are always debuggable
6. 10+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_update_debug_options_toggles_sdk() {
    // Send updateDebugOptions with debugSdkLibraries: true
    // Verify set_library_debuggable called with true for dart: libraries
}

#[tokio::test]
async fn test_isolate_runnable_applies_library_debuggability() {
    // Set debug_sdk_libraries = true
    // Simulate IsolateRunnable event
    // Verify library debuggability set before breakpoints
}
```

### Notes

- This is important for Flutter debugging — without it, stepping into framework code is an all-or-nothing experience.
- The ordering constraint (library debuggability → exception mode → breakpoints → resume) on `IsolateRunnable` is critical. If breakpoints are set before library debuggability, breakpoints in SDK code may not be hit even after toggling SDK debugging on.

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `debug_sdk_libraries`, `debug_external_package_libraries`, `app_package_name` fields to `DapAdapter`; initialized in `new_with_tx` |
| `crates/fdemon-dap/src/adapter/handlers.rs` | Added `updateDebugOptions` to dispatch; init fields in `handle_attach`; added `handle_update_debug_options`, `apply_library_debuggability`, `is_app_package` methods |
| `crates/fdemon-dap/src/adapter/events.rs` | Restructured `IsolateRunnable` handler: apply library debuggability first, then exception mode, then breakpoints |
| `crates/fdemon-dap/src/protocol/types.rs` | Added `debug_sdk_libraries`, `debug_external_package_libraries`, `package_name` fields to `AttachRequestArguments`; updated existing struct-literal test |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Added `mod update_debug_options` |
| `crates/fdemon-dap/src/adapter/tests/update_debug_options.rs` | New file: 20 unit tests |
| `crates/fdemon-dap/src/adapter/tests/exception_info.rs` | Reformatted by `cargo fmt` (pre-existing style inconsistency) |

### Notable Decisions/Tradeoffs

1. **`pub(super)` visibility for `apply_library_debuggability`**: The method is called from both `handlers.rs` and `events.rs`. Since both are submodules of `adapter/`, `pub(super)` correctly grants access to both.

2. **`is_app_package` uses trailing-slash match**: `package:my_app/` prefix matching prevents false positives where a package named `my_app` would match `my_app_test`.

3. **Ordering change in `IsolateRunnable`**: The existing exception-mode block (after breakpoints) was removed and replaced with the new order: library debuggability → exception mode → breakpoints. This is a functional improvement that satisfies the critical ordering constraint from the task spec.

4. **`apply_library_debuggability` returns `Result<(), String>`**: Uses plain `String` error (not `BackendError`) since this method is called from `events.rs` which uses fire-and-forget error handling.

5. **Empty `get_isolate` response**: When `get_isolate` returns an object without a `libraries` key, `unwrap_or(&empty_vec)` makes it a no-op. This is safe and avoids crashing on unexpected VM responses.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace` - Passed (all 4 library crates + binary)
- `cargo test -p fdemon-dap` - Passed (750 unit tests, 20 new tests)
- `cargo clippy -p fdemon-dap -- -D warnings` - Passed

### Risks/Limitations

1. **`apply_library_debuggability` in `IsolateRunnable`**: If `get_isolate` fails (e.g., VM not ready), the isolate's libraries won't have debuggability set. The failure is logged as a warning and breakpoints continue to be applied.

2. **No `IsolateRunnable` resume step**: The task spec mentions "Resume isolate" as step 4, but the existing `IsolateRunnable` handler in `events.rs` does not resume the isolate. This is intentional — the isolate is resumed by the Flutter tooling after `configurationDone`. Adding an unsolicited resume here would break the attach flow.
