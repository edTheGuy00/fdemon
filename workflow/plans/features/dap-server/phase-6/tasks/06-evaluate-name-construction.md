## Task: Implement evaluateName Construction

**Objective**: Set the `evaluateName` field on every DAP `Variable` returned by the adapter, enabling watch expressions to drill into nested objects. When a user right-clicks a variable in the IDE and selects "Add to Watch", the IDE uses `evaluateName` to construct the watch expression.

**Depends on**: 01-fix-variable-display-bugs

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/variables.rs`: Thread `evaluate_name` through `instance_ref_to_variable` and `expand_object`; set `evaluate_name` on every returned `DapVariable`

### Details

#### `evaluateName` rules:

| Context | evaluateName |
|---------|-------------|
| Local variable `x` | `"x"` |
| Field `name` of local `x` | `"x.name"` |
| Indexed element `[0]` of local `list` | `"list[0]"` |
| Map entry with string key `"foo"` of `myMap` | `'myMap["foo"]'` |
| Map entry with int key `42` | `"myMap[42]"` |
| Exception root | `"$_threadException"` |
| Global static field `counter` | `"counter"` |
| Nested field `x.inner.value` | `"x.inner.value"` |

#### Implementation:

1. Add `evaluate_name: Option<String>` parameter to `instance_ref_to_variable`:

```rust
pub(super) fn instance_ref_to_variable(
    &mut self,
    name: &str,
    instance_ref: &serde_json::Value,
    isolate_id: &str,
    evaluate_name: Option<&str>,  // NEW
) -> DapVariable
```

Set `evaluate_name` on the returned `DapVariable`.

2. In `get_scope_variables` for Locals, pass the variable name as `evaluate_name`:

```rust
let var = self.instance_ref_to_variable(name, value, isolate_id, Some(name));
```

3. In `expand_object`, construct child `evaluate_name` values:

```rust
// For fields:
let child_eval_name = parent_eval_name.map(|p| format!("{}.{}", p, field_name));

// For indexed elements:
let child_eval_name = parent_eval_name.map(|p| format!("{}[{}]", p, index));

// For map entries with string keys:
let child_eval_name = parent_eval_name.map(|p| format!("{}[\"{}\"]]", p, key_string));
```

4. Store `evaluate_name` alongside `VariableRef::Object` so it's available when expanding:

Either add `evaluate_name: Option<String>` to `VariableRef::Object`, or maintain a separate `HashMap<i64, String>` mapping variable references to their evaluate names.

### Acceptance Criteria

1. All local variables have `evaluateName` set to their name
2. Object fields have `evaluateName` = `parent.fieldName`
3. List elements have `evaluateName` = `parent[index]`
4. Map entries have `evaluateName` = `parent["key"]` or `parent[key]`
5. Adding a nested variable to watch works in VS Code
6. 8+ new unit tests

### Testing

```rust
#[test]
fn test_local_variable_evaluate_name() {
    // instance_ref_to_variable("myVar", ..., Some("myVar"))
    // Verify result.evaluate_name == Some("myVar")
}

#[tokio::test]
async fn test_field_evaluate_name() {
    // Expand object with parent evaluate_name "obj"
    // Verify child field has evaluate_name "obj.fieldName"
}

#[tokio::test]
async fn test_indexed_evaluate_name() {
    // Expand List with parent evaluate_name "myList"
    // Verify element [0] has evaluate_name "myList[0]"
}
```

### Notes

- `evaluate_name` is optional in the DAP spec — clients should handle `None` gracefully. But setting it enables the "Add to Watch" feature which is important for debugging workflows.
- For globals, `evaluate_name` should just be the field name (no prefix needed since globals are accessible by name directly).
- For exception scope, `evaluate_name` should be `"$_threadException"` for the root and `"$_threadException.fieldName"` for children.

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `evaluate_name_map: HashMap<i64, String>` field to `DapAdapter` and initialized it in `new_with_tx` |
| `crates/fdemon-dap/src/adapter/events.rs` | Clear `evaluate_name_map` in `on_resume()` and `on_hot_restart()` |
| `crates/fdemon-dap/src/adapter/variables.rs` | Added `instance_ref_to_variable_with_eval_name()` private method; updated public `instance_ref_to_variable()` to delegate to it with `None`; updated all internal callers in `get_scope_variables` (locals, exceptions) and `get_globals_variables` to pass evaluate_name; updated `expand_object()` signature with `parent_evaluate_name` parameter and child expression construction; updated `handle_variables` to look up parent evaluate_name and pass it to `expand_object` |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Added `mod evaluate_name` module declaration |
| `crates/fdemon-dap/src/adapter/tests/evaluate_name.rs` | NEW — 11 unit tests covering all acceptance criteria |

### Notable Decisions/Tradeoffs

1. **Separate HashMap vs. extending VariableRef::Object**: Chose `HashMap<i64, String>` (keyed by variable_reference i64) instead of adding `evaluate_name: Option<String>` to `VariableRef::Object`. This avoided breaking all existing test code that uses struct-literal `VariableRef::Object { isolate_id, object_id }` syntax, which would have required modifying existing test files (forbidden by task constraints).

2. **New private method vs. changed public signature**: Added `instance_ref_to_variable_with_eval_name()` as a private method rather than adding a parameter to the existing public `instance_ref_to_variable()`. This preserved all existing 3-arg call sites in tests and handlers.rs unchanged.

3. **Map key kind detection for evaluate_name**: The map expansion path inspects the `kind` field of the key InstanceRef (`"String"` → `parent["key"]`, `"Int"` → `parent[n]`) to produce appropriate Dart expressions per the DAP spec rules.

### Testing Performed

- `cargo check -p fdemon-dap` - Passed
- `cargo test -p fdemon-dap` - Passed (656 tests, 11 new)
- `cargo test --workspace --lib` - Passed (3490+ tests across all crates)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Passed

### Risks/Limitations

1. **Exception children**: When expanding an exception object's fields (via `expand_object`), the children will receive `"$_threadException.fieldName"` as evaluate_name — this is correct behavior per the task notes, and happens automatically because the exception root is stored in `evaluate_name_map` with `"$_threadException"`.

2. **Globals as evaluated expressions**: Globals receive just their field name (e.g., `"counter"`), not a qualified path. This matches the notes — global statics are accessible by name in Dart's evaluation context.
