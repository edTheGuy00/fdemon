## Task: Flutter Version State Types and Module

**Objective**: Create the `flutter_version/` module in `fdemon-app` with all state structs and types needed for the Flutter Version panel, add `UiMode::FlutterVersion`, and wire the state into `AppState`.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/flutter_version/mod.rs`: **NEW** Module root with re-exports
- `crates/fdemon-app/src/flutter_version/state.rs`: **NEW** `FlutterVersionState` and sub-states
- `crates/fdemon-app/src/flutter_version/types.rs`: **NEW** `FlutterVersionPane`, panel-specific enums
- `crates/fdemon-app/src/state.rs`: Add `UiMode::FlutterVersion` variant, `flutter_version_state` field, `show_flutter_version()` / `hide_flutter_version()` helpers
- `crates/fdemon-app/src/lib.rs`: Declare `flutter_version` module

### Details

#### 1. Module Structure

```
crates/fdemon-app/src/flutter_version/
├── mod.rs          — pub use re-exports
├── state.rs        — FlutterVersionState, SdkInfoState, VersionListState
└── types.rs        — FlutterVersionPane enum
```

This mirrors `new_session_dialog/` which has `mod.rs`, `state.rs`, `types.rs` (plus additional files for its more complex sub-states).

#### 2. State Definitions (`state.rs`)

```rust
use std::cell::Cell;
use std::path::PathBuf;
use fdemon_daemon::FlutterSdk;

/// Top-level state for the Flutter Version panel.
/// Follows the NewSessionDialogState pattern.
pub struct FlutterVersionState {
    /// Left pane — current SDK details
    pub sdk_info: SdkInfoState,
    /// Right pane — installed versions from FVM cache
    pub version_list: VersionListState,
    /// Which pane has keyboard focus
    pub focused_pane: FlutterVersionPane,
    /// Whether the panel is visible
    pub visible: bool,
    /// Status message shown at bottom (e.g., "Switched to 3.19.0")
    pub status_message: Option<String>,
}

/// Read-only display of the currently resolved SDK.
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
    /// Follows the Cell<usize> render-hint pattern (see docs/CODE_STANDARDS.md Principle 3).
    pub last_known_visible_height: Cell<usize>,
}
```

**Note**: `InstalledSdk` is defined in `fdemon-daemon` (Task 02) and re-exported. For this task, use a temporary placeholder or import from `fdemon_daemon::flutter_sdk::InstalledSdk`. If Task 02 isn't complete yet, define a minimal placeholder struct in `types.rs` with a `TODO` comment and update the import when Task 02 lands.

#### 3. Type Definitions (`types.rs`)

```rust
/// Which pane has keyboard focus in the Flutter Version panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlutterVersionPane {
    /// Left pane: current SDK info (read-only)
    #[default]
    SdkInfo,
    /// Right pane: installed versions list
    VersionList,
}
```

#### 4. Module Root (`mod.rs`)

```rust
//! # Flutter Version Panel State
//!
//! State and types for the Flutter Version panel (opened with `V`),
//! which displays the current SDK info and installed versions.

mod state;
mod types;

pub use state::*;
pub use types::*;
```

#### 5. `UiMode` Addition (`state.rs` in fdemon-app root)

Add `FlutterVersion` to the `UiMode` enum (after `Settings`, before `DevTools`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
    // ... existing variants ...
    Settings,
    FlutterVersion,  // NEW
    DevTools,
}
```

#### 6. `AppState` Integration (`state.rs`)

Add field to `AppState`:

```rust
pub struct AppState {
    // ... existing fields ...

    /// State for the Flutter Version panel overlay.
    pub flutter_version_state: FlutterVersionState,
}
```

Initialize in `AppState::new()` / `AppState::with_settings()` using `FlutterVersionState::default()`.

Add helper methods following the `show_new_session_dialog()` / `hide_new_session_dialog()` pattern:

```rust
impl AppState {
    /// Opens the Flutter Version panel, snapshotting the current SDK state.
    pub fn show_flutter_version(&mut self) {
        self.flutter_version_state = FlutterVersionState::new(self.resolved_sdk.clone());
        self.flutter_version_state.visible = true;
        self.ui_mode = UiMode::FlutterVersion;
    }

    /// Closes the Flutter Version panel, returning to Normal mode.
    pub fn hide_flutter_version(&mut self) {
        self.flutter_version_state.visible = false;
        self.ui_mode = UiMode::Normal;
    }
}
```

The `FlutterVersionState::new()` constructor takes `Option<FlutterSdk>` and populates `SdkInfoState` with a snapshot. The Dart SDK version can be read from `<sdk_root>/bin/cache/dart-sdk/version` synchronously (small file read, same as Phase 1's `read_version_file()`).

### Acceptance Criteria

1. `flutter_version/` module exists with `mod.rs`, `state.rs`, `types.rs`
2. `FlutterVersionState`, `SdkInfoState`, `VersionListState` are defined with all fields
3. `FlutterVersionPane` enum has `SdkInfo` and `VersionList` variants
4. `UiMode::FlutterVersion` is added and all existing `match` arms on `UiMode` are updated with `_ =>` or explicit arms (compiler will enforce)
5. `AppState.flutter_version_state` is initialized to default
6. `show_flutter_version()` snapshots `resolved_sdk` and sets `UiMode::FlutterVersion`
7. `hide_flutter_version()` clears visibility and sets `UiMode::Normal`
8. `VersionListState` uses `Cell<usize>` for `last_known_visible_height` render-hint
9. `cargo check --workspace` compiles (all match arms handled)
10. `cargo test --workspace` passes
11. `cargo clippy --workspace -- -D warnings` passes

### Testing

```rust
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
        assert!(!state.version_list.loading);
    }

    #[test]
    fn test_show_flutter_version_sets_ui_mode() {
        let mut state = AppState::default();
        state.show_flutter_version();
        assert_eq!(state.ui_mode, UiMode::FlutterVersion);
        assert!(state.flutter_version_state.visible);
    }

    #[test]
    fn test_show_flutter_version_snapshots_sdk() {
        let mut state = AppState::default();
        // Inject a fake SDK
        state.resolved_sdk = Some(fake_flutter_sdk());
        state.show_flutter_version();
        assert!(state.flutter_version_state.sdk_info.resolved_sdk.is_some());
    }

    #[test]
    fn test_hide_flutter_version_returns_to_normal() {
        let mut state = AppState::default();
        state.show_flutter_version();
        state.hide_flutter_version();
        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(!state.flutter_version_state.visible);
    }

    #[test]
    fn test_flutter_version_pane_default() {
        assert_eq!(FlutterVersionPane::default(), FlutterVersionPane::SdkInfo);
    }

    #[test]
    fn test_version_list_state_render_hint_default() {
        let state = VersionListState::default();
        assert_eq!(state.last_known_visible_height.get(), 0);
    }
}
```

### Notes

- **`UiMode` is `Copy`** — `FlutterVersion` needs no associated data (same as all other variants).
- **All exhaustive `match` arms on `UiMode` must be updated.** The compiler will flag every match in `keys.rs`, `update.rs`, and `render/mod.rs`. For now, add `UiMode::FlutterVersion => { /* TODO: Phase 2 */ }` stubs or combine with a `_` arm if one already exists. Tasks 05 and 07 will fill these in.
- **`FlutterVersionState::new()` reads `dart-sdk/version` synchronously.** This is a single small file read (< 100 bytes). If the file doesn't exist, `dart_version` is `None`.
- **`InstalledSdk` import**: If Task 02 isn't done yet, temporarily define `InstalledSdk` locally in `types.rs` with the fields from the PLAN (version, channel, path, is_active). Add a `// TODO: Replace with fdemon_daemon::flutter_sdk::InstalledSdk when Task 02 lands` comment.
