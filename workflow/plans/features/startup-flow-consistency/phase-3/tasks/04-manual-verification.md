## Task: Manual Verification of Auto-Launch Flow

**Objective**: Manually test the complete auto-launch flow with real devices to verify correct behavior across all scenarios.

**Depends on**: 03-integration-tests

**Estimated Time**: 0.5 hours

### Scope

- Manual testing only - no code changes expected
- Document any issues found for follow-up tasks

### Test Scenarios

#### Scenario 1: Auto-Start with Device Connected

**Setup:**
- Flutter project with `.fdemon/config.toml` containing `auto_start = true`
- Android or iOS device/emulator connected

**Steps:**
1. Run `cargo run -- /path/to/flutter/project`
2. Observe startup sequence

**Expected:**
1. Brief "Not Connected" normal mode (may be very quick)
2. Loading screen appears with spinner animation
3. "Detecting devices..." message
4. "Preparing launch..." message
5. Session starts, logs appear
6. Status bar shows running session

**Verify:**
- [ ] Loading spinner animates smoothly
- [ ] Messages update during discovery
- [ ] Session starts correctly
- [ ] No visual glitches

---

#### Scenario 2: Auto-Start with No Devices

**Setup:**
- `auto_start = true`
- No devices connected, no emulators running

**Steps:**
1. Disconnect all devices, close emulators
2. Run `cargo run -- /path/to/flutter/project`
3. Observe behavior

**Expected:**
1. Brief normal mode
2. Loading screen with discovery
3. StartupDialog appears with error message
4. Error explains "No devices found"

**Verify:**
- [ ] Error message is clear and helpful
- [ ] User can start emulator from dialog
- [ ] User can retry device discovery

---

#### Scenario 3: Manual Start (No Auto-Start)

**Setup:**
- `.fdemon/config.toml` with `auto_start = false` (or no config file)

**Steps:**
1. Run `cargo run -- /path/to/flutter/project`
2. Observe initial state
3. Press '+' to start session

**Expected:**
1. Normal mode immediately ("Not Connected")
2. Status bar shows disconnected state
3. Pressing '+' shows StartupDialog
4. Can select device and launch

**Verify:**
- [ ] No loading screen on startup
- [ ] '+' key works correctly
- [ ] StartupDialog functions normally

---

#### Scenario 4: Device Cache After Auto-Start

**Setup:**
- `auto_start = true`
- Device connected

**Steps:**
1. Start app, let auto-start complete
2. Press '+' to open StartupDialog
3. Observe device list

**Expected:**
1. Device list appears immediately (from cache)
2. No "loading devices..." delay
3. Background refresh may show briefly

**Verify:**
- [ ] Devices appear instantly
- [ ] Cache is populated from auto-launch discovery

---

#### Scenario 5: Key Press During Loading

**Setup:**
- `auto_start = true`

**Steps:**
1. Start app
2. Quickly press '+' while loading screen is visible
3. Observe behavior

**Expected:**
1. '+' key is ignored
2. Loading continues uninterrupted
3. No dialogs appear during loading

**Verify:**
- [ ] '+' is blocked during loading
- [ ] 'd' is blocked during loading
- [ ] Escape doesn't crash

---

#### Scenario 6: Saved Selection Persistence

**Setup:**
- `auto_start = true`
- Device connected
- Previous session saved a selection

**Steps:**
1. Run app, select a device, launch session
2. Quit app
3. Run app again

**Expected:**
1. Auto-start uses saved device/config from `.fdemon/settings.local.toml`
2. Same device is selected without user input

**Verify:**
- [ ] Saved selection is loaded
- [ ] Correct device is used
- [ ] Correct config (if any) is used

---

### Issue Documentation

If issues are found, document them here:

| Issue | Severity | Description | Follow-up |
|-------|----------|-------------|-----------|
| (none yet) | | | |

### Acceptance Criteria

1. All 6 scenarios pass
2. No crashes or panics
3. UI is responsive and animations are smooth
4. Error messages are clear and helpful
5. Any issues are documented for follow-up

### Notes

- Run tests on both macOS and Linux if possible
- Test with both Android emulator and iOS simulator if available
- Pay attention to timing - the "brief normal mode" should be very quick
- If issues are found, create follow-up tasks before completing this task

---

## Completion Summary

**Status:** Not Started

**Scenarios Tested:**
- [ ] Scenario 1: Auto-start with device
- [ ] Scenario 2: Auto-start no devices
- [ ] Scenario 3: Manual start
- [ ] Scenario 4: Device cache
- [ ] Scenario 5: Key press during loading
- [ ] Scenario 6: Saved selection

**Issues Found:**

(pending)

**Notes:**

(pending)
