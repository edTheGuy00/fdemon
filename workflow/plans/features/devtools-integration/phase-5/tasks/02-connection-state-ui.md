## Task: Connection State UI Indicators & Timeout Handling

**Objective**: Surface the VM Service connection state (connected, reconnecting, disconnected, timed out) to the TUI layer so DevTools panels show clear visual indicators, and add timeout handling for slow VM responses.

**Depends on**: None

**Estimated Time**: 4-6 hours

### Scope

- `crates/fdemon-app/src/state.rs`: Add connection state fields to `DevToolsViewState` or session state
- `crates/fdemon-app/src/message.rs`: Add message variants for connection state changes (if not already present)
- `crates/fdemon-app/src/handler/devtools.rs`: Handle connection state change messages, update UI state
- `crates/fdemon-app/src/actions.rs`: Add timeout wrappers around VM RPC calls, emit timeout messages
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: Render connection state indicator in the DevTools tab bar
- `crates/fdemon-tui/src/widgets/devtools/inspector.rs`: Show connection-lost state instead of empty tree
- `crates/fdemon-tui/src/widgets/devtools/layout_explorer.rs`: Show connection-lost state
- `crates/fdemon-tui/src/widgets/devtools/performance.rs`: Show reconnecting state (already shows disconnected)

### Details

#### 1. Current State Analysis

The VM service client (`client.rs`) already tracks `ConnectionState` internally:
```rust
enum ConnectionState {
    Disconnected,
    Connected,
    Reconnecting { attempt: u32 },
}
```

However, this state is internal to the daemon crate. The app layer knows about VM connection via:
- `SessionHandle.session.vm_connected: bool` — binary connected/not flag
- `Message::VmServiceConnected` / `Message::VmServiceDisconnected` — events

**Gap**: There's no `Reconnecting` state surfaced to the app/TUI. The TUI can only see connected or not-connected.

#### 2. Surface Reconnection State

**Option A (Recommended): Extend the existing message/state system**

Add a new field to session state or `DevToolsViewState`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting { attempt: u32, max_attempts: u32 },
    TimedOut,  // A specific request timed out
}
```

Add a message variant:
```rust
Message::VmServiceReconnecting { session_id: Uuid, attempt: u32 }
```

The daemon's background task already sends `VmServiceDisconnected` when connection drops. Modify it to also send `VmServiceReconnecting` events during the backoff loop, or add a new event variant to `VmServiceEvent`.

**Option B: Poll connection state**

Add a method to `VmServiceClient` that returns the current `ConnectionState`, and poll it periodically. This is simpler but less reactive.

**Recommendation**: Option A — it integrates with the existing TEA message flow.

#### 3. TUI Connection Indicator

In `widgets/devtools/mod.rs`, the tab bar already renders overlay indicators on the right side. Add a connection status indicator next to them:

```
┌─ DevTools ───────────────────────────────────────── ⚡ Connected ┐
│ [i] Inspector  [l] Layout  [p] Performance          Rainbow      │
└──────────────────────────────────────────────────────────────────┘
```

When reconnecting:
```
┌─ DevTools ─────────────────────────────── ↻ Reconnecting (2/10) ┐
│ [i] Inspector  [l] Layout  [p] Performance                       │
└──────────────────────────────────────────────────────────────────┘
```

When disconnected:
```
┌─ DevTools ──────────────────────────────────────── ✗ Disconnected ┐
│ [i] Inspector  [l] Layout  [p] Performance                        │
└───────────────────────────────────────────────────────────────────┘
```

Use appropriate colors:
- Connected: `Color::Green` with `⚡` or just no indicator (clean when connected)
- Reconnecting: `Color::Yellow` with `↻` symbol and attempt counter
- Disconnected: `Color::Red` with `✗` symbol

#### 4. Panel-Specific Disconnected States

Each panel should gracefully show a disconnected/unavailable state:

**Inspector** (when disconnected):
```
╭──────────────────────────────────────╮
│  VM Service disconnected             │
│                                      │
│  Widget tree is unavailable.         │
│  Waiting for reconnection...         │
│                                      │
│  Press [b] to open browser DevTools  │
│  Press [Esc] to return to logs       │
╰──────────────────────────────────────╯
```

**Layout Explorer** (when disconnected): Similar message.

**Performance** (when disconnected): Already handles this — shows "VM Service not connected" when `vm_connected = false`. Verify this also shows during `Reconnecting` state and consider showing "Reconnecting..." text.

#### 5. Request Timeout Handling

Currently, VM RPC calls (FetchWidgetTree, FetchLayoutData, ToggleOverlay) have no timeout. If the VM is slow or hung, the UI shows a loading spinner forever.

Add `tokio::time::timeout()` wrappers in `actions.rs`:

```rust
// In spawn_fetch_widget_tree():
let result = tokio::time::timeout(
    Duration::from_secs(10),
    vm_handle.call_extension(GET_ROOT_WIDGET_TREE, params)
).await;

match result {
    Ok(Ok(response)) => { /* parse tree */ }
    Ok(Err(rpc_error)) => { /* existing error handling */ }
    Err(_timeout) => {
        // Send timeout message
        tx.send(Message::WidgetTreeFetchTimeout { session_id }).ok();
    }
}
```

Add corresponding message variants:
```rust
Message::WidgetTreeFetchTimeout { session_id: Uuid },
Message::LayoutDataFetchTimeout { session_id: Uuid },
```

Handle timeouts in the handler by:
1. Setting `inspector.loading = false`
2. Setting an error state (e.g., `inspector.error = Some("Widget tree fetch timed out. Press [r] to retry.")`)
3. The panel renders this error message instead of the empty tree

Default timeout: 10 seconds (configurable in Phase 5 config if desired).

### Acceptance Criteria

1. `VmConnectionStatus` enum exists with Connected, Disconnected, Reconnecting, TimedOut variants
2. The DevTools tab bar shows a color-coded connection indicator
3. Reconnecting state shows attempt count (e.g., "Reconnecting 2/10")
4. Each DevTools panel shows a meaningful disconnected state (not just empty)
5. Widget tree fetch has a 10-second timeout
6. Layout data fetch has a 10-second timeout
7. Timeout results in a user-visible error message with retry hint
8. Inspector and Layout panels show the error message and allow retry with `r`
9. Performance panel distinguishes "disconnected" from "reconnecting" in its text

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_connection_status_display() {
        assert_eq!(VmConnectionStatus::Connected.label(), "Connected");
        assert_eq!(
            VmConnectionStatus::Reconnecting { attempt: 2, max_attempts: 10 }.label(),
            "Reconnecting (2/10)"
        );
    }

    #[test]
    fn test_inspector_shows_disconnected_state() {
        let mut state = DevToolsViewState::default();
        // Set vm_connected = false or connection_status = Disconnected
        // Render inspector widget, verify it shows disconnected message
    }

    #[test]
    fn test_timeout_sets_error_state() {
        let mut state = AppState::default();
        // Process WidgetTreeFetchTimeout message
        let (new_state, _) = handler::update(
            state,
            Message::WidgetTreeFetchTimeout { session_id: uuid },
        );
        assert!(new_state.devtools_view_state.inspector.error.is_some());
        assert!(!new_state.devtools_view_state.inspector.loading);
    }

    #[test]
    fn test_refresh_clears_error_and_retries() {
        // After a timeout, pressing 'r' should clear the error and re-fetch
    }
}
```

### Notes

- **Don't break the daemon crate's API boundary.** The `ConnectionState` enum is internal to `fdemon-daemon`. Surface it via messages (`VmServiceEvent::Reconnecting`) rather than exposing the internal enum.
- **The performance panel already handles `vm_connected = false`** — verify it works and just enhance the messaging.
- **Timeout value (10s) is reasonable** for Flutter's VM Service. Some operations (e.g., fetching a very large widget tree) can genuinely take several seconds. Don't set it too low.
- **Consider cancellation**: If the user exits DevTools mode while a fetch is in-flight, the timeout shouldn't cause errors. The response message handler should check that the session still matches.
