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

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/debugger_types.rs` | Add `StepOption::Rewind` variant with `"Rewind"` wire string |
| `crates/fdemon-daemon/src/vm_service/debugger.rs` | Add `frame_index: Option<i32>` parameter to `resume()` function |
| `crates/fdemon-dap/src/adapter/types.rs` | Add `StepMode::Rewind` variant to `StepMode` enum |
| `crates/fdemon-dap/src/adapter/backend.rs` | Add `frame_index: Option<i32>` to `resume()` in trait, `DynDebugBackendInner`, and `DynDebugBackend` |
| `crates/fdemon-dap/src/adapter/mod.rs` | Add `first_async_marker_index: Option<i32>` field to `DapAdapter`; initialize in constructor |
| `crates/fdemon-dap/src/adapter/events.rs` | Update all `resume()` call sites to pass `None` for frame_index; clear `first_async_marker_index` in `on_resume()` |
| `crates/fdemon-dap/src/adapter/handlers.rs` | Update `resume()` call sites; add `restartFrame` to dispatch; add `handle_restart_frame` handler |
| `crates/fdemon-dap/src/adapter/variables.rs` | Track first `AsyncSuspensionMarker` index in `handle_stack_trace` |
| `crates/fdemon-dap/src/adapter/test_helpers.rs` | Update `MockTestBackend::resume` default and blanket impl; update 3 override impls |
| `crates/fdemon-dap/src/adapter/evaluate.rs` | Update 2 inline `DebugBackend` mock impls |
| `crates/fdemon-dap/src/adapter/tests/backend_phase6.rs` | Update `resume_boxed` in `DynDebugBackendInner` test mock |
| `crates/fdemon-dap/src/adapter/tests/production_hardening.rs` | Update `TrackingBackend::resume` override |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Register `restart_frame` test module |
| `crates/fdemon-dap/src/adapter/tests/restart_frame.rs` | New file: 11 unit tests for `restartFrame` handler |
| `crates/fdemon-dap/src/protocol/types.rs` | Add `supports_restart_frame: Option<bool>` to `Capabilities`; set `Some(true)` in `fdemon_defaults()`; add `RestartFrameArguments` type |
| `crates/fdemon-dap/src/server/session.rs` | Update `NoopBackend::resume` and test `MockBackend::resume` |
| `crates/fdemon-dap/src/server/mod.rs` | Update `DynDebugBackendInner::resume_boxed` in `BackendHandle` |
| `crates/fdemon-app/src/handler/dap_backend.rs` | Add `StepMode::Rewind => StepOption::Rewind` match arm; add `frame_index` to `resume()` and `resume_boxed()` |

### Notable Decisions/Tradeoffs

1. **`debugger::resume` extended, not replaced**: Added `frame_index: Option<i32>` as a new parameter to the daemon-level `resume` function rather than creating a separate `rewind` function. This keeps the API surface minimal.

2. **`StepOption::Rewind` added to daemon**: Rather than handling `"Rewind"` as a string literal in `dap_backend.rs`, added the variant to `StepOption` to keep the mapping type-safe and consistent with the other step options.

3. **Async boundary guard uses `>=` comparison**: Frame index >= first_async_marker_index is rejected. This matches the task specification ("frames at or above") — a frame at exactly the marker index is also an async boundary.

4. **`on_resume()` clears `first_async_marker_index`**: The async marker state is per-stop, so it must be cleared when the debuggee resumes, matching the lifecycle of `frame_store` and `var_store`.

### Testing Performed

- `cargo check --workspace` — Passed
- `cargo test --workspace --lib` — Passed (1861 + 372 + 734 + 700 + 867 = 4534 tests, 0 failed)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings from new code)
- `cargo fmt --all` — Applied (no manual changes needed)
- `cargo test -p fdemon-dap --lib restart_frame` — 11/11 passed

### Risks/Limitations

1. **VM-level rewind errors not tested**: Integration tests would be needed to verify that the Dart VM actually responds correctly to `resume` with `step: "Rewind"` and `frameIndex`. Unit tests only verify the adapter-level logic.
2. **Async boundary detection relies on `stackTrace` being called first**: `first_async_marker_index` is only populated after the IDE sends a `stackTrace` request. If `restartFrame` is called before `stackTrace`, the guard will be `None` and allow all frames. This is acceptable since the VM will return an error if the frame is invalid.
