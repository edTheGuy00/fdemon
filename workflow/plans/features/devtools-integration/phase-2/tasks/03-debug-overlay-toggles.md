## Task: Debug Overlay Toggle Extensions

**Objective**: Implement typed wrappers for the 4 Flutter debug overlay toggle extensions (repaint rainbow, debug paint, performance overlay, widget inspector), with local state tracking for UI indicators.

**Depends on**: 01-extension-framework

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/extensions.rs`: Add toggle extension methods
- `crates/fdemon-daemon/src/vm_service/mod.rs`: Re-export new types

### Details

#### 1. Debug Overlay State

Track the current state of all debug overlays:

```rust
/// Current state of all Flutter debug overlay extensions.
#[derive(Debug, Clone, Default)]
pub struct DebugOverlayState {
    pub repaint_rainbow: Option<bool>,
    pub debug_paint: Option<bool>,
    pub performance_overlay: Option<bool>,
    pub widget_inspector: Option<bool>,
}
```

`Option<bool>` because the state is unknown until the first query. `None` = not yet queried or extension unavailable.

#### 2. Toggle Methods

Each overlay follows the same boolean extension pattern. All 4 use the `registerBoolServiceExtension` pattern where:
- **GET** (no `enabled` param): Returns current state
- **SET** (`enabled: "true"|"false"`): Sets new state and returns it

```rust
/// Toggle or query a boolean debug overlay extension.
///
/// If `enabled` is `Some`, sets the overlay to that state.
/// If `enabled` is `None`, queries the current state.
/// Returns the current state after the call.
pub async fn toggle_bool_extension(
    client: &VmServiceClient,
    method: &str,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    let args = enabled.map(|e| {
        let mut m = HashMap::new();
        m.insert("enabled".to_string(), e.to_string());
        m
    });
    let result = client.call_extension(method, isolate_id, args).await?;
    parse_bool_extension_response(&result)
}
```

Then expose specific typed methods:

```rust
/// Toggle or query the repaint rainbow overlay.
/// Debug mode only — returns Err in profile/release.
pub async fn repaint_rainbow(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::REPAINT_RAINBOW, isolate_id, enabled).await
}

/// Toggle or query the debug paint overlay.
/// Debug mode only — returns Err in profile/release.
pub async fn debug_paint(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::DEBUG_PAINT, isolate_id, enabled).await
}

/// Toggle or query the performance overlay on the device.
/// Available in debug and profile mode.
pub async fn performance_overlay(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::SHOW_PERFORMANCE_OVERLAY, isolate_id, enabled).await
}

/// Toggle or query the widget inspector overlay.
/// Debug mode only — returns Err in profile/release.
pub async fn widget_inspector(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::INSPECTOR_SHOW, isolate_id, enabled).await
}
```

#### 3. Bulk Query

Query all overlay states at once (useful for initial sync on connection):

```rust
/// Query the current state of all debug overlays.
/// Individual failures are captured as None (extension unavailable).
pub async fn query_all_overlays(
    client: &VmServiceClient,
    isolate_id: &str,
) -> DebugOverlayState {
    DebugOverlayState {
        repaint_rainbow: repaint_rainbow(client, isolate_id, None).await.ok(),
        debug_paint: debug_paint(client, isolate_id, None).await.ok(),
        performance_overlay: performance_overlay(client, isolate_id, None).await.ok(),
        widget_inspector: widget_inspector(client, isolate_id, None).await.ok(),
    }
}
```

#### 4. Convenience Toggle

A "flip" method that reads current state and sets the opposite:

```rust
/// Toggle an overlay to the opposite of its current state.
/// Returns the new state.
pub async fn flip_overlay(
    client: &VmServiceClient,
    method: &str,
    isolate_id: &str,
) -> Result<bool> {
    let current = toggle_bool_extension(client, method, isolate_id, None).await?;
    toggle_bool_extension(client, method, isolate_id, Some(!current)).await
}
```

### Acceptance Criteria

1. All 4 toggle extensions callable with typed API
2. GET (query) mode works — returns current state without changing it
3. SET mode works — sets new state and returns it
4. `query_all_overlays()` returns state for all overlays, with `None` for unavailable ones
5. `flip_overlay()` correctly reads-then-inverts
6. `DebugOverlayState` struct tracks all overlay states
7. Extension-not-available errors handled gracefully (return `Err`, not panic)
8. All new functions and types re-exported

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_overlay_state_default() {
        let state = DebugOverlayState::default();
        assert_eq!(state.repaint_rainbow, None);
        assert_eq!(state.debug_paint, None);
        assert_eq!(state.performance_overlay, None);
        assert_eq!(state.widget_inspector, None);
    }

    #[test]
    fn test_parse_bool_response_enabled_true() {
        let json = json!({
            "type": "_extensionType",
            "method": "ext.flutter.repaintRainbow",
            "enabled": "true"
        });
        assert!(parse_bool_extension_response(&json).unwrap());
    }

    #[test]
    fn test_parse_bool_response_enabled_false() {
        let json = json!({
            "type": "_extensionType",
            "method": "ext.flutter.repaintRainbow",
            "enabled": "false"
        });
        assert!(!parse_bool_extension_response(&json).unwrap());
    }

    #[test]
    fn test_parse_bool_response_missing_enabled() {
        let json = json!({"type": "_extensionType"});
        assert!(parse_bool_extension_response(&json).is_err());
    }

    #[test]
    fn test_parse_bool_response_wrong_type() {
        // VM Service returns strings, not JSON booleans
        let json = json!({"enabled": true});
        // Should still work if the value is a JSON boolean (defensive)
        // OR should fail — depends on implementation choice.
        // Recommend handling both string and bool for robustness.
    }
}
```

### Notes

- **All values are strings at the wire level.** The VM Service protocol requires `"true"` / `"false"` strings, not JSON booleans. The response also contains string `"enabled"` values. Be defensive and handle both string and JSON boolean in the parser.
- **Performance overlay is available in profile mode** while the other 3 are debug-only. The typed wrappers don't enforce this — callers handle the error.
- **No TEA integration in this task.** The `Message` variants, `UpdateAction` variants, and keybindings for triggering these toggles from the UI belong to Phase 4. This task only provides the callable API.
- The `flip_overlay()` method makes two RPC calls (read + write). For a single toggle operation from a keybinding, this is fine. For rapid toggling, consider caching.

---

## Completion Summary

**Status:** Not started
