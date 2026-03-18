## Task: Add Deletion Confirmation + Minor Code Quality Fixes

**Objective**: Add a confirmation step before deleting SDK versions, and address the remaining minor code quality issues from the review.

**Depends on**: None

**Severity**: MAJOR (confirmation) + MINOR (code quality)

### Scope

#### Major: Deletion Confirmation
- `crates/fdemon-app/src/flutter_version/state.rs`: Add `pending_delete: Option<usize>` field to `FlutterVersionState`
- `crates/fdemon-app/src/handler/flutter_version/actions.rs`: Implement double-press `d` confirmation pattern
- `crates/fdemon-app/src/message.rs`: (if needed) Add `FlutterVersionConfirmDelete` message variant

#### Minor Fixes (bundled — overlap with same files)
- `crates/fdemon-tui/src/widgets/flutter_version_panel/sdk_info.rs`: Remove `#[allow(dead_code)]` on `icons` field
- `crates/fdemon-tui/src/widgets/flutter_version_panel/version_list.rs`: Remove `#[allow(dead_code)]` on `icons` field
- `crates/fdemon-tui/src/widgets/flutter_version_panel/mod.rs`: Replace private `centered_rect` with shared `modal_overlay::centered_rect_percent`
- `crates/fdemon-app/src/handler/update.rs`: Move `FlutterVersionInstall`/`FlutterVersionUpdate` stubs into handler module

### Details

#### Part A: Deletion Confirmation (Major)

**Problem:** Pressing `d` immediately triggers `remove_dir_all` on a 2-3 GB SDK directory. The `d` key is adjacent to navigation keys (`j`/`k`), making accidental deletion likely.

**Design: Double-press pattern** (simpler than a dialog, consistent with TUI conventions like Vim's `dd`):

1. First `d` press: Set `pending_delete = Some(selected_index)`, show status message "Press d again to remove {version}"
2. Second `d` press (same index): Execute the removal, clear `pending_delete`
3. Any other key: Clear `pending_delete` (cancel the pending delete)
4. Navigation that changes `selected_index`: Clear `pending_delete`

**Implementation:**

Add to `FlutterVersionState`:
```rust
pub struct FlutterVersionState {
    // ... existing fields ...
    /// Index of version pending deletion (double-press confirmation).
    /// Set on first `d` press, cleared on second `d` or any other action.
    pub pending_delete: Option<usize>,
}
```

Modify `handle_remove` in `actions.rs`:
```rust
pub fn handle_remove(state: &mut AppState) -> UpdateResult {
    let version_list = &state.flutter_version_state.version_list;
    if version_list.installed_versions.is_empty() || version_list.loading {
        return UpdateResult::none();
    }

    let selected = version_list.selected_index;
    let sdk = &version_list.installed_versions[selected];

    if sdk.is_active {
        state.flutter_version_state.status_message =
            Some("Cannot remove the active SDK version".into());
        state.flutter_version_state.pending_delete = None;
        return UpdateResult::none();
    }

    // Double-press confirmation
    if state.flutter_version_state.pending_delete == Some(selected) {
        // Second press — confirmed, proceed with removal
        state.flutter_version_state.pending_delete = None;
        let path = sdk.path.clone();
        let version = sdk.version.clone();
        UpdateResult::action(UpdateAction::RemoveFlutterVersion { path, version })
    } else {
        // First press — set pending and show confirmation prompt
        state.flutter_version_state.pending_delete = Some(selected);
        state.flutter_version_state.status_message =
            Some(format!("Press d again to remove {}", sdk.version));
        UpdateResult::none()
    }
}
```

Clear `pending_delete` on navigation actions (in `navigation.rs` handlers for `j`/`k`/`Up`/`Down`):
```rust
state.flutter_version_state.pending_delete = None;
```

#### Part B: Remove `#[allow(dead_code)]` on `icons` Fields (Minor)

**Files:** `sdk_info.rs:40-43` and `version_list.rs:41-43`

Remove the `icons` field entirely from both widget structs since it is unused and Phase 3 can add it back when actually needed. Also remove the corresponding parameter from `new()` constructors and update `FlutterVersionPanel` where it passes `self.icons` to these sub-widgets.

If removing the field would be too disruptive (too many call-site changes), the alternative is to actually use the `icons` field — e.g., use `icons.active_marker` for the active version indicator character in `version_list.rs`. Choose the approach that results in cleaner code.

#### Part C: Replace Private `centered_rect` with Shared Utility (Minor)

**File:** `crates/fdemon-tui/src/widgets/flutter_version_panel/mod.rs`, lines 91-105

A shared utility already exists at `crates/fdemon-tui/src/widgets/modal_overlay.rs`:
```rust
pub fn centered_rect_percent(width_pct: u16, height_pct: u16, area: Rect) -> Rect
```

Replace:
```rust
// Before — private method with magic numbers
fn centered_rect(area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage(15), Constraint::Percentage(70), Constraint::Percentage(15),
    ]).split(area);
    Layout::horizontal([
        Constraint::Percentage(10), Constraint::Percentage(80), Constraint::Percentage(10),
    ]).split(vertical[1])[1]
}

// After — shared utility
use super::modal_overlay::centered_rect_percent;
// In render: centered_rect_percent(80, 70, area)
```

If the percentages differ from what `centered_rect_percent` provides, define named constants:
```rust
/// Panel width as percentage of terminal width.
const PANEL_WIDTH_PERCENT: u16 = 80;
/// Panel height as percentage of terminal height.
const PANEL_HEIGHT_PERCENT: u16 = 70;
```

#### Part D: Route Stub Handlers Through Handler Module (Minor)

**File:** `crates/fdemon-app/src/handler/update.rs`, lines 2496-2506

Move the `FlutterVersionInstall` and `FlutterVersionUpdate` stub handlers from inline in `update.rs` into `handler/flutter_version/actions.rs` (or a new `stubs.rs` if cleaner):

```rust
// In handler/flutter_version/actions.rs (or stubs.rs)
pub fn handle_install(state: &mut AppState) -> UpdateResult {
    state.flutter_version_state.status_message = Some("Install not yet available".into());
    UpdateResult::none()
}

pub fn handle_update(state: &mut AppState) -> UpdateResult {
    state.flutter_version_state.status_message = Some("Update not yet available".into());
    UpdateResult::none()
}
```

Then in `update.rs`:
```rust
Message::FlutterVersionInstall => flutter_version::actions::handle_install(state),
Message::FlutterVersionUpdate => flutter_version::actions::handle_update(state),
```

### Acceptance Criteria

1. **Confirmation**: First `d` press shows "Press d again to remove {version}"; second `d` executes removal
2. **Cancel**: Navigating away or pressing any non-`d` key clears the pending delete
3. **Active guard**: Attempting to delete the active version still shows the existing error message and clears any pending state
4. **`icons` field**: Either removed from both sub-widgets, or actually used for rendering
5. **`centered_rect`**: Uses the shared `modal_overlay::centered_rect_percent` utility (or documents why it can't)
6. **Stub routing**: `FlutterVersionInstall` and `FlutterVersionUpdate` handlers live in the handler module, not inline in `update.rs`
7. `cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

### Testing

```rust
#[test]
fn test_delete_requires_double_press() {
    let mut state = make_state_with_versions();
    state.flutter_version_state.version_list.selected_index = 1; // non-active version

    // First press — sets pending
    let result = handle_remove(&mut state);
    assert!(result.action.is_none());
    assert_eq!(state.flutter_version_state.pending_delete, Some(1));
    assert!(state.flutter_version_state.status_message.unwrap().contains("again"));

    // Second press — confirms
    let result = handle_remove(&mut state);
    assert!(result.action.is_some()); // RemoveFlutterVersion action
    assert!(state.flutter_version_state.pending_delete.is_none());
}

#[test]
fn test_delete_cancelled_by_navigation() {
    let mut state = make_state_with_versions();
    state.flutter_version_state.version_list.selected_index = 1;

    // First d press
    handle_remove(&mut state);
    assert_eq!(state.flutter_version_state.pending_delete, Some(1));

    // Navigate — clears pending
    handle_navigate_down(&mut state);
    assert!(state.flutter_version_state.pending_delete.is_none());
}

#[test]
fn test_delete_active_version_clears_pending() {
    let mut state = make_state_with_versions();
    state.flutter_version_state.version_list.selected_index = 0; // active version

    handle_remove(&mut state);
    assert!(state.flutter_version_state.pending_delete.is_none());
    assert!(state.flutter_version_state.status_message.unwrap().contains("active"));
}
```

### Notes

- The double-press pattern is preferred over `ConfirmDialog` because it's faster (no dialog rendering/dismissal), more consistent with TUI conventions, and doesn't require the complex dialog infrastructure for a single-action confirmation.
- The minor fixes (Parts B-D) are bundled here because they're small and several touch the same widget files.
- Part D may require updating the `flutter_version` handler module's `mod.rs` to expose the new functions.
