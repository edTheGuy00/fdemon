# Action Items: Startup Flow Consistency - Phase 3

**Review Date:** 2026-01-11
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 2

---

## Critical Issues (Must Fix)

### 1. State Machine Timing Inconsistency

- **Source:** logic_reasoning_checker
- **File:** `src/app/handler/update.rs`
- **Line:** 1662-1663
- **Problem:** `state.clear_loading()` is called BEFORE examining the result in the `AutoLaunchResult` handler, causing a brief UI flicker on error path (`Loading → Normal → StartupDialog`)
- **Required Action:** Move `clear_loading()` inside each match branch:

```rust
Message::AutoLaunchResult { result } => {
    match result {
        Ok(success) => {
            state.clear_loading();  // Move here
            // ... create session
        }
        Err(error_msg) => {
            state.clear_loading();  // And here
            // ... show error
        }
    }
}
```

- **Acceptance:** No UI state change before examining result. Error path should transition directly from Loading to StartupDialog.

---

### 2. Missing Concurrent Auto-Launch Guard

- **Source:** risks_tradeoffs_analyzer
- **File:** `src/app/handler/update.rs`
- **Line:** 1649
- **Problem:** No protection against duplicate `StartAutoLaunch` messages, which could spawn concurrent auto-launch tasks
- **Required Action:** Add guard check at the start of the handler:

```rust
Message::StartAutoLaunch { configs } => {
    if state.ui_mode == UiMode::Loading {
        return UpdateResult::none();  // Already launching
    }
    state.set_loading_phase("Starting...");
    UpdateResult::action(UpdateAction::DiscoverDevicesAndAutoLaunch { configs })
}
```

- **Acceptance:** Second `StartAutoLaunch` message while loading should be silently ignored.

---

## Major Issues (Should Fix)

### 3. Add Test for Concurrent Auto-Launch Guard

- **Source:** risks_tradeoffs_analyzer
- **File:** `src/app/handler/tests.rs`
- **Problem:** No test verifying that duplicate StartAutoLaunch is ignored
- **Suggested Action:** Add test:

```rust
#[test]
fn test_start_auto_launch_ignored_if_already_loading() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Loading;

    let result = update(&mut state, Message::StartAutoLaunch {
        configs: LoadedConfigs::default()
    });

    assert!(result.action.is_none()); // Should not spawn second task
}
```

---

## Minor Issues (Consider Fixing)

### 4. Key Blocking Logic Duplication

- **Source:** logic_reasoning_checker
- **File:** `src/app/handler/keys.rs:208-230`
- **Problem:** Both '+' and 'd' keys have identical loading check logic
- **Suggestion:** Extract to helper function:

```rust
fn should_block_new_session_keys(state: &AppState) -> bool {
    state.ui_mode == UiMode::Loading
}
```

### 5. DevicesDiscovered Comment Clarity

- **Source:** logic_reasoning_checker
- **File:** `src/app/handler/update.rs:470-471`
- **Problem:** Comment says "caller handles UI transition" but actual transition happens in AutoLaunchResult
- **Suggestion:** Clarify comment to specify that AutoLaunchResult handler manages the transition

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Critical issue #1 resolved (state machine timing)
- [ ] Critical issue #2 resolved (concurrent auto-launch guard)
- [ ] Major issue #3 resolved (add test)
- [ ] Verification commands pass:
  ```bash
  cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings
  ```
- [ ] Manual verification of error path UI transition (should go directly from Loading to StartupDialog)
