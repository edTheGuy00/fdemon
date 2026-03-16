## Task: Fix Branch B Guard Blocking Platform Capture on Android/macOS/iOS

**Objective**: Fix the hot-restart guard (Branch B) in `maybe_start_native_log_capture` so it only fires for custom-sources-only sessions (Linux/Windows/Web), not for platform-capture sessions (Android/macOS/iOS) where platform capture hasn't started yet.

**Depends on**: None

**Severity**: BUG

**Review Reference**: [PR #23 Copilot comment](https://github.com/edTheGuy00/fdemon/pull/23#discussion_r2936678247)

### Scope

- `crates/fdemon-app/src/handler/session.rs`: Fix Branch B guard (~line 352)
- `crates/fdemon-app/src/handler/tests.rs`: Add 2 new tests for Android + pre-app-only scenarios

### Context

`maybe_start_native_log_capture` (`session.rs:287–424`) has two early-return guards to prevent duplicate captures on hot-restart:

- **Branch A** (line 343): `native_log_shutdown_tx.is_some() && !has_unstarted_post_app` — platform capture is running AND all post-app sources are running → nothing left to do. (**Correct.**)

- **Branch B** (line 352): `!custom_source_handles.is_empty() && !has_unstarted_post_app` — designed for custom-sources-only sessions (Linux/Windows/Web) where `native_log_shutdown_tx` is never set. (**Buggy.**)

Branch B fires incorrectly on Android/macOS/iOS when:
1. Pre-app custom sources are running → `custom_source_handles` is non-empty
2. No post-app sources configured → `has_unstarted_post_app` is false
3. Platform capture hasn't started yet → `native_log_shutdown_tx` is None
4. Branch A skips (needs `is_some()`) → Branch B fires → returns `None`
5. `needs_platform_capture` is never evaluated → logcat/log-stream never starts

This only affects the **first `AppStart`** for sessions with pre-app sources and no post-app sources on platform-capture platforms. Hot-restart (where platform capture is already running) is handled correctly by Branch A.

### Details

**Option A (recommended): Hoist `needs_platform_capture` above the guard block** and add `!needs_platform_capture` to Branch B's condition:

Move the platform detection (currently at line ~364) above the guard block:

```rust
// Compute early — needed by guard Branch B.
let needs_platform_capture = platform == "android"
    || (cfg!(target_os = "macos") && platform == "macos")
    || (cfg!(target_os = "macos") && platform == "ios");
```

Then update Branch B:

```rust
// Custom-sources-only session (Linux/Windows/Web): some sources tracked +
// all post-app sources running → stop (hot-restart guard).
// Must NOT fire for platform-capture sessions where native_log_shutdown_tx
// being None means capture hasn't started yet — not that it's unneeded.
if !handle.custom_source_handles.is_empty()
    && !has_unstarted_post_app
    && !needs_platform_capture
{
    tracing::debug!(
        "All custom sources already running for session {} — skipping",
        session_id
    );
    return None;
}
```

Remove the duplicate `needs_platform_capture` computation at its original location (line ~364) since it's now computed earlier. The rest of the function (`has_platform_tools`, `has_custom_sources`, `should_start`) remains unchanged.

**Option B (alternative): Inline platform check without hoisting.**

Add a lightweight inline check to Branch B without moving `needs_platform_capture`:

```rust
let platform_needs_capture = platform == "android"
    || (cfg!(target_os = "macos") && platform == "macos")
    || (cfg!(target_os = "macos") && platform == "ios");

if !handle.custom_source_handles.is_empty()
    && !has_unstarted_post_app
    && !platform_needs_capture
{
    ...
}
```

Option A is preferred because it avoids duplicating the platform-detection expression. The computation is trivially cheap (string comparisons).

### Acceptance Criteria

1. On Android/macOS/iOS sessions with pre-app-only custom sources and `native_log_shutdown_tx = None`, `maybe_start_native_log_capture` returns `Some(StartNativeLogCapture)`.
2. On Linux/Windows/Web sessions with all custom sources running and no unstarted post-app sources, `maybe_start_native_log_capture` still returns `None` (existing behavior preserved).
3. On Android sessions where platform capture IS running (Branch A scenario), behavior is unchanged — still returns `None`.
4. Two new tests pass; all existing tests continue to pass.

### Testing

Add two tests in `handler/tests.rs`:

#### Test 1: Android + pre-app only + no platform capture → should return Some

```rust
#[test]
fn test_android_pre_app_only_allows_platform_capture_start() {
    // Android session with:
    // - Pre-app custom source running (in custom_source_handles)
    // - No post-app sources configured
    // - native_log_shutdown_tx = None (platform capture NOT started)
    // Branch B must NOT fire → function returns Some(StartNativeLogCapture)

    let device = android_device("android-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    state.tool_availability = ToolAvailability { adb: true, ..Default::default() };
    state.settings.native_logs.enabled = true;

    // Only a pre-app custom source (no post-app sources).
    state.settings.native_logs.custom_sources = vec![CustomSourceConfig {
        name: "server".to_string(),
        command: "python3".to_string(),
        args: vec!["server.py".to_string()],
        start_before_app: true,
        shared: false,
        ..Default::default()
    }];

    // Simulate pre-app source already running (stored in custom_source_handles).
    {
        let handle = state.session_manager.get_mut(session_id).unwrap();
        let (tx, _rx) = tokio::sync::watch::channel(false);
        handle.custom_source_handles.push(CustomSourceHandle {
            name: "server".to_string(),
            shutdown_tx: std::sync::Arc::new(tx),
            task_handle: None,
            start_before_app: true,
        });
        // native_log_shutdown_tx is None — platform capture not started.
        assert!(handle.native_log_shutdown_tx.is_none());
    }

    let app_start_msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "android-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });

    let action = maybe_start_native_log_capture(&state, session_id, &app_start_msg);
    assert!(action.is_some(), "Android with pre-app-only sources must still start platform capture");
}
```

#### Test 2: Linux + pre-app only → should still return None (preserve existing behavior)

```rust
#[test]
fn test_linux_pre_app_only_guard_still_fires() {
    // Linux session with:
    // - Pre-app custom source running
    // - No post-app sources
    // - native_log_shutdown_tx = None (Linux never sets it)
    // Branch B should fire → returns None (Linux doesn't need platform capture)

    let device = linux_device("linux-1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();
    // ... set up identical to above but with linux_device ...

    let action = maybe_start_native_log_capture(&state, session_id, &app_start_msg);
    assert!(action.is_none(), "Linux with all sources running should return None");
}
```

Adapt helper usage and state setup to match the test infrastructure in `tests.rs`. Use `android_device` / `linux_device` helpers and follow the pattern of existing tests like `test_guard_fires_on_hot_restart_with_pre_app_sources_only_running` (line ~8132).

### Notes

- The existing tests at lines 8132 and 8190 (`test_guard_fires_on_hot_restart_with_pre_app_sources_only_running`, `test_guard_allows_post_app_sources_when_only_pre_app_running`) both use `linux_device`. They were never testing the platform-capture path. This gap is what allowed the bug to ship.
- Branch A is not affected by this fix and continues to work correctly for all platforms.
- The downstream `StartNativeLogCapture` action handler and `spawn_native_log_capture` logic are unaffected — they already correctly handle the case where custom sources exist alongside platform capture.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/session.rs` | Hoisted `needs_platform_capture` above the guard block; added `&& !needs_platform_capture` to Branch B condition; removed duplicate computation at original location; updated comments to label Branch A/B and explain the fix |
| `crates/fdemon-app/src/handler/tests.rs` | Added two new tests: `test_android_pre_app_only_allows_platform_capture_start` and `test_linux_pre_app_only_guard_still_fires` |

### Notable Decisions/Tradeoffs

1. **Option A (hoist) over Option B (inline)**: The task recommended hoisting `needs_platform_capture` to avoid duplicating the platform-detection expression. This keeps a single source of truth for that computation and makes the Branch B comment self-explanatory.

2. **Comment clarity**: Added Branch A/B labels to the guard comments and a clear explanation of why `!needs_platform_capture` is required in Branch B, making the logic easier to audit in future reviews.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1699 tests, 0 failed)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **macOS-only paths**: The `needs_platform_capture` expression uses `cfg!(target_os = "macos")` for macOS/iOS branches, so on Linux CI the new Android test exercises the Android arm of the condition while macOS/iOS remain platform-gated. This is the same constraint that existed before the fix and is acceptable.
