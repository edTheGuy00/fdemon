## Task: Stack Traces and Scopes

**Objective**: Implement the `stackTrace` and `scopes` request handlers. Map VM Service `Frame` objects to DAP `StackFrame` objects with proper source locations, and derive scopes (Locals, Globals) from each frame.

**Depends on**: 04-thread-management

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-dap/src/adapter/stack.rs` — **NEW** Frame mapping, scope derivation
- `crates/fdemon-dap/src/adapter/mod.rs` — Wire handlers to dispatch

### Details

#### `stackTrace` Handler

```rust
impl<B: DebugBackend> DapAdapter<B> {
    pub async fn handle_stack_trace(&mut self, request: &DapRequest) -> DapResponse {
        let args: StackTraceArguments = parse_args(request)?;

        let isolate_id = match self.thread_map.isolate_id(args.thread_id) {
            Some(id) => id.to_string(),
            None => return DapResponse::error(request, "Unknown thread"),
        };

        // Get stack from VM Service
        let limit = args.levels.map(|l| l as i32);
        let stack_json = match self.backend.get_stack(&isolate_id, limit).await {
            Ok(v) => v,
            Err(e) => return DapResponse::error(request, format!("Failed to get stack: {}", e)),
        };

        // Parse frames from VM Service response
        let frames = stack_json.get("frames")
            .and_then(|f| f.as_array())
            .map(|arr| arr.as_slice())
            .unwrap_or(&[]);

        let start_frame = args.start_frame.unwrap_or(0) as usize;
        let mut dap_frames = Vec::new();
        let total_frames = frames.len();

        for (i, frame) in frames.iter().enumerate().skip(start_frame) {
            let frame_index = i as i32;

            // Allocate a DAP frame ID
            let frame_id = self.frame_store.allocate(FrameRef {
                isolate_id: isolate_id.clone(),
                frame_index,
            });

            // Extract frame info from VM Service JSON
            let kind = frame.get("kind").and_then(|k| k.as_str()).unwrap_or("");
            let code_name = frame.get("code")
                .and_then(|c| c.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("<unknown>");

            // Build source location
            let source = extract_source(frame);
            let (line, column) = extract_line_column(frame);

            // Map frame kind to presentation hint
            let presentation_hint = match kind {
                "AsyncSuspensionMarker" => Some("label".to_string()),
                "AsyncCausal" => None, // normal async frame
                _ => None,
            };

            let name = if kind == "AsyncSuspensionMarker" {
                "<asynchronous gap>".to_string()
            } else {
                code_name.to_string()
            };

            dap_frames.push(DapStackFrame {
                id: frame_id,
                name,
                source,
                line: line.unwrap_or(0) as i64,
                column: column.unwrap_or(0) as i64,
                end_line: None,
                end_column: None,
                presentation_hint,
            });
        }

        let body = serde_json::json!({
            "stackFrames": dap_frames,
            "totalFrames": total_frames,
        });
        DapResponse::success(request, Some(body))
    }
}
```

#### Source Extraction

```rust
/// Extract a DapSource from a VM Service frame.
fn extract_source(frame: &serde_json::Value) -> Option<DapSource> {
    let location = frame.get("location")?;
    let script = location.get("script")?;
    let uri = script.get("uri")?.as_str()?;

    // Convert Dart URI to filesystem path
    let path = dart_uri_to_path(uri);

    // Determine presentation hint
    let hint = if uri.starts_with("dart:") {
        Some("deemphasize".to_string()) // SDK sources are de-emphasized
    } else if uri.starts_with("package:flutter/") {
        Some("deemphasize".to_string()) // Framework sources
    } else {
        None // User code — normal emphasis
    };

    Some(DapSource {
        name: Some(uri.rsplit('/').next().unwrap_or(uri).to_string()),
        path,
        source_reference: None, // Phase 4 will add source references for SDK sources
        presentation_hint: hint,
    })
}

/// Convert a Dart URI to a filesystem path.
///
/// - `package:app/main.dart` → resolved via .dart_tool/package_config.json (Phase 4)
/// - `file:///path/to/file.dart` → `/path/to/file.dart`
/// - `dart:core/...` → None (SDK sources need source references)
fn dart_uri_to_path(uri: &str) -> Option<String> {
    if uri.starts_with("file://") {
        // Strip file:// prefix
        Some(uri.strip_prefix("file://").unwrap_or(uri).to_string())
    } else if uri.starts_with("dart:") {
        // SDK sources — no local path available in Phase 3
        None
    } else if uri.starts_with("package:") {
        // Package URIs need resolution via package_config.json
        // For Phase 3: attempt basic resolution from lib/ directory
        // Phase 4 will implement full package resolution
        None
    } else {
        None
    }
}
```

#### Line/Column Extraction

```rust
/// Extract line and column from a VM Service frame's location.
fn extract_line_column(frame: &serde_json::Value) -> (Option<i32>, Option<i32>) {
    let location = match frame.get("location") {
        Some(loc) => loc,
        None => return (None, None),
    };
    let line = location.get("line").and_then(|l| l.as_i64()).map(|l| l as i32);
    let column = location.get("column").and_then(|c| c.as_i64()).map(|c| c as i32);
    (line, column)
}
```

#### `scopes` Handler

Each frame has up to two scopes: **Locals** and **Globals**.

```rust
pub async fn handle_scopes(&mut self, request: &DapRequest) -> DapResponse {
    let args: ScopesArguments = parse_args(request)?;

    // Look up the frame reference
    let frame_ref = match self.frame_store.lookup(args.frame_id) {
        Some(fr) => fr.clone(),
        None => return DapResponse::error(request, "Invalid frame ID (stale or unknown)"),
    };

    // Create Locals scope — always present
    let locals_ref = self.var_store.allocate(VariableRef::Scope {
        frame_index: frame_ref.frame_index,
        scope_kind: ScopeKind::Locals,
    });

    let mut scopes = vec![DapScope {
        name: "Locals".to_string(),
        presentation_hint: Some("locals".to_string()),
        variables_reference: locals_ref,
        named_variables: None,
        indexed_variables: None,
        expensive: false,
    }];

    // Optionally add Globals scope (can be expensive — flagged as such)
    let globals_ref = self.var_store.allocate(VariableRef::Scope {
        frame_index: frame_ref.frame_index,
        scope_kind: ScopeKind::Globals,
    });

    scopes.push(DapScope {
        name: "Globals".to_string(),
        presentation_hint: Some("globals".to_string()),  // not a standard hint but useful
        variables_reference: globals_ref,
        named_variables: None,
        indexed_variables: None,
        expensive: true, // Globals can be large — flag for lazy loading
    });

    let body = serde_json::json!({ "scopes": scopes });
    DapResponse::success(request, Some(body))
}
```

#### Frame Store

```rust
impl FrameStore {
    pub fn new() -> Self {
        Self {
            frames: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn allocate(&mut self, frame_ref: FrameRef) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.frames.insert(id, frame_ref);
        id
    }

    pub fn lookup(&self, frame_id: i64) -> Option<&FrameRef> {
        self.frames.get(&frame_id)
    }

    pub fn reset(&mut self) {
        self.frames.clear();
        self.next_id = 1;
    }
}
```

### Acceptance Criteria

1. `stackTrace` returns correctly mapped `StackFrame` objects with source locations
2. Frame IDs are monotonic and unique within a stopped state
3. Async suspension markers are rendered as `"<asynchronous gap>"` with `presentation_hint: "label"`
4. SDK sources (`dart:`) and framework sources (`package:flutter/`) get `presentation_hint: "deemphasize"`
5. `scopes` returns Locals and Globals scopes for each frame
6. Scope variable references are allocated and stored for later `variables` lookup
7. `startFrame` and `levels` arguments are respected for delayed stack trace loading
8. `totalFrames` is returned correctly
9. Invalid/stale frame IDs return clear error messages
10. Unit tests cover frame mapping, source extraction, and scope derivation

### Testing

```rust
#[test]
fn test_extract_source_from_file_uri() {
    let frame = serde_json::json!({
        "location": {
            "script": { "uri": "file:///home/user/app/lib/main.dart" },
            "line": 42,
            "column": 5
        }
    });
    let source = extract_source(&frame).unwrap();
    assert_eq!(source.path.as_deref(), Some("/home/user/app/lib/main.dart"));
    assert_eq!(source.name.as_deref(), Some("main.dart"));
    assert!(source.presentation_hint.is_none()); // user code
}

#[test]
fn test_extract_source_dart_sdk_deemphasized() {
    let frame = serde_json::json!({
        "location": {
            "script": { "uri": "dart:core/list.dart" },
            "line": 100
        }
    });
    let source = extract_source(&frame).unwrap();
    assert_eq!(source.presentation_hint.as_deref(), Some("deemphasize"));
    assert!(source.path.is_none()); // SDK sources have no local path
}

#[test]
fn test_async_gap_frame() {
    // Frame with kind: "AsyncSuspensionMarker" should produce
    // name: "<asynchronous gap>", presentation_hint: "label"
}

#[test]
fn test_frame_store_reset_invalidates_all() {
    let mut store = FrameStore::new();
    let id = store.allocate(FrameRef { isolate_id: "i/1".into(), frame_index: 0 });
    assert!(store.lookup(id).is_some());
    store.reset();
    assert!(store.lookup(id).is_none());
}
```

### Notes

- **Helix quirk**: Helix does not support variable paging (`supportsVariablePaging: false`). The adapter should return complete variable lists without pagination for Helix compatibility.
- **Zed quirk**: Zed supports delayed stack trace loading — `startFrame` and `levels` should be respected.
- Both editors use `pathFormat: "path"` — always return filesystem paths, not URIs, in `DapSource.path`.
- Async frames in Dart create "async causal" frames and "async suspension markers". The markers should be rendered as visual separators, not debuggable frames.
- The `scopes` response must not make async calls — it should only allocate references. The expensive work happens when `variables` is called on those references.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/stack.rs` | Added `extract_source`, `extract_line_column`, `dart_uri_to_path` helper functions with full docs; added 20 unit tests covering all source extraction and URI conversion cases |
| `crates/fdemon-dap/src/adapter/mod.rs` | Updated imports to include `DapScope`, `DapStackFrame`, `ScopesArguments`, `StackTraceArguments`; re-exported new helper functions; replaced `handle_stack_trace` and `handle_scopes` stubs with full implementations; updated `test_handle_request_stub_commands_return_error` to remove the two now-implemented commands; added `StackMockBackend` and 14 new handler tests |

### Notable Decisions/Tradeoffs

1. **Helper functions in `stack.rs`**: The `extract_source`, `extract_line_column`, and `dart_uri_to_path` helpers were placed in `stack.rs` (not `mod.rs`) because they are pure frame-mapping utilities with no adapter state dependency. They are re-exported from `mod.rs` for external use.

2. **`startFrame` pagination**: The `startFrame` argument skips frames from the beginning of the slice before allocating frame IDs. This means frame IDs are allocated only for the returned subset, which matches DAP lazy-loading semantics (Zed sends non-zero `startFrame` values for deferred loading).

3. **`levels` passed to VM Service**: The `levels` argument is forwarded as the `limit` parameter to `backend.get_stack()`. The VM Service may return fewer frames than requested; `totalFrames` always reflects the count actually returned, which satisfies DAP clients.

4. **`handle_scopes` is synchronous**: Per the task notes and DAP spec, the `scopes` handler only allocates variable references without calling the VM Service. This is correct and intentional — the expensive `getObject` calls happen later when `variables` is invoked.

5. **Async suspension marker handling**: Frames with `kind: "AsyncSuspensionMarker"` are rendered as `"<asynchronous gap>"` with `presentation_hint: "label"`. No scope allocation is done for these frames (they have no variables).

### Testing Performed

- `cargo check -p fdemon-dap` — Passed
- `cargo test -p fdemon-dap` — Passed (259 tests, 0 failed)
- `cargo clippy -p fdemon-dap -- -D warnings` — Passed (0 warnings)

### Risks/Limitations

1. **`package:` URI resolution deferred**: `dart_uri_to_path` returns `None` for all `package:` URIs. User-written packages (not flutter) will have no source path until Phase 4 adds `.dart_tool/package_config.json` resolution. IDEs will show these as source-less frames.

2. **Async causal frames**: Only `AsyncSuspensionMarker` frames are specially handled. `AsyncCausal` frames (which represent the async causal stack) are treated as regular frames since the VM Service reports them with normal kind values.
