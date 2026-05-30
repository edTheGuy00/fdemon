## Task: Connected-tab empty-state messaging

**Objective**: Add empty-state messages to the Connected device list so the user understands *why* it is empty — distinguishing "no devices discovered at all" from "devices discovered but none are runnable for this project" (all filtered by `is_supported`).

**Depends on**: 01-device-supportability-flag

**Agent:** implementor

**Estimated Time**: 1–2h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs`: Add a Connected-tab empty-state block in `connected_device_list_render_with_regions` (mirroring the existing Bootable pattern at `:441-449`).

**Files Read (Dependencies):**
- `crates/fdemon-app/src/new_session_dialog/device_groups.rs`: `group_connected_devices` / `flatten_groups` behavior (post task-02 filtering).
- `crates/fdemon-daemon/src/devices.rs`: `Device::is_supported` (for understanding the filter, no direct use).

> `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` is listed in the TASKS.md overlap matrix as a possible write only if the message string is better composed at the call site. **Prefer keeping all logic inside `device_list.rs`** — it already has `list.devices` (the full, unfiltered slice) and `items` (the filtered/grouped result), which is everything needed. Touch `target_selector.rs` only if necessary.

### Details

`ConnectedDeviceList` holds `devices: &'a [Device]` — the **full, unfiltered** discovered list (`device_list.rs:29-30`). After task 02, `group_connected_devices(list.devices)` returns only supported devices, so `items` (the flattened list at `device_list.rs:209-210`) is empty in **two** distinct situations the renderer can tell apart using `list.devices`:

| `list.devices` | `items` (post-filter) | State to show |
|----------------|------------------------|---------------|
| empty | empty | "No connected devices" |
| non-empty | empty | "Devices found but none runnable for this project — check enabled platforms" |

Add the block immediately after `let items = flatten_groups(&groups);` (`device_list.rs:210`), before the visible-range calculation, mirroring the Bootable empty-state:

```rust
if items.is_empty() {
    use ratatui::layout::Alignment;
    use ratatui::widgets::Paragraph;

    let msg = if list.devices.is_empty() {
        "No connected devices"
    } else {
        // Devices were discovered but all are unsupported for this project.
        "Devices found but none runnable for this project — check enabled platforms"
    };
    Paragraph::new(msg)
        .alignment(Alignment::Center)
        .style(Style::default().fg(palette::TEXT_MUTED))
        .wrap(ratatui::widgets::Wrap { trim: true })
        .render(area, buf);
    return;
}
```

Use `Wrap` for the longer "none runnable" sentence so it doesn't truncate in a narrow dialog. Keep the muted style consistent with the Bootable message. The early `return` skips scroll indicators and click-region registration (correct — there are no rows).

### Acceptance Criteria

1. With zero discovered connected devices, the Connected tab shows "No connected devices" centered and muted.
2. With ≥1 discovered device but all `is_supported == false` (after task 02's filter), the Connected tab shows the "none runnable" message instead.
3. With ≥1 supported device, rows render as before — no empty-state, no regression to scroll/click-regions.
4. The longer message wraps rather than truncating in a narrow dialog width.
5. `cargo test -p fdemon-tui`, `cargo fmt`, `cargo clippy -p fdemon-tui` pass.

### Testing

Add render tests alongside the existing widget tests in `device_list.rs` (the suite already renders into a `Buffer` and asserts on cell contents — see the existing `content.contains("No bootable devices found")` test at `:636`).

```rust
#[test]
fn connected_empty_shows_no_devices() {
    // devices: &[] → expect "No connected devices"
}

#[test]
fn connected_all_unsupported_shows_none_runnable() {
    // devices: one device with is_supported = false → expect "none runnable"
    // (requires task 02's filter in group_connected_devices to be present)
}

#[test]
fn connected_with_supported_device_renders_rows_not_empty_state() {
    // devices: one supported device → buffer contains the device name, not the empty message
}
```

### Notes

- **Sequencing caveat (parallel with task 02):** the "none runnable" branch only *activates* once task 02's `group_connected_devices` filter is merged — until then `items` is non-empty when unsupported devices exist, so the branch is unreachable but harmless. The render logic here is correct and independent; the `connected_all_unsupported_shows_none_runnable` test will only pass once both tasks are integrated. If developed strictly in isolation before task 02 merges, mark that one test `#[ignore]` with a note, or stub a local filtered fixture.
- Do **not** add a second `is_supported` filter here — task 02 owns the single filter in `group_connected_devices`. This task only reads `list.devices.is_empty()` to choose the message.
- No new keybindings, messages, or state fields.
