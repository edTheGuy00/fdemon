## Task: Gate `Message::NewVersionAvailable` on `ui_mode` to prevent late-arrival banner surface

**Objective**: When a version-check task completes after the user has progressed past startup (auto-launch, or NSD already dismissed), the resulting `Message::NewVersionAvailable` currently sets `state.startup_notice` anyway — surfacing the banner whenever the user later opens NSD mid-session. Gate the handler arm so late messages are dropped.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**

- `crates/fdemon-app/src/handler/update.rs`: The `Message::NewVersionAvailable { latest }` arm (currently around line 360) becomes conditional on `state.is_new_session_dialog_visible()` — i.e. true only when `ui_mode` is `Startup` or `NewSessionDialog`.

**Files Read (Dependencies):**

- `crates/fdemon-app/src/state.rs:1707` — confirm `is_new_session_dialog_visible(&self) -> bool` exists and returns the union of `Startup | NewSessionDialog`.
- `crates/fdemon-app/src/message.rs` — confirm `NewVersionAvailable { latest: String }` variant shape.

### Details

**Current arm** (around `update.rs:360-364`):

```rust
Message::NewVersionAvailable { latest } => {
    state.startup_notice = Some(StartupNotice::NewVersionAvailable { latest });
    UpdateResult::none()
}
```

**After this task:**

```rust
Message::NewVersionAvailable { latest } => {
    if state.is_new_session_dialog_visible() {
        state.startup_notice = Some(StartupNotice::NewVersionAvailable { latest });
    } else {
        tracing::debug!(
            "Version check completed after dialog dismissed; dropping notice for v{}",
            latest
        );
    }
    UpdateResult::none()
}
```

**Why a `tracing::debug!`**: the user explicitly opted into the check (`version_check = true`), so silently discarding the result is fine, but a debug-level trace is useful when diagnosing "why didn't I see the banner" reports.

### Acceptance Criteria

1. `cargo test -p fdemon-app handler` passes with the existing test (`new_version_available_sets_startup_notice`) updated to put `state` into `UiMode::NewSessionDialog` before sending the message.
2. A new test `new_version_available_dropped_when_dialog_not_visible` asserts that when `state.ui_mode` is `UiMode::Normal`, the message does not set `state.startup_notice`.
3. `cargo clippy -p fdemon-app -- -D warnings` clean.

### Testing

In `crates/fdemon-app/src/handler/update.rs` test module (replace the existing test if its current setup doesn't drive `ui_mode` correctly):

```rust
#[test]
fn new_version_available_sets_startup_notice_when_dialog_visible() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::NewSessionDialog;
    let (new_state, _) = update(
        state,
        Message::NewVersionAvailable { latest: "0.6.0".into() },
    );
    assert_eq!(
        new_state.startup_notice,
        Some(StartupNotice::NewVersionAvailable { latest: "0.6.0".into() })
    );
}

#[test]
fn new_version_available_sets_startup_notice_when_in_startup() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Startup;
    let (new_state, _) = update(
        state,
        Message::NewVersionAvailable { latest: "0.6.0".into() },
    );
    assert!(new_state.startup_notice.is_some());
}

#[test]
fn new_version_available_dropped_when_dialog_not_visible() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Normal;
    let (new_state, _) = update(
        state,
        Message::NewVersionAvailable { latest: "0.6.0".into() },
    );
    assert!(new_state.startup_notice.is_none());
}
```

### Notes

- The existing `hide_new_session_dialog_clears_startup_notice` test at `state.rs:3052-3066` still verifies the post-dismissal clear path — leave it untouched.
- The fix is one `if` branch; the bulk of this task is updating/adding tests.
- This task is independent of task 04 (version-check refactor) — they touch disjoint files. They run in parallel in Wave 1.
- The existing helper `state.is_new_session_dialog_visible()` (at `state.rs:1707`) is exactly the right predicate — do not duplicate the `matches!` expression inline.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/version-check-banner-followup

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <Rationale>

### Testing Performed

- `cargo test -p fdemon-app handler` — Pending
- `cargo clippy -p fdemon-app -- -D warnings` — Pending

### Risks/Limitations

1. **<Risk>**: <Description>
