## Task: Fix Error Handling in Runner Startup

**Objective**: Replace `let _ =` patterns with proper error logging for critical startup operations in `runner.rs`.

**Depends on**: Phase 2

**Estimated Time**: 15 minutes

### Scope

- `src/tui/runner.rs`: Fix 2 error-ignoring patterns identified in review

### Details

The Phase 2 review identified two violations of `CODE_STANDARDS.md` lines 54-62 which explicitly forbids the `let _ = ...` anti-pattern for ignoring errors.

#### Issue 1: Terminal Draw Error (line 65)

```rust
// ❌ Current (violates CODE_STANDARDS.md)
let _ = term.draw(|frame| render::view(frame, &mut state));

// ✅ Required fix
if let Err(e) = term.draw(|frame| render::view(frame, &mut state)) {
    error!("Failed to render initial frame: {}", e);
}
```

**Why it matters:** Ignoring draw errors at startup hides potential terminal issues. The user won't know why the app isn't displaying correctly.

#### Issue 2: Channel Send Error (line 70)

```rust
// ❌ Current (violates CODE_STANDARDS.md)
let _ = msg_tx.send(Message::StartAutoLaunch { configs }).await;

// ✅ Required fix
if let Err(e) = msg_tx.send(Message::StartAutoLaunch { configs }).await {
    error!("Failed to send auto-start message: {}. Auto-start will not trigger.", e);
}
```

**Why it matters:** If this fails, auto-start won't trigger but the user won't know why. This creates a silent failure mode that's hard to debug.

### Acceptance Criteria

1. `runner.rs` line 65: Terminal draw error is logged with `error!` macro
2. `runner.rs` line 70: Channel send error is logged with `error!` macro
3. `cargo clippy -- -D warnings` passes (no new warnings)
4. Error messages are descriptive and explain the consequence of the failure

### Testing

No new unit tests required - this is logging-only. Manual verification:

```bash
cargo fmt
cargo check
cargo clippy -- -D warnings
cargo test
```

### Notes

- `tracing` crate's `error!` macro should already be imported in `runner.rs`
- If not, add `use tracing::error;` at the top
- The error messages should help users understand what went wrong AND what the consequence is

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/runner.rs` | Added `error` to tracing imports (line 12); Replaced `let _ = term.draw(...)` with proper error handling (lines 65-67); Replaced `let _ = msg_tx.send(...)` with proper error handling (lines 72-76) |

### Notable Decisions/Tradeoffs

1. **Error message format**: Used descriptive messages that explain both the failure and its consequence (e.g., "Auto-start will not trigger.") to aid debugging
2. **Import consolidation**: Extended existing `tracing::warn` import to include `error` for consistency

### Testing Performed

- `cargo fmt` - Passed (auto-formatted multiline error message)
- `cargo check` - Passed (0.06s)
- `cargo test --lib` - Passed (1337 passed; 0 failed; 3 ignored)
- `cargo clippy -- -D warnings` - Passed (1.44s, no warnings)

### Risks/Limitations

None. This is a low-risk logging-only change that improves debuggability without altering control flow.
