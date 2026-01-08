# Action Items: Phase 3 E2E Testing - Wave 2 & Wave 3

**Review Date:** 2026-01-08
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 3

---

## Critical Issues (Must Fix)

### 1. Remove or Fix `test_double_q_quick_quit`

- **Source:** Logic Reasoning Checker
- **File:** `tests/e2e/tui_interaction.rs`
- **Line:** 358-386
- **Problem:** Test documents behavior that doesn't exist in implementation. The second 'q' in confirm dialog mode returns `None` (no action) per `src/app/handler/keys.rs:58-73`.
- **Required Action:**
  - Option A: Implement double-'q' quick quit in `handle_key_confirm_dialog`
  - Option B: Remove the test entirely
- **Acceptance:** Test either passes consistently or is removed from the file.

---

### 2. Extract Magic Numbers to Named Constants

- **Source:** Code Quality Inspector
- **File:** `tests/e2e/tui_interaction.rs`
- **Lines:** 85, 132, 285, 339, 370, 411, 421, 424, 429, 456, 464, 477, 498, 507-517, 548, 559, 576
- **Problem:** Hardcoded timing values violate CODE_STANDARDS.md:86-93
- **Required Action:** Add constants at module level:
  ```rust
  /// Time to wait after sending input for processing
  const INPUT_PROCESSING_DELAY_MS: u64 = 200;
  /// Time to wait for initialization
  const INITIALIZATION_DELAY_MS: u64 = 500;
  /// Number of retries when checking process termination
  const TERMINATION_CHECK_RETRIES: usize = 20;
  /// Interval between termination checks
  const TERMINATION_CHECK_INTERVAL_MS: u64 = 100;
  ```
- **Acceptance:** No hardcoded `Duration::from_millis(N)` or magic loop counts in test functions.

---

### 3. Extract Termination Check to Helper Function

- **Source:** Code Quality Inspector
- **File:** `tests/e2e/tui_interaction.rs`
- **Lines:** 285-294, 339-353, 370-385 (3+ duplications)
- **Problem:** Same polling loop duplicated 4 times, violates DRY principle
- **Required Action:** Create helper function:
  ```rust
  /// Wait for process to terminate, checking periodically.
  ///
  /// Returns `true` if process terminated within retry limit.
  fn wait_for_termination(session: &mut FdemonSession) -> bool {
      for _ in 0..TERMINATION_CHECK_RETRIES {
          std::thread::sleep(Duration::from_millis(TERMINATION_CHECK_INTERVAL_MS));
          if let Ok(false) = session.session_mut().is_alive() {
              return true;
          }
      }
      false
  }
  ```
- **Acceptance:** Termination polling logic exists exactly once; all tests call the helper.

---

## Major Issues (Should Fix)

### 4. Add Comprehensive Module Documentation

- **Source:** Code Quality Inspector
- **File:** `tests/e2e/tui_interaction.rs`
- **Line:** 1-8
- **Problem:** 583-line file has only 3-line doc comment
- **Suggested Action:** Expand module docs to explain:
  - Test organization (sections)
  - Test isolation strategy (`#[serial]`)
  - Cleanup approach (`kill()` vs `quit()`)
  - Known limitations (device requirements)
  - How to run tests

---

### 5. Improve Quit Flow State Verification

- **Source:** Logic Reasoning Checker
- **File:** `tests/e2e/tui_interaction.rs`
- **Line:** 268-295 (`test_quit_confirmation_yes_exits`)
- **Problem:** Test doesn't verify confirmation dialog appeared before checking exit
- **Suggested Action:** Add explicit state check after `expect("quit|Quit")` and before sending 'y'

---

### 6. Standardize Cleanup Approach

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `tests/e2e/tui_interaction.rs`
- **Problem:** 13 of 17 tests use `kill()` instead of graceful `quit()`
- **Suggested Action:**
  - Use `quit()` as default cleanup method
  - Use `kill()` only when specifically testing abnormal termination
  - Document cleanup strategy in module docs

---

## Minor Issues (Consider Fixing)

### 7. Tighten Flexible Regex Patterns

- **File:** `tests/e2e/tui_interaction.rs`
- **Lines:** 40, 66, 112, 569
- **Problem:** Patterns accept many outcomes; could hide regressions
- **Suggestion:** Add comments explaining why each alternative is valid, or split into separate tests

---

### 8. Use Tokio Sleep Instead of Blocking Sleep

- **File:** `tests/e2e/tui_interaction.rs`
- **Problem:** `std::thread::sleep` blocks the tokio runtime thread pool
- **Suggestion:** Replace with `tokio::time::sleep(...).await` in `#[tokio::test]` contexts

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Issue #1: `test_double_q_quick_quit` resolved (fixed or removed)
- [ ] Issue #2: All timing values use named constants
- [ ] Issue #3: Termination check helper function exists and is used
- [ ] `cargo fmt` - Passes
- [ ] `cargo check` - No compilation errors
- [ ] `cargo clippy --test e2e -- -D warnings` - No warnings
- [ ] `cargo test --test e2e --no-run` - Tests compile

---

## Verification Commands

```bash
# Format code
cargo fmt

# Check compilation
cargo check

# Run clippy
cargo clippy --test e2e -- -D warnings

# Verify tests compile
cargo test --test e2e --no-run

# Run specific tests (requires built binary)
cargo test --test e2e tui_interaction -- --nocapture
```

---

**Estimated Effort:** 30-60 minutes
**Priority:** High (blocks merge)
