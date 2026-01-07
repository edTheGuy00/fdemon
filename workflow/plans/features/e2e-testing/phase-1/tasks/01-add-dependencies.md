## Task: Add E2E Testing Dependencies

**Objective**: Add the `mockall` crate to dev-dependencies for async trait mocking in integration tests.

**Depends on**: None

### Scope

- `Cargo.toml`: Add `mockall = "0.13"` to `[dev-dependencies]`

### Details

Add the mockall crate which provides procedural macros for generating mock objects. This is needed for:
- Mocking async traits in tests
- Creating test doubles for daemon communication
- Verifying method call expectations

```toml
[dev-dependencies]
# Existing
tokio-test = "0.4"
tempfile = "3"

# New for E2E testing
mockall = "0.13"
```

**Why mockall 0.13?**
- Supports native async traits (Rust 1.75+)
- No longer requires `async-trait` crate for async mocking
- Well-maintained with 84M+ downloads

### Acceptance Criteria

1. `mockall = "0.13"` is present in `Cargo.toml` under `[dev-dependencies]`
2. `cargo check` passes without errors
3. `cargo test` continues to pass (no regressions)

### Testing

```bash
# Verify dependency resolves
cargo check

# Verify existing tests still pass
cargo test
```

### Notes

- We're adding only `mockall` in this task; `expectrl` and `insta` are for Phase 3
- The version `0.13` requires Rust 1.75+ which the project already supports
- mockall works with `trait-variant` crate already in dependencies
