//! Debug overlay toggle extensions.
//!
//! Provides [`DebugOverlayState`], [`toggle_bool_extension`], and helpers
//! for querying and flipping Flutter debug overlay extensions.

use std::collections::HashMap;

use fdemon_core::prelude::*;

use super::ext;
use super::parse_bool_extension_response;
use super::VmServiceClient;

// ---------------------------------------------------------------------------
// Debug overlay state
// ---------------------------------------------------------------------------

/// Current state of all Flutter debug overlay extensions.
///
/// Each field is `Option<bool>` because the state is unknown until the first
/// query. `None` means the state has not yet been queried or the extension is
/// unavailable (e.g., profile/release build).
#[derive(Debug, Clone, Default)]
pub struct DebugOverlayState {
    /// Whether the repaint rainbow overlay is enabled.
    pub repaint_rainbow: Option<bool>,
    /// Whether the debug paint overlay is enabled.
    pub debug_paint: Option<bool>,
    /// Whether the performance overlay is enabled.
    pub performance_overlay: Option<bool>,
    /// Whether the widget inspector overlay is enabled.
    pub widget_inspector: Option<bool>,
}

// ---------------------------------------------------------------------------
// Toggle helpers
// ---------------------------------------------------------------------------

/// Toggle or query a boolean debug overlay extension.
///
/// If `enabled` is `Some`, sets the overlay to that state.
/// If `enabled` is `None`, queries the current state without changing it.
/// Returns the current state after the call.
///
/// # Errors
///
/// - [`Error::Protocol`] if the VM Service returned a JSON-RPC error (e.g.,
///   the extension is not available in profile/release mode).
/// - [`Error::ChannelClosed`] if the VM Service client connection is closed.
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

/// Toggle or query the repaint rainbow overlay.
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// If `enabled` is `Some`, sets the overlay to that state.
/// If `enabled` is `None`, queries the current state without changing it.
/// Returns the current state after the call.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn repaint_rainbow(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::REPAINT_RAINBOW, isolate_id, enabled).await
}

/// Toggle or query the debug paint overlay.
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// If `enabled` is `Some`, sets the overlay to that state.
/// If `enabled` is `None`, queries the current state without changing it.
/// Returns the current state after the call.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn debug_paint(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::DEBUG_PAINT, isolate_id, enabled).await
}

/// Toggle or query the performance overlay on the device.
///
/// Available in debug and profile mode.
///
/// If `enabled` is `Some`, sets the overlay to that state.
/// If `enabled` is `None`, queries the current state without changing it.
/// Returns the current state after the call.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn performance_overlay(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::SHOW_PERFORMANCE_OVERLAY, isolate_id, enabled).await
}

/// Toggle or query the widget inspector overlay.
///
/// Debug mode only — returns `Err` in profile/release builds.
///
/// If `enabled` is `Some`, sets the overlay to that state.
/// If `enabled` is `None`, queries the current state without changing it.
/// Returns the current state after the call.
///
/// # Errors
///
/// Returns an error if the extension is unavailable or the RPC call fails.
pub async fn widget_inspector(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::INSPECTOR_SHOW, isolate_id, enabled).await
}

// ---------------------------------------------------------------------------
// Bulk query
// ---------------------------------------------------------------------------

/// Query all 4 overlay extensions sequentially and return their states.
///
/// Each overlay that is not available (e.g., in profile mode) is returned as `None`.
/// Errors from individual overlay queries are silently converted to `None` to
/// support partial results in mixed-mode builds.
///
/// # Returns
///
/// A [`DebugOverlayState`] where each field is `Some(bool)` if the extension
/// responded successfully, or `None` if the extension is unavailable or the
/// call failed.
pub async fn query_all_overlays(client: &VmServiceClient, isolate_id: &str) -> DebugOverlayState {
    DebugOverlayState {
        repaint_rainbow: repaint_rainbow(client, isolate_id, None).await.ok(),
        debug_paint: debug_paint(client, isolate_id, None).await.ok(),
        performance_overlay: performance_overlay(client, isolate_id, None).await.ok(),
        widget_inspector: widget_inspector(client, isolate_id, None).await.ok(),
    }
}

// ---------------------------------------------------------------------------
// Convenience flip
// ---------------------------------------------------------------------------

/// Toggle an overlay to the opposite of its current state.
///
/// Makes two RPC calls: one to read the current state, one to set the
/// opposite. Returns the new state after the flip.
///
/// # Errors
///
/// - Returns an error if the read call fails (e.g., extension unavailable).
/// - Returns an error if the write call fails.
pub async fn flip_overlay(
    client: &VmServiceClient,
    method: &str,
    isolate_id: &str,
) -> Result<bool> {
    let current = toggle_bool_extension(client, method, isolate_id, None).await?;
    toggle_bool_extension(client, method, isolate_id, Some(!current)).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── DebugOverlayState ───────────────────────────────────────────────────

    #[test]
    fn test_debug_overlay_state_default_all_none() {
        let state = DebugOverlayState::default();
        assert_eq!(state.repaint_rainbow, None);
        assert_eq!(state.debug_paint, None);
        assert_eq!(state.performance_overlay, None);
        assert_eq!(state.widget_inspector, None);
    }

    #[test]
    fn test_debug_overlay_state_clone() {
        let state = DebugOverlayState {
            repaint_rainbow: Some(true),
            debug_paint: Some(false),
            performance_overlay: None,
            widget_inspector: Some(true),
        };
        let cloned = state.clone();
        assert_eq!(cloned.repaint_rainbow, Some(true));
        assert_eq!(cloned.debug_paint, Some(false));
        assert_eq!(cloned.performance_overlay, None);
        assert_eq!(cloned.widget_inspector, Some(true));
    }

    #[test]
    fn test_debug_overlay_state_partial_update() {
        let mut state = DebugOverlayState::default();
        state.repaint_rainbow = Some(true);
        assert_eq!(state.repaint_rainbow, Some(true));
        // Other fields should remain None.
        assert_eq!(state.debug_paint, None);
        assert_eq!(state.performance_overlay, None);
        assert_eq!(state.widget_inspector, None);
    }
}
