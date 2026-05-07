# Task 08: NewSession Handler Hygiene (Header Guard, Visibility)

## Goal

Two small fixes in the `new_session` handler family:

1. **Minor #9:** `handle_select_device_at` must verify the clamped index points to a `DeviceListItem::Device(_)`, not a header row, before emitting `NewSessionDialogDeviceSelect`.
2. **Minor #18:** Tighten `handler/new_session/mod.rs:11` from `pub mod clicks` to `pub(crate) mod clicks` to match sibling modules.

## Background

**Header guard.** `crates/fdemon-app/src/handler/new_session/clicks.rs:20-32` (current `handle_select_device_at`):
```rust
pub fn handle_select_device_at(state: &mut AppState, index: usize) -> UpdateResult {
    let target = state.new_session_dialog_state.target_selector_state_mut();
    let list_len = target.flat_list().len();
    if list_len == 0 { return UpdateResult::none(); }
    let clamped = index.min(list_len - 1);
    target.selected_index = clamped;
    UpdateResult::message(Message::NewSessionDialogDeviceSelect)
}
```
The flat list includes group headers (e.g., `DeviceListItem::ConnectedHeader`, `DeviceListItem::BootableHeader`). The renderer guards against header rows by only registering click regions for `DeviceListItem::Device(_)` rows. But the handler does not independently verify — if a click message ever reaches the handler with a header index (e.g., clamping into a header at end of list, or future render-time race), `DeviceSelect` fires with `selected_index` on a header.

**Visibility tightening.** `crates/fdemon-app/src/handler/new_session/mod.rs:11` declares `pub mod clicks`. Sibling modules use `mod` (crate-private) with selective re-exports. The `pub` is redundant since `new_session` itself is `pub(crate)`, but the inconsistency may signal intent to make `clicks` more broadly accessible. Tighten to `pub(crate) mod clicks`.

## Files

**Modify:**
- `crates/fdemon-app/src/handler/new_session/clicks.rs`
- `crates/fdemon-app/src/handler/new_session/mod.rs`

**Read (reference):**
- `crates/fdemon-app/src/new_session_dialog/target_selector.rs` — `DeviceListItem` variants, `flat_list` shape

## Plan

1. **Update `handle_select_device_at`** to verify the clamped index is a device:
   ```rust
   pub fn handle_select_device_at(state: &mut AppState, index: usize) -> UpdateResult {
       let target = state.new_session_dialog_state.target_selector_state_mut();
       let list_len = target.flat_list().len();
       if list_len == 0 { return UpdateResult::none(); }
       let clamped = index.min(list_len - 1);

       // Verify the clamped index is a device row, not a header.
       let is_device = matches!(
           target.flat_list().get(clamped),
           Some(DeviceListItem::Device(_))
       );
       if !is_device {
           return UpdateResult::none();
       }

       target.selected_index = clamped;
       UpdateResult::message(Message::NewSessionDialogDeviceSelect)
   }
   ```
   Adjust the import path for `DeviceListItem` if necessary.

2. **Tighten visibility** in `crates/fdemon-app/src/handler/new_session/mod.rs:11`:
   ```rust
   // Before:
   pub mod clicks;
   // After:
   pub(crate) mod clicks;
   ```
   Verify all `crate::handler::new_session::clicks::*` callers in the workspace still compile (they should — `pub(crate)` is broader than they need).

3. **Add a regression test** in `clicks.rs::tests`:
   ```rust
   #[test]
   fn handle_select_device_at_with_header_index_is_no_op() {
       // Build state with a flat_list where index 0 is ConnectedHeader, index 1 is Device.
       let mut state = AppState::new();
       /* ... seed target_selector_state with header at 0, device at 1 ... */
       let target = state.new_session_dialog_state.target_selector_state_mut();
       target.selected_index = 1;  // Start on the device.

       let initial_index = target.selected_index;
       handle_select_device_at(&mut state, 0);  // Click on header row.

       // Selection unchanged; no DeviceSelect emitted.
       assert_eq!(target.selected_index, initial_index);
   }
   ```

4. **Update existing test** that may have relied on the old header-permissive behavior. Search for `handle_select_device_at` in the test suite. If any existing test passes a header index expecting `DeviceSelect` to fire, update it.

5. **Quality gates**:
   ```bash
   cargo test -p fdemon-app handler::new_session::clicks
   cargo test --workspace
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

## Acceptance Criteria

- [ ] `handle_select_device_at` returns `UpdateResult::none()` if the clamped index does not point to a `DeviceListItem::Device(_)`.
- [ ] 1 new regression test passes; existing tests still pass.
- [ ] `handler/new_session/mod.rs:11` declared `pub(crate) mod clicks`.
- [ ] Quality gates pass.

## Notes

- This is a small task (~30min). The header-guard fix is defensive — the renderer already guards against the case, but defending in depth at the handler layer makes the function correct in isolation.
- T01 does not touch `handler/new_session/mod.rs` or `clicks.rs`. T08 owns both files in 5.5.
- The `DeviceListItem` enum may have variants beyond `Device` and the two headers — audit before writing the `matches!` pattern.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/new_session/clicks.rs` | Added `DeviceListItem` import; added header guard in `handle_select_device_at`; added regression test `handle_select_device_at_with_header_index_is_noop`; updated comment on existing clamp test |
| `crates/fdemon-app/src/handler/new_session/mod.rs` | Changed `pub mod clicks` to `pub(crate) mod clicks` |

### Notable Decisions/Tradeoffs

1. **`DeviceListItem` enum has only two variants**: Audited `device_groups.rs` — the enum is `Header(String)` and `Device(T)`. No additional variants exist, so `matches!(..., Some(DeviceListItem::Device(_)))` is exhaustive and correct.
2. **Borrow ordering for cached flat list**: `flat_list()` takes `&mut self` to populate the cache. Both calls (for `len()` and for `.get(clamped)`) release their borrows before `target.selected_index` is mutated, which Rust's NLL handles correctly.
3. **Existing clamp test still passes**: With 2 connected devices the flat list is `[Header, Device, Device]`, so clamping 999 lands on index 2 (a Device row), and `DeviceSelect` still fires as expected. Added a clarifying comment to the test.

### Testing Performed

- `cargo test -p fdemon-app handler::new_session::clicks` - Passed (10 tests, 1 new)
- `cargo test --workspace` - Passed (all result lines showed 0 failed)
- `cargo fmt --all -- --check` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **None**: Both changes are purely defensive/hygienic; no behavior change for the renderer-driven normal path.
