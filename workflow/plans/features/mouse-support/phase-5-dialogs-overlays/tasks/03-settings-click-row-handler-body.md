## Task: Settings Click-Row Handler Body & Double-Click Detection

**Objective**: Fill in `handler/settings_handlers::handle_settings_click_row` so single-click selects a row (sets `settings_view_state.selected_index = index`), and double-click on the same row within 400 ms emits a follow-up `Message::SettingsToggleEdit` via `UpdateResult::message`. Mirrors the Phase 4 `handle_click_log_row` chained-message pattern. Also resets `last_settings_click` whenever the active tab changes.

**Depends on**: 01 (the stub `handle_settings_click_row` and the `SettingsClickStamp` field must already exist)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/settings_handlers.rs`: Replace the stub body of `handle_settings_click_row` with the real implementation. Reset `last_settings_click` to `None` inside `handle_settings_goto_tab`, `handle_settings_next_tab`, `handle_settings_prev_tab` (the tab-change handlers).
- `crates/fdemon-app/src/state.rs`: If Task 01 placed the `last_settings_click` reset hooks elsewhere, ensure the field is reset on tab change either inline (preferred — see Notes) or via a helper method on `SettingsViewState`. *No new struct definitions in this task.*

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/log_view.rs::handle_click_log_row` (template — Phase 4 chained-message double-click logic).
- `crates/fdemon-app/src/state.rs::SettingsViewState` (`selected_index`, `editing`, `active_tab`).
- `crates/fdemon-app/src/handler/update.rs::UpdateResult` (for `UpdateResult::message(...)`).

### Details

#### Double-click window constant

Phase 4 used `400 ms` for log-view double-click detection. Use the same constant — extract it to a shared location so future surfaces (NewSessionDialog double-click on a device row, etc., should that be added later) can share. If Phase 4 already declared a constant in `handler/log_view.rs` or elsewhere, **reuse it** by re-exporting and importing; do NOT duplicate the literal.

If Phase 4 hard-coded `400` inline, this task introduces a new constant in a shared module:

```rust
// crates/fdemon-app/src/handler/mod.rs (or a new handler/click_window.rs):

/// Maximum elapsed time between two clicks for them to count as a double-click.
///
/// Shared by [`log_view::handle_click_log_row`] and
/// [`settings_handlers::handle_settings_click_row`] so the user-perceived
/// double-click feel is consistent across surfaces. 400 ms is the default
/// macOS double-click threshold and matches the upper bound of what most
/// users expect.
pub const DOUBLE_CLICK_WINDOW_MS: u64 = 400;
```

(Implementor's choice: if Phase 4 already extracted this, skip the new constant; if Phase 4 hard-coded `400` inline in `handler/log_view.rs`, take the small refactor of extracting it as part of this task.)

#### `handle_settings_click_row` body

```rust
/// Handle a single click on a settings panel row.
///
/// Sets `settings_view_state.selected_index = index` so the row appears
/// selected. If the same row was clicked within
/// [`DOUBLE_CLICK_WINDOW_MS`] ms, emits a follow-up [`Message::SettingsToggleEdit`]
/// via [`UpdateResult::message`] to enter edit mode (mirroring the Phase 4
/// log-view double-click pattern).
///
/// Single click never enters edit mode. Settings panel UX requires two clicks
/// to start editing — see Phase 5 PLAN.md notes for rationale.
///
/// # Edge cases
/// - If `index` is out of range for the active tab's item list, this clamps
///   `selected_index` to the last valid item (or 0 if empty). The widget
///   renderer (Task 10) only registers regions for visible rows, so an
///   out-of-range index from a click is unlikely; we clamp defensively.
/// - If `editing == true` (a previous click already entered edit mode), the
///   click is ignored — keyboard `Esc` must close the editor first. This
///   mirrors `handle_settings_next_item` / `handle_settings_prev_item` which
///   also no-op while editing.
pub fn handle_settings_click_row(state: &mut AppState, index: usize) -> UpdateResult {
    // Don't move selection while editing — user must close the editor first.
    if state.settings_view_state.editing {
        return UpdateResult::none();
    }

    let item_count = get_item_count_for_tab(state);
    let clamped = if item_count == 0 { 0 } else { index.min(item_count - 1) };

    // Read the previous click stamp (Copy, so no `take` needed).
    let prev = state.last_settings_click;
    let now = std::time::Instant::now();

    // Update selection.
    state.settings_view_state.selected_index = clamped;

    // Double-click detection: same row, within window.
    let is_double_click = match prev {
        Some(stamp) if stamp.index == clamped => {
            let elapsed_ms = now.duration_since(stamp.at).as_millis();
            elapsed_ms <= u128::from(DOUBLE_CLICK_WINDOW_MS)
        }
        _ => false,
    };

    if is_double_click {
        // Consume the stamp so a third click in the window doesn't trigger again.
        state.last_settings_click = None;
        // Emit the toggle-edit follow-up.
        UpdateResult::message(Message::SettingsToggleEdit)
    } else {
        // Record this click for potential future double-click pairing.
        state.last_settings_click = Some(SettingsClickStamp {
            index: clamped,
            at: now,
        });
        UpdateResult::none()
    }
}
```

#### Tab-change resets

In each of the existing tab-change handlers (`handle_settings_next_tab`, `handle_settings_prev_tab`, `handle_settings_goto_tab`), append a single line clearing `last_settings_click` so a click on row 5 of the Project tab and a click on row 5 of the User tab are not treated as a double-click pair:

```rust
pub fn handle_settings_next_tab(state: &mut AppState) -> UpdateResult {
    state.settings_view_state.next_tab();
    state.last_settings_click = None; // tab change invalidates the double-click pairing
    UpdateResult::none()
}

pub fn handle_settings_prev_tab(state: &mut AppState) -> UpdateResult {
    state.settings_view_state.prev_tab();
    state.last_settings_click = None;
    UpdateResult::none()
}

pub fn handle_settings_goto_tab(state: &mut AppState, idx: usize) -> UpdateResult {
    if let Some(tab) = SettingsTab::from_index(idx) {
        state.settings_view_state.goto_tab(tab);
        state.last_settings_click = None;
    }
    UpdateResult::none()
}
```

Also reset on `HideSettings` / `ForceHideSettings` for hygiene (a stale stamp doesn't hurt — `Instant`s elapse — but consistency with `last_log_click`'s reset-on-session-change is cleaner).

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes.
2. `cargo test --workspace` passes — the new tests below are added and pass.
3. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
4. `handle_settings_click_row` returns `UpdateResult::none()` on first click and on a click ≥ 400 ms after the previous one.
5. `handle_settings_click_row` returns `UpdateResult::message(Message::SettingsToggleEdit)` on the second click of the same `index` within 400 ms.
6. `handle_settings_click_row` is a no-op (no selection change, no follow-up) when `editing == true`.
7. Tab-change handlers (`handle_settings_next_tab`, `handle_settings_prev_tab`, `handle_settings_goto_tab`) reset `last_settings_click` to `None`.
8. `last_settings_click` is consumed (set to `None`) when a double-click fires, so a *third* click within the window does not trigger another `SettingsToggleEdit`.

### Testing

Add unit tests in `handler/settings_handlers.rs::tests` (or a new test module if none exists):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SettingsTab;
    use std::time::{Duration, Instant};

    fn fresh_state() -> AppState {
        AppState::new()
    }

    #[test]
    fn single_click_sets_selected_index_and_no_follow_up() {
        let mut s = fresh_state();
        s.show_settings();
        let result = handle_settings_click_row(&mut s, 3);
        assert_eq!(s.settings_view_state.selected_index, 3);
        assert!(result.message.is_none());
        assert!(s.last_settings_click.is_some());
    }

    #[test]
    fn second_click_same_row_within_window_emits_toggle_edit() {
        let mut s = fresh_state();
        s.show_settings();
        let _ = handle_settings_click_row(&mut s, 2);
        let result = handle_settings_click_row(&mut s, 2);
        assert!(matches!(
            result.message,
            Some(Message::SettingsToggleEdit)
        ));
        // Stamp consumed.
        assert!(s.last_settings_click.is_none());
    }

    #[test]
    fn second_click_different_row_does_not_emit_toggle_edit() {
        let mut s = fresh_state();
        s.show_settings();
        let _ = handle_settings_click_row(&mut s, 2);
        let result = handle_settings_click_row(&mut s, 5);
        assert!(result.message.is_none());
        assert_eq!(s.settings_view_state.selected_index, 5);
    }

    #[test]
    fn second_click_outside_window_does_not_emit_toggle_edit() {
        let mut s = fresh_state();
        s.show_settings();
        // Manually set a stale stamp.
        s.last_settings_click = Some(SettingsClickStamp {
            index: 2,
            at: Instant::now() - Duration::from_millis(500),
        });
        let result = handle_settings_click_row(&mut s, 2);
        assert!(result.message.is_none());
    }

    #[test]
    fn click_while_editing_is_no_op() {
        let mut s = fresh_state();
        s.show_settings();
        s.settings_view_state.selected_index = 1;
        s.settings_view_state.editing = true;
        let snapshot_before = s.settings_view_state.selected_index;
        let result = handle_settings_click_row(&mut s, 7);
        assert!(result.message.is_none());
        assert_eq!(s.settings_view_state.selected_index, snapshot_before);
    }

    #[test]
    fn tab_change_clears_click_stamp() {
        let mut s = fresh_state();
        s.show_settings();
        let _ = handle_settings_click_row(&mut s, 3);
        assert!(s.last_settings_click.is_some());
        let _ = handle_settings_goto_tab(&mut s, 1);
        assert!(s.last_settings_click.is_none());
    }

    #[test]
    fn third_click_within_window_does_not_double_fire() {
        let mut s = fresh_state();
        s.show_settings();
        let _ = handle_settings_click_row(&mut s, 2);
        let r2 = handle_settings_click_row(&mut s, 2);
        assert!(matches!(r2.message, Some(Message::SettingsToggleEdit)));
        // Third click within the same window should NOT re-fire toggle.
        let r3 = handle_settings_click_row(&mut s, 2);
        assert!(r3.message.is_none(), "third click must not re-toggle");
    }

    #[test]
    fn out_of_range_index_clamps_to_last_item() {
        let mut s = fresh_state();
        s.show_settings();
        let count = get_item_count_for_tab(&s);
        let too_far = count + 100;
        let _ = handle_settings_click_row(&mut s, too_far);
        assert_eq!(
            s.settings_view_state.selected_index,
            count.saturating_sub(1)
        );
    }
}
```

### Notes

- **Why `last_settings_click` and not `last_click: Option<Click>` with a discriminator.** Phase 4 used `last_log_click` and we mirror that. A unified `last_click` would force every surface to share a discriminator enum and prevent independent reset semantics (tab change resets settings; session change resets log). Per-surface stamps are cheaper and more localized.
- **Why double-click does not also send a `Message::SettingsClickRow` follow-up to re-set `selected_index`.** The first click already set `selected_index` to the same value, and the second click (this invocation) sets it again before the early-return. The follow-up `SettingsToggleEdit` operates on the now-current `selected_index`. No redundant message needed.
- **Why we don't reset `last_settings_click` on `HideSettings`.** `Instant`s naturally invalidate via the 400 ms window. Leaving the stamp set after closing settings is harmless. Listed in the acceptance criteria as "for hygiene" — implementer's call whether to add the line.
- **Why we early-return when `editing == true`.** Settings edit mode is text-input-driven; click-to-move-selection during editing would be hostile UX (user types `5`, suddenly the selection moves). The keyboard handlers already no-op on `j`/`k` when editing — we mirror that.
- **`UpdateResult::message(...)` semantics.** The follow-up message is queued; the engine processes it on the next `drain_pending_messages` iteration. The order is "this arm runs to completion → state mutation visible → follow-up `SettingsToggleEdit` arm runs → state mutation visible → render". This is the same flow Phase 4's `ClickLogRow` → `ToggleStackTraceForEntry` chain established.
- **Tests use `Instant::now()` directly.** Acceptable because the tests construct their own offsets via `Duration`. There is no fake-clock infrastructure in the project, and introducing one for these tests would be over-engineering. The slight non-determinism (a test could flake if scheduled with a 400 ms gap, which would require the test harness to pause for half a second between assertions) is mitigated by the test never sleeping — it always invokes the handler back-to-back.
