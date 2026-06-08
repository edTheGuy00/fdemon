## Task: Fix install-wizard detail-pane cramping + add line wrapping (Bug 1)

**Agent:** implementor

**Objective:** Give the install-wizard detail pane enough room to show the full
package list and copy-paste commands on small terminals, and stop clipping long
lines. Enlarge the panel, reclaim the wasted header row, narrow the left pane, and
add real text wrapping to the detail content so long guided commands / component
detail / doctor lines wrap instead of being cut off at the right edge.

**Depends on:** — (file-disjoint from Tasks 02 and 03; safe in a parallel worktree)

**Estimated Time:** 4–6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/mod.rs`
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`
- `crates/fdemon-tui/src/widgets/install_wizard/doctor_view.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/install_wizard/state.rs` — `WizardStep`, `GuidedCommand`,
  `ComponentCheck`, `detail_scroll`, `last_known_visible_height` shapes (read-only)
- `crates/fdemon-tui/src/widgets/modal_overlay.rs` — `centered_rect_percent`
  (read-only; no change)

### Details

Decision (confirmed with user): **pragmatic wrap + resize** — do both the cheap
layout levers and real wrapping for the content lines; keep the existing
line-offset scroll model.

**A. Resize levers (`mod.rs`):**

1. **Panel height 70 → 85%.** `PANEL_HEIGHT_PERCENT` (`mod.rs:69`). Biggest single
   vertical win (24-row terminal: 16 → 20 inner rows). Update the doc comment.
2. **Left pane 35 → 28%.** `LEFT_PANE_PERCENT` (`mod.rs:74`). The step list never
   needs more than ~20 columns (`"  ✓ Flutter SDK"` = 15). Gives the detail pane
   ~7 more columns on an 80-col terminal. Update the doc comment.
3. **Header 3 → 2 rows.** The outer vertical layout (`mod.rs:317-325`) reserves
   `Constraint::Length(3)` for a header that only writes title (row 0) + subtitle
   (row 1); row 2 is dead padding. Change to `Constraint::Length(2)`. The
   `render_header` guards (`area.height >= 2`, `area.height < 1`) already make this
   safe. Recover 1 row for the panes on every size.
4. **Update `MIN_RENDER_HEIGHT`** (`mod.rs:59`) from 13 → 12 and its doc comment to
   reflect the reduced header (3 header + 1 sep + 5 content + 1 sep + 1 footer + 2
   border = 13 → 2 header → 12). Confirm `VERTICAL_STEP_LIST_HEIGHT` (`mod.rs:79`,
   value 9) still accounts for its "header(2) + 5 steps + 2 padding" derivation
   (it already assumes a 2-row header, so it stays valid).

**Do NOT** change the footer or the two separators — the footer is already a single
`Constraint::Length(1)` row (`mod.rs:340`) and is correct. The perceived "extra
space at the bottom" is the cumulative effect of the squeezed panel + clipped
content, which the resize + wrapping resolve.

**B. Line wrapping (`step_detail.rs`, `doctor_view.rs`):**

Today every content line renders into `Rect::new(area.x, y, area.width, 1)` (1 row)
with no `.wrap()`, so anything wider than the pane is clipped. Switch the
content-line renderers to wrap and advance `y` by the wrapped height:

- Use `ratatui::widgets::{Paragraph, Wrap}`; build the `Paragraph` with
  `.wrap(Wrap { trim: false })`.
- Compute the wrapped height per item with `paragraph.line_count(area.width)`
  (available in the workspace ratatui version — verify; if absent, fall back to a
  small `wrapped_height(text, width)` helper that ceil-divides display width by
  `area.width`, accounting for the existing indstring prefixes).
- Render into `Rect::new(area.x, y, area.width, h.min(remaining_rows))` and advance
  `y += h` instead of `y += 1`.
- Apply to: `render_component_row` (`step_detail.rs:176-188`), the guided-command
  **command** row and **note** row (`step_detail.rs:495-520`) — the label row is
  short and may stay 1 row — and `DoctorView`'s per-line render
  (`doctor_view.rs:52-74`).

**Scroll interaction.** The component-list and doctor scroll use
`compute_corrected_scroll` over **logical item count** and write
`last_known_visible_height` back each frame (`step_detail.rs:565`). With wrapping,
one logical item can occupy >1 row, so the existing offset math still advances one
*item* per keypress but the visible row budget shrinks. Keep the model
item-based (do **not** rework into per-row virtual scroll — that is the rejected
"full rework" option). Ensure the per-item `y` advance respects the remaining
content height so a tall wrapped item near the bottom is clipped to the pane rather
than overflowing into the footer. The guided-command windowing
(`compute_guided_window` / `command_block_height`, `step_detail.rs:281-382`) must be
updated so `command_block_height` accounts for the wrapped command/note row counts
(it currently assumes command = 1 row, note = 1 row); otherwise the window math and
the `guided_section_full_height` reservation will disagree with what is rendered.
This is the most delicate part — keep `command_block_height`,
`guided_section_full_height`, and `render_guided_commands` consistent (they are
already documented as a shared contract).

**C. Cosmetic fixture refresh (this task owns `step_detail.rs`):**

Update the test fixtures/doc-comment in `step_detail.rs` that embed the literal
`sudo apt install openjdk-17-jdk` (the doc-comment example ~line 392 and
`make_state_android_jdk_missing` ~line 1265) to use a per-manager example
(e.g. `sudo pacman -S jdk17-openjdk`) so they don't imply apt is the only Linux
case. These are independent test constructors — purely cosmetic, not load-bearing.

### Acceptance Criteria

1. `PANEL_HEIGHT_PERCENT == 85`, `LEFT_PANE_PERCENT == 28`, header constraint is
   `Length(2)`, and `MIN_RENDER_HEIGHT == 12`, each with an updated doc comment.
2. A guided command longer than the detail-pane width (e.g. the full Linux apt
   prerequisites string) **wraps** onto multiple rows and is fully present in the
   rendered buffer — assert via a render test on an 80×24 area that the buffer
   contains the tail of the command string (e.g. `libgtk-3-dev`), which currently
   gets clipped.
3. Component detail rows and doctor lines also wrap (no mid-string clipping for
   lines wider than the pane).
4. The guided-command scroll window still keeps the selected command fully visible
   when wrapping makes the section taller than the pane (no panic, no overflow into
   the footer/separator rows).
5. No panic across tiny areas (existing `test_no_panic_*` tests stay green) and the
   narrow-terminal vertical-stack layout still renders.

### Testing

```rust
// widgets/install_wizard/{mod,step_detail,doctor_view}.rs test modules
// - NEW: long_guided_command_wraps_and_is_fully_visible — 80x24 area, Linux apt
//        prerequisites command; assert buffer contains the clipped tail token.
// - NEW: panel_height_percent_is_85 / left_pane_percent_is_28 (or assert via a
//        layout-derived rendered-width check).
// - UPDATE: any snapshot/min-size test that encoded the old 70%/35%/3-row header
//        or MIN_RENDER_HEIGHT == 13.
// - KEEP: all test_no_panic_*, narrow/vertical layout, footer/header tests green.
```

### Notes

- The render-hint write-back (`last_known_visible_height.set(...)`) and the
  `Cell` pattern must be preserved — see docs/CODE_STANDARDS.md Principle 3 /
  docs/REVIEW_FOCUS.md.
- If `Paragraph::line_count` is unavailable in the pinned ratatui version, prefer a
  tiny local `wrapped_height` helper over bumping the dependency in this task.
- This task is the only writer of the three TUI wizard files — fully parallel with
  Tasks 02 and 03.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` | `PANEL_HEIGHT_PERCENT` 70→85, `LEFT_PANE_PERCENT` 35→28, `MIN_RENDER_HEIGHT` 13→12, header constraint `Length(3)→Length(2)`, updated doc comments; added 3 new const-value tests |
| `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | `Wrap` import; `render_component_row` now wrap-aware (returns height); added `wrapped_height` + `unicode_display_width` helpers; `command_block_height` accepts `width` for wrap-aware sizing; `guided_section_full_height` accepts `width`; `compute_guided_window` accepts `width`; `render_guided_commands` uses `wrapped_height` for cmd/note rows; cosmetic fixture `apt→pacman`; updated all test call-sites; 8 new tests |
| `crates/fdemon-tui/src/widgets/install_wizard/doctor_view.rs` | `Wrap` import; added local `wrapped_height` helper; `render_doctor_line` wrap-aware (returns height); `Widget::render` iterates with advancing `y`; 2 new tests |

### Notable Decisions/Tradeoffs

1. **`wrapped_height` helper instead of `Paragraph::line_count`**: `line_count` is behind `#[instability::unstable(feature = "rendered-line-info")]` in ratatui 0.30 and therefore inaccessible without opt-in. Implemented a local `wrapped_height` that uses Unicode display-width measurement (CJK/emoji = 2, control = 0, rest = 1) and ceil-divides by `width`. This is a standard and stable approach that handles all real-world install-wizard text correctly.

2. **Item-based scroll model preserved**: wrapping makes one logical item occupy >1 row, but the existing component-list and doctor scroll use per-item offsets. As directed by the task, this was kept as-is — the per-item scroll model still works correctly; the visible-height hint written back to the handler will account for the fact that tall items reduce the number of items that fit.

3. **`width=0` fallback in tests**: The `guided_section_full_height` and `compute_guided_window` unit tests that check exact row arithmetic pass `width=0` to use the pre-wrapping (1 row each) fallback, ensuring the arithmetic remains identical to the pre-wrapping implementation for short command strings.

4. **Pre-existing flaky test**: `toolchain::jdk::tests::test_resolve_jdk_home_honors_java_home` in `fdemon-daemon` sets `JAVA_HOME` without test serialization and races with other tests in `--workspace` parallel runs. This pre-existing flakiness (noted in CLAUDE.md) is unrelated to this task's changes — the test passes cleanly when run in isolation.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test -p fdemon-tui` — Passed (1476 tests)
- `cargo test -p fdemon-daemon` — Passed (1071 tests)
- `cargo test -p fdemon-app` — Passed (2849 tests)
- `cargo test -p fdemon-core` — Passed (514 tests)
- `cargo test -p flutter-demon` — Passed (all integration tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- New tests added: `test_panel_height_percent_is_85`, `test_left_pane_percent_is_28`, `test_min_render_height_is_12`, `test_long_guided_command_wraps_and_is_fully_visible`, `test_component_row_wraps_long_detail`, `test_doctor_lines_wrap_on_narrow_pane`, `test_doctor_view_long_line_wraps`, `test_wrapped_height_*` (4 unit tests each for both helpers) — all pass

### Risks/Limitations

1. **`wrapped_height` vs ratatui internals**: The helper correctly counts display widths and ceil-divides. For very short texts (≤ `width` chars) it always returns 1 — identical to ratatui. The main divergence is with multi-span lines where the copy-hint `"  [c] copy"` extends beyond the command text; since `wrapped_height` measures only the command text (as specified), the height estimate may be 1 row less than ratatui would render in edge cases. However since the rendered height is clamped to `remaining` rows, there is no overflow risk — worst case the copy hint is slightly truncated on a very narrow pane.

2. **Scroll model accuracy**: With wrapping, component items may take >1 row, so the effective visible item count is somewhat overestimated by the item-based scroll. This is acceptable as the task explicitly directed keeping the item-based model.
