## Task: Add `DaemonCommand::ServeDevTools` Variant

**Objective**: Extend `DaemonCommand` to emit the `daemon.devtools.serve` (or equivalent, per RESEARCH.md) JSON-RPC method to the Flutter daemon's stdin. Wire request-tracking if the response carries a result body.

**Depends on**: 00-research-daemon-devtools-rpc

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/commands.rs` (around lines 180-201): Add `DaemonCommand::ServeDevTools { request_id: Option<String> }`. Serialize to `{"id": "<request_id>", "method": "<verified-method-from-RESEARCH>", "params": {}}` (use the exact method name from RESEARCH.md).
- `crates/fdemon-daemon/src/commands.rs`: Add `DaemonCommand::ServeDevTools` to the `serialize` method's match arm.
- `crates/fdemon-daemon/src/commands.rs`: If `RequestTracker` is used for request/response correlation, register the request-id appropriately.

**Files Read (Dependencies):**
- `workflow/plans/bugs/browser-devtools-dds-registration/RESEARCH.md`: For the verified method name and request shape.
- `crates/fdemon-daemon/src/commands.rs`: Existing command variants for reference.

### Details

Use the existing pattern in `commands.rs`. Example (placeholder method name — replace with RESEARCH-verified):

```rust
pub enum DaemonCommand {
    // ... existing variants ...
    ServeDevTools {
        request_id: Option<String>,
    },
}

impl DaemonCommand {
    pub fn serialize(&self) -> String {
        match self {
            // ... existing ...
            DaemonCommand::ServeDevTools { request_id } => {
                let id = request_id.as_deref().unwrap_or("devtools-serve");
                json!({
                    "id": id,
                    "method": "daemon.devtools.serve",  // <-- replace with RESEARCH value
                    "params": {}
                }).to_string()
            }
        }
    }
}
```

If RESEARCH.md indicates the method takes parameters (e.g., `host`, `port` hints), add them as fields on the variant.

### Acceptance Criteria

1. `DaemonCommand::ServeDevTools` exists with the correct shape.
2. `.serialize()` emits a JSON string matching the contract verified in RESEARCH.md.
3. New unit test in `commands.rs` parses the serialized output back via `serde_json::from_str::<Value>` and asserts:
   - `method` equals the verified method name.
   - `params` is an object (possibly empty).
   - `id` is present.
4. `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` pass.
5. No existing `DaemonCommand` variants are altered.

### Testing

```rust
#[test]
fn serve_devtools_command_serializes_correctly() {
    let cmd = DaemonCommand::ServeDevTools { request_id: Some("test-1".to_string()) };
    let json: serde_json::Value = serde_json::from_str(&cmd.serialize()).unwrap();
    assert_eq!(json["method"], "daemon.devtools.serve");  // or verified value
    assert!(json["params"].is_object());
    assert_eq!(json["id"], "test-1");
}
```

### Notes

- Keep the variant `Clone + Debug` consistent with existing variants.
- The `request_id` field is needed to correlate this request with the response when `daemon.devtools.serve` returns a result (vs. a fire-and-forget event).

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/commands.rs` | Added `ServeDevTools { request_id: Option<String> }` variant to `DaemonCommand`; added match arm in `build()` emitting `"devtools.serve"`; added match arm in `description()`; added 3 unit tests |

### Notable Decisions/Tradeoffs

1. **Existing `build(id: u64)` API preserved**: The task example showed a `serialize()` method, but the codebase uses `build(id: u64)`. The new variant integrates into `build()` with the existing pattern. When `request_id` is `Some`, the string overrides the numeric id in the JSON; when `None`, the tracker-supplied numeric id is used.

2. **Method name corrected to `"devtools.serve"`**: RESEARCH.md confirmed the correct two-part `domain.method` notation. The task body erroneously listed `"daemon.devtools.serve"` in the example, which was updated in the test per the prompt instructions.

3. **Early `return` in `build()` for `ServeDevTools`**: The `request_id` override required producing a `Value::String` or `Value::Number` for the `id` field, which does not fit the existing `(method, params)` destructure pattern. An early `return` keeps the logic clean without refactoring the entire match.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-daemon` - Passed (746 tests, 3 new: `serve_devtools_command_serializes_correctly`, `serve_devtools_uses_numeric_id_when_request_id_is_none`, `serve_devtools_description`)
- `cargo test --workspace` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Flutter < 1.22.0 returns `-32601`**: Documented in RESEARCH.md. The variant is correct; callers that use it need to handle the error response. No mitigation needed at this layer.
