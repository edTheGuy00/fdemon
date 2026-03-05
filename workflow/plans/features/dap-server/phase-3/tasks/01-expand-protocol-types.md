## Task: Expand DAP Protocol Types for Core Debugging

**Objective**: Add all DAP protocol types needed for Phase 3 core debugging to `fdemon-dap/src/protocol/types.rs`. This includes response body types, event body types, request argument types, and expanded capability flags.

**Depends on**: None

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-dap/src/protocol/types.rs`: Add new types and expand existing ones

### Details

Phase 2 has minimal types — only `DapMessage`, `DapRequest`, `DapResponse`, `DapEvent`, `Capabilities`, and `InitializeRequestArguments`. Phase 3 requires a significantly larger type surface.

#### New Types to Add

**Thread and execution types:**
```rust
/// DAP Thread object (maps to a Dart isolate).
pub struct DapThread {
    pub id: i64,
    pub name: String,
}
```

**Stack frame types:**
```rust
/// DAP StackFrame returned in stackTrace responses.
pub struct DapStackFrame {
    pub id: i64,
    pub name: String,
    pub source: Option<DapSource>,
    pub line: i64,
    pub column: i64,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
    pub presentation_hint: Option<String>, // "normal", "label", "subtle"
}

/// DAP Source object identifying a source file.
pub struct DapSource {
    pub name: Option<String>,
    pub path: Option<String>,
    pub source_reference: Option<i64>,
    pub presentation_hint: Option<String>, // "normal", "emphasize", "deemphasize"
}
```

**Scope and variable types:**
```rust
/// DAP Scope returned in scopes responses.
pub struct DapScope {
    pub name: String,
    pub presentation_hint: Option<String>, // "arguments", "locals", "registers"
    pub variables_reference: i64,
    pub named_variables: Option<i64>,
    pub indexed_variables: Option<i64>,
    pub expensive: bool,
}

/// DAP Variable returned in variables responses.
pub struct DapVariable {
    pub name: String,
    pub value: String,
    pub type_field: Option<String>,       // serde rename to "type"
    pub variables_reference: i64,
    pub named_variables: Option<i64>,
    pub indexed_variables: Option<i64>,
    pub evaluate_name: Option<String>,
    pub presentation_hint: Option<DapVariablePresentationHint>,
}

/// Presentation hints for variables.
pub struct DapVariablePresentationHint {
    pub kind: Option<String>,
    pub attributes: Option<Vec<String>>,
    pub visibility: Option<String>,
}
```

**Breakpoint types:**
```rust
/// DAP Breakpoint returned in setBreakpoints responses.
pub struct DapBreakpoint {
    pub id: Option<i64>,
    pub verified: bool,
    pub message: Option<String>,
    pub source: Option<DapSource>,
    pub line: Option<i64>,
    pub column: Option<i64>,
    pub end_line: Option<i64>,
    pub end_column: Option<i64>,
}

/// Source breakpoint from client's setBreakpoints request.
pub struct SourceBreakpoint {
    pub line: i64,
    pub column: Option<i64>,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
    pub log_message: Option<String>,
}

/// Arguments for setBreakpoints request.
pub struct SetBreakpointsArguments {
    pub source: DapSource,
    pub breakpoints: Option<Vec<SourceBreakpoint>>,
    pub source_modified: Option<bool>,
}

/// Arguments for setExceptionBreakpoints request.
pub struct SetExceptionBreakpointsArguments {
    pub filters: Vec<String>,
    pub filter_options: Option<Vec<ExceptionFilterOptions>>,
}

/// Per-filter options for exception breakpoints.
pub struct ExceptionFilterOptions {
    pub filter_id: String,
    pub condition: Option<String>,
}

/// Exception breakpoint filter advertised in Capabilities.
pub struct ExceptionBreakpointsFilter {
    pub filter: String,
    pub label: String,
    pub description: Option<String>,
    pub default: Option<bool>,
    pub supports_condition: Option<bool>,
    pub condition_description: Option<String>,
}
```

**Evaluate types:**
```rust
/// Arguments for evaluate request.
pub struct EvaluateArguments {
    pub expression: String,
    pub frame_id: Option<i64>,
    pub context: Option<String>, // "watch", "repl", "hover", "clipboard"
}

/// Response body for evaluate request.
pub struct EvaluateResponseBody {
    pub result: String,
    pub type_field: Option<String>,       // serde rename to "type"
    pub variables_reference: i64,
    pub named_variables: Option<i64>,
    pub indexed_variables: Option<i64>,
    pub presentation_hint: Option<DapVariablePresentationHint>,
}
```

**Request argument types:**
```rust
/// Arguments for stackTrace request.
pub struct StackTraceArguments {
    pub thread_id: i64,
    pub start_frame: Option<i64>,
    pub levels: Option<i64>,
}

/// Arguments for scopes request.
pub struct ScopesArguments {
    pub frame_id: i64,
}

/// Arguments for variables request.
pub struct VariablesArguments {
    pub variables_reference: i64,
    pub filter: Option<String>, // "indexed", "named"
    pub start: Option<i64>,
    pub count: Option<i64>,
}

/// Arguments for continue request.
pub struct ContinueArguments {
    pub thread_id: i64,
    pub single_thread: Option<bool>,
}

/// Arguments for next/stepIn/stepOut requests.
pub struct StepArguments {
    pub thread_id: i64,
    pub single_thread: Option<bool>,
    pub granularity: Option<String>, // "statement", "line", "instruction"
}

/// Arguments for pause request.
pub struct PauseArguments {
    pub thread_id: i64,
}

/// Arguments for threads request (no arguments needed).

/// Arguments for attach request.
pub struct AttachRequestArguments {
    pub vm_service_uri: Option<String>,
    pub session_id: Option<String>,
}

/// Arguments for disconnect request.
pub struct DisconnectArguments {
    pub restart: Option<bool>,
    pub terminate_debuggee: Option<bool>,
    pub suspend_debuggee: Option<bool>,
}
```

#### Expand `DapEvent` Constructors

Add convenience constructors to `DapEvent`:

```rust
impl DapEvent {
    pub fn stopped(reason: &str, thread_id: i64, description: Option<&str>) -> Self { ... }
    pub fn continued(thread_id: i64, all_threads_continued: bool) -> Self { ... }
    pub fn thread(reason: &str, thread_id: i64) -> Self { ... }
    pub fn breakpoint(reason: &str, breakpoint: &DapBreakpoint) -> Self { ... }
    pub fn exited(exit_code: i64) -> Self { ... }
}
```

#### Expand `Capabilities`

Add new fields to the `Capabilities` struct:

```rust
pub struct Capabilities {
    // Existing:
    pub supports_configuration_done_request: Option<bool>,
    // ... existing fields ...

    // New Phase 3 fields:
    pub supports_conditional_breakpoints: Option<bool>,
    pub supports_hit_conditional_breakpoints: Option<bool>,
    pub supports_log_points: Option<bool>,  // already exists
    pub supports_evaluate_for_hovers: Option<bool>,  // already exists
    pub supports_terminate_request: Option<bool>,
    pub supports_restart_request: Option<bool>,
    pub supports_delayed_stack_trace_loading: Option<bool>,  // already exists
    pub supports_exception_info_request: Option<bool>,  // already exists
    pub exception_breakpoint_filters: Option<Vec<ExceptionBreakpointsFilter>>,
}
```

Update `Capabilities::fdemon_defaults()`:

```rust
pub fn fdemon_defaults() -> Self {
    Self {
        supports_configuration_done_request: Some(true),
        supports_conditional_breakpoints: Some(true),
        supports_hit_conditional_breakpoints: Some(true),
        supports_evaluate_for_hovers: Some(true),
        supports_log_points: Some(true),
        supports_terminate_request: Some(true),
        supports_delayed_stack_trace_loading: Some(true),
        exception_breakpoint_filters: Some(vec![
            ExceptionBreakpointsFilter {
                filter: "All".into(),
                label: "All Exceptions".into(),
                description: Some("Break on all thrown exceptions".into()),
                default: Some(false),
                supports_condition: Some(false),
                condition_description: None,
            },
            ExceptionBreakpointsFilter {
                filter: "Unhandled".into(),
                label: "Uncaught Exceptions".into(),
                description: Some("Break on exceptions not caught by application code".into()),
                default: Some(true),
                supports_condition: Some(false),
                condition_description: None,
            },
        ]),
        ..Default::default()
    }
}
```

#### Serde Considerations

- All types use `#[serde(rename_all = "camelCase")]`
- Fields named `type` in the DAP spec must use `#[serde(rename = "type")]`
- Optional fields use `#[serde(default, skip_serializing_if = "Option::is_none")]`
- `DapSource.path` must be a filesystem path, not a `file://` URI (Helix sends `pathFormat: "path"`)

### Acceptance Criteria

1. All types listed above are defined with correct serde annotations
2. `DapEvent` has convenience constructors for `stopped`, `continued`, `thread`, `breakpoint`, `exited`
3. `Capabilities::fdemon_defaults()` declares all Phase 3 capabilities with exception breakpoint filters
4. All types round-trip through serde (serialize → deserialize → equal)
5. Existing Phase 2 tests continue to pass
6. New tests cover serialization format compliance with DAP spec (camelCase fields, correct `type` renames)

### Testing

```rust
#[test]
fn test_stopped_event_serialization() {
    let event = DapEvent::stopped("breakpoint", 1, None);
    let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
    assert_eq!(json["body"]["reason"], "breakpoint");
    assert_eq!(json["body"]["threadId"], 1);
}

#[test]
fn test_dap_variable_type_field_rename() {
    let var = DapVariable { type_field: Some("String".into()), ..Default::default() };
    let json = serde_json::to_value(&var).unwrap();
    assert!(json.get("type").is_some());
    assert!(json.get("typeField").is_none());
}

#[test]
fn test_capabilities_exception_filters() {
    let caps = Capabilities::fdemon_defaults();
    let json = serde_json::to_value(&caps).unwrap();
    let filters = json["exceptionBreakpointFilters"].as_array().unwrap();
    assert_eq!(filters.len(), 2);
    assert_eq!(filters[0]["filter"], "All");
    assert_eq!(filters[1]["filter"], "Unhandled");
}

#[test]
fn test_source_breakpoint_with_condition() {
    let bp = SourceBreakpoint { line: 42, condition: Some("x > 5".into()), ..Default::default() };
    let json = serde_json::to_value(&bp).unwrap();
    assert_eq!(json["line"], 42);
    assert_eq!(json["condition"], "x > 5");
}
```

### Notes

- Keep types in a single `types.rs` file — it's large but easier to discover than splitting across many small files
- Use `Option<T>` for all non-required DAP fields — clients may omit them
- Do NOT add types for Phase 4 features (custom requests, data breakpoints, function breakpoints) yet
- The `DapSource.source_reference` field will be used for SDK/package sources in Phase 4; for now, always set to `None` or `0`
- Match the exact field names from the [DAP specification](https://microsoft.github.io/debug-adapter-protocol/specification)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/protocol/types.rs` | Added 22 new types, 5 new `DapEvent` constructors, expanded `Capabilities` with 5 new fields and updated `fdemon_defaults()`, updated `test_capabilities_fdemon_defaults` test to reflect new defaults, added 32 new unit tests |

### Notable Decisions/Tradeoffs

1. **`type_field` rename pattern**: Both `DapVariable` and `EvaluateResponseBody` use `#[serde(rename = "type")]` on the `type_field` member to correctly wire to the DAP spec's `"type"` field while avoiding the Rust keyword. This matches the existing `InitializeRequestArguments` pattern with `clientID`/`adapterID`.

2. **`DapEvent::stopped` always sets `allThreadsStopped: true`**: The DAP spec says the adapter should set this when all threads are stopped (which is typical for Dart/Flutter on a pause event). This can be revisited when per-thread step semantics are needed.

3. **`DapEvent::breakpoint` uses `serde_json::to_value`**: The breakpoint event constructor serializes the `DapBreakpoint` inline via `serde_json::to_value`. If serialization fails (which should never happen for well-formed types), it falls back to `null`. This keeps the API ergonomic without propagating `Result`.

4. **Existing `test_capabilities_fdemon_defaults` updated**: The Phase 2 test asserted that most capabilities were `None`. With Phase 3 defaults now set, those assertions were updated to reflect the new enabled capabilities. The test still asserts that unimplemented capabilities remain `None`.

5. **`DapSource` derives `Default`**: Added `Default` derive to `DapSource` to support the `..Default::default()` spread pattern used throughout tests (and needed by downstream adapter code in later tasks).

6. **Types kept in single `types.rs` file**: Per task notes, all types are in one file rather than split into sub-modules. The file is ~600 lines of source + ~400 lines of tests, which is large but easily discoverable.

### Testing Performed

- `cargo check -p fdemon-dap` - Passed
- `cargo test -p fdemon-dap` - Passed (109 tests: 66 in types, 14 in codec, 29 in server/session/service)
- `cargo clippy -p fdemon-dap -- -D warnings` - Passed (zero warnings)
- `cargo fmt -p fdemon-dap` - Applied (minor line wrap adjustments)
- `cargo test --workspace --lib` - Passed (796 tests total, no regressions)

### Risks/Limitations

1. **`supports_restart_request` not set in `fdemon_defaults()`**: The task spec lists it as a new field but the `fdemon_defaults()` example in the spec omits it. It is defined on `Capabilities` but left `None` since the restart handler is not yet implemented. The test `test_capabilities_phase3_fields_in_json` explicitly asserts it is absent.

2. **`DapSource.source_reference`**: Set to `None` in all Phase 3 usage per task notes. Phase 4 will use it for SDK/package source references — the field is already modelled but not populated.
