## Task: Message Variants, UpdateAction Variants, and Update Wiring

**Objective**: Define all `Message` and `UpdateAction` variants needed for the Flutter Version panel, and wire them into `update.rs` with delegation stubs to handler functions (which Task 04 will implement).

**Depends on**: 01-state-types

### Scope

- `crates/fdemon-app/src/message.rs`: Add `FlutterVersion*` message variants
- `crates/fdemon-app/src/handler/mod.rs`: Add `UpdateAction` variants for SDK operations
- `crates/fdemon-app/src/handler/update.rs`: Wire all new message variants to handler delegation calls

### Details

#### 1. Message Variants (`message.rs`)

Add a new section block after the existing SDK messages (`SdkResolved`/`SdkResolutionFailed`):

```rust
pub enum Message {
    // ... existing variants ...

    // ── Flutter Version Panel ──

    /// Open the Flutter Version panel (V key in Normal mode)
    ShowFlutterVersion,

    /// Close the Flutter Version panel (Esc key)
    HideFlutterVersion,

    /// Priority-ordered escape: close panel → return to Normal
    FlutterVersionEscape,

    /// Switch pane focus (Tab key)
    FlutterVersionSwitchPane,

    /// Navigate up in the version list (k/Up)
    FlutterVersionUp,

    /// Navigate down in the version list (j/Down)
    FlutterVersionDown,

    /// Cache scan completed — populate version list
    FlutterVersionScanCompleted {
        versions: Vec<InstalledSdk>,
    },

    /// Cache scan failed
    FlutterVersionScanFailed {
        reason: String,
    },

    /// Switch to the selected version (Enter key)
    FlutterVersionSwitch,

    /// Version switch completed — SDK re-resolved
    FlutterVersionSwitchCompleted {
        version: String,
    },

    /// Version switch failed
    FlutterVersionSwitchFailed {
        reason: String,
    },

    /// Remove the selected version from cache (d key)
    FlutterVersionRemove,

    /// Version removal completed
    FlutterVersionRemoveCompleted {
        version: String,
    },

    /// Version removal failed
    FlutterVersionRemoveFailed {
        reason: String,
    },

    /// Install a new version (i key) — stub for Phase 3
    FlutterVersionInstall,

    /// Update the selected version (u key) — stub for Phase 3
    FlutterVersionUpdate,
}
```

**Note**: `InstalledSdk` must be imported from `fdemon_daemon::flutter_sdk`. Add to imports at the top of `message.rs`:
```rust
use fdemon_daemon::flutter_sdk::InstalledSdk;
```

#### 2. UpdateAction Variants (`handler/mod.rs`)

Add three new variants to the `UpdateAction` enum:

```rust
pub enum UpdateAction {
    // ... existing variants ...

    /// Scan the FVM cache for installed SDK versions.
    /// Triggered when the Flutter Version panel opens.
    ScanInstalledSdks {
        /// Root path of the currently active SDK (for `is_active` marking)
        active_sdk_root: Option<PathBuf>,
    },

    /// Switch the active Flutter SDK version.
    /// Writes `.fvmrc` in the project root and re-resolves the SDK.
    SwitchFlutterVersion {
        /// Version string to switch to (e.g., "3.19.0", "stable")
        version: String,
        /// Path to the selected SDK in the FVM cache
        sdk_path: PathBuf,
        /// Project root where `.fvmrc` will be written
        project_path: PathBuf,
        /// Explicit SDK path from settings (passed to re-resolution)
        explicit_sdk_path: Option<PathBuf>,
    },

    /// Remove an installed SDK version from the FVM cache.
    RemoveFlutterVersion {
        /// Version string being removed
        version: String,
        /// Path to the SDK directory to delete
        path: PathBuf,
        /// Root of the currently active SDK (to re-scan after removal)
        active_sdk_root: Option<PathBuf>,
    },
}
```

#### 3. Update Wiring (`update.rs`)

Add match arms for all new message variants. Each arm delegates to a handler function in the `flutter_version` handler module (Task 04). For now, use inline stubs that compile:

```rust
// ── Flutter Version Panel ──

Message::ShowFlutterVersion => {
    flutter_version::handle_show(state)
}

Message::HideFlutterVersion => {
    flutter_version::handle_hide(state)
}

Message::FlutterVersionEscape => {
    flutter_version::handle_escape(state)
}

Message::FlutterVersionSwitchPane => {
    flutter_version::handle_switch_pane(state)
}

Message::FlutterVersionUp => {
    flutter_version::handle_up(state)
}

Message::FlutterVersionDown => {
    flutter_version::handle_down(state)
}

Message::FlutterVersionScanCompleted { versions } => {
    flutter_version::handle_scan_completed(state, versions)
}

Message::FlutterVersionScanFailed { reason } => {
    flutter_version::handle_scan_failed(state, reason)
}

Message::FlutterVersionSwitch => {
    flutter_version::handle_switch(state)
}

Message::FlutterVersionSwitchCompleted { version } => {
    flutter_version::handle_switch_completed(state, version)
}

Message::FlutterVersionSwitchFailed { reason } => {
    flutter_version::handle_switch_failed(state, reason)
}

Message::FlutterVersionRemove => {
    flutter_version::handle_remove(state)
}

Message::FlutterVersionRemoveCompleted { version } => {
    flutter_version::handle_remove_completed(state, version)
}

Message::FlutterVersionRemoveFailed { reason } => {
    flutter_version::handle_remove_failed(state, reason)
}

Message::FlutterVersionInstall => {
    // Phase 3 stub
    state.flutter_version_state.status_message = Some("Install not yet available".into());
    UpdateResult::none()
}

Message::FlutterVersionUpdate => {
    // Phase 3 stub
    state.flutter_version_state.status_message = Some("Update not yet available".into());
    UpdateResult::none()
}
```

**Import the handler module** at the top of `update.rs`:
```rust
use super::flutter_version;
```

**Temporary stubs**: If Task 04 isn't complete yet, create a minimal `handler/flutter_version/mod.rs` with stub functions that return `UpdateResult::none()` so `update.rs` compiles. The stubs will be replaced by Task 04.

#### 4. Handler Module Stub (for compilation)

Create `handler/flutter_version/mod.rs` with minimal stubs:

```rust
//! Handler stubs for the Flutter Version panel.
//! Full implementation in Task 04.

use crate::handler::UpdateResult;
use crate::state::AppState;
use fdemon_daemon::flutter_sdk::InstalledSdk;

pub fn handle_show(state: &mut AppState) -> UpdateResult { UpdateResult::none() }
pub fn handle_hide(state: &mut AppState) -> UpdateResult { UpdateResult::none() }
pub fn handle_escape(state: &mut AppState) -> UpdateResult { UpdateResult::none() }
pub fn handle_switch_pane(state: &mut AppState) -> UpdateResult { UpdateResult::none() }
pub fn handle_up(state: &mut AppState) -> UpdateResult { UpdateResult::none() }
pub fn handle_down(state: &mut AppState) -> UpdateResult { UpdateResult::none() }
pub fn handle_scan_completed(state: &mut AppState, versions: Vec<InstalledSdk>) -> UpdateResult { UpdateResult::none() }
pub fn handle_scan_failed(state: &mut AppState, reason: String) -> UpdateResult { UpdateResult::none() }
pub fn handle_switch(state: &mut AppState) -> UpdateResult { UpdateResult::none() }
pub fn handle_switch_completed(state: &mut AppState, version: String) -> UpdateResult { UpdateResult::none() }
pub fn handle_switch_failed(state: &mut AppState, reason: String) -> UpdateResult { UpdateResult::none() }
pub fn handle_remove(state: &mut AppState) -> UpdateResult { UpdateResult::none() }
pub fn handle_remove_completed(state: &mut AppState, version: String) -> UpdateResult { UpdateResult::none() }
pub fn handle_remove_failed(state: &mut AppState, reason: String) -> UpdateResult { UpdateResult::none() }
```

Declare in `handler/mod.rs`:
```rust
pub mod flutter_version;
```

### Acceptance Criteria

1. All 18 `FlutterVersion*` message variants are added to `Message` enum
2. `ScanInstalledSdks`, `SwitchFlutterVersion`, `RemoveFlutterVersion` added to `UpdateAction`
3. All message variants are matched in `update()` — no unmatched arms
4. Handler delegation pattern follows existing code (e.g., `new_session::handle_*`)
5. `FlutterVersionInstall` and `FlutterVersionUpdate` are stubs with status messages
6. Handler module stub exists and compiles
7. `InstalledSdk` is properly imported from `fdemon_daemon`
8. `cargo check --workspace` compiles
9. `cargo test --workspace` passes
10. `cargo clippy --workspace -- -D warnings` passes

### Testing

The message variants and update wiring are tested indirectly through the handler tests (Task 04). For this task, verify compilation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flutter_version_messages_exist() {
        // Verify all variants can be constructed
        let _m1 = Message::ShowFlutterVersion;
        let _m2 = Message::HideFlutterVersion;
        let _m3 = Message::FlutterVersionEscape;
        let _m4 = Message::FlutterVersionSwitchPane;
        let _m5 = Message::FlutterVersionUp;
        let _m6 = Message::FlutterVersionDown;
        let _m7 = Message::FlutterVersionScanCompleted { versions: vec![] };
        let _m8 = Message::FlutterVersionScanFailed { reason: "test".into() };
        let _m9 = Message::FlutterVersionSwitch;
        let _m10 = Message::FlutterVersionSwitchCompleted { version: "3.19.0".into() };
        let _m11 = Message::FlutterVersionSwitchFailed { reason: "test".into() };
        let _m12 = Message::FlutterVersionRemove;
        let _m13 = Message::FlutterVersionRemoveCompleted { version: "3.19.0".into() };
        let _m14 = Message::FlutterVersionRemoveFailed { reason: "test".into() };
        let _m15 = Message::FlutterVersionInstall;
        let _m16 = Message::FlutterVersionUpdate;
    }
}
```

### Notes

- **`InstalledSdk` must be `Clone + Debug`** to be used in `Message` variants. Task 02 defines it with `#[derive(Debug, Clone)]`.
- **The handler stubs are intentionally minimal** — just enough to compile. Task 04 replaces them with real implementations.
- **`update.rs` is already ~2,400 lines.** Add the Flutter Version block as a contiguous section near the existing SDK messages (`SdkResolved`/`SdkResolutionFailed`) to keep related code together.
- **Naming convention**: All message variants use `FlutterVersion` prefix (matching `NewSessionDialog` prefix pattern). Handler functions use `handle_<action>` (matching `new_session::handle_*` pattern).

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/flutter_version/types.rs` | Removed placeholder `InstalledSdk` struct; replaced with `pub use fdemon_daemon::flutter_sdk::InstalledSdk;` re-export. Added `use std::path::PathBuf` inside `#[cfg(test)]` module. |
| `crates/fdemon-app/src/message.rs` | Added `use fdemon_daemon::flutter_sdk::InstalledSdk` import. Added 16 `FlutterVersion*` message variants after `SdkResolutionFailed`. |
| `crates/fdemon-app/src/handler/mod.rs` | Added `pub(crate) mod flutter_version;` declaration and updated doc comment. Added `ScanInstalledSdks`, `SwitchFlutterVersion`, `RemoveFlutterVersion` variants to `UpdateAction` enum. |
| `crates/fdemon-app/src/handler/flutter_version/mod.rs` | Created new file with 14 stub handler functions that return `UpdateResult::none()`. |
| `crates/fdemon-app/src/handler/update.rs` | Added `flutter_version` to imports. Added 16 match arms for all `FlutterVersion*` message variants delegating to `flutter_version::handle_*` functions. `FlutterVersionInstall` and `FlutterVersionUpdate` are inline Phase 3 stubs. |
| `crates/fdemon-app/src/actions/mod.rs` | Added no-op match arms for `ScanInstalledSdks`, `SwitchFlutterVersion`, `RemoveFlutterVersion` to prevent non-exhaustive pattern errors. |

### Notable Decisions/Tradeoffs

1. **`InstalledSdk` re-export in `types.rs`**: Rather than importing from `fdemon_daemon` in each consumer, the `types.rs` module re-exports it so the existing `use super::types::InstalledSdk` in `state.rs` continues to work without changes.
2. **`actions/mod.rs` no-ops**: The new `UpdateAction` variants required match arms in `actions/mod.rs` (the action dispatcher). Added explicit TODO-commented no-ops rather than a wildcard catch-all so that Task 04/05 cannot silently slip through unhandled.
3. **Stale compilation artifact**: Running `cargo clean -p fdemon-tui` was required after a git stash/unstash cycle to clear stale artifacts that caused false positive `fdemon_daemon` reference errors in the TUI crate. The workspace was clean after re-compilation.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (1725 tests)
- `cargo test --workspace` - Passed (all crates)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed
- `cargo clippy --workspace -- -D warnings` - Passed
- `cargo fmt --all` - Applied (no format changes needed)

### Risks/Limitations

1. **Handler stubs are no-ops**: All `FlutterVersion*` message handlers return `UpdateResult::none()`. The panel will not function until Task 04 provides real implementations.
2. **Action dispatcher no-ops**: `ScanInstalledSdks`, `SwitchFlutterVersion`, and `RemoveFlutterVersion` actions are registered but produce no side effects until Task 04/05 implementations replace the TODO stubs.
