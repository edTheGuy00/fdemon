# Action Items: Phase 3 Wave 3.5

**Review Date:** 2026-01-08
**Verdict:** ⚠️ APPROVED WITH CONCERNS
**Blocking Issues:** 0

---

## Should Fix (Non-Blocking)

### 1. ~~Update `FdemonSession::quit()` to Handle Confirmation Dialog~~ ✅ DONE

- **Source:** Risks Tradeoffs Analyzer
- **File:** `tests/e2e/pty_utils.rs`
- **Line:** 425-433
- **Resolution:** Updated `quit()` to send 'q' then 'y' with a brief pause between. The method now handles both cases (dialog shown or not).
- **Verified:** `cargo check --test e2e` and `cargo clippy --test e2e -- -D warnings` pass

---

## Consider Fixing (Future)

### 2. Extract Repeated Regex Patterns to Constants

- **Source:** Code Quality Inspector
- **File:** `tests/e2e/tui_interaction.rs`
- **Problem:** Similar regex patterns (session indicators, quit dialogs) appear multiple times with duplicate documentation.
- **Suggested Action:** Extract to module-level constants:
  ```rust
  /// Regex for quit confirmation dialog indicators
  const QUIT_DIALOG_PATTERN: &str = "(y/n)|confirm|Quit";

  /// Regex for session 1 indicator in tab bar
  const SESSION_1_PATTERN: &str = "\\[1\\]|Session 1";
  ```
- **Priority:** Low

### 3. Add Edge Case Tests for Double-'q' Feature

- **Source:** Risks Tradeoffs Analyzer
- **File:** `tests/e2e/tui_interaction.rs`
- **Problem:** Edge cases for the double-'q' feature are not fully tested.
- **Suggested Action:** Add tests for:
  - Rapid 'qqq' (three q's) doesn't cause issues
  - 'qq' with `confirm_quit=false` setting
  - 'q' followed by long delay, then 'q' (dialog timeout scenario)
- **Priority:** Low

### 4. Consider Polling-Based Waits Instead of Fixed Delays

- **Source:** Logic Reasoning Checker
- **File:** `tests/e2e/tui_interaction.rs`
- **Problem:** Fixed timing delays (200ms, 500ms) may be insufficient on slow CI systems.
- **Suggested Action:** Implement `wait_until_state(predicate, timeout)` helper that polls until a condition is met, rather than sleeping for fixed duration.
- **Priority:** Low

---

## Re-review Checklist

After addressing issues, verify:

- [ ] `quit()` method updated to send 'q' + 'y'
- [ ] Tests using `quit()` don't fall back to `kill()` timeout
- [ ] `cargo check` passes
- [ ] `cargo test --test e2e` passes
- [ ] `cargo clippy --test e2e -- -D warnings` passes
