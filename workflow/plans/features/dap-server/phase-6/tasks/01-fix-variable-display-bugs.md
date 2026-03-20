## Task: Fix Critical Variable Display Bugs

**Objective**: Fix the three critical bugs preventing variables from displaying correctly in IDE debuggers: (1) `"class"` vs `"classRef"` field name mismatch causing wrong type names for locals, (2) `extract_source` used instead of `extract_source_with_store` preventing SDK source viewing, (3) `supportsRestartRequest: true` advertised with no handler.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/variables.rs`: Fix `instance_ref_to_variable` to read both `"classRef"` and `"class"` field names; switch `handle_stack_trace` to use `extract_source_with_store`
- `crates/fdemon-dap/src/protocol/types.rs`: Remove `supports_restart_request: Some(true)` from `fdemon_defaults()` (until restart handler is implemented in Task 10)

**Files Read (Dependencies):**
- `crates/fdemon-dap/src/adapter/stack.rs`: `extract_source_with_store` signature and `SourceReferenceStore` API
- `crates/fdemon-daemon/src/vm_service/debugger_types.rs`: `InstanceRef` struct with `#[serde(rename_all = "camelCase")]`

### Details

#### Bug 1: `"class"` vs `"classRef"` mismatch (Primary cause of issue #24)

`VmServiceBackend::get_stack()` at `dap_backend.rs:207-216` calls `serde_json::to_value(&stack)` on a typed `Stack` struct. The `InstanceRef` struct has `class_ref: Option<ClassRef>` decorated with `#[serde(rename_all = "camelCase")]`, so it serializes to `"classRef"`. But `instance_ref_to_variable` at `variables.rs:365-370` reads `.get("class")`:

```rust
// BEFORE (broken):
let class_name = instance_ref
    .get("class")                    // ← always None for locals
    .and_then(|c| c.get("name"))
    .and_then(|n| n.as_str());

// AFTER (fixed):
let class_name = instance_ref
    .get("classRef")                 // typed Stack serialization (camelCase)
    .or_else(|| instance_ref.get("class"))  // raw VM wire format (expand_object path)
    .and_then(|c| c.get("name"))
    .and_then(|n| n.as_str());
```

This must also be applied in `expand_object` at line ~560-583 where fields are read — check all `.get("class")` usages in the file.

#### Bug 2: `extract_source` vs `extract_source_with_store`

`handle_stack_trace` at `variables.rs:97` calls `extract_source(frame)` which never assigns source references. Change to:

```rust
// BEFORE:
let source = extract_source(frame);

// AFTER:
let source = extract_source_with_store(
    frame,
    &mut self.source_reference_store,
    &isolate_id,
    Some(self.project_root.as_path()),  // or None if unavailable
);
```

This requires `handle_stack_trace` to have access to `self.source_reference_store` (it already does — `DapAdapter` owns it) and a project root path. If `DapAdapter` doesn't currently store a project root, pass `None` — the source reference allocation still works, just without package URI resolution.

#### Bug 3: `supportsRestartRequest` advertised without handler

At `protocol/types.rs:869`, `fdemon_defaults()` sets `supports_restart_request: Some(true)`. No `restart` handler exists in the dispatch table (`handlers.rs:36-55`). Remove this until Task 10 implements the handler:

```rust
// BEFORE:
supports_restart_request: Some(true),

// AFTER:
supports_restart_request: None,
```

### Acceptance Criteria

1. Locals variables show correct class names (e.g., `"MyClass"` instead of `"PlainInstance instance"`)
2. Collections show class-derived type names (e.g., `"List<String> (3 items)"` instead of `"List (length: 3)"`)
3. SDK source frames in stack trace have `sourceReference > 0`, enabling the IDE to request source text
4. Package source frames resolve to local paths when `.dart_tool/package_config.json` exists
5. `supportsRestartRequest` is not advertised in `initialize` response
6. All existing tests pass
7. 8+ new unit tests for the `classRef`/`class` dual-path lookup

### Testing

```rust
#[test]
fn test_instance_ref_to_variable_uses_class_ref_camel_case() {
    // Simulate typed Stack serialization (camelCase):
    // {"kind": "PlainInstance", "classRef": {"name": "MyWidget"}, "id": "objects/123"}
    // Verify variable.type == "MyWidget", not "PlainInstance"
}

#[test]
fn test_instance_ref_to_variable_uses_class_raw_wire() {
    // Simulate raw VM wire format:
    // {"kind": "PlainInstance", "class": {"name": "MyWidget"}, "id": "objects/123"}
    // Verify variable.type == "MyWidget"
}

#[test]
fn test_list_variable_shows_class_name_in_type() {
    // {"kind": "List", "classRef": {"name": "List<int>"}, "length": 3, "id": "objects/456"}
    // Verify variable.type == "List<int>"
}
```

### Notes

- The `expand_object` path calls `backend.get_object()` which returns raw VM wire JSON (not round-tripped through typed structs), so it uses `"class"` correctly. The fix only breaks if a future change starts round-tripping `get_object` through typed structs — the `.or_else()` pattern handles both.
- Mock tests use raw JSON directly and won't catch this bug. Integration testing with a real Flutter app is needed to verify the fix end-to-end.
