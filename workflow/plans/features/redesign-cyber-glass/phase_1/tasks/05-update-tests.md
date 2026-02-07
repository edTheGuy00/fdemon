## Task: Update Tests for Theme Migration

**Objective**: Fix all test failures caused by the style migration in Tasks 03 and 04. Update test assertions to match the new theme-sourced styles.

**Depends on**: 03-migrate-widget-styles, 04-consolidate-phase-mapping

### Scope

All test files and inline `#[cfg(test)]` modules in the `fdemon-tui` crate that assert on style/color values.

### Files with Style-Dependent Tests

| File | Test Type | Expected Impact |
|------|-----------|-----------------|
| `widgets/status_bar/tests.rs` | Separate test file, ~14 color assertions | **Medium** — phase indicator consolidation changes modifier behavior |
| `widgets/log_view/tests.rs` | Separate test file, ~6+ color assertions | **Low** — colors mapped 1:1 |
| `widgets/settings_panel/tests.rs` | Separate test file, ~7 color assertions | **Low** — colors mapped 1:1 |
| `render/tests.rs` | Separate test file, full-screen snapshots | **Medium** — loading screen + overlay colors |
| `widgets/header.rs` (inline tests) | Inline `#[cfg(test)]` | **Low** — simple color check |
| `widgets/tabs.rs` (inline tests) | Inline `#[cfg(test)]` | **Medium** — phase icon/color assertions may change |
| `widgets/confirm_dialog.rs` (inline tests) | Inline `#[cfg(test)]` | **Low** — dialog color assertions |
| `widgets/search_input.rs` (inline tests) | Inline `#[cfg(test)]` | **Low** — search style assertions |
| `selector.rs` (inline tests) | Inline `#[cfg(test)]` | **Low** — project selector assertions |
| `widgets/new_session_dialog/mod.rs` (inline tests) | Inline `#[cfg(test)]` | **Low** — footer/bg assertions |
| `widgets/new_session_dialog/tab_bar.rs` (inline tests) | Inline `#[cfg(test)]` | **Low** — tab style assertions |
| `widgets/new_session_dialog/device_list.rs` (inline tests) | Inline `#[cfg(test)]` | **Medium** — DeviceListStyles struct removed |
| `widgets/new_session_dialog/launch_context.rs` (inline tests) | Inline `#[cfg(test)]` | **Medium** — LaunchContextStyles struct removed |
| `widgets/new_session_dialog/fuzzy_modal.rs` (inline tests) | Inline `#[cfg(test)]` | **Medium** — `mod styles` block removed |
| `widgets/new_session_dialog/dart_defines_modal.rs` (inline tests) | Inline `#[cfg(test)]` | **Medium** — `mod styles` block removed |
| `widgets/new_session_dialog/target_selector.rs` (inline tests) | Inline `#[cfg(test)]` | **Low** |

### Details

#### Types of Test Breakage

**Type 1: Direct style assertions**
Tests that check `cell.fg == Color::Cyan` need updating to `cell.fg == palette::ACCENT` (though in Phase 1 these are the same named color, so many should still pass).

```rust
// These should still pass since palette::ACCENT == Color::Cyan in Phase 1
assert_eq!(cell.fg, Color::Cyan);
// But any that import Color directly may need the import updated
```

**Type 2: Struct/module removal**
Tests that reference `DeviceListStyles::default()`, `LaunchContextStyles::default()`, or `styles::MODAL_BG` will fail because those types/modules no longer exist.

```rust
// Before
let styles = DeviceListStyles::default();
assert_eq!(styles.header.fg, Some(Color::Yellow));

// After — these tests should be updated to test the rendering output, not the struct
// Or test the palette constant directly:
assert_eq!(palette::STATUS_YELLOW, Color::Yellow);
```

**Type 3: Phase indicator changes**
The consolidation in Task 04 may change exact modifier behavior (e.g., tabs now getting BOLD on phase icons when they didn't before). Tests asserting exact buffer content will need style expectation updates.

**Type 4: Buffer snapshot tests**
Full-screen render tests in `render/tests.rs` may need complete expected-output updates if any colors changed in the migration.

#### Migration Strategy

1. Run `cargo test -p fdemon-tui` to see all failures
2. Categorize failures into the types above
3. Fix Type 2 failures first (struct/module removal — most mechanical)
4. Fix Type 3 failures (phase indicator — change expected styles)
5. Fix Type 1 failures (if any — unlikely in Phase 1)
6. Fix Type 4 failures (buffer snapshots — update expected output)
7. Run full `cargo test --workspace` to verify no cross-crate regressions

### Acceptance Criteria

1. `cargo test -p fdemon-tui` passes with zero failures
2. `cargo test --workspace` passes with zero failures
3. `cargo clippy --workspace` passes with no warnings
4. No test is deleted — only updated to match new style sources
5. Tests that referenced removed structs (`DeviceListStyles`, `LaunchContextStyles`) are updated to either:
   - Test the rendering output directly, or
   - Test the palette constants directly

### Testing

This task IS the testing task. The deliverable is a green test suite.

Run the full quality gate:
```bash
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

### Notes

- **Minimize test changes**: Where possible, update only the assertion values, not the test structure. The goal is to keep test coverage identical.
- **Don't weaken tests**: If a test checked for a specific color, keep checking for that specific color (now via palette constant). Don't replace precise assertions with weaker ones.
- **Watch for Color::Cyan vs palette::ACCENT equality**: In Phase 1, `palette::ACCENT == Color::Cyan`, so most assertions using `Color::Cyan` will still pass. But if tests import `Color` and compare against it, they may need updated imports.
- **Buffer snapshot tests are fragile**: If `render/tests.rs` has pixel-perfect buffer assertions, even a single modifier change (like adding BOLD to a phase icon) will cause a failure. Be prepared to update full expected buffers.
- **Test count should not decrease**: The current test count for `fdemon-tui` is 427 widget tests. After this task, the count should be >= 427 (may increase if new theme module tests are added in Task 01).
