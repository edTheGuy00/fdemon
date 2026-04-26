# Task 02 — `set_error()` Doc Accuracy (F3)

**Agent:** implementor
**Phase:** 1
**Depends on:** none
**Files Modified (Write):**
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`

---

## Goal

Fix Minor finding F3 from PR #37's Copilot review: the doc comment on `set_error()`
inaccurately claims the helper is invoked only from the connected-device foreground
discovery failure path. In reality it has 13 call sites spanning boot failures,
launch-context errors, validation failures, and SDK-not-found paths.

This is a pure doc rewrite. **No behavior change.**

## Context

Current doc comment at
`crates/fdemon-app/src/new_session_dialog/target_selector_state.rs:271-278`:

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
    self.error = Some(error);
    self.loading = false;
    self.refreshing = false;
}
```

Verified callers (via `grep -rn "set_error" crates/`):

- `handler/update.rs:427` — `Message::DeviceDiscoveryFailed { is_background: false }` (the only path the current doc describes)
- `handler/update.rs:982` — session creation failure
- `handler/update.rs:997` — session creation error
- `handler/update.rs:1272` — boot device failure
- `handler/new_session/target_selector.rs:170` — selection error
- `handler/new_session/target_selector.rs:202` — boot failure from selector
- `handler/new_session/launch_context.rs:416` — "Device no longer available"
- `handler/new_session/launch_context.rs:430` — generic launch failure
- `handler/new_session/launch_context.rs:532` — "No Flutter SDK found ..."
- `handler/new_session/launch_context.rs:550` — session creation failure (launch path)
- `handler/new_session/launch_context.rs:584` — config save error
- `handler/new_session/launch_context.rs:608` — config save error (alternate)
- (Plus task 01 of this plan, which adds another call from `navigation.rs` for the
  SDK-missing path. Implementor should not assume this commit is in their worktree.)

The reviewer provided suggested replacement wording in the PR comment; this task
adopts that wording with light edits.

## Steps

1. Open `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` and
   locate the `set_error()` doc comment (around line 271).

2. Replace the doc comment with the following (the reviewer's suggested wording,
   lightly edited for project tone):

   ```rust
   /// Set a new-session dialog error state.
   ///
   /// This helper is used by many new-session error paths, not just the
   /// connected-device foreground discovery failure path. Callers include device
   /// discovery failures, session creation failures, boot failures, config save
   /// errors, "no Flutter SDK" surfaces from the launch context and dialog open,
   /// and several validation paths.
   ///
   /// It records the error and clears the connected-side `loading` and
   /// `refreshing` flags so the UI does not remain stuck in a connected
   /// in-progress state after an error is surfaced.
   ///
   /// `bootable_loading` and `bootable_refreshing` are intentionally **not**
   /// cleared here. Bootable discovery is independent (xcrun/emulator tools,
   /// not the Flutter SDK) and its in-flight flags are managed by their own
   /// success/failure paths. Callers that need to clear bootable indicators on
   /// a particular error must do so themselves.
   pub fn set_error(&mut self, error: String) {
       self.error = Some(error);
       self.loading = false;
       self.refreshing = false;
   }
   ```

3. **Do not change the function body.** Only the doc comment changes. The clearing
   semantics (`loading` and `refreshing` cleared, bootable flags untouched) are
   correct and intentional.

4. **Do not** add or remove tests. Existing tests
   (`test_set_error_clears_refreshing` at line 445, etc.) cover the behavior.

5. Run verification:
   - `cargo fmt --all`
   - `cargo check -p fdemon-app`
   - `cargo test -p fdemon-app --lib`
   - `cargo clippy -p fdemon-app --lib -- -D warnings`

## Acceptance Criteria

- [ ] `set_error()`'s doc comment no longer claims the helper is invoked only from
      `Message::DeviceDiscoveryFailed { is_background: false }`.
- [ ] The new doc comment lists the broad categories of callers (discovery,
      session creation, boot, launch-context, config, SDK-not-found).
- [ ] The new doc comment explicitly states `bootable_loading` and
      `bootable_refreshing` are not cleared and explains why (independent
      discovery path).
- [ ] The function body is unchanged.
- [ ] `cargo test -p fdemon-app --lib` passes (no test changes needed).
- [ ] `cargo clippy -p fdemon-app --lib -- -D warnings` clean.

## Out of Scope

- Changing `set_error()`'s clearing semantics (e.g. extending it to clear bootable
  flags). The current behavior is correct; bootable discovery is independent.
- Refactoring callers to a new helper (e.g. `set_error_clear_all`). Out of scope
  per the BUG.md.
- Adding tests for the comment wording. Doc-only changes don't warrant test
  additions.

---

## Completion Summary

**Status:** Done
**Branch:** fix/remove-cache-device-ttl

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | Replaced `set_error()` doc comment with accurate multi-caller description |

### Notable Decisions/Tradeoffs

1. **Adopted reviewer's suggested wording verbatim**: The task specified adopting the PR #37 reviewer's language with light edits. The replacement comment accurately lists the broad caller categories and explicitly names both `bootable_loading` and `bootable_refreshing` as intentionally not cleared.
2. **No behavior change**: The function body (`self.error = Some(error); self.loading = false; self.refreshing = false;`) was not touched.

### Testing Performed

- `cargo fmt --all` - Passed (no changes needed)
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app --lib` - Passed (1895 tests, 0 failed)
- `cargo clippy -p fdemon-app --lib -- -D warnings` - Passed (clean)

### Risks/Limitations

1. **None**: This is a pure doc rewrite with no semantic changes. All acceptance criteria are met.
