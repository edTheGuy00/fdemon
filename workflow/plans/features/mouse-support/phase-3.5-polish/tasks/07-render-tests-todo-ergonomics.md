# Task 07: Render-tests TODO ergonomics

**Status:** Not Started
**Estimated Hours:** 0.1h
**Depends On:** —
**Crate / Area:** `fdemon-tui`

## Goal

Discharge review item 15: three render-test doc comments in `crates/fdemon-tui/src/render/tests.rs` carry `TODO(phase-5): tag-filter overlay precedence …` notes on the *outer doc comment*. The tests themselves assert exact counts (`len() == 6`, `len() == 3`, `len() == 9`) which Phase 5 will likely change. TODO comments on outer doc blocks tend to drift away from the assertions they're meant to flag.

Move the Phase-5 update notes from the outer `///` comments to *inline* `//` comments next to the asserted counts, so when Phase 5 lands and the assertion changes, the TODO is colocated with the change site and naturally gets updated.

## Files Modified (Write)

- `crates/fdemon-tui/src/render/tests.rs`

## Files Read

- (none required)

## Implementation Steps

1. **Locate the three affected tests** at approximately lines 59, 105, and 156 of `render/tests.rs`. Their outer doc comments contain text similar to:
   > /// TODO(phase-5): tag-filter overlay precedence — when Phase 5 wires the
   > /// Settings panel's internal regions, this test should be updated to also
   > /// verify that panel regions exist alongside header regions.

2. **For each test:**
   a. Remove the `TODO(phase-5)` lines from the outer doc comment (keep the rest of the doc).
   b. Find the assertion that the TODO refers to (typically `assert_eq!(regions.iter().count(), N, ...)`).
   c. Add an inline comment immediately above the assertion:
      ```rust
      // Phase 5: Settings/DevTools/dialog panels will register their own regions on top of these.
      // Update this exact-count assertion to `>= N` (or split into per-source counts) when those land.
      assert_eq!(regions.iter().count(), 6, "...");
      ```

3. **Verify all three tests still compile and pass.** No assertion logic changes — only comment placement.

## Acceptance Criteria

- [ ] No `TODO(phase-5)` text appears in any outer `///` doc comment in `render/tests.rs`
- [ ] Each affected test has an inline `//` comment block colocated with the count-asserting `assert_eq!` describing the Phase-5 update path
- [ ] All three render tests still pass without modification:
  - `view_populates_header_shortcut_regions_at_120x24`
  - `view_populates_tab_regions_with_three_sessions`
  - `view_header_regions_present_in_settings_mode_because_header_always_renders` (the third has its own `TODO(phase-5)` comment about updating once Settings-panel regions land)
- [ ] `cargo test -p fdemon-tui --lib render::tests` passes
- [ ] `cargo fmt --all -- --check` passes

## Notes

- This is a comment-only refactor. No assertion logic changes.
- The point of the move is *colocation*: future Phase 5 implementors editing the assertion will see the TODO right next to the change site, instead of buried in the outer doc block.
- Do not delete the TODO content — only relocate it.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-abab981d77a861c01

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/render/tests.rs` | Removed `TODO(phase-5)` lines from three outer `///` doc comments; added inline `//` comments colocated with the count-asserting assertions in each test |

### Notable Decisions/Tradeoffs

1. **Third test inline comment**: The third test (`view_header_regions_present_in_settings_mode_because_header_always_renders`) uses `assert!(!regions.is_empty(), ...)` rather than an exact count. The existing inline comment already mentioned Phase 5 additions; the updated comment is more explicit about what the Phase 5 update path looks like (split into per-source counts or check specific panel-region entries), matching the spirit of the removed TODO.

### Testing Performed

- `cargo test -p fdemon-tui --lib render::tests` - Passed (8/8 tests)
- `cargo fmt --all -- --check` - Passed (no output = clean)

### Risks/Limitations

1. **None**: This is a comment-only refactor with no assertion logic changes. All tests pass unchanged.
