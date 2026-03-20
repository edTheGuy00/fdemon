//! VM Service debug type definitions.
//!
//! This module contains serde-deserializable types matching the Dart VM Service
//! Protocol v4.20+ for debugging RPCs and debug stream events. All types use
//! `#[serde(rename_all = "camelCase")]` to match the JSON-RPC wire format.
//!
//! ## Usage
//!
//! These types are consumed by the debug RPC wrappers (task 03), debug event
//! parsing (task 02), and per-session debug state (task 04).
//!
//! ## References
//!
//! - Dart VM Service Protocol:
//!   <https://github.com/dart-lang/sdk/blob/main/runtime/vm/service/service.md>

use serde::{Deserialize, Serialize};

use super::protocol::StreamEvent;

// ---------------------------------------------------------------------------
// Source location types
// ---------------------------------------------------------------------------

/// Reference to a Dart script (lightweight, used in source locations).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRef {
    /// Unique script ID (e.g. `"scripts/1"`).
    pub id: String,
    /// URI of the script (e.g. `"package:app/main.dart"`).
    pub uri: String,
}

/// A resolved source code location within a script.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    /// The script containing this location.
    pub script: ScriptRef,
    /// Token position within the script (byte offset into the token stream).
    pub token_pos: i64,
    /// Human-readable 1-based line number, if available.
    pub line: Option<i32>,
    /// Human-readable 1-based column number, if available.
    pub column: Option<i32>,
}

// ---------------------------------------------------------------------------
// Breakpoint types
// ---------------------------------------------------------------------------

/// A VM Service breakpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Breakpoint {
    /// Unique breakpoint ID (e.g. `"breakpoints/1"`).
    pub id: String,
    /// Human-readable breakpoint number shown to the user.
    pub breakpoint_number: i32,
    /// Whether the breakpoint is currently enabled.
    pub enabled: bool,
    /// Whether the breakpoint has been resolved to a source location.
    pub resolved: bool,
    /// Source location of the breakpoint.
    ///
    /// Uses `serde_json::Value` for flexibility because the shape differs
    /// between a resolved `SourceLocation` and an `UnresolvedSourceLocation`.
    /// The DAP adapter (Phase 3) handles discrimination between the two.
    pub location: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Stack and frame types
// ---------------------------------------------------------------------------

/// The kind of a stack frame.
///
/// The VM Service sends these as PascalCase strings, so we use a custom
/// deserializer to handle that casing.
#[derive(Debug, Clone, Serialize)]
pub enum FrameKind {
    /// A regular synchronous Dart frame.
    Regular,
    /// An async causal frame (reconstructed async stack).
    AsyncCausal,
    /// An async suspension marker between causal frame groups.
    AsyncSuspensionMarker,
}

impl<'de> Deserialize<'de> for FrameKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "Regular" => Ok(FrameKind::Regular),
            "AsyncCausal" => Ok(FrameKind::AsyncCausal),
            "AsyncSuspensionMarker" => Ok(FrameKind::AsyncSuspensionMarker),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["Regular", "AsyncCausal", "AsyncSuspensionMarker"],
            )),
        }
    }
}

/// A reference to a Dart function (lightweight).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionRef {
    /// Unique function ID.
    pub id: String,
    /// Human-readable function name.
    pub name: String,
}

/// A single stack frame from `getStack()`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    /// 0-based index of this frame in the stack.
    pub index: i32,
    /// The function executing in this frame, if available.
    pub function: Option<FunctionRef>,
    /// The source location of the current execution point, if available.
    pub location: Option<SourceLocation>,
    /// Variables bound in this frame scope.
    ///
    /// Only populated when the isolate is paused.
    pub vars: Option<Vec<BoundVariable>>,
    /// The kind of frame (regular, async causal, or suspension marker).
    pub kind: Option<FrameKind>,
}

/// A variable bound in a stack frame scope.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundVariable {
    /// Variable name as it appears in source.
    pub name: String,
    /// The current value of the variable.
    pub value: InstanceRef,
}

/// Reference to a Dart object instance (lightweight).
///
/// For full object details, call `getObject()` with the `id`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRef {
    /// Unique instance ID.
    ///
    /// `None` for Sentinel values, which don't have IDs.
    pub id: Option<String>,
    /// The instance kind (e.g. `"String"`, `"Int"`, `"List"`).
    pub kind: String,
    /// Reference to the class of this instance, if available.
    pub class_ref: Option<ClassRef>,
    /// A string representation of the value, if available.
    pub value_as_string: Option<String>,
    /// Whether `value_as_string` is truncated due to length limits.
    pub value_as_string_is_truncated: Option<bool>,
    /// Length for List/Map/String instances, if applicable.
    pub length: Option<i64>,
}

/// Reference to a Dart class (lightweight).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassRef {
    /// Unique class ID.
    pub id: String,
    /// Human-readable class name.
    pub name: String,
}

// ---------------------------------------------------------------------------
// Stack response
// ---------------------------------------------------------------------------

/// Response from the `getStack()` RPC.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stack {
    /// The synchronous stack frames, ordered from top (most recent) to bottom.
    pub frames: Vec<Frame>,
    /// Async causal frames, if the isolate is paused within async code.
    pub async_causal_frames: Option<Vec<Frame>>,
    /// Awaiter frames, if available (Dart 2.17+).
    pub awaiter_frames: Option<Vec<Frame>>,
    /// Whether the stack was truncated due to its depth.
    pub truncated: Option<bool>,
}

// ---------------------------------------------------------------------------
// Script list
// ---------------------------------------------------------------------------

/// Response from the `getScripts()` RPC.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptList {
    /// All scripts loaded in the isolate.
    pub scripts: Vec<ScriptRef>,
}

// ---------------------------------------------------------------------------
// Step and exception mode enums
// ---------------------------------------------------------------------------

/// Step options for the `resume()` RPC.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum StepOption {
    /// Step into function calls.
    Into,
    /// Step over the current line.
    Over,
    /// Step out of the current function.
    Out,
    /// Step over async suspension points.
    OverAsyncSuspension,
    /// Rewind to the start of the selected frame (`restartFrame`).
    ///
    /// Only valid for synchronous frames below the first async suspension
    /// marker. The VM will re-execute the frame from its entry point.
    Rewind,
}

impl StepOption {
    /// Returns the wire-format string for the VM Service protocol.
    pub fn as_str(&self) -> &'static str {
        match self {
            StepOption::Into => "Into",
            StepOption::Over => "Over",
            StepOption::Out => "Out",
            StepOption::OverAsyncSuspension => "OverAsyncSuspension",
            StepOption::Rewind => "Rewind",
        }
    }
}

/// Exception pause mode for `setIsolatePauseMode()`.
///
/// Controls when the debugger pauses on thrown exceptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExceptionPauseMode {
    /// Never pause on exceptions.
    None,
    /// Pause only on unhandled exceptions (default).
    #[default]
    Unhandled,
    /// Pause on all thrown exceptions, including handled ones.
    All,
}

impl ExceptionPauseMode {
    /// Returns the wire-format string for the VM Service protocol.
    pub fn as_str(&self) -> &'static str {
        match self {
            ExceptionPauseMode::None => "None",
            ExceptionPauseMode::Unhandled => "Unhandled",
            ExceptionPauseMode::All => "All",
        }
    }
}

// ---------------------------------------------------------------------------
// Debug stream event types
// ---------------------------------------------------------------------------

/// Reference to an isolate (present on all debug and isolate stream events).
///
/// This type differs from `protocol::IsolateRef` in that `name` is optional,
/// which matches the `@Isolate` shape used in debug/isolate stream events.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IsolateRef {
    /// Unique isolate ID (e.g. `"isolates/1234"`).
    pub id: String,
    /// Human-readable isolate name, if provided.
    pub name: Option<String>,
}

/// Parsed event from the VM Service Debug stream.
///
/// Each variant corresponds to a `kind` value in the Debug stream.
/// See the Dart VM Service Protocol spec for full semantics.
#[derive(Debug, Clone)]
pub enum DebugEvent {
    /// The isolate has paused at program start (before executing any code).
    PauseStart {
        /// The isolate that paused.
        isolate: IsolateRef,
        /// The top-most frame at the pause point, if available.
        top_frame: Option<Frame>,
    },
    /// The isolate has paused at a breakpoint.
    PauseBreakpoint {
        /// The isolate that paused.
        isolate: IsolateRef,
        /// The top-most frame at the pause point.
        top_frame: Option<Frame>,
        /// The breakpoint that triggered the pause, if any.
        breakpoint: Option<Breakpoint>,
        /// All breakpoints that triggered simultaneously.
        pause_breakpoints: Vec<Breakpoint>,
        /// Whether the pause occurred at an async suspension point.
        at_async_suspension: bool,
    },
    /// The isolate has paused due to an exception being thrown.
    PauseException {
        /// The isolate that paused.
        isolate: IsolateRef,
        /// The top-most frame at the pause point.
        top_frame: Option<Frame>,
        /// The exception object that was thrown.
        exception: Option<InstanceRef>,
    },
    /// The isolate has paused just before it exits.
    PauseExit {
        /// The isolate that paused.
        isolate: IsolateRef,
        /// The top-most frame at the pause point.
        top_frame: Option<Frame>,
    },
    /// The isolate has paused due to an interrupt request.
    PauseInterrupted {
        /// The isolate that paused.
        isolate: IsolateRef,
        /// The top-most frame at the pause point.
        top_frame: Option<Frame>,
        /// Whether the pause occurred at an async suspension point.
        at_async_suspension: bool,
    },
    /// The isolate has paused after a hot-reload (post-request pause).
    PausePostRequest {
        /// The isolate that paused.
        isolate: IsolateRef,
        /// The top-most frame at the pause point.
        top_frame: Option<Frame>,
    },
    /// The isolate has resumed execution.
    Resume {
        /// The isolate that resumed.
        isolate: IsolateRef,
    },
    /// A new breakpoint was added.
    BreakpointAdded {
        /// The isolate the breakpoint belongs to.
        isolate: IsolateRef,
        /// The newly added breakpoint.
        breakpoint: Breakpoint,
    },
    /// A breakpoint was resolved to a source location.
    BreakpointResolved {
        /// The isolate the breakpoint belongs to.
        isolate: IsolateRef,
        /// The resolved breakpoint.
        breakpoint: Breakpoint,
    },
    /// A breakpoint was removed.
    BreakpointRemoved {
        /// The isolate the breakpoint was removed from.
        isolate: IsolateRef,
        /// The removed breakpoint.
        breakpoint: Breakpoint,
    },
    /// A breakpoint was updated (e.g. enabled/disabled).
    BreakpointUpdated {
        /// The isolate the breakpoint belongs to.
        isolate: IsolateRef,
        /// The updated breakpoint.
        breakpoint: Breakpoint,
    },
    /// An object was inspected via the Inspector API.
    Inspect {
        /// The isolate the inspectee belongs to.
        isolate: IsolateRef,
        /// The object being inspected.
        inspectee: InstanceRef,
    },
}

/// Parsed event from the VM Service Isolate stream.
///
/// Each variant corresponds to a `kind` value in the Isolate stream.
#[derive(Debug, Clone)]
pub enum IsolateEvent {
    /// A new isolate has started.
    IsolateStart {
        /// The newly started isolate.
        isolate: IsolateRef,
    },
    /// An isolate is now runnable (i.e. ready to execute).
    IsolateRunnable {
        /// The isolate that became runnable.
        isolate: IsolateRef,
    },
    /// An isolate has exited.
    IsolateExit {
        /// The isolate that exited.
        isolate: IsolateRef,
    },
    /// An isolate's metadata has been updated (e.g. its name changed).
    IsolateUpdate {
        /// The updated isolate.
        isolate: IsolateRef,
    },
    /// A hot-reload was performed on an isolate.
    IsolateReload {
        /// The reloaded isolate.
        isolate: IsolateRef,
    },
    /// A Flutter service extension was registered by an isolate.
    ServiceExtensionAdded {
        /// The isolate that registered the extension.
        isolate: IsolateRef,
        /// The RPC name of the registered extension (e.g. `"ext.flutter.reassemble"`).
        extension_rpc: String,
    },
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse a Debug stream event from a typed VM Service stream event.
///
/// # Arguments
///
/// * `event` — A reference to the deserialized [`StreamEvent`]. The `isolate`
///   field is read from the typed `event.isolate`, and kind-specific fields
///   (e.g. `topFrame`, `breakpoint`) are read from the flattened `event.data`.
///
/// # Returns
///
/// `Some(DebugEvent)` for recognized event kinds, `None` for unrecognized kinds
/// or when the required `isolate` field is absent.
pub fn parse_debug_event(event: &StreamEvent) -> Option<DebugEvent> {
    let isolate = event.isolate.as_ref().map(|iso| IsolateRef {
        id: iso.id.clone(),
        name: Some(iso.name.clone()),
    })?;

    match event.kind.as_str() {
        "PauseStart" => Some(DebugEvent::PauseStart {
            isolate,
            top_frame: parse_top_frame(&event.data),
        }),
        "PauseBreakpoint" => Some(DebugEvent::PauseBreakpoint {
            isolate,
            top_frame: parse_top_frame(&event.data),
            breakpoint: parse_breakpoint_field(&event.data, "breakpoint"),
            pause_breakpoints: parse_breakpoint_array(&event.data, "pauseBreakpoints"),
            at_async_suspension: event
                .data
                .get("atAsyncSuspension")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }),
        "PauseException" => Some(DebugEvent::PauseException {
            isolate,
            top_frame: parse_top_frame(&event.data),
            exception: parse_instance_ref_field(&event.data, "exception"),
        }),
        "PauseExit" => Some(DebugEvent::PauseExit {
            isolate,
            top_frame: parse_top_frame(&event.data),
        }),
        "PauseInterrupted" => Some(DebugEvent::PauseInterrupted {
            isolate,
            top_frame: parse_top_frame(&event.data),
            at_async_suspension: event
                .data
                .get("atAsyncSuspension")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }),
        "PausePostRequest" => Some(DebugEvent::PausePostRequest {
            isolate,
            top_frame: parse_top_frame(&event.data),
        }),
        "Resume" => Some(DebugEvent::Resume { isolate }),
        "BreakpointAdded" => {
            let breakpoint = parse_breakpoint_field(&event.data, "breakpoint")?;
            Some(DebugEvent::BreakpointAdded {
                isolate,
                breakpoint,
            })
        }
        "BreakpointResolved" => {
            let breakpoint = parse_breakpoint_field(&event.data, "breakpoint")?;
            Some(DebugEvent::BreakpointResolved {
                isolate,
                breakpoint,
            })
        }
        "BreakpointRemoved" => {
            let breakpoint = parse_breakpoint_field(&event.data, "breakpoint")?;
            Some(DebugEvent::BreakpointRemoved {
                isolate,
                breakpoint,
            })
        }
        "BreakpointUpdated" => {
            let breakpoint = parse_breakpoint_field(&event.data, "breakpoint")?;
            Some(DebugEvent::BreakpointUpdated {
                isolate,
                breakpoint,
            })
        }
        "Inspect" => {
            let inspectee = parse_instance_ref_field(&event.data, "inspectee")?;
            Some(DebugEvent::Inspect { isolate, inspectee })
        }
        _ => None,
    }
}

/// Parse an Isolate stream event from a typed VM Service stream event.
///
/// # Arguments
///
/// * `event` — A reference to the deserialized [`StreamEvent`]. The `isolate`
///   field is read from the typed `event.isolate`, and kind-specific fields
///   (e.g. `extensionRPC`) are read from the flattened `event.data`.
///
/// # Returns
///
/// `Some(IsolateEvent)` for recognized event kinds, `None` for unrecognized kinds
/// or when the required `isolate` field is absent.
pub fn parse_isolate_event(event: &StreamEvent) -> Option<IsolateEvent> {
    let isolate = event.isolate.as_ref().map(|iso| IsolateRef {
        id: iso.id.clone(),
        name: Some(iso.name.clone()),
    })?;

    match event.kind.as_str() {
        "IsolateStart" => Some(IsolateEvent::IsolateStart { isolate }),
        "IsolateRunnable" => Some(IsolateEvent::IsolateRunnable { isolate }),
        "IsolateExit" => Some(IsolateEvent::IsolateExit { isolate }),
        "IsolateUpdate" => Some(IsolateEvent::IsolateUpdate { isolate }),
        "IsolateReload" => Some(IsolateEvent::IsolateReload { isolate }),
        "ServiceExtensionAdded" => {
            let extension_rpc = event
                .data
                .get("extensionRPC")
                .and_then(|v| v.as_str())
                .map(str::to_owned)?;
            Some(IsolateEvent::ServiceExtensionAdded {
                isolate,
                extension_rpc,
            })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Internal parsing helpers
// ---------------------------------------------------------------------------

/// Extract a `Frame` from the `"topFrame"` field of an event JSON object.
fn parse_top_frame(data: &serde_json::Value) -> Option<Frame> {
    data.get("topFrame")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// Extract a `Breakpoint` from the named field of an event JSON object.
fn parse_breakpoint_field(data: &serde_json::Value, field: &str) -> Option<Breakpoint> {
    data.get(field)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// Extract a `Vec<Breakpoint>` from the named array field of an event JSON object.
///
/// Returns an empty `Vec` if the field is absent or not an array.
fn parse_breakpoint_array(data: &serde_json::Value, field: &str) -> Vec<Breakpoint> {
    data.get(field)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// Extract an `InstanceRef` from the named field of an event JSON object.
fn parse_instance_ref_field(data: &serde_json::Value, field: &str) -> Option<InstanceRef> {
    data.get(field)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::protocol::IsolateRef as ProtocolIsolateRef;
    use super::*;
    use serde_json::json;

    /// Helper: build a `StreamEvent` with the given kind and isolate for tests.
    fn make_event(kind: &str, data: serde_json::Value) -> StreamEvent {
        StreamEvent {
            kind: kind.to_string(),
            isolate: Some(ProtocolIsolateRef {
                id: "isolates/1".to_string(),
                name: "main".to_string(),
                number: None,
                is_system_isolate: None,
            }),
            timestamp: None,
            data,
        }
    }

    /// Helper: build a `StreamEvent` with a named isolate.
    fn make_event_with_isolate(
        kind: &str,
        isolate_id: &str,
        isolate_name: &str,
        data: serde_json::Value,
    ) -> StreamEvent {
        StreamEvent {
            kind: kind.to_string(),
            isolate: Some(ProtocolIsolateRef {
                id: isolate_id.to_string(),
                name: isolate_name.to_string(),
                number: None,
                is_system_isolate: None,
            }),
            timestamp: None,
            data,
        }
    }

    // -- parse_debug_event ---------------------------------------------------

    #[test]
    fn test_parse_pause_start_event() {
        let event = make_event_with_isolate("PauseStart", "isolates/1", "main", json!({}));
        let result = parse_debug_event(&event).unwrap();
        match result {
            DebugEvent::PauseStart { isolate, top_frame } => {
                assert_eq!(isolate.id, "isolates/1");
                assert_eq!(isolate.name.as_deref(), Some("main"));
                assert!(top_frame.is_none());
            }
            other => panic!("Expected PauseStart, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_pause_breakpoint_event() {
        let event = make_event_with_isolate(
            "PauseBreakpoint",
            "isolates/123",
            "main",
            json!({
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
            }),
        );

        let result = parse_debug_event(&event).unwrap();
        assert!(matches!(result, DebugEvent::PauseBreakpoint { .. }));

        if let DebugEvent::PauseBreakpoint {
            isolate,
            top_frame,
            breakpoint,
            pause_breakpoints,
            at_async_suspension,
        } = result
        {
            assert_eq!(isolate.id, "isolates/123");
            let frame = top_frame.unwrap();
            assert_eq!(frame.index, 0);
            let func = frame.function.unwrap();
            assert_eq!(func.name, "myFunc");
            let loc = frame.location.unwrap();
            assert_eq!(loc.line, Some(42));
            assert_eq!(loc.column, Some(5));
            let bp = breakpoint.unwrap();
            assert_eq!(bp.breakpoint_number, 1);
            assert!(bp.enabled);
            assert!(bp.resolved);
            assert!(pause_breakpoints.is_empty());
            assert!(!at_async_suspension);
        }
    }

    #[test]
    fn test_parse_pause_breakpoint_with_async_suspension() {
        let event = make_event(
            "PauseBreakpoint",
            json!({ "pauseBreakpoints": [], "atAsyncSuspension": true }),
        );

        let result = parse_debug_event(&event).unwrap();
        if let DebugEvent::PauseBreakpoint {
            at_async_suspension,
            ..
        } = result
        {
            assert!(at_async_suspension);
        }
    }

    #[test]
    fn test_parse_pause_exception_event() {
        let event = make_event(
            "PauseException",
            json!({
                "topFrame": { "index": 0, "vars": [] },
                "exception": {
                    "id": "objects/42",
                    "kind": "PlainInstance",
                    "classRef": { "id": "classes/1", "name": "Error" },
                    "valueAsString": "Something went wrong"
                }
            }),
        );

        let result = parse_debug_event(&event).unwrap();
        if let DebugEvent::PauseException { exception, .. } = result {
            let ex = exception.unwrap();
            assert_eq!(ex.kind, "PlainInstance");
            assert_eq!(ex.value_as_string.as_deref(), Some("Something went wrong"));
            let class_ref = ex.class_ref.unwrap();
            assert_eq!(class_ref.name, "Error");
        } else {
            panic!("Expected PauseException");
        }
    }

    #[test]
    fn test_parse_pause_exit_event() {
        let event = make_event("PauseExit", json!({}));
        let result = parse_debug_event(&event).unwrap();
        assert!(matches!(result, DebugEvent::PauseExit { .. }));
    }

    #[test]
    fn test_parse_pause_interrupted_event() {
        let event = make_event("PauseInterrupted", json!({ "atAsyncSuspension": false }));
        let result = parse_debug_event(&event).unwrap();
        assert!(matches!(
            result,
            DebugEvent::PauseInterrupted {
                at_async_suspension: false,
                ..
            }
        ));
    }

    #[test]
    fn test_parse_pause_post_request_event() {
        let event = make_event("PausePostRequest", json!({}));
        let result = parse_debug_event(&event).unwrap();
        assert!(matches!(result, DebugEvent::PausePostRequest { .. }));
    }

    #[test]
    fn test_parse_resume_event() {
        let event = make_event("Resume", json!({}));
        let result = parse_debug_event(&event).unwrap();
        assert!(matches!(result, DebugEvent::Resume { .. }));
    }

    #[test]
    fn test_parse_breakpoint_added_event() {
        let event = make_event(
            "BreakpointAdded",
            json!({
                "breakpoint": {
                    "id": "breakpoints/5",
                    "breakpointNumber": 5,
                    "enabled": true,
                    "resolved": false
                }
            }),
        );

        let result = parse_debug_event(&event).unwrap();
        if let DebugEvent::BreakpointAdded { breakpoint, .. } = result {
            assert_eq!(breakpoint.id, "breakpoints/5");
            assert_eq!(breakpoint.breakpoint_number, 5);
            assert!(!breakpoint.resolved);
        } else {
            panic!("Expected BreakpointAdded");
        }
    }

    #[test]
    fn test_parse_breakpoint_resolved_event() {
        let event = make_event(
            "BreakpointResolved",
            json!({
                "breakpoint": {
                    "id": "breakpoints/5",
                    "breakpointNumber": 5,
                    "enabled": true,
                    "resolved": true,
                    "location": {
                        "script": { "id": "scripts/1", "uri": "package:app/main.dart" },
                        "tokenPos": 200,
                        "line": 10
                    }
                }
            }),
        );

        let result = parse_debug_event(&event).unwrap();
        if let DebugEvent::BreakpointResolved { breakpoint, .. } = result {
            assert!(breakpoint.resolved);
            assert!(breakpoint.location.is_some());
        } else {
            panic!("Expected BreakpointResolved");
        }
    }

    #[test]
    fn test_parse_breakpoint_removed_event() {
        let event = make_event(
            "BreakpointRemoved",
            json!({
                "breakpoint": {
                    "id": "breakpoints/3",
                    "breakpointNumber": 3,
                    "enabled": true,
                    "resolved": true
                }
            }),
        );

        let result = parse_debug_event(&event).unwrap();
        assert!(matches!(result, DebugEvent::BreakpointRemoved { .. }));
    }

    #[test]
    fn test_parse_breakpoint_updated_event() {
        let event = make_event(
            "BreakpointUpdated",
            json!({
                "breakpoint": {
                    "id": "breakpoints/2",
                    "breakpointNumber": 2,
                    "enabled": false,
                    "resolved": true
                }
            }),
        );

        let result = parse_debug_event(&event).unwrap();
        if let DebugEvent::BreakpointUpdated { breakpoint, .. } = result {
            assert!(!breakpoint.enabled);
        } else {
            panic!("Expected BreakpointUpdated");
        }
    }

    #[test]
    fn test_parse_inspect_event() {
        let event = make_event(
            "Inspect",
            json!({
                "inspectee": {
                    "id": "objects/77",
                    "kind": "String",
                    "valueAsString": "hello"
                }
            }),
        );

        let result = parse_debug_event(&event).unwrap();
        if let DebugEvent::Inspect { inspectee, .. } = result {
            assert_eq!(inspectee.kind, "String");
            assert_eq!(inspectee.value_as_string.as_deref(), Some("hello"));
        } else {
            panic!("Expected Inspect");
        }
    }

    #[test]
    fn test_parse_unknown_debug_event_returns_none() {
        // Must include a valid isolate so the unknown-kind path is tested,
        // not the missing-isolate early return (issue #13).
        let event = make_event("UnknownEvent", json!({}));
        assert!(parse_debug_event(&event).is_none());
    }

    #[test]
    fn test_parse_debug_event_missing_isolate_returns_none() {
        // If `isolate` field is absent, parsing should fail gracefully.
        let event = StreamEvent {
            kind: "Resume".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({}),
        };
        assert!(parse_debug_event(&event).is_none());
    }

    // -- Integration test: full JSON -> StreamEvent -> parse_debug_event ----

    #[test]
    fn test_parse_debug_event_from_raw_json() {
        // Raw JSON as the VM Service would send it (isolate key at top level).
        // Verifies that serde #[flatten] correctly separates the typed `isolate`
        // field from the `data` remainder, and that parse_debug_event succeeds.
        let raw = json!({
            "kind": "PauseBreakpoint",
            "isolate": {
                "id": "isolates/123",
                "name": "main",
                "number": "1",
                "isSystemIsolate": false
            },
            "topFrame": {
                "index": 0,
                "kind": "Regular"
            },
            "pauseBreakpoints": [],
            "atAsyncSuspension": false,
            "timestamp": 1_234_567_890_i64
        });
        let stream_event: StreamEvent = serde_json::from_value(raw).unwrap();
        // The typed `isolate` field must be populated.
        assert!(
            stream_event.isolate.is_some(),
            "StreamEvent.isolate must be Some after deserialization"
        );
        // The flatten remainder must NOT contain `isolate` (serde consumed it).
        assert!(
            stream_event.data.get("isolate").is_none(),
            "StreamEvent.data must not contain 'isolate' (consumed by typed field)"
        );

        let debug_event = parse_debug_event(&stream_event);
        assert!(
            debug_event.is_some(),
            "parse_debug_event must succeed with real VM JSON"
        );
        match debug_event.unwrap() {
            DebugEvent::PauseBreakpoint { isolate, .. } => {
                assert_eq!(isolate.id, "isolates/123");
            }
            other => panic!("Expected PauseBreakpoint, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_isolate_event_from_raw_json() {
        // Integration test: raw JSON -> StreamEvent -> parse_isolate_event.
        let raw = json!({
            "kind": "IsolateStart",
            "isolate": {
                "id": "isolates/456",
                "name": "worker",
                "number": "2",
                "isSystemIsolate": false
            },
            "timestamp": 1_234_567_890_i64
        });
        let stream_event: StreamEvent = serde_json::from_value(raw).unwrap();
        assert!(
            stream_event.isolate.is_some(),
            "StreamEvent.isolate must be Some after deserialization"
        );
        assert!(
            stream_event.data.get("isolate").is_none(),
            "StreamEvent.data must not contain 'isolate' (consumed by typed field)"
        );

        let isolate_event = parse_isolate_event(&stream_event);
        assert!(
            isolate_event.is_some(),
            "parse_isolate_event must succeed with real VM JSON"
        );
        match isolate_event.unwrap() {
            IsolateEvent::IsolateStart { isolate } => {
                assert_eq!(isolate.id, "isolates/456");
            }
            other => panic!("Expected IsolateStart, got {:?}", other),
        }
    }

    // -- parse_isolate_event -------------------------------------------------

    #[test]
    fn test_parse_isolate_start_event() {
        let event = make_event_with_isolate("IsolateStart", "isolates/456", "worker", json!({}));
        let result = parse_isolate_event(&event).unwrap();
        assert!(matches!(result, IsolateEvent::IsolateStart { .. }));

        if let IsolateEvent::IsolateStart { isolate } = result {
            assert_eq!(isolate.id, "isolates/456");
            assert_eq!(isolate.name.as_deref(), Some("worker"));
        }
    }

    #[test]
    fn test_parse_isolate_runnable_event() {
        let event = make_event("IsolateRunnable", json!({}));
        let result = parse_isolate_event(&event).unwrap();
        assert!(matches!(result, IsolateEvent::IsolateRunnable { .. }));
    }

    #[test]
    fn test_parse_isolate_exit_event() {
        let event = make_event_with_isolate("IsolateExit", "isolates/2", "worker", json!({}));
        let result = parse_isolate_event(&event).unwrap();
        assert!(matches!(result, IsolateEvent::IsolateExit { .. }));
    }

    #[test]
    fn test_parse_isolate_update_event() {
        let event =
            make_event_with_isolate("IsolateUpdate", "isolates/1", "main (renamed)", json!({}));
        let result = parse_isolate_event(&event).unwrap();
        assert!(matches!(result, IsolateEvent::IsolateUpdate { .. }));
    }

    #[test]
    fn test_parse_isolate_reload_event() {
        let event = make_event("IsolateReload", json!({}));
        let result = parse_isolate_event(&event).unwrap();
        assert!(matches!(result, IsolateEvent::IsolateReload { .. }));
    }

    #[test]
    fn test_parse_service_extension_added_event() {
        let event = make_event(
            "ServiceExtensionAdded",
            json!({ "extensionRPC": "ext.flutter.reassemble" }),
        );

        let result = parse_isolate_event(&event).unwrap();
        if let IsolateEvent::ServiceExtensionAdded {
            extension_rpc,
            isolate,
        } = result
        {
            assert_eq!(extension_rpc, "ext.flutter.reassemble");
            assert_eq!(isolate.id, "isolates/1");
        } else {
            panic!("Expected ServiceExtensionAdded");
        }
    }

    #[test]
    fn test_parse_service_extension_added_missing_rpc_returns_none() {
        // When `extensionRPC` is absent, parse_isolate_event must return None
        // rather than producing an event with an empty extension_rpc string.
        let event = StreamEvent {
            kind: "ServiceExtensionAdded".to_string(),
            isolate: Some(ProtocolIsolateRef {
                id: "isolates/1".to_string(),
                name: "main".to_string(),
                number: None,
                is_system_isolate: None,
            }),
            timestamp: None,
            data: json!({}), // no extensionRPC field
        };
        assert!(parse_isolate_event(&event).is_none());
    }

    #[test]
    fn test_parse_unknown_isolate_event_returns_none() {
        // Must include a valid isolate so the unknown-kind path is tested.
        let event = make_event("UnknownIsolateKind", json!({}));
        assert!(parse_isolate_event(&event).is_none());
    }

    #[test]
    fn test_parse_isolate_event_missing_isolate_returns_none() {
        let event = StreamEvent {
            kind: "IsolateStart".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({}),
        };
        assert!(parse_isolate_event(&event).is_none());
    }

    // -- StepOption ----------------------------------------------------------

    #[test]
    fn test_step_option_as_str() {
        assert_eq!(StepOption::Into.as_str(), "Into");
        assert_eq!(StepOption::Over.as_str(), "Over");
        assert_eq!(StepOption::Out.as_str(), "Out");
        assert_eq!(
            StepOption::OverAsyncSuspension.as_str(),
            "OverAsyncSuspension"
        );
    }

    // -- ExceptionPauseMode --------------------------------------------------

    #[test]
    fn test_exception_pause_mode_as_str() {
        assert_eq!(ExceptionPauseMode::None.as_str(), "None");
        assert_eq!(ExceptionPauseMode::Unhandled.as_str(), "Unhandled");
        assert_eq!(ExceptionPauseMode::All.as_str(), "All");
    }

    #[test]
    fn test_exception_pause_mode_default_is_unhandled() {
        assert_eq!(ExceptionPauseMode::default(), ExceptionPauseMode::Unhandled);
    }

    #[test]
    fn test_exception_pause_mode_roundtrip() {
        let modes = [
            ExceptionPauseMode::None,
            ExceptionPauseMode::Unhandled,
            ExceptionPauseMode::All,
        ];
        for mode in &modes {
            let json = serde_json::to_value(mode).unwrap();
            let roundtrip: ExceptionPauseMode = serde_json::from_value(json).unwrap();
            assert_eq!(*mode, roundtrip);
        }
    }

    // -- Type deserialization roundtrips ------------------------------------

    #[test]
    fn test_script_ref_deserialize() {
        let json = json!({
            "id": "scripts/99",
            "uri": "package:my_app/src/utils.dart",
            "type": "@Script"
        });
        let script: ScriptRef = serde_json::from_value(json).unwrap();
        assert_eq!(script.id, "scripts/99");
        assert_eq!(script.uri, "package:my_app/src/utils.dart");
    }

    #[test]
    fn test_source_location_deserialize() {
        let json = json!({
            "script": { "id": "scripts/1", "uri": "package:app/main.dart" },
            "tokenPos": 500,
            "line": 25,
            "column": 8
        });
        let loc: SourceLocation = serde_json::from_value(json).unwrap();
        assert_eq!(loc.token_pos, 500);
        assert_eq!(loc.line, Some(25));
        assert_eq!(loc.column, Some(8));
        assert_eq!(loc.script.uri, "package:app/main.dart");
    }

    #[test]
    fn test_source_location_optional_fields() {
        // line and column are optional — should parse without them
        let json = json!({
            "script": { "id": "scripts/1", "uri": "dart:core" },
            "tokenPos": 0
        });
        let loc: SourceLocation = serde_json::from_value(json).unwrap();
        assert_eq!(loc.line, None);
        assert_eq!(loc.column, None);
    }

    #[test]
    fn test_breakpoint_deserialize_with_location() {
        let json = json!({
            "id": "breakpoints/7",
            "breakpointNumber": 7,
            "enabled": true,
            "resolved": true,
            "location": {
                "type": "SourceLocation",
                "script": { "id": "scripts/2", "uri": "package:app/main.dart" },
                "tokenPos": 42,
                "line": 10,
                "column": 1
            }
        });
        let bp: Breakpoint = serde_json::from_value(json).unwrap();
        assert_eq!(bp.breakpoint_number, 7);
        assert!(bp.resolved);
        assert!(bp.location.is_some());
    }

    #[test]
    fn test_breakpoint_deserialize_without_location() {
        let json = json!({
            "id": "breakpoints/3",
            "breakpointNumber": 3,
            "enabled": false,
            "resolved": false
        });
        let bp: Breakpoint = serde_json::from_value(json).unwrap();
        assert!(!bp.enabled);
        assert!(bp.location.is_none());
    }

    #[test]
    fn test_frame_kind_deserialize_regular() {
        let json = json!("Regular");
        let kind: FrameKind = serde_json::from_value(json).unwrap();
        assert!(matches!(kind, FrameKind::Regular));
    }

    #[test]
    fn test_frame_kind_deserialize_async_causal() {
        let json = json!("AsyncCausal");
        let kind: FrameKind = serde_json::from_value(json).unwrap();
        assert!(matches!(kind, FrameKind::AsyncCausal));
    }

    #[test]
    fn test_frame_kind_deserialize_async_suspension_marker() {
        let json = json!("AsyncSuspensionMarker");
        let kind: FrameKind = serde_json::from_value(json).unwrap();
        assert!(matches!(kind, FrameKind::AsyncSuspensionMarker));
    }

    #[test]
    fn test_frame_kind_deserialize_unknown_fails() {
        let json = json!("regular"); // wrong casing
        let result: Result<FrameKind, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_instance_ref_deserialize_full() {
        let json = json!({
            "id": "objects/5",
            "kind": "String",
            "classRef": { "id": "classes/dart:core/String", "name": "String" },
            "valueAsString": "hello world",
            "valueAsStringIsTruncated": false,
            "length": 11
        });
        let inst: InstanceRef = serde_json::from_value(json).unwrap();
        assert_eq!(inst.id.as_deref(), Some("objects/5"));
        assert_eq!(inst.kind, "String");
        assert_eq!(inst.value_as_string.as_deref(), Some("hello world"));
        assert_eq!(inst.value_as_string_is_truncated, Some(false));
        assert_eq!(inst.length, Some(11));
        let class_ref = inst.class_ref.unwrap();
        assert_eq!(class_ref.name, "String");
    }

    #[test]
    fn test_instance_ref_sentinel_no_id() {
        // Sentinels don't have an id
        let json = json!({
            "kind": "Sentinel",
            "valueAsString": "<expired>"
        });
        let inst: InstanceRef = serde_json::from_value(json).unwrap();
        assert!(inst.id.is_none());
        assert_eq!(inst.kind, "Sentinel");
    }

    #[test]
    fn test_stack_deserialize() {
        let json = json!({
            "frames": [
                {
                    "index": 0,
                    "function": { "id": "func/1", "name": "main" },
                    "location": {
                        "script": { "id": "scripts/1", "uri": "package:app/main.dart" },
                        "tokenPos": 10
                    },
                    "vars": [],
                    "kind": "Regular"
                }
            ],
            "truncated": false
        });
        let stack: Stack = serde_json::from_value(json).unwrap();
        assert_eq!(stack.frames.len(), 1);
        assert_eq!(stack.frames[0].index, 0);
        assert_eq!(stack.truncated, Some(false));
        assert!(stack.async_causal_frames.is_none());
        assert!(stack.awaiter_frames.is_none());
    }

    #[test]
    fn test_stack_with_async_frames() {
        let json = json!({
            "frames": [],
            "asyncCausalFrames": [
                {
                    "index": 0,
                    "kind": "AsyncCausal"
                }
            ],
            "awaiterFrames": [],
            "truncated": true
        });
        let stack: Stack = serde_json::from_value(json).unwrap();
        assert!(stack.async_causal_frames.is_some());
        assert_eq!(stack.truncated, Some(true));
    }

    #[test]
    fn test_script_list_deserialize() {
        let json = json!({
            "scripts": [
                { "id": "scripts/1", "uri": "package:app/main.dart" },
                { "id": "scripts/2", "uri": "package:app/widgets.dart" }
            ]
        });
        let list: ScriptList = serde_json::from_value(json).unwrap();
        assert_eq!(list.scripts.len(), 2);
        assert_eq!(list.scripts[0].uri, "package:app/main.dart");
    }

    #[test]
    fn test_isolate_ref_deserialize_with_optional_name() {
        // name is optional in the debug IsolateRef
        let json = json!({ "id": "isolates/99" });
        let isolate: IsolateRef = serde_json::from_value(json).unwrap();
        assert_eq!(isolate.id, "isolates/99");
        assert!(isolate.name.is_none());
    }

    #[test]
    fn test_isolate_ref_deserialize_ignores_unknown_fields() {
        // Unknown fields should be ignored (no #[serde(deny_unknown_fields)])
        let json = json!({
            "type": "@Isolate",
            "id": "isolates/1",
            "name": "main",
            "number": "1",
            "isSystemIsolate": false
        });
        let isolate: IsolateRef = serde_json::from_value(json).unwrap();
        assert_eq!(isolate.id, "isolates/1");
        assert_eq!(isolate.name.as_deref(), Some("main"));
    }

    #[test]
    fn test_bound_variable_deserialize() {
        let json = json!({
            "name": "counter",
            "value": {
                "id": "objects/10",
                "kind": "Int",
                "valueAsString": "42"
            }
        });
        let var: BoundVariable = serde_json::from_value(json).unwrap();
        assert_eq!(var.name, "counter");
        assert_eq!(var.value.kind, "Int");
        assert_eq!(var.value.value_as_string.as_deref(), Some("42"));
    }

    #[test]
    fn test_function_ref_deserialize() {
        let json = json!({
            "type": "@Function",
            "id": "functions/main",
            "name": "main"
        });
        let func: FunctionRef = serde_json::from_value(json).unwrap();
        assert_eq!(func.id, "functions/main");
        assert_eq!(func.name, "main");
    }

    #[test]
    fn test_class_ref_deserialize() {
        let json = json!({
            "type": "@Class",
            "id": "classes/1",
            "name": "MyWidget"
        });
        let class: ClassRef = serde_json::from_value(json).unwrap();
        assert_eq!(class.id, "classes/1");
        assert_eq!(class.name, "MyWidget");
    }

    #[test]
    fn test_frame_with_vars() {
        let json = json!({
            "index": 1,
            "function": { "id": "func/2", "name": "build" },
            "location": {
                "script": { "id": "scripts/3", "uri": "package:app/widget.dart" },
                "tokenPos": 300,
                "line": 55,
                "column": 12
            },
            "vars": [
                {
                    "name": "context",
                    "value": {
                        "id": "objects/20",
                        "kind": "PlainInstance"
                    }
                }
            ],
            "kind": "Regular"
        });
        let frame: Frame = serde_json::from_value(json).unwrap();
        assert_eq!(frame.index, 1);
        let vars = frame.vars.unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "context");
    }
}
