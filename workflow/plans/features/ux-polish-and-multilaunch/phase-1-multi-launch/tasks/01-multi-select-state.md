## Task: Multi-select state model in TargetSelectorState

**Objective**: Add an identity-keyed multi-selection set to `TargetSelectorState` (independent of the cursor), with toggle / select-all / clear / query operations scoped to the Connected tab, surviving list refreshes.

**Depends on**: None

**Estimated Time**: 2–3h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`: add the selection set, methods, and stale-id pruning; add unit tests.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/new_session_dialog/device_groups.rs`: `DeviceListItem`, `group_connected_devices`, `flatten_groups` — already used here for cursor logic.

### Details

Add a checked set keyed by device `id`. Use a `BTreeSet<String>` for deterministic iteration/test output.

```rust
use std::collections::BTreeSet;

pub struct TargetSelectorState {
    // ... existing fields ...

    /// Device ids checked for multi-launch (Connected tab only). Independent
    /// of `selected_index` (the cursor). Keyed by id so it survives refreshes.
    pub checked_device_ids: BTreeSet<String>,
}
```

Initialize empty in `Default`.

Methods (Connected-tab scoped — no-op on Bootable):

```rust
/// Toggle the cursor device's checked state. Connected tab only; ignores headers.
pub fn toggle_checked_cursor(&mut self) {
    if self.active_tab != TargetTab::Connected { return; }
    if let Some(id) = self.selected_device_id() {
        if !self.checked_device_ids.remove(&id) {
            self.checked_device_ids.insert(id);
        }
    }
}

/// Select all connected devices, or clear if all are already checked.
pub fn toggle_select_all(&mut self) {
    if self.active_tab != TargetTab::Connected { return; }
    let all_ids: Vec<String> = self.connected_devices.iter().map(|d| d.id.clone()).collect();
    let all_checked = !all_ids.is_empty()
        && all_ids.iter().all(|id| self.checked_device_ids.contains(id));
    if all_checked {
        self.checked_device_ids.clear();
    } else {
        self.checked_device_ids = all_ids.into_iter().collect();
    }
}

pub fn clear_checked(&mut self) { self.checked_device_ids.clear(); }
pub fn is_checked(&self, device_id: &str) -> bool { self.checked_device_ids.contains(device_id) }
pub fn checked_count(&self) -> usize { self.checked_device_ids.len() }

/// Checked devices in list order (skips any id no longer present).
pub fn checked_devices(&self) -> Vec<&Device> {
    self.connected_devices.iter()
        .filter(|d| self.checked_device_ids.contains(&d.id))
        .collect()
}
```

**Pruning:** in `set_connected_devices()`, after assigning the new list, retain only ids that still exist:

```rust
let present: std::collections::HashSet<&str> =
    self.connected_devices.iter().map(|d| d.id.as_str()).collect();
self.checked_device_ids.retain(|id| present.contains(id.as_str()));
```

(`selected_device_id()` already exists and returns the cursor device id on the Connected tab — reuse it for `toggle_checked_cursor`.)

### Acceptance Criteria

1. `checked_device_ids` defaults empty and survives `clone()`.
2. `toggle_checked_cursor` adds then removes the cursor device's id; no-op on a header or Bootable tab.
3. `toggle_select_all` checks all connected ids when not all checked; clears when all already checked; no-op on Bootable.
4. `checked_devices()` returns devices in list order and excludes pruned ids.
5. `set_connected_devices()` drops ids absent from the new list and keeps ids still present.
6. `is_checked` / `checked_count` reflect the set accurately.

### Testing

```rust
#[test]
fn toggle_checked_cursor_adds_then_removes() { /* set Connected, cursor on a device, toggle twice */ }

#[test]
fn select_all_then_clear_roundtrip() { /* all checked -> toggle_select_all clears */ }

#[test]
fn set_connected_devices_prunes_missing_checked_ids() {
    // check A and B, then set list to [B, C] -> only B remains checked
}

#[test]
fn multi_select_ops_noop_on_bootable_tab() { /* active_tab = Bootable -> toggle/select_all do nothing */ }
```

### Notes

- Do not couple the checked set to `selected_index`; the cursor and the selection are orthogonal.
- Keep the API minimal and pure (no rendering/handler concerns) so tasks 02/03/04 can consume it without further state changes.
- Forward-compat with Phase 5: when `Device` gains `is_supported`, `toggle_checked_cursor` and `toggle_select_all` should skip unsupported devices. Leave a `// TODO(phase-5)` marker rather than implementing now.

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | Added `checked_device_ids: BTreeSet<String>` field, 6 new methods (`toggle_checked_cursor`, `toggle_select_all`, `clear_checked`, `is_checked`, `checked_count`, `checked_devices`), pruning in `set_connected_devices`, and 10 unit tests |

### Notable Decisions/Tradeoffs

1. **Test cursor positioning**: The flat list for Connected tab always starts with a platform group header, so `selected_index = 0` does not point to a device. The `toggle_checked_cursor_adds_then_removes` test explicitly calls `select_device_by_id("dev-a")` before checking the cursor — this matches the real usage pattern where navigation helpers always land on device rows.

2. **`use std::collections::BTreeSet` at top-level**: Added as a module-level import rather than using the fully-qualified path inside the method body, keeping the implementation method bodies clean.

3. **`TODO(phase-5)` markers**: Left on both `toggle_checked_cursor` and `toggle_select_all` per the task spec, so Phase 5 `is_supported` filtering can be threaded in without API changes.

### Testing Performed

- `cargo test -p fdemon-app -- new_session_dialog::target_selector_state` — 19 passed, 0 failed
- `cargo test -p fdemon-app` — 2557 passed, 0 failed
- `cargo check --workspace --all-targets` — Passed
- `cargo fmt --all -- --check` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **No keybinding wired yet**: The multi-select state is pure model — tasks 02/03/04 wire up keybindings and rendering. Consumers will need to call `toggle_checked_cursor()` / `toggle_select_all()` from the handler.
