## Task: Remove redundant platform-letter icons from device rows

**Objective**: Drop the `[M]` / `[W]` / `[D]` platform-letter icon prefix from connected and bootable device rows. The platform group headers (`IOS DEVICES`, `WEB`, `DESKTOP`, …) already label each group, and the new `[ ]` multi-select checkbox now competes for horizontal width — the per-row icon is pure redundancy.

**Depends on**: None

**Estimated Time**: 2–3h

**Addresses review item**: UI #8 (user request)

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs`: remove the icon-prefix span from both list renderers, update width math, delete the now-dead icon helpers/field/builder, update tests.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`: confirms `ConnectedDeviceList`/`BootableDeviceList` construction sites (no `.with_icons()` callers).
- `crates/fdemon-tui/src/theme/icons.rs`: `[M]`/`[W]`/`[D]` glyph source (`smartphone`/`globe`/`monitor`), for context only.

### Details

Research findings (grounding the dead-code removal):
- `[M]` = `icons.smartphone()` (`icons.rs:41`), `[W]` = `icons.globe()` (`:48`), `[D]` = `icons.monitor()` (`:55`) — `IconMode::Unicode`.
- The icon prefix is rendered as the `" {icon} "` span in the connected list (`device_list.rs:170`) and bootable list (`device_list.rs:380`).
- Group headers are **always** rendered (`group_connected_devices` filters empty groups; `flatten_groups` pushes a `Header` before every group, in both full and compact layouts) — so removing per-row icons never leaves a platform unlabeled.
- `with_icons` has **zero external call sites**; `icons: IconSet` is only consumed by `device_icon`/`bootable_device_icon`, which are only called to build the prefix. Removing the prefix makes all four dead.

**Connected list `render_item` (Device arm, ~lines 121–193):**
- Remove the `icon`/`icon_prefix` computation and the `spans.push(Span::styled(icon_prefix, style))` for the icon.
- Keep the checkbox span (`[x] `/`[ ] `), the name span, and the type suffix.
- Update `reserved`: drop the `icon_prefix.len()` term so the name reclaims those columns:
  ```rust
  let reserved = checkbox_width + type_suffix.len();
  ```

**Bootable list `render_item` (Device arm, ~lines 357–398):**
- Remove the `prefix` (icon) span and its `bootable_device_icon` call.
- Update `reserved = runtime_suffix.len();` (no checkbox on bootable rows).
- Keep name + runtime suffix.

**Dead-code cleanup (after the spans are gone):**
- Delete `device_icon` (`~lines 29–45`) and `bootable_device_icon` (`~lines 48–58`).
- Remove the `icons: IconSet` field from both `ConnectedDeviceList` and `BootableDeviceList`, drop its initialization in `new()`, and delete the `with_icons` builders on both.
- Remove now-unused imports (`IconSet`, `IconMode`) if nothing else in the file uses them. Leave `palette` import intact.
- Leave `crates/fdemon-tui/src/widgets/header.rs::device_icon_for_platform` untouched — it is a separate function.

> Consider whether to leave a 1–2 column indent in place of the removed icon so device rows still visually nest under their header. The existing header indent is two spaces (`Span::raw("  ")`); match the device rows' left edge to whatever looks aligned. This is a visual judgment — keep the checkbox/name readable and aligned under the header.

### Acceptance Criteria

1. No `[M]`/`[W]`/`[D]` (or any platform icon glyph) renders on connected or bootable device rows in either full or compact layout.
2. The checkbox (`[x]`/`[ ]`) and device name shift left to reclaim the freed width; name truncation accounts for the removed prefix (no over- or under-reservation).
3. Platform group headers still render for every group (unchanged).
4. `device_icon`, `bootable_device_icon`, the `icons` field, and `with_icons` are removed with no remaining references; no dead-code or unused-import warnings.
5. Widget tests updated: assert the icon glyph is **absent** from rendered output and that the checkbox/name render at the expected columns. Existing checkbox tests (`renders_checkbox_for_each_device`, `renders_checked_glyph_for_checked_device`) still pass (adjusted for the new layout if they asserted icon presence).
6. `cargo test -p fdemon-tui` and `cargo clippy --workspace --all-targets -- -D warnings` pass.

### Notes

- TUI-only change; no app/handler/state changes.
- If any existing test asserts on the `[M]`/`[W]`/`[D]` text, update it to assert absence instead.
- Bootable list keeps its existing layout otherwise (no checkbox there — multi-select is Connected-tab only).

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` | Removed `device_icon`, `bootable_device_icon` functions; removed `icons: IconSet` field and `with_icons` builder from both `ConnectedDeviceList` and `BootableDeviceList`; removed icon prefix spans from both `render_item` methods; updated `reserved` width math in both lists; removed unused `IconSet`, `IconMode` imports |

### Notable Decisions/Tradeoffs

1. **No visual indent added**: The task noted considering adding a 1–2 column indent in place of the icon to maintain visual nesting. Since the header already provides two-space indentation (`Span::raw("  ")`), and the checkbox/device-name now starts at column 0 (or column 4 when a checkbox is present), the alignment is clean. Adding extra padding would have misaligned the checkbox from the edge unnecessarily.
2. **`platform` variable removed**: After removing the `device_icon` call, the `platform` variable that was computed from `device.platform_type`/`device.platform` is no longer needed, so it was dropped along with its use.

### Testing Performed

- `cargo test -p fdemon-tui` — Passed (1301 unit tests, 7 doc tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (no warnings)
- `cargo fmt --all` — No format changes needed

### Risks/Limitations

1. **Visual regression**: The icon prefix is removed entirely; terminals with large device lists may look slightly more dense. However, the group headers continue to label every group, so no platform information is lost.
