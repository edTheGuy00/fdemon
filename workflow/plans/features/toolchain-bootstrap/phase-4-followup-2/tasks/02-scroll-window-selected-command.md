## Task: Anchor a scroll window to the selected guided command (F1 — finish M1)

**Severity:** MINOR (completes a partially-fixed MAJOR — original M1)

**Objective**: Make the *selected* guided command — its row, highlight, and inline
`[c] copy` hint — **always** visible, even when the guided section is clamped on a short
terminal. Today `c` can copy a command that is scrolled off-screen, re-opening the
original M1 visible/copied divergence in the short-terminal regime.

**Depends on**: 01-extract-step-caption-helper (re-touches the same render function)

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`

**Files Read (Dependencies):**
- `fdemon-app::install_wizard`: `selected_command_index`, `GuidedCommand`,
  `WizardStepKind`.

### Details

The first-round M1 fix sized the bottom section to the full command count and clamped it
to `content_area.height` (`step_detail.rs:526-535`), which fixed the all-terminals clip.
But `render_guided_commands` (`:320-428`) still draws commands **top-to-bottom from
`area.y`**, guarded by `y < area.y + area.height`, with **no scroll offset toward
`selected_command_index`** — the index only drives styling (`is_selected` at `:380`,
copy hint at `:406`). So when `content_area.height < guided_section_full_height()`, the
section is clamped and trailing command blocks are clipped from the bottom. At
`selected_command_index = 2` on a short detail pane (macOS CLT + CocoaPods + Rosetta,
≈12 rows full), the Rosetta block + inline `[c] copy` fall outside `area` and are skipped
— yet `c` still copies `selected_guided_command()` = command 2. Visible ≠ copied.

This is task-01 "option 2" from the first-round plan (anchor a scroll window to the
selected index), which the first round did not implement (it took "option 1": size +
clamp).

**Fix:** When the guided section's `area` is too short to show every command block,
render a **window of command blocks anchored so the selected command is always visible**.
Approach (implementer's discretion; keep it within the existing layout idioms):

1. Compute each command block's height using the **same** per-command math as
   `guided_section_full_height` (reuse the helper, or factor a `command_block_height(cmd,
   is_first, has_caption)` so the window math and the total math share one source).
2. Determine the available rows for command blocks =
   `area.height - header(1) - caption_rows(0/1)`.
3. Choose a start index so the **selected** command's full block fits within the
   available rows — e.g. walk backwards from `selected_command_index` accumulating block
   heights until the budget is exhausted, then render forward from that start. (A simple
   "ensure selected is in `[start, end)` and its block fits" windowing is sufficient —
   there is no per-line scroll, only per-command-block.)
4. Render the header + caption, then the windowed command blocks, preserving the existing
   bounds guards as the final safety net. Keep the `[c] copy` hint inline on the selected
   command's command row.

Preserve:
- The **saturating clamp** `full_height.min(content_area.height)` at `:535` and the
  `bottom_area` height computation at `:567-574` (the review explicitly **rejected**
  simplifying these — they intentionally clamp the Rect to the content region).
- `docs/CODE_STANDARDS.md` Responsive Layout Principle 2 (all content bounds-guarded /
  via layout) and Principle 4 (named constants with derivation comments) for any new
  threshold.
- Tall-terminal output: when everything fits (`full_height <= area.height`), the window
  is the full list and rendering is byte-for-byte unchanged.

### Acceptance Criteria

1. On a short detail pane where not all command blocks fit, the block at
   `selected_command_index` is fully rendered (label + command + inline `[c] copy` +
   optional note) within `area`.
2. `c` copies a command that is currently visible — no visible/copied divergence at any
   terminal height that can render at least one full command block.
3. With `selected_command_index = 2` and a short pane (≈10–12 rows), the third command's
   text and its `copy` hint are present in the rendered buffer.
4. Tall-terminal rendering (everything fits) is visually unchanged from the current
   behavior; existing render tests stay green.
5. The saturating clamp / `bottom_area` math is preserved; no out-of-bounds Rect, no
   panic on tiny terminals (≤8 rows).

### Testing

```rust
#[cfg(test)]
mod tests {
    // - NEW: Prerequisites component present + 3 guided commands, area height ~10-12,
    //   selected_command_index = 2 → assert the 3rd command's text AND its 'copy' hint
    //   are present in the rendered buffer (regression for F1 short-terminal clip).
    // - NEW: same fixture, selected_command_index = 0 → command 0 visible.
    // - existing tall-terminal (height 30) selected-index-2 test stays green.
    // - existing single-command + no-panic-on-tiny-terminal tests stay green.
}
```

### Notes

- The first-round 8-row `test_no_panic_small_terminal_with_component_and_multiple_commands`
  asserts only "no panic" — it does **not** assert selected-command visibility. Add the
  visibility assertion at a renderable short height (≈10–12), where at least the selected
  block fits.
- Reuse task 01's `step_caption` helper for caption rows; do not re-derive `has_caption`.
- This task only touches `step_detail.rs`.
