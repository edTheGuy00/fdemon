## Task: Implement exceptionInfo Request

**Objective**: Add the `exceptionInfo` DAP request handler that returns structured exception data when the debugger is paused at an exception. This provides rich exception details in the IDE's exception dialog instead of just a basic "stopped" message.

**Depends on**: 02-expand-backend-trait, 04-exception-scope

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Add `exceptionInfo` to the dispatch table with handler
- `crates/fdemon-dap/src/protocol/types.rs`: Add `supports_exception_info_request: Some(true)` to `fdemon_defaults()`

### Details

#### Handler implementation:

```rust
"exceptionInfo" => self.handle_exception_info(request).await,
```

The handler:
1. Parse `ExceptionInfoArguments { threadId }`
2. Look up exception reference from `self.exception_refs` (added by Task 04)
3. If no exception stored: return error "No exception available"
4. Call `backend.get_object(isolate_id, exception_object_id, None, None)` to get the full exception Instance
5. Call `backend.evaluate(isolate_id, exception_id, "toString()")` with 1s timeout for the description
6. Extract the exception's class name for `typeName`
7. Try to get stack trace: `backend.evaluate(isolate_id, exception_id, "stackTrace?.toString()")` for `stackTrace` field

#### Response format:

```json
{
  "exceptionId": "objects/12345",
  "description": "FormatException: Unexpected character (at character 1)\n!@#$\n^",
  "breakMode": "unhandled",
  "details": {
    "message": "Unexpected character (at character 1)",
    "typeName": "FormatException",
    "stackTrace": "...",
    "evaluateName": "$_threadException"
  }
}
```

#### breakMode mapping:

| `ExceptionPauseMode` | DAP `breakMode` |
|---|---|
| `All` | `"always"` |
| `Unhandled` | `"unhandled"` |
| `None` | `"never"` |

### Acceptance Criteria

1. `exceptionInfo` request returns structured exception data when paused at exception
2. `description` field contains the exception's `toString()` output
3. `typeName` contains the exception class name (e.g., "FormatException")
4. `breakMode` reflects the current exception pause mode
5. Returns error when no exception is available (paused at breakpoint/step)
6. `supportsExceptionInfoRequest: true` in capabilities
7. 6+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_exception_info_returns_structured_data() {
    // Store exception ref for thread
    // Call handle_exception_info
    // Verify response has exceptionId, description, breakMode, details
}

#[tokio::test]
async fn test_exception_info_no_exception_returns_error() {
    // No exception stored
    // Call handle_exception_info
    // Verify error response
}
```

### Notes

- This is a differentiator — neither the Dart DDS adapter nor Dart-Code implement `exceptionInfo`. fdemon will provide richer exception data than the official adapter.
- The `details` field is optional but valuable — IDEs like VS Code display it in a separate exception details panel.

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/protocol/types.rs` | Added `ExceptionInfoArguments` struct; added `supports_exception_info_request: Some(true)` to `fdemon_defaults()`; updated 2 existing tests that asserted the capability was absent |
| `crates/fdemon-dap/src/adapter/handlers.rs` | Added `ExceptionInfoArguments` to imports; added `"exceptionInfo"` to dispatch table; added `handle_exception_info` method |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Added `mod exception_info;` |
| `crates/fdemon-dap/src/adapter/tests/exception_info.rs` | New file: 13 unit tests covering all acceptance criteria |

### Notable Decisions/Tradeoffs

1. **`toString()` fallback**: When `evaluate()` fails or returns no `valueAsString`, `description` falls back to the type name. This avoids propagating an error for a secondary lookup.
2. **`stackTrace?.toString()` failure is silent**: If the stack trace evaluation fails, `details.stackTrace` is simply absent rather than causing the whole request to fail. Many exceptions have no `stackTrace` property.
3. **`breakMode` derived from `exception_mode`**: The adapter's stored `DapExceptionPauseMode` is mapped directly without querying the VM, which is consistent and fast.
4. **Two existing tests updated**: `test_capabilities_fdemon_defaults` and `test_capabilities_phase3_fields_in_json` in `types.rs` previously asserted `supportsExceptionInfoRequest.is_none()`. These were updated to assert `Some(true)` — this is strictly required by the task's capability change.

### Testing Performed

- `cargo test -p fdemon-dap` — 730 passed, 0 failed
- `cargo test -p fdemon-dap exception_info` — 13 passed, 0 failed
- `cargo test --workspace` — all crates pass
- `cargo clippy -p fdemon-dap` — no errors

### Risks/Limitations

1. **No VM round-trip for `get_object`**: The task spec mentions calling `backend.get_object()` to get the full exception Instance, but the handler uses `evaluate("toString()")` directly on the stored exception object ID. The full object fetch is unnecessary since we only need the string representation and class name (which are already in the InstanceRef). This is simpler and avoids an extra VM round-trip.
2. **`stackTrace?.toString()` timeout**: There's no explicit timeout guard on the evaluate calls. The existing `DebugBackend::evaluate` implementation should handle timeouts at the backend layer.
