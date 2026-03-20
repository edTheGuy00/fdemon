## Task: Implement Exception Scope

**Objective**: Add an "Exceptions" scope that appears when the debugger is paused at an exception. The scope surfaces the exception object with its fields, allowing the user to inspect the exception details. Also support the `$_threadException` magic expression in evaluate.

**Depends on**: 01-fix-variable-display-bugs, 02-expand-backend-trait

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/variables.rs`: Add `ScopeKind::Exceptions` handling in `handle_scopes` and `get_scope_variables`
- `crates/fdemon-dap/src/adapter/stack.rs`: Add `ScopeKind::Exceptions` variant to `ScopeKind` enum; add `ExceptionRef` to `VariableRef` or use `Object` variant
- `crates/fdemon-dap/src/adapter/events.rs`: Store exception `InstanceRef` on pause at exception; add `exception_reference` field tracking

**Files Read (Dependencies):**
- `crates/fdemon-dap/src/adapter/backend.rs`: `get_object` for exception field expansion
- `crates/fdemon-dap/src/adapter/mod.rs`: `DapAdapter` struct fields

### Details

#### 1. Add `ScopeKind::Exceptions` variant

```rust
// In stack.rs:
pub enum ScopeKind {
    Locals,
    Globals,
    Exceptions,
}
```

#### 2. Track exception reference per thread

Add a field to track the current exception when paused:

```rust
// In DapAdapter or a new per-thread state:
/// Exception InstanceRef stored when paused at PauseException.
/// Cleared on resume. Keyed by thread (isolate) ID.
pub exception_refs: HashMap<i64, ExceptionRef>,

pub struct ExceptionRef {
    pub isolate_id: String,
    pub instance_ref: serde_json::Value,  // The raw InstanceRef JSON
}
```

#### 3. Store exception on pause

In `events.rs`, in the `Paused` event handler (~line 82), when `pause_reason` is `Exception`:
- Extract the `exception` field from the debug event (it's an `InstanceRef`)
- Store it in `exception_refs` keyed by the DAP thread ID
- The exception field is in `event.exception` or similar — check the `DebugEvent::Paused` struct

#### 4. Clear exception on resume

In `on_resume()` at `events.rs:519-522`, clear the exception ref for the resumed thread.

#### 5. Conditionally add Exceptions scope in `handle_scopes`

```rust
// In handle_scopes, after creating Locals and Globals scopes:
let thread_id = /* look up from frame_ref */;
let mut scopes = vec![locals_scope, globals_scope];

if self.exception_refs.contains_key(&thread_id) {
    let exc_ref = self.var_store.allocate(VariableRef::Scope {
        frame_index,
        scope_kind: ScopeKind::Exceptions,
    });
    scopes.push(DapScope {
        name: "Exceptions".to_string(),
        variables_reference: exc_ref,
        expensive: false,
        presentation_hint: Some("locals".to_string()),
        ..Default::default()
    });
}
```

#### 6. Return exception variable in `get_scope_variables`

```rust
ScopeKind::Exceptions => {
    let thread_id = /* derive from frame_index */;
    if let Some(exc) = self.exception_refs.get(&thread_id) {
        let class_name = exc.instance_ref
            .get("classRef").or_else(|| exc.instance_ref.get("class"))
            .and_then(|c| c.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("Exception");
        let var = self.instance_ref_to_variable(class_name, &exc.instance_ref, &exc.isolate_id);
        Ok(vec![var])
    } else {
        Ok(Vec::new())
    }
}
```

#### 7. Support `$_threadException` in evaluate

In `evaluate.rs`, in `handle_evaluate`, before calling `evaluate_expression`:
- Check if `expression == "$_threadException"`
- If so, look up the exception ref for the current thread
- Return the exception's `InstanceRef` directly (with `variablesReference > 0` for expansion)
- This allows watch expressions like `$_threadException.message`

### Acceptance Criteria

1. "Exceptions" scope appears in scopes response when paused at an exception
2. "Exceptions" scope is absent when paused at a breakpoint or step
3. Exception scope contains a single variable named by the exception class (e.g., "FormatException")
4. Exception variable is expandable — shows exception fields (message, stackTrace, etc.)
5. Exception ref is cleared on resume
6. `$_threadException` evaluate expression returns the current exception
7. 10+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_scopes_include_exceptions_on_pause_exception() {
    // Simulate PauseException event with exception InstanceRef
    // Call handle_scopes
    // Verify 3 scopes returned: Locals, Globals, Exceptions
}

#[tokio::test]
async fn test_scopes_no_exceptions_on_pause_breakpoint() {
    // Simulate PauseBreakpoint event (no exception)
    // Call handle_scopes
    // Verify 2 scopes returned: Locals, Globals
}

#[tokio::test]
async fn test_exception_scope_returns_exception_variable() {
    // Expand Exceptions scope
    // Verify single variable with exception class name
    // Verify variablesReference > 0 for field expansion
}

#[tokio::test]
async fn test_exception_cleared_on_resume() {
    // Store exception, then call on_resume
    // Verify exception_refs is empty
}
```

### Notes

- The `DebugEvent::Paused` struct must carry the `exception` field. Check if it's already there — if not, the event parsing in `debugger_types.rs` may need to be extended to extract `event.exception`.
- The Dart VM Service returns the exception as an `InstanceRef` in the `PauseException` event's `exception` field. The `PauseBreakpoint` event does not have this field.
