## Task: Add VM Service Debug Type Definitions

**Objective**: Define all Rust types needed for VM Service debugging RPCs and debug stream events. These types will be consumed by the RPC wrappers (task 03), event parsing (task 02), and session debug state (task 04).

**Depends on**: None

### Scope

- `crates/fdemon-daemon/src/vm_service/debugger_types.rs` — **NEW FILE**: All debug-related type definitions
- `crates/fdemon-daemon/src/vm_service/mod.rs` — Add `pub mod debugger_types;` and re-exports

### Details

Create `debugger_types.rs` with serde-deserializable types matching the Dart VM Service Protocol v4.20+. All types use `#[serde(rename_all = "camelCase")]` to match the JSON-RPC wire format.

#### Types to define

**Source location types:**

```rust
/// Reference to a Dart script (lightweight, used in source locations).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRef {
    pub id: String,
    pub uri: String,
}

/// A resolved source code location within a script.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub script: ScriptRef,
    pub token_pos: i64,
    pub line: Option<i32>,
    pub column: Option<i32>,
}
```

**Breakpoint types:**

```rust
/// A VM Service breakpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Breakpoint {
    pub id: String,
    pub breakpoint_number: i32,
    pub enabled: bool,
    pub resolved: bool,
    /// SourceLocation when resolved, UnresolvedSourceLocation otherwise.
    /// Use Value for flexibility since the shape differs.
    pub location: Option<serde_json::Value>,
}
```

**Stack and frame types:**

```rust
/// The kind of a stack frame.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum FrameKind {
    Regular,
    AsyncCausal,
    AsyncSuspensionMarker,
}

/// A reference to a Dart function (lightweight).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionRef {
    pub id: String,
    pub name: String,
}

/// A single stack frame from getStack().
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub index: i32,
    pub function: Option<FunctionRef>,
    pub location: Option<SourceLocation>,
    pub vars: Option<Vec<BoundVariable>>,
    pub kind: Option<FrameKind>,
}

/// A variable bound in a stack frame scope.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundVariable {
    pub name: String,
    pub value: InstanceRef,
}

/// Reference to a Dart object instance (lightweight).
/// For full object details, call getObject() with the id.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRef {
    pub id: Option<String>,
    pub kind: String,
    pub class_ref: Option<ClassRef>,
    pub value_as_string: Option<String>,
    pub value_as_string_is_truncated: Option<bool>,
    pub length: Option<i64>,
}

/// Reference to a Dart class (lightweight).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassRef {
    pub id: String,
    pub name: String,
}
```

**Stack response:**

```rust
/// Response from getStack() RPC.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stack {
    pub frames: Vec<Frame>,
    pub async_causal_frames: Option<Vec<Frame>>,
    pub awaiter_frames: Option<Vec<Frame>>,
    pub truncated: Option<bool>,
}
```

**Script list:**

```rust
/// Response from getScripts() RPC.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptList {
    pub scripts: Vec<ScriptRef>,
}
```

**Step and exception mode enums:**

```rust
/// Step options for the resume() RPC.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum StepOption {
    Into,
    Over,
    Out,
    OverAsyncSuspension,
}

impl StepOption {
    /// Returns the wire-format string for the VM Service protocol.
    pub fn as_str(&self) -> &'static str {
        match self {
            StepOption::Into => "Into",
            StepOption::Over => "Over",
            StepOption::Out => "Out",
            StepOption::OverAsyncSuspension => "OverAsyncSuspension",
        }
    }
}

/// Exception pause mode for setIsolatePauseMode().
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExceptionPauseMode {
    None,
    #[default]
    Unhandled,
    All,
}

impl ExceptionPauseMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExceptionPauseMode::None => "None",
            ExceptionPauseMode::Unhandled => "Unhandled",
            ExceptionPauseMode::All => "All",
        }
    }
}
```

**Debug stream event types:**

```rust
/// Reference to an isolate (present on all debug/isolate events).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IsolateRef {
    pub id: String,
    pub name: Option<String>,
}

/// Parsed event from the VM Service Debug stream.
#[derive(Debug, Clone)]
pub enum DebugEvent {
    PauseStart {
        isolate: IsolateRef,
        top_frame: Option<Frame>,
    },
    PauseBreakpoint {
        isolate: IsolateRef,
        top_frame: Option<Frame>,
        breakpoint: Option<Breakpoint>,
        pause_breakpoints: Vec<Breakpoint>,
        at_async_suspension: bool,
    },
    PauseException {
        isolate: IsolateRef,
        top_frame: Option<Frame>,
        exception: Option<InstanceRef>,
    },
    PauseExit {
        isolate: IsolateRef,
        top_frame: Option<Frame>,
    },
    PauseInterrupted {
        isolate: IsolateRef,
        top_frame: Option<Frame>,
        at_async_suspension: bool,
    },
    PausePostRequest {
        isolate: IsolateRef,
        top_frame: Option<Frame>,
    },
    Resume {
        isolate: IsolateRef,
    },
    BreakpointAdded {
        isolate: IsolateRef,
        breakpoint: Breakpoint,
    },
    BreakpointResolved {
        isolate: IsolateRef,
        breakpoint: Breakpoint,
    },
    BreakpointRemoved {
        isolate: IsolateRef,
        breakpoint: Breakpoint,
    },
    BreakpointUpdated {
        isolate: IsolateRef,
        breakpoint: Breakpoint,
    },
    Inspect {
        isolate: IsolateRef,
        inspectee: InstanceRef,
    },
}

/// Parsed event from the VM Service Isolate stream.
#[derive(Debug, Clone)]
pub enum IsolateEvent {
    IsolateStart { isolate: IsolateRef },
    IsolateRunnable { isolate: IsolateRef },
    IsolateExit { isolate: IsolateRef },
    IsolateUpdate { isolate: IsolateRef },
    IsolateReload { isolate: IsolateRef },
    ServiceExtensionAdded { isolate: IsolateRef, extension_rpc: String },
}
```

**Parsing functions:**

```rust
/// Parse a Debug stream event from a raw VM Service StreamEvent.
/// Returns None for unrecognized event kinds.
pub fn parse_debug_event(kind: &str, data: &serde_json::Value) -> Option<DebugEvent> { ... }

/// Parse an Isolate stream event from a raw VM Service StreamEvent.
/// Returns None for unrecognized event kinds.
pub fn parse_isolate_event(kind: &str, data: &serde_json::Value) -> Option<IsolateEvent> { ... }
```

#### Module registration

In `crates/fdemon-daemon/src/vm_service/mod.rs`, add:
- `pub mod debugger_types;`
- Re-export key types: `DebugEvent`, `IsolateEvent`, `StepOption`, `ExceptionPauseMode`, `Breakpoint`, `Frame`, `Stack`, `InstanceRef`, `ScriptRef`, `SourceLocation`, `IsolateRef`

### Acceptance Criteria

1. All types compile and derive `Debug`, `Clone`
2. Serde types roundtrip correctly with example VM Service JSON (add JSON fixture tests)
3. `DebugEvent` covers all Debug stream event kinds listed in the Dart VM Service spec
4. `IsolateEvent` covers all Isolate stream event kinds
5. `parse_debug_event()` correctly parses fixture JSON for each event kind
6. `parse_isolate_event()` correctly parses fixture JSON for each event kind
7. Unrecognized event kinds return `None` (not errors)
8. `StepOption::as_str()` returns the exact wire-format strings
9. `ExceptionPauseMode::as_str()` returns the exact wire-format strings
10. `cargo clippy` clean, no warnings

### Testing

Write comprehensive unit tests using JSON fixtures derived from the Dart VM Service spec. Test each event kind, each type's deserialization, and edge cases (missing optional fields, unknown fields ignored).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_pause_breakpoint_event() {
        let data = json!({
            "type": "Event",
            "kind": "PauseBreakpoint",
            "isolate": { "type": "@Isolate", "id": "isolates/123", "name": "main" },
            "topFrame": {
                "type": "Frame",
                "index": 0,
                "function": { "type": "@Function", "id": "func/1", "name": "myFunc" },
                "location": {
                    "type": "SourceLocation",
                    "script": { "type": "@Script", "id": "scripts/1", "uri": "package:app/main.dart" },
                    "tokenPos": 100,
                    "line": 42,
                    "column": 5
                },
                "vars": []
            },
            "breakpoint": {
                "type": "Breakpoint",
                "id": "breakpoints/1",
                "breakpointNumber": 1,
                "enabled": true,
                "resolved": true
            },
            "pauseBreakpoints": [],
            "atAsyncSuspension": false
        });

        let event = parse_debug_event("PauseBreakpoint", &data).unwrap();
        assert!(matches!(event, DebugEvent::PauseBreakpoint { .. }));
    }

    #[test]
    fn test_parse_isolate_start_event() {
        let data = json!({
            "type": "Event",
            "kind": "IsolateStart",
            "isolate": { "type": "@Isolate", "id": "isolates/456", "name": "worker" }
        });

        let event = parse_isolate_event("IsolateStart", &data).unwrap();
        assert!(matches!(event, IsolateEvent::IsolateStart { .. }));
    }

    #[test]
    fn test_parse_unknown_debug_event_returns_none() {
        let data = json!({});
        assert!(parse_debug_event("UnknownEvent", &data).is_none());
    }

    #[test]
    fn test_step_option_as_str() {
        assert_eq!(StepOption::Into.as_str(), "Into");
        assert_eq!(StepOption::Over.as_str(), "Over");
        assert_eq!(StepOption::Out.as_str(), "Out");
        assert_eq!(StepOption::OverAsyncSuspension.as_str(), "OverAsyncSuspension");
    }

    #[test]
    fn test_exception_pause_mode_as_str() {
        assert_eq!(ExceptionPauseMode::None.as_str(), "None");
        assert_eq!(ExceptionPauseMode::Unhandled.as_str(), "Unhandled");
        assert_eq!(ExceptionPauseMode::All.as_str(), "All");
    }
}
```

### Notes

- Use `serde_json::Value` for `Breakpoint.location` since resolved vs unresolved source locations have different shapes. The DAP adapter (Phase 3) will handle the discrimination.
- `InstanceRef.id` is `Option<String>` because Sentinel values don't have IDs.
- The `Frame.vars` field is `Option<Vec<BoundVariable>>` — it's only populated when the isolate is paused.
- `FrameKind` needs a custom deserializer or `#[serde(rename_all = "PascalCase")]` since the VM Service sends PascalCase strings.
- All types use `serde_json` — no new crate dependencies needed for `fdemon-daemon`.

---

## Completion Summary

**Status:** Not started
