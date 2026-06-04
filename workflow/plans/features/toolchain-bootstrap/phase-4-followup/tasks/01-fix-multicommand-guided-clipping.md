## Task: Fix multi-command guided-section clipping in step_detail.rs (M1 + m5)

**Severity:** MAJOR (M1), MINOR (m5)

**Objective**: Ensure that when a Prerequisites step has component checks **and**
multiple guided commands (the real macOS path: Xcode CLT + CocoaPods + Rosetta),
the **selected** command — together with its selection highlight and `[c] copy`
hint — is always visible, so that what `c` copies matches what the user sees.

**Depends on**: None

**Estimated Time**: 3-5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`

**Files Read (Dependencies):**
- `fdemon-app::install_wizard` re-exports: `selected_command_index`, `GuidedCommand`,
  `WizardStepKind`.

### Details

**M1 — clipping (MAJOR).** When `step.components` is non-empty AND the step carries
guided commands, the bottom section is reserved as
`GUIDED_SECTION_HEADER_HEIGHT(1) + caption_rows(1) + GUIDED_COMMAND_MIN_HEIGHT(4) = 6`
rows, and `bottom_area.height` is effectively capped to that (`step_detail.rs:470-524`,
specifically the `bottom_section_height` reservation ~`484` and the `bottom_area`
computation ~`512-523`). `render_guided_commands` (~`264-372`) draws header(1) +
caption(1) + command-0 block (label + command + optional note), consuming all 6
rows; commands at index 1+ fail the `y < area.y + area.height` guards and are
entirely clipped — on **any** terminal height. Because the guided section has no
independent scroll (`detail_scroll` only drives the component list / doctor view),
pressing `]` (`select_next_command`) moves the selection and the `c` copy target
(`selected_guided_command`, app `state.rs:125`) to a command whose highlight and
`[c] copy` hint are off-screen, while `c` copies the invisible command.

This is the production macOS Prerequisites path: `check_prerequisites()`
(`fdemon-daemon .../prerequisites.rs`) always returns a `Prerequisites`
`ComponentCheck`, so `components` is non-empty, and `prerequisites_guided_commands()`
(app `state.rs`) can return 3 commands. The existing test
`make_state_prerequisites_macos_three_commands` (`step_detail.rs:~1052-1060`) masks
the bug by setting `components: vec![]`, which routes through the un-capped
full-content-area branch.

Pick one of the two fixes (implementer's discretion, prefer the one that fits the
existing layout idioms in this file):

1. **Size the guided section to the command count** when components are present —
   reserve `header + caption + Σ per-command block heights` (label + command +
   optional note + conditional leading blank), clamped to `content_area.height`
   (keep the existing saturating clamp so the Rect never exceeds the content area —
   see the *rejected* "simplification" note in the review; that clamp is intentional).
2. **Anchor a scroll window to `selected_command_index`** so the selected command
   (and its `[c]` hint) is always within the rendered window when space is tight.

Either way, follow `docs/CODE_STANDARDS.md` Responsive Layout Principle 2 (all
content via the `Layout` system / bounds-guarded) and Principle 4 (named constants
with derivation comments).

**m5 — doc accuracy (MINOR).** Update the `GUIDED_COMMAND_MIN_HEIGHT` doc
(`step_detail.rs:66-70`) and the related layout block comments (~`247-261`): the
`[c] copy` hint is rendered **inline on the command row** (~`350-357`), not as a
separate row, and the leading blank row is **conditional** (`needs_blank = i > 0 ||
!has_caption`, ~`319`) — skipped for command 0 under a caption. State the real
derivation, e.g. "label(1) + command-with-inline-`[c]`-copy(1) + optional note(1) +
optional leading blank(1); blank skipped under a caption."

### Acceptance Criteria

1. On a Prerequisites step with a `Prerequisites` component present and 3 guided
   commands, all three commands (or a scrolled window that always includes the
   selected one) render without clipping on a reasonably-sized terminal.
2. With `selected_command_index = 2`, the selected command's row, its highlight, and
   its `[c] copy` hint are all visible.
3. `c` copies a command that is currently visible (no visible/copied divergence).
4. The single-command AndroidTools / Prerequisites path is visually unchanged.
5. The `GUIDED_COMMAND_MIN_HEIGHT` doc-comment matches what the renderer draws.
6. No out-of-bounds Rect on small terminals (retain the saturating-clamp behavior).

### Testing

```rust
#[cfg(test)]
mod tests {
    // - new: macOS 3-command Prerequisites WITH a Prerequisites component present,
    //   selected_command_index = 2 → assert the selected command's text AND its
    //   'copy' hint are present in the rendered buffer (regression for M1).
    // - existing single-command render tests stay green.
    // - small-terminal render does not panic and clamps within content area.
}
```

### Notes

- The review's adversarial verifier **rejected** a separate claim that the
  `bottom_area` height expression should be simplified to `bottom_section_height` —
  that expression intentionally computes `min(B, H)` to clamp to the content region.
  Do **not** apply that "simplification"; preserve the saturating clamp.
