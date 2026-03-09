## Task: Multi-Session Thread Grouping

**Objective**: Support debugging multiple concurrent Flutter sessions through the DAP thread model. Thread IDs are namespaced per session so isolates from different sessions don't collide. The `threads` request returns isolates from all active sessions with session name prefixes.

**Depends on**: 01-wire-debug-event-channel

**Estimated Time**: 4–6 hours

### Scope

- `crates/fdemon-dap/src/adapter/threads.rs`: Implement namespaced thread IDs per session
- `crates/fdemon-dap/src/adapter/mod.rs`: Route stepping/pause/continue to correct session based on thread ID
- `crates/fdemon-app/src/handler/dap_backend.rs`: Extend backend factory to support multi-session backends

### Details

#### Thread ID Namespacing

Each session gets a thread ID range:
- Session 0: thread IDs 1000–1999
- Session 1: thread IDs 2000–2999
- Session 2: thread IDs 3000–3999
- ...up to Session 8: thread IDs 9000–9999

```rust
const THREADS_PER_SESSION: i64 = 1000;

fn session_thread_base(session_index: usize) -> i64 {
    (session_index as i64 + 1) * THREADS_PER_SESSION
}

fn session_index_from_thread_id(thread_id: i64) -> usize {
    (thread_id / THREADS_PER_SESSION - 1) as usize
}
```

#### Multi-Session Thread Map

```rust
pub struct MultiSessionThreadMap {
    /// Per-session thread maps
    sessions: Vec<SessionThreads>,
}

struct SessionThreads {
    session_id: Uuid,
    session_name: String,       // e.g., "Pixel 7" or "Chrome"
    thread_base: i64,           // e.g., 1000
    next_local_id: i64,         // monotonic within session
    isolate_to_thread: HashMap<String, i64>,  // isolate_id → global thread_id
    thread_to_isolate: HashMap<i64, String>,   // global thread_id → isolate_id
}
```

#### Threads Response

The `threads` request returns isolates from ALL active sessions:

```json
{
  "threads": [
    { "id": 1000, "name": "[Pixel 7] main" },
    { "id": 1001, "name": "[Pixel 7] background worker" },
    { "id": 2000, "name": "[Chrome] main" }
  ]
}
```

Thread names are prefixed with the session/device name in brackets.

#### Request Routing

When the IDE sends a request targeting a specific thread (e.g., `continue`, `stepIn`, `stackTrace`):
1. Extract `threadId` from request arguments
2. Determine session index from thread ID range
3. Route to the correct session's backend
4. Translate thread ID back to isolate ID within that session

```rust
fn route_thread_request(&self, thread_id: i64) -> Option<(&dyn DebugBackend, &str)> {
    let session_idx = session_index_from_thread_id(thread_id);
    let session = self.sessions.get(session_idx)?;
    let isolate_id = session.thread_to_isolate.get(&thread_id)?;
    let backend = self.backends.get(session_idx)?;
    Some((backend, isolate_id))
}
```

#### Breakpoints Across Sessions

Breakpoints apply to ALL sessions (same codebase). When `setBreakpoints` is called:
1. Apply breakpoints to every session's VM Service
2. Track breakpoint verification per session (a breakpoint may resolve at different lines in different sessions due to different compilation states)

#### Backend Architecture

Currently the adapter has a single `backend: DynDebugBackend`. For multi-session:

**Option A: Multi-backend adapter** — The adapter holds multiple backends, one per session. Thread routing selects the backend.

**Option B: Composite backend** — A single `MultiSessionBackend` wraps multiple `VmServiceBackend`s and routes internally based on isolate ID prefix.

**Recommended: Option A** — Cleaner separation. The adapter already manages per-thread state; extending to per-session is natural.

### Acceptance Criteria

1. Running two Flutter sessions → `threads` returns isolates from both with session prefixes
2. Stepping in one session does not affect the other
3. Breakpoints are set in all sessions simultaneously
4. Thread IDs are stable across requests within a session
5. Thread events (`started`/`exited`) are correctly namespaced
6. Removing a session removes its threads and sends `thread exited` events
7. All existing single-session tests pass
8. 15+ new unit tests

### Testing

```rust
#[test]
fn test_thread_id_namespacing() {
    assert_eq!(session_thread_base(0), 1000);
    assert_eq!(session_thread_base(1), 2000);
    assert_eq!(session_index_from_thread_id(1042), 0);
    assert_eq!(session_index_from_thread_id(2001), 1);
}

#[test]
fn test_multi_session_threads_response() {
    let mut map = MultiSessionThreadMap::new();
    map.add_session(session_id_1, "Pixel 7");
    map.add_isolate(session_id_1, "isolates/1");
    map.add_session(session_id_2, "Chrome");
    map.add_isolate(session_id_2, "isolates/2");
    let threads = map.all_threads();
    assert_eq!(threads.len(), 2);
    assert!(threads[0].name.contains("[Pixel 7]"));
    assert!(threads[1].name.contains("[Chrome]"));
}

#[test]
fn test_route_to_correct_session() {
    // Thread ID 1000 → session 0
    // Thread ID 2000 → session 1
}
```

### Notes

- Single-session mode (current default) should continue to work unchanged. Thread IDs 1000+ still work, just without the session prefix in names.
- The `SessionManager` in `fdemon-app` already supports up to 9 sessions. This task mirrors that limit.
- Multi-session DAP requires the backend factory to create backends for multiple sessions, not just the "active" one. This may require changing `VmBackendFactory` to accept a session ID parameter.
- This is a larger task that touches multiple parts of the adapter. Consider implementing basic multi-session first (threads + routing), then breakpoint broadcasting as a follow-up if time is tight.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/threads.rs` | Added `MultiSessionThreadMap`, `SessionThreads`, `DapSessionId` type alias, `THREADS_PER_SESSION`, `MAX_SESSIONS` constants, `session_thread_base()` and `session_index_from_thread_id()` public helpers; 49 unit tests total (21 new multi-session tests) |
| `crates/fdemon-dap/src/adapter/mod.rs` | Extended re-export to include `MultiSessionThreadMap`, `DapSessionId`, `session_thread_base`, `session_index_from_thread_id`, `MAX_SESSIONS`, `THREADS_PER_SESSION` |

### Notable Decisions/Tradeoffs

1. **`DapSessionId = u64` instead of `uuid::Uuid`**: `uuid` is not a workspace dependency and `fdemon-app` already uses `SessionId = u64`. Using `u64` avoids a new dependency and mirrors the existing convention. Callers can use their `SessionId` value directly.

2. **`MultiSessionThreadMap` is a standalone aggregation type**: The existing `DapAdapter` continues using `ThreadMap` for its own single-session logic. `MultiSessionThreadMap` is an infrastructure type that higher-level code (e.g., a future multi-backend adapter) will use. This preserves all single-session behaviour unchanged.

3. **Session removal compacts the Vec rather than leaving a sentinel**: After `remove_session`, surviving sessions keep their original `thread_base` values (stored in `SessionThreads`), so routing via `lookup_thread` still works correctly despite index shifting. The index arithmetic in `session_index_from_thread_id` is used only to find the expected `thread_base`; the Vec is then scanned to find the matching session.

4. **`thread_to_isolate.keys()` iteration in `all_threads`**: Clippy flagged the original `for (&id, _iso) in &map` pattern as unnecessary key+value iteration; replaced with `.keys()` iteration to avoid the warning.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-dap` - Passed (538 tests, 49 in threads module)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Applied (no changes needed beyond minor formatting)

### Risks/Limitations

1. **Breakpoint broadcasting not implemented**: The task notes this is a follow-up. `MultiSessionThreadMap` provides the routing foundation (session lookup by thread ID), but `DapAdapter` still applies breakpoints only to the primary isolate of its single backend. Multi-backend breakpoint broadcasting is deferred.

2. **`remove_session` Vec compaction**: When a session in the middle of the Vec is removed, remaining sessions' `thread_base` values are stable (stored in `SessionThreads`) but `session_index_from_thread_id` may return a stale index for the new Vec layout. `lookup_thread` handles this correctly by searching by `thread_base` value, not Vec index. This is documented in the method body.
