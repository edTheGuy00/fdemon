## Task: Add parse failure logging and fix stream_id imports

**Objective**: Add diagnostic logging when recognized stream events fail to parse, and fix inconsistent `stream_id` constant imports.

**Depends on**: 01-fix-serde-flatten-bug (call sites change in Task 01)

**Review Issues**: #2 (parse failure logging), #3 (stream_id import path)

### Scope

- `crates/fdemon-app/src/actions/vm_service.rs`:
  - Add `use fdemon_daemon::vm_service::protocol::stream_id;` to the import block
  - Replace inline `fdemon_daemon::vm_service::protocol::stream_id::DEBUG` with `stream_id::DEBUG`
  - Replace inline `fdemon_daemon::vm_service::protocol::stream_id::ISOLATE` with `stream_id::ISOLATE`
  - Add `tracing::debug!` on the `None` branch for Debug and Isolate stream routing

### Details

**Issue #3 — Import fix:**

The existing pattern in `client.rs` imports `stream_id` as a module:
```rust
use super::protocol::{
    parse_vm_message, stream_id, IsolateInfo, ...
};
// Then used as:
stream_id::EXTENSION, stream_id::DEBUG, etc.
```

The new code in `vm_service.rs` should follow the same pattern:
```rust
use fdemon_daemon::vm_service::protocol::stream_id;
// Then used as:
stream_id::DEBUG, stream_id::ISOLATE
```

**Issue #2 — Parse failure logging:**

After Task 01 changes the call sites, the Debug/Isolate stream blocks will look like:
```rust
if event.params.stream_id == stream_id::DEBUG {
    if let Some(debug_event) = parse_debug_event(&event.params.event) {
        // ... send message
    } else {
        tracing::debug!(
            "Debug stream: unrecognized or malformed event kind '{}'",
            event.params.event.kind
        );
    }
    continue;
}

if event.params.stream_id == stream_id::ISOLATE {
    if let Some(isolate_event) = parse_isolate_event(&event.params.event) {
        // ... send message
    } else {
        tracing::debug!(
            "Isolate stream: unrecognized or malformed event kind '{}'",
            event.params.event.kind
        );
    }
    continue;
}
```

Note: The other stream handlers (Extension, GC, Logging) also silently drop parse failures. This task only adds logging for the new Debug and Isolate streams. Addressing the existing streams is out of scope.

### Acceptance Criteria

1. `stream_id` constants are imported via `use fdemon_daemon::vm_service::protocol::stream_id;` — no full paths inline
2. `tracing::debug!` emitted when `parse_debug_event` returns `None` for a recognized Debug stream event
3. `tracing::debug!` emitted when `parse_isolate_event` returns `None` for a recognized Isolate stream event
4. Log messages include the event kind for diagnosis
5. `cargo check --workspace` passes
6. `cargo clippy --workspace -- -D warnings` passes

### Testing

- Manual verification: enable `RUST_LOG=fdemon_app=debug` and confirm log output when an unrecognized event kind is received
- No new unit tests required (logging side-effects are not unit-testable in this codebase)

### Notes

- Keep log level at `debug!` not `warn!` — unrecognized event kinds are expected when the VM introduces new events. This is diagnostic, not an error.
- The existing Extension/GC/Logging handlers follow the same silent-drop pattern. A separate task could add logging to those, but it's out of scope here.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/vm_service.rs` | Added `use fdemon_daemon::vm_service::protocol::stream_id;` import; replaced two inline full-path `fdemon_daemon::vm_service::protocol::stream_id::DEBUG/ISOLATE` references with `stream_id::DEBUG` and `stream_id::ISOLATE`; added `else` branches with `tracing::debug!` on both the Debug and Isolate stream parse-failure paths |

### Notable Decisions/Tradeoffs

1. **Import placement**: Added the `stream_id` import as a separate `use` statement rather than nesting it inside the existing `fdemon_daemon::vm_service` block. `rustfmt` reordered it to sit above the multi-item import, which is idiomatic (shorter/simpler imports before grouped ones). The alternative — adding `protocol::stream_id` to the existing block — would have required either a re-export in `fdemon_daemon::vm_service` (which doesn't exist) or restructuring into a nested `use` tree. The current approach is cleaner and idiomatic.

2. **Log level `debug!` not `warn!`**: Kept at `debug!` as specified. Unrecognized event kinds are expected as the Dart VM introduces new protocol events; `warn!` would create noise in production logs.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo clippy --workspace -- -D warnings` - Passed
- `cargo test --workspace` - Passed (all tests)
- `cargo fmt --all` - Applied (reordered imports to idiomatic position)

### Risks/Limitations

1. **No unit tests**: The task explicitly states no new unit tests are required, and logging side-effects are not unit-testable in this codebase. The `else` branches are only exercised at runtime when the VM sends an unrecognized event kind.
