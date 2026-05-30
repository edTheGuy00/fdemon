## Task: Connected-tab "N hidden" footer for the partial-unsupported case

**Objective**: Close the MEDIUM UX discoverability gap (m1). Phase 5's empty-state message
only fires when *all* connected devices are filtered out. In the common mixed case (≥1
supported device alongside ≥1 hidden unsupported device), the unsupported device vanishes
with no breadcrumb. Add a muted footer telling the user how many devices were hidden.

**Depends on**: None (reads the already-merged Phase 5 `group_connected_devices` behavior)

**Agent:** implementor

**Estimated Time**: 1–2h

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/new_session_dialog/device_groups.rs`: `group_connected_devices` / `flatten_groups` (already filters unsupported; read-only, no change).

### Details

`ConnectedDeviceList` holds `devices: &'a [Device]` — the **full, unfiltered** discovered
list. After Phase 5, `group_connected_devices(list.devices)` → `flatten_groups` yields only
supported rows (`items`). The number hidden is therefore the count of unsupported devices in
the full list:

```rust
let hidden = list.devices.iter().filter(|d| !d.is_supported).count();
```

Three cases in `connected_device_list_render_with_regions`:

| `items` (post-filter) | `hidden` | Behavior |
|------------------------|----------|----------|
| empty | (handled by existing empty-state block) | unchanged — "No connected devices" / "none runnable" |
| non-empty | `0` | rows only, no footer (unchanged) |
| non-empty | `> 0` | rows **plus** a muted footer: "(N hidden: not runnable for this project)" |

**Placement.** The footer must live inside the allocated `area` and must not overlap the
device rows or the existing scroll indicators. Follow the responsive-layout guidance in
`docs/CODE_STANDARDS.md` (Principle 2 — include every element in the layout system; never
compute a `Rect` by offset arithmetic). Reserve a 1-row slot for the footer when
`hidden > 0` by splitting `area` with `Layout::vertical` (list area + `Constraint::Length(1)`
footer), and render rows into the list sub-area. Keep the muted style
(`Style::default().fg(palette::TEXT_MUTED)`) consistent with the empty-state messages.
Singular/plural is acceptable to keep simple ("1 hidden" is fine), but prefer a clear phrase:

```rust
let footer = format!("({hidden} hidden: not runnable for this project)");
```

**Click-regions / scroll.** Reserving the footer row reduces the list viewport by 1 when
`hidden > 0`. Ensure the visible-range / scroll math and per-row click-region registration
use the **reduced** list area height so rows, cursor, and click hit-tests stay aligned (do
not register a click region over the footer row). This is the main correctness risk — verify
with a render test that a click maps to the right device when the footer is present.

> Do **not** add a second `is_supported` filter here — Phase 5 owns the single filter in
> `group_connected_devices`. This task only *counts* `!is_supported` in the full list for the
> footer text and reserves a row for it.

**Also fold in the two device_list.rs nitpicks while here:**
- **n2** — hoist the scoped `use ratatui::layout::Alignment;` / `use ratatui::widgets::Paragraph;`
  from inside the empty-state block to the top-of-file imports, matching the rest of the module.
- **n4** — replace the `\u{2014}` escape in the existing "none runnable" message with the
  literal em dash `—`.

### Acceptance Criteria

1. Mixed case (≥1 supported + ≥1 unsupported): device rows render **and** a muted "(N hidden: not runnable for this project)" footer shows, with the correct N.
2. All-supported case (`hidden == 0`): no footer, rendering identical to today.
3. All-unsupported and zero-devices cases: unchanged (existing empty-state block still wins; no footer).
4. With the footer present, scroll/visible-range and per-row click-regions remain correct — a click still selects the intended device (no off-by-one), and no click region covers the footer row.
5. The footer and rows all render within the allocated `area` (no offset-arithmetic `Rect`).
6. n2 (imports hoisted) and n4 (literal em dash) applied.
7. `cargo test -p fdemon-tui`, `cargo fmt`, `cargo clippy -p fdemon-tui -- -D warnings` pass.

### Testing

Add render tests alongside the existing `device_list.rs` widget tests (which render into a
`Buffer` and assert on cell contents):

```rust
#[test]
fn connected_mixed_shows_hidden_footer() {
    // devices: one supported + one unsupported → buffer contains the supported device
    // name AND "1 hidden"
}

#[test]
fn connected_all_supported_has_no_hidden_footer() {
    // devices: two supported → buffer must NOT contain "hidden"
}

#[test]
fn connected_click_maps_correctly_with_footer_present() {
    // mixed case → registered click region for row 0 maps to the supported device index,
    // and no region is registered on the footer row
}
```

### Notes

- The footer is presentation-only; no new state, message, or keybinding.
- Keep the message wording aligned with the existing "none runnable" empty-state for consistency.
- If reserving the footer row meaningfully shrinks a tiny dialog, the `Constraint::Min(0)` list slot + `Length(1)` footer ordering ensures the list absorbs remaining space and the footer is the first to clip gracefully under extreme height pressure.
