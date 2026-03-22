## Task: Improve Variable Type Rendering

**Objective**: Fix type rendering gaps in `instance_ref_to_variable` and `expand_object`: add Record type expansion, string truncation indicators, Sentinel/WeakReference handling, and fix Set expansion (currently falls to wrong code path).

**Depends on**: 01-fix-variable-display-bugs

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/variables.rs`: Add match arms for `Record`, `WeakReference`, `Sentinel`; fix Set expansion in `expand_object`; add string truncation indicator

### Details

#### 1. String truncation indicator

At `variables.rs:399-410`, the `"String"` arm formats strings but doesn't check `valueAsStringIsTruncated`:

```rust
// BEFORE:
"String" => {
    let val = value_as_string.unwrap_or("");
    (format!("\"{}\"", val), 0, "String".to_string())
}

// AFTER:
"String" => {
    let val = value_as_string.unwrap_or("");
    let truncated = instance_ref.get("valueAsStringIsTruncated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let display = if truncated {
        format!("\"{}...\"", val)
    } else {
        format!("\"{}\"", val)
    };
    // If truncated, assign a variablesReference so user can see full string
    let var_ref = if truncated && obj_id.is_some() {
        self.var_store.allocate(VariableRef::Object { ... })
    } else { 0 };
    (display, var_ref, "String".to_string())
}
```

#### 2. Record type support

Add explicit `"Record"` arm:

```rust
"Record" => {
    let length = instance_ref.get("length")
        .and_then(|l| l.as_i64())
        .unwrap_or(0);
    let display = format!("Record ({} fields)", length);
    let var_ref = allocate_object_ref(isolate_id, obj_id);
    (display, var_ref, "Record".to_string())
}
```

In `expand_object`, add `"Record"` handling. Records have `fields` like `PlainInstance` but field names are `$1`, `$2`, etc. for positional fields:

```rust
"Record" => {
    // Records use the same "fields" structure as PlainInstance
    // Fields are named "$1", "$2" for positional, or their names for named fields
    expand_fields(obj, isolate_id)
}
```

#### 3. WeakReference type

```rust
"WeakReference" => {
    let display = "WeakReference".to_string();
    let var_ref = allocate_object_ref(isolate_id, obj_id);
    (display, var_ref, "WeakReference".to_string())
}
```

In `expand_object`, `WeakReference` has a `target` field which may be `null` (if the target was garbage collected).

#### 4. Sentinel handling

```rust
"Sentinel" => {
    let display = value_as_string
        .map(|s| s.to_string())
        .unwrap_or_else(|| "<optimized out>".to_string());
    (display, 0, "Sentinel".to_string())
}
```

#### 5. Fix Set expansion in `expand_object`

Currently `"Set"` is matched in `instance_ref_to_variable` collection arm but falls to the `_` (fields) path in `expand_object`. Sets are stored like Lists in the VM Service — they have an `elements` array:

```rust
// In expand_object, extend the List match:
"List" | "Set" | "Uint8List" | ... => {
    // Read elements array
}
```

#### 6. Update `is_expandable` in `evaluate.rs`

Add `"Record"`, `"WeakReference"` to the match in `is_expandable`:

```rust
pub fn is_expandable(instance: &serde_json::Value) -> bool {
    matches!(kind, "List" | "Map" | "Set" | "PlainInstance" | "Closure" | "Record" | "WeakReference" | ...)
}
```

### Acceptance Criteria

1. Truncated strings show `"hello..."` with ellipsis indicator
2. Record types display as `"Record (N fields)"` and expand to show `$1`, `$2` etc.
3. WeakReference displays as `"WeakReference"` and expands to show `target`
4. Sentinels display as their `valueAsString` or `"<optimized out>"`
5. Set expansion shows indexed elements (not empty fields)
6. 15+ new unit tests

### Testing

```rust
#[test]
fn test_string_truncation_indicator() {
    // InstanceRef with kind: "String", valueAsString: "hello world",
    // valueAsStringIsTruncated: true
    // Verify display value ends with "..."
}

#[test]
fn test_record_type_display() {
    // InstanceRef with kind: "Record", length: 3
    // Verify "Record (3 fields)" display
}

#[tokio::test]
async fn test_set_expansion_uses_elements() {
    // get_object returns Instance with kind: "Set", elements: [...]
    // Verify elements are returned as indexed variables [0], [1], ...
}
```

### Notes

- `TypeParameter` and `TypeArguments` kinds should be filtered from variable expansion — they are internal VM details. If an `@TypeArguments` entry appears in a fields list, skip it.
- For full-string viewing of truncated strings, `expand_object` should call `get_object` with no offset/count to get the complete `valueAsString`. This is a secondary concern — the truncation indicator is the priority.

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/variables.rs` | Fixed String arm to check `valueAsStringIsTruncated` and show ellipsis; added `Record`, `WeakReference`, `Sentinel` match arms in `instance_ref_to_variable`; fixed `expand_object` to route `"Set"` through the `elements` path; added `"Record"` and `"WeakReference"` expansion arms; added `TypeArguments` field filtering in the generic `_` fields path |
| `crates/fdemon-dap/src/adapter/evaluate.rs` | Extended `is_expandable` to include `"Record"` and `"WeakReference"` |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Registered new `variable_type_rendering` test module |
| `crates/fdemon-dap/src/adapter/tests/variable_type_rendering.rs` | New file with 20 unit tests |

### Notable Decisions/Tradeoffs

1. **WeakReference null target normalization**: When the `target` field from `get_object` is JSON null (absent or explicit null), the code normalises it to `{"kind": "Null"}` so that `instance_ref_to_variable` renders it as `"null"` rather than `"<unknown>"`. This matches the behavior the VM would exhibit when a WeakReference target has been GC'd.

2. **TypeArguments filtering in `_` arm only**: The `@TypeArguments` filter is applied in the catch-all `_` arm of `expand_object`. The new `Record` arm does not filter them (records should not have TypeArguments entries in their fields per the VM spec).

3. **Set display unchanged in `instance_ref_to_variable`**: The existing collection arm already covers `"Set"` for display. The only fix needed was routing `"Set"` to the `elements` expansion path in `expand_object`.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-dap` - Passed (645 tests, 20 new from this task)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **Record expansion**: The VM Service spec says Record fields use `$1`, `$2` names for positional fields. The implementation relies on the VM sending these names correctly in the `fields` array — no additional mapping is done.
2. **Full-string viewing for truncated strings**: Per the task notes, this is a secondary concern. The truncation indicator and `variablesReference` allocation are implemented; the actual full-string retrieval in `expand_object` for truncated String objects falls through to the generic fields path which will return empty (no fields on a String instance). This can be improved in a future task by adding a special `"String"` arm in `expand_object` that calls `get_object` and reads `valueAsString`.
