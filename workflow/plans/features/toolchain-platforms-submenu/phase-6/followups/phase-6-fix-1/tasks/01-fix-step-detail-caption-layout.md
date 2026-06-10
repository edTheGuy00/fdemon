# Task 01: Fix step-detail caption-row layout reservation

**Status:** Not Started
**Agent:** implementor
**Complexity:** medium
**Depends On:** —
**Estimated Hours:** 1–2

## Objective

Stop the component list from rendering onto the FlutterSdk step-caption row in tight panels
(review finding M1, workflow/reviews/features/toolchain-platforms-submenu-phase-6/ACTION_ITEMS.md §1).

## Root Cause (verified)

In `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`:

- Line ~734: `bottom_section_height` is `ACTION_HINT_HEIGHT` (1) in the no-guided-commands case.
- Line ~752: `let component_height = content_area.height.saturating_sub(bottom_section_height)` —
  the component loop clamp `component_area_bottom = content_area.y + component_height` (line ~773)
  is derived from this.
- Lines ~789–799: `has_step_caption` and `effective_bottom_height` (`ACTION_HINT_HEIGHT + 1` when a
  caption is present) are computed AFTER the component loop, and `bottom_y` uses
  `effective_bottom_height`.

When `has_step_caption` is true, the caption renders at `y + height − 2` but the component loop may
render its last row at `y + height − 2` as well (its clamp allows up to `y + height − 1` exclusive)
— the two share a row.

## Required Changes

1. Hoist the `has_step_caption` determination (currently lines ~787–791: not guided-commands, step
   caption exists for the selected step) so it is available BEFORE the component-height computation.
2. Compute `effective_bottom_height` once, before the component loop, and derive
   `component_height = content_area.height.saturating_sub(effective_bottom_height)` from it.
   Keep a single source of truth — do not leave two divergent bottom-height variables; the later
   `bottom_y` computation must use the same value.
3. Audit the nearby small-height guards (e.g. line ~718 `content_area.height >= ACTION_HINT_HEIGHT`)
   to ensure no underflow/empty-area regressions when height is 1 or 2.
4. Locate by symbol, not line — line numbers are a snapshot.

## Acceptance Criteria

- [ ] New regression test: FlutterSdk step selected (caption active), component list long enough to
      fill the pane, pane height tight enough to trigger the clamp → assert the caption row contains
      ONLY the caption text (no component text) and the action-hint row is intact.
- [ ] Existing step_detail tests pass unchanged (the M1-history tests around lines ~1767 and ~1815
      still pass).
- [ ] Guided-commands path behavior unchanged (caption logic only applies when
      `!has_guided_commands`).
- [ ] `cargo test -p fdemon-tui --lib` green; `cargo fmt --all -- --check` and
      `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Files

**Write:** `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`
**Read:** `crates/fdemon-app/src/install_wizard/` (state types, read-only)

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | Hoisted `has_step_caption` / `effective_bottom_height` before component loop; added 2 regression tests |

### Notable Decisions/Tradeoffs

1. **Single-variable hoisting**: Collapsed the old two-variable pattern (`bottom_section_height` + later `effective_bottom_height`) into a single `effective_bottom_height` computed before the component loop. The guided-commands branch now lives entirely inside `effective_bottom_height`'s computation, removing the dead `bottom_section_height` variable. This is cleaner and eliminates the divergence that caused M1.

2. **Test width = 80**: The regression test uses width 80 (not 60) so the full caption text `"...v opens the version picker"` (66 chars) fits without terminal truncation. The test comment documents this choice.

3. **Minimal-height edge case test**: Added a second test with height=4 (`HEADER_HEIGHT(2) + caption(1) + hint(1)`) that verifies no panic and both caption/hint render when `component_height=0`. This covers the underflow guard path.

### Testing Performed

- `cargo test -p fdemon-tui --lib` — 1532 passed, 0 failed (new tests: `test_flutter_sdk_caption_not_overwritten_by_component_list`, `test_flutter_sdk_caption_minimal_height_no_panic`)
- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo test --workspace` — All green (all test result lines show 0 failures)

### Risks/Limitations

1. **Guided-commands branch unchanged**: The fix only affects the `!has_guided_commands` path. The guided-commands path already computed the correct height. No behavior change there.
2. **PathConfig/FlutterSdk caption**: Both are executable steps; `step_caption` returns `None` for PathConfig but `Some(...)` for FlutterSdk. The fix correctly adds the extra row reservation only for FlutterSdk (where the caption exists).
