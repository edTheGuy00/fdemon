//! Drag-to-select-text state machine for the log view (`UiMode::Normal`).
//!
//! A plain (no-Shift) left-button press/drag/release over the log content drives
//! a character-precise text selection that auto-scrolls past the viewport edges
//! (see the `Tick` handler) and copies on release. Shift+drag is deliberately
//! NOT handled here so it can fall through to the terminal's native selection
//! when mouse capture is suspended (`Alt+m`) — see
//! `workflow/plans/bugs/log-text-selection-broken/BUG.md`.
//!
//! All fragile cell↔character mapping lives in the renderer, which publishes
//! `selection_rows` + edge metadata onto the per-session
//! [`crate::log_view_state::LogViewState`]. These handlers only read that map and
//! mutate the selection.

use crate::input_mouse::MouseButton;
use crate::log_view_state::LogSelection;
use crate::message::Message;
use crate::state::AppState;

/// Try to begin a drag-selection at a left press.
///
/// Returns `true` when the press landed on log content and started a selection
/// (the caller treats the press as consumed). Returns `false` when the press was
/// on a non-log clickable surface or empty space — in that case any existing
/// selection is cleared and the caller falls through to normal click handling.
pub(super) fn try_start_left_press(state: &mut AppState, x: u16, y: u16) -> bool {
    // Resolve which region (if any) is under the cursor. A log row registers a
    // `ClickLogRow` action; anything else (tabs, the jump-to-latest pill, link
    // badges) must keep its existing click behaviour, so we do NOT consume it.
    //
    // EXCEPTION (TEA): mouse_regions is a render-hint cell — same approved
    // exception as `handle_right_click`. The guard puts the registry back on drop.
    let regions = state.mouse_regions.take_guard();
    let hit = regions
        .hit_test(x, y, MouseButton::Left)
        .and_then(|e| e.on_left.as_ref())
        .map(|a| a.resolve(x, y));
    drop(regions);

    let on_log_row = matches!(hit, Some(Message::ClickLogRow { .. }));
    if !on_log_row {
        // Press elsewhere clears any prior selection; the caller handles the click.
        if let Some(h) = state.session_manager.selected_mut() {
            h.session.log_view_state.clear_selection();
        }
        return false;
    }

    if let Some(h) = state.session_manager.selected_mut() {
        let lvs = &mut h.session.log_view_state;
        if let Some(point) = lvs.locate_selection_point(x, y) {
            lvs.selection = Some(LogSelection::new(point));
            lvs.drag_autoscroll = None;
            return true;
        }
    }
    false
}

/// Extend the selection focus to the dragged cell, and arm edge auto-scroll when
/// the cursor leaves the content area's top/bottom.
///
/// Modifiers are intentionally ignored: once a selection has begun on a plain
/// press, holding Shift mid-drag still extends it.
pub(super) fn handle_drag(
    state: &mut AppState,
    x: u16,
    y: u16,
    button: MouseButton,
) -> Option<Message> {
    if button != MouseButton::Left {
        return None;
    }
    let h = state.session_manager.selected_mut()?;
    let lvs = &mut h.session.log_view_state;
    // Only act while a drag we started is in progress.
    if !lvs.selection.is_some_and(|s| s.dragging) {
        return None;
    }

    // Arm / disarm edge auto-scroll (carried out on `Tick`).
    lvs.drag_autoscroll = if y < lvs.content_top_y {
        Some(-1)
    } else if y >= lvs.content_bottom_y {
        Some(1)
    } else {
        None
    };

    // Inside the content area, snap focus to the precise cell. Beyond an edge,
    // leave focus to the Tick handler (which extends it to the edge line as the
    // view scrolls).
    if lvs.drag_autoscroll.is_none() {
        if let Some(point) = lvs.locate_selection_point(x, y) {
            if let Some(sel) = lvs.selection.as_mut() {
                sel.focus = point;
            }
        }
    }
    None
}

/// Finish a drag: copy a real selection, or emit the deferred click when the
/// press never moved (so double-click stack-trace toggling still works).
///
/// Focus is intentionally NOT re-snapped to the release cell: the last drag
/// event already positioned it, and that is exactly the selection the most
/// recent render measured into `selection_text` — keeping them identical means
/// the copied text always matches the on-screen highlight.
pub(super) fn handle_release(state: &mut AppState, button: MouseButton) -> Option<Message> {
    if button != MouseButton::Left {
        return None;
    }
    let h = state.session_manager.selected_mut()?;
    let lvs = &mut h.session.log_view_state;
    let sel = lvs.selection?;
    if !sel.dragging {
        return None;
    }

    lvs.drag_autoscroll = None;

    if sel.is_nonempty() {
        // Real selection → copy; keep the highlight (dragging ends).
        if let Some(s) = lvs.selection.as_mut() {
            s.dragging = false;
        }
        Some(Message::CopySelection)
    } else {
        // No movement → a plain click. Preserve the existing click behaviour
        // (double-click stack-trace toggle) and drop the empty selection.
        let click = Message::ClickLogRow {
            entry_id: sel.anchor.entry_id,
            frame_index: sel.anchor.frame_index,
        };
        lvs.clear_selection();
        Some(click)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_view_state::{LogViewState, SelPoint, SelectionRow};
    use crate::mouse_regions::{MouseAction, MouseRect};
    use crate::state::UiMode;

    fn test_device(id: &str) -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: id.to_string(),
            name: format!("dev-{id}"),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
            is_supported: true,
            capabilities: None,
        }
    }

    /// State with one (auto-selected) session in Normal mode.
    fn setup() -> AppState {
        let mut s = AppState::new();
        s.session_manager
            .create_session(&test_device("d1"))
            .unwrap();
        s.ui_mode = UiMode::Normal;
        s
    }

    fn nowrap_row(entry_id: u64, rect: MouseRect, text_len: usize) -> SelectionRow {
        SelectionRow {
            rect,
            entry_id,
            frame_index: None,
            base_col: 0,
            left_indicator: false,
            right_indicator: false,
            text_len,
            wrap_width: 0,
            top_clip: 0,
            text: String::new(),
        }
    }

    /// Publish one selection row + a matching `ClickLogRow` region so press/drag
    /// mapping has something to resolve. Sets content edges from the row's rect.
    fn install_row(state: &mut AppState, row: SelectionRow) {
        let (rect, entry_id, frame_index) = (row.rect, row.entry_id, row.frame_index);
        {
            let lvs = &mut state
                .session_manager
                .selected_mut()
                .unwrap()
                .session
                .log_view_state;
            lvs.content_top_y = rect.y;
            lvs.content_bottom_y = rect.y + 1;
            lvs.selection_rows = vec![row];
        }
        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            rect,
            MouseAction::emit(Message::ClickLogRow {
                entry_id,
                frame_index,
            }),
        );
        state.mouse_regions.set(regions);
    }

    fn selected_lvs(state: &AppState) -> &LogViewState {
        &state
            .session_manager
            .selected()
            .unwrap()
            .session
            .log_view_state
    }

    // ── try_start_left_press ───────────────────────────────────────────────

    #[test]
    fn try_start_begins_selection_on_log_row() {
        let mut s = setup();
        install_row(&mut s, nowrap_row(42, MouseRect::new(0, 0, 20, 1), 15));
        assert!(try_start_left_press(&mut s, 5, 0));
        let sel = selected_lvs(&s).selection.unwrap();
        assert!(sel.dragging);
        assert_eq!(sel.anchor, sel.focus);
        assert_eq!(
            sel.anchor,
            SelPoint {
                entry_id: 42,
                frame_index: None,
                col: 5
            }
        );
    }

    #[test]
    fn try_start_returns_false_off_log_row() {
        let mut s = setup();
        assert!(!try_start_left_press(&mut s, 5, 0));
        assert!(selected_lvs(&s).selection.is_none());
    }

    #[test]
    fn try_start_clears_existing_selection_when_off_row() {
        let mut s = setup();
        install_row(&mut s, nowrap_row(42, MouseRect::new(0, 0, 20, 1), 15));
        assert!(try_start_left_press(&mut s, 5, 0));

        // Remove the row + region, then press elsewhere.
        s.session_manager
            .selected_mut()
            .unwrap()
            .session
            .log_view_state
            .selection_rows
            .clear();
        let mut regions = s.mouse_regions.take();
        regions.clear();
        s.mouse_regions.set(regions);

        assert!(!try_start_left_press(&mut s, 100, 100));
        assert!(
            selected_lvs(&s).selection.is_none(),
            "prior selection cleared on off-row press"
        );
    }

    // ── handle_drag ────────────────────────────────────────────────────────

    #[test]
    fn drag_extends_focus_inside_content() {
        let mut s = setup();
        install_row(&mut s, nowrap_row(42, MouseRect::new(0, 0, 20, 1), 15));
        assert!(try_start_left_press(&mut s, 3, 0));
        assert!(handle_drag(&mut s, 9, 0, MouseButton::Left).is_none());
        let sel = selected_lvs(&s).selection.unwrap();
        assert_eq!(sel.anchor.col, 3);
        assert_eq!(sel.focus.col, 9);
        assert!(selected_lvs(&s).drag_autoscroll.is_none());
    }

    #[test]
    fn drag_below_bottom_arms_downward_autoscroll() {
        let mut s = setup();
        install_row(&mut s, nowrap_row(42, MouseRect::new(0, 0, 20, 1), 15));
        assert!(try_start_left_press(&mut s, 3, 0));
        handle_drag(&mut s, 5, 5, MouseButton::Left); // y=5 >= bottom (1)
        assert_eq!(selected_lvs(&s).drag_autoscroll, Some(1));
    }

    #[test]
    fn drag_above_top_arms_upward_autoscroll() {
        let mut s = setup();
        install_row(&mut s, nowrap_row(42, MouseRect::new(0, 5, 20, 1), 15));
        assert!(try_start_left_press(&mut s, 3, 5));
        handle_drag(&mut s, 5, 0, MouseButton::Left); // y=0 < top (5)
        assert_eq!(selected_lvs(&s).drag_autoscroll, Some(-1));
    }

    #[test]
    fn drag_without_selection_is_noop() {
        let mut s = setup();
        assert!(handle_drag(&mut s, 5, 0, MouseButton::Left).is_none());
        assert!(selected_lvs(&s).selection.is_none());
    }

    // ── handle_release ─────────────────────────────────────────────────────

    #[test]
    fn release_with_movement_emits_copy_and_keeps_selection() {
        let mut s = setup();
        install_row(&mut s, nowrap_row(42, MouseRect::new(0, 0, 20, 1), 15));
        assert!(try_start_left_press(&mut s, 3, 0));
        handle_drag(&mut s, 9, 0, MouseButton::Left);
        let msg = handle_release(&mut s, MouseButton::Left);
        assert!(matches!(msg, Some(Message::CopySelection)));
        let sel = selected_lvs(&s).selection.unwrap();
        assert!(!sel.dragging, "drag ends but the highlight persists");
        assert!(sel.is_nonempty());
    }

    #[test]
    fn release_without_movement_emits_click_and_clears() {
        let mut s = setup();
        install_row(&mut s, nowrap_row(42, MouseRect::new(0, 0, 20, 1), 15));
        assert!(try_start_left_press(&mut s, 3, 0));
        let msg = handle_release(&mut s, MouseButton::Left);
        assert!(matches!(
            msg,
            Some(Message::ClickLogRow {
                entry_id: 42,
                frame_index: None
            })
        ));
        assert!(
            selected_lvs(&s).selection.is_none(),
            "empty selection is cleared on click"
        );
    }

    #[test]
    fn release_non_left_button_is_noop() {
        let mut s = setup();
        install_row(&mut s, nowrap_row(42, MouseRect::new(0, 0, 20, 1), 15));
        assert!(try_start_left_press(&mut s, 3, 0));
        assert!(handle_release(&mut s, MouseButton::Right).is_none());
    }
}
