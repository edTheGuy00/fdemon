## Task: Filter unsupported devices from the Connected list and multi-select

**Objective**: Exclude `is_supported == false` connected devices from the new-session dialog's Connected tab at the single shared grouping chokepoint, and resolve the two Phase-1 `TODO(phase-5)` guards so multi-select can never check an unsupported device.

**Depends on**: 01-device-supportability-flag

**Agent:** implementor

**Estimated Time**: 2–3h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/new_session_dialog/device_groups.rs`: Add an `is_supported` filter inside `group_connected_devices` (the shared chokepoint used by both state and the TUI widget).
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`: Resolve `TODO(phase-5)` in `toggle_checked_cursor` (`:389`) and `toggle_select_all` (`:407`); add a safety-net filter to `checked_devices` (`:447`).

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/devices.rs`: the new `Device::is_supported` field.

### Details

**1. Filter at the shared chokepoint (`device_groups.rs:131-144`).**
`group_connected_devices` is called by *three* places that must stay index-aligned: `compute_flat_list` (`target_selector_state.rs:174`), `selected_connected_device` (`:209`), and the TUI widget `connected_device_list_render_with_regions` (`device_list.rs:209`). Filtering here — and only here — keeps the flat list, cursor, checked-set, and click-regions consistent.

```rust
pub fn group_connected_devices(devices: &[Device]) -> Vec<DeviceGroup<&Device>> {
    let mut groups: BTreeMap<PlatformGroup, Vec<&Device>> = BTreeMap::new();

    for device in devices.iter().filter(|d| d.is_supported) {
        let group = PlatformGroup::from_device(device);
        groups.entry(group).or_default().push(device);
    }
    // ...unchanged
}
```

`TargetSelectorState::connected_devices` still stores the *full* discovered list (needed so task 03 can tell "found N but all unsupported" from "found 0"); only the grouped/flattened view excludes unsupported devices.

**2. Guard `toggle_checked_cursor` (`target_selector_state.rs:391-400`).** After resolving the cursor to a device id, look the device up in `connected_devices` and early-return if it is not supported (defensive — the cursor should never land on an unsupported row once it is filtered out of the flat list, but the id-based lookup makes the guard robust):

```rust
pub fn toggle_checked_cursor(&mut self) {
    if self.active_tab != TargetTab::Connected {
        return;
    }
    if let Some(id) = self.selected_device_id() {
        // Phase 5: never check an unsupported device.
        if !self.is_connected_device_supported(&id) {
            return;
        }
        if !self.checked_device_ids.remove(&id) {
            self.checked_device_ids.insert(id);
        }
    }
}
```

Add a small private helper:

```rust
fn is_connected_device_supported(&self, id: &str) -> bool {
    self.connected_devices
        .iter()
        .find(|d| d.id == id)
        .map(|d| d.is_supported)
        .unwrap_or(false)
}
```

**3. Guard `toggle_select_all` (`target_selector_state.rs:408-426`).** Restrict the candidate id set to supported devices so "select all" only checks runnable targets and the "all checked?" test compares against the same subset:

```rust
let all_ids: Vec<String> = self
    .connected_devices
    .iter()
    .filter(|d| d.is_supported)
    .map(|d| d.id.clone())
    .collect();
```

**4. Safety net in `checked_devices` (`target_selector_state.rs:447-452`).** Add `.filter(|d| d.is_supported)` so an unsupported device can never reach launch even if its id were somehow already in `checked_device_ids` (e.g. checked before a refresh flipped the flag):

```rust
pub fn checked_devices(&self) -> Vec<&Device> {
    self.connected_devices
        .iter()
        .filter(|d| self.checked_device_ids.contains(&d.id) && d.is_supported)
        .collect()
}
```

> **Do NOT** add a filter in the TUI widget (task 03's territory) or in `compute_flat_list`/`selected_connected_device` — they already route through `group_connected_devices` and would double-filter.

### Acceptance Criteria

1. A connected device list containing an `is_supported == false` device renders/flattens **without** that device; supported devices keep their relative order and grouping.
2. `selected_connected_device` and `compute_flat_list` never surface an unsupported device (verified via the shared `group_connected_devices` change).
3. `toggle_checked_cursor` does nothing when the cursor id maps to an unsupported device; `toggle_select_all` checks only supported devices and its all-on/all-off toggle is computed over the supported subset.
4. `checked_devices()` never returns an unsupported device.
5. Both `TODO(phase-5)` comments are removed/resolved.
6. `cargo test -p fdemon-app`, `cargo fmt`, `cargo clippy -p fdemon-app` pass.

### Testing

Add unit tests in `target_selector_state.rs` (and/or `device_groups.rs`). Build a `connected_devices` vec mixing supported and unsupported entries:

```rust
#[test]
fn group_connected_excludes_unsupported() {
    let devices = vec![
        device("a", true),
        device("b", false), // unsupported
        device("c", true),
    ];
    let groups = group_connected_devices(&devices);
    let ids: Vec<&str> = groups.iter()
        .flat_map(|g| g.devices.iter().map(|d| d.id.as_str()))
        .collect();
    assert_eq!(ids, vec!["a", "c"]);
}

#[test]
fn select_all_skips_unsupported() {
    let mut state = state_with(vec![device("a", true), device("b", false)]);
    state.toggle_select_all();
    let checked: Vec<&str> = state.checked_devices().iter().map(|d| d.id.as_str()).collect();
    assert_eq!(checked, vec!["a"]); // "b" never checked
}

#[test]
fn checked_devices_drops_unsupported_safety_net() {
    let mut state = state_with(vec![device("a", true)]);
    state.checked_device_ids.insert("ghost".into()); // simulate stale/unsupported id
    assert!(state.checked_devices().iter().all(|d| d.is_supported));
}
```

(Use the existing test helpers/fixtures in this module; `device(id, is_supported)` is illustrative.)

### Notes

- **Single chokepoint is the whole point** — see the TASKS.md "Critical design note." The TUI widget calls `group_connected_devices` directly, so the filter there fixes rendering and click-regions for free.
- `set_connected_devices` already prunes stale checked ids on refresh (`:242-249`); the `checked_devices` safety net additionally covers a device whose `is_supported` flips false on a later poll without its id changing.
- No keybinding or message changes — the guards are internal to existing handlers.

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/new_session_dialog/device_groups.rs` | Added `is_supported` filter inside `group_connected_devices` (the shared chokepoint); updated doc comment. |
| `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | Resolved both `TODO(phase-5)` guards in `toggle_checked_cursor` and `toggle_select_all`; added `.is_supported` to `checked_devices()` safety net; added private `is_connected_device_supported()` helper; added 6 new unit tests. |

### Notable Decisions/Tradeoffs

1. **Single chokepoint**: The filter is placed only in `group_connected_devices` as specified. `compute_flat_list`, `selected_connected_device`, and the TUI widget all route through this function and pick up the filter for free with no double-filtering risk.
2. **`connected_devices` stores the full list**: Unsupported devices remain in `TargetSelectorState::connected_devices` so task 03 can distinguish "found N but all unsupported" from "found 0". Only the grouped/rendered view excludes them.
3. **`is_connected_device_supported` private helper**: The id-based lookup is robust even if a cursor somehow resolved to an unsupported device (edge case). Tests exercise this directly.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed (0 warnings)
- `cargo test -p fdemon-app` — Passed (2618 tests, 6 new Phase-5 tests)
- `cargo test --workspace` — Passed (all crates, 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **None**: All acceptance criteria met. Both `TODO(phase-5)` comments removed, single chokepoint filter in place, safety net added.
