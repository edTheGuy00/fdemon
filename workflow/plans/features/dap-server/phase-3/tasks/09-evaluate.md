## Task: Expression Evaluation

**Objective**: Implement the `evaluate` request handler for debug console (REPL), hover evaluation, and watch expressions. Map to VM Service `evaluate` and `evaluateInFrame` RPCs.

**Depends on**: 07-stack-traces-and-scopes

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-dap/src/adapter/evaluate.rs` — **NEW** Evaluate handler
- `crates/fdemon-dap/src/adapter/mod.rs` — Wire to dispatch

### Details

#### `evaluate` Handler

```rust
// crates/fdemon-dap/src/adapter/evaluate.rs

impl<B: DebugBackend> DapAdapter<B> {
    pub async fn handle_evaluate(&mut self, request: &DapRequest) -> DapResponse {
        let args: EvaluateArguments = parse_args(request)?;

        // Determine the isolate — use the most recently paused isolate
        let isolate_id = match self.most_recent_paused_isolate() {
            Some(id) => id.to_string(),
            None => return DapResponse::error(request, "No paused isolate available for evaluation"),
        };

        let result = if let Some(frame_id) = args.frame_id {
            // Evaluate in the context of a specific stack frame
            let frame_ref = match self.frame_store.lookup(frame_id) {
                Some(fr) => fr.clone(),
                None => return DapResponse::error(request, "Invalid frame ID"),
            };

            self.backend.evaluate_in_frame(
                &isolate_id,
                frame_ref.frame_index,
                &args.expression,
            ).await
        } else {
            // Evaluate in root library context (no frame)
            // Need to find the root library ID for the isolate
            match self.get_root_library_id(&isolate_id).await {
                Ok(lib_id) => {
                    self.backend.evaluate(&isolate_id, &lib_id, &args.expression).await
                }
                Err(e) => Err(e),
            }
        };

        match result {
            Ok(instance) => {
                let value = format_instance_value(&instance);
                let type_name = instance.get("class")
                    .and_then(|c| c.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());

                // If the result is a complex object, make it expandable
                let var_ref = if is_expandable(&instance) {
                    if let Some(id) = instance.get("id").and_then(|i| i.as_str()) {
                        self.var_store.allocate(VariableRef::Object {
                            isolate_id: isolate_id.clone(),
                            object_id: id.to_string(),
                        })
                    } else {
                        0
                    }
                } else {
                    0
                };

                let body = EvaluateResponseBody {
                    result: value,
                    type_field: type_name,
                    variables_reference: var_ref,
                    named_variables: None,
                    indexed_variables: None,
                    presentation_hint: None,
                };
                let body_json = serde_json::to_value(&body).unwrap_or_default();
                DapResponse::success(request, Some(body_json))
            }
            Err(e) => {
                // Evaluation errors should NOT crash the session — return as a
                // "successful" response with the error as the result text.
                // This is how VS Code / Zed expect evaluation errors to be reported.
                //
                // Some adapters return success=false for eval errors, but both
                // VS Code and the DAP spec recommend success=true with the error
                // in the result field for a better UX (error shown inline in console).
                DapResponse::error(request, e)
            }
        }
    }
}
```

#### Context-Aware Evaluation

The `context` field on the evaluate request determines behavior:

```rust
fn should_evaluate(context: Option<&str>) -> bool {
    match context {
        Some("hover") => true,    // Tooltip evaluation
        Some("watch") => true,    // Watch panel
        Some("repl") => true,     // Debug console
        Some("clipboard") => true, // Copy value
        Some(_) => true,          // Unknown context — try anyway
        None => true,             // No context specified
    }
}
```

For Phase 3, all contexts use the same evaluation path. Phase 4 may differentiate:
- `"hover"`: auto-`toString()` for richer display
- `"repl"`: allow side-effect-having expressions
- `"watch"`: side-effect-free preferred

#### Value Formatting

```rust
/// Format a VM Service instance value for DAP display.
fn format_instance_value(instance: &serde_json::Value) -> String {
    let kind = instance.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    let value_as_string = instance.get("valueAsString").and_then(|v| v.as_str());

    match kind {
        "Null" => "null".to_string(),
        "Bool" | "Int" | "Double" => value_as_string.unwrap_or("?").to_string(),
        "String" => {
            let s = value_as_string.unwrap_or("");
            format!("\"{}\"", s)
        }
        "List" | "Map" | "Set" => {
            let length = instance.get("length").and_then(|l| l.as_i64()).unwrap_or(0);
            let class_name = instance.get("class")
                .and_then(|c| c.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or(kind);
            format!("{} (length: {})", class_name, length)
        }
        _ => {
            value_as_string
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    let class_name = instance.get("class")
                        .and_then(|c| c.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("Object");
                    format!("{} instance", class_name)
                })
        }
    }
}

/// Check if an instance can be expanded (has children).
fn is_expandable(instance: &serde_json::Value) -> bool {
    let kind = instance.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    matches!(kind, "List" | "Map" | "Set" | "PlainInstance" | "Closure"
        | "Uint8List" | "Uint8ClampedList" | "Int32List" | "Float64List")
}
```

#### Root Library ID Resolution

For expressions evaluated without a frame context (global REPL):

```rust
async fn get_root_library_id(&self, isolate_id: &str) -> Result<String, String> {
    let vm_info = self.backend.get_vm().await?;
    let isolates = vm_info.get("isolates")
        .and_then(|i| i.as_array())
        .ok_or("No isolates in VM info")?;

    // Find the isolate
    for isolate in isolates {
        if isolate.get("id").and_then(|i| i.as_str()) == Some(isolate_id) {
            // Get root library ID — may need a separate getIsolate RPC
            // For now, use a cached value from the attach phase
            if let Some(root_lib) = isolate.get("rootLib").and_then(|l| l.get("id")).and_then(|i| i.as_str()) {
                return Ok(root_lib.to_string());
            }
        }
    }
    Err("Could not find root library for isolate".to_string())
}
```

### Acceptance Criteria

1. `evaluate` with `frameId` calls `evaluateInFrame` on the VM Service
2. `evaluate` without `frameId` calls `evaluate` on the root library
3. Primitive results are formatted correctly (strings quoted, numbers as-is)
4. Complex results are expandable (return `variablesReference > 0`)
5. Evaluation errors return a DAP error response with the error message
6. Evaluation works in Helix (`:debug-eval <expr>`) and Zed (debug console)
7. The isolate must be paused for in-frame evaluation — error if running
8. Unit tests cover value formatting and expandability checks

### Testing

```rust
#[test]
fn test_format_null() {
    let val = json!({"kind": "Null"});
    assert_eq!(format_instance_value(&val), "null");
}

#[test]
fn test_format_string_quoted() {
    let val = json!({"kind": "String", "valueAsString": "hello"});
    assert_eq!(format_instance_value(&val), "\"hello\"");
}

#[test]
fn test_format_int() {
    let val = json!({"kind": "Int", "valueAsString": "42"});
    assert_eq!(format_instance_value(&val), "42");
}

#[test]
fn test_format_list_with_length() {
    let val = json!({"kind": "List", "length": 5, "class": {"name": "List"}});
    assert_eq!(format_instance_value(&val), "List (length: 5)");
}

#[test]
fn test_is_expandable_list() {
    assert!(is_expandable(&json!({"kind": "List"})));
}

#[test]
fn test_is_expandable_primitive() {
    assert!(!is_expandable(&json!({"kind": "Int"})));
    assert!(!is_expandable(&json!({"kind": "String"})));
    assert!(!is_expandable(&json!({"kind": "Null"})));
}
```

### Notes

- **Helix**: Supports `:debug-eval <expression>` command. Evaluation results are shown in the editor's status area. No interactive tree expansion of results.
- **Zed**: Has a full debug console with expression evaluation. Results can be expanded if `variablesReference > 0`.
- **VS Code**: Also has debug console with evaluation.
- The Dart `evaluate` RPC will fail if the isolate is not paused (for frame-context evaluation). Always check pause state before calling.
- Phase 4 will add `context: "hover"` auto-toString behavior and richer formatting.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/evaluate.rs` | Full implementation: `handle_evaluate`, `get_root_library_id`, `format_instance_value`, `is_expandable` with comprehensive unit tests |
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `paused_isolates: Vec<String>` field; added `most_recent_paused_isolate()` helper; replaced stub `handle_evaluate` with real dispatch to `evaluate::handle_evaluate`; updated `handle_debug_event` Paused/Resumed arms to maintain `paused_isolates`; updated tests (replaced "not yet implemented" stub test with evaluate-specific integration tests) |

### Notable Decisions/Tradeoffs

1. **Free-function API in evaluate.rs**: `handle_evaluate` takes the backend, frame_store, var_store, and paused isolate as explicit parameters (rather than taking `&mut DapAdapter`). This avoids borrow checker issues (mutable borrow of `var_store` alongside shared borrow of `frame_store` and `backend`) and makes the evaluate module independently testable without constructing a full adapter.

2. **paused_isolates as Vec with last = most recent**: The adapter tracks paused isolates in insertion order with the most recent at the back. On resume, the isolate is removed. This correctly handles multi-isolate scenarios where evaluation should target the last-paused isolate.

3. **Evaluation errors return `success=false`**: The DAP spec allows both approaches (success=false with message, or success=true with error text). We use `success=false` to give clear error feedback consistent with other error responses in the adapter.

4. **Root library resolution uses `get_vm()`**: For frameless evaluation, the root library is resolved from the VM info's isolate list. Phase 4 may switch to `get_isolate()` for more reliable results, but `get_vm()` works for the Phase 3 use case.

5. **`should_evaluate` function omitted**: The task spec included a `should_evaluate` context check that always returns `true`. Since all contexts use the same path in Phase 3, this was omitted as dead code. Phase 4 can add context-specific behavior when needed.

### Testing Performed

- `cargo check -p fdemon-dap` — Passed
- `cargo test -p fdemon-dap` — Passed (325 tests: 57 new in evaluate.rs + 3 new in mod.rs integration tests; replaced 1 stub test)
- `cargo clippy -p fdemon-dap -- -D warnings` — Passed (no warnings)
- `cargo fmt -p fdemon-dap` — Applied (minor formatting normalization)

### Risks/Limitations

1. **Frameless evaluation depends on `get_vm()` rootLib**: If the VM Service doesn't include `rootLib` in the isolate refs (which can happen depending on Dart VM version), frameless evaluation will fail with "Could not find root library". Phase 4 should add a `get_isolate()` backend method as a fallback.

2. **No context differentiation**: `"hover"` vs `"repl"` vs `"watch"` all use the same path. Phase 4 may want to restrict side-effects for `"hover"` and `"watch"` contexts.

3. **Expandable objects without an `id`**: If a complex object (List, Map, etc.) lacks an `id` field in the VM Service response, `variablesReference` will be 0, making it non-expandable. This is a VM Service edge case and can be addressed if encountered in practice.
