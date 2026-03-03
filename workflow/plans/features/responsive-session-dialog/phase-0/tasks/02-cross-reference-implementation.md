## Task: Cross-Reference Guidelines Against Phase 1-3 Implementation

**Objective**: Verify that every principle and example in the new "Responsive Layout Guidelines" section accurately reflects the patterns used in the actual Phases 1-3 implementation. Fix any discrepancies between the documented guidelines and the real code.

**Depends on**: 01-write-responsive-layout-guidelines

**Estimated Time**: 1 hour

### Scope

- `docs/CODE_STANDARDS.md`: Review and update the newly added "Responsive Layout Guidelines" section
- Read-only reference files (do not modify):
  - `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` — Phase 1 constants and height-based decisions
  - `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` — Phase 2 layout-managed button
  - `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` — Phase 3 render-hint write-back
  - `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` — Phase 3 `Cell<usize>` field
  - `crates/fdemon-app/src/handler/new_session/target_selector.rs` — Phase 3 handler fallback

### Details

For each principle, verify the following alignment points:

#### Principle 1 (Space-based decisions) — aligns with Phase 1

| Guideline Claim | Actual Implementation | Verify |
|---|---|---|
| Check `area.height < MIN_EXPANDED_HEIGHT` | `chunks[2].height < MIN_EXPANDED_LAUNCH_HEIGHT` in `render_horizontal()` | Threshold value and comparison direction match |
| Both horizontal and vertical paths use height checks | `render_horizontal()` checks launch compact; `render_vertical()` checks both target and launch compact | Both paths confirmed |
| Named constant for threshold | `MIN_EXPANDED_LAUNCH_HEIGHT = 29`, `MIN_EXPANDED_TARGET_HEIGHT = 10` | Values documented correctly |

#### Principle 2 (Content within bounds) — aligns with Phase 2

| Guideline Claim | Actual Implementation | Verify |
|---|---|---|
| Include all elements in Layout | `calculate_fields_layout()` returns `[Rect; 13]` with button at `[11]`, `Min(0)` at `[12]` | Slot count and button index match |
| Use `Min(0)` absorber | Last constraint is `Constraint::Min(0)` | Present and correct |
| Named constant for button slot | `LAUNCH_BUTTON_SLOT = 11` | Constant name and value match |

#### Principle 3 (Scroll tracking) — aligns with Phase 3

| Guideline Claim | Actual Implementation | Verify |
|---|---|---|
| `Cell<usize>` for render-hint | `pub last_known_visible_height: Cell<usize>` on `TargetSelectorState` | Field name and type match |
| Default to 0, fallback in handler | `Cell::new(0)` default; `effective_visible_height()` returns `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` when 0 | Fallback logic matches |
| Render-time correction not written back | `corrected_scroll` used for rendering only, never assigned to `state.scroll_offset` | Write-back behavior confirmed |
| TEA exception annotation | `// EXCEPTION: TEA render-hint write-back via Cell` at call sites | Annotation convention matches |

#### Principle 4 (Named constants) — aligns with Phases 1-2

| Guideline Claim | Actual Implementation | Verify |
|---|---|---|
| Doc comments on constants | All 4 threshold constants + `LAUNCH_BUTTON_SLOT` have `///` comments | Comments present |
| Grouped near controlling widget | Constants in `mod.rs` lines 133-151 near layout logic | Location correct |

#### Principle 5 (Hysteresis) — aligns with Phase 1

| Guideline Claim | Actual Implementation | Verify |
|---|---|---|
| Hysteresis pair with gap | `MIN_EXPANDED_LAUNCH_HEIGHT = 29` / `COMPACT_LAUNCH_HEIGHT_THRESHOLD = 24` (5-row gap) | Gap size and values match |
| Compact threshold currently unused | `#[allow(dead_code)]` on `COMPACT_LAUNCH_HEIGHT_THRESHOLD` and `COMPACT_TARGET_HEIGHT_THRESHOLD` | Dead-code annotation present |
| Stateless note in docs | Guidelines mention "start with expand threshold only, add hysteresis if flickering observed" | Matches actual implementation approach |

#### Additional Checks

- Verify no guideline references a pattern that doesn't exist in the codebase
- Verify the anti-pattern examples in the summary table map to real "before" states that existed prior to Phases 1-3
- Verify no typos in constant names, function names, or type names

### Acceptance Criteria

1. Every code example in the guidelines section is consistent with the actual implementation
2. All constant names and values referenced match the real codebase
3. The anti-pattern examples accurately describe the pre-implementation state
4. The correct-pattern examples accurately describe the post-implementation state
5. No references to hypothetical or non-existent patterns, types, or functions
6. Any discrepancies found in Task 01's output are corrected in `docs/CODE_STANDARDS.md`

### Testing

No code tests needed — this is a documentation review task.

Verification:
- `cargo check --workspace` still passes (no code changes)
- Manual comparison of guideline text against source files listed in Scope

### Notes

- This task exists because Phase 0 was completed after Phases 1-3 rather than before. The cross-reference step ensures the documentation matches reality rather than the original design spec (which had minor differences, e.g., the plan said `MIN_EXPANDED_LAUNCH_HEIGHT = 28` but implementation used `29`).
- If any discrepancies are found, update the guidelines (not the implementation) — the code is already tested and working.
- Pay special attention to the `Cell<usize>` default value: the plan mentioned `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` as the default, but the implementation uses `0` as the default (with `0` meaning "not yet rendered") and the handler falls back to `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` when it reads `0`. The guideline should reflect the actual `0`-default pattern.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `docs/CODE_STANDARDS.md` | Fixed 3 discrepancies in the "Responsive Layout Guidelines" section |

### Notable Decisions/Tradeoffs

1. **Principle 4 derivation arithmetic**: The guideline said "5 fields × 4 rows + 4 spacers + 1 button spacer + 3 button rows = 29" — this evaluates to 28, not 29. The actual `calculate_fields_layout` has a leading spacer at slot [0] plus 4 inter-field spacers = 5 regular spacers, plus 1 button spacer at slot [10], plus 3-row button at slot [11]. Corrected to "5 spacers + 1 button spacer" so the arithmetic adds up correctly (5×4 + 5 + 1 + 3 = 29).

2. **Principle 5 dead_code attribute ordering**: The guideline had `#[allow(dead_code)]` before the `///` doc comment. The actual code in `mod.rs` (lines 137–141, 148–151) places the `///` doc comment first, then `#[allow(dead_code)]`. Fixed to match idiomatic Rust ordering and match the actual implementation.

3. **Principle 3 annotation comment**: The guideline showed `// EXCEPTION: TEA render-hint write-back via Cell` but the actual call sites in `target_selector.rs` (lines 75 and 154) use `// EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md`. Updated both the inline code example and the "Annotate every call site with:" block to include the full annotation.

### Testing Performed

- `cargo check --workspace` — Passed (0.40s, no code changes)
- Manual cross-reference of all 5 principles against the 5 source files listed in Scope — Completed

### Risks/Limitations

1. **Guideline examples are illustrative**: Principles 1, 2, 4, and 5 use simplified/generic examples (e.g., `MIN_EXPANDED_HEIGHT` instead of the actual `MIN_EXPANDED_LAUNCH_HEIGHT`/`MIN_EXPANDED_TARGET_HEIGHT`). This is intentional — the guidelines are generalizing the pattern for the entire codebase, not re-documenting the exact implementation. Only factual errors (wrong arithmetic, wrong ordering, wrong annotation text) were corrected.
