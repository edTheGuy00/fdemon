## Task: Implement completions Request

**Objective**: Add the `completions` DAP request handler that provides auto-complete suggestions for the debug console. This enables IntelliSense-like behavior when typing expressions in the debug console REPL.

**Depends on**: 02-expand-backend-trait

**Estimated Time**: 3–5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Add `completions` to dispatch table with handler
- `crates/fdemon-dap/src/protocol/types.rs`: Add `supports_completions_request` field to `Capabilities` struct, set `Some(true)` in `fdemon_defaults()`

### Details

#### Strategy: Scope-based completions (conservative, accurate)

Rather than attempting partial expression evaluation (complex and error-prone), enumerate identifiers available in the current scope:

1. **Local variables** from `frame.vars`: names of all `BoundVariable` entries
2. **Library top-level names** from `getObject(rootLibId)`: function names, class names, top-level variable names
3. **Keywords**: `true`, `false`, `null`, `this`

#### Handler:

```rust
async fn handle_completions(&mut self, request: &DapRequest) -> DapResponse {
    let args = parse_args::<CompletionsArguments>(request);
    let text = &args.text;
    let column = args.column; // 1-based cursor position in text

    // Get the prefix the user is typing (text up to cursor)
    let prefix = &text[..((column - 1) as usize).min(text.len())];
    // Extract the last identifier fragment being typed
    let fragment = extract_last_identifier(prefix);

    let isolate_id = self.most_recent_isolate_id()
        .ok_or("No isolate available")?;

    let mut items: Vec<CompletionItem> = Vec::new();

    // 1. Local variables from current frame
    if let Some(frame_id) = args.frame_id {
        if let Some(frame_ref) = self.frame_store.lookup(frame_id) {
            let stack = self.backend.get_stack(&frame_ref.isolate_id, Some(frame_ref.frame_index + 1)).await?;
            if let Some(vars) = stack.get("frames")
                .and_then(|f| f.as_array())
                .and_then(|f| f.get(frame_ref.frame_index as usize))
                .and_then(|f| f.get("vars"))
                .and_then(|v| v.as_array()) {
                for var in vars {
                    if let Some(name) = var.get("name").and_then(|n| n.as_str()) {
                        if name.starts_with(fragment) || fragment.is_empty() {
                            items.push(CompletionItem {
                                label: name.to_string(),
                                type_field: Some("variable".to_string()),
                                sort_text: Some(format!("0_{}", name)), // locals first
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }
    }

    // 2. Dart keywords
    for kw in &["true", "false", "null", "this"] {
        if kw.starts_with(fragment) || fragment.is_empty() {
            items.push(CompletionItem {
                label: kw.to_string(),
                type_field: Some("keyword".to_string()),
                sort_text: Some(format!("2_{}", kw)),
                ..Default::default()
            });
        }
    }

    // 3. Library top-level names (if available and not too slow)
    // This is optional — can be deferred if it makes the response too slow

    // Deduplicate and limit
    items.truncate(50);

    DapResponse::success(request, json!({ "targets": items }))
}
```

#### Helper: `extract_last_identifier`

```rust
fn extract_last_identifier(text: &str) -> &str {
    // Find the last run of [a-zA-Z0-9_$] characters
    let end = text.len();
    let start = text.rfind(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
        .map(|i| i + 1)
        .unwrap_or(0);
    &text[start..end]
}
```

#### CompletionItem type:

Add to `protocol/types.rs`:
```rust
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_text: Option<String>,
}
```

### Acceptance Criteria

1. Debug console auto-complete suggests local variable names
2. Keywords (`true`, `false`, `null`, `this`) appear in suggestions
3. Suggestions filtered by the text fragment being typed
4. `supportsCompletionsRequest: true` in capabilities
5. Works without a frame context (suggests keywords only)
6. 8+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_completions_includes_locals() {
    // MockBackend: get_stack returns frame with vars: [name: "counter", name: "widget"]
    // Call completions with text "cou", column 4
    // Verify "counter" is in results
}

#[tokio::test]
async fn test_completions_includes_keywords() {
    // Call completions with text "tr", column 3
    // Verify "true" is in results
}

#[tokio::test]
async fn test_completions_empty_prefix_returns_all() {
    // Call completions with text "", column 1
    // Verify all locals + keywords returned
}
```

### Notes

- This is a major differentiator — neither the Dart DDS adapter nor Dart-Code implement `completions`. fdemon will be the only Dart DAP adapter with debug console autocomplete.
- The conservative approach (scope-only) guarantees accuracy — no false suggestions. A more sophisticated approach (parsing `obj.` for member access) can be added later.
- Library top-level name enumeration requires `get_isolate` → `rootLib` → `get_object(libraryId)` → `variables + functions + classes`. This adds latency but provides richer completions. Consider making it optional or cached.

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/protocol/types.rs` | Added `CompletionItem` struct, `CompletionsArguments` struct, `supports_completions_request` field on `Capabilities`, `supports_completions_request: Some(true)` in `fdemon_defaults()` |
| `crates/fdemon-dap/src/adapter/handlers.rs` | Added `"completions"` to dispatch table, implemented `handle_completions` method, added `extract_last_identifier` free helper function; added `CompletionItem` and `CompletionsArguments` to imports |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Registered `mod completions` test module |
| `crates/fdemon-dap/src/adapter/tests/completions.rs` | New file: 16 unit tests covering all acceptance criteria |

### Notable Decisions/Tradeoffs

1. **Handler uses `most_recent_paused_isolate` / `primary_isolate_id` pattern**: The task spec referenced a non-existent `most_recent_isolate_id()` method. I used the established pattern from other handlers (fall through: paused isolate → primary isolate) but since the completions handler doesn't need an isolate at the adapter level (it uses the frame's isolate_id directly), I omitted the isolate selection entirely. The `get_stack` call uses `frame_ref.isolate_id` which is already stored in the `FrameRef`.

2. **Graceful degradation on `get_stack` failure**: If `get_stack` fails (e.g., app resumed between frameId allocation and completions request), the handler skips locals but still returns keywords — consistent with the task spec's "works without frame context" requirement.

3. **`extract_last_identifier` is `pub(crate)`**: This allows testing it directly from the test module without making it fully public. Consistent with `parse_args` and other helpers in the same file.

4. **`CompletionItem` uses `#[derive(Default)]` and `Serialize` only (no `Deserialize`)**: The type is only ever constructed and serialized by the server, never deserialized from client input. This is correct per the DAP spec.

### Testing Performed

- `cargo check -p fdemon-dap` - Passed
- `cargo test -p fdemon-dap` - Passed (786 tests, 16 new completions tests all pass)
- `cargo clippy -p fdemon-dap -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Passed (no formatting changes needed beyond automatic sort)
- `cargo check --workspace` - Passed

### Risks/Limitations

1. **Library top-level names not implemented**: The task spec explicitly marks this as optional ("can be deferred if it makes the response too slow"). Not implementing it keeps response latency minimal. Can be added as a follow-up.

2. **No deduplication of locals**: If the same variable name appeared in multiple frames (unlikely but possible in edge cases), it would appear twice. The 50-item cap limits impact.
