# Bugfix Plan: "Flutter SDK not found" Flash on Startup

## TL;DR

Regression from Phase 1 fixes: removing the bare PATH fallback in `find_flutter_sdk` (Task 02) causes `Err(FlutterNotFound)` on machines where the system PATH strategy can't fully resolve the SDK root (wrapper scripts, missing VERSION file). The error flashes in the new session dialog for 1-3 frames, then is silently cleared when `set_bootable_devices()` unconditionally zeros `target_selector.error`. Two fixes needed: restore a PATH-based fallback and make the error field resilient to unrelated side-effects.

## Bug Reports

### Bug 1: Bare PATH Fallback Removal Causes SDK Detection Failure

**Symptom:** On startup, the new session dialog momentarily shows "Flutter SDK not found. Configure sdk_path in .fdemon/config.toml or ensure flutter is on your PATH." — then it disappears, leaving an empty dialog.

**Expected:** The dialog should show a loading spinner then populate with discovered devices, with no error flash.

**Root Cause Analysis:**

1. Phase 1 fixes Task 02 removed the `try_system_path_bare()` fallback from `find_flutter_sdk()` (`locator.rs:451-469` in old code). This fallback was the safety net for Flutter installations where:
   - The `flutter` binary is on PATH but is a **wrapper script** (Homebrew shim, shell wrapper) — `resolve_sdk_root_from_binary()` follows `fs::canonicalize()` which returns the script's own path, and `parent().parent()` doesn't point to a valid SDK root.
   - The PATH-resolved SDK root passes `validate_sdk_path()` but has a **missing or unreadable VERSION file** — `try_resolve_sdk()` returns `None` instead of building a usable `FlutterSdk`.

2. With the fallback gone, `find_flutter_sdk` returns `Err(FlutterNotFound)` after all 10 strategies fail.

3. `Engine::new()` sets `resolved_sdk = None` (`engine.rs:206-213`).

4. `dispatch_startup_action()` finds `flutter_executable() == None` and enqueues `Message::DeviceDiscoveryFailed { error: "Flutter SDK not found...", is_background: false }` (`runner.rs:187-193`).

5. `drain_pending_messages()` processes the message, calling `target_selector.set_error(...)` (`update.rs:405-428`).

**Affected Files:**
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs` — `try_system_path` strategy too strict, bare fallback removed
- `crates/fdemon-tui/src/runner.rs:187-193` — sends the error message

---

### Bug 2: `set_bootable_devices()` Silently Clears SDK Error

**Symptom:** The "Flutter SDK not found" error appears for 1-3 frames then vanishes, leaving an empty/blank dialog with no error, no devices, and no loading spinner.

**Expected:** If the SDK is genuinely not found, the error should persist until the user resolves the issue.

**Root Cause Analysis:**

1. `ToolAvailabilityChecked` arrives (async, within seconds of startup) and triggers `UpdateAction::DiscoverBootableDevices` when `xcrun_simctl || android_emulator` is true (`update.rs:1189-1208`).

2. Bootable device discovery completes (uses `xcrun simctl list` / `emulator -list-avds` — does **not** require Flutter SDK).

3. `BootableDevicesDiscovered` handler calls `target_selector.set_bootable_devices(...)` (`update.rs:1216-1232`).

4. `set_bootable_devices()` unconditionally sets `self.error = None` (`target_selector_state.rs:237`), erasing the SDK error as a side effect.

5. The same issue exists in `set_connected_devices()` (`target_selector_state.rs:215`).

**Affected Files:**
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs:215,237` — unconditional `self.error = None`

---

## The Transient Timeline

```
Frame 1:   Loading spinner (initial state, before drain_pending_messages)
Frame 2:   "Flutter SDK not found" (DeviceDiscoveryFailed processed)
Frame 3-N: Error persists (waiting for bootable discovery to complete)
Frame N+1: Blank — no error, no devices (BootableDevicesDiscovered clears error)
```

---

## Affected Modules

- `crates/fdemon-daemon/src/flutter_sdk/locator.rs`: Restore PATH-based fallback with distinct `SdkSource` variant
- `crates/fdemon-daemon/src/flutter_sdk/types.rs`: Add `SdkSource::PathInferred` variant
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`: Make `set_error` / error-clearing logic category-aware

---

## Fix Approach

### Fix 1: Restore PATH Fallback with Distinct SdkSource Variant

Add a final fallback after strategy 10 that creates a usable `FlutterSdk` when the `flutter` binary exists on PATH but the SDK root can't be fully resolved. Use a new `SdkSource::PathInferred` variant (not `SystemPath`) to make the limited resolution explicit.

**Key difference from the removed `try_system_path_bare`:**
- Use `resolve_sdk_root_from_binary` result if available (even without VERSION file)
- Fall back to the binary's grandparent directory as root
- Set `version` to `"unknown"` only when `read_version_file` fails
- The `PathInferred` source variant signals to downstream consumers that the SDK was not fully validated

**Steps:**
1. Add `SdkSource::PathInferred` to `types.rs` with `Display` impl
2. Add a `try_resolve_sdk_lenient` helper (or extend `try_resolve_sdk` with a `lenient: bool` parameter) that builds a `FlutterSdk` even when `read_version_file` fails, using `version: "unknown".to_string()`
3. After strategy 10 fails, add strategy 11: re-scan PATH with the lenient resolver
4. Preserve the `Err(FlutterNotFound)` termination after strategy 11 fails

### Fix 2: Categorize Target Selector Errors

Prevent `set_bootable_devices()` and `set_connected_devices()` from clearing SDK-level errors. Two options:

**Option A (Minimal):** Only clear `self.error` in `set_connected_devices()` (since connected device results imply the SDK is working). Do NOT clear `self.error` in `set_bootable_devices()` (bootable discovery is SDK-independent).

**Option B (Robust):** Replace `error: Option<String>` with a typed error:
```rust
pub enum TargetSelectorError {
    SdkNotFound(String),   // Sticky — only cleared by successful SDK resolution
    DiscoveryFailed(String), // Transient — cleared by successful discovery
}
```
Then `set_bootable_devices` only clears `DiscoveryFailed`, and `set_connected_devices` clears both.

**Recommendation:** Option A for now — minimal change, low risk. Option B can be considered for Phase 2 when more error states are needed.

---

## Edge Cases & Risks

### Flutter Wrapper Scripts
- **Risk:** Homebrew, snap, or custom wrapper scripts for `flutter` where `fs::canonicalize` resolves to the wrapper, not the SDK binary.
- **Mitigation:** The lenient fallback creates a usable `FlutterSdk` with the binary path from PATH, even if the SDK root can't be fully resolved. Device discovery only needs the executable path, not the SDK root.

### Missing VERSION File
- **Risk:** Custom Flutter builds or development checkouts may lack a VERSION file.
- **Mitigation:** The lenient resolver sets `version: "unknown"` and proceeds. The `channel` field will also be `None`.

### Sticky Error Accumulation
- **Risk:** If `set_bootable_devices` stops clearing errors, a stale `DiscoveryFailed` error from a previous discovery attempt could persist.
- **Mitigation:** `set_connected_devices` still clears errors (connected devices require a working SDK). The only error that becomes sticky is `SdkNotFound`, which is correct behavior.

---

## Task Dependency Graph

```
┌────────────────────────────────────┐
│  01-restore-path-fallback          │
│  (locator.rs + types.rs)           │
└────────────────┬───────────────────┘
                 │
                 ▼
┌────────────────────────────────────┐
│  02-fix-error-clearing             │
│  (target_selector_state.rs)        │
└────────────────────────────────────┘
```

Task 02 depends on 01 because the fix behavior should be verified end-to-end: with the PATH fallback restored, the error should never appear in the first place on most machines. Task 02 then ensures that if the SDK is genuinely absent, the error persists correctly.

---

## Success Criteria

- [ ] `find_flutter_sdk` returns `Ok(sdk)` with `SdkSource::PathInferred` when `flutter` is on PATH but SDK root can't be fully resolved
- [ ] `FlutterSdk` from the fallback has a valid `executable` path and can be used for device discovery
- [ ] `set_bootable_devices()` does NOT clear `target_selector.error`
- [ ] `set_connected_devices()` still clears `target_selector.error` (successful device discovery implies working SDK)
- [ ] No "Flutter SDK not found" flash on startup when Flutter is available on PATH
- [ ] When Flutter is genuinely absent, the error persists (not cleared by bootable discovery)
- [ ] All existing tests pass, no regressions
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

---

## Milestone Deliverable

The new session dialog no longer flashes "Flutter SDK not found" on startup. SDK detection gracefully handles wrapper scripts and missing VERSION files via a lenient PATH fallback. Error display in the target selector is resilient to unrelated state updates from bootable device discovery.
