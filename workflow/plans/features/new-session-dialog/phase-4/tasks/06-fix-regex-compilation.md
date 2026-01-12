## Task: Fix Regex Compilation Performance

**Objective**: Compile the API pattern regex once per process lifetime using static initialization instead of on every `parse_avd_name()` call.

**Depends on**: 05-discovery-integration

**Source**: Code Quality Inspector (Review Issue #2)

### Scope

- `src/daemon/avds.rs`: Convert runtime regex compilation to static initialization

### Details

Currently `parse_avd_name()` creates a new `Regex` object on every invocation (lines 81-89). This is inefficient for repeated calls during AVD list parsing.

**Current Code:**
```rust
fn parse_avd_name(name: &str) -> AvdInfo {
    // Regex created on every call
    if let Some(caps) = Regex::new(r"_API_(\d+)$").ok().and_then(|re| re.captures(name)) {
        // ...
    }
}
```

**Required Change:**
```rust
use once_cell::sync::Lazy;
use regex::Regex;

static API_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"_API_(\d+)$").expect("Invalid API pattern regex")
});

fn parse_avd_name(name: &str) -> AvdInfo {
    if let Some(caps) = API_PATTERN.captures(name) {
        // ...
    }
}
```

### Acceptance Criteria

1. Regex is compiled once per process lifetime using `once_cell::sync::Lazy`
2. `parse_avd_name()` uses the static regex for pattern matching
3. `cargo test avds` passes
4. `cargo clippy -- -D warnings` shows no new warnings

### Testing

Existing tests in `daemon/avds.rs` should continue to pass:
- `test_parse_avd_name_with_api`
- `test_parse_avd_name_without_api`
- Edge case tests

### Notes

- `once_cell` is already a dependency in Rust's standard library as of 1.70 (check Cargo.toml for version)
- If `once_cell` is not available, use `std::sync::LazyLock` (stabilized in Rust 1.80)
- Pattern `_API_(\d+)$` extracts API level from AVD names like `Pixel_7_API_34`

---

## Completion Summary

**Status:** Not started
