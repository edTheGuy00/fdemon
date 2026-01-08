## Task: Final Validation

**Objective**: Run all tests (PTY + TestBackend) and verify everything passes with good coverage and performance after Phase 3.6 fixes.

**Depends on**: 07-update-architecture, 08-strengthen-search-test

**Note**: This task incorporates Phase 3.5 Task 13, running validation after all review fixes are applied.

### Scope

- Validation only - no code changes
- Run complete test suite
- Verify no flakiness
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
cargo test --lib widgets

# Render tests
cargo test --lib render

# Test utilities
cargo test --lib test_utils
```

#### 3. Run PTY Tests Separately

```bash
# PTY-based E2E tests
cargo test --test e2e
```

#### 4. Verify Test Execution Time

```bash
# Time TestBackend tests
time cargo test --lib

# Time PTY tests
time cargo test --test e2e

# Time full suite
time cargo test
```

**Targets:**
- TestBackend tests: <10 seconds
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

#### 6. Verify Code Quality

```bash
# Format check
cargo fmt -- --check

# Lint check
cargo clippy -- -D warnings

# Doc check
cargo doc --no-deps
```

#### 7. Verify Snapshot Status

```bash
# Check for pending snapshots
cargo insta test --check

# List all snapshots
find . -name "*.snap" -type f | wc -l
```

### Validation Checklist

#### Phase 3.6 Fixes Verified
- [ ] OR→AND assertions fixed (task 01)
- [ ] Terminal field documented (task 02)
- [ ] test_device() deduplicated (tasks 03-04)
- [ ] status_bar.rs refactored to <500 lines (task 05)
- [ ] TestTerminal::draw_with() added (task 06)
- [ ] ARCHITECTURE.md updated (task 07)
- [ ] SearchInput test strengthened (task 08)

#### Test Results
- [ ] All lib tests pass
- [ ] All e2e tests pass
- [ ] No flaky tests (5/5 runs pass)
- [ ] Clippy clean
- [ ] Fmt clean

#### Performance
- [ ] TestBackend tests <10s
- [ ] PTY tests <60s
- [ ] Full suite <90s

### Results Documentation

Update task completion summary with:

```markdown
## Validation Results

**Date:** YYYY-MM-DD
**Commit:** <sha>

### Test Summary

| Category | Tests | Pass | Fail | Time |
|----------|-------|------|------|------|
| Lib tests | XX | XX | 0 | Xs |
| E2E tests | XX | XX | 0 | XXs |
| **Total** | **XX** | **XX** | **0** | **XXs** |

### Flakiness Check

5/5 runs passed - 0% flakiness

### Code Quality

- `cargo fmt -- --check`: PASS
- `cargo clippy -- -D warnings`: PASS
- `cargo doc --no-deps`: PASS

### Phase 3.6 Issues Resolved

1. ✅ OR→AND assertions fixed
2. ✅ test_device() deduplicated
3. ✅ status_bar.rs refactored
4. ✅ ARCHITECTURE.md updated
5. ✅ All review concerns addressed
```

### Acceptance Criteria

1. All tests pass (0 failures)
2. No flaky tests (5/5 runs pass)
3. All Phase 3.6 fixes verified working
4. Code quality checks pass
5. Performance targets met

---

## Completion Summary

**Status:** ❌ Not done

**Validation Results:**
- (pending)
