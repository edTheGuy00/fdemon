## Task: Fix serde flatten bug in parse_debug_event / parse_isolate_event

**Objective**: Fix the critical production bug where all Debug and Isolate stream events are silently dropped due to serde `#[serde(flatten)]` consuming the `isolate` field before the flatten remainder is passed to the parsers.

**Depends on**: None

**Review Issues**: #1 (critical), #7 (clone-per-event), #13 (bad test)

### Root Cause

`StreamEvent` in `protocol.rs:116-128` has:
```rust
pub struct StreamEvent {
    pub kind: String,
    pub isolate: Option<IsolateRef>,   // serde consumes "isolate" key here
    pub timestamp: Option<i64>,
    #[serde(flatten)]
    pub data: Value,                   // does NOT contain "isolate"
}
```

`parse_debug_event(kind, data)` and `parse_isolate_event(kind, data)` receive `&event.params.event.data` and call `parse_isolate_ref(data)` which does `data.get("isolate")` — always `None` because serde already consumed `"isolate"` into the typed field.

### Scope

- `crates/fdemon-daemon/src/vm_service/debugger_types.rs`:
  - Change `parse_debug_event` signature from `(kind: &str, data: &Value)` to `(event: &StreamEvent)` — matching `parse_gc_event`, `parse_flutter_error`, `parse_log_record`
  - Change `parse_isolate_event` signature similarly
  - Read `isolate` from `event.isolate.clone()?` instead of `parse_isolate_ref(data)`
  - Read kind-specific fields from `event.data` (topFrame, breakpoint, etc.)
  - Remove `parse_isolate_ref` helper (no longer needed)
  - Keep `parse_top_frame`, `parse_breakpoint_field`, `parse_breakpoint_array`, `parse_instance_ref_field` but update them to take `&Value` from `event.data`
  - Update all 61+ unit tests to construct `StreamEvent` structs instead of raw `Value` objects

- `crates/fdemon-app/src/actions/vm_service.rs`:
  - Update call sites at lines 209-212 and 226-230: change `parse_debug_event(&kind, &data)` to `parse_debug_event(&event.params.event)`
  - Same for `parse_isolate_event`

### Details

**Working pattern to follow** (from `parse_gc_event` in `performance.rs:294-323`):
```rust
pub fn parse_gc_event(event: &StreamEvent) -> Option<GcEvent> {
    if event.kind != "GC" { return None; }
    let gc_type = event.data.get("gcType")...;     // kind-specific from flatten
    let isolate_id = event
        .isolate                                    // typed field on StreamEvent
        .as_ref()
        .map(|iso| iso.id.clone());
    // ...
}
```

**New signatures:**
```rust
pub fn parse_debug_event(event: &StreamEvent) -> Option<DebugEvent> {
    let isolate = event.isolate.clone()?;
    match event.kind.as_str() {
        "PauseStart" => Some(DebugEvent::PauseStart {
            isolate,
            top_frame: parse_top_frame(&event.data),
        }),
        // ... all other arms use &event.data for kind-specific fields
        _ => None,
    }
}
```

**Call site update:**
```rust
// BEFORE
if let Some(debug_event) = parse_debug_event(
    &event.params.event.kind,
    &event.params.event.data,
) { ... }

// AFTER
if let Some(debug_event) = parse_debug_event(&event.params.event) { ... }
```

**Test update pattern:**
```rust
// BEFORE (tests pass but don't catch the bug)
let data = json!({
    "isolate": { "id": "isolates/1", "name": "main", ... },
    "topFrame": { ... }
});
let event = parse_debug_event("PauseStart", &data).unwrap();

// AFTER (tests exercise the actual production path)
let event = StreamEvent {
    kind: "PauseStart".to_string(),
    isolate: Some(IsolateRef { id: "isolates/1".into(), ... }),
    timestamp: None,
    data: json!({ "topFrame": { ... } }),  // only remainder fields
};
let debug_event = parse_debug_event(&event).unwrap();
```

**Integration test** (new — validates full JSON deserialization through StreamEvent):
```rust
#[test]
fn test_parse_debug_event_from_raw_json() {
    // Raw JSON as the VM Service would send it
    let raw = json!({
        "kind": "PauseBreakpoint",
        "isolate": { "id": "isolates/123", "name": "main", "number": "1", "isSystemIsolate": false },
        "topFrame": { "index": 0, "kind": "Regular", "code": { ... } },
        "timestamp": 1234567890
    });
    let stream_event: StreamEvent = serde_json::from_value(raw).unwrap();
    let debug_event = parse_debug_event(&stream_event);
    assert!(debug_event.is_some(), "parse_debug_event must succeed with real VM JSON");
    // Verify isolate was correctly extracted
    match debug_event.unwrap() {
        DebugEvent::PauseBreakpoint { isolate, .. } => {
            assert_eq!(isolate.id, "isolates/123");
        }
        other => panic!("Expected PauseBreakpoint, got {:?}", other),
    }
}
```

**Fixing issue #13 (bad test)**: When updating tests, ensure `test_parse_unknown_debug_event_returns_none` provides a valid `isolate` in the `StreamEvent` so it actually tests the `_ => None` catch-all, not the missing-isolate early return.

**Resolving issue #7 (clone-per-event)**: By accepting `&StreamEvent`, the `isolate` is cloned once from the typed field. The `parse_isolate_ref` helper (which cloned from `Value`) is eliminated entirely. Internal helpers like `parse_top_frame` still clone from `&event.data`, but those are per-field, not per-event.

### Acceptance Criteria

1. `parse_debug_event` accepts `&StreamEvent` and returns `Some(DebugEvent)` when called with a `StreamEvent` deserialized from real VM Service JSON
2. `parse_isolate_event` accepts `&StreamEvent` and returns `Some(IsolateEvent)` similarly
3. `parse_isolate_ref` helper is removed
4. Call sites in `vm_service.rs` updated to pass `&event.params.event`
5. Integration test added that deserializes raw JSON → `StreamEvent` → `parse_debug_event` and asserts success
6. All existing unit tests updated to construct `StreamEvent` instead of raw `Value`
7. `test_parse_unknown_debug_event_returns_none` includes a valid isolate to test the `_ => None` path
8. `cargo test --workspace` passes with no regressions

### Testing

- All 61+ existing `debugger_types.rs` tests updated to use `StreamEvent`
- New integration tests exercising full JSON → StreamEvent → parse path for both debug and isolate events
- `cargo check --workspace` passes
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes

### Notes

- The `StreamEvent` type must be imported in `debugger_types.rs` — it lives in `super::protocol::StreamEvent`
- The `IsolateRef` type is already defined in `debugger_types.rs` and re-exported, so no circular dependency
- This is the only blocking issue — must be resolved before the Phase 1 branch can merge

---

## Completion Summary

**Status:** Not Started
