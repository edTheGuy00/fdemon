## Task: Variables.rs Correctness and Cleanup Fixes

**Objective**: Fix evaluateName map key escaping (M6), remove the pointless self-assignment (L4), and eliminate unnecessary `.clone()` calls (L5) in `variables.rs`.

**Depends on**: None

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/variables.rs`: Three fixes

**Files Read (Dependencies):**
- None

### Details

#### Fix 1: M6 — Escape special characters in evaluateName map string keys (line 1313)

Current code:
```rust
"String" => format!("{}[\"{}\"]", p, key_str),
```

`key_str` comes from the VM's `valueAsString` field and can contain `"`, `\`, `$`, `\n`, etc. The generated expression becomes invalid Dart (e.g., `myMap["hello "world""]`).

Add a Dart string escaping helper and apply it:

```rust
/// Escape a string for use inside a Dart double-quoted string literal.
fn escape_dart_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '$' => escaped.push_str("\\$"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
```

Then update the map key formatting:

```rust
"String" => format!("{}[\"{}\"]", p, escape_dart_string(key_str)),
```

Also check the catch-all `_` branch at line 1315 — if it also interpolates `key_str` into a quoted context, apply the same escaping.

#### Fix 2: L4 — Remove pointless self-assignment (line 714)

```rust
var.name = var.name.clone(); // ensure owned
```

`var.name` is already a `String` (owned). This line does nothing. Remove it.

#### Fix 3: L5 — Remove unnecessary `.clone()` on owned values (lines 362, 377)

In the `handle_variables` match:

```rust
// Current (line 362):
self.expand_object(
    &isolate_id.clone(),
    &object_id.clone(),
    ...
)

// Fixed:
self.expand_object(
    &isolate_id,
    &object_id,
    ...
)
```

`isolate_id` and `object_id` are destructured from owned `VariableRef::Object` — they are already `String`s. The `.clone()` before `&` is redundant. Apply the same fix to lines 377–381 for `VariableRef::GetterEval`.

### Acceptance Criteria

1. `evaluateName` for map entries with string keys containing `"`, `\`, `$`, or `\n` produces valid Dart expressions
2. No pointless self-assignment at line 714
3. No unnecessary `.clone()` at lines 362, 377
4. Existing tests pass: `cargo test -p fdemon-dap`
5. `cargo clippy -p fdemon-dap` clean

### Testing

```rust
#[test]
fn test_escape_dart_string_quotes() {
    assert_eq!(escape_dart_string(r#"hello "world""#), r#"hello \"world\""#);
}

#[test]
fn test_escape_dart_string_backslash() {
    assert_eq!(escape_dart_string(r"path\to\file"), r"path\\to\\file");
}

#[test]
fn test_escape_dart_string_dollar() {
    assert_eq!(escape_dart_string("cost: $100"), r"cost: \$100");
}

#[test]
fn test_escape_dart_string_newline() {
    assert_eq!(escape_dart_string("line1\nline2"), r"line1\nline2");
}

#[tokio::test]
async fn test_map_evaluate_name_with_special_chars_in_key() {
    // Mock a map with a key containing quotes
    // Expand the map variables
    // Assert the evaluateName is properly escaped: myMap["hello \"world\""]
}
```

### Notes

- The `escape_dart_string` helper should be a private function in `variables.rs`. If it's needed elsewhere later, it can be moved to a shared location.
- The catch-all `_` branch for map keys uses `key_str` in a toString-style display, not necessarily in a quoted expression context. Check whether it also needs escaping.
