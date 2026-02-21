## Task: Fix Non-Functional Recording Toggle

**Objective**: Make the network recording toggle actually pause/resume data collection. Currently, toggling recording only changes the UI indicator (REC/PAUSED) but does not stop the polling task from merging new entries.

**Depends on**: None
**Severity**: CRITICAL
**Review ref**: REVIEW.md Issue #2

### Scope

- `crates/fdemon-app/src/handler/devtools/network.rs`: Add `recording` guard in `handle_http_profile_received`, fix doc comment on `handle_toggle_network_recording`
- `crates/fdemon-app/src/handler/tests.rs`: Add tests for recording toggle behavior

### Root Cause

The polling task in `actions.rs` (line ~1576-1619) polls unconditionally — it has no reference to `NetworkState::recording`. The handler comment on `handle_toggle_network_recording` (line ~157-158) falsely claims "The polling task checks this flag each cycle."

The `recording` flag is only flipped by the toggle handler; nothing in the data path reads it.

### Fix

**Approach**: Option (b) from the review — check `recording` in `handle_http_profile_received`. This stays within TEA (no async state sharing needed) and is a minimal change.

In `crates/fdemon-app/src/handler/devtools/network.rs`, modify `handle_http_profile_received` (~line 17-28):

```rust
pub(crate) fn handle_http_profile_received(
    state: &mut AppState,
    session_id: SessionId,
    timestamp: i64,
    entries: Vec<HttpProfileEntry>,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // Always advance the timestamp so the poller stays incremental.
        // This ensures that when recording resumes, only NEW requests appear
        // (not a flood of everything during the pause).
        handle.session.network.last_poll_timestamp = Some(timestamp);

        // Only merge entries when recording is active.
        if handle.session.network.recording {
            handle.session.network.merge_entries(entries);
        }
    }
    UpdateResult::none()
}
```

**Key design decision**: `last_poll_timestamp` MUST still be updated even when `recording = false`. This keeps the poller's `updatedSince` cursor advancing so paused-then-resumed recording doesn't flood with stale entries.

Also fix the incorrect doc comment on `handle_toggle_network_recording`:

```rust
/// Toggle recording on/off.
///
/// Flips the `recording` flag. When recording is off, the polling task
/// continues but `handle_http_profile_received` discards incoming entries
/// while still advancing the timestamp cursor.
```

### Tests

Add tests that:
1. Call `handle_http_profile_received` with `recording = true` → entries are merged
2. Call `handle_http_profile_received` with `recording = false` → entries are NOT merged, but `last_poll_timestamp` IS updated
3. Toggle recording off → send entries → toggle on → send entries → only second batch visible

### Verification

```bash
cargo test -p fdemon-app -- recording
cargo test -p fdemon-app -- http_profile_received
cargo clippy -p fdemon-app
```

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/network.rs` | Fixed `handle_http_profile_received` to always advance `last_poll_timestamp` but only call `merge_entries` when `recording = true`. Fixed doc comment on `handle_toggle_network_recording` to accurately describe the guard-in-handler approach. |
| `crates/fdemon-app/src/handler/tests.rs` | Added 3 integration-level tests: `test_http_profile_received_merges_entries_when_recording_on`, `test_http_profile_received_discards_entries_when_recording_off_but_advances_timestamp`, `test_http_profile_received_only_shows_entries_after_recording_resumed`. |

### Notable Decisions/Tradeoffs

1. **Guard in TEA handler, not async task**: The fix checks `recording` inside `handle_http_profile_received` (pure, synchronous TEA handler) rather than inside the async polling task. This avoids any need for shared state or `Arc<AtomicBool>` across the async boundary, keeping the implementation within TEA constraints.
2. **Timestamp always advances**: `last_poll_timestamp` is updated unconditionally so the poller's `updatedSince` cursor keeps moving during a pause. Without this, resuming recording would flood the list with all requests that arrived during the pause — a UX regression.

### Testing Performed

- `cargo test -p fdemon-app -- recording` - Passed (4 tests: 1 pre-existing, 3 new)
- `cargo test -p fdemon-app -- http_profile_received` - Passed (5 tests: 2 pre-existing, 3 new)
- `cargo test -p fdemon-app` - Passed (998 unit tests + 1 doc test)
- `cargo clippy -p fdemon-app` - Passed (no warnings)
- `cargo fmt -p fdemon-app -- --check` - Passed (no formatting changes needed)

### Risks/Limitations

1. **Polling task still runs while paused**: Entries are discarded at the handler level, but the background polling task continues to call `getHttpProfile` at the normal interval. This is intentional (keeps the cursor moving) and matches the revised doc comment. A future optimization could add a short-circuit in the polling task to skip the RPC call entirely, but that would require sharing state across the async boundary and is outside the scope of this fix.
