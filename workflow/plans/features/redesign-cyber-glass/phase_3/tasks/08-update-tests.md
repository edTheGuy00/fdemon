## Task: Update Tests for Phase 3 Changes

**Objective**: Fix all test failures caused by Phase 3 changes (palette migration, modal redesign, styling updates) and add targeted new tests for the redesigned components.

**Depends on**: 03-redesign-modal-frame, 04-redesign-target-selector, 05-redesign-launch-context, 06-redesign-modal-footer, 07-migrate-nested-modals

### Scope

- `crates/fdemon-tui/src/theme/palette.rs` — Update palette tests
- `crates/fdemon-tui/src/theme/styles.rs` — Update style assertion tests
- `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` — Update dialog rendering tests
- `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` — Update tab bar tests
- `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` — Update device list tests
- `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` — Update launch context tests
- `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs` — Update fuzzy modal tests
- `crates/fdemon-tui/src/widgets/new_session_dialog/dart_defines_modal.rs` — Update dart defines tests
- `crates/fdemon-tui/src/widgets/modal_overlay.rs` — Update overlay tests
- `crates/fdemon-tui/src/render/tests.rs` — Update render snapshot tests
- Any other test files that fail due to palette/styling changes

### Details

#### Categories of Test Failures

**1. Palette color value assertions:**

Tests that compare exact color values against named colors will fail:

```rust
// Before (will fail):
assert_eq!(style.fg, Some(Color::Cyan));
assert_eq!(style.fg, Some(Color::Green));

// After (correct):
assert_eq!(style.fg, Some(palette::ACCENT));  // Now Rgb(88,166,255)
assert_eq!(style.fg, Some(palette::STATUS_GREEN));  // Now Rgb(16,185,129)
```

**Strategy**: Always assert against `palette::` constants, not raw `Color::` values. This makes tests resilient to future palette changes.

**2. Style function assertions:**

Tests in `styles.rs` that verify style colors:

```rust
// These should still pass since they compare against palette constants:
assert_eq!(text_primary().fg, Some(palette::TEXT_PRIMARY));
// palette::TEXT_PRIMARY is now Rgb(201,209,217) — assertion still works
```

**3. Buffer content assertions:**

Tests that render widgets to `TestBackend` and check content strings will mostly still pass (they check text content, not colors). However, tests that check styled content or background colors may need updates.

**4. Removed palette constant references:**

Tests referencing removed constants (`MODAL_FUZZY_BG`, `MODAL_DART_DEFINES_*`, etc.) will fail to compile. Update or remove these tests.

**5. Layout changes:**

Tests that assert on specific positioning may break due to:
- 40/60 pane split (was 50/50)
- Header area (was title-on-border)
- Footer styling changes

#### Test Update Strategy

**Phase A: Fix compilation errors**

1. Search for references to removed palette constants in test files
2. Replace with new palette constants or remove if the test is no longer relevant
3. Verify `cargo check --workspace` passes

**Phase B: Fix assertion failures**

1. Run `cargo test --workspace` and collect failures
2. For each failure, determine if:
   - The assertion is wrong (update expected value)
   - The test is checking a removed feature (remove or refactor)
   - The test is checking layout that changed (update positioning assertions)

**Phase C: Add new tests**

For each redesigned component, add tests verifying:

**Palette tests:**
```rust
#[test]
fn test_palette_uses_rgb_values() {
    match palette::ACCENT {
        Color::Rgb(88, 166, 255) => {},
        _ => panic!("ACCENT should be Rgb(88, 166, 255)"),
    }
    match palette::DEEPEST_BG {
        Color::Rgb(10, 12, 16) => {},
        _ => panic!("DEEPEST_BG should be Rgb(10, 12, 16)"),
    }
}
```

**Modal overlay tests:**
```rust
#[test]
fn test_dim_background_applies_to_all_cells() {
    // Verify all cells in area get dimmed
}

#[test]
fn test_render_shadow_offset() {
    // Verify shadow appears at +1 offset
}
```

**Dialog rendering tests:**
```rust
#[test]
fn test_dialog_renders_header_with_title() {
    // Verify "New Session" text appears in header area
}

#[test]
fn test_dialog_renders_footer_hints() {
    // Verify footer shows keyboard hints
}

#[test]
fn test_dialog_40_60_pane_split() {
    // Verify target selector takes ~40% width
}
```

**Tab bar tests:**
```rust
#[test]
fn test_tab_bar_pill_style_renders() {
    // Verify tab bar renders with container
}
```

#### Quality Gate

After all updates:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

All four commands must pass cleanly.

### Acceptance Criteria

1. `cargo check --workspace` passes (no compilation errors from removed constants)
2. `cargo test --workspace` passes (all assertions updated for new palette/layout)
3. `cargo clippy --workspace -- -D warnings` passes (no clippy warnings)
4. `cargo fmt --all` produces no changes
5. No test assertions compare against raw `Color::` named values — all use `palette::` constants
6. New tests added for: RGB palette values, modal overlay, dialog header rendering
7. Removed palette constant tests cleaned up

### Testing

Run the full quality gate:

```bash
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

Expected: zero failures, zero warnings.

### Notes

- **Test count preservation**: Phase 1 had 1,532 tests. The count may decrease slightly if tests for removed features are dropped, or increase if new tests are added. Aim for net-positive test count.
- **Snapshot tests**: If `insta` snapshot tests exist, they will need `cargo insta review` to accept new snapshots. Check if the project uses `insta`.
- **render/tests.rs**: The render tests may check full-screen snapshots. These will break due to background color changes, overlay rendering, etc. Update expected output to match the new design.
- **Parallelism**: Test fixes can be done incrementally — fix compilation first, then run tests and fix failures one by one. No need to batch all changes.
