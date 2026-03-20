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
