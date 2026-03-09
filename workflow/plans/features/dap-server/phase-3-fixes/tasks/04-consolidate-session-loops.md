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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/server/session.rs` | Added `DebugEventSource` enum with `recv()` method; added `run_inner` private method on `impl<B: DebugBackend> DapClientSession<B>`; refactored `run_on` and `run_on_with_backend` to thin wrappers; added two new tests |

### Notable Decisions/Tradeoffs

1. **Option A (enum wrapper) implemented as specified**: `DebugEventSource` is a module-private enum in `session.rs`. The `recv()` method encapsulates all channel-specific error handling (lag warnings, closed-branch disable). `run_inner` holds a single `tokio::select!` loop shared by both entry points.

2. **`run_inner` takes `event_rx: &mut mpsc::Receiver<DapMessage>` as a parameter**: Both `run_on` and `run_on_with_backend` create their own `mpsc::channel` for adapter-generated DAP events and pass the receiver to `run_inner`. This avoids storing the channel in `self` and keeps the method signature clean.

3. **`run_on` kept on `impl DapClientSession<NoopBackend>`**: Preserved the existing public API surface. It constructs `DebugEventSource::Broadcast(Some(log_event_rx))` and delegates to `run_inner`. The `run` (TCP convenience) method is unchanged.

4. **Busy-poll fix from task 02 is preserved**: `DebugEventSource::recv()` transitions `Broadcast(Some(_))` to `None` on `RecvError::Closed`, then parks on `std::future::pending()` — same semantics as the `Option<broadcast::Receiver>` fix, now encapsulated in the enum.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed
- `cargo test -p fdemon-dap server::session` — Passed (38 tests including 2 new)
- `cargo test --workspace` — Passed (all crates, 0 failures)
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)

### Risks/Limitations

1. **`DebugEventSource` is module-private**: It cannot be used outside `session.rs`. This is intentional — it is an implementation detail. If future tasks need to construct sessions with custom event sources from outside the module, the enum would need to be made `pub(crate)`.

2. **`run_inner` has a slightly larger parameter list than the task sketch**: The task sketch omitted `event_rx` from the signature, but the actual implementation needs it since both entry points create their own channel. This is consistent with the design and has no correctness impact.
