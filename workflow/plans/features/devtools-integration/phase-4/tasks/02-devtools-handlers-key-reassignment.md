## Task: DevTools Handlers & Key Reassignment

**Objective**: Reassign the `d` key from `OpenNewSessionDialog` to `EnterDevToolsMode`, create the `handle_key_devtools()` key handler function, and implement all DevTools message handlers including browser DevTools launching and debug overlay toggling.

**Depends on**: 01-devtools-state-foundation

**Estimated Time**: 5-7 hours

### Scope

- `crates/fdemon-app/src/handler/keys.rs`: Reassign `d`, add `handle_key_devtools()`
- `crates/fdemon-app/src/handler/devtools.rs`: **NEW** — DevTools message handler functions
- `crates/fdemon-app/src/handler/mod.rs`: Add `pub mod devtools;`
- `crates/fdemon-app/src/handler/update.rs`: Route new DevTools messages to handlers
- `crates/fdemon-app/src/actions.rs`: Implement async task spawning for tree/layout fetch and overlay toggle

### Details

#### 1. Reassign `d` Key in `handle_key_normal()` (`keys.rs:160-169`)

**Before:**
```rust
// 'd' for adding device/session (alternative to '+')
InputKey::Char('d') => {
    if state.ui_mode == UiMode::Loading {
        None
    } else {
        Some(Message::OpenNewSessionDialog)
    }
}
```

**After:**
```rust
// 'd' for DevTools mode (Phase 4)
// Only available when a session is running and VM Service is connected
InputKey::Char('d') => {
    let has_vm = state.session_manager.active_session()
        .map(|h| h.session.vm_connected)
        .unwrap_or(false);
    if has_vm {
        Some(Message::EnterDevToolsMode)
    } else {
        // No VM Service connection — silently ignore or show toast
        None
    }
}
```

**`+` key remains unchanged** at `keys.rs:152-158` — it continues to open NewSessionDialog.

#### 2. Add `handle_key_devtools()` to Key Dispatch (`keys.rs:9-18`)

Add the new arm to the `handle_key()` match:

```rust
pub fn handle_key(state: &mut AppState, key: KeyEvent) -> Option<Message> {
    match state.ui_mode {
        // ...existing arms...
        UiMode::DevTools => handle_key_devtools(state, key),
    }
}
```

#### 3. Implement `handle_key_devtools()` (`keys.rs`)

```rust
fn handle_key_devtools(state: &AppState, key: KeyEvent) -> Option<Message> {
    use crossterm::event::{KeyCode, KeyModifiers};

    match (key.modifiers, key.code) {
        // ── Exit DevTools ──
        (KeyModifiers::NONE, KeyCode::Esc) => Some(Message::ExitDevToolsMode),

        // ── Sub-panel switching ──
        (KeyModifiers::NONE, KeyCode::Char('i')) => {
            Some(Message::SwitchDevToolsPanel(DevToolsPanel::Inspector))
        }
        (KeyModifiers::NONE, KeyCode::Char('l')) => {
            Some(Message::SwitchDevToolsPanel(DevToolsPanel::Layout))
        }
        (KeyModifiers::NONE, KeyCode::Char('p')) => {
            Some(Message::SwitchDevToolsPanel(DevToolsPanel::Performance))
        }

        // ── Browser DevTools ──
        (KeyModifiers::NONE, KeyCode::Char('b')) => Some(Message::OpenBrowserDevTools),

        // ── Debug overlay toggles ──
        (KeyModifiers::CONTROL, KeyCode::Char('r')) => {
            Some(Message::ToggleDebugOverlay { extension: DebugOverlayKind::RepaintRainbow })
        }
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
            Some(Message::ToggleDebugOverlay { extension: DebugOverlayKind::PerformanceOverlay })
        }
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            Some(Message::ToggleDebugOverlay { extension: DebugOverlayKind::DebugPaint })
        }

        // ── Inspector-specific navigation (when Inspector panel active) ──
        (KeyModifiers::NONE, KeyCode::Up | KeyCode::Char('k'))
            if state.devtools_view_state.active_panel == DevToolsPanel::Inspector =>
        {
            // Move selection up (handled inline or via Message)
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Up))
        }
        (KeyModifiers::NONE, KeyCode::Down | KeyCode::Char('j'))
            if state.devtools_view_state.active_panel == DevToolsPanel::Inspector =>
        {
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Down))
        }
        (KeyModifiers::NONE, KeyCode::Enter | KeyCode::Right | KeyCode::Char('l'))
            if state.devtools_view_state.active_panel == DevToolsPanel::Inspector =>
        {
            // Note: 'l' for expand only when in Inspector panel (overrides layout switch)
            // Users can still switch to Layout via the 'l' key when not in inspector
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Expand))
        }
        (KeyModifiers::NONE, KeyCode::Left | KeyCode::Char('h'))
            if state.devtools_view_state.active_panel == DevToolsPanel::Inspector =>
        {
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Collapse))
        }
        (KeyModifiers::NONE, KeyCode::Char('r'))
            if state.devtools_view_state.active_panel == DevToolsPanel::Inspector =>
        {
            // Refresh widget tree
            let session_id = state.session_manager.active_session()
                .map(|h| h.session.id);
            session_id.map(|id| Message::RequestWidgetTree { session_id: id })
        }

        // ── Quit still works from DevTools mode ──
        (KeyModifiers::NONE, KeyCode::Char('q')) => Some(Message::RequestQuit),

        _ => None,
    }
}
```

**Note on `l` key conflict:** When in the Inspector sub-panel, `l` is used for expand/navigate-right (vim-style). When in Layout or Performance panels, `l` switches to Layout. To resolve: Inspector navigation keys (`j`/`k`/`h`/`l`) only apply when `active_panel == Inspector`. The panel-switch `l` key should be handled as a fallthrough when NOT in Inspector mode.

Add `InspectorNav` enum to `message.rs`:

```rust
/// Navigation commands for the widget inspector tree view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorNav {
    Up,
    Down,
    Expand,
    Collapse,
}
```

And the additional message variant:
```rust
/// Navigate within the widget inspector tree.
DevToolsInspectorNavigate(InspectorNav),
```

#### 4. Create `handler/devtools.rs` (New File)

```rust
//! DevTools mode message handlers.

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::{DebugOverlayKind, DevToolsPanel, InspectorNav, Message};
use crate::state::AppState;

/// Handle entering DevTools mode from Normal mode.
pub fn handle_enter_devtools_mode(state: &mut AppState) -> UpdateResult {
    state.enter_devtools_mode();

    // If Inspector panel is active and no tree loaded, auto-fetch
    if state.devtools_view_state.active_panel == DevToolsPanel::Inspector
        && state.devtools_view_state.inspector.root.is_none()
    {
        if let Some(handle) = state.session_manager.active_session() {
            let session_id = handle.session.id;
            return UpdateResult::action(UpdateAction::FetchWidgetTree { session_id });
        }
    }

    UpdateResult::none()
}

/// Handle exiting DevTools mode (return to Normal).
pub fn handle_exit_devtools_mode(state: &mut AppState) -> UpdateResult {
    state.exit_devtools_mode();
    UpdateResult::none()
}

/// Handle switching DevTools sub-panel.
pub fn handle_switch_panel(state: &mut AppState, panel: DevToolsPanel) -> UpdateResult {
    state.switch_devtools_panel(panel);

    match panel {
        DevToolsPanel::Inspector => {
            // Auto-fetch tree if not loaded
            if state.devtools_view_state.inspector.root.is_none() {
                if let Some(handle) = state.session_manager.active_session() {
                    let session_id = handle.session.id;
                    state.devtools_view_state.inspector.loading = true;
                    return UpdateResult::action(UpdateAction::FetchWidgetTree { session_id });
                }
            }
        }
        DevToolsPanel::Layout => {
            // Auto-fetch layout if inspector has a selected widget
            // (handled in Task 05)
        }
        DevToolsPanel::Performance => {
            // Performance data is already streaming via Phase 3 — nothing to fetch
        }
    }

    UpdateResult::none()
}

/// Handle widget tree fetch completion.
pub fn handle_widget_tree_fetched(
    state: &mut AppState,
    session_id: uuid::Uuid,
    root: Box<fdemon_core::widget_tree::DiagnosticsNode>,
) -> UpdateResult {
    // Only update if this is for the active session
    if state.session_manager.active_session()
        .map(|h| h.session.id) == Some(session_id)
    {
        state.devtools_view_state.inspector.root = Some(*root);
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.error = None;
        // Auto-expand root node
        if let Some(ref root) = state.devtools_view_state.inspector.root {
            if let Some(ref value_id) = root.value_id {
                state.devtools_view_state.inspector.expanded.insert(value_id.clone());
            }
        }
    }
    UpdateResult::none()
}

/// Handle widget tree fetch failure.
pub fn handle_widget_tree_fetch_failed(
    state: &mut AppState,
    session_id: uuid::Uuid,
    error: String,
) -> UpdateResult {
    if state.session_manager.active_session()
        .map(|h| h.session.id) == Some(session_id)
    {
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.error = Some(error);
    }
    UpdateResult::none()
}

/// Handle inspector tree navigation.
pub fn handle_inspector_navigate(state: &mut AppState, nav: InspectorNav) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    let visible = inspector.visible_nodes();
    let count = visible.len();

    if count == 0 {
        return UpdateResult::none();
    }

    match nav {
        InspectorNav::Up => {
            if inspector.selected_index > 0 {
                inspector.selected_index -= 1;
            }
        }
        InspectorNav::Down => {
            if inspector.selected_index < count.saturating_sub(1) {
                inspector.selected_index += 1;
            }
        }
        InspectorNav::Expand => {
            if let Some((node, _depth)) = visible.get(inspector.selected_index) {
                if let Some(value_id) = &node.value_id {
                    if !inspector.is_expanded(value_id) && !node.children.is_empty() {
                        inspector.expanded.insert(value_id.clone());
                    }
                }
            }
        }
        InspectorNav::Collapse => {
            if let Some((node, _depth)) = visible.get(inspector.selected_index) {
                if let Some(value_id) = &node.value_id {
                    if inspector.is_expanded(value_id) {
                        inspector.expanded.remove(value_id);
                    }
                }
            }
        }
    }

    UpdateResult::none()
}

/// Handle opening Flutter DevTools in the system browser.
pub fn handle_open_browser_devtools(state: &AppState) -> UpdateResult {
    let ws_uri = state.session_manager.active_session()
        .and_then(|h| h.session.ws_uri.clone());

    let Some(ws_uri) = ws_uri else {
        tracing::warn!("Cannot open browser DevTools: no VM Service URI available");
        return UpdateResult::none();
    };

    // Construct the DevTools URL
    let encoded_uri = urlencoding::encode(&ws_uri);
    let url = format!("https://devtools.flutter.dev/?uri={encoded_uri}");

    // Get custom browser from settings (empty = system default)
    let browser = &state.settings.devtools.browser;

    let result = open_url_in_browser(&url, browser);
    if let Err(e) = result {
        tracing::error!("Failed to open browser: {e}");
    }

    UpdateResult::none()
}

/// Open a URL in the system browser (cross-platform).
///
/// If `browser` is non-empty, uses it as the browser command.
/// Otherwise uses the system default.
fn open_url_in_browser(url: &str, browser: &str) -> std::io::Result<()> {
    use std::process::Command;

    if !browser.is_empty() {
        // Custom browser specified in config
        Command::new(browser).arg(url).spawn()?;
        return Ok(());
    }

    // System default browser
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(url).spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/C", "start", "", url]).spawn()?;
    }

    Ok(())
}

/// Handle debug overlay toggle result.
pub fn handle_debug_overlay_toggled(
    state: &mut AppState,
    extension: DebugOverlayKind,
    enabled: bool,
) -> UpdateResult {
    match extension {
        DebugOverlayKind::RepaintRainbow => {
            state.devtools_view_state.overlay_repaint_rainbow = enabled;
        }
        DebugOverlayKind::DebugPaint => {
            state.devtools_view_state.overlay_debug_paint = enabled;
        }
        DebugOverlayKind::PerformanceOverlay => {
            state.devtools_view_state.overlay_performance = enabled;
        }
    }
    UpdateResult::none()
}
```

#### 5. Route Messages in `update.rs`

Add arms to the `update()` match:

```rust
Message::EnterDevToolsMode => devtools::handle_enter_devtools_mode(state),
Message::ExitDevToolsMode => devtools::handle_exit_devtools_mode(state),
Message::SwitchDevToolsPanel(panel) => devtools::handle_switch_panel(state, panel),
Message::OpenBrowserDevTools => devtools::handle_open_browser_devtools(state),
Message::RequestWidgetTree { session_id } => {
    state.devtools_view_state.inspector.loading = true;
    UpdateResult::action(UpdateAction::FetchWidgetTree { session_id })
}
Message::WidgetTreeFetched { session_id, root } => {
    devtools::handle_widget_tree_fetched(state, session_id, root)
}
Message::WidgetTreeFetchFailed { session_id, error } => {
    devtools::handle_widget_tree_fetch_failed(state, session_id, error)
}
Message::RequestLayoutData { session_id, node_id } => {
    state.devtools_view_state.layout_explorer.loading = true;
    UpdateResult::action(UpdateAction::FetchLayoutData { session_id, node_id })
}
Message::LayoutDataFetched { session_id, layout } => {
    // (handled in Task 05)
    UpdateResult::none()
}
Message::LayoutDataFetchFailed { session_id, error } => {
    // (handled in Task 05)
    UpdateResult::none()
}
Message::ToggleDebugOverlay { extension } => {
    if let Some(handle) = state.session_manager.active_session() {
        let session_id = handle.session.id;
        UpdateResult::action(UpdateAction::ToggleOverlay { session_id, extension })
    } else {
        UpdateResult::none()
    }
}
Message::DebugOverlayToggled { extension, enabled } => {
    devtools::handle_debug_overlay_toggled(state, extension, enabled)
}
Message::DevToolsInspectorNavigate(nav) => {
    devtools::handle_inspector_navigate(state, nav)
}
```

#### 6. Implement Async Actions in `actions.rs`

Add `handle_action` arms for the new `UpdateAction` variants:

```rust
UpdateAction::FetchWidgetTree { session_id } => {
    if let Some(handle) = session_manager.get(&session_id) {
        if let Some(vm_handle) = &handle.vm_request_handle {
            let vm_handle = vm_handle.clone();
            let msg_tx = msg_tx.clone();
            tokio::spawn(async move {
                let isolate_id = vm_handle.main_isolate_id().await;
                match isolate_id {
                    Some(isolate_id) => {
                        match fdemon_daemon::vm_service::extensions::inspector::get_root_widget_tree(
                            &vm_handle, &isolate_id, "devtools-inspector"
                        ).await {
                            Ok(root) => {
                                let _ = msg_tx.send(Message::WidgetTreeFetched {
                                    session_id,
                                    root: Box::new(root),
                                });
                            }
                            Err(e) => {
                                let _ = msg_tx.send(Message::WidgetTreeFetchFailed {
                                    session_id,
                                    error: e.to_string(),
                                });
                            }
                        }
                    }
                    None => {
                        let _ = msg_tx.send(Message::WidgetTreeFetchFailed {
                            session_id,
                            error: "No isolate ID available".to_string(),
                        });
                    }
                }
            });
        }
    }
}

UpdateAction::FetchLayoutData { session_id, node_id } => {
    // Similar pattern — uses extensions::layout::get_layout_explorer_node()
    // Full implementation in Task 05
}

UpdateAction::ToggleOverlay { session_id, extension } => {
    if let Some(handle) = session_manager.get(&session_id) {
        if let Some(vm_handle) = &handle.vm_request_handle {
            let vm_handle = vm_handle.clone();
            let msg_tx = msg_tx.clone();
            tokio::spawn(async move {
                let isolate_id = vm_handle.main_isolate_id().await;
                if let Some(isolate_id) = isolate_id {
                    let ext_method = match extension {
                        DebugOverlayKind::RepaintRainbow => {
                            fdemon_daemon::vm_service::extensions::ext::REPAINT_RAINBOW
                        }
                        DebugOverlayKind::DebugPaint => {
                            fdemon_daemon::vm_service::extensions::ext::DEBUG_PAINT
                        }
                        DebugOverlayKind::PerformanceOverlay => {
                            fdemon_daemon::vm_service::extensions::ext::SHOW_PERFORMANCE_OVERLAY
                        }
                    };
                    match fdemon_daemon::vm_service::extensions::overlays::toggle_bool_extension(
                        &vm_handle, &isolate_id, ext_method
                    ).await {
                        Ok(enabled) => {
                            let _ = msg_tx.send(Message::DebugOverlayToggled { extension, enabled });
                        }
                        Err(e) => {
                            tracing::warn!("Failed to toggle overlay: {e}");
                        }
                    }
                }
            });
        }
    }
}
```

### Acceptance Criteria

1. `d` key in Normal mode emits `EnterDevToolsMode` (only when VM connected), not `OpenNewSessionDialog`
2. `+` key still emits `OpenNewSessionDialog` (unchanged)
3. `Esc` in DevTools mode returns to `UiMode::Normal`
4. `i`/`l`/`p` keys switch between Inspector/Layout/Performance panels
5. `b` opens Flutter DevTools URL in system browser using ws_uri
6. `Ctrl+r`/`Ctrl+p`/`Ctrl+d` toggle debug overlays via VM Service
7. Inspector navigation (Up/Down/Enter/Left) works when Inspector panel is active
8. `r` key in Inspector panel triggers widget tree refresh
9. `q` key still triggers quit from DevTools mode
10. Widget tree fetch spawns async task and sends result as Message
11. Browser opening works on macOS (`open`), Linux (`xdg-open`), Windows (`cmd /C start`)
12. Custom browser from `settings.devtools.browser` is respected

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_d_key_emits_enter_devtools_when_vm_connected() {
        let mut state = AppState::new();
        // Setup: create session with vm_connected = true
        // ...
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        let msg = handle_key_normal(&state, key);
        assert_eq!(msg, Some(Message::EnterDevToolsMode));
    }

    #[test]
    fn test_d_key_ignored_when_no_vm_service() {
        let mut state = AppState::new();
        // No sessions — VM not connected
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        let msg = handle_key_normal(&state, key);
        assert_eq!(msg, None);
    }

    #[test]
    fn test_plus_key_still_opens_new_session_dialog() {
        let state = AppState::new();
        let key = KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE);
        let msg = handle_key_normal(&state, key);
        assert_eq!(msg, Some(Message::OpenNewSessionDialog));
    }

    #[test]
    fn test_esc_exits_devtools_mode() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::DevTools;
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let msg = handle_key_devtools(&state, key);
        assert_eq!(msg, Some(Message::ExitDevToolsMode));
    }

    #[test]
    fn test_panel_switch_keys() {
        let state = AppState::new();
        let i_key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
        assert_eq!(handle_key_devtools(&state, i_key), Some(Message::SwitchDevToolsPanel(DevToolsPanel::Inspector)));

        let l_key = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        // Only switches to Layout when NOT in Inspector panel
        // ...

        let p_key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
        assert_eq!(handle_key_devtools(&state, p_key), Some(Message::SwitchDevToolsPanel(DevToolsPanel::Performance)));
    }

    #[test]
    fn test_enter_devtools_mode_handler() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Normal;
        let result = devtools::handle_enter_devtools_mode(&mut state);
        assert_eq!(state.ui_mode, UiMode::DevTools);
    }

    #[test]
    fn test_exit_devtools_mode_handler() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::DevTools;
        devtools::handle_exit_devtools_mode(&mut state);
        assert_eq!(state.ui_mode, UiMode::Normal);
    }
}
```

### Notes

- **URL encoding**: The `ws_uri` must be URL-encoded when constructing the DevTools browser URL. Consider using `urlencoding::encode()` or `percent_encoding` crate. Check if either is already a dependency, otherwise `urlencoding` is lightweight (zero-dep).
- **`l` key conflict in Inspector**: When the Inspector panel is active, `l` is used for "expand right" (vim navigation). The panel-switch `l` (to Layout) should only fire when NOT in Inspector panel, OR use a different approach: `l` always goes to Layout, and `Enter`/`Right` are the only expand keys. The implementer should pick the simpler approach.
- **`open_url_in_browser` is fire-and-forget**: Uses `Command::spawn()` (non-blocking), same pattern as `editor.rs:232-249`. The browser process runs independently.
- **Overlay toggle is async**: The VM Service call returns the new state (enabled/disabled). The `DebugOverlayToggled` message updates `DevToolsViewState` overlay flags.
- **Existing tests in `keys.rs`**: There are already tests for `d` key behavior (lines 740-760). These must be updated to reflect the new DevTools behavior.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/message.rs` | Added `InspectorNav` enum and `DevToolsInspectorNavigate(InspectorNav)` message variant |
| `crates/fdemon-app/src/handler/mod.rs` | Added `pub(crate) mod devtools;`; updated `FetchWidgetTree` and `ToggleOverlay` variants to carry `vm_handle: Option<VmRequestHandle>` |
| `crates/fdemon-app/src/handler/devtools.rs` | **New file** — all DevTools handler functions with full test coverage |
| `crates/fdemon-app/src/handler/keys.rs` | Reassigned `d` key (VM-connected guard); replaced `UiMode::DevTools => None` stub; added `handle_key_devtools()`; updated key tests |
| `crates/fdemon-app/src/handler/update.rs` | Replaced stub DevTools match arms with real `devtools::*` handler calls; added `DevToolsInspectorNavigate` arm |
| `crates/fdemon-app/src/process.rs` | Added `hydrate_fetch_widget_tree()` and `hydrate_toggle_overlay()` hydration functions; chained them in dispatch loop |
| `crates/fdemon-app/src/actions.rs` | Replaced `FetchWidgetTree` and `ToggleOverlay` stubs with real async implementations (`spawn_fetch_widget_tree`, `spawn_toggle_overlay`); added `DebugOverlayKind`, `ext`, `parse_bool_extension_response`, `parse_diagnostics_node_response` imports |

### Notable Decisions/Tradeoffs

1. **`SessionId = u64`**: Task spec referenced `uuid::Uuid` but the actual type is `u64`. All handler functions use the correct type.

2. **`session_manager.selected()` not `active_session()`**: Task spec referenced a non-existent `active_session()` method. Corrected to `selected()` throughout.

3. **Hydration pattern for `vm_handle`**: `FetchWidgetTree` and `ToggleOverlay` follow the same hydration pattern as `StartPerformanceMonitoring` — handlers return `vm_handle: None` and `process.rs` populates it from `AppState.session_manager` before dispatch. If the handle is unavailable (VM not connected), the action is silently discarded.

4. **Inline overlay flip using `VmRequestHandle`**: The overlay flip functions (`flip_overlay`, `toggle_bool_extension`) in `fdemon_daemon` take `&VmServiceClient`, but `handle_action` only receives `VmRequestHandle`. The flip was implemented inline using `VmRequestHandle.call_extension()` + `parse_bool_extension_response()` directly, avoiding the `VmServiceClient` dependency.

5. **`get_root_widget_tree` inline in `actions.rs`**: Similarly, `get_root_widget_tree` takes `&VmServiceClient`. The widget tree fetch was inlined in `spawn_fetch_widget_tree()` using `VmRequestHandle.call_extension()` with the same fallback logic (getRootWidgetTree → getRootWidgetSummaryTree).

6. **Inline URL encoding**: No `urlencoding` crate in the workspace. Implemented `percent_encode_uri()` inline following RFC 3986 unreserved characters, placed in `devtools.rs`.

7. **`l` key conflict resolution**: When `active_panel == Inspector`, `l` means expand-right (vim navigation). When not in Inspector, `l` switches to Layout panel. Implemented via Rust match guards (`if in_inspector` / `if !in_inspector`).

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed (0 warnings)
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)
- `cargo test --workspace` — 1941 unit tests passed across all 4 crates (fdemon-app: 823, fdemon-daemon: 337, fdemon-tui: 463, fdemon-core: 318); e2e PTY tests have pre-existing failures unrelated to this change

### Risks/Limitations

1. **`FetchLayoutData` remains a stub**: Per task spec, full implementation belongs to Task 05 (layout explorer). The match arm logs a debug trace and no-ops.

2. **Object group lifecycle**: `spawn_fetch_widget_tree` uses a fixed group name `"fdemon-inspector-1"`. A persistent `ObjectGroupManager` per session would be needed for multi-fetch workflows (refresh, subtree fetch). This is sufficient for the initial inspector view and will be addressed in follow-up tasks.

3. **Pre-existing e2e test failures**: 24 e2e tests in `flutter-demon` binary crate fail due to PTY/process timeout issues (settings_page and tui_interaction). These are pre-existing and unrelated to this task.
