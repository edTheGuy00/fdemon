## Task: Session Integration — Wire DapAdapter into DapClientSession

**Objective**: Replace the Phase 2 stub handlers in `DapClientSession` with real `DapAdapter` dispatch. Make the session async, add the `Attached` state, and implement the `DebugBackend` trait in `fdemon-app` to bridge to the actual VM Service client.

**Depends on**: 05-breakpoint-management, 06-execution-control, 08-variables, 09-evaluate

**Estimated Time**: 5-7 hours

### Scope

- `crates/fdemon-dap/src/server/session.rs` — Async session, DapAdapter integration
- `crates/fdemon-app/src/handler/dap.rs` — DebugBackend implementation, message wiring
- `crates/fdemon-app/src/actions/mod.rs` — Updated SpawnDapServer action with backend
- `crates/fdemon-app/src/engine.rs` — DebugBackend construction and event forwarding
- `crates/fdemon-dap/src/server/mod.rs` — Pass backend factory to accept loop

### Details

This is the most complex task — it wires everything together across the `fdemon-dap` and `fdemon-app` crates.

#### 1. Add `Attached` Session State

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Uninitialized,
    Initializing,
    Configured,
    Attached,      // NEW: actively debugging
    Disconnecting,
}
```

#### 2. Make Session Async and Adapter-Aware

The current `handle_request` is synchronous. Phase 3 handlers need async VM Service calls. Restructure:

```rust
pub struct DapClientSession<B: DebugBackend> {
    state: SessionState,
    next_seq: i64,
    client_info: Option<InitializeRequestArguments>,
    adapter: Option<DapAdapter<B>>,
    event_tx: mpsc::Sender<DapMessage>,  // for events sent alongside responses
}

impl<B: DebugBackend> DapClientSession<B> {
    pub async fn run_on<R, W>(
        reader: R,
        writer: W,
        mut shutdown_rx: watch::Receiver<bool>,
        backend: B,
        debug_event_rx: mpsc::Receiver<DebugEvent>,
    ) -> Result<()>
    where
        R: tokio::io::AsyncRead + Unpin + Send,
        W: tokio::io::AsyncWrite + Unpin + Send,
    {
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let mut session = Self::new(event_tx.clone(), backend);
        let mut reader = BufReader::new(reader);

        loop {
            tokio::select! {
                // DAP request from client
                result = read_message(&mut reader) => {
                    match result {
                        Ok(Some(DapMessage::Request(req))) => {
                            let responses = session.handle_request(&req).await;
                            for msg in &responses {
                                write_message(&mut writer, msg).await?;
                            }
                            if session.state == SessionState::Disconnecting {
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(e) => { tracing::warn!("DAP read error: {}", e); break; }
                        _ => {}
                    }
                }

                // DAP events from adapter (stopped, continued, thread, etc.)
                Some(event) = event_rx.recv() => {
                    let event = session.stamp_event(event);
                    write_message(&mut writer, &event).await?;
                }

                // Debug events from VM Service (forwarded by Engine)
                Some(debug_event) = debug_event_rx.recv() => {
                    if let Some(adapter) = &mut session.adapter {
                        adapter.handle_debug_event(debug_event).await;
                    }
                }

                // Server shutdown
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        let event = session.make_event(DapEvent::terminated());
                        let _ = write_message(&mut writer, &DapMessage::Event(event)).await;
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn handle_request(&mut self, request: &DapRequest) -> Vec<DapMessage> {
        match request.command.as_str() {
            // Phase 2 handlers (session-level, not adapter-level)
            "initialize" => self.handle_initialize(request),
            "configurationDone" => self.handle_configuration_done(request),
            "disconnect" => self.handle_disconnect(request),

            // Phase 3 handlers — delegate to adapter
            _ => {
                if self.state != SessionState::Configured && self.state != SessionState::Attached {
                    let resp = self.make_response(
                        DapResponse::error(request, "Not attached — send configurationDone and attach first")
                    );
                    return vec![DapMessage::Response(resp)];
                }

                if let Some(adapter) = &mut self.adapter {
                    let response = adapter.handle_request(request).await;
                    let response = self.make_response(response);

                    // If this was an attach request and it succeeded, transition state
                    if request.command == "attach" && response.success {
                        self.state = SessionState::Attached;
                    }

                    vec![DapMessage::Response(response)]
                } else {
                    let resp = self.make_response(
                        DapResponse::error(request, "No adapter available")
                    );
                    vec![DapMessage::Response(resp)]
                }
            }
        }
    }
}
```

#### 3. Implement `DebugBackend` in `fdemon-app`

The concrete backend lives in `fdemon-app` because it has access to `VmRequestHandle`:

```rust
// crates/fdemon-app/src/handler/dap.rs (or a new dap_backend.rs)

use fdemon_daemon::vm_service::VmRequestHandle;
use fdemon_dap::adapter::{DebugBackend, StepMode, BreakpointResult};

/// Concrete DebugBackend implementation that delegates to VmRequestHandle.
pub struct VmServiceBackend {
    handle: VmRequestHandle,
}

impl VmServiceBackend {
    pub fn new(handle: VmRequestHandle) -> Self {
        Self { handle }
    }
}

#[async_trait::async_trait]
impl DebugBackend for VmServiceBackend {
    async fn pause(&self, isolate_id: &str) -> Result<(), String> {
        fdemon_daemon::vm_service::debugger::pause(&self.handle, isolate_id)
            .await
            .map_err(|e| e.to_string())
    }

    async fn resume(&self, isolate_id: &str, step: Option<StepMode>) -> Result<(), String> {
        let vm_step = step.map(|s| match s {
            StepMode::Over => fdemon_daemon::vm_service::debugger_types::StepOption::Over,
            StepMode::Into => fdemon_daemon::vm_service::debugger_types::StepOption::Into,
            StepMode::Out => fdemon_daemon::vm_service::debugger_types::StepOption::Out,
        });
        fdemon_daemon::vm_service::debugger::resume(&self.handle, isolate_id, vm_step)
            .await
            .map_err(|e| e.to_string())
    }

    async fn add_breakpoint(
        &self, isolate_id: &str, uri: &str, line: i32, column: Option<i32>,
    ) -> Result<BreakpointResult, String> {
        let bp = fdemon_daemon::vm_service::debugger::add_breakpoint_with_script_uri(
            &self.handle, isolate_id, uri, line, column,
        ).await.map_err(|e| e.to_string())?;

        Ok(BreakpointResult {
            vm_id: bp.id,
            resolved: bp.resolved,
            line: bp.location.as_ref().and_then(|l| l.line),
            column: bp.location.as_ref().and_then(|l| l.column),
        })
    }

    async fn remove_breakpoint(&self, isolate_id: &str, bp_id: &str) -> Result<(), String> {
        fdemon_daemon::vm_service::debugger::remove_breakpoint(&self.handle, isolate_id, bp_id)
            .await
            .map_err(|e| e.to_string())
    }

    async fn set_exception_pause_mode(&self, isolate_id: &str, mode: &str) -> Result<(), String> {
        let vm_mode = match mode {
            "All" => fdemon_daemon::vm_service::debugger_types::ExceptionPauseMode::All,
            "Unhandled" => fdemon_daemon::vm_service::debugger_types::ExceptionPauseMode::Unhandled,
            _ => fdemon_daemon::vm_service::debugger_types::ExceptionPauseMode::None,
        };
        fdemon_daemon::vm_service::debugger::set_isolate_pause_mode(&self.handle, isolate_id, vm_mode)
            .await
            .map_err(|e| e.to_string())
    }

    async fn get_stack(&self, isolate_id: &str, limit: Option<i32>) -> Result<serde_json::Value, String> {
        let stack = fdemon_daemon::vm_service::debugger::get_stack(&self.handle, isolate_id, limit)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&stack).map_err(|e| e.to_string())
    }

    // ... similar implementations for get_object, evaluate, evaluate_in_frame, get_vm, get_scripts
}
```

#### 4. Engine Integration — Backend Construction

When the Engine starts a DAP session (TCP or stdio), it must:
1. Get the `VmRequestHandle` from the active Flutter session
2. Construct a `VmServiceBackend`
3. Create a `debug_event_tx` channel for forwarding debug stream events
4. Pass both to the DAP session

```rust
// In the SpawnDapServer action handler
let vm_handle = session_manager.active_session()
    .and_then(|s| s.vm_request_handle())
    .ok_or("No active session with VM Service connection")?;

let backend = VmServiceBackend::new(vm_handle.clone());
let (debug_event_tx, debug_event_rx) = mpsc::channel(64);

// Store debug_event_tx so the Engine can forward debug stream events to it
```

#### 5. Debug Event Forwarding

The Engine already subscribes to VM Service debug stream events. Add a forwarding path:

```rust
// When a debug event arrives and a DAP session is active:
if let Some(debug_event_tx) = &self.dap_debug_event_tx {
    let dap_event = match &vm_event {
        VmServiceEvent::PauseBreakpoint { isolate_id, .. } => {
            Some(DebugEvent::Paused {
                isolate_id: isolate_id.clone(),
                reason: PauseReason::Breakpoint,
            })
        }
        VmServiceEvent::PauseException { isolate_id, .. } => {
            Some(DebugEvent::Paused {
                isolate_id: isolate_id.clone(),
                reason: PauseReason::Exception,
            })
        }
        VmServiceEvent::Resume { isolate_id } => {
            Some(DebugEvent::Resumed {
                isolate_id: isolate_id.clone(),
            })
        }
        // ... map all relevant VM events to DebugEvent variants
        _ => None,
    };

    if let Some(event) = dap_event {
        let _ = debug_event_tx.send(event).await;
    }
}
```

#### 6. DapServerEvent Updates

Add new event variants for debugging lifecycle:

```rust
pub enum DapServerEvent {
    ClientConnected { client_id: String },
    ClientDisconnected { client_id: String },
    ServerError { reason: String },
    // NEW:
    DebugSessionStarted { client_id: String },
    DebugSessionEnded { client_id: String },
}
```

### Acceptance Criteria

1. `DapClientSession` transitions through: Uninitialized → Initializing → Configured → Attached → Disconnecting
2. After `attach`, all debugging commands are dispatched to `DapAdapter`
3. `VmServiceBackend` correctly calls all VM Service debug RPCs
4. Debug events from VM Service are forwarded to the adapter via channel
5. The adapter translates debug events to DAP events (stopped, continued, thread)
6. DAP events are written to the client with correct sequence numbers
7. The session handles concurrent DAP requests and debug events (via `tokio::select!`)
8. Existing Phase 2 tests continue to pass (initialization handshake unchanged)
9. New integration tests verify end-to-end attach flow
10. `cargo clippy --workspace` passes (no circular dependencies)

### Testing

```rust
// Use a mock DebugBackend for testing
struct MockBackend {
    // Pre-configured responses
}

#[async_trait::async_trait]
impl DebugBackend for MockBackend {
    async fn pause(&self, _: &str) -> Result<(), String> { Ok(()) }
    async fn resume(&self, _: &str, _: Option<StepMode>) -> Result<(), String> { Ok(()) }
    async fn get_vm(&self) -> Result<serde_json::Value, String> {
        Ok(json!({ "isolates": [{ "id": "isolates/1", "name": "main" }] }))
    }
    // ... other methods
}

#[tokio::test]
async fn test_full_debug_session() {
    // initialize → configurationDone → attach → setBreakpoints → disconnect
    let backend = MockBackend::new();
    // Run session on duplex streams
    // Verify correct responses at each step
}
```

### Notes

- This task is the most architecturally critical — it touches both `fdemon-dap` and `fdemon-app`
- The `DebugBackend` trait keeps the dependency arrow correct: `fdemon-app → fdemon-dap`, never the reverse
- The `DapClientSession` becomes generic over `B: DebugBackend`. For TCP mode, the server's accept loop must construct the backend. For stdio mode, it's provided at launch.
- Consider whether `async_trait` should be a workspace-level dependency or specific to `fdemon-dap`
- The debug event forwarding path must handle the case where no DAP session is active (drop events silently)
- Thread-safety: the `DapAdapter` is NOT shared across threads — each session has its own adapter instance

---

## Completion Summary

**Status:** Not Started
