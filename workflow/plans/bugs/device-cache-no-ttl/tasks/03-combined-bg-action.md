# Task 03 — Combined Background Refresh UpdateAction

**Agent:** implementor
**Phase:** 1
**Depends on:** none
**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mod.rs`
- `crates/fdemon-app/src/actions/mod.rs`

**Files Read:** `crates/fdemon-app/src/spawn.rs`

---

## Goal

Introduce a single `UpdateAction` variant that triggers **both** the connected-device
background refresh and the bootable-device background refresh. `UpdateResult` carries
at most one action, and the dialog needs to refresh both lists on open, so combining
the two discoveries in one variant is the simplest approach.

## Steps

1. Open `crates/fdemon-app/src/handler/mod.rs`.

2. **Add a new variant** to the `UpdateAction` enum, alongside
   `RefreshDevicesBackground` (around line 69):

   ```rust
   /// Refresh both connected and bootable device lists in the background.
   ///
   /// Dispatched when the new-session dialog opens with cached data already
   /// shown, so that both lists are kept fresh without a loading screen.
   /// Errors on either side are logged only; the user keeps seeing the
   /// previous device lists until the discovery returns.
   RefreshDevicesAndBootableBackground { flutter: FlutterExecutable },
   ```

   Place it directly after the existing `RefreshDevicesBackground` variant for
   readability.

3. Open `crates/fdemon-app/src/actions/mod.rs`.

4. **Wire the variant** in the action dispatch `match`. Insert a new arm next to the
   existing `RefreshDevicesBackground` arm (around line 79) and the
   `DiscoverBootableDevices` arm (around line 126):

   ```rust
   UpdateAction::RefreshDevicesAndBootableBackground { flutter } => {
       // Connected device refresh — errors logged only (UI shows cached list).
       spawn::spawn_device_discovery_background(msg_tx.clone(), flutter);
       // Bootable refresh — errors logged only.
       spawn::spawn_bootable_device_discovery(msg_tx, tool_availability);
   }
   ```

   Note: `tool_availability` is already a parameter on the action-dispatch function
   (used by `DiscoverBootableDevices`); re-use it here. `msg_tx` is cloned because
   both spawn calls take ownership.

5. **Verify** that `spawn_bootable_device_discovery` accepts the `tool_availability`
   value as it stands at dispatch time (i.e. it's `Clone` or already owned by the
   dispatcher). Check `actions/mod.rs:127` for the existing call signature — the new
   arm should mirror it.

6. **No new tests required at the action layer** — `UpdateAction` is a plain enum with
   no logic. The combined behaviour is exercised by tests in tasks 04 and the
   end-to-end dialog tests.

## Acceptance Criteria

- [ ] `UpdateAction::RefreshDevicesAndBootableBackground { flutter }` exists in
      `handler/mod.rs`.
- [ ] The action-dispatch match in `actions/mod.rs` handles the new variant by calling
      both `spawn_device_discovery_background` and `spawn_bootable_device_discovery`.
- [ ] `cargo build --workspace` succeeds with no exhaustive-match warnings.
- [ ] `cargo test -p fdemon-app --lib` passes.

## Out of Scope

- Dispatching the new action from anywhere (handled in task 04).
- Removing the existing `RefreshDevicesBackground` variant — keep it; other callers
  may exist (e.g. session lifecycle) and we don't want to change their semantics in
  this fix.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-af5fb4d58eb6bbf6e

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mod.rs` | Added `RefreshDevicesAndBootableBackground { flutter }` variant to `UpdateAction` enum, placed after `RefreshDevicesBackground` |
| `crates/fdemon-app/src/actions/mod.rs` | Added match arm for `RefreshDevicesAndBootableBackground` calling both `spawn_device_discovery_background` (with cloned `msg_tx`) and `spawn_bootable_device_discovery` |

### Notable Decisions/Tradeoffs

1. **Clone order**: `msg_tx.clone()` is passed to `spawn_device_discovery_background` while the original `msg_tx` is consumed by `spawn_bootable_device_discovery`, mirroring the exact pattern in the task spec.
2. **No new tests**: As specified in the task, `UpdateAction` is a plain enum with no logic — the combined behavior is exercised by downstream task 04 tests.

### Testing Performed

- `cargo build --workspace` - Passed (full workspace, no exhaustive-match warnings)
- `cargo test -p fdemon-app --lib` - Passed (1884 passed; 0 failed; 4 ignored)

### Risks/Limitations

1. **No callers yet**: The new variant is not dispatched from anywhere until task 04 wires it up on dialog open.
