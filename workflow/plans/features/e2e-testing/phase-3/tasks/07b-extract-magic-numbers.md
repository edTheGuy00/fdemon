## Task: Extract Magic Numbers to Named Constants

**Objective**: Replace all hardcoded timing values in `tui_interaction.rs` with named constants per CODE_STANDARDS.md:86-93.

**Depends on**: 07-test-quit-key

**Priority**: 🔴 CRITICAL (Blocking)

### Scope

- `tests/e2e/tui_interaction.rs`: Extract magic numbers to module-level constants

### Background

Per `docs/CODE_STANDARDS.md:86-93`, magic numbers must use named constants. The test file currently has hardcoded values scattered throughout:

**Identified locations:**
- Lines 85, 132: `Duration::from_millis(200)` - input processing delay
- Lines 285, 339, 370, 411, 421, 424, 429, 456, 464, 477, 498: `Duration::from_millis(100)` - termination check interval
- Lines 507-517, 548, 559, 576: `Duration::from_millis(500)` - initialization delay
- Multiple locations: `for _ in 0..20` - retry counts

### Implementation

Add constants at the top of `tui_interaction.rs` (after imports, before tests):

```rust
// ===========================================================================
// Test Timing Constants
// ===========================================================================

/// Time to wait after sending input for the application to process it.
/// This accounts for PTY buffering and async event handling.
const INPUT_PROCESSING_DELAY_MS: u64 = 200;

/// Time to wait for application initialization (header rendering, etc.).
/// Longer than input delay since startup involves more work.
const INITIALIZATION_DELAY_MS: u64 = 500;

/// Number of attempts when checking for process termination.
/// Combined with TERMINATION_CHECK_INTERVAL_MS, allows up to 2 seconds.
const TERMINATION_CHECK_RETRIES: usize = 20;

/// Interval between termination status checks.
/// Short enough to detect quick exits, long enough to avoid CPU spinning.
const TERMINATION_CHECK_INTERVAL_MS: u64 = 100;
```

Then replace all hardcoded values:

```rust
// BEFORE:
std::thread::sleep(Duration::from_millis(200));

// AFTER:
std::thread::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS));
```

```rust
// BEFORE:
for _ in 0..20 {
    std::thread::sleep(Duration::from_millis(100));
    // ...
}

// AFTER:
for _ in 0..TERMINATION_CHECK_RETRIES {
    std::thread::sleep(Duration::from_millis(TERMINATION_CHECK_INTERVAL_MS));
    // ...
}
```

### Acceptance Criteria

1. Module-level constants defined with doc comments explaining their purpose
2. No hardcoded `Duration::from_millis(N)` where N is a literal number in test bodies
3. No magic loop counts (e.g., `0..20`) in test bodies
4. `cargo fmt` - Passes
5. `cargo check` - No compilation errors
6. `cargo clippy --test e2e -- -D warnings` - No warnings

### Testing

```bash
# Verify no hardcoded timing values remain
grep -n "from_millis([0-9]" tests/e2e/tui_interaction.rs
# Should return empty (no matches in test bodies)

# Verify compilation
cargo check

# Verify tests still work
cargo test --test e2e --no-run
```

### Notes

- Constants should be descriptive and explain WHY that particular value was chosen
- Consider grouping related constants with section comments
- If a test needs a different timing value, use a local const with explanation
- This change is purely mechanical and should not affect test behavior

---

## Completion Summary

**Status:** Not Started
