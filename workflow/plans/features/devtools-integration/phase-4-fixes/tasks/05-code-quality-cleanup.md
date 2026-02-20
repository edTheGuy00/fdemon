## Task: Code Quality Cleanup — Shared Helpers, Constants, Dead Code, Visibility

**Objective**: Address code quality violations found in the review: extract duplicated `truncate_str`, replace magic numbers with named constants, remove `#[allow(dead_code)]` on unused `icons` fields, fix `render_tab_bar` visibility, and improve `truncate_str` performance.

**Depends on**: 02-fix-vm-connection, 04-session-switch-reset

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: Extract shared `truncate_str`, fix `render_tab_bar` visibility
- `crates/fdemon-tui/src/widgets/devtools/inspector.rs`: Remove local `truncate_str`, remove unused `icons` field
- `crates/fdemon-tui/src/widgets/devtools/layout_explorer.rs`: Remove local `truncate_str`, remove unused `icons` field, add doc comment to `format_constraint_value`
- `crates/fdemon-tui/src/widgets/devtools/performance.rs`: Extract magic numbers as named constants

### Details

#### 1. Extract and Improve `truncate_str`

Both `inspector.rs:482-492` and `layout_explorer.rs:385-395` have identical `truncate_str` implementations that allocate a `Vec<char>` on every call. Extract to `devtools/mod.rs` as a zero-allocation version:

```rust
/// Truncate a string to at most `max_chars` Unicode characters.
/// Returns a `&str` slice — no allocation when the string fits.
pub(super) fn truncate_str(s: &str, max_chars: usize) -> &str {
    if max_chars == 0 {
        return "";
    }
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
```

**Important:** The return type changes from `String` to `&str`. Update all call sites in `inspector.rs` and `layout_explorer.rs`. Most pass the result to `buf.set_string(x, y, &result, style)` which accepts `AsRef<str>`, so the change is straightforward. Check each call site to confirm no `.push_str()` or ownership is needed — if any call site needs owned, use `.to_string()` at the call site.

Remove the local `truncate_str` functions from both files.

#### 2. Replace Magic Numbers in `performance.rs`

The file already has a constants section (lines 18-29). Add the missing threshold constants:

```rust
// ── Style threshold constants ─────────────────────────────────────────────────

/// Cap for sparkline bar heights (2x the 16.67ms frame budget at 60fps).
const SPARKLINE_MAX_MS: u64 = 33;
/// FPS at or above this value is considered healthy (green).
const FPS_GREEN_THRESHOLD: f64 = 55.0;
/// FPS at or above this value (but below green) is degraded (yellow).
const FPS_YELLOW_THRESHOLD: f64 = 30.0;
/// Memory utilization below this is healthy (green).
const MEM_GREEN_THRESHOLD: f64 = 0.6;
/// Memory utilization below this (but above green) is elevated (yellow).
const MEM_YELLOW_THRESHOLD: f64 = 0.8;
/// Jank frame percentage below this is acceptable (yellow, not red).
const JANK_WARN_THRESHOLD: f64 = 0.05;
```

Replace the inline literals at lines 233, 401-403, 410-414, and 425 with these constants.

#### 3. Remove Unused `icons` Field

Both `WidgetInspector` (inspector.rs:38) and `LayoutExplorer` (layout_explorer.rs:29) have:

```rust
#[allow(dead_code)]
icons: IconSet,
```

The `icons` field is never read in either widget's render path (`PerformancePanel` does use it). Remove the field and the `#[allow(dead_code)]` from both structs. Update constructors (`new()`) to stop accepting the `icons` parameter. Update all call sites:

- `devtools/mod.rs:84` — `WidgetInspector::new(&self.state.inspector, self.icons)` → `WidgetInspector::new(&self.state.inspector)`
- `devtools/mod.rs:96-100` — `LayoutExplorer::new(...)` — remove `self.icons` arg

If icons will be needed in the future (e.g., for tree expand/leaf icons in Inspector), add them back when the feature is implemented. Dead code should not be suppressed.

#### 4. Fix `render_tab_bar` Visibility

Change `pub fn render_tab_bar` (mod.rs:128) to private `fn render_tab_bar`. Update tests in the same file that call it directly — they should drive through `Widget::render()` instead, or use `pub(crate)` if direct access is needed for focused testing.

If the tests specifically need to assert tab bar content in isolation, `pub(crate)` is acceptable. But `pub` exposes it as part of the widget's external interface, which is incorrect.

#### 5. Add Doc Comment to `format_constraint_value`

`layout_explorer.rs:376` — `pub fn format_constraint_value(value: f64) -> String` lacks a `///` doc comment. Add one:

```rust
/// Format a layout constraint value for display. Infinite values render as "∞".
pub fn format_constraint_value(value: f64) -> String {
```

### Acceptance Criteria

1. `truncate_str` exists in exactly one place (`devtools/mod.rs`) and returns `&str`
2. No `Vec<char>` allocation in `truncate_str` for strings shorter than `max_chars`
3. All magic numbers in `performance.rs` style functions replaced with named constants
4. No `#[allow(dead_code)]` annotations remain on the `icons` fields
5. `render_tab_bar` is not `pub` (either private or `pub(crate)`)
6. `format_constraint_value` has a `///` doc comment
7. All existing tests pass
8. `cargo clippy -- -D warnings` passes

### Testing

No new tests needed — this is a refactor. All existing tests should continue to pass unchanged. If `render_tab_bar` tests need updating, adjust them to call through `Widget::render()` or accept `pub(crate)`.

Run the full test suite:
```bash
cargo test -p fdemon-tui --lib
```

### Notes

- `truncate_str` returning `&str` instead of `String` is a signature change. If any call site computes the truncated string and stores it in a struct, it would need a lifetime parameter or a `.to_string()` call. Check the 6 call sites in `inspector.rs` and 3 in `layout_explorer.rs` carefully.
- The `icons` removal may cascade: check if `DevToolsView::new()` still needs the `icons` parameter (it passes it to `PerformancePanel`, which DOES use it). The `DevToolsView` struct should keep its `icons` field since `PerformancePanel` uses it.
- The duplicate `test_performance_panel_small_terminal` / `test_performance_panel_minimum_size` tests in `performance.rs` should also be deduplicated (remove one).

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Added `pub(super) fn truncate_str` (zero-allocation `&str` return); changed `render_tab_bar` from `pub fn` to private `fn` |
| `crates/fdemon-tui/src/widgets/devtools/inspector.rs` | Removed local `truncate_str` (Vec<char> allocating); removed `icons: IconSet` field and constructor parameter; updated `new()` signature; added `use super::truncate_str`; removed needless `&` borrows at call sites; updated all tests to use new 1-arg `new()` |
| `crates/fdemon-tui/src/widgets/devtools/layout_explorer.rs` | Removed local `truncate_str`; removed `icons: IconSet` field and constructor parameter; updated `new()` signature; updated doc comment on `format_constraint_value`; added `use super::truncate_str`; removed needless `&` borrows at call sites; updated all tests to use new 2-arg `new()` |
| `crates/fdemon-tui/src/widgets/devtools/performance.rs` | Added 6 named threshold constants (`SPARKLINE_MAX_MS`, `FPS_GREEN_THRESHOLD`, `FPS_YELLOW_THRESHOLD`, `MEM_GREEN_THRESHOLD`, `MEM_YELLOW_THRESHOLD`, `JANK_WARN_THRESHOLD`); replaced all magic number literals in style functions; removed duplicate `test_performance_panel_minimum_size` test |

### Notable Decisions/Tradeoffs

1. **`truncate_str` visibility `pub(super)`**: Task specified `pub(super)`. Child modules (`inspector.rs`, `layout_explorer.rs`) access it via `use super::truncate_str`. Private items in a parent are visible to descendants in Rust, so this works cleanly.

2. **`render_tab_bar` made private**: Tests that call it directly are in `mod tests` inside `mod.rs` (same module as `DevToolsView`), so they can access private methods without any visibility upgrade needed.

3. **Needless borrow removal**: Clippy `needless_borrows_for_generic_args` correctly flagged `&desc_trunc` etc. since the variables are now `&str` (not `String`). All call sites updated to pass the `&str` directly.

4. **`DevToolsView` keeps `icons` field**: As specified, only `WidgetInspector` and `LayoutExplorer` had their `icons` fields removed. `PerformancePanel` actively uses icons for section titles.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-tui --lib` - Passed (518 tests)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Passed (formatting consistent)

### Risks/Limitations

1. **Test count**: Went from 519 to 518 tests due to removal of the duplicate `test_performance_panel_minimum_size` in `performance.rs`. This is intentional.
