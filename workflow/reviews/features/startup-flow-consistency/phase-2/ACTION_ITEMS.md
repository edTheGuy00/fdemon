# Action Items: Startup Flow Consistency - Phase 2

**Review Date:** 2026-01-11
**Verdict:** APPROVED WITH CONCERNS
**Blocking Issues:** 2

---

## Critical Issues (Must Fix)

*None*

---

## Major Issues (Must Fix Before Merge)

### 1. Add Error Logging for Channel Send

**Source:** Code Quality Inspector, Risks Analyzer
**File:** `src/tui/runner.rs`
**Line:** 70
**Problem:** Silent failure mode when auto-start message send fails

**Required Action:**
```rust
// Change from:
let _ = msg_tx.send(Message::StartAutoLaunch { configs }).await;

// To:
if let Err(e) = msg_tx.send(Message::StartAutoLaunch { configs }).await {
    error!("Failed to send auto-start message: {}. Auto-start will not trigger.", e);
}
```

**Acceptance:** `cargo clippy` passes, error is logged on channel failure

### 2. Add Error Logging for Terminal Draw

**Source:** Code Quality Inspector, Risks Analyzer
**File:** `src/tui/runner.rs`
**Line:** 65
**Problem:** Silent failure if first render fails

**Required Action:**
```rust
// Change from:
let _ = term.draw(|frame| render::view(frame, &mut state));

// To:
if let Err(e) = term.draw(|frame| render::view(frame, &mut state)) {
    error!("Failed to render initial frame: {}", e);
}
```

**Acceptance:** `cargo clippy` passes, error is logged on draw failure

---

## Minor Issues (Should Fix)

### 3. Document Manual Testing Results

**Source:** Risks Analyzer
**Files:**
- `workflow/plans/.../phase-2/tasks/02-update-runner.md`
- `workflow/plans/.../phase-2/tasks/03-verify-animation.md`

**Problem:** Task completions don't document actual manual testing

**Suggested Action:**
1. Run `cargo run` with `.fdemon/config.toml` containing `auto_start = true`
2. Observe: Normal mode (brief) -> Loading screen -> Session starts
3. Run `cargo run` with `auto_start = false`
4. Observe: Normal mode stays, user can press '+' to start
5. Document results in task completion summaries

**Acceptance:** Manual testing documented in task files

### 4. Add TODO Comments to Dead Code

**Source:** Code Quality Inspector
**File:** `src/tui/startup.rs`
**Lines:** 43, 95, 182, 220, 235, 284

**Problem:** Dead code attributes without cleanup references

**Suggested Action:**
```rust
// Add comment before each #[allow(dead_code)]:
// TODO(phase-4): Remove after cleanup - see workflow/plans/.../phase-4/
#[allow(dead_code)]
async fn animate_during_async<T, F>(...) { ... }
```

**Acceptance:** Each dead code function has a TODO comment

---

## Considerations (Nice to Have)

### 5. Add Edge Case Tests

**Source:** Code Quality Inspector
**File:** `src/tui/startup.rs`

**Missing test coverage:**
- `test_startup_flutter_with_invalid_path_sets_normal_mode`
- `test_startup_flutter_handles_empty_configs`

**Suggested Action:** Add tests to the existing `mod tests` block

### 6. Set Phase 4 Deadline

**Source:** Risks Analyzer
**File:** `workflow/plans/.../TASKS.md` (or create tracking issue)

**Suggested Action:** Add deadline note: "Phase 4 must complete within 2 weeks of Phase 3 completion to avoid dead code drift"

---

## Re-review Checklist

After addressing issues, verify:

- [ ] Items 1 & 2: `cargo clippy -- -D warnings` passes
- [ ] Items 1 & 2: Error logging confirmed via code review
- [ ] Item 3: Manual testing documented in task files
- [ ] Item 4: TODO comments added to dead code
- [ ] Full verification: `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings`

---

## Estimated Fix Time

| Item | Effort |
|------|--------|
| 1. Channel send error logging | 5 min |
| 2. Terminal draw error logging | 5 min |
| 3. Document manual testing | 15 min |
| 4. TODO comments | 10 min |
| **Total** | **~35 min** |

---

## Notes

The core implementation is architecturally sound and follows TEA principles correctly. These action items are polish - improving error visibility and documentation. None affect the fundamental design decisions made in Phase 2.

Once items 1-2 are addressed, this phase can be considered complete and ready to proceed to Phase 3.
