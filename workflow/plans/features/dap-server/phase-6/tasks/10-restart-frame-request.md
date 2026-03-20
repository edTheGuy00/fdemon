## Task: Implement restartFrame Request

**Objective**: Add the `restartFrame` DAP request that rewinds execution to the start of a selected stack frame using the Dart VM Service's `kRewind` step mode. This enables the "Restart Frame" action in IDE debuggers.

**Depends on**: 02-expand-backend-trait

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Add `restartFrame` to the dispatch table with handler
- `crates/fdemon-dap/src/adapter/types.rs`: Add `Rewind` variant to `StepMode` enum
- `crates/fdemon-dap/src/protocol/types.rs`: Add `supports_restart_frame` field to `Capabilities` struct, set `Some(true)` in `fdemon_defaults()`

**Files Read (Dependencies):**
- `crates/fdemon-dap/src/adapter/stack.rs`: `FrameStore` for frame lookup, async marker tracking

### Details

#### 1. Add `Rewind` step mode:

```rust
// In adapter/types.rs:
pub enum StepMode {
    Over,
    Into,
    Out,
    Rewind,  // NEW — maps to VM Service "Rewind" step option
}
```

Update `StepMode` → VM Service string mapping (wherever it's serialized):
```rust
StepMode::Rewind => "Rewind",
```

#### 2. Update `DebugBackend::resume` to accept frame index:

The `resume` method currently takes `Option<StepMode>`. For `Rewind`, it also needs a `frameIndex` parameter. Two options:
- Change signature to `resume(isolate_id, step: Option<StepMode>, frame_index: Option<i32>)`
- Keep existing signature and add `resume_with_frame(isolate_id, step: StepMode, frame_index: i32)` for `Rewind` specifically

Recommendation: Change the signature of `resume` since only `Rewind` uses `frame_index`:
```rust
async fn resume(&self, isolate_id: &str, step: Option<StepMode>, frame_index: Option<i32>) -> Result<(), BackendError>;
```

Update all existing call sites to pass `None` for `frame_index`.

#### 3. Handler implementation:

```rust
async fn handle_restart_frame(&mut self, request: &DapRequest) -> DapResponse {
    let args = parse_args::<RestartFrameArguments>(request);
    let frame_ref = self.frame_store.lookup(args.frame_id)
        .ok_or("Invalid or stale frame ID")?;

    // Check for async suspension marker — cannot rewind past async boundary
    // Track the first async marker index in the frame store
    if let Some(first_async_index) = self.first_async_marker_index {
        if frame_ref.frame_index >= first_async_index {
            return DapResponse::error(request,
                "Cannot restart frame above an async suspension boundary");
        }
    }

    self.backend.resume(
        &frame_ref.isolate_id,
        Some(StepMode::Rewind),
        Some(frame_ref.frame_index),
    ).await?;

    // The VM will pause at the rewound frame — a PauseBreakpoint/PauseInterrupted
    // event will arrive and trigger a "stopped" DAP event automatically
    DapResponse::success(request, json!({}))
}
```

#### 4. Track async suspension marker index:

In `handle_stack_trace`, when processing frames, record the index of the first `AsyncSuspensionMarker` frame:

```rust
if frame_kind == "AsyncSuspensionMarker" {
    if self.first_async_marker_index.is_none() {
        self.first_async_marker_index = Some(frame_index);
    }
}
```

Add `first_async_marker_index: Option<i32>` to `DapAdapter` state, cleared on resume.

#### 5. Capability:

Add to `Capabilities` struct:
```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub supports_restart_frame: Option<bool>,
```

Set in `fdemon_defaults()`:
```rust
supports_restart_frame: Some(true),
```

Also add `supports_restart_request: Some(true)` back (removed in Task 01) since Task 13 or this task can implement session-level restart as a `hot_restart` call.

### Acceptance Criteria

1. `restartFrame` rewinds execution to the selected frame
2. `supportsRestartFrame: true` in capabilities
3. Async frames (above first async marker) are rejected with clear error
4. After rewind, VM pauses at the rewound frame and `stopped` event is sent
5. `StepMode::Rewind` is correctly mapped to VM Service `"Rewind"` step option
6. All existing `resume` call sites updated for new signature
7. 8+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_restart_frame_calls_resume_with_rewind() {
    // Set up frame store with frame at index 2
    // Call handle_restart_frame
    // Verify backend.resume called with StepMode::Rewind and frame_index 2
}

#[tokio::test]
async fn test_restart_frame_rejects_async_frame() {
    // Set first_async_marker_index = Some(3)
    // Try to restart frame at index 3
    // Verify error response
}

#[tokio::test]
async fn test_restart_frame_allows_sync_frame() {
    // Set first_async_marker_index = Some(3)
    // Restart frame at index 1 (below async marker)
    // Verify success
}
```

### Notes

- This is a killer feature for Flutter debugging — developers can rewind to re-execute a function without restarting the app.
- The VM's `Rewind` step is only valid for frames below the first async suspension marker. Attempting to rewind above it will cause the VM to return an error.
- After rewind, the existing `PauseInterrupted` or `PauseBreakpoint` event handler will naturally emit the `stopped` DAP event — no special handling needed.
