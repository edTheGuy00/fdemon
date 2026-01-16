# Task 04: Implement Device Cache Usage

## Objective

Use the existing device cache when opening NewSessionDialog to provide instant device list, with background refresh for freshness.

## Priority

**High** - Significant UX improvement for perceived performance

## Problem

Current behavior:
1. User opens dialog → Always shows loading spinner
2. `flutter devices --machine` runs (3-5 seconds)
3. Results populate the list

Even if devices were discovered 5 seconds ago, user waits again.

## Existing Infrastructure

**Cache exists but is unused:**

```rust
// src/app/state.rs:335-342
pub device_cache: Option<Vec<Device>>,
pub devices_last_updated: Option<std::time::Instant>,

// src/app/state.rs:497-508
pub fn get_cached_devices(&self) -> Option<&Vec<Device>> {
    const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);
    if let (Some(ref devices), Some(updated)) = (&self.device_cache, self.devices_last_updated) {
        if updated.elapsed() < CACHE_TTL {
            return Some(devices);
        }
    }
    None
}
```

## Solution

### Step 1: Check Cache Before Triggering Discovery

**File:** `src/app/handler/new_session/navigation.rs`

Update `handle_open_new_session_dialog`:

```rust
pub fn handle_open_new_session_dialog(state: &mut AppState) -> UpdateResult {
    let configs = crate::config::load_all_configs(&state.project_path);
    state.show_new_session_dialog(configs);

    // Check cache first
    if let Some(cached_devices) = state.get_cached_devices() {
        tracing::debug!(
            "Using cached devices ({} devices, age: {:?})",
            cached_devices.len(),
            state.devices_last_updated.map(|t| t.elapsed())
        );

        // Populate dialog with cached devices immediately
        state.new_session_dialog_state
            .target_selector
            .set_connected_devices(cached_devices.clone());

        // Trigger background refresh to keep data fresh
        return UpdateResult::action(UpdateAction::RefreshDevicesBackground);
    }

    // Cache miss or expired - show loading and discover
    tracing::debug!("Device cache miss, triggering discovery");
    state.new_session_dialog_state.target_selector.loading = true;
    UpdateResult::action(UpdateAction::DiscoverDevices)
}
```

### Step 2: Add Background Refresh Action

**File:** `src/app/handler/mod.rs`

Add new action variant:

```rust
pub enum UpdateAction {
    // ... existing variants ...

    /// Refresh devices in background (no loading spinner)
    RefreshDevicesBackground,
}
```

### Step 3: Handle Background Refresh Action

**File:** `src/tui/actions.rs`

Add handling for the new action:

```rust
pub async fn handle_action(
    action: UpdateAction,
    // ... params ...
) {
    match action {
        // ... existing matches ...

        UpdateAction::RefreshDevicesBackground => {
            // Same as DiscoverDevices but doesn't set loading flag
            tokio::spawn(async move {
                match crate::daemon::discover_devices().await {
                    Ok(result) => {
                        let _ = msg_tx.send(Message::DevicesDiscovered {
                            devices: result.devices,
                        }).await;
                    }
                    Err(e) => {
                        // Log error but don't show to user (background refresh)
                        tracing::warn!("Background device refresh failed: {}", e);
                    }
                }
            });
        }

        // ... rest of matches ...
    }
}
```

### Step 4: Update DevicesDiscovered Handler

**File:** `src/app/handler/update.rs`

Ensure handler updates both cache and dialog state gracefully:

```rust
Message::DevicesDiscovered { devices } => {
    let device_count = devices.len();

    // Update global cache
    state.set_device_cache(devices.clone());

    // Update dialog state if dialog is open
    if state.ui_mode == UiMode::Startup || state.ui_mode == UiMode::NewSessionDialog {
        // Only update if dialog is showing (not closed during refresh)
        let target_selector = &mut state.new_session_dialog_state.target_selector;

        // Preserve selection if possible
        let previous_selection = target_selector.selected_device_id();

        target_selector.set_connected_devices(devices);

        // Restore selection if device still exists
        if let Some(device_id) = previous_selection {
            target_selector.select_device_by_id(&device_id);
        }
    }

    info!("Devices updated: {} devices", device_count);
    UpdateResult::none()
}
```

### Step 5: Add Helper to Preserve Selection

**File:** `src/tui/widgets/new_session_dialog/target_selector.rs`

Add methods to get/restore selection:

```rust
impl TargetSelectorState {
    /// Get the currently selected device ID (if any)
    pub fn selected_device_id(&self) -> Option<String> {
        let items = match self.active_tab {
            TargetTab::Connected => self.get_flat_connected_list(),
            TargetTab::Bootable => return None, // Bootable doesn't need this
        };

        items.get(self.selected_index).and_then(|item| {
            match item {
                DeviceListItem::Device(d) => Some(d.id.clone()),
                _ => None,
            }
        })
    }

    /// Select device by ID if it exists in the list
    pub fn select_device_by_id(&mut self, device_id: &str) -> bool {
        let items = self.get_flat_connected_list();
        for (index, item) in items.iter().enumerate() {
            if let DeviceListItem::Device(d) = item {
                if d.id == device_id {
                    self.selected_index = index;
                    return true;
                }
            }
        }
        false
    }
}
```

## Files to Modify

| File | Changes |
|------|---------|
| `src/app/handler/new_session/navigation.rs` | Check cache, use cached devices, trigger background refresh |
| `src/app/handler/mod.rs` | Add `RefreshDevicesBackground` action |
| `src/tui/actions.rs` | Handle background refresh action |
| `src/app/handler/update.rs` | Preserve selection on device update |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Add selection preservation helpers |

## Acceptance Criteria

1. Dialog opens instantly with cached devices (< 100ms for cache hit)
2. Loading spinner NOT shown when cache is fresh
3. Background refresh updates list silently
4. Cache expires after 30 seconds (existing TTL)
5. Selection preserved when devices refresh
6. Loading spinner shown only on cache miss
7. `cargo check` passes

## Testing

```bash
cargo check
cargo test cache
cargo test device
```

**Manual Testing:**
1. Open dialog (first time) → See loading spinner, wait for devices
2. Close dialog, wait 5 seconds
3. Open dialog again → Devices appear instantly (no spinner)
4. Wait 35 seconds (cache expires)
5. Open dialog → See loading spinner again

Add unit tests:

```rust
#[test]
fn test_cached_devices_used_on_dialog_open() {
    let mut state = test_app_state();

    // Pre-populate cache
    state.set_device_cache(vec![test_device(1), test_device(2)]);

    // Open dialog
    let result = handle_open_new_session_dialog(&mut state);

    // Should have devices immediately
    assert_eq!(state.new_session_dialog_state.target_selector.connected_devices.len(), 2);

    // Should trigger background refresh, not foreground discovery
    assert!(matches!(result.action, Some(UpdateAction::RefreshDevicesBackground)));

    // Should NOT show loading
    assert!(!state.new_session_dialog_state.target_selector.loading);
}

#[test]
fn test_cache_miss_shows_loading() {
    let mut state = test_app_state();
    // No cache set

    let result = handle_open_new_session_dialog(&mut state);

    // Should show loading
    assert!(state.new_session_dialog_state.target_selector.loading);

    // Should trigger foreground discovery
    assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
}
```

## Notes

- The 30-second TTL is defined in `get_cached_devices()` - can be adjusted if needed
- Background refresh errors are logged but not shown to user
- Selection preservation prevents jarring UX when devices refresh

---

## Completion Summary

**Status:** Not Started
