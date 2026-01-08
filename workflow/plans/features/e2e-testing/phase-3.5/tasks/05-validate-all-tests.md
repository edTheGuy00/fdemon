## Task: Validate All Tests Pass

**Objective**: Run the complete E2E test suite and verify all tests pass reliably.

**Depends on**: 02-ci-timeout-extension, 03-test-categorization, 04-retry-config

### Scope

- Validation only - no code changes
- Run full test suite multiple times
- Document any remaining issues

### Details

#### 1. Run Full Test Suite

```bash
# Clean build
cargo clean

# Build in test mode
cargo build --tests

# Run all E2E tests
cargo test --test e2e -- --nocapture 2>&1 | tee test-results.log
```

#### 2. Verify Pass Rate

Run tests multiple times to check for flakiness:

```bash
#!/bin/bash
# Run 5 times and count failures
failures=0
for i in {1..5}; do
    echo "=== Run $i ==="
    if ! cargo test --test e2e; then
        failures=$((failures + 1))
    fi
done
echo "Failures: $failures/5"
```

Target: 0 failures out of 5 runs (100% pass rate)

#### 3. Verify Test Categories

Check that tests are properly categorized:

```bash
# Count TUI tests (use spawn())
grep -c "FdemonSession::spawn(" tests/e2e/tui_interaction.rs

# Count headless tests (use spawn_headless())
grep -c "spawn_headless(" tests/e2e/*.rs

# Count ignored tests
grep -c "#\[ignore\]" tests/e2e/*.rs
```

#### 4. Verify Execution Time

```bash
# Time the full suite
time cargo test --test e2e

# Target: <60 seconds
```

#### 5. CI Simulation

```bash
# Simulate CI environment
CI=true cargo test --test e2e
```

### Acceptance Criteria

1. All non-ignored tests pass (0 failures)
2. 5 consecutive runs all pass (no flakiness)
3. Test execution time <60 seconds
4. CI simulation passes
5. Test counts documented:
   - Total tests: X
   - TUI tests: Y
   - Headless tests: Z
   - Ignored tests: W

### Expected Results

After all Phase 3.5 fixes:

| Metric | Target | Notes |
|--------|--------|-------|
| Pass rate | 100% | All non-ignored tests |
| Flakiness | 0% | 5 consecutive runs pass |
| Execution time | <60s | Full E2E suite |
| CI pass rate | >95% | With retry mechanism |

### Documentation

Create `VALIDATION.md` with results:

```markdown
# Phase 3.5 Validation Results

**Date:** YYYY-MM-DD
**Commit:** <sha>

## Test Results

- Total E2E tests: XX
- Passing: XX
- Failing: 0
- Ignored: XX

## Flakiness Check

- Run 1: PASS
- Run 2: PASS
- Run 3: PASS
- Run 4: PASS
- Run 5: PASS

## Execution Time

- Average: XXs
- Max: XXs
- Target: <60s

## Issues Found

(List any remaining issues for future phases)
```

---

## Completion Summary

**Status:** Not Started

**Validation Results:**
- Total tests: (pending)
- Pass rate: (pending)
- Flakiness: (pending)
- Execution time: (pending)
