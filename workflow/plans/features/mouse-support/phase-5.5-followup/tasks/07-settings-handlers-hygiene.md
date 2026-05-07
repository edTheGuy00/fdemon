# Task 07: Settings Handlers Hygiene (Save Helper, Test Renames, Stub Cleanup)

## Goal

Three small hygiene fixes inside `crates/fdemon-app/src/handler/settings_handlers.rs`:

1. **Minor #10:** Extract the duplicated save-active-tab logic into a private helper shared by `handle_settings_save` and `handle_settings_save_and_close`.
2. **Minor #11:** Rename Phase-5 double-click tests to follow the project naming convention `test_<function>_<scenario>_<expected_result>`.
3. **Minor #17:** Stop calling `mark_dirty()` in the cycle/increment stub functions until they actually mutate values (or implement them).

## Background

**Save-logic duplication.** `handle_settings_save` (~lines 158-198) and `handle_settings_save_and_close` (~lines 346-381) contain identical `match state.settings_view_state.active_tab` blocks loading and saving the active tab. The only difference is that `_and_close` calls `state.hide_settings()` on success. Phase 5 added `_and_close` without factoring out the common logic.

**Test naming.** Several tests in the `handle_settings_click_row` suite use names like `single_click_sets_selected_index_and_no_follow_up` (no `test_` prefix, no function-name segment). `docs/CODE_STANDARDS.md` requires `test_<function>_<scenario>_<expected_result>`.

**Stub functions marking dirty.** `handle_settings_cycle_enum_next`, `handle_settings_cycle_enum_prev`, and `handle_settings_increment` (~lines 269-291) all call `state.settings_view_state.mark_dirty()` despite being no-op stubs. A user clicking a cycle button sees the form go dirty without anything changing.

## Files

**Modify:**
- `crates/fdemon-app/src/handler/settings_handlers.rs`

**Read (reference):**
- `docs/CODE_STANDARDS.md` — test naming convention

## Plan

1. **Extract `save_active_tab(state: &mut AppState) -> crate::error::Result<()>`** as a private helper:
   ```rust
   fn save_active_tab(state: &mut AppState) -> crate::error::Result<()> {
       match state.settings_view_state.active_tab {
           SettingsTab::Project => save_project_settings(state)?,
           SettingsTab::UserPrefs => save_user_settings(state)?,
           SettingsTab::LaunchConfig => save_launch_config(state)?,
           SettingsTab::Vscode => save_vscode_config(state)?,
       }
       state.settings_view_state.mark_clean();
       Ok(())
   }
   ```
   (Adapt to the actual tab enum and per-tab save fn names.)

   Update `handle_settings_save` to call `save_active_tab(state)` and propagate the result. Update `handle_settings_save_and_close` to call `save_active_tab(state)` and, on `Ok`, also call `state.hide_settings()`.

2. **Rename Phase-5 double-click tests** to follow the `test_<function>_<scenario>_<expected_result>` convention. Identify the affected tests by searching the file:
   - `single_click_sets_selected_index_and_no_follow_up` → `test_handle_settings_click_row_single_click_sets_index_no_follow_up`
   - `second_click_same_row_within_window_emits_toggle_edit` → `test_handle_settings_click_row_double_click_same_row_emits_toggle_edit`
   - `second_click_outside_window_does_not_emit_toggle_edit` → `test_handle_settings_click_row_second_click_after_window_no_toggle`
   - `third_click_within_window_does_not_double_fire` → `test_handle_settings_click_row_third_click_in_window_no_retrigger`
   - `tab_change_clears_click_stamp` → `test_handle_settings_goto_tab_clears_click_stamp`
   - `tab_change_next_clears_click_stamp` → `test_handle_settings_next_tab_clears_click_stamp`
   - `tab_change_prev_clears_click_stamp` → `test_handle_settings_prev_tab_clears_click_stamp`
   - `click_while_editing_is_no_op` → `test_handle_settings_click_row_while_editing_is_no_op`
   - `click_row_with_no_session_is_no_op` → `test_handle_settings_click_row_with_no_session_is_no_op`
   - `out_of_range_index_clamps_to_last_item` → `test_handle_settings_click_row_out_of_range_index_clamps_to_last`

   Verify by re-reading the file and matching all functions in the test module that don't start with `test_`. Be exhaustive — any non-conforming name in this file's tests should be fixed in this task.

3. **Remove `mark_dirty()` from stub functions**:
   - `handle_settings_cycle_enum_next` — body currently calls `mark_dirty()` and returns `UpdateResult::none()`. Remove the `mark_dirty()` call. Add a `// stub — no-op until field-by-field cycle logic is implemented` comment.
   - `handle_settings_cycle_enum_prev` — same.
   - `handle_settings_increment` — same.

   Document the rationale: marking dirty without changing values misleads the user and trips the unsaved-changes confirm-dialog warning. The stub should be a true no-op until implemented.

4. **Quality gates**:
   ```bash
   cargo test -p fdemon-app handler::settings_handlers
   cargo test --workspace
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

## Acceptance Criteria

- [ ] Private `save_active_tab(state)` helper exists and is called by both `handle_settings_save` and `handle_settings_save_and_close`.
- [ ] No duplicated `match active_tab { ... }` save block remains in either public handler.
- [ ] All test functions in this file follow `test_<function>_<scenario>_<expected_result>`.
- [ ] `handle_settings_cycle_enum_next/_prev` and `handle_settings_increment` no longer call `mark_dirty()`.
- [ ] All existing tests still pass (renames are signature-only; logic unchanged).
- [ ] Quality gates pass.

## Notes

- **No new tests are required** for this task — the renames preserve test bodies, the helper extraction is verified by existing save tests, and the stub `mark_dirty()` removal is verified by an existing test that asserts unsaved-state after cycle (if such a test exists; if it does, update it to assert clean-state).
- T01 does not modify `handler/settings_handlers.rs`. T07 owns this file exclusively in 5.5.
- T07 ↔ T08: T08 modifies `handler/new_session/{clicks,mod}.rs`. No overlap with `handler/settings_handlers.rs`.
- If the cycle/increment stubs have ever been wired up in any caller (search for `cycle_enum_next` / `increment`), removing `mark_dirty()` may regress that caller's UX. Audit before removing.
