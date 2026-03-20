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
