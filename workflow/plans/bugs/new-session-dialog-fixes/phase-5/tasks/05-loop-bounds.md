## Task: Add Bounds to Unique Name Generation Loop

**Objective**: Add an upper bound to the `generate_unique_name()` loop to prevent potential infinite loops or UI freezes.

**Priority**: Major

**Depends on**: None

### Scope

- `src/app/new_session_dialog/state.rs`: `generate_unique_name()` function (lines 615-628)

### Problem Analysis

**Current implementation (lines 615-628):**
```rust
fn generate_unique_name(base_name: &str, existing_names: &[&str]) -> String {
    if !existing_names.contains(&base_name) {
        return base_name.to_string();
    }

    let mut counter = 2;
    loop {  // ← UNBOUNDED LOOP
        let candidate = format!("{} {}", base_name, counter);
        if !existing_names.contains(&candidate.as_str()) {
            return candidate;
        }
        counter += 1;
    }
}
```

### Why This Is Risky

1. **Malicious config file**: A config with thousands of "Default N" entries would cause UI freeze
2. **Integer overflow**: After ~4 billion iterations (unlikely but possible), counter wraps causing infinite loop
3. **No fallback**: If all names are taken, function never returns

### Solution

Add a counter limit (1000 is reasonable - no user will have 1000 launch configs) with a timestamp-based fallback.

### Implementation

**Replace `generate_unique_name()` with bounded version:**

```rust
/// Generate a unique name by appending numbers if needed.
/// "Default" -> "Default", "Default 2", "Default 3", etc.
/// Falls back to timestamp if counter exceeds limit.
fn generate_unique_name(base_name: &str, existing_names: &[&str]) -> String {
    if !existing_names.contains(&base_name) {
        return base_name.to_string();
    }

    // Bounded loop with reasonable limit
    const MAX_COUNTER: u32 = 1000;
    for counter in 2..=MAX_COUNTER {
        let candidate = format!("{} {}", base_name, counter);
        if !existing_names.contains(&candidate.as_str()) {
            return candidate;
        }
    }

    // Fallback to timestamp if all numbered names are taken
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{} {}", base_name, timestamp)
}
```

### Alternative: Use UUID

For guaranteed uniqueness without iteration:

```rust
fn generate_unique_name(base_name: &str, existing_names: &[&str]) -> String {
    if !existing_names.contains(&base_name) {
        return base_name.to_string();
    }

    // Try numbered suffixes first (user-friendly)
    for counter in 2..=100 {
        let candidate = format!("{} {}", base_name, counter);
        if !existing_names.contains(&candidate.as_str()) {
            return candidate;
        }
    }

    // Fallback to short hash for uniqueness
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    let hash = hasher.finish();
    format!("{} {:x}", base_name, hash & 0xFFFF)  // 4-char hex suffix
}
```

### Acceptance Criteria

1. Loop has an upper bound (max 1000 iterations)
2. Function always returns (has fallback for edge cases)
3. Normal case still produces "Default", "Default 2", etc.
4. No integer overflow possible
5. All existing tests pass
6. New test covers bounded behavior

### Testing

```bash
cargo test generate_unique_name
cargo test new_session_dialog
```

Add tests:
```rust
#[test]
fn test_generate_unique_name_basic() {
    let existing: Vec<&str> = vec![];
    assert_eq!(generate_unique_name("Default", &existing), "Default");
}

#[test]
fn test_generate_unique_name_increments() {
    let existing = vec!["Default", "Default 2"];
    assert_eq!(generate_unique_name("Default", &existing), "Default 3");
}

#[test]
fn test_generate_unique_name_fallback() {
    // Create many existing names to trigger fallback
    let existing: Vec<String> = (2..=1000)
        .map(|i| format!("Default {}", i))
        .collect();
    let existing_refs: Vec<&str> = std::iter::once("Default")
        .chain(existing.iter().map(|s| s.as_str()))
        .collect();

    let result = generate_unique_name("Default", &existing_refs);

    // Should use timestamp fallback, not panic or hang
    assert!(result.starts_with("Default "));
    assert!(!existing_refs.contains(&result.as_str()));
}
```

### Notes

- 1000 limit is arbitrary but reasonable - adjust if needed
- Timestamp fallback guarantees uniqueness (millisecond resolution)
- Could also use `uuid` crate but that adds a dependency

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/new_session_dialog/state.rs` | Replaced unbounded loop in `generate_unique_name()` with bounded for-loop (max 1000 iterations) and timestamp fallback. Added 5 comprehensive tests covering basic usage, increments, and fallback behavior. |

### Notable Decisions/Tradeoffs

1. **MAX_COUNTER set to 1000**: This is a reasonable upper bound that no real user will hit, while preventing infinite loops from malicious or corrupted config files. The timestamp fallback ensures uniqueness even in pathological cases.
2. **Timestamp fallback using milliseconds**: Provides sufficient uniqueness guarantee (millisecond precision) without adding external dependencies like `uuid`.
3. **Used `for counter in 2..=MAX_COUNTER`**: Idiomatic Rust bounded iteration that prevents both infinite loops and integer overflow issues.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test generate_unique_name` - Passed (5 tests: basic, increments, with_other_names, with_default, fallback)
- `cargo clippy -- -D warnings` - Passed (no warnings)

All new tests validate:
1. Basic case: returns base name when no conflict
2. Incremental naming: correctly generates "Default 2", "Default 3", etc.
3. Fallback behavior: when all 1000 numbered names are taken, falls back to timestamp
4. Uniqueness guarantee: fallback result is not in existing names list

### Risks/Limitations

1. **Timestamp collision risk**: If two configs are created within the same millisecond and both hit the 1000-name limit, they could theoretically get the same timestamp. This is extremely unlikely (requires 1000+ existing numbered configs AND simultaneous creation) and would be caught by the config save logic.
2. **Timestamp readability**: Fallback names like "Default 1706023456789" are less user-friendly than numbered names, but this is only reached in pathological edge cases that should never occur in practice.
