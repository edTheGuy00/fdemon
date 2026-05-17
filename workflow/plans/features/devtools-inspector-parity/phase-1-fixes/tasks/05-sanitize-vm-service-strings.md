## Task: Sanitise VM Service Strings at the Deserialize Boundary

**Objective**: Apply `strip_ansi_codes()` to user-rendered string fields on `DiagnosticsNode` (`description`, `creation_location.file`, `creation_location.name`) at the serde deserialize boundary so ANSI escape bytes from the Dart VM Service cannot leak through to the terminal.

**Depends on**: 04 (same file `widget_tree.rs` — must run sequentially after 04 lands)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/widget_tree.rs` — custom serde deserializer (or `#[serde(deserialize_with = ...)]` attribute) on string fields that flow to the terminal.

**Files Read (Dependencies):**
- `crates/fdemon-core/src/ansi.rs` — `pub fn strip_ansi_codes(input: &str) -> String` (line 95).
- `crates/fdemon-daemon/src/protocol.rs:380` — existing usage pattern in the daemon log layer.

### Review Items Resolved

- **M7** — No ANSI/control-character sanitisation on VM Service strings before terminal rendering

### Details

The cleanest insertion point is a custom serde deserializer that strips ANSI on the way in, so every downstream consumer (renderer, log, future serializer) sees clean data.

#### Option A — `deserialize_with` attribute (recommended)

Define a small helper at module scope:

```rust
fn deserialize_sanitized_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: String = serde::Deserialize::deserialize(deserializer)?;
    Ok(crate::ansi::strip_ansi_codes(&raw))
}

fn deserialize_sanitized_option_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(raw.map(|s| crate::ansi::strip_ansi_codes(&s)))
}
```

Apply on the relevant fields:

```rust
pub struct DiagnosticsNode {
    // ...
    #[serde(deserialize_with = "deserialize_sanitized_string", default)]
    pub description: String,
    // ...
}

pub struct CreationLocation {
    #[serde(deserialize_with = "deserialize_sanitized_string", default)]
    pub file: String,
    #[serde(deserialize_with = "deserialize_sanitized_option_string", default)]
    pub name: Option<String>,
    // ...
}
```

Audit the full struct: any other string field that ends up in the terminal (e.g. `level`, `runtime_type`, property values) should also be sanitised. The implementor should grep `DiagnosticsNode` field usages in `crates/fdemon-tui/` to find the comprehensive list — if it's only the three above, document that. If more fields qualify, sanitise all of them.

#### Tests

```rust
#[test]
fn deserialize_strips_ansi_escape_from_description() {
    let json = r#"{
        "description": "Container[31mRED[0m",
        "valueId": "node-1",
        "objectId": "obj-1",
        "type": "Container",
        "children": []
    }"#;
    let node: DiagnosticsNode = serde_json::from_str(json).unwrap();
    assert_eq!(node.description, "ContainerRED");
}

#[test]
fn deserialize_strips_ansi_from_creation_location_fields() {
    // similar — embed a CSI sequence in a file path
}

#[test]
fn deserialize_preserves_unicode_box_drawing() {
    // sanity: ensure that legitimate box-drawing chars in widget names survive
}
```

### Acceptance Criteria

1. `DiagnosticsNode.description` is sanitised at deserialize time.
2. `CreationLocation.file` and `CreationLocation.name` are sanitised at deserialize time.
3. Any other terminal-rendered string field on `DiagnosticsNode` is sanitised (audit + document the full list in the task's completion summary).
4. The `serde(default)` attribute is preserved on any field that already had it (don't regress the resilience to missing fields).
5. New test: ANSI escape in `description` is stripped on deserialize.
6. New test: ANSI escape in `creation_location.file` is stripped on deserialize.
7. Existing tests in `widget_tree.rs` continue to pass.
8. `cargo test -p fdemon-core` passes.
9. `cargo clippy -p fdemon-core --all-targets -- -D warnings` passes.

### Notes

- Do NOT add a `_raw` companion field for the unsanitised value. If a future consumer needs raw data, it can be added then — YAGNI today.
- The `strip_ansi_codes` function does more than ANSI escape stripping (it also strips backslash-prefixed box-drawing and trailing backslashes per its doc). For widget descriptions this is acceptable — any of those would be malformed input. Verify no Flutter widget legitimately ships those patterns in its `runtimeType` name (it shouldn't — widget names are Dart identifiers).
- Worktree note: this task is sequential with task 04 — both write `widget_tree.rs`.
- Wave: W2. Runs in parallel with task 06 (no shared write files between 05 and 06).

---

## Completion Summary

**Status:** Not Started
**Branch:** —

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
