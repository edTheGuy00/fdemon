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
