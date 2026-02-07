## Task: Fix Footer Height Desync in Log View

**Objective**: Fix the edge case where `footer_height` is set to 1 even when the footer isn't rendered, stealing a content line in very small terminal areas.

**Depends on**: None

**Review Reference**: REVIEW.md #3 (Critical), ACTION_ITEMS.md #3

### Scope

- `crates/fdemon-tui/src/widgets/log_view/mod.rs:1017-1042`

### Details

**Root cause**: At line 1018, `footer_height` is computed as `if has_footer { 1 } else { 0 }`. But the actual footer rendering at line 1022 has an additional guard: `if inner.height > 1`. When `inner.height == 1` and `has_footer` is true, `footer_height` is 1 but the footer is skipped, making `visible_lines = 0`.

**Fix**: Change line 1018-1019 to incorporate the render guard:

```rust
let footer_height = if has_footer && inner.height > 1 { 1 } else { 0 };
```

This is a one-line fix that aligns the height calculation with the actual rendering condition.

### Acceptance Criteria

1. When `inner.height <= 1` and `status_info.is_some()`, `footer_height` is 0
2. When `inner.height > 1` and `status_info.is_some()`, `footer_height` is 1 (unchanged)
3. When `status_info.is_none()`, `footer_height` is 0 regardless (unchanged)
4. No regression in normal-sized terminal rendering
5. `cargo check -p fdemon-tui` passes

### Testing

Add a unit test that creates a LogView with `status_info` in a very small area (height 3 — borders consume 2, leaving `inner.height = 1`) and verifies that content lines are not stolen by the phantom footer.

### Notes

- This is an edge case that only manifests on extremely small terminal windows or when the log area is compressed. It's unlikely to affect normal usage, but it's a logic correctness issue.
- The compact threshold magic number `60` at line 1025 (`let compact = area.width < 60;`) is noted as a minor issue in the review. Consider extracting to a constant (e.g., `MIN_FULL_STATUS_WIDTH`) while making this fix, but it's optional.
