## Task: Update Tests for All Phase 2 Fixes

**Objective**: Update snapshot tests, fix broken tests from dead code removal, and add targeted unit tests for the critical fixes.

**Depends on**: Tasks 01-07 (all fixes must be complete)

**Review Reference**: ACTION_ITEMS.md Re-review Checklist

### Scope

- `crates/fdemon-tui/src/render/snapshots/`: Update all snapshot files affected by header height changes (Task 01)
- `crates/fdemon-tui/src/render/tests.rs`: Update full-screen snapshot tests
- `crates/fdemon-tui/src/widgets/log_view/tests.rs`: Add footer height desync test, update any tests broken by `build_title()` removal or icon changes
- `crates/fdemon-tui/src/layout.rs`: Add tests for dynamic header height based on session count (if layout tests existed for removed functions, they're already gone from Task 04)
- `crates/fdemon-tui/src/widgets/tabs.rs`: Verify remaining `SessionTabs` tests pass after `HeaderWithTabs` removal
- `crates/fdemon-tui/src/render/mod.rs:116-154`: Extract duplicate search overlay rendering into a helper function (minor issue from review)

### Details

**Test categories**:

#### 1. Snapshot test updates (from Task 01 — header height change)

The 4 snapshot files will likely need regeneration because the header area size may change when multi-session mode is tested:
- `fdemon_tui__render__tests__normal_initializing.snap`
- `fdemon_tui__render__tests__normal_reloading.snap`
- `fdemon_tui__render__tests__normal_running.snap`
- `fdemon_tui__render__tests__normal_stopped.snap`

Run `cargo test -p fdemon-tui` with `INSTA_UPDATE=1` to regenerate snapshots, then review the diffs.

#### 2. New unit tests

**Layout dynamic height test**:
```rust
#[test]
fn test_create_with_sessions_single_session_height() {
    let areas = create_with_sessions(Rect::new(0, 0, 80, 24), 1);
    assert_eq!(areas.header.height, 3);
}

#[test]
fn test_create_with_sessions_multi_session_height() {
    let areas = create_with_sessions(Rect::new(0, 0, 80, 24), 3);
    assert!(areas.header.height >= 5);
}
```

**Footer height desync test**:
```rust
#[test]
fn test_footer_height_not_stolen_in_small_area() {
    // Create LogView with status_info in a 3-row area (1 inner row)
    // Verify content area is not reduced by phantom footer
}
```

#### 3. Search overlay deduplication (minor fix)

Extract the duplicate search overlay code in `render/mod.rs:116-154` into a helper:
```rust
fn render_search_overlay(frame: &mut Frame, areas: &ScreenAreas, state: &AppState, force: bool) {
    if let Some(handle) = state.session_manager.selected() {
        if force || !handle.session.search_state.query.is_empty() {
            let search_area = Rect::new(
                areas.logs.x + 1,
                areas.logs.y + areas.logs.height.saturating_sub(3),
                areas.logs.width.saturating_sub(2),
                1,
            );
            frame.render_widget(Clear, search_area);
            frame.render_widget(
                widgets::SearchInput::new(&handle.session.search_state).inline(),
                search_area,
            );
        }
    }
}
```

Then both `UiMode::SearchInput` and `UiMode::Normal` call this helper with `force: true` / `force: false`.

#### 4. Minor fixes bundled with test update

- `header.rs:138,159`: Remove unnecessary `.clone()` on `left_spans` and `shortcuts` Vec
- `header.rs:185,200`: Extract magic number `4` to a named constant (e.g., `const HEADER_SECTION_PADDING: u16 = 4;`)
- `log_view/mod.rs:1025`: Extract magic number `60` to a named constant (e.g., `const MIN_FULL_STATUS_WIDTH: u16 = 60;`)

### Acceptance Criteria

1. `cargo test --workspace --lib` passes (all 1,589+ tests)
2. `cargo fmt --all` passes
3. `cargo clippy --workspace -- -D warnings` passes
4. All snapshot tests are up to date
5. New unit tests cover the 3 critical fixes
6. Search overlay code is deduplicated in `render/mod.rs`
7. No magic numbers remain in header.rs and log_view footer
8. No unnecessary `.clone()` calls in header.rs

### Testing

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace --lib
cargo clippy --workspace -- -D warnings
```

### Notes

- Run snapshot updates with `INSTA_UPDATE=1` environment variable if using `insta` for snapshot testing
- The search overlay deduplication is a minor refactor — if it causes unexpected complexity, it can be deferred
- Magic number extraction and clone removal are low-risk cleanups that naturally belong in the test update pass

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/header.rs` | Added `HEADER_SECTION_PADDING` constant (line 20), removed unnecessary `.clone()` calls on `left_spans` and `shortcuts` (lines 140, 161), replaced magic number `4` with constant (lines 190, 203) |
| `crates/fdemon-tui/src/render/mod.rs` | Extracted duplicate search overlay rendering into `render_search_overlay()` helper function (lines 21-47), replaced duplicate code in `UiMode::SearchInput` and `UiMode::Normal` with helper calls (lines 142, 146) |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | Added `test_footer_height_not_stolen_in_small_area()` test (lines 961-1011) to verify footer height calculations in constrained spaces |

### Notable Decisions/Tradeoffs

1. **Layout tests already comprehensive**: The required dynamic height tests (`test_create_with_sessions_single_session_height` and `test_create_with_sessions_multi_session_height`) were already implemented in `layout.rs` with more thorough coverage as `test_create_layout_single_session`, `test_create_layout_multiple_sessions`, and `test_create_with_sessions_returns_different_heights`. No additional tests were needed.

2. **MIN_FULL_STATUS_WIDTH already extracted**: Task 03 already extracted the magic number `60` to `MIN_FULL_STATUS_WIDTH` constant in `log_view/mod.rs:32`, so no additional change was needed.

3. **Snapshot tests unchanged**: All 4 snapshot tests pass without updates because they test single-session states, and the layout changes (Task 01) only affect multi-session mode (>1 session). Previous tasks already updated snapshots for icon and color changes.

4. **Footer height test revised**: Initial test design was corrected after discovering that very small areas (inner.height = 1) correctly result in visible_lines = 0 due to top metadata bar. Final test uses a 5-row area to properly test footer presence/absence difference.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace --lib` - Passed (419 tests, up from 418)
- `cargo clippy --workspace -- -D warnings` - Passed
- `cargo test -p fdemon-tui --lib render::tests` - Passed (all 4 snapshot tests unchanged)

### Risks/Limitations

1. **Test count**: Test count increased by 1 (418 → 419) instead of expected increase, because layout tests were already comprehensive and MIN_FULL_STATUS_WIDTH constant already existed from Task 03.

2. **Snapshot stability**: Snapshot tests remain stable because they test single-session scenarios. Future multi-session snapshot tests may need baseline updates.
