# Phase 1: Space-Aware Compact/Expanded Decision - Task Index

## Overview

Decouple the compact/expanded rendering decision from layout orientation (Horizontal vs Vertical). Instead, use actual available vertical space to choose the right rendering mode for both LaunchContext and TargetSelector.

**Total Tasks:** 5
**Estimated Hours:** 8-12 hours

## Task Dependency Graph

```
┌──────────────────────────┐
│  01-threshold-constants  │
└────────┬────────┬────────┘
         │        │
         │        │    ┌──────────────────────────────┐
         │        │    │  02-render-panes-compact-arg  │
         │        │    └──────────────┬────────────────┘
         │        │                   │
         │        ▼                   │
         │  ┌─────────────────────────┴──────┐
         │  │  03-horizontal-height-decision  │
         │  └────────────────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│  04-vertical-height-decision │
└──────────────────────────────┘
         │
         │    (03 also feeds into 05)
         │        │
         ▼        ▼
┌──────────────────────────────┐
│  05-unit-tests               │
└──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-threshold-constants](tasks/01-threshold-constants.md) | Not Started | - | 1h | `mod.rs` |
| 2 | [02-render-panes-compact-arg](tasks/02-render-panes-compact-arg.md) | Not Started | - | 1-2h | `mod.rs` |
| 3 | [03-horizontal-height-decision](tasks/03-horizontal-height-decision.md) | Not Started | 1, 2 | 2-3h | `mod.rs` |
| 4 | [04-vertical-height-decision](tasks/04-vertical-height-decision.md) | Not Started | 1 | 2-3h | `mod.rs` |
| 5 | [05-unit-tests](tasks/05-unit-tests.md) | Not Started | 3, 4 | 2-3h | `mod.rs`, `target_selector.rs`, `launch_context.rs` |

## Success Criteria

Phase 1 is complete when:

- [ ] Narrow-but-tall terminal (e.g., 50x40) shows expanded Launch Context fields
- [ ] Wide-but-short terminal (e.g., 100x25) shows compact Launch Context fields
- [ ] Existing horizontal/vertical layout switching still works correctly at standard sizes
- [ ] Hysteresis prevents flickering when resizing across thresholds
- [ ] All existing tests pass (`cargo test -p fdemon-tui`)
- [ ] New unit tests cover height-based compact/expanded decisions
- [ ] `cargo clippy --workspace -- -D warnings` passes

## Notes

- All changes in this phase are within `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` (primary) with test additions touching `target_selector.rs` and `launch_context.rs`
- The `compact` flag on `TargetSelector` and `LaunchContextWithDevice` already exists as a builder method — no struct changes needed in child widgets
- `render_panes()` is only called from `render_horizontal()`, so changes to it won't affect the vertical path
- The `calculate_fields_layout()` function in `launch_context.rs` consumes 25 rows for fields; adding 1 spacer + 3 button = 29 rows minimum for full mode (this informs the threshold constants)
