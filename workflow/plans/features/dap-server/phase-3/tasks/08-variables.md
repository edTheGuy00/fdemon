## Task: Variables and Object Expansion

**Objective**: Implement the `variables` request handler with lazy object expansion. Map VM Service `Instance` objects to DAP `Variable` objects, handling primitives inline and providing drill-down references for complex types.

**Depends on**: 07-stack-traces-and-scopes

**Estimated Time**: 4-5 hours

### Scope

- `crates/fdemon-dap/src/adapter/stack.rs` — Variables handler, object expansion logic
- `crates/fdemon-dap/src/adapter/mod.rs` — Wire to dispatch

### Details

#### `variables` Handler

```rust
impl<B: DebugBackend> DapAdapter<B> {
    pub async fn handle_variables(&mut self, request: &DapRequest) -> DapResponse {
        let args: VariablesArguments = parse_args(request)?;

        let var_ref = match self.var_store.lookup(args.variables_reference) {
            Some(vr) => vr.clone(),
            None => return DapResponse::error(request, "Invalid variables reference (stale)"),
        };

        let variables = match var_ref {
            VariableRef::Scope { frame_index, scope_kind } => {
                self.get_scope_variables(frame_index, scope_kind).await
            }
            VariableRef::Object { isolate_id, object_id } => {
                self.expand_object(&isolate_id, &object_id, args.start, args.count).await
            }
        };

        match variables {
            Ok(vars) => {
                let body = serde_json::json!({ "variables": vars });
                DapResponse::success(request, Some(body))
            }
            Err(e) => DapResponse::error(request, format!("Failed to get variables: {}", e)),
        }
    }
}
```

#### Scope Variables (Locals)

When a scope is expanded, fetch the variables from the VM Service frame:

```rust
async fn get_scope_variables(
    &mut self,
    frame_index: i32,
    scope_kind: ScopeKind,
) -> Result<Vec<DapVariable>, String> {
    let frame_ref = self.frame_store.lookup_by_index(frame_index)
        .ok_or("Frame not found")?;
    let isolate_id = &frame_ref.isolate_id;

    match scope_kind {
        ScopeKind::Locals => {
            // Get the stack again to access frame variables
            let stack = self.backend.get_stack(isolate_id, Some(frame_index + 1)).await?;
            let frame = stack.get("frames")
                .and_then(|f| f.as_array())
                .and_then(|arr| arr.get(frame_index as usize))
                .ok_or("Frame index out of bounds")?;

            let vars = frame.get("vars")
                .and_then(|v| v.as_array())
                .map(|arr| arr.as_slice())
                .unwrap_or(&[]);

            let mut result = Vec::new();
            for var in vars {
                let name = var.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                let value = var.get("value").unwrap_or(&serde_json::Value::Null);
                result.push(self.instance_ref_to_variable(name, value, isolate_id));
            }
            Ok(result)
        }
        ScopeKind::Globals => {
            // Globals are expensive — return empty for now, Phase 4 adds full support
            Ok(Vec::new())
        }
    }
}
```

#### Instance-to-Variable Mapping

The core mapping from Dart VM `InstanceRef` → DAP `Variable`:

```rust
/// Convert a VM Service InstanceRef to a DAP Variable.
fn instance_ref_to_variable(
    &mut self,
    name: &str,
    instance_ref: &serde_json::Value,
    isolate_id: &str,
) -> DapVariable {
    let kind = instance_ref.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    let class_name = instance_ref.get("class")
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str());
    let value_as_string = instance_ref.get("valueAsString")
        .and_then(|v| v.as_str());
    let obj_id = instance_ref.get("id")
        .and_then(|i| i.as_str());

    match kind {
        // ── Primitives: inline value, no expansion ───────────────────
        "Null" => DapVariable {
            name: name.to_string(),
            value: "null".to_string(),
            type_field: Some("Null".to_string()),
            variables_reference: 0,
            ..Default::default()
        },

        "Bool" => DapVariable {
            name: name.to_string(),
            value: value_as_string.unwrap_or("false").to_string(),
            type_field: Some("bool".to_string()),
            variables_reference: 0,
            ..Default::default()
        },

        "Int" | "Double" => DapVariable {
            name: name.to_string(),
            value: value_as_string.unwrap_or("0").to_string(),
            type_field: Some(kind.to_lowercase()),
            variables_reference: 0,
            ..Default::default()
        },

        "String" => {
            let value = value_as_string.map(|s| format!("\"{}\"", s))
                .unwrap_or_else(|| "\"\"".to_string());
            DapVariable {
                name: name.to_string(),
                value,
                type_field: Some("String".to_string()),
                variables_reference: 0,
                ..Default::default()
            }
        }

        // ── Collections: expandable ──────────────────────────────────
        "List" | "Map" | "Set" | "Uint8ClampedList" | "Uint8List"
        | "Int32List" | "Float64List" => {
            let length = instance_ref.get("length")
                .and_then(|l| l.as_i64())
                .unwrap_or(0);
            let type_name = class_name.unwrap_or(kind);
            let value = format!("{} (length: {})", type_name, length);

            let var_ref = if let Some(id) = obj_id {
                self.var_store.allocate(VariableRef::Object {
                    isolate_id: isolate_id.to_string(),
                    object_id: id.to_string(),
                })
            } else {
                0
            };

            DapVariable {
                name: name.to_string(),
                value,
                type_field: Some(type_name.to_string()),
                variables_reference: var_ref,
                indexed_variables: Some(length),
                ..Default::default()
            }
        }

        // ── Plain instances: expandable via fields ───────────────────
        "PlainInstance" | "Closure" | "RegExp" | "Type" | "StackTrace" => {
            let type_name = class_name.unwrap_or(kind);
            let value = value_as_string
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{} instance", type_name));

            let var_ref = if let Some(id) = obj_id {
                self.var_store.allocate(VariableRef::Object {
                    isolate_id: isolate_id.to_string(),
                    object_id: id.to_string(),
                })
            } else {
                0
            };

            DapVariable {
                name: name.to_string(),
                value,
                type_field: Some(type_name.to_string()),
                variables_reference: var_ref,
                ..Default::default()
            }
        }

        // ── Fallback ─────────────────────────────────────────────────
        _ => DapVariable {
            name: name.to_string(),
            value: value_as_string.unwrap_or("<unknown>").to_string(),
            type_field: class_name.map(|s| s.to_string()),
            variables_reference: 0,
            ..Default::default()
        },
    }
}
```

#### Object Expansion

When a user clicks to expand a variable (collection or instance), the IDE sends a `variables` request with the `variablesReference` that was allocated for that object:

```rust
async fn expand_object(
    &mut self,
    isolate_id: &str,
    object_id: &str,
    start: Option<i64>,
    count: Option<i64>,
) -> Result<Vec<DapVariable>, String> {
    let obj = self.backend.get_object(isolate_id, object_id, start, count).await?;
    let obj_type = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match obj_type {
        "Instance" => {
            let kind = obj.get("kind").and_then(|k| k.as_str()).unwrap_or("");
            match kind {
                "List" | "Uint8List" | "Uint8ClampedList" | "Int32List" | "Float64List" => {
                    // Expand list elements
                    let elements = obj.get("elements")
                        .and_then(|e| e.as_array())
                        .map(|a| a.as_slice())
                        .unwrap_or(&[]);
                    let offset = start.unwrap_or(0);

                    Ok(elements.iter().enumerate().map(|(i, elem)| {
                        let index = offset + i as i64;
                        self.instance_ref_to_variable(&format!("[{}]", index), elem, isolate_id)
                    }).collect())
                }

                "Map" => {
                    // Expand map associations
                    let associations = obj.get("associations")
                        .and_then(|a| a.as_array())
                        .map(|a| a.as_slice())
                        .unwrap_or(&[]);

                    Ok(associations.iter().enumerate().map(|(i, assoc)| {
                        let key = assoc.get("key")
                            .and_then(|k| k.get("valueAsString"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        let value = assoc.get("value").unwrap_or(&serde_json::Value::Null);
                        self.instance_ref_to_variable(&format!("[{}]", key), value, isolate_id)
                    }).collect())
                }

                _ => {
                    // Expand instance fields
                    let fields = obj.get("fields")
                        .and_then(|f| f.as_array())
                        .map(|a| a.as_slice())
                        .unwrap_or(&[]);

                    Ok(fields.iter().map(|field| {
                        let name = field.get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("?");
                        let value = field.get("value").unwrap_or(&serde_json::Value::Null);
                        self.instance_ref_to_variable(name, value, isolate_id)
                    }).collect())
                }
            }
        }
        _ => Ok(Vec::new()),
    }
}
```

### Acceptance Criteria

1. Locals scope shows all variables from the current frame
2. Primitive types (null, bool, int, double, String) display inline values with `variablesReference: 0`
3. Collections (List, Map, Set) display type and length, with `variablesReference > 0` for expansion
4. Plain instances display class name, with `variablesReference > 0` for field expansion
5. Expanding a List shows indexed elements `[0]`, `[1]`, ...
6. Expanding a Map shows keyed entries `[key1]`, `[key2]`, ...
7. Expanding a PlainInstance shows named fields
8. Nested expansion works (expanding a field that is itself a complex object)
9. Stale variable references return clear error messages
10. `start` and `count` parameters work for large collections
11. String values are displayed with quotes
12. Helix's flat variable popup renders correctly (no pagination dependency)

### Testing

```rust
#[test]
fn test_primitive_null_no_expansion() {
    let var = instance_ref_to_variable("x", &json!({"kind": "Null"}), "i/1");
    assert_eq!(var.value, "null");
    assert_eq!(var.variables_reference, 0);
}

#[test]
fn test_string_quoted() {
    let var = instance_ref_to_variable("name", &json!({
        "kind": "String", "valueAsString": "hello"
    }), "i/1");
    assert_eq!(var.value, "\"hello\"");
}

#[test]
fn test_list_shows_length_and_is_expandable() {
    let var = instance_ref_to_variable("items", &json!({
        "kind": "List", "length": 3, "id": "objects/1",
        "class": {"name": "List"}
    }), "i/1");
    assert!(var.value.contains("length: 3"));
    assert!(var.variables_reference > 0);
    assert_eq!(var.indexed_variables, Some(3));
}

#[test]
fn test_plain_instance_expandable() {
    let var = instance_ref_to_variable("widget", &json!({
        "kind": "PlainInstance", "id": "objects/2",
        "class": {"name": "Container"}
    }), "i/1");
    assert!(var.value.contains("Container"));
    assert!(var.variables_reference > 0);
}
```

### Notes

- **Guard against runaway expansion**: Limit nested expansion depth implicitly by the VM Service's `getObject` response size. Do not recursively expand objects.
- **Helix limitation**: Variable display is a flat popup, not a tree. Deeply nested variables will display as expandable but the UX is limited. This is a Helix limitation, not an fdemon issue.
- **Zed support**: Zed has a full Variables panel with tree expansion — our implementation supports this.
- **Performance**: `getObject` calls are async and may be slow for large objects. The `expensive: true` flag on the Globals scope tells IDEs to defer loading until explicitly requested.
- The `evaluate_name` field on `DapVariable` enables "set value" in some IDEs — leave it as `None` for Phase 3.

---

## Completion Summary

**Status:** Not Started
