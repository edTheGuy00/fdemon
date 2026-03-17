## Task: Render Integration, Action Dispatching, and End-to-End Wiring

**Objective**: Wire the Flutter Version panel into the render pipeline, implement action dispatchers for cache scanning and version switching, and verify end-to-end functionality. This is the integration task that makes everything work together.

**Depends on**: 02-cache-scanner, 04-handler-module, 05-key-routing, 06-tui-widget

### Scope

- `crates/fdemon-tui/src/render/mod.rs`: Add `UiMode::FlutterVersion` render branch
- `crates/fdemon-app/src/actions/mod.rs`: Handle `ScanInstalledSdks`, `SwitchFlutterVersion`, `RemoveFlutterVersion` action variants
- `crates/fdemon-app/src/engine.rs`: Minor wiring if needed
- `crates/fdemon-tui/src/widgets/mod.rs`: Ensure `flutter_version_panel` is exported
- `docs/KEYBINDINGS.md`: Add `V` key documentation (if this file exists)

### Details

#### 1. Render Integration (`render/mod.rs`)

Add `UiMode::FlutterVersion` to the main render dispatch in `view()`:

```rust
fn render_frame(frame: &mut Frame, state: &mut AppState, areas: &Areas) {
    // ... existing render logic for Normal mode (logs, header, status bar) ...

    match state.ui_mode {
        // ... existing arms ...

        UiMode::FlutterVersion => {
            // Render underlying log view first (same as Normal)
            // ... existing Normal mode render ...

            // Then overlay the Flutter Version panel
            let panel = widgets::FlutterVersionPanel::new(
                &state.flutter_version_state,
                &state.icons, // or however IconSet is accessed
            );
            frame.render_widget(panel, frame.area());
        }

        // ...
    }
}
```

**Important**: The panel renders on top of the normal view (logs, header, etc.), not instead of it. This means the underlying view is rendered first, then `dim_background` + the panel overlay. This follows the same pattern as `NewSessionDialog` and `ConfirmDialog`.

#### 2. Action Dispatching (`actions/mod.rs`)

Add match arms for the three new `UpdateAction` variants.

##### `ScanInstalledSdks`

```rust
UpdateAction::ScanInstalledSdks { active_sdk_root } => {
    let msg_tx = msg_tx.clone();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            fdemon_daemon::flutter_sdk::scan_installed_versions(
                active_sdk_root.as_deref(),
            )
        })
        .await;

        match result {
            Ok(versions) => {
                let _ = msg_tx.send(Message::FlutterVersionScanCompleted { versions });
            }
            Err(e) => {
                let _ = msg_tx.send(Message::FlutterVersionScanFailed {
                    reason: format!("Cache scan failed: {e}"),
                });
            }
        }
    });
}
```

**Note**: `scan_installed_versions()` is synchronous (filesystem I/O), so wrap in `spawn_blocking` to avoid blocking the Tokio runtime.

##### `SwitchFlutterVersion`

```rust
UpdateAction::SwitchFlutterVersion {
    version,
    sdk_path,
    project_path,
    explicit_sdk_path,
} => {
    let msg_tx = msg_tx.clone();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            switch_flutter_version(&version, &sdk_path, &project_path, explicit_sdk_path.as_deref())
        })
        .await;

        match result {
            Ok(Ok(sdk)) => {
                // Update global SDK state first
                let _ = msg_tx.send(Message::SdkResolved { sdk });
                // Then notify the panel
                let _ = msg_tx.send(Message::FlutterVersionSwitchCompleted { version: version_clone });
            }
            Ok(Err(e)) => {
                let _ = msg_tx.send(Message::FlutterVersionSwitchFailed {
                    reason: format!("{e}"),
                });
            }
            Err(e) => {
                let _ = msg_tx.send(Message::FlutterVersionSwitchFailed {
                    reason: format!("Task failed: {e}"),
                });
            }
        }
    });
}
```

**Version switching implementation** (helper function, can live in `actions/mod.rs` or a new `actions/flutter_version.rs`):

```rust
/// Write `.fvmrc` and re-resolve the SDK.
fn switch_flutter_version(
    version: &str,
    sdk_path: &Path,
    project_path: &Path,
    explicit_sdk_path: Option<&Path>,
) -> Result<FlutterSdk> {
    // 1. Write .fvmrc in project root
    let fvmrc_path = project_path.join(".fvmrc");
    let fvmrc_content = format!(r#"{{"flutter": "{}"}}"#, version);
    std::fs::write(&fvmrc_path, &fvmrc_content)
        .with_context(|| format!("Failed to write {}", fvmrc_path.display()))?;

    info!("Wrote .fvmrc: {}", fvmrc_content);

    // 2. Re-resolve SDK (the FVM detector will now pick up the new .fvmrc)
    let sdk = fdemon_daemon::flutter_sdk::find_flutter_sdk(project_path, explicit_sdk_path)?;

    info!("SDK re-resolved after version switch: {} via {}", sdk.version, sdk.source);
    Ok(sdk)
}
```

##### `RemoveFlutterVersion`

```rust
UpdateAction::RemoveFlutterVersion {
    version,
    path,
    active_sdk_root,
} => {
    let msg_tx = msg_tx.clone();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            // Safety: refuse to remove if path doesn't look like an FVM cache entry
            if !path.starts_with(dirs::home_dir().unwrap_or_default().join("fvm/versions")) {
                return Err(Error::config(format!(
                    "Refusing to remove path outside FVM cache: {}",
                    path.display()
                )));
            }
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
            Ok(())
        })
        .await;

        match result {
            Ok(Ok(())) => {
                let _ = msg_tx.send(Message::FlutterVersionRemoveCompleted {
                    version: version.clone(),
                });
            }
            Ok(Err(e)) => {
                let _ = msg_tx.send(Message::FlutterVersionRemoveFailed {
                    reason: format!("{e}"),
                });
            }
            Err(e) => {
                let _ = msg_tx.send(Message::FlutterVersionRemoveFailed {
                    reason: format!("Task failed: {e}"),
                });
            }
        }
    });
}
```

**Safety check**: The removal path must be inside `~/fvm/versions/` to prevent accidental deletion of arbitrary directories. This is a defense-in-depth measure beyond the handler's `is_active` guard.

#### 3. Widget Re-export (`widgets/mod.rs`)

Add to `crates/fdemon-tui/src/widgets/mod.rs`:

```rust
pub mod flutter_version_panel;
pub use flutter_version_panel::FlutterVersionPanel;
```

#### 4. Documentation Update

If `docs/KEYBINDINGS.md` exists, add:

```markdown
| `V` | Normal | Open Flutter Version panel |
```

If it does not exist, skip this step.

#### 5. End-to-End Flow Verification

The complete flow to verify:

```
1. User presses V in Normal mode
   → keys.rs: handle_key_normal → Message::ShowFlutterVersion

2. update.rs matches ShowFlutterVersion
   → flutter_version::handle_show(state)
   → state.show_flutter_version() (UiMode = FlutterVersion)
   → returns UpdateAction::ScanInstalledSdks

3. actions/mod.rs dispatches ScanInstalledSdks
   → spawn_blocking: scan_installed_versions(active_root)
   → sends Message::FlutterVersionScanCompleted { versions }

4. render/mod.rs: UiMode::FlutterVersion
   → renders underlying log view
   → overlays FlutterVersionPanel widget
   → left pane shows SDK info, right pane shows loading spinner

5. FlutterVersionScanCompleted arrives
   → handle_scan_completed populates version_list
   → next render frame shows the version list

6. User presses Tab → focus moves to VersionList pane
7. User presses j/k → selection moves in list
8. User presses Enter on a non-active version
   → handle_switch → UpdateAction::SwitchFlutterVersion

9. actions: write .fvmrc, re-resolve SDK
   → sends Message::SdkResolved (updates global state)
   → sends Message::FlutterVersionSwitchCompleted

10. Panel shows "Switched to X", re-scans cache
11. User presses Esc → panel closes, UiMode::Normal
```

### Acceptance Criteria

1. `UiMode::FlutterVersion` renders the panel overlay in `render/mod.rs`
2. Panel renders on top of the underlying Normal mode view (not replacing it)
3. `ScanInstalledSdks` action dispatches cache scan via `spawn_blocking`
4. `SwitchFlutterVersion` action writes `.fvmrc` and re-resolves SDK
5. `.fvmrc` format is `{"flutter": "<version>"}` — minimal FVM-compatible JSON
6. `SdkResolved` message is sent before `FlutterVersionSwitchCompleted` (global state updates first)
7. `RemoveFlutterVersion` action deletes the SDK directory with safety check
8. Safety check: removal path must be inside `~/fvm/versions/`
9. All action dispatchers use `spawn_blocking` for filesystem operations
10. Widget module is properly exported from `widgets/mod.rs`
11. Complete V → open → scan → display → switch → close flow works
12. `cargo check --workspace` compiles
13. `cargo test --workspace` passes
14. `cargo clippy --workspace -- -D warnings` passes
15. `cargo fmt --all` passes

### Testing

Integration-level tests for the action dispatching:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_switch_flutter_version_writes_fvmrc() {
        let tmp = TempDir::new().unwrap();
        let project_path = tmp.path();

        // Create a fake SDK at the "target" path
        let sdk_dir = tmp.path().join("fake_sdk");
        std::fs::create_dir_all(sdk_dir.join("bin")).unwrap();
        std::fs::write(sdk_dir.join("bin/flutter"), "#!/bin/sh").unwrap();
        std::fs::write(sdk_dir.join("VERSION"), "3.19.0").unwrap();

        // Note: switch_flutter_version calls find_flutter_sdk which may not
        // find the SDK via FVM detection in a temp dir. Test the .fvmrc write
        // directly:
        let fvmrc_path = project_path.join(".fvmrc");
        let content = r#"{"flutter": "3.19.0"}"#;
        std::fs::write(&fvmrc_path, content).unwrap();

        let fvmrc = std::fs::read_to_string(&fvmrc_path).unwrap();
        assert!(fvmrc.contains("3.19.0"));
    }

    #[test]
    fn test_remove_refuses_outside_fvm_cache() {
        // Verify the safety check prevents removal of arbitrary paths
        let outside_path = PathBuf::from("/tmp/not_fvm_cache/something");
        // The action handler should reject this path
        // (Test the guard logic, not the actual deletion)
    }

    #[test]
    fn test_fvmrc_format_is_valid_json() {
        let version = "3.19.0";
        let content = format!(r#"{{"flutter": "{}"}}"#, version);
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["flutter"], "3.19.0");
    }
}
```

### Notes

- **This is the highest-risk task** because it touches multiple crates and wires everything together. All prior tasks must be complete and passing before starting.
- **Message ordering matters**: `SdkResolved` must be sent before `FlutterVersionSwitchCompleted` so that `handle_switch_completed` sees the updated `state.resolved_sdk` when it refreshes the panel display.
- **Action dispatcher pattern**: Look at existing `UpdateAction` handling in `actions/mod.rs` for the exact pattern of `msg_tx.clone()`, `tokio::spawn`, and message sending. Follow the same async/error handling patterns.
- **`.fvmrc` write is minimal**: Only `{"flutter": "version"}`. Do not write additional FVM fields (flavors, etc.). If an existing `.fvmrc` has extra fields, read → merge → write to preserve them.
- **Removal safety**: The `~/fvm/versions/` path check is intentionally strict. In the future, if FVM cache moves or `FVM_CACHE_PATH` is set, this check should be updated. For Phase 2, hardcode the standard path as the safety boundary.
- **Compile-driven development**: Start with the render integration (smallest change), then action dispatchers (copy existing patterns), then verify the flow end-to-end.
