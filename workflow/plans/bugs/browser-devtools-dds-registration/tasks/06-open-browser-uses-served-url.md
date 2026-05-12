## Task: `handle_open_browser_devtools` Prefers Served URL

**Objective**: Modify `handle_open_browser_devtools` to use `session.devtools_endpoint.url(ws_uri)` when available; otherwise fall back to the legacy `build_local_devtools_url`. Keep the legacy fallback intact for now — task 07 layers the recovery toast on top.

**Depends on**: 05-eager-serve-on-vmservice-ready

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/mod.rs` (lines 385-401 `handle_open_browser_devtools`):
  - Read `session.devtools_endpoint`.
  - If `Some(endpoint)`, build `endpoint.url(&session.ws_uri)`.
  - If `None`, fall back to `build_local_devtools_url`.
- `crates/fdemon-app/src/handler/devtools/mod.rs` (existing tests around lines 720-744): Update to assert the served-URL shape when `devtools_endpoint` is populated; add new tests for the legacy fallback path.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/session.rs`: For `Session.devtools_endpoint`.

### Details

```rust
pub fn handle_open_browser_devtools(state: &mut AppState) -> UpdateResult {
    let Some(session) = state.session_manager.selected() else {
        return UpdateResult::default();
    };
    let ws_uri = session.session.ws_uri.as_deref().unwrap_or("");
    if ws_uri.is_empty() {
        warn!("DevTools open requested but no VM Service ws_uri yet");
        return UpdateResult::default();
    }

    let url = match &session.session.devtools_endpoint {
        Some(endpoint) => {
            info!(host = %endpoint.host, port = endpoint.port, "Opening served DevTools URL");
            endpoint.url(ws_uri)
        }
        None => {
            warn!("No served DevTools endpoint — falling back to legacy URL");
            let encoded = percent_encode_uri(ws_uri);
            build_local_devtools_url(ws_uri, &encoded)
        }
    };

    UpdateResult::with_action(UpdateAction::OpenBrowserDevTools {
        url,
        browser: state.settings.browser.clone(),
    })
}
```

### Acceptance Criteria

1. When `session.devtools_endpoint = Some(...)`, the opened URL is the served URL (`http://<host>:<port>/?uri=<encoded>`).
2. When `session.devtools_endpoint = None`, the URL is the legacy `http://<DDS-host>/devtools/?uri=<encoded>`.
3. Existing tests at `mod.rs:720-744` are updated and continue to pass.
4. New tests cover both branches with distinct fixtures.
5. `cargo test --workspace` and clippy pass.

### Testing

```rust
#[test]
fn open_browser_uses_served_endpoint_when_available() {
    let mut state = AppState::test_default();
    let sid = state.add_test_session_with_ws_uri("ws://127.0.0.1:1234/abc=/ws");
    state.session_manager.get_mut(sid).unwrap().session.devtools_endpoint = Some(DevToolsEndpoint {
        host: "127.0.0.1".into(),
        port: 9100,
        served_at: Instant::now(),
    });
    let result = handle_open_browser_devtools(&mut state);
    match result.action.unwrap() {
        UpdateAction::OpenBrowserDevTools { url, .. } => {
            assert!(url.starts_with("http://127.0.0.1:9100/?uri="));
            assert!(url.contains("ws%3A%2F%2F127.0.0.1%3A1234"));
        }
        _ => panic!("expected OpenBrowserDevTools"),
    }
}

#[test]
fn open_browser_falls_back_to_legacy_url_when_no_endpoint() {
    let mut state = AppState::test_default();
    state.add_test_session_with_ws_uri("ws://127.0.0.1:1234/abc=/ws");
    let result = handle_open_browser_devtools(&mut state);
    match result.action.unwrap() {
        UpdateAction::OpenBrowserDevTools { url, .. } => {
            assert!(url.contains("/devtools/?uri="));  // legacy shape
        }
        _ => panic!("expected OpenBrowserDevTools"),
    }
}
```

### Notes

- Don't break `build_local_devtools_url` — it stays as the fallback.
- No new keybinding; `B` continues to dispatch `OpenBrowserDevTools`.
- The recovery toast for the fallback case lands in task 07.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Updated `handle_open_browser_devtools` to check `session.devtools_endpoint` and use `endpoint.url(ws_uri)` when present; falls back to `build_local_devtools_url`. Changed log line to `info!(base_url = %endpoint.base_url, ...)`. Added 3 new tests. |

### Notable Decisions/Tradeoffs

1. **Kept `&AppState` (immutable borrow)**: The task file snippet used `&mut AppState`, but the function only reads state so `&AppState` is correct and more restrictive. Kept the existing signature.
2. **`DevToolsEndpoint { base_url, served_at }` shape**: Task file's testing section referenced `{host, port}` but per the key context override, the actual shape is `{base_url, served_at}`. Tests use the correct shape.
3. **`endpoint.url()` delegates percent-encoding**: The `DevToolsEndpoint::url()` method in `session.rs` handles encoding internally, so no duplication in the handler.

### Testing Performed

- `cargo test -p fdemon-app handler::devtools` — Passed (201 tests)
- `cargo check --workspace --all-targets` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo fmt --all -- --check` — Passed

### Risks/Limitations

1. **Legacy fallback unchanged**: When no endpoint is present, the legacy DDS-path URL is used unchanged. Task 07 will add the recovery toast for this case.
