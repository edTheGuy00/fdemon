## Task: Breakpoint Management

**Objective**: Implement `setBreakpoints` and `setExceptionBreakpoints` request handlers with full breakpoint lifecycle management — add, remove, diff, verify, and track the mapping between DAP IDs and VM Service IDs.

**Depends on**: 03-adapter-core-structure

**Estimated Time**: 4-5 hours

### Scope

- `crates/fdemon-dap/src/adapter/breakpoints.rs` — **NEW** BreakpointState, set/remove logic
- `crates/fdemon-dap/src/adapter/mod.rs` — Wire handlers to dispatch

### Details

#### Breakpoint State

```rust
// crates/fdemon-dap/src/adapter/breakpoints.rs

use std::collections::HashMap;

/// Tracks all breakpoints managed by the adapter.
pub struct BreakpointState {
    /// Next DAP breakpoint ID to assign.
    next_id: i64,
    /// DAP breakpoint ID → tracked breakpoint info.
    by_dap_id: HashMap<i64, AdapterBreakpoint>,
    /// VM Service breakpoint ID → DAP breakpoint ID.
    vm_to_dap: HashMap<String, i64>,
    /// Source path → list of DAP breakpoint IDs in that file.
    by_source: HashMap<String, Vec<i64>>,
}

/// A breakpoint tracked by the adapter.
#[derive(Debug, Clone)]
pub struct AdapterBreakpoint {
    pub dap_id: i64,
    pub vm_id: Option<String>,     // None until VM Service confirms
    pub source_path: String,
    pub line: i64,
    pub column: Option<i64>,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
    pub log_message: Option<String>,
    pub verified: bool,
}
```

#### `setBreakpoints` Handler

The `setBreakpoints` request is **per-file** — the client sends the complete list of desired breakpoints for a single source file. The adapter must diff against existing breakpoints:

```rust
impl<B: DebugBackend> DapAdapter<B> {
    pub async fn handle_set_breakpoints(&mut self, request: &DapRequest) -> DapResponse {
        // 1. Parse SetBreakpointsArguments
        let args: SetBreakpointsArguments = parse_args(request)?;
        let source_path = args.source.path.as_deref().unwrap_or("");
        let uri = path_to_dart_uri(source_path); // Convert /abs/path → package:app/...

        // 2. Get existing breakpoints for this file
        let existing_dap_ids = self.breakpoint_state.by_source
            .get(&uri).cloned().unwrap_or_default();

        // 3. Determine desired breakpoints from request
        let desired = args.breakpoints.unwrap_or_default();

        // 4. Remove breakpoints no longer in the request
        for dap_id in &existing_dap_ids {
            if let Some(bp) = self.breakpoint_state.by_dap_id.get(dap_id) {
                let still_wanted = desired.iter().any(|d| d.line == bp.line);
                if !still_wanted {
                    if let Some(vm_id) = &bp.vm_id {
                        // Get the isolate to remove from
                        if let Some(isolate_id) = self.primary_isolate_id() {
                            let _ = self.backend.remove_breakpoint(&isolate_id, vm_id).await;
                        }
                    }
                    self.breakpoint_state.remove(dap_id);
                }
            }
        }

        // 5. Add/update breakpoints from the request
        let mut response_breakpoints = Vec::new();
        for sbp in &desired {
            // Check if already exists at this line
            let existing = self.breakpoint_state.find_by_source_line(&uri, sbp.line);

            if let Some(dap_id) = existing {
                // Already exists — return current state
                if let Some(bp) = self.breakpoint_state.by_dap_id.get(&dap_id) {
                    response_breakpoints.push(to_dap_breakpoint(bp));
                }
            } else {
                // New breakpoint — add via VM Service
                let dap_id = self.breakpoint_state.allocate_id();
                let mut bp = AdapterBreakpoint {
                    dap_id,
                    vm_id: None,
                    source_path: uri.clone(),
                    line: sbp.line,
                    column: sbp.column,
                    condition: sbp.condition.clone(),
                    hit_condition: sbp.hit_condition.clone(),
                    log_message: sbp.log_message.clone(),
                    verified: false,
                };

                if let Some(isolate_id) = self.primary_isolate_id() {
                    match self.backend.add_breakpoint(
                        &isolate_id, &uri, sbp.line as i32, sbp.column.map(|c| c as i32),
                    ).await {
                        Ok(result) => {
                            bp.vm_id = Some(result.vm_id.clone());
                            bp.verified = result.resolved;
                            if let Some(line) = result.line {
                                bp.line = line as i64;
                            }
                            self.breakpoint_state.vm_to_dap.insert(result.vm_id, dap_id);
                        }
                        Err(e) => {
                            bp.verified = false;
                            // Breakpoint unverified with error message
                        }
                    }
                }

                response_breakpoints.push(to_dap_breakpoint(&bp));
                self.breakpoint_state.track(bp);
            }
        }

        let body = serde_json::json!({ "breakpoints": response_breakpoints });
        DapResponse::success(request, Some(body))
    }
}
```

#### Path-to-URI Conversion

Convert absolute filesystem paths to Dart package URIs for the VM Service:

```rust
/// Convert an absolute filesystem path to a Dart package URI.
///
/// For example:
/// - `/home/user/myapp/lib/main.dart` → `package:myapp/main.dart`
/// - `/home/user/myapp/lib/src/widget.dart` → `package:myapp/src/widget.dart`
///
/// Falls back to `file://` URI if the path is outside a known package.
fn path_to_dart_uri(path: &str) -> String {
    // Strategy:
    // 1. Look for `lib/` in the path
    // 2. Extract package name from pubspec.yaml or directory name
    // 3. Build package: URI
    // Fallback: use file:// URI
    // This is a simplified version — Phase 4 will use .dart_tool/package_config.json
    format!("file://{}", path)
}
```

**Note**: Full `package:` URI resolution requires reading `.dart_tool/package_config.json`. For Phase 3, use `file://` URIs which the VM Service also accepts. Phase 4 will add proper package URI resolution.

#### `setExceptionBreakpoints` Handler

```rust
pub async fn handle_set_exception_breakpoints(&mut self, request: &DapRequest) -> DapResponse {
    let args: SetExceptionBreakpointsArguments = parse_args(request)?;

    // Map DAP filter names to VM Service exception pause mode
    let mode = if args.filters.contains(&"All".to_string()) {
        "All"
    } else if args.filters.contains(&"Unhandled".to_string()) {
        "Unhandled"
    } else {
        "None"
    };

    self.exception_mode = mode.to_string();

    // Apply to all known isolates
    for (isolate_id, _) in self.thread_map.all_threads() {
        if let Some(isolate_id) = self.thread_map.isolate_id(isolate_id) {
            let _ = self.backend.set_exception_pause_mode(isolate_id, mode).await;
        }
    }

    // Response: empty breakpoints array (exception breakpoints don't have IDs)
    let body = serde_json::json!({ "breakpoints": [] });
    DapResponse::success(request, Some(body))
}
```

#### `BreakpointResolved` Event Handling

When the VM Service resolves a breakpoint (e.g., after a deferred library loads), update the tracked breakpoint and notify the IDE:

```rust
pub async fn handle_breakpoint_resolved(&mut self, vm_id: &str, line: Option<i32>, column: Option<i32>) {
    if let Some(&dap_id) = self.breakpoint_state.vm_to_dap.get(vm_id) {
        if let Some(bp) = self.breakpoint_state.by_dap_id.get_mut(&dap_id) {
            bp.verified = true;
            if let Some(line) = line {
                bp.line = line as i64;
            }

            let dap_bp = to_dap_breakpoint(bp);
            let event = DapEvent::breakpoint("changed", &dap_bp);
            let _ = self.event_tx.send(DapMessage::Event(event)).await;
        }
    }
}
```

### Acceptance Criteria

1. `setBreakpoints` correctly diffs existing vs desired breakpoints per file
2. Removed breakpoints are cleaned up via `removeBreakpoint` on the VM Service
3. New breakpoints are added via `addBreakpointWithScriptUri`
4. Each breakpoint gets a unique monotonic DAP ID
5. VM Service breakpoint IDs are tracked for lifecycle management
6. `setExceptionBreakpoints` maps filter names to VM Service exception pause modes
7. `BreakpointResolved` events update verification status and notify the IDE
8. Unresolved breakpoints return `verified: false` with an appropriate message
9. Unit tests cover the diff logic, ID allocation, and event handling

### Testing

```rust
#[test]
fn test_breakpoint_state_diff_add_new() {
    let mut state = BreakpointState::new();
    // No existing breakpoints
    // Request for line 10, 20
    // Both should be added
}

#[test]
fn test_breakpoint_state_diff_remove_old() {
    let mut state = BreakpointState::new();
    // Existing breakpoints at lines 10, 20, 30
    // Request for lines 10, 30 only
    // Line 20 should be removed
}

#[test]
fn test_breakpoint_state_diff_no_change() {
    // Existing = desired → no VM Service calls needed
}

#[test]
fn test_exception_filter_mapping() {
    // ["All"] → "All"
    // ["Unhandled"] → "Unhandled"
    // [] → "None"
    // ["All", "Unhandled"] → "All" (All takes precedence)
}

#[test]
fn test_breakpoint_resolved_updates_verification() {
    // Track unverified breakpoint → resolve → verified = true
}
```

### Notes

- The `setBreakpoints` request replaces ALL breakpoints for a given source file — it's not incremental
- Use `file://` URIs for Phase 3; proper `package:` URI resolution is Phase 4
- Both Zed and Helix support conditional breakpoints (Zed: right-click, Helix: `<space>G<C-c>`). These are passed through as `condition` on `SourceBreakpoint` — the actual evaluation happens in Phase 4 (requires `evaluateInFrame` at the pause point).
- For Phase 3, conditions are stored but not evaluated — breakpoints with conditions behave as unconditional. Document this limitation.
- Log points (breakpoints with `logMessage`) similarly require evaluation — defer to Phase 4.

---

## Completion Summary

**Status:** Not Started
