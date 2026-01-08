## Task: Tighten Flexible Regex Patterns

**Objective**: Improve test assertions by either tightening flexible regex patterns or documenting why each alternative is valid.

**Depends on**: 07f-standardize-cleanup

**Priority**: 🟢 MINOR (Consider Fixing)

### Scope

- `tests/e2e/tui_interaction.rs`: Review and improve regex patterns

### Background

Several tests use very permissive regex patterns that accept many outcomes as "success". For example:

```rust
session.expect("Running|Starting|Error|No device|Waiting|Loading|Connected")
```

This pattern accepts 7 different outcomes, which could hide regressions. If the app suddenly shows "Error" when it should show "Running", the test still passes.

### Locations to Review

- Line 40: Multiple status alternatives
- Line 66: Device state alternatives
- Line 112: Startup state alternatives
- Line 569: Output validation alternatives

### Options

**Option A: Split into separate tests**
Create specific tests for each expected scenario:

```rust
#[tokio::test]
#[serial]
async fn test_startup_shows_running_status() {
    // ... setup with device available
    session.expect("Running|Connected").expect("Should show running status");
}

#[tokio::test]
#[serial]
async fn test_startup_shows_waiting_without_device() {
    // ... setup without device
    session.expect("Waiting|No device").expect("Should show waiting status");
}
```

**Option B: Document each alternative**
Add comments explaining why each is valid:

```rust
// Valid startup states depend on device availability:
// - "Running" / "Connected": Device available, Flutter attached
// - "Starting" / "Loading": Device available, Flutter launching
// - "Waiting" / "No device": No device available (CI environment)
// - "Error": Device attachment failed (acceptable in headless mode)
session.expect("Running|Starting|Error|No device|Waiting|Loading|Connected")
    .expect("Should show valid startup state");
```

**Option C: Use test fixtures to control state**
If using `TestFixture`, configure it to produce a specific state:

```rust
let fixture = TestFixture::simple_app()
    .with_device("mock-device-id");  // Ensures device is available
session.expect("Running|Connected").expect("Should be running with device");
```

### Recommended Approach

For each flexible pattern:

1. **Document**: Add comment explaining valid alternatives
2. **Consider splitting**: If different outcomes indicate different code paths, create separate tests
3. **Use fixtures**: Control preconditions to narrow expected outcomes

### Acceptance Criteria

1. Each flexible regex pattern has a comment explaining valid alternatives
2. Critical state transitions have specific tests (not catch-all patterns)
3. Tests can distinguish between "expected failure states" and "unexpected bugs"
4. No change to test pass/fail behavior (unless bugs are found)

### Testing

```bash
# Run tests - should still pass
cargo test --test e2e tui_interaction -- --nocapture
```

### Notes

- This is a quality improvement, not a blocking issue
- Balance specificity with test maintainability
- Some flexibility is acceptable for environment-dependent behavior
- Consider adding test categories: "with device" vs "headless"

---

## Completion Summary

**Status:** Not Started
