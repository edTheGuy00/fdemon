## Task: Fix Multi-Session Header to Show Tabs and Device Info

**Objective**: Make session tabs visible when 2+ sessions are active. Currently the header is fixed at 3 rows (1 inner row after borders), but tabs require 2 inner rows. Users cannot see which session is active or that 1/2/3 keys switch sessions.

**Depends on**: None

**Review Reference**: REVIEW.md #1 (Critical), ACTION_ITEMS.md #1

### Scope

- `crates/fdemon-tui/src/layout.rs:63-79`: Make `create_with_sessions` use the `session_count` parameter to return `Length(5)` when `session_count > 1` (giving 3 inner rows: title + tabs + breathing room), keep `Length(3)` for single session.
- `crates/fdemon-tui/src/widgets/header.rs:59-89`: The `MainHeader` widget already has correct multi-session rendering logic (`if inner.height >= 2` branch with title row + tabs row). It just needs the layout to provide enough height.

### Details

**Root cause**: `create_with_sessions` at `layout.rs:64` discards `session_count` with `let _ = session_count` and always returns `Length(3)` for the header. A 3-row block with `Borders::ALL` has only 1 inner row. The `MainHeader` widget's multi-session branch requires `inner.height >= 2` to render tabs, so it always falls through to the single-row fallback.

**Fix approach**:

1. In `layout.rs:create_with_sessions`, replace the `let _ = session_count` discard with actual logic:
   - When `session_count <= 1`: use `Length(3)` (current behavior, 1 inner row for title)
   - When `session_count > 1`: use `Length(5)` (3 inner rows: title row + tabs row + 1 row breathing room)

2. Verify the `MainHeader` widget's existing multi-session logic works correctly with the new height. The code at `header.rs:59-89` already handles the split into title area and tabs area when `inner.height >= 2`.

3. Confirm that the device pill (currently hidden in multi-session mode) is visible. The `render_title_row` is called with `show_extras: false` in multi-session mode (line 70). Consider whether device info should be shown in the tabs row or title row for multi-session mode. At minimum, the session tabs should show the device name per tab (which `SessionTabs` already does via the session's `device_name` field).

### Acceptance Criteria

1. With 2+ sessions, session tabs are visible below the header title
2. Active session is visually highlighted in the tab bar
3. Session switching hints (1/2/3 keys) are discoverable
4. Single-session mode is unchanged (3-row header, no tabs)
5. Layout proportions remain reasonable (log area still has adequate space)
6. `cargo check -p fdemon-tui` passes

### Testing

- Existing snapshot tests will need updating (header height changes)
- Add a unit test that verifies `create_with_sessions` returns different heights for 1 vs 2+ sessions
- Verify `MainHeader` renders tabs when inner height is 2+

### Notes

- The `MainHeader` widget code at `header.rs:59-89` is already correctly structured for multi-session rendering — it just never gets enough height to execute that branch. This fix is primarily a layout change.
- The gap row between header and logs (`Length(1)`) should be preserved regardless of session count.
- Consider edge case: terminal is very short and can't fit expanded header + gap + logs. The `Min(3)` constraint on logs should handle this gracefully.
