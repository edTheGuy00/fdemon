## Task: Fix TEA Violation — Move Browser Launch to UpdateAction

**Objective**: Remove the side effect (`std::process::Command::spawn()`) from `handle_open_browser_devtools` in the TEA update path, replacing it with a proper `UpdateAction::OpenBrowserDevTools` that is dispatched in `actions.rs`.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-app/src/handler/devtools.rs`: Refactor `handle_open_browser_devtools` to return action; move `open_url_in_browser` to `actions.rs`
- `crates/fdemon-app/src/handler/mod.rs`: Add `OpenBrowserDevTools` variant to `UpdateAction` enum
- `crates/fdemon-app/src/actions.rs`: Add handler arm for the new action
- `crates/fdemon-app/src/handler/tests.rs`: Update test to assert action is returned

### Details

#### Current Code (TEA Violation)

`handle_open_browser_devtools` (devtools.rs:241-264) calls `open_url_in_browser()` directly, which executes `std::process::Command::new(...).spawn()` — a real OS call — inside the pure TEA `update()` chain. It then returns `UpdateResult::none()` since the side effect already happened.

#### Fix: Return Action Instead

**Step 1 — Add `UpdateAction` variant** (handler/mod.rs):

Add after the existing variants (around line 175):

```rust
/// Open the Flutter DevTools URL in the system browser.
OpenBrowserDevTools {
    url: String,
    browser: String,
},
```

This variant needs no `vm_handle` — it's a fire-and-forget OS call unrelated to the VM Service.

**Step 2 — Refactor handler** (devtools.rs):

```rust
pub fn handle_open_browser_devtools(state: &AppState) -> UpdateResult {
    let ws_uri = state.session_manager.selected()
        .and_then(|h| h.session.ws_uri.clone());
    let Some(ws_uri) = ws_uri else {
        tracing::warn!("Cannot open browser DevTools: no VM Service URI available");
        return UpdateResult::none();
    };
    let encoded_uri = percent_encode_uri(&ws_uri);
    let url = format!("https://devtools.flutter.dev/?uri={encoded_uri}");
    let browser = state.settings.devtools.browser.clone();
    UpdateResult::action(UpdateAction::OpenBrowserDevTools { url, browser })
}
```

Remove the `if let Err(e) = open_url_in_browser(...)` call from the handler.

**Step 3 — Move `open_url_in_browser` to actions.rs** and add the dispatch arm:

Move `open_url_in_browser` from devtools.rs to actions.rs (it's an I/O function, not a pure handler helper). Add a match arm in `handle_action`:

```rust
UpdateAction::OpenBrowserDevTools { url, browser } => {
    tokio::spawn(async move {
        if let Err(e) = open_url_in_browser(&url, &browser) {
            tracing::error!("Failed to open browser DevTools: {e}");
        }
    });
}
```

**Step 4 — Fix platform fallback** (bonus): Add a fallback for unsupported platforms in `open_url_in_browser`:

```rust
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
{
    return Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no browser opener available for this platform",
    ));
}
```

Currently the function silently returns `Ok(())` when no `#[cfg]` block matches.

**Step 5 — Keep `percent_encode_uri` in devtools.rs**: The URL encoding helper is a pure function called before the action is created. It stays in `devtools.rs` alongside the handler.

### Acceptance Criteria

1. `handle_open_browser_devtools` returns `UpdateResult::action(UpdateAction::OpenBrowserDevTools { .. })` with no direct I/O
2. `open_url_in_browser` lives in `actions.rs` and is called from the `handle_action` dispatch
3. The browser still opens when the user presses `b` in DevTools mode
4. On unsupported platforms, an error is logged instead of silent success
5. Test verifies the handler returns the correct action with expected URL and browser string
6. All existing tests pass

### Testing

```rust
#[test]
fn test_open_browser_devtools_returns_action() {
    let mut state = make_state_with_session();
    // Set ws_uri on the session
    state.session_manager.selected_mut().unwrap().session.ws_uri = Some("ws://127.0.0.1:12345/abc=/ws".to_string());

    let result = devtools::handle_open_browser_devtools(&state);
    assert!(result.action.is_some());

    if let Some(UpdateAction::OpenBrowserDevTools { url, browser }) = result.action {
        assert!(url.starts_with("https://devtools.flutter.dev/?uri="));
        assert!(url.contains("ws%3a%2f%2f") || url.contains("ws%3A%2F%2F"));
    } else {
        panic!("Expected OpenBrowserDevTools action");
    }
}

#[test]
fn test_open_browser_devtools_no_ws_uri_returns_none() {
    let state = make_state_with_session(); // no ws_uri set
    let result = devtools::handle_open_browser_devtools(&state);
    assert!(result.action.is_none());
}
```

### Notes

- The `handle_action` function signature already has all needed parameters. `OpenBrowserDevTools` doesn't need `msg_tx`, `session_cmd_sender`, or any of the other parameters — just the url and browser string captured in the action.
- `tokio::spawn` for the browser launch is safe — `Command::spawn()` is non-blocking (it forks), but wrapping in `tokio::spawn` ensures the update loop isn't blocked even if there's a brief delay.
- Existing test for `handle_open_browser_devtools` in devtools.rs tests.rs (if any) should be updated to check for `UpdateAction` instead of void return.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mod.rs` | Added `OpenBrowserDevTools { url: String, browser: String }` variant to `UpdateAction` enum |
| `crates/fdemon-app/src/handler/devtools.rs` | Refactored `handle_open_browser_devtools` to return `UpdateResult::action(UpdateAction::OpenBrowserDevTools { .. })` instead of calling browser directly; removed `open_url_in_browser` from this file; added `make_state_with_session` test helper; added `test_open_browser_devtools_returns_action` and `test_open_browser_devtools_no_ws_uri_returns_none` tests |
| `crates/fdemon-app/src/actions.rs` | Added `OpenBrowserDevTools` match arm to `handle_action`; moved `open_url_in_browser` here with platform fallback error for unsupported platforms |

### Notable Decisions/Tradeoffs

1. **Platform `return Ok(())` after each `#[cfg]` block**: Added explicit `return Ok(())` after each platform-specific `Command::spawn()` call to avoid "unreachable code" warnings from the trailing `Ok(())`. The `#[allow(unreachable_code)]` annotation on that trailing `Ok(())` keeps clippy/rustc satisfied on supported platforms where all `#[cfg(not(...))]` blocks are dead code.

2. **`tokio::spawn` for browser launch**: The `Command::spawn()` call itself is non-blocking (it forks), but wrapping it in `tokio::spawn` ensures the event loop is never held up by any brief delay (e.g. PATH lookup), matching the pattern used by other fire-and-forget actions in `actions.rs`.

3. **`percent_encode_uri` stays in `devtools.rs`**: It is a pure function used only by the handler to build the URL before the action is created — no I/O. Moving it would have been unnecessary churn.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed
- `cargo test -p fdemon-app` — Passed (828 tests, 0 failed, 5 ignored)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings)

### Risks/Limitations

1. **Unsupported platform error**: The `#[cfg(not(any(...)))]` fallback returns `Err(Unsupported)` which is logged by `handle_action`. On such platforms the browser simply will not open, which is better than silently claiming success.

2. **Browser launch is fire-and-forget**: If the spawned browser process fails after the initial `spawn()` succeeds (e.g. exits with a non-zero code), the error is not surfaced to the user. This is consistent with how browser launchers typically work.
