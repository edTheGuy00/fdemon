## Task: Final Validation

**Objective**: Run all tests (PTY + TestBackend) and verify everything passes with good coverage and performance.

**Depends on**: 11-screen-snapshots, 12-ui-mode-transitions

### Scope

- Validation only - no code changes
- Run complete test suite
- Generate coverage report
- Document results

### Details

#### 1. Run Complete Test Suite

```bash
# Clean build
cargo clean

# Run all tests
cargo test 2>&1 | tee validation-results.txt

# Count results
echo "Test Summary:"
grep -E "^test result:" validation-results.txt
```

#### 2. Run TestBackend Tests Separately

```bash
# Widget tests
cargo test tui::widgets --lib 2>&1 | tee widget-tests.txt

# Render tests
cargo test tui::render --lib 2>&1 | tee render-tests.txt

# Test utilities
cargo test tui::test_utils --lib 2>&1 | tee utils-tests.txt
```

#### 3. Run PTY Tests Separately

```bash
# PTY-based E2E tests
cargo test --test e2e 2>&1 | tee e2e-tests.txt
```

#### 4. Verify Test Execution Time

```bash
# Time TestBackend tests
time cargo test tui --lib

# Time PTY tests
time cargo test --test e2e

# Time full suite
time cargo test
```

**Targets:**
- TestBackend tests: <5 seconds
- PTY tests: <60 seconds
- Full suite: <90 seconds

#### 5. Check Test Flakiness

```bash
#!/bin/bash
# Run 5 times to detect flakiness
echo "Running flakiness check..."
for i in {1..5}; do
    echo "=== Run $i ==="
    cargo test --lib || echo "FAIL: Run $i (lib)"
    cargo test --test e2e || echo "FAIL: Run $i (e2e)"
done
```

**Target:** 0 failures across 5 runs

#### 6. Generate Coverage Report (Optional)

```bash
# Install tarpaulin if needed
cargo install cargo-tarpaulin

# Generate coverage
cargo tarpaulin --out Html --output-dir coverage/

# Open report
open coverage/tarpaulin-report.html
```

#### 7. Verify Snapshot Status

```bash
# Check for pending snapshots
cargo insta test --check

# List all snapshots
find . -name "*.snap" -type f
```

### Validation Checklist

#### PTY Tests
- [ ] All PTY tests pass
- [ ] No timeouts or flaky failures
- [ ] CI simulation passes (`CI=true cargo test --test e2e`)
- [ ] Execution time <60 seconds

#### TestBackend Tests
- [ ] All widget tests pass
- [ ] All render tests pass
- [ ] All snapshot tests pass
- [ ] Execution time <5 seconds

#### Overall
- [ ] Full `cargo test` passes
- [ ] No warnings in test output
- [ ] 5 consecutive runs all pass (no flakiness)
- [ ] Total execution time <90 seconds

### Results Documentation

Create `VALIDATION-RESULTS.md`:

```markdown
# Phase 3.5 Validation Results

**Date:** YYYY-MM-DD
**Commit:** <sha>

## Test Summary

| Category | Tests | Pass | Fail | Ignored | Time |
|----------|-------|------|------|---------|------|
| Handler tests | XX | XX | 0 | X | Xs |
| Widget tests | XX | XX | 0 | X | Xs |
| Render tests | XX | XX | 0 | X | Xs |
| PTY tests | XX | XX | 0 | X | XXs |
| **Total** | **XX** | **XX** | **0** | **X** | **XXs** |

## Flakiness Check

| Run | Lib Tests | E2E Tests |
|-----|-----------|-----------|
| 1 | PASS | PASS |
| 2 | PASS | PASS |
| 3 | PASS | PASS |
| 4 | PASS | PASS |
| 5 | PASS | PASS |

**Flakiness Rate:** 0%

## Coverage (Optional)

- Line coverage: XX%
- Branch coverage: XX%

## Performance

- TestBackend tests: Xs
- PTY tests: XXs
- Full suite: XXs

## Issues Found

(List any issues discovered during validation)

## Recommendations

(Any follow-up work needed)
```

### Acceptance Criteria

1. All tests pass (0 failures)
2. No flaky tests (5/5 runs pass)
3. TestBackend tests <5s
4. PTY tests <60s
5. Full suite <90s
6. Results documented

### Success Metrics

| Metric | Target | Actual |
|--------|--------|--------|
| Pass rate | 100% | ? |
| Flakiness | 0% | ? |
| TestBackend time | <5s | ? |
| PTY time | <60s | ? |
| Total time | <90s | ? |

---

## Completion Summary

**Status:** Not Started

**Validation Results:**
- Total tests: (pending)
- Pass rate: (pending)
- Flakiness: (pending)
- TestBackend time: (pending)
- PTY time: (pending)
- Total time: (pending)
