## Task: Define Height Threshold Constants with Hysteresis

**Objective**: Introduce named constants for the minimum heights required for expanded vs compact rendering of LaunchContext and TargetSelector, with hysteresis buffers to prevent flickering during terminal resize.

**Depends on**: None

**Estimated Time**: 1 hour

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs`: Add threshold constants near existing `MIN_HORIZONTAL_WIDTH` etc. (lines 117-131)

### Details

Add the following constants to `mod.rs`, grouped with the existing layout threshold constants:

```rust
/// Minimum content-area height for LaunchContext to render in expanded (full) mode.
/// Expanded mode needs: 5 fields × 4 rows + 4 spacers + 1 spacer + 3 button = 28 rows.
/// We use 28 as the expand threshold.
const MIN_EXPANDED_LAUNCH_HEIGHT: u16 = 28;

/// Height at which LaunchContext switches back to compact mode (hysteresis).
/// This is 4 rows below the expand threshold to prevent flickering during resize.
const COMPACT_LAUNCH_HEIGHT_THRESHOLD: u16 = 24;

/// Minimum content-area height for TargetSelector to render in full mode.
/// Full mode needs: 3-row tab bar + Min(5) device list + 1-row footer = 9 rows minimum.
/// We use 10 to give the device list a reasonable viewport.
const MIN_EXPANDED_TARGET_HEIGHT: u16 = 10;

/// Height at which TargetSelector switches back to compact mode (hysteresis).
const COMPACT_TARGET_HEIGHT_THRESHOLD: u16 = 7;
```

**Hysteresis logic**: The expand/compact decision uses two separate thresholds:
- Switch to expanded when `height >= MIN_EXPANDED_*_HEIGHT`
- Switch back to compact when `height <= COMPACT_*_HEIGHT_THRESHOLD`
- In the gap between the two thresholds, maintain the previous mode

Since the current rendering is stateless (no "previous mode" stored), the initial implementation should use the expand threshold for the decision and the compact threshold as a lower bound. Specifically:
- `height >= MIN_EXPANDED_LAUNCH_HEIGHT` → expanded
- `height < MIN_EXPANDED_LAUNCH_HEIGHT` → compact

Hysteresis will be refined in a later phase if flickering is observed. For now, the two constants document the intended thresholds and the gap between them.

### Acceptance Criteria

1. Constants are defined with `/// doc comments` explaining the rationale
2. Constants follow `SCREAMING_SNAKE_CASE` naming convention
3. Constants are grouped near the existing `MIN_HORIZONTAL_WIDTH` block (lines 117-131)
4. Values are derived from actual layout measurements:
   - `calculate_fields_layout()` uses 25 rows for fields; button needs 1 spacer + 3 rows = 29 total, but the `Min(0)` absorber means 28 rows is sufficient since the last spacer can collapse
   - Compact mode needs: 2 border + 5 fields + 1 spacer + 3 button = 11 rows
5. `cargo check -p fdemon-tui` passes

### Testing

No dedicated tests for constants themselves — they'll be exercised by tasks 03-05.

### Notes

- The `LaunchContext::min_height()` method (line 847-849 of `launch_context.rs`) returns `29`. Our threshold of `28` is intentionally 1 less because `calculate_fields_layout` has a `Min(0)` absorber at the end that handles the final spacer gracefully.
- The hysteresis gap (4 rows) is chosen to be larger than typical terminal resize increments (1-2 rows at a time).
- These constants are private to `mod.rs` — they're implementation details of the dialog layout, not part of any public API.
