## Task: Document Synchronous process_message Ordering Assumption

**Objective**: Add a code comment documenting why `engine.process_message()` is called synchronously before the event loop starts, and what invariant must hold for this to remain safe.

**Depends on**: None

**Priority**: Consider (optional improvement)

### Scope

- `crates/fdemon-tui/src/runner.rs`: Add doc comment at lines 51 and 137

### Details

Both `run_with_project()` and `run_with_project_and_dap()` call `engine.process_message(Message::StartAutoLaunch { configs })` synchronously before `run_loop()` starts. This is inside an `async fn` but `process_message` is synchronous — it processes the message through `handler::update()` inline and enqueues any resulting `UpdateAction` for the subsequent `run_loop`.

The Architecture Enforcer noted this pattern is consistent with the existing `StartDapServer` message dispatch, but flagged that if the handler ever returns a follow-up `Message` (via `UpdateResult.message`), the ordering guarantees could break — the follow-up message would be processed before the event loop's normal drain cycle starts.

**Current safety**: `StartAutoLaunch` handler returns `UpdateAction::DiscoverDevicesAndAutoLaunch` (a side effect action), not a follow-up message. So this is safe today.

**Proposed documentation**:

```rust
// ORDERING: process_message is called synchronously before run_loop starts.
// This is safe because StartAutoLaunch returns an UpdateAction (async side effect),
// not a follow-up Message. If the handler is changed to return a follow-up Message,
// this call site must switch to engine.msg_sender().try_send() to preserve ordering.
```

### Acceptance Criteria

1. Both call sites (lines ~51 and ~137) have a comment explaining the ordering assumption
2. Comment specifies what would break (follow-up messages) and the migration path (use msg_sender)
3. No code changes — documentation only

### Testing

No tests needed — documentation-only change.

### Notes

- If task 09 (extract startup dispatch) is implemented first, this comment would go on the extracted helper instead
- Consider whether an `assert!` or `debug_assert!` checking that no follow-up message was returned would be appropriate as a runtime safety net
