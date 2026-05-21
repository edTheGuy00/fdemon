//! # Performance Extension Toggles
//!
//! Boolean service extensions controlling the Flutter performance profilers.
//! Currently: widget-rebuild profiling (`ext.flutter.profileWidgetBuilds`).

use fdemon_core::prelude::*;

use super::ext;
use super::overlays::toggle_bool_extension;
use super::VmServiceClient;

/// Toggle (or query) the `ext.flutter.profileWidgetBuilds` extension.
///
/// * `enabled = Some(true)` — enable rebuild tracking.
/// * `enabled = Some(false)` — disable.
/// * `enabled = None` — query current state without changing it.
///
/// Returns the new (or current) state. The extension's effect is to emit
/// `Flutter.RebuiltWidgets` Extension events for each frame; the subscription
/// lives on the already-active `Extension` stream.
///
/// Available in debug mode only — returns `Err` in profile/release builds.
///
/// # Errors
///
/// - [`Error::Protocol`] if the VM Service returned a JSON-RPC error (e.g.,
///   the extension is not available in profile/release mode).
/// - [`Error::ChannelClosed`] if the VM Service client connection is closed.
pub async fn set_profile_widget_builds(
    client: &VmServiceClient,
    isolate_id: &str,
    enabled: Option<bool>,
) -> Result<bool> {
    toggle_bool_extension(client, ext::PROFILE_WIDGET_BUILDS, isolate_id, enabled).await
}

/// Convenience wrapper: query the current `ext.flutter.profileWidgetBuilds` state.
///
/// Equivalent to calling `set_profile_widget_builds(client, isolate_id, None)`.
///
/// Returns `true` if widget-rebuild profiling is currently enabled.
///
/// # Errors
///
/// - [`Error::Protocol`] if the VM Service returned a JSON-RPC error.
/// - [`Error::ChannelClosed`] if the VM Service client connection is closed.
pub async fn get_profile_widget_builds(client: &VmServiceClient, isolate_id: &str) -> Result<bool> {
    set_profile_widget_builds(client, isolate_id, None).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::ext;

    // Note: end-to-end round-trip coverage for `set_profile_widget_builds` is
    // provided by `toggle_bool_extension`'s tests in the parent module
    // (`extensions/overlays.rs`).  The tests below verify only the invariants
    // that are specific to this module.

    // Verify that the extension method name constant used by the public
    // functions is exactly what the Flutter engine expects.
    #[test]
    fn set_profile_widget_builds_uses_correct_extension_name() {
        assert_eq!(
            ext::PROFILE_WIDGET_BUILDS,
            "ext.flutter.profileWidgetBuilds"
        );
    }

    // Round-trip: parse_bool_extension_response parses {"enabled": "true"} → true.
    // This mirrors what toggle_bool_extension (and therefore set_profile_widget_builds)
    // does with the response from the VM Service.
    #[test]
    fn set_profile_widget_builds_round_trips_enabled_true() {
        use super::super::parse_bool_extension_response;
        use serde_json::json;

        let mock_response = json!({"enabled": "true"});
        let result = parse_bool_extension_response(&mock_response).unwrap();
        assert!(
            result,
            "parse of {{\"enabled\":\"true\"}} should return true"
        );
    }
}
