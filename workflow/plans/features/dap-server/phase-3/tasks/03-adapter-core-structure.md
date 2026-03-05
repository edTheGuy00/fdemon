## Task: Create DapAdapter Core Structure

**Objective**: Build the `adapter/` module inside `fdemon-dap` that bridges DAP protocol requests to VM Service operations. This is the central dispatch and coordination layer — all other adapter tasks build on it.

**Depends on**: 01-expand-protocol-types

**Estimated Time**: 4-5 hours

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs` — **NEW** DapAdapter struct, request dispatch, trait definition
- `crates/fdemon-dap/src/lib.rs` — Register `adapter` module, re-export public types
- `crates/fdemon-dap/Cargo.toml` — No new dependencies needed (uses existing `tokio`, `serde_json`)

### Details

#### Architecture Challenge: No Circular Dependencies

`fdemon-dap` cannot depend on `fdemon-daemon` or `fdemon-app` — that would create circular dependencies in the workspace graph. The adapter needs to call VM Service RPCs, but it can't import `VmRequestHandle` directly.

**Solution: Trait-based abstraction**

Define a trait in `fdemon-dap` that describes the debugging operations the adapter needs. The Engine integration layer (Task 10) provides the concrete implementation.

```rust
// crates/fdemon-dap/src/adapter/mod.rs

use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

/// Trait abstracting the debug operations the DAP adapter needs.
///
/// Implemented by the Engine integration layer to bridge to the actual
/// VM Service client. This avoids `fdemon-dap` depending on `fdemon-daemon`.
#[async_trait::async_trait]
pub trait DebugBackend: Send + Sync + 'static {
    // ── Execution control ───────────────────────────────────────────
    async fn pause(&self, isolate_id: &str) -> Result<(), String>;
    async fn resume(&self, isolate_id: &str, step: Option<StepMode>) -> Result<(), String>;

    // ── Breakpoints ─────────────────────────────────────────────────
    async fn add_breakpoint(
        &self,
        isolate_id: &str,
        uri: &str,
        line: i32,
        column: Option<i32>,
    ) -> Result<BreakpointResult, String>;
    async fn remove_breakpoint(&self, isolate_id: &str, breakpoint_id: &str) -> Result<(), String>;
    async fn set_exception_pause_mode(&self, isolate_id: &str, mode: &str) -> Result<(), String>;

    // ── Stack inspection ────────────────────────────────────────────
    async fn get_stack(&self, isolate_id: &str, limit: Option<i32>) -> Result<serde_json::Value, String>;
    async fn get_object(
        &self,
        isolate_id: &str,
        object_id: &str,
        offset: Option<i64>,
        count: Option<i64>,
    ) -> Result<serde_json::Value, String>;

    // ── Evaluation ──────────────────────────────────────────────────
    async fn evaluate(
        &self,
        isolate_id: &str,
        target_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, String>;
    async fn evaluate_in_frame(
        &self,
        isolate_id: &str,
        frame_index: i32,
        expression: &str,
    ) -> Result<serde_json::Value, String>;

    // ── Thread/isolate info ─────────────────────────────────────────
    async fn get_vm(&self) -> Result<serde_json::Value, String>;
    async fn get_scripts(&self, isolate_id: &str) -> Result<serde_json::Value, String>;
}

/// Step mode for resume operations.
#[derive(Debug, Clone, Copy)]
pub enum StepMode {
    Over,
    Into,
    Out,
}

/// Result from adding a breakpoint.
#[derive(Debug, Clone)]
pub struct BreakpointResult {
    pub vm_id: String,
    pub resolved: bool,
    pub line: Option<i32>,
    pub column: Option<i32>,
}
```

**Alternative considered**: Using `serde_json::Value`-based message passing (no trait). Rejected because the trait provides compile-time API guarantees and better documentation.

**Note on `async_trait`**: Check if `fdemon-dap` already has `async-trait` in its deps. If not, add it to `Cargo.toml`. Alternatively, use a channel-based approach with request/response enums to avoid the `async_trait` dependency — evaluate which is cleaner.

#### DapAdapter Struct

```rust
/// The core DAP adapter that translates between DAP protocol and VM Service.
///
/// Each `DapAdapter` instance is bound to a single DAP client session and
/// a single Flutter debug session. It holds per-session state for ID allocation,
/// variable references, and thread mapping.
pub struct DapAdapter<B: DebugBackend> {
    /// The backend providing VM Service operations.
    backend: B,

    /// Channel to send DAP events back to the session writer.
    event_tx: mpsc::Sender<DapMessage>,

    /// Isolate ID → DAP thread ID mapping.
    thread_map: ThreadMap,

    /// Per-stopped-state variable reference allocator and lookup.
    var_store: VariableStore,

    /// Per-stopped-state frame ID allocator and lookup.
    frame_store: FrameStore,

    /// Breakpoint tracking state.
    breakpoint_state: BreakpointState,

    /// Current exception pause mode.
    exception_mode: String, // "None", "Unhandled", "All"
}
```

#### Request Dispatch

```rust
impl<B: DebugBackend> DapAdapter<B> {
    /// Handle a DAP request and return the response.
    ///
    /// This is the main dispatch point. The session calls this for every
    /// request that requires adapter involvement (i.e., everything except
    /// initialize/configurationDone/disconnect which the session handles).
    pub async fn handle_request(&mut self, request: &DapRequest) -> DapResponse {
        match request.command.as_str() {
            "attach" => self.handle_attach(request).await,
            "threads" => self.handle_threads(request).await,
            "setBreakpoints" => self.handle_set_breakpoints(request).await,
            "setExceptionBreakpoints" => self.handle_set_exception_breakpoints(request).await,
            "continue" => self.handle_continue(request).await,
            "next" => self.handle_next(request).await,
            "stepIn" => self.handle_step_in(request).await,
            "stepOut" => self.handle_step_out(request).await,
            "pause" => self.handle_pause(request).await,
            "stackTrace" => self.handle_stack_trace(request).await,
            "scopes" => self.handle_scopes(request).await,
            "variables" => self.handle_variables(request).await,
            "evaluate" => self.handle_evaluate(request).await,
            "disconnect" => self.handle_disconnect(request).await,
            _ => DapResponse::error(request, format!("unsupported command: {}", request.command)),
        }
    }

    /// Notify the adapter of a VM Service debug event.
    ///
    /// Called by the Engine integration layer when a debug stream event
    /// arrives. The adapter translates it to DAP events and sends them
    /// via `event_tx`.
    pub async fn handle_debug_event(&mut self, event: DebugEvent) {
        // Dispatch based on event kind → emit stopped/continued/thread events
    }
}
```

#### ID Allocation Types

```rust
/// Maps Dart isolate IDs to DAP thread IDs (monotonic integers).
pub struct ThreadMap {
    isolate_to_thread: HashMap<String, i64>,
    thread_to_isolate: HashMap<i64, String>,
    next_id: i64,
}

/// Allocates and looks up variable references for a stopped state.
///
/// Variable references are invalidated on every resume. When the debugger
/// resumes and then stops again, a fresh VariableStore is created.
pub struct VariableStore {
    references: HashMap<i64, VariableRef>,
    next_ref: i64,
}

/// What a variable reference points to.
pub enum VariableRef {
    /// A scope (locals, globals) for a specific frame.
    Scope { frame_index: i32, scope_kind: ScopeKind },
    /// A VM Service object that can be expanded.
    Object { isolate_id: String, object_id: String },
}

pub enum ScopeKind {
    Locals,
    Globals,
}

/// Allocates and looks up frame IDs for a stopped state.
pub struct FrameStore {
    frames: HashMap<i64, FrameRef>,
    next_id: i64,
}

/// What a frame ID points to.
pub struct FrameRef {
    pub isolate_id: String,
    pub frame_index: i32,
}
```

#### Debug Event Types

Define the events the adapter receives from the Engine (via channel):

```rust
/// Debug events forwarded from the Engine to the adapter.
#[derive(Debug, Clone)]
pub enum DebugEvent {
    /// An isolate paused.
    Paused {
        isolate_id: String,
        reason: PauseReason,
    },
    /// An isolate resumed.
    Resumed { isolate_id: String },
    /// A new isolate started.
    IsolateStart {
        isolate_id: String,
        name: String,
    },
    /// An isolate exited.
    IsolateExit { isolate_id: String },
    /// A breakpoint was resolved by the VM.
    BreakpointResolved {
        vm_breakpoint_id: String,
        line: Option<i32>,
        column: Option<i32>,
    },
    /// The Flutter app exited.
    AppExited { exit_code: Option<i64> },
}

/// Reason for a pause event.
#[derive(Debug, Clone)]
pub enum PauseReason {
    Breakpoint,
    Exception,
    Step,
    Interrupted,
    Entry,
    Exit,
}
```

#### Module Structure

```
crates/fdemon-dap/src/adapter/
├── mod.rs           ← DapAdapter, DebugBackend trait, request dispatch, DebugEvent
├── threads.rs       ← ThreadMap, handle_threads, handle_attach (Task 04)
├── breakpoints.rs   ← BreakpointState, handle_set_breakpoints (Task 05)
├── stack.rs         ← FrameStore, VariableStore, handle_stack_trace/scopes/variables (Tasks 07-08)
└── evaluate.rs      ← handle_evaluate (Task 09)
```

### Acceptance Criteria

1. `adapter/mod.rs` compiles with `DapAdapter` struct, `DebugBackend` trait, and dispatch method
2. `ThreadMap`, `VariableStore`, `FrameStore` types are defined with allocation/lookup methods
3. `DebugEvent` enum is defined for all VM Service debug events
4. `handle_request` dispatches to stub methods for each command (stubs return error "not yet implemented")
5. Event sending via `event_tx` channel is wired up
6. `VariableStore` resets on resume (invalidation)
7. All ID allocation is monotonic and 1-based
8. Unit tests cover `ThreadMap`, `VariableStore`, `FrameStore` allocation and lookup
9. `crates/fdemon-dap/src/lib.rs` re-exports `adapter` module

### Testing

```rust
#[test]
fn test_thread_map_allocates_monotonic_ids() {
    let mut map = ThreadMap::new();
    let id1 = map.get_or_create("isolates/1");
    let id2 = map.get_or_create("isolates/2");
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn test_thread_map_reuses_existing_id() {
    let mut map = ThreadMap::new();
    let id1 = map.get_or_create("isolates/1");
    let id2 = map.get_or_create("isolates/1");
    assert_eq!(id1, id2);
}

#[test]
fn test_variable_store_reset() {
    let mut store = VariableStore::new();
    let ref1 = store.allocate(VariableRef::Object { ... });
    assert!(store.lookup(ref1).is_some());
    store.reset();
    assert!(store.lookup(ref1).is_none());
}

#[test]
fn test_frame_store_allocates_monotonic_ids() {
    let mut store = FrameStore::new();
    let id1 = store.allocate(FrameRef { isolate_id: "isolates/1".into(), frame_index: 0 });
    let id2 = store.allocate(FrameRef { isolate_id: "isolates/1".into(), frame_index: 1 });
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}
```

### Notes

- The `DebugBackend` trait is the **key abstraction** that keeps `fdemon-dap` independent of `fdemon-daemon`. The concrete implementation lives in `fdemon-app` (Task 10).
- Consider whether `async_trait` is acceptable or if a channel-based approach is preferable. Both are valid; `async_trait` is simpler to implement and test.
- `VariableStore` must be cheap to create since it's rebuilt on every stop. Use a simple `HashMap` — no need for complex allocators.
- Frame IDs and variable references are session-scoped and invalidated on resume. They do NOT persist across stop/resume cycles.
- The `event_tx` channel is bounded (capacity 64 or similar) to prevent unbounded memory growth if the session writer falls behind.

---

## Completion Summary

**Status:** Not Started
