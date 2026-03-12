## Task: Cap pending_watcher_errors Buffer

**Objective**: Add a capacity limit to `pending_watcher_errors: Vec<String>` to prevent unbounded memory growth if many watcher errors fire before a session starts.

**Depends on**: None

**Priority**: Consider (optional improvement)

### Scope

- `crates/fdemon-app/src/state.rs`: Add capacity constant and enforce it
- `crates/fdemon-app/src/handler/update.rs`: Apply cap when pushing errors (around line 307)

### Details

`pending_watcher_errors` buffers watcher errors that arrive before any Flutter session exists, then flushes them into the first session's log on `SessionStarted`. The buffer has no capacity limit.

**Risk scenario**: Misconfigured watcher path that the OS continuously re-reports errors for, combined with a user who never starts a session (e.g., reviewing the TUI then quitting). The buffer grows unboundedly.

**Practical risk**: Low. The notify crate typically doesn't spam repeat errors for the same path. But the fix is trivial and follows the project's pattern of defensive coding.

**Proposed fix**:

```rust
// In state.rs or a constants module
const MAX_PENDING_WATCHER_ERRORS: usize = 50;

// In handler/update.rs where errors are pushed:
if state.pending_watcher_errors.len() < MAX_PENDING_WATCHER_ERRORS {
    state.pending_watcher_errors.push(message.clone());
}
```

Alternatively, use a ring-buffer approach (consistent with the project's use of `RingBuffer` elsewhere) or truncate old entries.

### Acceptance Criteria

1. `pending_watcher_errors` cannot grow past a defined constant
2. The constant is named and documented
3. Oldest errors are either dropped or a "N more errors suppressed" message is added
4. Existing drain-on-session-start behavior unchanged
5. Unit test verifying the cap

### Testing

```bash
cargo test -p fdemon-app -- pending_watcher
cargo clippy -p fdemon-app -- -D warnings
```

### Notes

- The 50-error cap is a suggestion — adjust based on what feels reasonable for a pre-session error buffer
- A drain-on-quit path could also be added (currently errors are silently dropped if no session ever starts) but that's lower value since the TUI is shutting down anyway

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `MAX_PENDING_WATCHER_ERRORS: usize = 50` public constant with doc comment after `AppState` struct definition; updated `pending_watcher_errors` field doc to reference the cap |
| `crates/fdemon-app/src/handler/update.rs` | Imported `MAX_PENDING_WATCHER_ERRORS`; wrapped `pending_watcher_errors.push()` with a length guard |
| `crates/fdemon-app/src/handler/tests.rs` | Imported `MAX_PENDING_WATCHER_ERRORS`; added 3 unit tests: buffering with no session, cap enforcement, oldest-entries-kept on overflow |

### Notable Decisions/Tradeoffs

1. **Oldest errors kept, overflow dropped**: The proposed fix drops new errors once the cap is reached, preserving the earliest errors. This is the simplest and lowest-overhead approach. A ring-buffer alternative (dropping oldest) was not chosen because the first watcher errors are most useful for diagnosing a misconfiguration.
2. **Constant placed in `state.rs` as `pub`**: Placing it near the field it governs makes the relationship explicit and allows tests to reference it symbolically via the import, avoiding magic numbers in test assertions.
3. **No "N more errors suppressed" message**: Acceptance criterion 3 offered "dropped" as an acceptable outcome (the alternative of a suppressed-count message adds complexity for a low-probability scenario). The drop approach was chosen for simplicity.

### Testing Performed

- `cargo test -p fdemon-app -- pending_watcher` - Passed (3 tests)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- Pre-existing `fdemon-tui` snapshot failures confirmed unrelated to this task (version string mismatch present before changes)

### Risks/Limitations

1. **Overflow errors silently dropped**: Errors exceeding the cap are discarded with no log or counter. In pathological scenarios a user might see only the first 50 errors, missing subsequent ones. Given the practical risk is rated low, this is acceptable.
