## Task: Named Constants and Import Cleanup

**Objective**: Replace magic numbers with named constants, promote inline constants to module scope, and consolidate inline `use` declarations to top-level imports.

**Depends on**: None

**Review Issues**: #3 (Minor), #5 (Minor), #6 (Minor)

### Scope

- `crates/fdemon-app/src/actions/vm_service.rs`: Add `VM_CONNECT_TIMEOUT` constant, fix redundant `std::time::` path
- `crates/fdemon-app/src/actions/network.rs`: Move 4 inline `use` declarations to top-level
- `crates/fdemon-app/src/actions/inspector/mod.rs`: Promote `LAYOUT_FETCH_TIMEOUT` to module scope

### Details

#### Issue #3: Magic number in `vm_service.rs`

**Line 44** uses an unnamed timeout with a redundant fully-qualified path:
```rust
let connect_result = tokio::time::timeout(
    std::time::Duration::from_secs(10),   // ← magic number + redundant std::time::
    VmServiceClient::connect(&ws_uri),
)
```

`Duration` is already imported at line 15 (`use std::time::Duration;`).

**Fix:** Add a named constant alongside the existing heartbeat constants (lines 28-34):
```rust
/// Maximum time to wait for the initial VM Service WebSocket connection.
const VM_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
```

Replace line 44:
```rust
let connect_result = tokio::time::timeout(
    VM_CONNECT_TIMEOUT,
    VmServiceClient::connect(&ws_uri),
)
```

#### Issue #5: Inline `use` declarations in `network.rs`

Four inline `use` declarations exist inside function/async block bodies:

| Line | Declaration | Context |
|------|-------------|---------|
| 73 | `use fdemon_daemon::vm_service::network;` | `spawn_network_monitoring` async block |
| 227 | `use fdemon_daemon::vm_service::network;` | `spawn_fetch_http_request_detail` async block |
| 284 | `use fdemon_daemon::vm_service::network;` | `spawn_clear_http_profile` async block |
| 322 | `use std::process::Command;` | `open_url_in_browser` function body |

**Fix:** Move all to top-level imports. Merge the two `fdemon_daemon` imports:
```rust
use fdemon_daemon::vm_service::{network, VmRequestHandle};
```
Add:
```rust
use std::process::Command;
```
Remove all 4 inline declarations.

#### Issue #6: `LAYOUT_FETCH_TIMEOUT` inside async closure in `inspector/mod.rs`

**Line 288** defines the constant inside a `tokio::spawn(async move { ... })` block:
```rust
tokio::spawn(async move {
    // ...
    const LAYOUT_FETCH_TIMEOUT: Duration = Duration::from_secs(10);
```

**Fix:** Move to module scope, after the `use` block and before the first function:
```rust
/// Timeout for a single `getLayoutExplorerNode` RPC call.
const LAYOUT_FETCH_TIMEOUT: Duration = Duration::from_secs(10);
```

Remove the inline definition at line 288.

### Acceptance Criteria

1. `vm_service.rs` has `VM_CONNECT_TIMEOUT` constant at module scope; no `std::time::Duration` usage (use bare `Duration`)
2. `network.rs` has zero inline `use` declarations; all imports at top level
3. `inspector/mod.rs` has `LAYOUT_FETCH_TIMEOUT` at module scope; no inline `const` in async blocks
4. No magic number literals for timeouts/durations in any `actions/` file
5. `cargo clippy --workspace -- -D warnings` clean
6. `cargo test -p fdemon-app` passes

### Testing

No new behavior tests needed — these are pure style/organizational changes. Task 05 will add constant verification tests that depend on the constants being at module scope.

### Notes

- The `inspector/widget_tree.rs` constants (`MAX_POLLS`, `POLL_INTERVAL`, `POLL_CALL_TIMEOUT`) at lines 27-29 are inside a function body (not an async closure). This is standard Rust and doesn't need to change — they are scoped to the function that uses them.
- After promoting `LAYOUT_FETCH_TIMEOUT`, the warning log at `inspector/mod.rs:300` still hardcodes `"10s"`. This can optionally be updated to use `LAYOUT_FETCH_TIMEOUT.as_secs()` for a single source of truth, but this is not required.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/vm_service.rs` | Added `VM_CONNECT_TIMEOUT: Duration = Duration::from_secs(10)` constant at module scope (alongside existing heartbeat constants); replaced `std::time::Duration::from_secs(10)` magic literal with the named constant |
| `crates/fdemon-app/src/actions/network.rs` | Added `use std::process::Command;` and merged `use fdemon_daemon::vm_service::{network, VmRequestHandle};` at top-level; removed all 4 inline `use` declarations from function/async block bodies |
| `crates/fdemon-app/src/actions/inspector/mod.rs` | Promoted `LAYOUT_FETCH_TIMEOUT: Duration = Duration::from_secs(10)` from inside the `tokio::spawn(async move { ... })` block to module scope (with doc comment); removed the inline `const` definition |

### Notable Decisions/Tradeoffs

1. **Import merge in `network.rs`**: The existing top-level import `use fdemon_daemon::vm_service::VmRequestHandle;` was replaced with the merged form `use fdemon_daemon::vm_service::{network, VmRequestHandle};` as specified in the task, consolidating all `fdemon_daemon::vm_service` imports into one line.
2. **`LAYOUT_FETCH_TIMEOUT` log message**: The warning log at `inspector/mod.rs` still hardcodes `"10s"` as noted in the task notes — this is acceptable and not required to change for this task.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo test -p fdemon-app` - Passed (1161 unit tests + 1 doc test, 0 failed)
- `cargo fmt --all` - Applied (no formatting changes needed after edits)

### Risks/Limitations

1. **None**: All changes are pure style/organizational. No behavior was modified; the same constant values (10s, 30s, 5s, 3) are used throughout.
