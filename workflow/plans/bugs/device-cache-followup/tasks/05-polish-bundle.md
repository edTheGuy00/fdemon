# Task 05 — Polish Bundle (m1, m3, m5, m7, n1, n3)

**Agent:** implementor
**Phase:** 2
**Depends on:** none (Wave 3, after Wave 2 has merged)
**Files Modified (Write):**
- `crates/fdemon-app/src/handler/tests.rs`
- `crates/fdemon-app/src/handler/mod.rs`
- `crates/fdemon-app/src/handler/new_session/navigation.rs`
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`

---

## Goal

Apply the minor cleanups bundled together because they each touch only a few lines and
land naturally in adjacent files.

| Item | Where | What |
|---|---|---|
| m1 | `handler/tests.rs` | Symmetric `BootableDevicesDiscovered` clearing test |
| m3 | `target_selector_state.rs` | Comment on `set_error()` asymmetric clearing |
| m5 | `navigation.rs` | Comment on close+reopen race near `refreshing = true` |
| m7 | `navigation.rs` | Collapse dead `if/else` in `handle_close_new_session_dialog` |
| n1 | `handler/mod.rs` | Multi-line `RefreshDevicesAndBootableBackground` doc |
| n3 | `navigation.rs` | Capture `cached_devices.len()` to a local before `.clone()` |

## Steps

### m1 — Symmetric bootable test

Open `crates/fdemon-app/src/handler/tests.rs`. Find the existing
`test_devices_discovered_clears_refreshing` test (added during the parent plan's task 04).
Add a sibling test below it:

```rust
#[test]
fn test_bootable_devices_discovered_clears_bootable_refreshing() {
    let mut state = AppState::new();
    state.show_new_session_dialog(LoadedConfigs::default());
    state.new_session_dialog_state.target_selector.bootable_refreshing = true;

    let _ = handler::update(
        &mut state,
        Message::BootableDevicesDiscovered {
            ios_simulators: vec![],
            android_avds: vec![],
        },
    );

    assert!(
        !state.new_session_dialog_state.target_selector.bootable_refreshing,
        "BootableDevicesDiscovered must clear the bootable_refreshing flag"
    );
}
```

Match the imports / scaffolding the existing test uses.

### m3 — Comment on `set_error()` asymmetric clearing

Open `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`. Find
`set_error()` (around line 271-276). Add a doc comment above the function (or an inline
comment) explaining why only `refreshing` is cleared:

```rust
/// Set the connected-discovery error state.
///
/// Clears `loading` and `refreshing` because this is invoked only from the
/// connected-device foreground failure path (`Message::DeviceDiscoveryFailed`
/// with `is_background: false`). `bootable_refreshing` is intentionally **not**
/// cleared here — bootable failures are routed through their own paths
/// (`spawn_bootable_device_discovery` swallows errors via `unwrap_or_default()`),
/// and clearing the bootable indicator on a connected error would be misleading.
pub fn set_error(&mut self, error: String) {
    // ... existing body
}
```

Adjust phrasing as needed; the goal is a single concise paragraph for the next
maintainer.

### m5 — Comment on close+reopen race

Open `crates/fdemon-app/src/handler/new_session/navigation.rs`. Find the cache-hit branch
in `handle_open_new_session_dialog` where `refreshing = true` is written (introduced by
the parent plan's task 04, around lines 246-253). Add a brief comment above the writes:

```rust
// Set refreshing flags AFTER set_*_devices() (which clears them).
//
// Race: if the user closes and quickly reopens the dialog while a previous
// background discovery is in flight, that discovery's DevicesDiscovered message
// will arrive at the new dialog and clear `refreshing` before this open's own
// discovery completes. Convergence is correct (last write wins), but the visual
// cue may briefly disappear and reappear. Acceptable transient flicker.
state.new_session_dialog_state.target_selector.refreshing = true;
```

### m7 — Collapse dead branch in `handle_close_new_session_dialog`

In the same file, find `handle_close_new_session_dialog` (around lines 268-280). The
body currently has an `if/else` where both branches assign `UiMode::Normal`:

```rust
if state.session_manager.has_running_sessions() {
    state.ui_mode = UiMode::Normal;
} else {
    // No sessions, stay in startup mode
    state.ui_mode = UiMode::Normal;
}
```

Replace with a single unconditional assignment and remove the misleading comment:

```rust
state.ui_mode = UiMode::Normal;
```

The "stay in startup mode" comment was inaccurate (no path returns to `UiMode::Startup`
after the dialog opens — the startup flow transitions away from `Startup` via
`show_new_session_dialog`).

### n1 — Multi-line `RefreshDevicesAndBootableBackground` doc

Open `crates/fdemon-app/src/handler/mod.rs`. Find `RefreshDevicesAndBootableBackground`
(around line 80, introduced by the parent plan's task 03). Currently a one-liner:

```rust
RefreshDevicesAndBootableBackground { flutter: FlutterExecutable },
```

Reformat to match sibling variants' style:

```rust
RefreshDevicesAndBootableBackground {
    /// Flutter executable to use for both background discovery tasks.
    flutter: FlutterExecutable,
},
```

If `DiscoverDevicesAndBootable` from task 03 is already in the file with the same
multi-line shape, mirror its style for consistency.

### n3 — Capture `len()` before clone

In `navigation.rs`, find the cache-hit branch in `handle_open_new_session_dialog` where
`cached_devices.clone()` is forwarded to `set_connected_devices`. Capture the length
into a local before the clone for clarity:

```rust
let cached_len = cached_devices.len();
let age = state.devices_last_updated.map(|t| t.elapsed());
state.new_session_dialog_state
    .target_selector
    .set_connected_devices(cached_devices.clone());
tracing::debug!("Using cached devices ({} devices, age: {:?})", cached_len, age);
```

Adjust to match the actual existing structure in the post-task-02 code.

### Verification

- `cargo fmt --all`
- `cargo check --workspace`
- `cargo test --workspace --lib`
- `cargo clippy --workspace --lib -- -D warnings`

## Acceptance Criteria

- [ ] m1: `test_bootable_devices_discovered_clears_bootable_refreshing` is present in
      `handler/tests.rs` and passes.
- [ ] m3: `set_error()` in `target_selector_state.rs` has a doc comment explaining why
      `bootable_refreshing` is not cleared.
- [ ] m5: A comment near the `refreshing = true` write in `navigation.rs` documents the
      close+reopen race.
- [ ] m7: `handle_close_new_session_dialog` is a single `state.ui_mode = UiMode::Normal;`
      with no misleading comment.
- [ ] n1: `RefreshDevicesAndBootableBackground` is multi-line with `///` doc on `flutter`.
- [ ] n3: `cached_devices.len()` is captured to a local before `.clone()` in the
      cache-hit branch.
- [ ] `cargo test --workspace --lib` passes.
- [ ] `cargo clippy --workspace --lib -- -D warnings` clean.

## Out of Scope

- Changing the asymmetric clearing behaviour of `set_error()` (m3 is a comment-only fix).
- Resolving the close+reopen race itself — only documenting it.
- Resolving the pre-existing TODO at `target_selector_state.rs:455` about
  `calculate_scroll_offset` (m9 — explicitly out of scope per the followup plan).
- The icon routing and compact-mode work (handled in task 04).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a82394c45b9c3b098

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/tests.rs` | Added `test_bootable_devices_discovered_clears_bootable_refreshing` test (m1) |
| `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | Replaced one-liner `/// Set error state` doc with multi-paragraph comment explaining asymmetric clearing (m3) |
| `crates/fdemon-app/src/handler/new_session/navigation.rs` | Added race comment above `refreshing = true` writes (m5); collapsed dead `if/else` in `handle_close_new_session_dialog` to single assignment (m7); captured `cached_devices.len()` to `cached_len` local before `.clone()` (n3) |
| `crates/fdemon-app/src/handler/mod.rs` | Expanded `RefreshDevicesAndBootableBackground` to multi-line with `///` doc on `flutter` field (n1) |

### Notable Decisions/Tradeoffs

1. **Pre-existing test failure**: `flutter_sdk::locator::tests::test_flutter_wrapper_detection` fails before and after these changes — confirmed via `git stash` round-trip. It is an environment-sensitive test unrelated to this task.
2. **m7 doc comment**: Removed the misleading "No sessions, stay in startup mode" comment along with the dead branch, and trimmed the function's doc comment to match the simplified body.

### Testing Performed

- `cargo fmt --all` - Passed (reformatted navigation.rs and tests.rs)
- `cargo check --workspace` - Passed
- `cargo test --workspace --lib` - 733 passed, 1 pre-existing failure (unrelated)
- `cargo clippy --workspace --lib -- -D warnings` - Passed (clean)
- `cargo test -p fdemon-app --lib test_bootable_devices_discovered_clears_bootable_refreshing` - Passed

### Risks/Limitations

1. **Pre-existing test failure**: `test_flutter_wrapper_detection` in `fdemon-daemon` fails due to an environment condition (a `flutter` wrapper present in the test environment), not related to this task's changes.
