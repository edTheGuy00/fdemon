## Task: Error UX Improvements

**Objective**: Replace raw error strings with user-friendly messages in DevTools panels, add fallback UI when specific features are unavailable (e.g., profile/release mode), and provide actionable guidance in error states.

**Depends on**: 01-expand-devtools-config, 02-connection-state-ui

**Estimated Time**: 3-5 hours

### Scope

- `crates/fdemon-app/src/handler/devtools.rs`: Map raw RPC errors to user-friendly error messages
- `crates/fdemon-app/src/state.rs`: Add error state fields to `InspectorState` and `LayoutExplorerState` if not present
- `crates/fdemon-tui/src/widgets/devtools/inspector.rs`: Render error states with actionable hints
- `crates/fdemon-tui/src/widgets/devtools/layout_explorer.rs`: Render error states with hints
- `crates/fdemon-tui/src/widgets/devtools/performance.rs`: Improve disconnected/unavailable messaging

### Details

#### 1. Error Classification

Categorize errors that can occur in DevTools into user-friendly groups:

| Raw Error | User-Friendly Message | Action Hint |
|-----------|----------------------|-------------|
| Extension not registered | "Widget inspector not available in this mode" | "Try running in debug mode" |
| Isolate not found | "Flutter app isolate not found" | "The app may have restarted. Press [r] to retry" |
| RPC timeout (from Task 02) | "Request timed out" | "Press [r] to retry" |
| Connection lost | "VM Service connection lost" | "Reconnecting automatically..." |
| No VM URI available | "VM Service not available" | "Ensure the app is running in debug mode" |
| Object group expired | "Widget data expired" | "Press [r] to refresh" |
| Parse error (malformed response) | "Unexpected response from Flutter" | "Press [r] to retry, or press [b] for browser DevTools" |

#### 2. Error State in Inspector/Layout State

Ensure `InspectorState` and `LayoutExplorerState` have an error field:

```rust
pub struct InspectorState {
    pub root: Option<DiagnosticsNode>,
    pub loading: bool,
    pub error: Option<DevToolsError>,  // NEW or verify exists
    // ... other fields
}

pub struct DevToolsError {
    pub message: String,
    pub hint: String,
}
```

#### 3. Error Rendering in Panels

When an error is present, render a centered error box instead of the normal panel content:

```
┌────────────────────────────────────────────────┐
│                                                │
│     ⚠ Widget inspector not available           │
│                                                │
│     The widget inspector requires debug mode.  │
│     Try running your app with `--debug` flag.  │
│                                                │
│     [r] Retry   [b] Browser DevTools           │
│     [Esc] Return to logs                       │
│                                                │
└────────────────────────────────────────────────┘
```

Use `Color::Yellow` for the warning icon and title, `Color::DarkGray` for the description, and the standard key hint styling for action hints.

#### 4. Profile/Release Mode Detection

When the Flutter app is running in profile or release mode, many service extensions are unavailable. Detect this and show appropriate messaging:

- The VM connection might succeed but `ext.flutter.inspector.*` calls fail with "extension not registered"
- Show: "DevTools features are limited in profile/release mode. Full functionality requires debug mode."
- Still allow: Performance panel (frame timing works in profile mode), browser DevTools launch

#### 5. Graceful Degradation Per Panel

- **Inspector**: If tree fetch fails, show error + retry hint. Don't show an empty tree with no explanation.
- **Layout**: If layout fetch fails, show error. If no widget is selected in inspector, show "Select a widget in the Inspector panel first" instead of empty state.
- **Performance**: If monitoring fails to start, show why (no VM, not debug mode, etc.). The current "VM Service not connected" text is functional but could be more helpful.

#### 6. Error Clearing

Errors should be cleared when:
- User presses `r` (refresh/retry) — clear error, set loading = true, re-fetch
- User switches sessions — clear error (new session, new state)
- VM reconnects — clear errors (connection restored)
- User switches panels — don't clear (preserve panel-specific errors)

### Acceptance Criteria

1. Raw RPC error strings never appear in the TUI
2. Each error type maps to a specific user-friendly message with an action hint
3. Inspector panel shows a centered error box when tree fetch fails
4. Layout panel shows "Select a widget first" when no inspector node is selected
5. Layout panel shows an error box when layout fetch fails
6. Profile/release mode shows a meaningful message about limited functionality
7. Error states include actionable key hints ([r] Retry, [b] Browser, [Esc] Return)
8. Pressing `r` clears the error and retries the operation
9. VM reconnection clears errors

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_error_maps_to_user_friendly_message() {
        let error = map_rpc_error("Method not found: ext.flutter.inspector.getRootWidgetTree");
        assert_eq!(error.message, "Widget inspector not available in this mode");
        assert!(error.hint.contains("debug mode"));
    }

    #[test]
    fn test_error_rendered_in_inspector_panel() {
        let mut state = InspectorState::default();
        state.error = Some(DevToolsError {
            message: "Request timed out".into(),
            hint: "Press [r] to retry".into(),
        });
        // Render widget, verify buffer contains error message
    }

    #[test]
    fn test_refresh_clears_error() {
        let mut state = AppState::default();
        state.devtools_view_state.inspector.error = Some(/* ... */);
        let (new_state, action) = handler::update(state, Message::RequestWidgetTree);
        assert!(new_state.devtools_view_state.inspector.error.is_none());
        assert!(new_state.devtools_view_state.inspector.loading);
    }

    #[test]
    fn test_layout_shows_select_widget_hint_when_no_selection() {
        let state = LayoutExplorerState::default();
        // No selected node in inspector
        // Render layout explorer, verify "Select a widget" message
    }
}
```

### Notes

- **Keep error messages concise.** The TUI has limited space — long error descriptions get truncated in small terminals.
- **Don't over-engineer error types.** A simple `{ message, hint }` pair is sufficient. No need for error codes or complex error hierarchies.
- **The Performance panel already has decent disconnected messaging** — just verify and enhance slightly if needed.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `DevToolsError { message, hint }` struct; changed `InspectorState.error` and `LayoutExplorerState.error` fields from `Option<String>` to `Option<DevToolsError>` |
| `crates/fdemon-app/src/lib.rs` | Re-exported `DevToolsError` from the crate's public API |
| `crates/fdemon-app/src/handler/devtools.rs` | Added `map_rpc_error()` function with full error classification table; updated `handle_widget_tree_fetch_failed`, `handle_layout_data_fetch_failed`, `handle_widget_tree_fetch_timeout`, `handle_layout_data_fetch_timeout`, and `handle_switch_panel` to use `DevToolsError`; added 14 new tests |
| `crates/fdemon-app/src/handler/update.rs` | Updated `RequestWidgetTree` and `RequestLayoutData` handlers to use `DevToolsError`; added error clearing before fetch start |
| `crates/fdemon-app/src/handler/tests.rs` | Fixed 3 tests that used the old `Option<String>` error type |
| `crates/fdemon-tui/src/widgets/devtools/inspector.rs` | Replaced `render_error` with `render_error_box` that renders centered warning icon + message + hint + key hints; updated imports and tests |
| `crates/fdemon-tui/src/widgets/devtools/layout_explorer.rs` | Replaced `render_error` with `render_error_box`; updated `render_no_selection` to say "Select a widget in the Inspector panel first"; updated imports and tests |

### Notable Decisions/Tradeoffs

1. **`DevToolsError` as struct not enum**: The task says "don't over-engineer" — a simple `{ message, hint }` pair is sufficient. Using a struct allows direct field access without pattern matching in the render code.
2. **map_rpc_error is pub**: Made public so it can be called from `update.rs` and tested externally. All error assignments in both `devtools.rs` and `update.rs` now go through this function.
3. **Error cleared on retry (RequestWidgetTree/RequestLayoutData)**: Added `error = None` before `record_fetch_start()` so pressing `r` clears any previous error immediately, satisfying acceptance criterion 8.
4. **Performance panel not changed**: The Performance panel already has rich disconnected messaging (reconnecting counts, error details, generic fallback). No changes were needed per task notes.
5. **Layout no-selection message**: Changed from "No widget selected. Switch to the Inspector panel..." to "Select a widget in the Inspector panel first" as the primary line, matching acceptance criterion 4 exactly.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app -p fdemon-tui` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app -p fdemon-tui` - Passed (897 + 530 + 1 + 7 = 1,435 tests)

### Risks/Limitations

1. **No Display impl on DevToolsError**: `DevToolsError` derives `Debug` but not `Display`. Any `format!("{}", error)` in tests/code would fail — addressed by updating existing tests to use `.message` and `.hint` fields directly.
