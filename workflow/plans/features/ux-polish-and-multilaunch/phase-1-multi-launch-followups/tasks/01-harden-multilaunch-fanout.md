## Task: Harden the multi-launch fan-out failure path

**Objective**: Fix the orphaned-session leak, persist the auto-launch default for the first *successfully launched* device, ANSI-strip daemon-sourced strings in user-facing messages, document the eviction-policy coupling, and add the missing cap-hit-mid-loop test — all within `handle_launch` / `spawn_one`.

**Depends on**: None

**Estimated Time**: 3–4h

**Addresses review items**: M1 (orphan leak), M2 (cap-hit test), m3 (eviction comment), m4 (save_last_selection), m6 (ANSI strip)

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/new_session/launch_context.rs`: fix `spawn_one` orphan rollback, move `save_last_selection` to the first successful device, ANSI-strip toast/error strings, add documentation comment, add the cap-hit test.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session_manager.rs`: `remove_session` (pub, line 201), `ensure_capacity`/`evict_oldest_stopped` (lines 163–192), `MAX_SESSIONS`.
- `crates/fdemon-core/src/ansi.rs`: `strip_ansi_codes` (pub, re-exported at `fdemon_core` root).

### Details

#### M1 — Orphaned-session leak (`spawn_one`, lines ~705–715)

The no-SDK branch returns `Err` after `create_session_*` already inserted the session. The session sits in `AppPhase::Initializing` forever — `evict_oldest_stopped` only reclaims `Stopped` sessions, so it is **never** garbage-collected and permanently consumes one of the 9 slots, blocking relaunch of that device. The inline comment claiming "There is no undo API" is **false**: `SessionManager::remove_session(session_id)` is public.

**Preferred fix — hoist the SDK check before session creation** (fail fast, zero orphans). Since `flutter_executable()` is state-global (same result for every device in the loop), resolve it once at the top of `spawn_one` (or once before the loop in `handle_launch`) and return `Err` before `create_session_*` runs:

```rust
// Near the top of spawn_one, before create_session_*:
let Some(flutter) = state.flutter_executable() else {
    return Err(
        "No Flutter SDK found. Configure sdk_path in .fdemon/config.toml or install Flutter."
            .to_string(),
    );
};
// ... create session, then use `flutter` in the SpawnSession action ...
```

**Alternative (if the pre-app branch makes hoisting awkward):** keep the late check but roll back before returning:

```rust
let Some(flutter) = state.flutter_executable() else {
    state.session_manager.remove_session(session_id); // undo the create
    tracing::warn!("handle_launch: no Flutter SDK — cannot spawn session");
    return Err("No Flutter SDK found. ...".to_string());
};
```

Either way, **delete the misleading comment** about "no undo API / garbage-collected by capacity eviction."

#### m4 — `save_last_selection` may persist a skipped device (lines ~533, 676–684)

Today `is_primary = (i == 0)` and `save_last_selection` is called inside `spawn_one` only when `is_primary`. But the call sits *after* the active-session skip-check, so:
- If device 0 is skipped (active session), `spawn_one` returns `Err` before the save — and device 1 (`is_primary == false`) never saves either → **no** device persists the default.

**Fix:** Remove `save_last_selection` (and the `is_primary` parameter) from `spawn_one`. In `handle_launch`, after the loop, call it **once** for the first device that successfully launched — track the first successful `(device_id, config_name)` alongside `first_session_id`:

```rust
// In the loop, when spawn_one returns Ok, also capture the first successful device id/config.
// After the loop, before clearing the checked set:
if let Some((dev_id, cfg_name)) = first_success {
    if let Err(e) = crate::config::save_last_selection(&state.project_path, cfg_name.as_deref(), Some(&dev_id)) {
        tracing::warn!("handle_launch: failed to persist last selection: {e}");
    }
}
```

#### m3 — Document the eviction coupling (in `spawn_one` near `create_session_*`)

Add a comment explaining why returning already-built actions on a mid-loop cap hit is safe:

```rust
// Cap handling: create_session_* enforces MAX_SESSIONS, evicting only the
// oldest *Stopped* session. Sessions created earlier in THIS fan-out loop are
// Initializing and therefore never evicted, so already-built actions can never
// reference an evicted session id. If the eviction policy ever changes to evict
// active sessions, this loop must be revisited (dangling-action-id risk).
```

#### m6 — ANSI-strip daemon-sourced strings (toast at lines ~569–583, `summarize_skipped` ~729–739)

`device.name` and the reason strings can originate from `flutter devices` stdout. Strip ANSI before display using the public core helper:

```rust
use fdemon_core::strip_ansi_codes;
// when building the toast / summarize_skipped entries:
format!("{} ({})", strip_ansi_codes(name), strip_ansi_codes(reason))
```

Apply consistently in both the partial-launch toast and `summarize_skipped`.

#### M2 — Cap-hit-mid-loop test

Add a test that exercises the genuine capacity error mid-loop (distinct from the existing active-session skip test). Seed 8 active (`Running`) sessions so one free slot remains, then check 2 fresh devices: the first creates session #9 (success), the second hits `ensure_capacity` Err ("Maximum of 9 concurrent sessions reached"). Use the existing test helpers `state_with_sdk()`, `make_device(id, name)`, and `seed_checked_devices(state, devices)`.

```rust
#[test]
fn launch_partial_when_cap_hit_mid_loop_emits_toast_no_panic() {
    use fdemon_core::AppPhase;
    let mut state = state_with_sdk();
    state.ui_mode = UiMode::NewSessionDialog;

    // Fill 8 of 9 slots with active (non-evictable) sessions.
    for i in 0..8 {
        let d = make_device(&format!("filler-{i}"), &format!("Filler {i}"));
        let sid = state.session_manager.create_session(&d).expect("create");
        state.session_manager.get_mut(sid).unwrap().session.phase = AppPhase::Running;
    }

    // Two fresh checked devices: first fills slot 9, second hits the cap.
    let devices = vec![make_device("dev-a", "Device A"), make_device("dev-b", "Device B")];
    seed_checked_devices(&mut state, devices);

    let result = handle_launch(&mut state);

    assert_eq!(result.actions().len(), 1, "exactly one device should launch before the cap");
    assert!(!state.toasts.is_empty(), "a warn toast should report the skipped device");
    assert_eq!(state.ui_mode, UiMode::Normal, "dialog closes on partial success");
    // (No panic reaching this point is itself part of the assertion.)
}
```

> Confirm the exact `phase` mutation accessor (`get_mut(sid).unwrap().session.phase`) against the existing `launch_skips_device_with_active_session` test and mirror whatever it uses.

### Acceptance Criteria

1. No session remains in `SessionManager` when `spawn_one` fails before spawning (verified by a test asserting session count is unchanged on the no-SDK path).
2. The "no undo API" comment is gone; rollback or fail-fast is in place.
3. `save_last_selection` persists the first *successfully launched* device; a test where device 0 is skipped but device 1 launches asserts device 1's id is persisted.
4. Toast and `summarize_skipped` strings are ANSI-stripped.
5. The eviction-coupling comment is present in `spawn_one`.
6. The cap-hit-mid-loop test passes (partial launch, warn toast, `ui_mode == Normal`, no panic).
7. All existing `launch_context.rs` tests still pass; `cargo test -p fdemon-app` and `cargo clippy --workspace --all-targets -- -D warnings` pass.

### Notes

- Keep the zero-checked single-device path byte-for-byte unchanged (it must still pass the legacy single-launch tests).
- If you hoist the SDK check, ensure the pre-app-sources branch (`SpawnPreAppSources`) still behaves correctly — it does not consume `flutter`, so resolving the SDK early must not change that branch's output.
- Do not address the pre-app shared-source double-trigger here (tracked separately); only the orphan/persist/ANSI/test items are in scope.
