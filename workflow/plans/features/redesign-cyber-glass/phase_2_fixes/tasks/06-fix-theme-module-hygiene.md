## Task: Fix Theme Module Hygiene

**Objective**: Remove overly broad `#![allow(dead_code)]` suppressions from theme files, fix `SOURCE_*` palette constants, fix `Color::Black` inside `styles.rs`, update `icons.rs` docstring, and deduplicate `centered_rect()`.

**Depends on**: Tasks 02, 04, 05 (icons must be stabilized, dead code removed, palette migration complete — so we know which constants are actually used)

**Review Reference**: REVIEW.md #7, #8 (Major), ACTION_ITEMS.md #8, #10, Minor #4, #5

### Scope

#### 1. Remove `#![allow(dead_code)]` from theme files

- `crates/fdemon-tui/src/theme/palette.rs:7`: Remove `#![allow(dead_code)]`
- `crates/fdemon-tui/src/theme/icons.rs:7`: Remove `#![allow(dead_code)]`
- `crates/fdemon-tui/src/theme/styles.rs:4`: Remove `#![allow(dead_code)]`

After removing, run `cargo check` to identify genuinely unused constants. For constants intentionally kept for future phases, add targeted `#[allow(dead_code)]` on the specific item with a comment explaining why.

#### 2. Fix `SOURCE_*` palette constants

- `crates/fdemon-tui/src/theme/palette.rs:56-58`: `SOURCE_APP` is `Color::Magenta` but log_view uses `STATUS_GREEN` for App source. `SOURCE_FLUTTER` is `Color::Blue` but log_view uses `STATUS_INDIGO`. Either:
  - Update `SOURCE_*` constants to match actual log_view usage, OR
  - Have log_view use `SOURCE_*` constants (and update the values to match current behavior)

The second approach is preferred — update `SOURCE_APP = STATUS_GREEN`, `SOURCE_FLUTTER = STATUS_INDIGO`, then have `log_view/styles.rs` use the `SOURCE_*` constants.

#### 3. Fix `Color::Black` in `styles.rs`

- `crates/fdemon-tui/src/theme/styles.rs:82`: `focused_selected()` uses `Color::Black` directly. Replace with `palette::CONTRAST_FG` (add this constant to palette if not already added by Task 05).

#### 4. Deduplicate `centered_rect()`

- `crates/fdemon-tui/src/widgets/confirm_dialog.rs:29-33`: Has a private `centered_rect()` that is functionally identical to `modal_overlay::centered_rect()`.
- Replace the private function with a call to `crate::widgets::modal_overlay::centered_rect()`.

### Details

**`#![allow(dead_code)]` removal strategy**:

After Tasks 02, 04, and 05 are complete, many previously-unused constants will now be consumed. Remove the file-level suppression and let the compiler identify what's truly unused. Expected outcomes:
- `palette.rs`: Most constants should now be used. `NERD_*` (renamed from `ICON_*` in Task 02) may need targeted allows.
- `icons.rs`: After Task 02 renames Nerd Font constants to `NERD_*`, the unused `NERD_*` constants get targeted `#[allow(dead_code)]`.
- `styles.rs`: Most functions should be used after Task 05 migrates widget code.

**`SOURCE_*` fix**:

Current state in `palette.rs`:
```
SOURCE_APP = Color::Magenta       (but log_view uses STATUS_GREEN for App)
SOURCE_FLUTTER = Color::Blue      (but log_view uses STATUS_INDIGO for Flutter)
```

Fix: Update `palette.rs` to match actual usage:
```
SOURCE_APP = Color::Green         (matches STATUS_GREEN)
SOURCE_FLUTTER = STATUS_INDIGO    (matches what log_view actually uses)
```

Then update `log_view/styles.rs` to use `palette::SOURCE_APP` and `palette::SOURCE_FLUTTER` instead of `palette::STATUS_GREEN` and `palette::STATUS_INDIGO` directly. This gives semantic meaning to the color choice.

### Acceptance Criteria

1. No `#![allow(dead_code)]` in `palette.rs`, `icons.rs`, or `styles.rs`
2. Any remaining unused constants have targeted `#[allow(dead_code)]` with explanation comment
3. `SOURCE_*` constants match actual log_view color usage
4. No `Color::` references inside theme module files (all use palette constants)
5. `confirm_dialog.rs` uses `modal_overlay::centered_rect()` instead of its own copy
6. `cargo check -p fdemon-tui` passes
7. `cargo clippy -p fdemon-tui` passes

### Testing

- Compile check is primary verification
- Run `cargo test -p fdemon-tui` to ensure no regressions

### Notes

- This task should be done AFTER Tasks 02, 04, and 05 to minimize churn — we need to know the final state of icon constants and palette usage before auditing dead code
- The `icons.rs` docstring fix (removing reference to nonexistent `icon()` function) may already be done in Task 02. Verify and skip if so.
