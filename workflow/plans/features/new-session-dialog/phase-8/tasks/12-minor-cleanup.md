## Task: Minor Cleanup

**Objective**: Address minor issues identified in the review to improve code quality.

**Depends on**: 11-update-e2e-snapshots

**Estimated Time**: 25 minutes

**Priority**: 🟡 Minor

**Source**: Code Quality Inspector, Risks/Tradeoffs Analyzer

### Scope

- `src/app/state.rs`: Update stale comments
- `src/app/handler/tests.rs`: Improve test assertions
- `src/tui/startup.rs`: Clean up dead code markers

### Issues to Address

#### 1. Update Stale Comments (state.rs:335-336)

**Current:**
```rust
/// Global device cache (shared between DeviceSelector and StartupDialog)
```

**Fixed:**
```rust
/// Global device cache (used by NewSessionDialog)
```

#### 2. Improve Test Assertions (handler/tests.rs)

Replace `panic!()` with proper assertions at lines 860, 918, 982, 1926:

**Current:**
```rust
panic!("Expected ReloadAllSessions action, got {:?}", result.action);
```

**Fixed:**
```rust
assert!(
    matches!(result.action, Some(UpdateAction::ReloadAllSessions)),
    "Expected ReloadAllSessions action, got {:?}", result.action
);
```

#### 3. Dead Code Cleanup (startup.rs)

Review `#[allow(dead_code)]` markers at lines 44, 95, 189, 228, 244, 294:
- If code is truly unused, remove it
- If code is needed for future work, add a TODO comment with context
- If code is conditionally compiled, use `#[cfg(...)]` instead

### Process

1. Update comments in state.rs
2. Replace panic! with assert! in test files
3. Audit dead code in startup.rs
4. Run full verification suite

### Acceptance Criteria

1. No stale comments referencing DeviceSelector or StartupDialog
2. Test assertions use `assert!` or `matches!` instead of `panic!`
3. Dead code either removed or documented with TODO
4. `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

### Testing

```bash
cargo fmt
cargo check
cargo test
cargo clippy -- -D warnings
```

### Notes

- These are polish items - don't block release on minor issues
- The clone audit (47 clones) is deferred to a separate optimization task
- UiMode::Startup vs NewSessionDialog unification is a larger refactor - track separately if needed

### Optional Enhancements (Not Required)

1. **Clone Audit**: Review 47 `.clone()` calls for unnecessary allocations
2. **UiMode Unification**: Merge `UiMode::Startup` and `UiMode::NewSessionDialog` if semantically equivalent

---

## Completion Summary

**Status:** Not Started
