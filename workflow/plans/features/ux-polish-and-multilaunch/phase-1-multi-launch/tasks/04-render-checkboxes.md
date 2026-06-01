## Task: Render checkboxes & selection hints

**Objective**: Show a checkbox per connected device, reflect the checked set visually, surface the checked count, and update the footer hint to advertise `Space` / `a`.

**Depends on**: 01-multi-select-state

**Estimated Time**: 2–3h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs`: extend `ConnectedDeviceList` to know which device ids are checked and render a `[x]`/`[ ]` prefix on device rows.
- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`: pass the checked set into `ConnectedDeviceList`, render the checked count, and update the footer hint.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`: `is_checked`, `checked_count`, `checked_device_ids`.

### Details

**`ConnectedDeviceList`** (`device_list.rs`): add a borrowed checked set and render the box. The cursor highlight (`is_selected`) is unchanged; the checkbox is an independent prefix.

```rust
pub struct ConnectedDeviceList<'a> {
    devices: &'a [Device],
    selected_index: usize,
    is_focused: bool,
    scroll_offset: usize,
    checked: &'a std::collections::BTreeSet<String>,  // NEW
    icons: IconSet,
}
```

Update `new(..)` to take `checked: &'a BTreeSet<String>` (or add a `with_checked(set)` builder to avoid churning all call sites — prefer the builder if `new` has many callers).

In `render_item` for `DeviceListItem::Device(device)`, prepend the checkbox:

```rust
let box_glyph = if self.checked.contains(&device.id) { "[x] " } else { "[ ] " };
// styled with palette::ACCENT when checked, palette::TEXT_MUTED otherwise,
// placed before the platform icon span. Account for its width (4 cols) in the
// `reserved` width calc so name truncation stays correct.
```

Keep clickable-region recording (`connected_device_list_render_with_regions`) intact — the checkbox is inside the existing row rect.

**`target_selector.rs` widget:** when building `ConnectedDeviceList`, pass `&state.target_selector.checked_device_ids`. Update the footer hint and (optionally) the pane title:

```text
// hint, was: "[Enter] Select  [r] Refresh"
"Space select · a all · Enter launch · r refresh"
// when checked_count() > 0, show e.g. "(2 selected)" in the pane title or hint
```

### Acceptance Criteria

1. Each connected device row renders a checkbox prefix; checked devices show `[x]`, others `[ ]`.
2. The checkbox state tracks `target_selector.is_checked(device.id)`; toggling via `Space` (task 02) flips the glyph on next render.
3. The cursor highlight and existing icon/name/type layout still render correctly; name truncation accounts for the checkbox width.
4. The footer hint advertises `Space` and `a`; the checked count is visible when > 0.
5. Headers render without a checkbox.

### Testing

```rust
#[test]
fn renders_checkbox_for_each_device() {
    // build ConnectedDeviceList with an empty checked set -> output contains "[ ]"
}

#[test]
fn renders_checked_glyph_for_checked_device() {
    // checked set contains device id -> output contains "[x]"
}
```

(Use the buffer-to-string rendering pattern already present in the widget tests — render into a `Buffer` and scan symbols.)

### Notes

- Checkbox is orthogonal to the cursor highlight; both can be active on the same row.
- Prefer a `with_checked(..)` builder if `ConnectedDeviceList::new` has multiple call sites, to minimize churn and conflict surface.
- This task is TUI-only; it does not import from the app handler layer beyond the existing `&TargetSelectorState` the widget already receives.
- Bootable list rendering is unchanged (no multi-select there).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a9f7ce0b4ee1242d4

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` | Added `checked: Option<&'a BTreeSet<String>>` field to `ConnectedDeviceList`; added `with_checked()` builder; updated `render_item` to prepend `[x]`/`[ ]` checkbox span per device row with ACCENT/TEXT_MUTED colours; deducts checkbox width (4 cols) from reserved width for correct name truncation; added 4 unit tests |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | Pass `&state.checked_device_ids` via `.with_checked()` in both `render_full` and `render_compact`; updated `render_footer` to show `"Space select · a all · Enter launch · r refresh"` (with `(N selected)` suffix when count > 0) |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Updated inline `render_target_selector_regions` path: pass `with_checked()` for `ConnectedDeviceList`; update inline footer string to match the new hint |

### Notable Decisions/Tradeoffs

1. **Builder pattern (`with_checked`)**: `ConnectedDeviceList::new` has multiple call sites (TargetSelector widget, NewSessionDialog region-aware path). Using a builder avoids churning `new`'s signature and minimises merge surface — consistent with the task's instruction.
2. **Checkbox style**: Checked rows use `ACCENT` colour; unchecked use `TEXT_MUTED`. The checkbox is rendered as a plain `Span` before the existing icon prefix span, orthogonal to the cursor highlight.
3. **Backward compatibility**: `checked` defaults to `None`; without `with_checked()` no checkbox characters appear, preserving the existing rendering contract for any future callers.
4. **Header rows**: The match arm for `DeviceListItem::Header` is unchanged — checkbox logic is entirely inside the `DeviceListItem::Device` arm.

### Testing Performed

- `cargo check -p fdemon-tui` — Passed
- `cargo test -p fdemon-tui --lib` — Passed (1301 tests)
- `cargo test --workspace --lib` — Passed (6029 tests across all crates)
- `cargo clippy -p fdemon-tui` — No warnings

### Risks/Limitations

1. **Width accounting uses byte length for `icon_prefix`**: The existing code uses `prefix.len()` (bytes) not char count for width reservation. The task preserves this behaviour unchanged — emoji icons are ASCII substitutes in the Unicode icon set so the byte length matches the display width. If Nerd Font glyphs are later added with multi-byte sequences this could need revisiting, but that is a pre-existing issue outside this task's scope.
