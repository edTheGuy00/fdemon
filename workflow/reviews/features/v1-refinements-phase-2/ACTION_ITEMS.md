# Action Items: v1-refinements Phase 2

**Review Date:** 2026-02-23
**Verdict:** NEEDS WORK
**Blocking Issues:** 2

## Critical Issues (Must Fix)

### 1. Dart defines modal: Esc persists instead of discarding
- **Source:** Logic Checker, Risks Analyzer, Code Quality
- **File:** `crates/fdemon-app/src/handler/settings_dart_defines.rs:40-58`
- **File:** `crates/fdemon-app/src/handler/keys.rs:652`
- **File:** `crates/fdemon-app/src/message.rs` (new variant needed)
- **Problem:** `SettingsDartDefinesClose` unconditionally saves to disk. Pressing Esc saves user changes instead of discarding them. Extra args modal correctly discards on Esc, creating an inconsistency.
- **Required Action:** Add `SettingsDartDefinesCancel` message variant. Create `handle_settings_dart_defines_cancel()` that clears modal state without persisting. Map Esc in the List pane to Cancel instead of Close. Keep Close as the "save and exit" path.
- **Acceptance:** Pressing Esc in dart defines modal discards changes. Both modals have consistent Esc = cancel behavior. Add test verifying Esc does not persist.

### 2. No mutual exclusion guard on modal open handlers
- **Source:** Logic Checker, Risks Analyzer, Architecture Enforcer
- **File:** `crates/fdemon-app/src/handler/settings_dart_defines.rs:20`
- **File:** `crates/fdemon-app/src/handler/settings_extra_args.rs:32`
- **Problem:** Neither `handle_settings_dart_defines_open` nor `handle_settings_extra_args_open` checks `has_modal_open()`. A programmatic dispatch could open both modals simultaneously, clobbering `editing_config_idx`.
- **Required Action:** Add `if state.settings_view_state.has_modal_open() { return UpdateResult::none(); }` at the top of both open handlers.
- **Acceptance:** Guard present in both handlers. Test verifying second open is no-op when first modal is active.

## Major Issues (Should Fix)

### 3. Magic string literals for field routing
- **Source:** Code Quality
- **File:** `crates/fdemon-app/src/handler/settings_handlers.rs:93,106`
- **File:** `crates/fdemon-app/src/handler/settings.rs` (apply_launch_config_change)
- **File:** `crates/fdemon-app/src/settings_items.rs:46`
- **Problem:** `"dart_defines"`, `"extra_args"`, `"launch.__add_new__"` scattered as string literals across 5+ files. A rename silently breaks routing.
- **Suggested Action:** Define constants in `settings_items.rs` and reference everywhere:
  ```rust
  pub const FIELD_DART_DEFINES: &str = "dart_defines";
  pub const FIELD_EXTRA_ARGS: &str = "extra_args";
  pub const SENTINEL_ADD_NEW: &str = "launch.__add_new__";
  ```

### 4. Silent data-loss path in dart defines close
- **Source:** Code Quality
- **File:** `crates/fdemon-app/src/handler/settings_dart_defines.rs:40-58`
- **Problem:** If `editing_config_idx` is `None` while `dart_defines_modal` is `Some`, modal is consumed without persisting.
- **Suggested Action:** Add `tracing::warn!` when config_idx is absent.

### 5. Inaccurate doc comment on `SettingsDartDefinesClose`
- **Source:** Code Quality
- **File:** `crates/fdemon-app/src/message.rs`
- **Problem:** Says "without saving changes" but handler saves.
- **Suggested Action:** Update doc comment to match actual behavior (update again after fix #1).

### 6. Extra args confirm closes silently when nothing selected
- **Source:** Code Quality
- **File:** `crates/fdemon-app/src/handler/settings_extra_args.rs:114-136`
- **Problem:** Enter with no selection closes modal silently.
- **Suggested Action:** Return early (keep modal open) when `selected_value()` is `None`.

## Minor Issues (Consider Fixing)

### 7. Shared `editing_config_idx` between two modals
- Split into `dart_defines_config_idx` / `extra_args_config_idx`, or add prominent `// SHARED` comment.
- File: `crates/fdemon-app/src/state.rs:512-517`

### 8. `hide_settings()` does not clear modal state
- Clear modal fields in `hide_settings()` or `handle_force_hide_settings`.
- File: `crates/fdemon-app/src/state.rs:868-870`

### 9. HashMap ordering causes defines to shuffle
- Sort alphabetically by key when loading into modal.
- File: `crates/fdemon-app/src/handler/settings_dart_defines.rs:23`

### 10. Add `+1` constant for add-new button count
- Replace magic `+ 1` with `const ADD_NEW_BUTTON_COUNT: usize = 1`.
- File: `crates/fdemon-app/src/handler/settings_handlers.rs:400-401`

### 11. Add doc comment to `PRESET_EXTRA_ARGS`
- File: `crates/fdemon-app/src/handler/settings_extra_args.rs:16-23`

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] Critical #1 resolved: Esc discards dart defines changes
- [ ] Critical #2 resolved: `has_modal_open()` guard + test
- [ ] Major #3 resolved: Magic strings replaced with constants
- [ ] Major #5 resolved: Doc comment accurate
- [ ] Verification: `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings`
