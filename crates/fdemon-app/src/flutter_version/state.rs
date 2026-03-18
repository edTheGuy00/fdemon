//! State types for the Flutter Version panel.

use std::cell::Cell;
use std::path::Path;

use fdemon_daemon::FlutterSdk;

use super::types::{FlutterVersionPane, InstalledSdk};

/// Top-level state for the Flutter Version panel.
///
/// Follows the `NewSessionDialogState` pattern: owned by `AppState`,
/// initialized via `FlutterVersionState::new()` when the panel is opened,
/// and reset to `default()` when not in use.
#[derive(Debug, Default)]
pub struct FlutterVersionState {
    /// Left pane — current SDK details (read-only snapshot)
    pub sdk_info: SdkInfoState,
    /// Right pane — installed versions from FVM cache
    pub version_list: VersionListState,
    /// Which pane has keyboard focus
    pub focused_pane: FlutterVersionPane,
    /// Whether the panel is visible
    pub visible: bool,
    /// Status message shown at bottom (e.g., "Switched to 3.19.0")
    pub status_message: Option<String>,
    /// Index of version pending deletion (double-press `d` confirmation).
    /// Set on first `d` press, cleared on second `d` press or any other action.
    pub pending_delete: Option<usize>,
}

impl FlutterVersionState {
    /// Create a new `FlutterVersionState` by snapshotting the currently resolved SDK.
    ///
    /// Reads the Dart SDK version synchronously from
    /// `<sdk_root>/bin/cache/dart-sdk/version` (small file, < 100 bytes).
    /// If the file does not exist or cannot be read, `dart_version` is `None`.
    pub fn new(resolved_sdk: Option<FlutterSdk>) -> Self {
        let dart_version = resolved_sdk
            .as_ref()
            .and_then(|sdk| read_dart_version(&sdk.root));

        Self {
            sdk_info: SdkInfoState {
                resolved_sdk,
                dart_version,
            },
            version_list: VersionListState::default(),
            focused_pane: FlutterVersionPane::default(),
            visible: false,
            status_message: None,
            pending_delete: None,
        }
    }
}

/// Read the Dart SDK version from `<sdk_root>/bin/cache/dart-sdk/version`.
///
/// Returns `None` if the file does not exist or cannot be read.
/// This is a small synchronous file read (< 100 bytes) and is safe to call
/// from the message handler during panel open.
pub(crate) fn read_dart_version(sdk_root: &Path) -> Option<String> {
    let version_path = sdk_root
        .join("bin")
        .join("cache")
        .join("dart-sdk")
        .join("version");
    std::fs::read_to_string(&version_path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Read-only display of the currently resolved SDK.
#[derive(Debug, Default)]
pub struct SdkInfoState {
    /// Snapshot of the resolved SDK at panel open time.
    /// `None` when no SDK was detected.
    pub resolved_sdk: Option<FlutterSdk>,
    /// Dart SDK version (read from `<sdk>/bin/cache/dart-sdk/version`)
    pub dart_version: Option<String>,
}

/// Scrollable list of installed SDK versions.
pub struct VersionListState {
    /// Installed versions scanned from the FVM cache.
    pub installed_versions: Vec<InstalledSdk>,
    /// Currently selected index in the list.
    pub selected_index: usize,
    /// Scroll offset for the visible window.
    pub scroll_offset: usize,
    /// Whether a cache scan is in progress.
    pub loading: bool,
    /// Error message from a failed scan.
    pub error: Option<String>,
    /// Render-hint: actual visible height from the last rendered frame.
    /// Follows the `Cell<usize>` render-hint pattern (see docs/CODE_STANDARDS.md Principle 3).
    /// Defaults to 0, which signals "not yet rendered — use fallback".
    /// Written by the renderer; not mutated by message handlers.
    pub last_known_visible_height: Cell<usize>,
}

impl Default for VersionListState {
    fn default() -> Self {
        Self {
            installed_versions: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            loading: true, // Start in loading state — scan is always triggered on panel open
            error: None,
            last_known_visible_height: Cell::new(0),
        }
    }
}

impl std::fmt::Debug for VersionListState {
    /// Manual Debug impl so `last_known_visible_height` shows its current value
    /// rather than the internal `Cell` representation.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VersionListState")
            .field("installed_versions", &self.installed_versions)
            .field("selected_index", &self.selected_index)
            .field("scroll_offset", &self.scroll_offset)
            .field("loading", &self.loading)
            .field("error", &self.error)
            .field(
                "last_known_visible_height",
                &self.last_known_visible_height.get(),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flutter_version_state_default() {
        let state = FlutterVersionState::default();
        assert!(!state.visible);
        assert_eq!(state.focused_pane, FlutterVersionPane::SdkInfo);
        assert!(state.version_list.installed_versions.is_empty());
        assert_eq!(state.version_list.selected_index, 0);
        assert!(state.version_list.loading); // starts in loading state
        assert!(state.status_message.is_none());
        assert!(state.pending_delete.is_none());
    }

    #[test]
    fn test_flutter_version_state_new_with_no_sdk() {
        let state = FlutterVersionState::new(None);
        assert!(!state.visible);
        assert!(state.sdk_info.resolved_sdk.is_none());
        assert!(state.sdk_info.dart_version.is_none());
    }

    #[test]
    fn test_version_list_state_render_hint_default() {
        let state = VersionListState::default();
        assert_eq!(state.last_known_visible_height.get(), 0);
    }

    #[test]
    fn test_version_list_state_render_hint_can_be_set() {
        let state = VersionListState::default();
        // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md
        state.last_known_visible_height.set(42);
        assert_eq!(state.last_known_visible_height.get(), 42);
    }

    #[test]
    fn test_sdk_info_state_default_has_no_sdk() {
        let state = SdkInfoState::default();
        assert!(state.resolved_sdk.is_none());
        assert!(state.dart_version.is_none());
    }

    #[test]
    fn test_read_dart_version_missing_path_returns_none() {
        let path = std::path::Path::new("/nonexistent/sdk/root");
        assert!(read_dart_version(path).is_none());
    }

    #[test]
    fn test_read_dart_version_reads_version_file() {
        use std::fs;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let dart_sdk = tmp.path().join("bin").join("cache").join("dart-sdk");
        fs::create_dir_all(&dart_sdk).unwrap();
        fs::write(dart_sdk.join("version"), "3.3.0\n").unwrap();

        let result = read_dart_version(tmp.path());
        assert_eq!(result, Some("3.3.0".to_string()));
    }

    #[test]
    fn test_read_dart_version_empty_file_returns_none() {
        use std::fs;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let dart_sdk = tmp.path().join("bin").join("cache").join("dart-sdk");
        fs::create_dir_all(&dart_sdk).unwrap();
        fs::write(dart_sdk.join("version"), "   \n").unwrap();

        // Empty/whitespace-only content should return None
        let result = read_dart_version(tmp.path());
        assert!(result.is_none());
    }
}
