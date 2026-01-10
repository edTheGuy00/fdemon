## Task: Verify Loading Animation Works in Event Loop

**Objective**: Confirm that the loading animation works correctly when triggered via message, without the special `animate_during_async()` helper.

**Depends on**: 02-update-runner

**Estimated Time**: 0.5 hours

### Scope

- Verification only - no code changes expected
- May require minor fixes if issues are found

### Details

#### How Animation Should Work

The loading animation is already handled by the TEA message loop:

1. **Tick Message**: `Message::Tick` is sent periodically (from terminal event polling timeout)
2. **Tick Handler** (`update.rs:145-147`):
   ```rust
   Message::Tick => {
       state.tick_loading_animation_with_cycling(false);
       UpdateResult::none()
   }
   ```
3. **Animation Method** (`state.rs:1031-1035`):
   ```rust
   pub fn tick_loading_animation_with_cycling(&mut self, cycle_messages: bool) {
       if let Some(ref mut loading) = self.loading_state {
           loading.tick(cycle_messages);
       }
   }
   ```
4. **Render**: Each loop iteration renders the updated animation frame

#### Verification Steps

1. **Check Tick Generation**
   - Verify `event::poll()` returns `Message::Tick` on timeout
   - Default timeout should be ~100ms for smooth animation
   - Location: `src/tui/event.rs`

2. **Check Handler Calls Tick**
   - Verify `Message::Tick` handler calls `tick_loading_animation_with_cycling`
   - Location: `src/app/handler/update.rs`

3. **Check Message Cycling**
   - The auto-launch flow should cycle messages during device discovery
   - `AutoLaunchProgress` messages update the loading text
   - Verify handler updates loading message correctly

4. **Manual Test**
   - Run app with `auto_start=true`
   - Observe loading screen animation (spinner should rotate)
   - Observe message updates ("Detecting devices...", "Preparing launch...")

### Potential Issues to Check

1. **Tick Interval Too Slow**
   - If animation is choppy, check `event::poll()` timeout
   - Should be ~100ms for 10fps animation

2. **Message Cycling Not Working**
   - `AutoLaunchProgress` handler must call `update_loading_message()`
   - Verify Phase 1 Task 3 handler is correct

3. **Animation Not Visible**
   - `UiMode::Loading` must be set by `StartAutoLaunch` handler
   - Verify Phase 1 Task 3 sets loading state

4. **Race Condition**
   - Message might arrive before loading state is set
   - `AutoLaunchProgress` handler should be safe (no-op if no loading state)

### Acceptance Criteria

1. Loading spinner animates smoothly (~10fps)
2. Loading messages update during device discovery
3. No visual glitches or freezes
4. Animation stops when session starts (loading clears)
5. If issues found, document and create fix tasks

### Testing

```bash
# Run with auto_start enabled
cd /path/to/flutter/project
cargo run

# Observe:
# 1. Brief "Not Connected" normal mode
# 2. Loading screen with animated spinner
# 3. "Detecting devices..." message
# 4. "Preparing launch..." message
# 5. Session starts, normal mode with logs
```

### Notes

- This is primarily a verification task
- If issues are found, create additional tasks to fix them
- The old `animate_during_async()` was doing the same thing manually
- Now the event loop handles it naturally via `Message::Tick`

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (pending - verification only)

**Verification Results:**

(pending)

**Issues Found:**

(pending)

**Notes:**

(pending)
