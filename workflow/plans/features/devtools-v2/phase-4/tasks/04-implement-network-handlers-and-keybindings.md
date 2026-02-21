## Task: Implement Network Handlers and Key Bindings

**Objective**: Create the handler sub-module for all network-related message processing and wire up key bindings for the Network panel. This is the TEA "update" logic: it receives network messages and UI interactions, mutates `NetworkState`, and returns `UpdateAction`s for async operations.

**Depends on**: Task 03 (add-network-state-and-messages)

### Scope

- `crates/fdemon-app/src/handler/devtools/network.rs`: **NEW** — All network handler functions
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Add `pub(crate) mod network;`, extend `handle_switch_panel()`, extend `parse_default_panel()`
- `crates/fdemon-app/src/handler/keys.rs`: Add `'n'` key binding, add `in_network` panel-specific key guards
- `crates/fdemon-app/src/handler/update.rs`: Wire all network message variants to handler functions

### Details

#### Create `handler/devtools/network.rs`

Follow the pattern from `handler/devtools/inspector.rs` and `handler/devtools/performance.rs`:
- All functions are `pub(crate)`
- All functions take `&mut AppState` and return `UpdateResult`
- Functions are pure (no async, no VM calls) — they only mutate state and return actions

```rust
//! # Network Monitor Handlers
//!
//! TEA update functions for all network-related messages: HTTP profile
//! polling results, request detail fetching, navigation, filtering,
//! recording toggle, and clear operations.

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::NetworkNav;
use crate::state::AppState;
use fdemon_core::network::{HttpProfileEntry, HttpProfileEntryDetail, NetworkDetailTab};

/// Handle incoming HTTP profile poll results.
///
/// Merges new/updated entries into the session's network state and
/// stores the timestamp for incremental polling.
pub(crate) fn handle_http_profile_received(
    state: &mut AppState,
    session_id: SessionId,
    timestamp: i64,
    entries: Vec<HttpProfileEntry>,
) -> UpdateResult {
    if let Some(session) = state.session_manager.session_mut(&session_id) {
        session.network.merge_entries(entries);
        session.network.last_poll_timestamp = Some(timestamp);
    }
    UpdateResult::none()
}

/// Handle full request detail received.
pub(crate) fn handle_http_request_detail_received(
    state: &mut AppState,
    session_id: SessionId,
    detail: Box<HttpProfileEntryDetail>,
) -> UpdateResult {
    if let Some(session) = state.session_manager.session_mut(&session_id) {
        session.network.loading_detail = false;
        session.network.selected_detail = Some(detail);
    }
    UpdateResult::none()
}

/// Handle detail fetch failure.
pub(crate) fn handle_http_request_detail_failed(
    state: &mut AppState,
    session_id: SessionId,
    error: String,
) -> UpdateResult {
    if let Some(session) = state.session_manager.session_mut(&session_id) {
        session.network.loading_detail = false;
        session.network.last_error = Some(error);
    }
    UpdateResult::none()
}

/// Handle network extensions unavailable (release mode).
pub(crate) fn handle_network_extensions_unavailable(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    if let Some(session) = state.session_manager.session_mut(&session_id) {
        session.network.extensions_available = Some(false);
        session.network.recording = false;
    }
    UpdateResult::none()
}

/// Handle network monitoring task started.
pub(crate) fn handle_network_monitoring_started(
    state: &mut AppState,
    session_id: SessionId,
    shutdown_tx: Arc<watch::Sender<bool>>,
    task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.handle_mut(&session_id) {
        handle.network_shutdown_tx = Some(shutdown_tx);
        handle.network_task_handle = task_handle.lock().ok().and_then(|mut g| g.take());
    }
    UpdateResult::none()
}

/// Navigate the request list.
pub(crate) fn handle_network_navigate(
    state: &mut AppState,
    nav: NetworkNav,
) -> UpdateResult {
    let Some(session) = state.session_manager.active_session_mut() else {
        return UpdateResult::none();
    };
    match nav {
        NetworkNav::Up => session.network.select_prev(),
        NetworkNav::Down => session.network.select_next(),
        NetworkNav::PageUp => {
            for _ in 0..10 { session.network.select_prev(); }
        }
        NetworkNav::PageDown => {
            for _ in 0..10 { session.network.select_next(); }
        }
    }

    // Trigger detail fetch for the newly selected request
    fetch_selected_detail_action(state)
}

/// Select a specific request by index.
pub(crate) fn handle_network_select_request(
    state: &mut AppState,
    index: Option<usize>,
) -> UpdateResult {
    let Some(session) = state.session_manager.active_session_mut() else {
        return UpdateResult::none();
    };
    session.network.selected_index = index;
    session.network.selected_detail = None;

    if index.is_some() {
        fetch_selected_detail_action(state)
    } else {
        UpdateResult::none()
    }
}

/// Switch detail sub-tab.
pub(crate) fn handle_network_switch_detail_tab(
    state: &mut AppState,
    tab: NetworkDetailTab,
) -> UpdateResult {
    if let Some(session) = state.session_manager.active_session_mut() {
        session.network.detail_tab = tab;
    }
    UpdateResult::none()
}

/// Toggle recording on/off.
pub(crate) fn handle_toggle_network_recording(
    state: &mut AppState,
) -> UpdateResult {
    let Some(session) = state.session_manager.active_session_mut() else {
        return UpdateResult::none();
    };
    session.network.recording = !session.network.recording;
    // Note: the polling task checks session.network.recording and skips
    // poll cycles when false. No need to start/stop the task itself.
    UpdateResult::none()
}

/// Clear all recorded network entries.
pub(crate) fn handle_clear_network_profile(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    if let Some(session) = state.session_manager.session_mut(&session_id) {
        session.network.clear();
    }
    // Also clear on the VM side
    UpdateResult::action(UpdateAction::ClearHttpProfile {
        session_id,
        vm_handle: None, // hydrated by process.rs
    })
}

/// Update filter text.
pub(crate) fn handle_network_filter_changed(
    state: &mut AppState,
    filter: String,
) -> UpdateResult {
    if let Some(session) = state.session_manager.active_session_mut() {
        session.network.filter = filter;
        // Reset selection when filter changes
        session.network.selected_index = None;
        session.network.selected_detail = None;
        session.network.scroll_offset = 0;
    }
    UpdateResult::none()
}

/// Helper: build a FetchHttpRequestDetail action for the currently selected entry.
fn fetch_selected_detail_action(state: &AppState) -> UpdateResult {
    let session_id = state.session_manager.active_session_id();
    let Some(session_id) = session_id else {
        return UpdateResult::none();
    };
    let Some(session) = state.session_manager.session(&session_id) else {
        return UpdateResult::none();
    };
    let Some(entry) = session.network.selected_entry() else {
        return UpdateResult::none();
    };

    let request_id = entry.id.clone();
    // Mark as loading
    // Note: we can't mutate here since we only have &AppState.
    // The loading_detail flag is set by the caller before calling this.

    UpdateResult::action(UpdateAction::FetchHttpRequestDetail {
        session_id,
        request_id,
        vm_handle: None, // hydrated by process.rs
    })
}
```

#### Update `handler/devtools/mod.rs`

1. Add module declaration:
```rust
pub(crate) mod network;
```

2. Extend `handle_switch_panel()` — add the `Network` arm:
```rust
DevToolsPanel::Network => {
    state.devtools_view_state.active_panel = DevToolsPanel::Network;
    // Start network monitoring if not already running and VM connected
    if let Some(session_id) = state.session_manager.active_session_id() {
        if let Some(session) = state.session_manager.session(&session_id) {
            if session.vm_connected && session.network.extensions_available != Some(false) {
                return UpdateResult::action(UpdateAction::StartNetworkMonitoring {
                    session_id,
                    handle: None,
                    poll_interval_ms: 1000,
                });
            }
        }
    }
    UpdateResult::none()
}
```

3. Extend `parse_default_panel()`:
```rust
"network" | "net" => DevToolsPanel::Network,
```

#### Update `handler/keys.rs`

In `handle_key_devtools()`, add:

1. Panel switch key (alongside existing `'i'` and `'p'`):
```rust
InputKey::Char('n') => Some(Message::SwitchDevToolsPanel(DevToolsPanel::Network)),
```

2. Panel-specific guards. Add `let in_network = ...` alongside existing `in_inspector` and `in_performance`:
```rust
let in_network = state.devtools_view_state.active_panel == DevToolsPanel::Network;
```

3. Network panel key bindings:
```rust
// Navigation
InputKey::Up | InputKey::Char('k') if in_network => Some(Message::NetworkNavigate(NetworkNav::Up)),
InputKey::Down | InputKey::Char('j') if in_network => Some(Message::NetworkNavigate(NetworkNav::Down)),
InputKey::PageUp if in_network => Some(Message::NetworkNavigate(NetworkNav::PageUp)),
InputKey::PageDown if in_network => Some(Message::NetworkNavigate(NetworkNav::PageDown)),

// Selection
InputKey::Enter if in_network => {
    // In narrow mode: toggle detail view; in wide mode: no-op (details always visible)
    // For now, just ensure the selected request detail is fetched
    if let Some(session) = state.session_manager.active_session() {
        if session.network.selected_index.is_some() {
            return Some(Message::NetworkSelectRequest {
                index: session.network.selected_index,
            });
        }
    }
    None
}
InputKey::Esc if in_network => {
    // Deselect current request
    Some(Message::NetworkSelectRequest { index: None })
}

// Detail sub-tab switching
InputKey::Char('g') if in_network => Some(Message::NetworkSwitchDetailTab(NetworkDetailTab::General)),
InputKey::Char('h') if in_network => Some(Message::NetworkSwitchDetailTab(NetworkDetailTab::Headers)),
InputKey::Char('q') if in_network => Some(Message::NetworkSwitchDetailTab(NetworkDetailTab::RequestBody)),
InputKey::Char('s') if in_network => Some(Message::NetworkSwitchDetailTab(NetworkDetailTab::ResponseBody)),
InputKey::Char('t') if in_network => Some(Message::NetworkSwitchDetailTab(NetworkDetailTab::Timing)),

// Recording toggle
InputKey::Char(' ') if in_network => Some(Message::ToggleNetworkRecording),

// Clear
InputKey::Ctrl('x') if in_network => {
    state.session_manager.active_session_id()
        .map(|sid| Message::ClearNetworkProfile { session_id: sid })
}

// Filter mode (future: toggle input mode for filter text)
// InputKey::Char('/') if in_network => Some(Message::EnterNetworkFilterMode),
```

#### Update `handler/update.rs`

Wire all network message variants to handler functions in the `match message { ... }` block:

```rust
// ── Network Monitor Messages ─────────────────────────────────────────
Message::VmServiceHttpProfileReceived { session_id, timestamp, entries } =>
    devtools::network::handle_http_profile_received(state, session_id, timestamp, entries),
Message::VmServiceHttpRequestDetailReceived { session_id, detail } =>
    devtools::network::handle_http_request_detail_received(state, session_id, detail),
Message::VmServiceHttpRequestDetailFailed { session_id, error } =>
    devtools::network::handle_http_request_detail_failed(state, session_id, error),
Message::VmServiceNetworkMonitoringStarted { session_id, network_shutdown_tx, network_task_handle } =>
    devtools::network::handle_network_monitoring_started(state, session_id, network_shutdown_tx, network_task_handle),
Message::VmServiceNetworkExtensionsUnavailable { session_id } =>
    devtools::network::handle_network_extensions_unavailable(state, session_id),
Message::NetworkNavigate(nav) =>
    devtools::network::handle_network_navigate(state, nav),
Message::NetworkSelectRequest { index } =>
    devtools::network::handle_network_select_request(state, index),
Message::NetworkSwitchDetailTab(tab) =>
    devtools::network::handle_network_switch_detail_tab(state, tab),
Message::ToggleNetworkRecording =>
    devtools::network::handle_toggle_network_recording(state),
Message::ClearNetworkProfile { session_id } =>
    devtools::network::handle_clear_network_profile(state, session_id),
Message::NetworkFilterChanged(filter) =>
    devtools::network::handle_network_filter_changed(state, filter),
```

### Acceptance Criteria

1. `handler/devtools/network.rs` exists with all handler functions
2. `handle_http_profile_received()` merges entries and stores timestamp
3. `handle_http_request_detail_received()` stores detail and clears loading flag
4. `handle_network_navigate()` moves selection and triggers detail fetch
5. `handle_toggle_network_recording()` flips recording flag
6. `handle_clear_network_profile()` clears state and returns `ClearHttpProfile` action
7. `handle_network_filter_changed()` updates filter and resets selection
8. `'n'` key switches to Network panel from DevTools mode
9. `Up/Down/j/k` navigate request list when in Network panel
10. `g/h/q/s/t` switch detail sub-tabs when in Network panel
11. `Space` toggles recording when in Network panel
12. `Ctrl+x` clears network history when in Network panel
13. All message variants wired in `update.rs`
14. `cargo check -p fdemon-app` passes
15. `cargo test -p fdemon-app` passes

### Testing

Add tests in `handler/devtools/network.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    // Follow the pattern from handler/devtools/inspector.rs tests
    // Use the same test state helpers

    #[test]
    fn test_handle_http_profile_received_stores_entries() {
        let mut state = make_devtools_state();
        let session_id = active_session_id(&state);
        let entries = vec![make_entry("1", "GET", Some(200))];
        let result = handle_http_profile_received(&mut state, session_id, 5000, entries);
        assert!(result.action.is_none());
        let session = state.session_manager.active_session().unwrap();
        assert_eq!(session.network.entries.len(), 1);
        assert_eq!(session.network.last_poll_timestamp, Some(5000));
    }

    #[test]
    fn test_handle_toggle_recording() {
        let mut state = make_devtools_state();
        assert!(state.session_manager.active_session().unwrap().network.recording);
        handle_toggle_network_recording(&mut state);
        assert!(!state.session_manager.active_session().unwrap().network.recording);
        handle_toggle_network_recording(&mut state);
        assert!(state.session_manager.active_session().unwrap().network.recording);
    }

    #[test]
    fn test_handle_clear_returns_action() {
        let mut state = make_devtools_state();
        let session_id = active_session_id(&state);
        let result = handle_clear_network_profile(&mut state, session_id);
        assert!(matches!(result.action, Some(UpdateAction::ClearHttpProfile { .. })));
    }

    #[test]
    fn test_handle_navigate_down_selects_first() {
        let mut state = make_devtools_state_with_entries(3);
        let result = handle_network_navigate(&mut state, NetworkNav::Down);
        let session = state.session_manager.active_session().unwrap();
        assert_eq!(session.network.selected_index, Some(0));
    }

    #[test]
    fn test_handle_switch_detail_tab() {
        let mut state = make_devtools_state();
        handle_network_switch_detail_tab(&mut state, NetworkDetailTab::Headers);
        let session = state.session_manager.active_session().unwrap();
        assert_eq!(session.network.detail_tab, NetworkDetailTab::Headers);
    }

    #[test]
    fn test_handle_filter_resets_selection() {
        let mut state = make_devtools_state_with_entries(3);
        state.session_manager.active_session_mut().unwrap().network.selected_index = Some(1);
        handle_network_filter_changed(&mut state, "POST".to_string());
        let session = state.session_manager.active_session().unwrap();
        assert_eq!(session.network.filter, "POST");
        assert!(session.network.selected_index.is_none());
    }

    #[test]
    fn test_handle_extensions_unavailable() {
        let mut state = make_devtools_state();
        let session_id = active_session_id(&state);
        handle_network_extensions_unavailable(&mut state, session_id);
        let session = state.session_manager.active_session().unwrap();
        assert_eq!(session.network.extensions_available, Some(false));
        assert!(!session.network.recording);
    }
}
```

### Notes

- **Handler purity**: All handler functions are synchronous and pure — they mutate `AppState` and return `UpdateResult` but never perform I/O. Async operations (VM service calls, polling) are triggered via `UpdateAction` and executed in `actions.rs`.
- **Detail fetch on navigation**: When the user navigates to a different request (`handle_network_navigate`), the handler returns a `FetchHttpRequestDetail` action. The action will be hydrated with the VM handle by `process.rs` and dispatched to `actions.rs`.
- **Recording toggle doesn't stop the polling task**: The polling task checks `session.network.recording` each cycle and skips the poll when false. This avoids the complexity of stopping/restarting the background task. The task only dies on session disconnect.
- **Key binding ordering**: Network-specific guards (`if in_network`) must be placed correctly relative to global DevTools keys. The `'n'` key is a global panel switch (not guarded by `in_network`). Sub-tab keys (`g`, `h`, `q`, `s`, `t`) are only active `if in_network`. This prevents conflicts with other panels using the same keys.
- **Filter mode**: The `/` key for entering filter mode is commented out initially. Full filter input (text entry with cursor) would require a mini text input widget, which can be added as a follow-up. For now, the `NetworkFilterChanged` message handler exists but is only triggered programmatically.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/network.rs` | NEW — all network handler functions with 19 unit tests |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Added `pub(crate) mod network;`, extended `handle_switch_panel()` with Network arm, extended `parse_default_panel()` with `"network"` / `"net"` |
| `crates/fdemon-app/src/handler/keys.rs` | Added `NetworkNav` and `NetworkDetailTab` imports; added `in_network` guard; added `'n'` panel-switch key; added Up/Down/j/k/PageUp/PageDown navigation, Enter selection, Esc deselect, g/h/q/s/t detail-tab, Space recording toggle, Ctrl+x clear |
| `crates/fdemon-app/src/handler/update.rs` | Replaced all 11 network message stub impls with calls to `devtools::network::*` handler functions; added network task abort to `VmServiceDisconnected` |
| `crates/fdemon-app/src/session/handle.rs` | Added `network_shutdown_tx` and `network_task_handle` fields to `SessionHandle`; updated `new()`, `Debug` impl |

### Notable Decisions/Tradeoffs

1. **SessionHandle fields**: The task called `handle.network_shutdown_tx` and `handle.network_task_handle` in the handler — these required adding fields to `SessionHandle`. This mirrors the identical pattern for `perf_shutdown_tx` / `perf_task_handle`.
2. **Esc key in Network panel**: Rather than immediately exiting DevTools, Esc first clears the network request selection (analogous to how Performance deselects a frame before exiting). This provides a more natural "dismiss" UX.
3. **`'q'` key guard ordering**: The `'q'` sub-tab key is guarded by `if in_network` and appears before the global `'q' => RequestQuit` arm. This is safe because in_network is false for other panels.
4. **`fetch_selected_detail_action` uses immutable borrow**: The helper takes `&AppState` (not `&mut`) to avoid borrow conflicts when called after navigation mutations.
5. **Network task cleanup on disconnect**: Added abort/shutdown for `network_task_handle`/`network_shutdown_tx` to `VmServiceDisconnected` handler, matching the same cleanup done for performance tasks.

### Testing Performed

- `cargo check -p fdemon-app` — Passed
- `cargo test -p fdemon-app` — Passed (992 passed, 5 ignored)
- `cargo test -p fdemon-app handler::devtools::network` — Passed (19 new tests)
- `cargo fmt --all` — No changes needed
- `cargo clippy -p fdemon-app -- -D warnings` — Passed (no warnings)
- `cargo check --workspace` — Passed

### Risks/Limitations

1. **`'q'` key conflict in Network panel**: The `'q'` key switches to RequestBody sub-tab when `in_network` is active. Users who habitually press `'q'` to quit while in the Network panel will instead switch detail tabs. This is mitigated by the fact that Esc exits DevTools mode from any panel.
2. **`'h'` key conflict**: `'h'` is Headers sub-tab in Network but Collapse in Inspector. The guards are mutually exclusive so there is no actual conflict, but it may surprise users switching panels.
