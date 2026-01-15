# Action Items: Phase 5 - Target Selector Widget

**Review Date:** 2026-01-15
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 2

---

## Critical Issues (Must Fix)

### 1. Rename BootableDevice Enum in TUI Layer

- **Source:** Architecture Enforcer
- **File:** `src/tui/widgets/new_session_dialog/device_groups.rs:109`
- **Problem:** Type name `BootableDevice` conflicts with `core::BootableDevice`. Two different types with same name creates confusion and import ambiguity.
- **Required Action:** Rename enum from `BootableDevice` to `GroupedBootableDevice` or `BootableDeviceVariant`
- **Files to Update:**
  - `src/tui/widgets/new_session_dialog/device_groups.rs` - Rename enum and update all internal references
  - `src/tui/widgets/new_session_dialog/device_list.rs` - Update import and usages
  - `src/tui/widgets/new_session_dialog/target_selector.rs` - Update import and usages
  - `src/tui/widgets/new_session_dialog/mod.rs` - Update re-export
  - All test files using the type
- **Acceptance:** `cargo check` passes with renamed type, no ambiguous import errors

### 2. Fix Selection Index Reset Logic

- **Source:** Logic Reasoning Checker
- **File:** `src/tui/widgets/new_session_dialog/state.rs:653`
- **Problem:** `NewSessionDialogState.switch_tab()` resets `selected_target_index = 0` blindly, which might point to a header (non-selectable item)
- **Required Action:** Change reset logic to use `first_selectable_index()` pattern like `TargetSelectorState.set_tab()` does
- **Acceptance:** After tab switch, selection always points to a device, not a header

---

## Major Issues (Should Fix)

### 3. Consolidate Loading Flag Management

- **Source:** Logic Reasoning Checker
- **File:** `src/app/handler/update.rs:1738-1751` and `src/tui/widgets/new_session_dialog/state.rs:656`
- **Problem:** Both handler and state method set `loading_bootable` flag, creating potential race conditions
- **Suggested Action:** Choose one source of truth:
  - Option A: Handler manages flags (preferred for TEA purity)
  - Option B: State method manages flags with handler trusting state
- **Acceptance:** Only one location sets/clears loading flags

### 4. Standardize Error Clearing Logic

- **Source:** Logic Reasoning Checker, Code Quality Inspector
- **Files:**
  - `src/tui/widgets/new_session_dialog/state.rs:752` (set_connected_devices)
  - `src/tui/widgets/new_session_dialog/target_selector.rs:183-184` (set_connected_devices)
  - `src/app/handler/update.rs:1799-1809` (DeviceDiscoveryFailed)
- **Problem:**
  - State methods inconsistent in clearing errors
  - Discovery failure clears BOTH loading flags instead of just the relevant one
- **Suggested Action:**
  - Add `self.error = None;` to `NewSessionDialogState.set_connected_devices()`
  - Change `DeviceDiscoveryFailed` handler to only clear the appropriate flag (may need message to include context about which discovery failed)
- **Acceptance:** Error clearing behavior is consistent across all state methods

### 5. Add Error Feedback for Empty Device Selection

- **Source:** Code Quality Inspector
- **File:** `src/app/handler/update.rs:1768-1785`
- **Problem:** When no device is selected on Bootable tab, handler silently returns `None`
- **Suggested Action:** Add logging and/or set error state:
  ```rust
  if let Some(device) = state.new_session_dialog_state.selected_bootable_device() {
      // ... boot logic
  } else {
      warn!("Cannot boot device: no device selected");
      // Optionally: state.new_session_dialog_state.set_error("No device selected".to_string());
  }
  ```
- **Acceptance:** Silent failures are logged at minimum

### 6. Optimize Navigation Performance

- **Source:** Code Quality Inspector, Risks Tradeoffs Analyzer
- **File:** `src/tui/widgets/new_session_dialog/target_selector.rs:118-147`
- **Problem:** `current_flat_list()` clones all device strings on every navigation (Up/Down key)
- **Suggested Action:**
  - Cache flattened list in `TargetSelectorState`
  - Invalidate cache when devices are updated
  - Benchmark with 100+ devices to confirm improvement
- **Acceptance:** No new allocations during navigation; verified with benchmarks

---

## Minor Issues (Consider Fixing)

### 7. Remove or Document scroll_offset Field

- **File:** `src/tui/widgets/new_session_dialog/target_selector.rs`
- **Problem:** Field exists but is never used
- **Suggested Action:** Either:
  - Remove the field entirely
  - Add `#[allow(dead_code)]` with TODO comment referencing future task
- **Acceptance:** No dead code warnings or clear documentation of intent

### 8. Add Missing Documentation

- **Files:**
  - `src/tui/widgets/new_session_dialog/target_selector.rs` - `TargetSelectorState::new()`
  - `src/tui/widgets/new_session_dialog/device_list.rs` - `calculate_scroll_offset()`
  - `src/tui/widgets/new_session_dialog/device_list.rs` - `DeviceListStyles`
- **Suggested Action:** Add `///` doc comments explaining purpose and usage
- **Acceptance:** `cargo doc` builds with no "missing documentation" warnings for public items

### 9. Add Defensive Navigation Check

- **File:** `src/tui/widgets/new_session_dialog/device_groups.rs:215-241`
- **Problem:** If `selected_index` points to a header, `next_selectable()` behavior is unpredictable
- **Suggested Action:** Add validation that finds nearest selectable index instead of defaulting to 0
- **Acceptance:** Navigation is predictable even if state becomes corrupted

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Critical issue #1 resolved: BootableDevice enum renamed
- [ ] Critical issue #2 resolved: Selection index reset correctly
- [ ] Major issues #3-#6 resolved or justified
- [ ] `cargo fmt` - Code formatted
- [ ] `cargo check` - No compilation errors
- [ ] `cargo test` - All tests pass
- [ ] `cargo clippy -- -D warnings` - No clippy warnings
- [ ] Type conflict no longer exists (no ambiguous imports)
- [ ] Navigation works correctly when tab switching

---

## Pre-existing Issues (Track Separately)

### Layer Violation in core/events.rs

- **File:** `src/core/events.rs:3`
- **Issue:** `use crate::daemon::DaemonMessage;` - Core layer should not depend on daemon layer
- **Action:** Create separate tracking issue for architectural cleanup
- **Not blocking for Phase 5 merge**

---

Generated with Claude Code
