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
