## Task: Remove Dead Code

**Objective**: Remove all dead code identified in the review: unused `StatusBar`/`StatusBarCompact` module, legacy `HeaderWithTabs` and render functions in `tabs.rs`, deprecated `build_title()` in log view, and 7 dead layout utility functions.

**Depends on**: None

**Review Reference**: REVIEW.md #5 (Major), ACTION_ITEMS.md #7

### Scope

- `crates/fdemon-tui/src/widgets/status_bar/mod.rs`: Delete entire module (289 lines) — fully replaced by `LogView::render_bottom_metadata()` via `StatusInfo`
- `crates/fdemon-tui/src/widgets/status_bar/tests.rs`: Delete test file (comes with module deletion)
- `crates/fdemon-tui/src/widgets/mod.rs:19`: Remove `pub use status_bar::{StatusBar, StatusBarCompact};` export
- `crates/fdemon-tui/src/widgets/mod.rs:20`: Remove `pub use tabs::HeaderWithTabs` from `pub use tabs::{HeaderWithTabs, SessionTabs};` (keep `SessionTabs`)
- `crates/fdemon-tui/src/widgets/tabs.rs:129-288`: Delete `HeaderWithTabs` struct, its `Widget` impl, and the 3 legacy render functions (`render_tabs_header`, `render_simple_header`, `render_single_session_header`)
- `crates/fdemon-tui/src/widgets/tabs.rs:290+`: Delete tests for the removed legacy code (keep `SessionTabs` tests)
- `crates/fdemon-tui/src/widgets/log_view/mod.rs:798-835`: Delete deprecated `build_title()` function (has `#[allow(dead_code)]` and comment "deprecated - now in metadata bar")
- `crates/fdemon-tui/src/layout.rs`: Remove 7 `#[allow(dead_code)]` functions and the `LayoutMode` enum:
  - `LayoutMode` enum (line 22)
  - `LayoutMode::from_width()` (line 37)
  - `create()` (line 53)
  - `use_compact_footer()` (line 86)
  - `use_compact_header()` (line 92)
  - `header_height()` (line 98)
  - `timestamp_format()` (line 104)
  - `max_visible_tabs()` (line 117)

### Details

**Why these are safe to remove**:

1. **StatusBar module**: The render pipeline at `render/mod.rs` has an explicit comment: "Status bar removed - status info is now integrated into the log view's bottom metadata bar". Zero references to `StatusBar` or `StatusBarCompact` outside the module and its tests. The 16 hardcoded `Color::` references in this module are all legacy.

2. **HeaderWithTabs + legacy functions**: The render pipeline uses `MainHeader`, not `HeaderWithTabs`. The export at `widgets/mod.rs:20` is unused. The 3 legacy render functions contain 10 hardcoded `Color::` references that haven't been migrated.

3. **`build_title()`**: Explicitly marked deprecated with `#[allow(dead_code)]`. Its functionality is replaced by inline logic in `render_metadata_bar()`.

4. **Layout utility functions**: All 7 functions have `#[allow(dead_code)]` and are only referenced by each other or by tests within the same file. The active render pipeline only uses `create_with_sessions`.

**Order of operations**:
1. Delete `status_bar/` directory entirely (mod.rs + tests.rs)
2. Update `widgets/mod.rs` to remove the `mod status_bar` declaration, `StatusBar`/`StatusBarCompact` export, and `HeaderWithTabs` export
3. Delete legacy code in `tabs.rs` (lines 129-288 + associated tests)
4. Delete `build_title()` in `log_view/mod.rs`
5. Delete dead layout functions in `layout.rs`
6. Run `cargo check -p fdemon-tui` to verify no compile errors

### Acceptance Criteria

1. `StatusBar` and `StatusBarCompact` are completely removed (module + tests + exports)
2. `HeaderWithTabs` and its 3 legacy render functions are removed from `tabs.rs`
3. `build_title()` is removed from `log_view/mod.rs`
4. All 7 `#[allow(dead_code)]` layout functions are removed from `layout.rs`
5. No remaining `#[allow(dead_code)]` annotations in `layout.rs`
6. `cargo check -p fdemon-tui` passes with no errors
7. `cargo clippy -p fdemon-tui` passes with no warnings

### Testing

- Compile check is the primary verification — if the code was truly dead, removing it won't break anything
- Run `cargo test -p fdemon-tui` to ensure no tests depended on the removed code (tests for the removed code itself should also be deleted)

### Notes

- Removing the status_bar module eliminates 16 of the ~46 hardcoded `Color::` references
- Removing the legacy tabs code eliminates 10 more, bringing the remaining count to ~20
- Be careful to preserve `SessionTabs` in `tabs.rs` — it IS used by the active render pipeline
- The `status_bar/styles.rs` file (if it exists) should also be removed with the module
