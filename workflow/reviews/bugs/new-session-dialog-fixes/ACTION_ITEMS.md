# Action Items: New Session Dialog Fixes

**Review Date:** 2026-01-23
**Verdict:** NEEDS WORK
**Blocking Issues:** 3 Critical + 5 Major = 8 issues

---

## Critical Issues (Must Fix)

### 1. Duplicate Cache Checking Logic

- **Source:** Logic & Reasoning Checker
- **File:** `src/app/state.rs` AND `src/app/handler/new_session/navigation.rs`
- **Lines:** 419-433 (state.rs), 163-196 (navigation.rs)
- **Problem:** Cache is checked and devices populated in BOTH `show_new_session_dialog()` AND `handle_open_new_session_dialog()`, causing redundant calls.
- **Required Action:** Remove cache checking from `show_new_session_dialog()`:
  ```rust
  // Remove these lines from show_new_session_dialog():
  if let Some(cached_devices) = self.get_cached_devices() {
      self.new_session_dialog_state
          .target_selector
          .set_connected_devices(cached_devices.clone());
  }

  if let Some((simulators, avds)) = self.get_cached_bootable_devices() {
      self.new_session_dialog_state
          .target_selector
          .set_bootable_devices(simulators, avds);
  }
  ```
  Keep the cache checking in `handle_open_new_session_dialog()` which also handles background refresh.
- **Acceptance:** Only one location checks and populates cache; no redundant calls.

---

### 2. Auto-Config Creation Bypasses Editability Validation

- **Source:** Logic & Reasoning Checker
- **File:** `src/app/handler/new_session/launch_context.rs`
- **Lines:** 185-199 (flavor), 285-303 (dart-defines)
- **Problem:** Direct config mutation (`config.config.flavor = flavor`) bypasses editability checks in `set_flavor()`.
- **Required Action:** Remove direct config mutation blocks:
  ```rust
  // DELETE these lines (185-199 for flavor, 285-303 for dart-defines):
  if let Some(config_idx) = state
      .new_session_dialog_state
      .launch_context
      .selected_config_index
  {
      if let Some(config) = state
          .new_session_dialog_state
          .launch_context
          .configs
          .configs
          .get_mut(config_idx)
      {
          config.config.flavor = flavor;  // or config.config.dart_defines = ...
      }
  }
  ```
  Config persistence should happen through the auto-save mechanism only.
- **Acceptance:** No direct config field mutation in handlers; all changes go through state methods.

---

### 3. Vertical Space Budget Not Validated

- **Source:** Risks & Tradeoffs Analyzer
- **Files:** `src/tui/widgets/new_session_dialog/target_selector.rs`, `launch_context.rs`
- **Problem:** Adding 4 lines of borders with `MIN_VERTICAL_HEIGHT: 20` was not tested for content overflow.
- **Required Action:**
  1. Run application with terminal height set to exactly 20 lines
  2. Verify all content is visible (tab bar, device list, config fields, launch button)
  3. Document the vertical space breakdown:
     ```
     Target Selector border: 2 lines
     Target Selector content: X lines
     Launch Context border: 2 lines
     Launch Context content: Y lines
     Total: 20 lines
     ```
  4. If content overflows, either increase `MIN_VERTICAL_HEIGHT` or reduce border usage
- **Acceptance:** Compact mode is usable at minimum terminal height; all elements visible.

---

## Major Issues (Should Fix)

### 1. Unwrap Calls in Handler Logging

- **Source:** Code Quality Inspector, Risks & Tradeoffs Analyzer
- **File:** `src/app/handler/new_session/launch_context.rs`
- **Lines:** 170, 276
- **Problem:** `.unwrap()` on `selected_config()` violates no-panic standard.
- **Suggested Action:** Replace with safe pattern:
  ```rust
  // Instead of:
  tracing::info!(
      "Auto-created config '{}' for flavor selection",
      state.new_session_dialog_state.launch_context.selected_config().unwrap().config.name
  );

  // Use:
  if let Some(config) = state.new_session_dialog_state.launch_context.selected_config() {
      tracing::info!("Auto-created config '{}' for flavor selection", config.config.name);
  }
  ```

---

### 2. Error Not Cleared in `set_bootable_devices()`

- **Source:** Logic & Reasoning Checker
- **File:** `src/tui/widgets/new_session_dialog/target_selector.rs`
- **Line:** ~228 (inside `set_bootable_devices()`)
- **Problem:** Error message persists after successful bootable discovery.
- **Suggested Action:** Add error clearing:
  ```rust
  pub fn set_bootable_devices(&mut self, ios_simulators: Vec<IosSimulator>, android_avds: Vec<AndroidAvd>) {
      self.ios_simulators = ios_simulators;
      self.android_avds = android_avds;
      self.bootable_loading = false;
      self.error = None;  // ADD THIS LINE
      // ...
  }
  ```

---

### 3. Width Threshold May Need Adjustment

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `src/tui/widgets/new_session_dialog/launch_context.rs`
- **Line:** 824 (`MODE_FULL_LABEL_MIN_WIDTH`)
- **Problem:** Threshold of 48 was set before borders added 2 columns overhead.
- **Suggested Action:**
  1. Test at terminal widths 48, 49, 50 columns with compact mode
  2. If mode labels overflow/wrap, increase threshold to 50:
     ```rust
     const MODE_FULL_LABEL_MIN_WIDTH: u16 = 50;  // Was 48
     ```

---

### 4. Tool Availability Timeout Not Verified

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `src/app/handler/update.rs` (and spawn layer)
- **Lines:** 1031-1051
- **Problem:** If tool check hangs, bootable tab shows loading forever.
- **Suggested Action:**
  1. Verify spawn layer has timeout for tool availability check
  2. If not, add timeout handling in handler:
     ```rust
     // After 5 seconds, assume no tools available
     state.new_session_dialog_state.target_selector.bootable_loading = false;
     ```
  3. Or add message: `Message::ToolAvailabilityTimeout`

---

### 5. Unbounded Loop in Unique Name Generation

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `src/app/new_session_dialog/state.rs`
- **Lines:** 615-628
- **Problem:** Unbounded loop could freeze UI with many configs.
- **Suggested Action:** Add counter limit:
  ```rust
  fn generate_unique_name(base_name: &str, existing_names: &[&str]) -> String {
      if !existing_names.contains(&base_name) {
          return base_name.to_string();
      }

      for counter in 2..=1000 {  // ADD LIMIT
          let candidate = format!("{} {}", base_name, counter);
          if !existing_names.contains(&candidate.as_str()) {
              return candidate;
          }
      }

      // Fallback to timestamp
      format!("{} {}", base_name, std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .map(|d| d.as_secs())
          .unwrap_or(0))
  }
  ```

---

## Minor Issues (Consider Fixing)

### 1. Cache Cloning Performance

- **Problem:** Device lists are cloned each time dialog opens.
- **Suggestion:** Consider `Arc<Vec<Device>>` for cheaper cloning if device lists become large.

### 2. Hardcoded Cache TTL

- **Problem:** 30-second TTL is hardcoded in multiple places.
- **Suggestion:** Extract to module-level constant or make configurable in settings.

### 3. Nested If-Let Chains

- **Problem:** Deep nesting in auto-config handlers reduces readability.
- **Suggestion:** Use Rust 2024 let-else pattern when available:
  ```rust
  let Some(config_idx) = state.new_session_dialog_state.launch_context.selected_config_index else {
      return UpdateResult::none();
  };
  ```

### 4. Missing Cache TTL Documentation

- **Problem:** Task 03 mentioned 5s TTL but implementation uses 30s.
- **Suggestion:** Add comment explaining the rationale for 30s.

### 5. DartDefine Empty Value Handling

- **Problem:** Empty values stored as empty strings without validation.
- **Suggestion:** Document or validate against Flutter CLI expected format.

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Critical Issue 1: Duplicate cache checking removed
- [ ] Critical Issue 2: Direct config mutation removed
- [ ] Critical Issue 3: Vertical space validated at 20 lines
- [ ] Major Issue 1: Unwrap calls replaced
- [ ] Major Issue 2: Error clearing added to `set_bootable_devices()`
- [ ] Major Issue 3: Width threshold tested with borders
- [ ] Major Issue 4: Tool timeout verified or added
- [ ] Major Issue 5: Loop bounded in unique name generation
- [ ] `cargo fmt` passes
- [ ] `cargo check` passes
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Manual testing at minimum terminal dimensions (80x20)

---

## Priority Order

1. **High Priority (Before Merge):**
   - Critical Issues 1, 2, 3
   - Major Issues 1, 2

2. **Medium Priority (Soon After Merge):**
   - Major Issues 3, 4, 5

3. **Low Priority (When Convenient):**
   - Minor Issues 1-5
