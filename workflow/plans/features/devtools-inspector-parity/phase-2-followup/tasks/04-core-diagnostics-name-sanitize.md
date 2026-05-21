## Task: Sanitize `DiagnosticsNode` string fields against ANSI codes

**Objective**: Add `deserialize_with = "deserialize_sanitized_option_string"` to `DiagnosticsNode.name` (M4, the rendering gap) and to four additional string fields as defense-in-depth coverage (m9): `level`, `node_type`, `style`, `value_id`.

**Depends on**: None

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/widget_tree.rs`

**Files Read (Dependencies):**
- `crates/fdemon-core/src/ansi.rs` — `strip_ansi_codes()` (used by `deserialize_sanitized_option_string`)
- `workflow/reviews/features/devtools-inspector-parity/phase-2/REVIEW.md` — M4, m9 findings

### Details

#### Background

Phase 2 added `deserialize_sanitized_option_string` to `DiagnosticsNode.property_type` (Phase 2 task 01, cross-cutting constraint #6). However:

- `DiagnosticsNode.name` is rendered directly to the terminal buffer via `buf.set_string()` in `properties_tab.rs:235` and `render_object_tab.rs:223`. It has no sanitization, so adversarial or malformed VM Service responses with ANSI sequences in property names can corrupt terminal state.
- Other string fields (`level`, `node_type`, `style`, `value_id`) are not currently rendered but are present in the struct and could be added to renderers in future phases. Sanitizing them now is a one-line-per-field defense-in-depth change.

`object_id` and `location_id` are intentionally excluded — they're internal opaque tokens, not user-facing strings.

#### 1. Locate the `DiagnosticsNode` struct definition

`crates/fdemon-core/src/widget_tree.rs:~50-110` defines `DiagnosticsNode`. Phase 2 task 01 set the precedent for `property_type` at lines 104-109:

```rust
#[serde(
    default,
    rename = "propertyType",
    deserialize_with = "deserialize_sanitized_option_string"
)]
pub property_type: Option<String>,
```

#### 2. Add the same attribute to five additional fields

For each of the five fields (`name`, `level`, `node_type`, `style`, `value_id`):
- Field must be typed as `Option<String>` for `deserialize_sanitized_option_string` to apply.
- Add `deserialize_with = "deserialize_sanitized_option_string"` to the existing `#[serde(...)]` attribute, preserving the existing `default` and `rename` parts.

**Pattern:**

```rust
// BEFORE
#[serde(default, rename = "valueId")]
pub value_id: Option<String>,

// AFTER
#[serde(
    default,
    rename = "valueId",
    deserialize_with = "deserialize_sanitized_option_string"
)]
pub value_id: Option<String>,
```

Apply consistently to:
- `name` (the M4 critical fix — renderers consume this directly)
- `level` (currently used in `filter_and_sort_by_level` — string match `Some("hidden")`, etc. — sanitizing it won't break the match since ANSI strings won't match those literal values)
- `node_type` (defense-in-depth)
- `style` (defense-in-depth)
- `value_id` (defense-in-depth — IDs are opaque but could theoretically be displayed in debug output)

#### 3. Verify type compatibility

If any of the five fields is NOT `Option<String>`, this approach doesn't apply directly. Inspect the actual field types:
- If a field is `String` (not optional), use `deserialize_sanitized_string` instead.
- If a field is `Option<String>`, use `deserialize_sanitized_option_string`.

Both helpers already exist in `widget_tree.rs` and are documented around lines 1007–1025 per Phase 1.5.

### Acceptance Criteria

1. `DiagnosticsNode.name` deserializes with ANSI codes stripped from the input JSON.
2. The four defense-in-depth fields (`level`, `node_type`, `style`, `value_id`) similarly strip ANSI codes at deserialize time.
3. Existing `DiagnosticsNode` tests in `widget_tree.rs` continue to pass.
4. New tests verify ANSI sanitization on at least `name` and one other field.
5. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

Add these tests to the existing `#[cfg(test)] mod tests` block in `widget_tree.rs`, near the existing `property_type` sanitization tests added in Phase 2 task 01:

```rust
#[test]
fn diagnostics_node_name_strips_ansi_codes() {
    let json = serde_json::json!({
        "description": "Container",
        "name": "\u{001b}[31mwidget_name\u{001b}[0m"
    });
    let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
    assert_eq!(node.name.as_deref(), Some("widget_name"));
}

#[test]
fn diagnostics_node_name_passes_clean_strings() {
    let json = serde_json::json!({
        "description": "Container",
        "name": "padding"
    });
    let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
    assert_eq!(node.name.as_deref(), Some("padding"));
}

#[test]
fn diagnostics_node_level_strips_ansi_codes() {
    let json = serde_json::json!({
        "description": "Container",
        "level": "\u{001b}[33mfine\u{001b}[0m"
    });
    let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
    assert_eq!(node.level.as_deref(), Some("fine"));
    // Verify that the level filter still works after sanitization
    assert!(matches!(node.level.as_deref(), Some("fine")));
}

#[test]
fn diagnostics_node_value_id_strips_ansi_codes() {
    let json = serde_json::json!({
        "description": "Container",
        "valueId": "\u{001b}[36mobjects/42\u{001b}[0m"
    });
    let node: DiagnosticsNode = serde_json::from_value(json).unwrap();
    assert_eq!(node.value_id.as_deref(), Some("objects/42"));
}
```

### Notes

- The `\u{001b}` escape is the ESC character (0x1B); used inside `serde_json::json!` macro since Rust string literals require this form.
- Sanitizing `level` does NOT break the `filter_and_sort_by_level` logic — that function compares `level.as_deref()` against literal strings `"hidden"` and `"fine"`. A clean Flutter response will not contain ANSI bytes in `level` to begin with; the change only protects against adversarial / malformed input.
- The defense-in-depth fields chosen (`level`, `node_type`, `style`, `value_id`) cover all known string fields currently in the struct. If a future field is added (e.g. `description_short`), it should follow the same sanitization pattern.
- The `description` field already uses `deserialize_sanitized_string` (Phase 1.5) — no change needed.
- The `property_type` field already uses `deserialize_sanitized_option_string` (Phase 2 task 01) — no change needed.
- If any field on `DiagnosticsNode` does not actually exist (the review listed `style`, but verify it's present), skip that field. The file is the source of truth.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/widget_tree.rs` | Added `deserialize_sanitized_option_string` to `name`, `level`, `node_type`, `style`, `value_id` fields; added 6 new tests |

### Notable Decisions/Tradeoffs

1. **`node_type` already had `rename = "type"`**: Added `default` alongside `deserialize_with` to keep the multi-attribute serde block consistent with the pattern used by `property_type`. The JSON key `"type"` is preserved via the explicit rename.
2. **`level`, `name`, `style`, `value_id` had no serde attributes**: Each got `#[serde(default, deserialize_with = "deserialize_sanitized_option_string")]`. The `rename_all = "camelCase"` at the struct level still handles `valueId` mapping automatically — no explicit `rename` needed.
3. **`object_id` and `location_id` excluded intentionally**: As specified — internal opaque tokens that are never rendered.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (2367 + 445 + 800 + 842 + 1112 + others; 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **`level` sanitization is safe**: `filter_and_sort_by_level` compares against clean literals like `"hidden"`, `"fine"`, `"off"`. Sanitized ANSI-free strings still match correctly. Adversarial inputs with ANSI sequences would have failed those matches anyway.
2. **Defense-in-depth only for non-rendered fields**: `node_type`, `style`, `value_id` are not currently rendered to the terminal, but sanitizing them now prevents future regressions if rendering is added.
