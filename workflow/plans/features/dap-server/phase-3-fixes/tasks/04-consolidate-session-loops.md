## Task: Consolidate `run_on` and `run_on_with_backend` Select Loops

**Objective**: Eliminate ~80 lines of duplicated select-loop logic between `run_on` (NoopBackend) and `run_on_with_backend` (generic backend). Currently, any bug fix to the event loop must be applied in two places.

**Depends on**: 01-wire-tcp-backend, 02-fix-stdio-busy-poll (both modify the session event loop)

**Estimated Time**: 3–4 hours

**Severity**: MAJOR — maintenance hazard; divergent bug fixes are inevitable.

### Scope

- `crates/fdemon-dap/src/server/session.rs`: Unify the two select loops into a single implementation

### Details

#### Current Duplication

Both `run_on` (lines 363–461) and `run_on_with_backend` (lines 260–341) contain a `tokio::select!` loop with 4 arms:

| Arm | `run_on_with_backend` | `run_on` | Identical? |
|-----|----------------------|----------|------------|
| ARM 1: inbound DAP request | ~24 lines | ~24 lines | Yes — verbatim |
| ARM 2: adapter events → client | ~15 lines | ~15 lines | Yes — verbatim |
| ARM 3: debug events from engine | `mpsc::Receiver`, no error handling | `broadcast::Receiver`, handles `Lagged`+`Closed` | **No** |
| ARM 4: server shutdown | ~8 lines | ~8 lines | Yes — verbatim |

Arms 1, 2, and 4 are copy-paste identical. The only meaningful difference is ARM 3: the channel type and its error handling.

#### Design Options

**Option A (recommended): Enum wrapper over channel types**

Create an enum that abstracts over the two receiver types:

```rust
enum DebugEventSource {
    /// Dedicated per-client channel (from backend factory, task 01)
    Dedicated(mpsc::Receiver<DebugEvent>),
    /// Shared broadcast channel (log forwarding to all clients)
    Broadcast(Option<broadcast::Receiver<DebugEvent>>),
    /// No event source (stdio with no backend)
    None,
}

impl DebugEventSource {
    async fn recv(&mut self) -> Option<DebugEvent> {
        match self {
            Self::Dedicated(rx) => rx.recv().await,
            Self::Broadcast(Some(rx)) => match rx.recv().await {
                Ok(event) => Some(event),
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("DAP log event receiver lagged, dropped {} events", n);
                    None // Continue loop, try again
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::debug!("DAP log event broadcast channel closed");
                    *self = Self::None; // Disable permanently (fixes busy-poll)
                    None
                }
            },
            Self::Broadcast(None) | Self::None => std::future::pending().await,
        }
    }
}
```

Then a single `run_inner` method handles all cases:

```rust
async fn run_inner<R, W>(
    &mut self,
    reader: &mut BufReader<R>,
    writer: &mut W,
    shutdown_rx: &mut watch::Receiver<bool>,
    debug_events: &mut DebugEventSource,
) -> Result<()> { /* single select loop */ }
```

**Option B: Trait abstraction over receivers**

Define a `DebugEventReceiver` trait with `async fn recv(&mut self) -> Option<DebugEvent>` and implement it for both `mpsc::Receiver` and `broadcast::Receiver`. This is cleaner but may require `async_trait` or similar for dynamic dispatch.

**Option C: Macro-based deduplication**

Use a macro to generate the common arms. Maintains compile-time monomorphization but reduces readability.

#### Recommendation

Option A is the best balance of simplicity and correctness. The enum is local to the session module, easy to understand, and naturally incorporates the busy-poll fix from task 02.

### Acceptance Criteria

1. Single select-loop implementation (no duplicated arms)
2. `run_on` and `run_on_with_backend` either delegate to a common method or are replaced by a single entry point
3. All existing session tests pass without modification
4. No performance regression (the enum dispatch is a single match, negligible cost)
5. The busy-poll fix from task 02 is preserved (broadcast `Closed` disables the branch)

### Testing

- All existing `session.rs` tests must pass
- All existing `stdio.rs` tests must pass
- New test: session with `DebugEventSource::None` processes requests normally
- New test: session with `DebugEventSource::Broadcast` handles `Closed` by disabling

### Notes

- Wait for tasks 01 and 02 to land first, as both modify the select loop and we don't want to refactor twice.
- The `run_on` method is on `impl DapClientSession<NoopBackend>` while `run_on_with_backend` is on `impl<B: DebugBackend> DapClientSession<B>`. The consolidated method should be on the generic impl.
- Consider whether `run_on` (NoopBackend entry point) should be kept as a thin convenience wrapper that constructs `DebugEventSource::Broadcast` and delegates, or removed entirely in favor of the unified entry point.
