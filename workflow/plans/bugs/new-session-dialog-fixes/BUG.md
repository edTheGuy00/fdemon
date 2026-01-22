# Bugfix Plan: New Session Dialog Issues

## TL;DR

Six bugs affecting the NewSessionDialog: (1-2) Device caching not working properly - connected devices aren't pre-populated from cache on dialog open, and bootable devices discovery isn't triggered at startup; (3-4) Portrait layout missing borders/titles and using abbreviated mode labels despite available space; (5) ESC key not working in fuzzy search modals; (6) No auto-creation of default config when flavor/dart-defines are set without a config selected. Fixes involve cache pre-population, startup discovery triggers, responsive layout rendering, key handler verification, and auto-config creation logic.

---

## Bug Reports

### Bug 1: Connected Devices Not Cached on First Launch

**Symptom:** When the new session dialog opens, devices take time to load and appear to not be cached. After launching a session and reopening the dialog, devices must be discovered again. Only on the third open do devices appear instantly.

**Expected:** After initial device discovery, subsequent dialog opens should instantly show cached devices while background refresh occurs.

**Root Cause Analysis:**

1. `show_new_session_dialog()` at `src/app/state.rs:399-401` creates a **NEW** `NewSessionDialogState` without checking the existing device cache
2. The cache (`device_cache` field in `AppState`) is populated when `Message::DevicesDiscovered` is received at `src/app/handler/update.rs:287`
3. However, the cache is **never read** when opening the dialog - it always starts with an empty device list and `loading: true`

**Affected Files:**
- `src/app/state.rs:399-401` - `show_new_session_dialog()` doesn't use cache
- `src/app/state.rs:493-517` - Cache methods exist but aren't called on dialog open

---

### Bug 2: Bootable Devices List Never Populates on First Open

**Symptom:** Bootable devices (emulators/simulators) list is empty on first dialog open. Only on second dialog open are bootable devices visible. Pressing "r" to refresh appears to do nothing.

**Expected:** Bootable device discovery should be triggered at startup (after tool availability check), and the list should populate on first dialog open.

**Root Cause Analysis:**

1. At startup (`src/tui/runner.rs:68-72`), only `spawn_device_discovery()` is called - **NO** call to `spawn_bootable_device_discovery()`
2. Bootable discovery depends on `tool_availability` (xcrun_simctl, android_emulator), which is checked asynchronously
3. `TargetSelectorState::default()` sets `bootable_loading: false` (line 71), so the bootable tab shows empty list (not loading state)
4. The refresh handler at `src/app/handler/new_session/target_selector.rs:82-98` IS correctly implemented - "r" key DOES trigger bootable discovery when on the Bootable tab
5. **Likely issue:** Either the key isn't being routed correctly, or the discovery results aren't being rendered (race condition with tool availability)

**Affected Files:**
- `src/tui/runner.rs:68-72` - Missing `spawn_bootable_device_discovery()` at startup
- `src/app/handler/update.rs:1031-1042` - `ToolAvailabilityChecked` handler should trigger bootable discovery
- `src/tui/widgets/new_session_dialog/target_selector.rs:56-71` - Default state shows empty (not loading)

---

### Bug 3: Portrait Layout Missing Section Titles and Borders

**Symptom:** In portrait layout, the Target Selector and Launch Context sections do not have visible borders or titles, unlike the horizontal layout which has clearly separated sections with titles.

**Expected:** Portrait layout should maintain visual consistency with horizontal layout - sections should have borders and titles for clarity.

**Root Cause Analysis:**

1. `render_vertical()` at `src/tui/widgets/new_session_dialog/mod.rs:339-406` uses `render_compact()` methods
2. `TargetSelector::render_compact()` at `target_selector.rs:426-467` renders **WITHOUT** a `Block` wrapper (no border, no title)
3. `LaunchContextWithDevice::render_compact()` at `launch_context.rs:771-804` similarly renders **WITHOUT** borders
4. Contrast with full mode: `render_full()` uses `Block::default().title(" Target Selector ").borders(Borders::ALL)`
5. **Design decision:** Compact mode was designed to maximize content space, but this sacrifices visual clarity

**Affected Files:**
- `src/tui/widgets/new_session_dialog/target_selector.rs:426-467` - `render_compact()` missing Block
- `src/tui/widgets/new_session_dialog/launch_context.rs:771-804` - `render_compact()` missing Block

---

### Bug 4: Portrait Layout Content Not Using Full Width

**Symptom:** Mode buttons show abbreviated text "Dbg", "Prof", "Rel" instead of full words "Debug", "Profile", "Release" despite having enough horizontal space available.

**Expected:** Mode buttons should display full labels when sufficient width is available (portrait mode has 40-69 columns).

**Root Cause Analysis:**

1. `render_mode_inline()` at `src/tui/widgets/new_session_dialog/launch_context.rs:806-874` uses **HARDCODED** abbreviated labels:
   ```rust
   "(●)Dbg"   // 6 chars
   "(●)Prof"  // 7 chars
   "(●)Rel"   // 6 chars
   ```
2. Portrait layout threshold is `MIN_VERTICAL_WIDTH: u16 = 40` (mod.rs:121)
3. Dialog uses 90% of terminal width (`mod.rs:345`), so a 50-width terminal gives 45 columns
4. Full labels (`"(●) Debug"`, `"(●) Profile"`, `"(●) Release"`) need ~40 chars total - **FITS in portrait mode**
5. The `area.width` parameter is available but **NOT CHECKED** for adaptive label selection

**Affected Files:**
- `src/tui/widgets/new_session_dialog/launch_context.rs:806-874` - `render_mode_inline()` hardcoded labels

---

### Bug 5: Fuzzy Search Modals Cannot Be Exited (ESC Does Nothing)

**Status:** ⚠️ DEFERRED - Needs Further Investigation

> **Note:** This bug was reported but could not be reproduced during testing. The ESC key now works correctly. The issue may have been intermittent or related to a specific terminal/environment. Marking for future investigation if it resurfaces.

**Symptom:** When in fuzzy search modals for config, flavors, or dart defines, pressing ESC does nothing - the user is stuck in the modal.

**Expected:** Pressing ESC should close the fuzzy modal and return to the main dialog.

**Root Cause Analysis:**

The code analysis shows ESC handling **IS implemented**:

1. Key routing at `src/app/handler/keys.rs:462-463` checks `is_fuzzy_modal_open()` FIRST
2. `handle_fuzzy_modal_key()` at `keys.rs:483-493` returns `Message::NewSessionDialogCloseFuzzyModal` on ESC
3. `handle_dart_defines_modal_key()` at `keys.rs:495-506` returns `Message::NewSessionDialogCloseDartDefinesModal` on ESC
4. Handlers at `update.rs:973,994` call `close_modal()` and `close_dart_defines_modal_with_changes()`
5. Tests exist and **PASS**: `test_escape_closes_fuzzy_modal`, `test_escape_closes_dart_defines_modal`

**Potential Causes (if bug resurfaces):**
- Terminal not sending ESC correctly (some terminals have ESC delay)
- Key event not reaching handler (routing issue)
- Modal state not being cleared properly (state corruption)
- Double-ESC required (first exits input mode, second closes modal)

**Affected Files:**
- `src/app/handler/keys.rs:483-506` - Modal key handlers (verify routing)
- `src/app/handler/new_session/fuzzy_modal.rs:42-46` - `handle_close_fuzzy_modal()` (verify execution)
- `src/app/new_session_dialog/state.rs:703-706` - `close_modal()` (verify state change)

**Action:** No fix needed at this time. Monitor for recurrence.

---

### Bug 6: No Auto-Creation of Default Config When Flavor/Dart-Defines Set

**Symptom:** If no configuration is selected and the user sets flavor or dart-defines values, these values are used for the current launch but are NOT saved. On next dialog open, the values are lost.

**Expected:** When flavor/dart-defines are set without a config selected, automatically create a new "Default" config and save it to `.fdemon/launch.toml`.

**Root Cause Analysis:**

1. Auto-save logic at `src/app/handler/new_session/launch_context.rs:148-167` requires `selected_config_index.is_some()`:
   ```rust
   let should_auto_save = if let Some(config_idx) = selected_config_index {
       // Check if FDemon source...
   } else {
       false  // ← NO AUTO-SAVE when no config selected
   };
   ```
2. At launch time (`launch_context.rs:288-311`), a **transient** `LaunchConfig` is created but NOT saved to `LoadedConfigs` or disk
3. Helper functions exist: `create_default_launch_config()` at `config/launch.rs:151-163` and `add_launch_config()` at `config/launch.rs:165-186`
4. These helpers are **NEVER CALLED** from the NewSessionDialog flow

**Affected Files:**
- `src/app/handler/new_session/launch_context.rs:148-167, 208-227` - Auto-save condition too strict
- `src/config/launch.rs:151-186` - Config creation helpers (need to be called)
- `src/app/new_session_dialog/state.rs` - May need helper to insert new config

---

## Affected Modules

| Module | Changes Required |
|--------|------------------|
| `src/app/state.rs` | Pre-populate devices from cache when opening dialog |
| `src/tui/runner.rs` | Trigger bootable discovery after tool availability check |
| `src/app/handler/update.rs` | Chain bootable discovery from `ToolAvailabilityChecked` |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Add borders/titles to `render_compact()` |
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Add borders to compact, responsive mode labels |
| `src/app/handler/keys.rs` | Verify ESC key routing (may need debug logging) |
| `src/app/handler/new_session/launch_context.rs` | Auto-create config when flavor/dart-defines set |
| `src/config/launch.rs` | (Already has helpers, may need minor adjustments) |

---

## Phases

### Phase 1: Device Discovery & Caching Fixes (Bugs 1 & 2) - Critical

These bugs severely impact first-run experience. Users cannot select devices efficiently.

**Steps:**

1. **Pre-populate from cache on dialog open** (`src/app/state.rs`)
   - In `show_new_session_dialog()`, check `get_cached_devices()`
   - If cache exists, call `target_selector.set_connected_devices(cached_devices)`
   - Keep `loading: true` to indicate background refresh is happening

2. **Trigger bootable discovery after tool check** (`src/app/handler/update.rs`)
   - In `Message::ToolAvailabilityChecked` handler, return `UpdateAction::DiscoverBootableDevices`
   - This ensures bootable discovery happens AFTER we know which tools are available

3. **Initialize bootable tab with loading state** (`target_selector.rs`)
   - Change default `bootable_loading: true` to show loading indicator
   - Or trigger bootable discovery on tab switch if list is empty

4. **Add debug logging to verify "r" key flow** (temporary, for investigation)
   - Add `tracing::debug!` in `handle_refresh_devices()` to verify it's being called

**Measurable Outcomes:**
- Second dialog open shows devices instantly (from cache)
- Bootable tab populates automatically on first dialog open
- "r" key refreshes devices (with debug confirmation)

---

### Phase 2: Portrait Layout Styling Fixes (Bugs 3 & 4)

Visual polish to ensure portrait layout is as usable as horizontal layout.

**Steps:**

1. **Add borders and titles to compact mode** (`target_selector.rs`, `launch_context.rs`)
   - Wrap compact render content in `Block::default().title(...).borders(Borders::ALL)`
   - Use slightly thinner borders (e.g., `PLAIN` instead of `ROUNDED`) to save space
   - Or use top-only border with inline title to minimize vertical space

2. **Add responsive mode labels** (`launch_context.rs`)
   - In `render_mode_inline()`, check `area.width`
   - If width >= 45, use full labels: "Debug", "Profile", "Release"
   - If width < 45, use abbreviated: "Dbg", "Prof", "Rel"

**Measurable Outcomes:**
- Portrait layout shows section titles and borders
- Mode buttons show full labels when space allows

---

### Phase 3: Modal ESC Key Fix (Bug 5) - DEFERRED

**Status:** ⚠️ Deferred - Bug could not be reproduced. ESC key works correctly in current testing.

**Action:** No tasks planned. Will revisit if issue resurfaces.

**If issue resurfaces, investigate:**
1. Add debug logging to key handlers
2. Test in multiple terminals (iTerm2, Terminal.app, VS Code terminal)
3. Check for race conditions or state corruption

---

### Phase 4: Auto-Config Creation (Bug 6)

Improves UX by persisting user-entered values automatically.

**Steps:**

1. **Add auto-create logic in flavor handler** (`launch_context.rs`)
   - After setting flavor, check if `selected_config_index.is_none()` AND `flavor.is_some()`
   - If true, create new config with `create_default_launch_config()`
   - Add config to `LoadedConfigs` and update `selected_config_index`
   - Trigger `UpdateAction::AutoSaveConfig`

2. **Add auto-create logic in dart-defines handler** (`launch_context.rs`)
   - Same pattern as flavor handler
   - Check if `selected_config_index.is_none()` AND `!dart_defines.is_empty()`

3. **Handle unique naming**
   - Use `add_launch_config()` which handles unique naming ("Default", "Default 2", etc.)

4. **Add helper to state** (`state.rs`)
   - Consider adding `create_and_select_default_config()` method to encapsulate logic

**Measurable Outcomes:**
- Setting flavor without config selected creates "Default" config
- Setting dart-defines without config selected creates "Default" config
- Config is saved to `.fdemon/launch.toml`
- Next dialog open shows the created config pre-selected

---

## Edge Cases & Risks

### Device Caching
- **Risk:** Stale cache showing outdated device list
- **Mitigation:** Always trigger background refresh; cache has TTL (5 seconds from `DEVICE_CACHE_TTL`)

### Bootable Discovery
- **Risk:** Tool availability check fails or times out
- **Mitigation:** Gracefully handle unavailable tools; show appropriate error message

### Portrait Layout Borders
- **Risk:** Adding borders reduces usable content area
- **Mitigation:** Use minimal borders (single line or top-only) to minimize space usage

### Mode Label Responsiveness
- **Risk:** Calculation of available width might not account for padding/margins
- **Mitigation:** Use conservative threshold (e.g., 45 columns) with buffer

### ESC Key Handling
- **Risk:** Fix might break other key handling
- **Mitigation:** Comprehensive testing of all keyboard shortcuts in dialog

### Auto-Config Creation
- **Risk:** Creating configs user didn't explicitly request
- **Mitigation:** Only create when user explicitly sets flavor/dart-defines; use clear naming ("Default")

---

## Further Considerations

1. **Cache TTL Strategy:** Should cache TTL be configurable? Current hardcoded 5 seconds may be too short for some workflows.

2. **Bootable Discovery Frequency:** Should bootable devices be cached like connected devices? Currently they're not cached.

3. **Portrait Layout Threshold:** Should the 70-column threshold for horizontal layout be configurable or adaptive based on content?

4. **Auto-Config Naming:** Should the auto-created config be named "Default" or something more descriptive like "Quick Launch" or "Session Config"?

---

## Task Dependency Graph

```
Phase 1: Device Discovery & Caching
├── 01-cache-preload (Bug 1)
├── 02-bootable-discovery-startup (Bug 2)
│   └── depends on: tool availability check
└── 03-refresh-key-verification (Bug 2)

Phase 2: Portrait Layout
├── 04-compact-borders-titles (Bug 3)
└── 05-responsive-mode-labels (Bug 4)

Phase 3: ESC Key Fix - DEFERRED
└── (no tasks - bug not reproducible)

Phase 4: Auto-Config Creation
└── 06-auto-config-creation (Bug 6)
    └── depends on: config system understanding
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] Opening dialog second time shows devices instantly from cache
- [ ] Bootable devices populate on first dialog open
- [ ] "r" key refreshes both connected and bootable tabs
- [ ] No regression in device selection flow

### Phase 2 Complete When:
- [ ] Portrait layout shows section titles and borders
- [ ] Mode buttons show full labels ("Debug", "Profile", "Release") when width >= 45
- [ ] No visual regression in horizontal layout

### Phase 3: DEFERRED
- Bug not reproducible; ESC key works correctly
- Will revisit if issue resurfaces

### Phase 4 Complete When:
- [ ] Setting flavor without config creates new "Default" config
- [ ] Setting dart-defines without config creates new "Default" config
- [ ] Config is saved to `.fdemon/launch.toml`
- [ ] Next dialog open shows created config

---

## Milestone Deliverable

When all phases complete:
- **First-run experience** is smooth - devices (including bootable) appear immediately
- **Portrait layout** is visually consistent with horizontal layout
- **Modal interactions** work reliably with ESC key
- **User preferences** (flavor/dart-defines) are automatically persisted

The NewSessionDialog will be production-ready with no UX friction points.
