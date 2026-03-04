# Action Items: DAP Server Phase 1

**Review Date:** 2026-03-04
**Verdict:** :x: REJECTED
**Blocking Issues:** 1

## Critical Issues (Must Fix)

### 1. Fix `parse_debug_event` / `parse_isolate_event` serde flatten bug

- **Source:** Risks & Tradeoffs Analyzer, confirmed by codebase research
- **Files:** `crates/fdemon-daemon/src/vm_service/debugger_types.rs` (lines 431-433, 517-518), `crates/fdemon-app/src/actions/vm_service.rs` (lines 209-212, 226-230)
- **Problem:** `StreamEvent` has `pub isolate: Option<IsolateRef>` as a named field alongside `#[serde(flatten)] pub data: Value`. Serde consumes `isolate` into the named field, so `data.get("isolate")` always returns `None`. All debug/isolate events are silently dropped in production.
- **Required Action:**
  1. Change `parse_debug_event` and `parse_isolate_event` signatures to accept `&StreamEvent` (matching `parse_flutter_error`, `parse_gc_event`, `parse_log_record` patterns)
  2. Read `isolate` from `event.isolate` instead of `data.get("isolate")`
  3. Continue using `event.data` for other fields (`topFrame`, `breakpoint`, etc.)
  4. Update call sites in `actions/vm_service.rs` to pass `&event.params.event`
  5. Add an integration test that deserializes raw JSON through `StreamEvent` and passes it to the parser
- **Acceptance:** `parse_debug_event` returns `Some(DebugEvent)` when called with a `StreamEvent` deserialized from real VM Service JSON

## Major Issues (Should Fix)

### 2. Add logging for parse failures on recognized streams

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-app/src/actions/vm_service.rs` (lines 208-238)
- **Problem:** When `parse_debug_event` / `parse_isolate_event` returns `None` for a recognized stream, the event is silently dropped with no trace
- **Suggested Action:** Add `tracing::debug!` on the `None` branch for each stream routing block

### 3. Import `stream_id` instead of using full path

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-app/src/actions/vm_service.rs` (lines 208, 225)
- **Problem:** Full path `fdemon_daemon::vm_service::protocol::stream_id::DEBUG` is inconsistent with how `client.rs` uses the constants
- **Suggested Action:** Add `use fdemon_daemon::vm_service::protocol::stream_id;` to the import block

### 4. Change `debug` module visibility to `pub(crate)`

- **Source:** Architecture Enforcer
- **File:** `crates/fdemon-app/src/handler/devtools/mod.rs` (line 11)
- **Problem:** `pub mod debug` but `network` and `performance` are `pub(crate)`. No re-exports exist for `debug`.
- **Suggested Action:** Change to `pub(crate) mod debug;`

### 5. Fix `ServiceExtensionAdded` empty-string RPC name

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-daemon/src/vm_service/debugger_types.rs` (lines 530-531)
- **Problem:** `unwrap_or("")` produces a semantically invalid event when `extensionRPC` is absent
- **Suggested Action:** Use `.map(str::to_owned)?` to return `None` instead of an empty string

### 6. Add `untrack_breakpoint` to `BreakpointRemoved` handler

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/handler/devtools/debug.rs` (lines 96-98)
- **Problem:** VM-initiated breakpoint removal leaves stale entries in `DebugState`
- **Suggested Action:** Call `handle.session.debug.untrack_breakpoint(&breakpoint.id)` in the `BreakpointRemoved` arm

## Minor Issues (Consider Fixing)

### 7. Add doc comment to `PauseReason::Step` explaining it's unused

- `crates/fdemon-app/src/session/debug_state.rs` line 22 — clarify this is a forward-looking placeholder

### 8. Add intent comments to no-op breakpoint handler arms

- `crates/fdemon-app/src/handler/devtools/debug.rs` lines 96-101 — match the comment style of `BreakpointAdded`

### 9. Add test note to `debugger.rs` test module

- Explain why tests are synchronous parameter-construction tests, not async RPC tests

### 10. Fix `test_parse_unknown_debug_event_returns_none`

- Include a valid `isolate` field in the test JSON so the test exercises the `_ => None` catch-all, not the missing-isolate path

### 11. Upgrade debug action stubs to `warn!` level

- `crates/fdemon-app/src/actions/mod.rs` lines 336-403 — `debug!` is off by default in release builds

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Critical issue #1 resolved (parser accepts `&StreamEvent`)
- [ ] Integration test added for full StreamEvent deserialization -> parse path
- [ ] All major issues resolved or justified
- [ ] `cargo fmt --all` passes
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes (no regressions)
- [ ] `cargo clippy --workspace -- -D warnings` passes
