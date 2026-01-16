# Feature: NewSessionDialog Polish & Bug Fixes

## Summary

Address four issues identified after the NewSessionDialog implementation to improve usability, fix bugs, and enhance performance.

## Problem Statement

1. **Responsive Layout Missing**: Terminal shows "Terminal too small. Need at least 80x24" instead of adapting to smaller sizes
2. **Scrollable Sections Missing**: Device lists are clipped when they exceed available height; users can't see selected items
3. **Emulator/Simulator Boot Broken**: "Unknown platform: ios/android" error prevents booting any emulator or simulator
4. **Device Discovery Not Cached**: Dialog always shows loading spinner, even when devices were discovered seconds ago

---

## Issue 1: Responsive Layout

### Current Behavior

When terminal width < 80 or height < 24, the dialog displays:
```
Terminal too small. Need at least 80x24 (current: 75x38)
```

No content is rendered - the entire dialog is replaced with this error message.

### Investigation Findings

**Location:** `src/tui/widgets/new_session_dialog/mod.rs`

| Item | Location | Description |
|------|----------|-------------|
| `MIN_WIDTH` | Line 54 | Hard-coded constant: `80` |
| `MIN_HEIGHT` | Line 57 | Hard-coded constant: `24` |
| `fits_in_area()` | Lines 156-158 | Boolean check: `width >= 80 && height >= 24` |
| `render_too_small()` | Lines 160-180 | Renders error message in red |
| `Widget::render()` | Lines 183-226 | Checks size first, renders error or dialog |

**Key Issue:** The dialog has a single horizontal layout (Target Selector left, Launch Context right) requiring ~80 columns. There's no vertical layout fallback for narrow terminals.

**Inconsistency:** The main app UI supports compact mode down to ~40 columns (`src/tui/layout.rs`), but the dialog doesn't adapt.

### Proposed Solution

Implement a **responsive layout system** with two modes:

1. **Horizontal Layout** (current, for wider terminals):
   ```
   ┌─────────────────────────────────────────────────────────────┐
   │  ┌── Target Selector ──────┐ ┌── Launch Context ────────┐  │
   │  │        50% width        │ │       50% width          │  │
   │  └─────────────────────────┘ └──────────────────────────┘  │
   └─────────────────────────────────────────────────────────────┘
   ```

2. **Vertical Layout** (new, for narrow terminals):
   ```
   ┌─────────────────────────────┐
   │  ┌── Target Selector ────┐  │
   │  │       100% width      │  │
   │  │       ~60% height     │  │
   │  └───────────────────────┘  │
   │  ┌── Launch Context ─────┐  │
   │  │       100% width      │  │
   │  │       ~40% height     │  │
   │  └───────────────────────┘  │
   └─────────────────────────────┘
   ```

**Thresholds:**
- **Horizontal mode**: width >= 70 (narrower than current 80)
- **Vertical mode**: width >= 40, height >= 20
- **Too small**: below 40x20

**Files to Modify:**
- `src/tui/widgets/new_session_dialog/mod.rs` - Layout mode detection and rendering
- `src/tui/widgets/new_session_dialog/target_selector.rs` - Adapt to variable width
- `src/tui/widgets/new_session_dialog/launch_context.rs` - Adapt to variable width

---

## Issue 2: Scrollable Sections

### Current Behavior

Device lists in the Target Selector are rendered without scrolling:
- All devices are converted to `ListItem` and passed to `List::new()`
- Items beyond the visible area are simply **not rendered** (clipped)
- User can navigate with Up/Down keys, but selected items may be off-screen
- No visual indication that more items exist

### Investigation Findings

**Location:** `src/tui/widgets/new_session_dialog/device_list.rs`

| Item | Location | Description |
|------|----------|-------------|
| `ConnectedDeviceList::render()` | Lines 105-118 | Renders all items without scroll state |
| `BootableDeviceList::render()` | Lines 209-244 | Renders all items without scroll state |
| `calculate_scroll_offset()` | Lines 247-277 | **Exists but UNUSED** |
| Unit tests | Lines 394-427 | Tests for scroll calculation (passing) |

**State Structure:** `TargetSelectorState` (lines 21-66 in `target_selector.rs`)
- Has `selected_index: usize` for tracking selection
- **Missing:** `scroll_offset: usize` for tracking visible range

**Comparison:** Other modals in the same dialog **DO** implement scrolling:
- `FuzzyModalState` has `scroll_offset` (line 31-32 in `state.rs`)
- `DartDefinesModalState` has `scroll_offset` (lines 191-192)
- Both have `adjust_scroll()` methods that keep selection visible

**Pattern Exists:** `src/tui/selector.rs` shows correct pattern using `ListState` from ratatui.

### Proposed Solution

Add scroll support to device lists using the **existing `calculate_scroll_offset()` function**:

1. **Add scroll state to `TargetSelectorState`:**
   ```rust
   pub struct TargetSelectorState {
       // ... existing fields ...
       pub scroll_offset: usize,  // NEW
   }
   ```

2. **Update `adjust_scroll()` after navigation:**
   ```rust
   pub fn select_next(&mut self) {
       // ... existing logic ...
       self.adjust_scroll(visible_height);
   }

   fn adjust_scroll(&mut self, visible_height: usize) {
       self.scroll_offset = calculate_scroll_offset(
           self.selected_index,
           visible_height,
           self.scroll_offset,
       );
   }
   ```

3. **Modify rendering to use scroll offset:**
   - Skip items before `scroll_offset`
   - Only render items within visible range
   - Add scroll indicators (e.g., "↑ more" / "↓ more")

**Files to Modify:**
- `src/tui/widgets/new_session_dialog/target_selector.rs` - Add scroll_offset field
- `src/tui/widgets/new_session_dialog/device_list.rs` - Use scroll offset in rendering
- `src/app/handler/new_session/target_selector.rs` - Call adjust_scroll after navigation

---

## Issue 3: Emulator/Simulator Boot Failure

### Current Behavior

When user presses `b` to boot a simulator/emulator:
```
Failed to boot 6488AF1E-BC33-445B-90BB-564A3AB30F89: Unknown platform: ios
Failed to boot Pixel_9_Pro_Fold: Unknown platform: android
```

The boot functions are never called - the error comes from platform string matching.

### Investigation Findings

**Root Cause:** Case mismatch between platform strings.

**Bug Location:** `src/app/handler/new_session/target_selector.rs` (lines 51-58)

```rust
// Current code sets LOWERCASE strings:
GroupedBootableDevice::IosSimulator(sim) => {
    (sim.udid.clone(), "ios".to_string())  // ← lowercase
}
GroupedBootableDevice::AndroidAvd(avd) => {
    (avd.name.clone(), "android".to_string())  // ← lowercase
}
```

**Mismatch Location:** `src/tui/spawn.rs` (lines 311-323)

```rust
// Spawner expects CAPITALIZED strings:
let result = match platform.as_str() {
    "iOS" => crate::daemon::boot_simulator(&device_id).await,     // ← capitalized
    "Android" => crate::daemon::boot_avd(&device_id, ...).await,  // ← capitalized
    _ => {
        // Falls here with "ios" or "android" → "Unknown platform" error
    }
};
```

**Type-Safe Alternative Exists:** `src/core/types.rs` has a `Platform` enum (lines 625-637) with proper `Display` implementation that outputs "iOS"/"Android". However, the boot flow uses raw `String` instead.

### Proposed Solution

**Option A: Quick Fix (string case correction)**
- Change lines 53 and 56 in `target_selector.rs` to use capitalized strings
- Minimal change, immediate fix
- Risk: Still uses fragile string matching

**Option B: Type-Safe Fix (recommended)**
- Use `core::Platform` enum throughout the boot flow
- Update `UpdateAction::BootDevice` to use `Platform` instead of `String`
- Update `Message::BootDevice` to use `Platform` instead of `String`
- Match on enum variants instead of strings
- Compiler-enforced correctness

**Files to Modify:**
- `src/app/handler/new_session/target_selector.rs` - Fix platform string or use enum
- `src/app/message.rs` - Change `BootDevice.platform` type (for Option B)
- `src/app/handler/mod.rs` - Change `UpdateAction::BootDevice.platform` type (for Option B)
- `src/tui/spawn.rs` - Update `spawn_device_boot()` signature (for Option B)

---

## Issue 4: Device List Caching

### Current Behavior

Every time the NewSessionDialog opens:
1. Device list is empty (spinner shown)
2. `flutter devices --machine` command runs (3-5 seconds)
3. Results populate the list

Even if user opened the dialog 5 seconds ago, they wait again.

### Investigation Findings

**Cache Infrastructure EXISTS but is UNUSED:**

**Cache Fields:** `src/app/state.rs` (lines 335-342)
```rust
/// Global device cache (used by NewSessionDialog)
pub device_cache: Option<Vec<Device>>,
pub devices_last_updated: Option<std::time::Instant>,
```

**Cache Methods:**
- `get_cached_devices()` - Returns cached devices if < 30 seconds old (lines 497-508)
- `set_device_cache()` - Updates cache with timestamp (lines 514-517)

**Cache is UPDATED by:**
- `DevicesDiscovered` message handler (line 287 in `update.rs`)
- Startup auto-launch (`startup.rs` lines 118, 163)

**Cache is NEVER CHECKED when dialog opens:**

`src/app/handler/new_session/navigation.rs` (line 161):
```rust
pub fn handle_open_new_session_dialog(state: &mut AppState) -> UpdateResult {
    let configs = crate::config::load_all_configs(&state.project_path);
    state.show_new_session_dialog(configs);
    UpdateResult::action(UpdateAction::DiscoverDevices)  // ALWAYS discovers!
}
```

**Design Intent:** Comments mention "Task 08e - Device Cache Sharing" but implementation is incomplete.

### Proposed Solution

1. **Check cache before triggering discovery:**
   ```rust
   pub fn handle_open_new_session_dialog(state: &mut AppState) -> UpdateResult {
       let configs = crate::config::load_all_configs(&state.project_path);
       state.show_new_session_dialog(configs);

       // Check cache first
       if let Some(cached_devices) = state.get_cached_devices() {
           state.new_session_dialog_state.target_selector
               .set_connected_devices(cached_devices.clone());
           // Still trigger background refresh for freshness
           return UpdateResult::action(UpdateAction::DiscoverDevicesBackground);
       }

       // Cache miss - show loading and discover
       state.new_session_dialog_state.target_selector.loading = true;
       UpdateResult::action(UpdateAction::DiscoverDevices)
   }
   ```

2. **Add background refresh action:**
   - New `UpdateAction::DiscoverDevicesBackground` that updates cache silently
   - No loading spinner shown
   - Results update the list if dialog is still open

3. **Populate dialog with cached devices on initialization:**
   - Modify `show_new_session_dialog()` to accept optional cached devices
   - Or add `new_with_cached_devices()` constructor to `NewSessionDialogState`

**Benefits:**
- Instant device list if cache is fresh (< 30 seconds)
- Background refresh keeps data current
- No waiting for quick re-opens

**Files to Modify:**
- `src/app/handler/new_session/navigation.rs` - Add cache check
- `src/app/handler/mod.rs` - Add `DiscoverDevicesBackground` action
- `src/tui/spawn.rs` - Handle background discovery action
- `src/app/state.rs` - Optional: modify `show_new_session_dialog()` signature

---

## Design Decisions

### Issue 1: Responsive Layout
- **Decision:** Vertical layout at width < 70 (not too aggressive)
- **Rationale:** Allows usage in split terminals, IDE embedded terminals
- **Trade-off:** More complex layout logic, but better accessibility

### Issue 2: Scrollable Sections
- **Decision:** Manual scroll offset (not ratatui ListState)
- **Rationale:** Consistent with existing FuzzyModal/DartDefinesModal patterns
- **Trade-off:** More code, but matches project conventions

### Issue 3: Boot Fix
- **Decision:** Use Platform enum (Option B) instead of quick string fix
- **Rationale:** Type-safe, prevents future regressions, follows Rust best practices
- **Trade-off:** More files to modify, but correct long-term solution

### Issue 4: Caching
- **Decision:** Cache-first with background refresh
- **Rationale:** Best UX - instant results plus fresh data
- **Trade-off:** Slightly more complex flow, but significantly better perceived performance

---

## Success Criteria

1. **Responsive Layout:**
   - [ ] Dialog renders in vertical mode at 60x30 terminal
   - [ ] Dialog renders in horizontal mode at 100x40 terminal
   - [ ] "Too small" message only shown below 40x20
   - [ ] All functionality works in both modes

2. **Scrollable Sections:**
   - [ ] Device list scrolls when > 10 items
   - [ ] Selected item always visible
   - [ ] Scroll indicators show when content is clipped
   - [ ] Works for both Connected and Bootable tabs

3. **Emulator/Simulator Boot:**
   - [ ] iOS simulator boots successfully from Bootable tab
   - [ ] Android AVD boots successfully from Bootable tab
   - [ ] Booted device appears in Connected tab
   - [ ] No "Unknown platform" errors

4. **Device Caching:**
   - [ ] Dialog opens instantly with cached devices (< 100ms)
   - [ ] Background refresh updates list silently
   - [ ] Cache expires after 30 seconds
   - [ ] Loading spinner only shown on cache miss

---

## Verification

```bash
cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings
```

**Manual Testing:**
1. Resize terminal to various sizes, verify layout adapts
2. Connect 15+ devices, verify scrolling works
3. Boot iOS simulator from Bootable tab
4. Boot Android AVD from Bootable tab
5. Open dialog, close, open again quickly - verify instant device list

---

## Files Summary

### Files to Modify

| File | Issue | Changes |
|------|-------|---------|
| `src/tui/widgets/new_session_dialog/mod.rs` | 1 | Layout mode detection, vertical layout rendering |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | 1, 2 | Add scroll_offset, adapt to variable width |
| `src/tui/widgets/new_session_dialog/device_list.rs` | 2 | Use scroll offset, add scroll indicators |
| `src/tui/widgets/new_session_dialog/launch_context.rs` | 1 | Adapt to variable width |
| `src/app/handler/new_session/target_selector.rs` | 2, 3 | adjust_scroll calls, fix platform strings/enum |
| `src/app/handler/new_session/navigation.rs` | 4 | Add cache check before discovery |
| `src/app/message.rs` | 3 | Change BootDevice.platform type |
| `src/app/handler/mod.rs` | 3, 4 | Change UpdateAction, add background action |
| `src/tui/spawn.rs` | 3, 4 | Update boot matching, handle background discovery |
| `src/app/state.rs` | 4 | Optional: modify show_new_session_dialog |

---

## References

- Original NewSessionDialog plan: `workflow/plans/features/new-session-dialog/PLAN.md`
- Phase 8 implementation: `workflow/plans/features/new-session-dialog/phase-8/`
- Platform enum: `src/core/types.rs:625-637`
- Scroll offset helper: `src/tui/widgets/new_session_dialog/device_list.rs:247-277`
- Device cache: `src/app/state.rs:335-342, 497-517`
