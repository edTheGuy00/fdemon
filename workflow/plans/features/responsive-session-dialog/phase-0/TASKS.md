# Phase 0: Document Responsive Layout Standards - Task Index

## Overview

Establish project-wide responsive layout guidelines in `docs/CODE_STANDARDS.md`. These standards codify the patterns implemented in Phases 1-3 and serve as the reference for all future layout work across all widgets.

**Total Tasks:** 2
**Estimated Hours:** 2-3 hours

## Task Dependency Graph

```
┌────────────────────────────────────────┐
│  01-write-responsive-layout-guidelines │
└───────────────────┬────────────────────┘
                    │
                    ▼
┌────────────────────────────────────────┐
│  02-cross-reference-implementation     │
└────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-write-responsive-layout-guidelines](tasks/01-write-responsive-layout-guidelines.md) | Done | - | 1-2h | `docs/CODE_STANDARDS.md` |
| 2 | [02-cross-reference-implementation](tasks/02-cross-reference-implementation.md) | Done | 1 | 1h | `docs/CODE_STANDARDS.md`, `crates/fdemon-tui/` |

## Success Criteria

Phase 0 is complete when:

- [ ] `docs/CODE_STANDARDS.md` has a "Responsive Layout Guidelines" section after "Architectural Code Patterns"
- [ ] All 5 principles are documented with rationale and code examples
- [ ] Anti-pattern examples show the "before" (wrong) and "after" (correct) patterns
- [ ] Guidelines are general enough to apply to any widget, not just the New Session dialog
- [ ] Code examples in the guidelines accurately reference patterns from the actual Phases 1-3 implementation
- [ ] No stale or hypothetical examples — every code snippet reflects real codebase patterns

## Notes

- Phase 0 was originally planned to run before Phases 1-3 but is being completed after them. This is actually advantageous: the guidelines can reference real, battle-tested patterns rather than hypothetical ones.
- The five principles from the plan are: (1) space-based layout decisions, (2) content within allocated area, (3) scroll-to-selected visibility, (4) named threshold constants, (5) hysteresis at breakpoints.
- Key implementation references to draw from:
  - Phase 1: `MIN_EXPANDED_LAUNCH_HEIGHT`, height-based compact decisions in `render_horizontal()`/`render_vertical()`
  - Phase 2: `calculate_fields_layout()` with `Min(0)` absorber, `LAUNCH_BUTTON_SLOT` constant
  - Phase 3: `Cell<usize>` render-hint feedback, `effective_visible_height()` fallback pattern, render-time scroll correction
