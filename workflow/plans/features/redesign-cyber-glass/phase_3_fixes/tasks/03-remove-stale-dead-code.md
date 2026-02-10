## Task: Remove Stale `#[allow(dead_code)]` Annotations

**Objective**: Remove incorrect `#[allow(dead_code)]` annotations from actively-used palette constants. `SURFACE` and `GRADIENT_BLUE` are used throughout the codebase but still carry dead_code suppression from Task 01 of Phase 3.

**Depends on**: None

**Review Reference**: REVIEW.md #3 (Major), ACTION_ITEMS.md #3

### Scope

- `crates/fdemon-tui/src/theme/palette.rs` lines 14, 43: Remove `#[allow(dead_code)]` from `SURFACE` and `GRADIENT_BLUE`

### Details

**Constants and their usage**:

| Constant | Line | Annotation | Actual Usage | Action |
|----------|------|------------|-------------|--------|
| `SURFACE` | 14-15 | `#[allow(dead_code)]` | Used 10+ times across `launch_context.rs`, `dart_defines_modal.rs`, `mod.rs`, `fuzzy_modal.rs` | **Remove annotation** |
| `GRADIENT_BLUE` | 43-44 | `#[allow(dead_code)]` | Used 2+ times in `launch_context.rs` (LaunchButton) | **Remove annotation** |
| `GRADIENT_INDIGO` | 45-46 | `#[allow(dead_code)]` | Not used anywhere outside palette.rs | **Keep annotation** |

**Fix**: Simply remove the two `#[allow(dead_code)]` lines above `SURFACE` and `GRADIENT_BLUE`. Leave the annotation on `GRADIENT_INDIGO` since it is genuinely unused (reserved for future gradient effects).

### Acceptance Criteria

1. `SURFACE` constant has no `#[allow(dead_code)]` annotation
2. `GRADIENT_BLUE` constant has no `#[allow(dead_code)]` annotation
3. `GRADIENT_INDIGO` retains its `#[allow(dead_code)]` annotation
4. `cargo check -p fdemon-tui` passes (no dead_code warnings)
5. `cargo clippy -p fdemon-tui -- -D warnings` passes

### Testing

- No test changes needed — this is annotation cleanup only
- Verify clippy does not emit dead_code warnings after removal

### Notes

- These annotations were added during Task 01 (palette migration) when the constants were first introduced, before the subsequent tasks that used them.
