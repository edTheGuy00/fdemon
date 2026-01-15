# Action Items: New Session Dialog - Phase 6

**Review Date:** 2026-01-15
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 3

---

## Critical Issues (Must Fix)

### 1. Fix Infinite Loop in Field Navigation

**Source:** Logic Reasoning Checker
**File:** `src/tui/widgets/new_session_dialog/state.rs:80-98`
**Problem:** Loop condition `next.next() != start` is incorrect and could cause infinite loop

**Required Action:**
Change the loop condition in both `next_enabled()` and `prev_enabled()` methods:

```rust
// BEFORE (buggy)
while is_disabled(next) && next.next() != start {

// AFTER (fixed)
while is_disabled(next) && next != start {
```

**Acceptance:**
- [ ] Loop condition fixed in `next_enabled()`
- [ ] Loop condition fixed in `prev_enabled()`
- [ ] Add test case for "all fields disabled" scenario
- [ ] `cargo test launch_context` passes

### 2. Add Unit Tests for New Handlers

**Source:** Code Quality Inspector
**File:** `src/app/handler/update.rs` (lines 1903-2296)
**Problem:** 394 new lines of handler code with zero test coverage

**Required Action:**
Create tests for the new message handlers. At minimum, test:
- `NewSessionDialogFieldNext`/`Prev` navigation
- `NewSessionDialogModeNext`/`Prev` cycling with editability checks
- `NewSessionDialogLaunch` with/without device selected
- Auto-save triggering for FDemon configs

**Acceptance:**
- [ ] Create `src/app/handler/tests/new_session_dialog_phase6.rs` (or add to existing tests.rs)
- [ ] Test field navigation respects active_pane
- [ ] Test mode cycling respects VSCode read-only
- [ ] Test launch requires device selection
- [ ] All new tests pass

### 3. Document or Implement Placeholder Actions

**Source:** Risks & Tradeoffs Analyzer
**File:** `src/tui/actions.rs:109-123`
**Problem:** Placeholder implementations create silent failures - UI suggests functionality but nothing happens

**Required Action (choose one):**

**Option A:** Implement the actions
- Implement `AutoSaveConfig` using `config::writer::save_fdemon_configs()`
- Implement `LaunchFlutterSession` session creation

**Option B:** Add clear user feedback
- Add info!() log message visible to users
- Consider showing transient "Feature coming soon" notification in UI

**Acceptance:**
- [ ] Either actions are implemented OR user feedback is added
- [ ] No silent failures when users trigger launch/auto-save

---

## Major Issues (Should Fix)

### 4. Plan File Splitting for update.rs and state.rs

**Source:** Code Quality Inspector
**File:** `src/app/handler/update.rs` (2,835 lines), `src/tui/widgets/new_session_dialog/state.rs` (2,058 lines)
**Problem:** Both files exceed the 500-line guideline by 400-500%

**Suggested Action:**
Create a tracking issue for file splitting with proposed structure:

**update.rs split:**
```
src/app/handler/
├── mod.rs           (main update fn, routing)
├── keys.rs          (existing)
├── helpers.rs       (existing)
├── session.rs       (session handlers)
├── new_session_dialog.rs (dialog handlers)
└── tests.rs         (existing)
```

**state.rs split:**
```
src/tui/widgets/new_session_dialog/
├── mod.rs
├── state/
│   ├── mod.rs
│   ├── dialog.rs
│   ├── launch_context.rs
│   ├── fuzzy_modal.rs
│   ├── dart_defines.rs
│   └── types.rs
└── ...
```

**Acceptance:**
- [ ] Create GitHub issue tracking file splitting task
- [ ] Link issue in code comments at top of both files

### 5. Refactor Editability Check Duplication

**Source:** Logic Reasoning Checker
**File:** `src/app/handler/update.rs:2027-2113`
**Problem:** Handler duplicates logic that exists in `LaunchContextState::is_mode_editable()`

**Suggested Action:**
Extract editability validation to use state methods:

```rust
// Instead of duplicating the check:
if config.source == ConfigSource::VSCode { ... }

// Call the state method:
if !state.new_session_dialog_state.is_mode_editable() { ... }
```

**Acceptance:**
- [ ] Mode handlers use state method for editability check
- [ ] Flavor handlers use state method
- [ ] DartDefines handlers use state method

### 6. Improve Launch Tab Validation Error Message

**Source:** Logic Reasoning Checker
**File:** `src/app/handler/update.rs:2219-2277`
**Problem:** If user is on Bootable tab, launch fails with misleading "Please select a device first"

**Suggested Action:**
Either:
- Auto-switch to Connected tab if no device selected but connected devices exist
- Show clearer error: "Cannot launch from Bootable tab. Switch to Connected tab."

**Acceptance:**
- [ ] Error message is clear about the actual problem
- [ ] OR auto-switching behavior is implemented

---

## Minor Issues (Consider Fixing)

### 7. Remove or Use LaunchContextState Methods

**File:** `src/tui/widgets/new_session_dialog/state.rs:242-277`
**Problem:** Methods like `focus_next()`, `focus_prev()` are implemented but unused

**Options:**
- Remove unused methods to reduce confusion
- Refactor handlers to use the `LaunchContextState` methods instead of direct state manipulation

### 8. Add File Locking to Config Writer

**File:** `src/config/writer.rs:42`
**Problem:** Concurrent writes to `.fdemon/launch.toml` not protected

**Suggested Action:**
```rust
use fs2::FileExt;

// In save_fdemon_configs:
let file = std::fs::File::create(&config_path)?;
file.lock_exclusive()?;
std::fs::write(&config_path, content)?;
file.unlock()?;
```

### 9. Fix ConfigAutoSaver Race Condition

**File:** `src/config/writer.rs:222-242`
**Problem:** Multiple rapid saves clone configs at spawn time, intermediate state may be lost

**Suggested Action:**
Use tokio::select! with cancellation or implement a write queue pattern.

### 10. Consolidate Widget Rendering Code

**File:** `src/tui/widgets/new_session_dialog/launch_context.rs:583-777`
**Problem:** `LaunchContext` and `LaunchContextWithDevice` share 110+ duplicated lines

**Suggested Action:**
Extract shared rendering logic to a helper method.

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] All critical issues resolved
- [ ] All major issues resolved or justified
- [ ] `cargo fmt` passes
- [ ] `cargo check` passes
- [ ] `cargo test` passes (all tests)
- [ ] `cargo clippy -- -D warnings` passes
- [ ] New handler tests added and passing
- [ ] File splitting tracked in issue (if not done in this PR)
