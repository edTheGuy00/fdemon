# Action Items: New Session Dialog Phase 8

**Review Date:** 2026-01-16
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 3

---

## Critical Issues (Must Fix)

### 1. Fix '+' and 'd' Key Handlers

- **Source:** Logic Reasoning Checker, Architecture Enforcer
- **File:** `src/app/handler/keys.rs`
- **Lines:** 167-189
- **Problem:** Both keys send deprecated messages when no sessions exist, causing silent failures
- **Required Action:** Update both key handlers to use `Message::OpenNewSessionDialog`

```rust
// Replace lines 167-189 with:
(KeyCode::Char('+'), KeyModifiers::NONE) | (KeyCode::Char('d'), KeyModifiers::NONE) => {
    if state.ui_mode == UiMode::Loading {
        None
    } else {
        Some(Message::OpenNewSessionDialog)
    }
}
```

- **Acceptance:**
  - User presses `+` or `d` without sessions → NewSessionDialog opens
  - No deprecated message warnings in logs

### 2. Remove Deprecated Message Variants

- **Source:** All Agents
- **File:** `src/app/message.rs`
- **Lines:** 340-395 (approximate)
- **Problem:** 30+ deprecated message variants that can still be constructed
- **Required Action:** Delete these message variants from the enum:

```
StartupDialogOpen
StartupDialogClose
StartupDialogSelectNext
StartupDialogSelectPrevious
StartupDialogSelectConfig
StartupDialogCycleMode
StartupDialogSwitchSection
StartupDialogSetFlavor
StartupDialogSetDartDefines
StartupDialogLaunch
StartupDialogRefresh
StartupDialogUp
StartupDialogDown
StartupDialogLeft
StartupDialogRight
StartupDialogJumpToSection
ShowDeviceSelector
HideDeviceSelector
DeviceSelectorSelectNext
DeviceSelectorSelectPrevious
DeviceSelectorConfirm
DeviceSelectorRefresh
DeviceSelectorDevicesReceived
DeviceSelectorError
ShowStartupDialog
HideStartupDialog
```

- **Acceptance:**
  - `cargo check` passes
  - No deprecated message variants in `message.rs`
  - Any remaining references fail at compile time

### 3. Remove Deprecated Message Handlers

- **Source:** All Agents
- **File:** `src/app/handler/update.rs`
- **Lines:** 278-354, 696-794
- **Problem:** ~100 lines of handlers that only log warnings
- **Required Action:** Delete all match arms for deprecated messages

- **Acceptance:**
  - `cargo check` passes
  - No warning-only handlers remain
  - `update.rs` reduced by ~100 lines

---

## Major Issues (Should Fix)

### 4. Fix Test Compilation Errors

- **Source:** Architecture Enforcer
- **File:** `src/app/handler/tests.rs`
- **Lines:** 555, 1803-1807
- **Problem:** Tests reference deleted types (`UiMode::StartupDialog`, `startup_dialog_state`)
- **Required Action:**
  - Remove or update `test_close_session_shows_device_selector_when_multiple()`
  - Update assertions to use `UiMode::NewSessionDialog` and `new_session_dialog_state`

- **Acceptance:**
  - `cargo test --lib` compiles
  - All handler tests pass

### 5. Fix Render Test Compilation Errors

- **Source:** Architecture Enforcer
- **File:** `src/tui/render/tests.rs`
- **Lines:** 80-214, 323, 369, 468, 503
- **Problem:** 7+ tests reference deleted `UiMode::DeviceSelector` and `device_selector` field
- **Required Action:**
  - Replace DeviceSelector tests with NewSessionDialog equivalents
  - Update transition tests to use new modes

- **Acceptance:**
  - `cargo test render` compiles
  - All render tests pass

### 6. Update E2E Snapshot Tests

- **Source:** Risks/Tradeoffs Analyzer
- **File:** `tests/e2e/`
- **Problem:** E2E snapshots not updated for new dialog UI
- **Required Action:**
  - Run `cargo test --test e2e`
  - Review failures
  - Update snapshots with `cargo insta review`

- **Acceptance:**
  - All E2E tests pass
  - Snapshots reflect NewSessionDialog UI

---

## Minor Issues (Consider Fixing)

### 7. Audit and Reduce Cloning

- **Source:** Code Quality Inspector
- **Files:** `src/app/handler/*.rs`
- **Problem:** 47 `.clone()` calls, many unnecessary
- **Suggested Action:** Review clone calls, use references where possible

### 8. Update Stale Comments

- **Source:** Risks/Tradeoffs Analyzer
- **File:** `src/app/state.rs:335-336`
- **Problem:** Comments reference deleted DeviceSelector/StartupDialog
- **Suggested Action:** Update comments to reference NewSessionDialog

### 9. Remove or Track Dead Code

- **Source:** Code Quality Inspector
- **Files:** `src/tui/startup.rs`
- **Problem:** Multiple `#[allow(dead_code)]` without tracking
- **Suggested Action:** Remove dead code or add TODO with issue reference

### 10. Improve Test Assertions

- **Source:** Code Quality Inspector
- **Files:** `src/app/handler/tests.rs`
- **Lines:** 860, 918, 982, 1926
- **Problem:** Tests use `panic!()` instead of proper assertions
- **Suggested Action:** Replace with `assert!` or `matches!` macros

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] All critical issues resolved (1-3)
- [ ] All major issues resolved (4-6)
- [ ] Minor issues resolved or justified
- [ ] `cargo fmt` - No formatting errors
- [ ] `cargo check` - Compiles successfully
- [ ] `cargo test` - All tests pass
- [ ] `cargo clippy -- -D warnings` - No warnings
- [ ] Manual test: `+` key opens dialog without sessions
- [ ] Manual test: `d` key opens dialog without sessions
- [ ] E2E snapshots updated and passing

---

## Estimated Effort

| Issue | Effort |
|-------|--------|
| Fix key handlers | 5 min |
| Remove deprecated messages | 15 min |
| Remove deprecated handlers | 10 min |
| Fix handler tests | 20 min |
| Fix render tests | 30 min |
| Update E2E snapshots | 20 min |
| **Total** | ~1.5 hours |

---

## Notes

- Run verification frequently: `cargo fmt && cargo check && cargo test`
- Test key handlers manually after fix
- Deprecated message removal may expose additional compile errors - fix as they appear
- Consider merging `UiMode::Startup` and `UiMode::NewSessionDialog` in follow-up task
