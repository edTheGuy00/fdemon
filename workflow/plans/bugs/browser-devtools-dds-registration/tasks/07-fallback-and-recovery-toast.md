## Task: Recovery Toast on Legacy-URL Fallback

**Objective**: When `B` falls back to the legacy URL because no DevTools endpoint was served, show a clear user-facing toast explaining what happened and how to recover (e.g., "Update Flutter SDK to ≥ 3.16 or run `dart devtools` manually").

**Depends on**: 06-open-browser-uses-served-url

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/mod.rs` (`handle_open_browser_devtools`):
  - When falling back, additionally emit a `Message::ShowToast` (or push to a toast queue, whatever the existing pattern is) with a clear, actionable message.
- `crates/fdemon-app/src/state.rs`: If a toast / notification queue does not yet exist on `AppState`, add a small `Vec<Toast>` (or reuse existing notification system if present — audit first).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs`, `crates/fdemon-tui/src/widgets/`: For the existing toast / notification rendering pattern.

### Details

Audit for the existing notification system first — fdemon may already have a `notifications` module or status-line message queue. Reuse it.

```rust
let url = match &session.session.devtools_endpoint {
    Some(endpoint) => endpoint.url(ws_uri),
    None => {
        warn!("Falling back to legacy DevTools URL");
        let toast_msg = if session.session.devtools_serve_pending {
            "DevTools server is still starting — try again in a moment.".to_string()
        } else {
            "DevTools is not registered with DDS. Try: \
             update Flutter (≥ 3.16) or run `dart devtools` and paste the VM Service URI manually.".to_string()
        };
        // Push toast or emit message — match existing pattern
        push_toast(state, ToastLevel::Warn, toast_msg);
        let encoded = percent_encode_uri(ws_uri);
        build_local_devtools_url(ws_uri, &encoded)
    }
};
```

### Acceptance Criteria

1. When fallback is hit, a user-facing toast/notification appears with an actionable message.
2. When the served endpoint is available, no toast is shown.
3. When the user presses `B` while `devtools_serve_pending = true`, a different toast tells them to wait (or the open is deferred — pick one and document).
4. Existing tests pass; new tests cover both toast emission paths.
5. The toast disappears after a few seconds (reuse existing TTL mechanism if present).

### Testing

```rust
#[test]
fn fallback_path_emits_toast() {
    let mut state = AppState::test_default();
    state.add_test_session_with_ws_uri("ws://...");
    let result = handle_open_browser_devtools(&mut state);
    // Either assert action carries toast, or assert state.toasts has new entry
    assert!(state.toasts.iter().any(|t| t.text.contains("DevTools is not registered with DDS")));
}

#[test]
fn pending_serve_emits_different_toast() {
    let mut state = AppState::test_default();
    let sid = state.add_test_session_with_ws_uri("ws://...");
    state.session_manager.get_mut(sid).unwrap().session.devtools_serve_pending = true;
    handle_open_browser_devtools(&mut state);
    assert!(state.toasts.iter().any(|t| t.text.contains("still starting")));
}

#[test]
fn served_endpoint_no_toast() {
    let mut state = AppState::test_default();
    let sid = state.add_test_session_with_ws_uri("ws://...");
    state.session_manager.get_mut(sid).unwrap().session.devtools_endpoint = Some(/* ... */);
    handle_open_browser_devtools(&mut state);
    assert!(state.toasts.is_empty());
}
```

### Notes

- If the existing notification system uses a different API surface (e.g., `Message::ShowStatus(...)`), use that instead.
- Keep toast messages short — terminal real estate is limited.
- If `state.toasts` doesn't exist yet, this is the time to add it (or punt to a tiny pre-task; user can decide).

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `Toast` struct, `ToastLevel` enum, `TOAST_TTL_SECS` constant; added `toasts: Vec<Toast>` field to `AppState`; added `push_toast()` and `expire_toasts()` methods |
| `crates/fdemon-app/src/handler/update.rs` | Added `state.expire_toasts()` call in the `Tick` arm |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Changed `handle_open_browser_devtools` signature to `&mut AppState`; pushes a `ToastLevel::Warn` toast on fallback (two flavours: pending vs no endpoint); updated all existing tests; added 3 new toast tests |
| `crates/fdemon-tui/src/render/mod.rs` | Added `render_toasts()` helper and called it at the end of `view()` so toasts appear on top of all other UI elements |

### Notable Decisions/Tradeoffs

1. **No separate toast module**: Toasts are small enough to live directly on `AppState` and `render/mod.rs`. A separate widget module would be over-engineering for a `Vec<Toast>` and a 50-line renderer.
2. **Pending → still open legacy fallback**: Per the task's "pick the simpler one" guidance, `devtools_serve_pending = true` shows a "still starting" toast but still opens the legacy fallback URL. The alternative (deferring the open) would require a new state machine and is unnecessary complexity.
3. **`&mut AppState` for the handler**: The signature change is minimal; the call site in `update.rs` already passes `&mut state` so no other callers were affected.
4. **Toast wording uses ≥ 1.22 (RESEARCH.md)**: The task file says "≥ 3.16" but RESEARCH.md verifies the correct minimum is Flutter ≥ 1.22 (October 2020 stable). The implemented message uses 1.22.
5. **Right-aligned toast pill**: Toasts are rendered right-aligned so they do not overlap the most important left-aligned log content, and they are visually distinct from search overlays and link-highlight bars.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo test --workspace` — Passed (2148 + 842 + others, 0 failed)
  - `fallback_path_emits_toast` — new test, passes
  - `pending_serve_emits_different_toast` — new test, passes
  - `served_endpoint_no_toast` — new test, passes
  - All pre-existing tests pass (signature change is backward-compatible at the update.rs call site)

### Risks/Limitations

1. **Toast TTL is wall-clock based**: Toast expiry uses `Instant::elapsed()` checked on each `Tick`. The `Tick` frequency (100 ms in the TUI event loop) means toasts could persist up to `TOAST_TTL_SECS + 0.1s` — negligible in practice.
2. **No TUI snapshot tests for toasts**: The existing TUI snapshot tests do not cover the toast overlay because they render without an active `Tick` loop. The handler-layer tests verify the push/no-push behaviour; visual correctness can be confirmed by running the app.
