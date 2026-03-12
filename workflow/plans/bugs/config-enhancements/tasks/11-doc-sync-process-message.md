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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/runner.rs` | Added `# Ordering` doc section to `dispatch_startup_action` function; added `ORDERING:` inline comment block to the `StartDapServer` call site in `run_with_project_and_dap` |

### Notable Decisions/Tradeoffs

1. **Comment placement on helper vs. call sites**: Task 09 was already complete, so the primary ordering comment was added as a `# Ordering` rustdoc section on `dispatch_startup_action` (the extracted helper). The `StartDapServer` call site in `run_with_project_and_dap` received a parallel inline `ORDERING:` comment since it is a separate synchronous `process_message` call not covered by the helper.

2. **Two-location coverage**: The original task referenced "lines ~51 and ~137" as the two `StartAutoLaunch` call sites. Since task 09 consolidated those into the `dispatch_startup_action` helper, the comment on the helper covers both former call sites. The `StartDapServer` call site was also documented as it follows the same pre-loop synchronous dispatch pattern.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-tui` - Passed
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed

### Risks/Limitations

1. **Documentation only**: No runtime enforcement. The comment describes a constraint that a future developer could violate without a compile-time guard. An optional `debug_assert!` was noted in the task but not added — it would require introspecting the `UpdateResult` return value, which would need a refactor beyond this documentation task's scope.
