//! Core types for the Flutter Version panel.

// Re-export InstalledSdk from fdemon_daemon so the rest of the crate can use it
// without reaching through to the daemon crate directly.
pub use fdemon_daemon::flutter_sdk::InstalledSdk;

/// Which pane has keyboard focus in the Flutter Version panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlutterVersionPane {
    /// Left pane: current SDK info (read-only)
    #[default]
    SdkInfo,
    /// Right pane: installed versions list
    VersionList,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_flutter_version_pane_default() {
        assert_eq!(FlutterVersionPane::default(), FlutterVersionPane::SdkInfo);
    }

    #[test]
    fn test_flutter_version_pane_variants_are_distinct() {
        assert_ne!(FlutterVersionPane::SdkInfo, FlutterVersionPane::VersionList);
    }

    #[test]
    fn test_installed_sdk_fields() {
        let sdk = InstalledSdk {
            version: "3.19.0".to_string(),
            channel: Some("stable".to_string()),
            path: PathBuf::from("/usr/local/flutter"),
            is_active: true,
        };
        assert_eq!(sdk.version, "3.19.0");
        assert_eq!(sdk.channel.as_deref(), Some("stable"));
        assert!(sdk.is_active);
    }
}
