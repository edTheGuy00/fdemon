## Task: Update Tests for Phase 2 Changes

**Objective**: Fix all test failures caused by the Phase 2 visual redesign. Update test assertions, snapshot files, and add new tests for the redesigned components.

**Depends on**: 01-terminal-background, 02-redesign-header, 03-redesign-log-view, 04-merge-status-into-log, 05-update-layout

### Scope

All test files in the `fdemon-tui` crate that are affected by the Phase 2 changes.

### Files with Expected Test Breakage

| File | Impact | Reason |
|------|--------|--------|
| `render/tests.rs` | **High** — 4 snapshot tests | Layout change (no status bar), background fill, header redesign all change snapshot output |
| `widgets/header.rs` (inline tests) | **High** | Header render logic completely changed |
| `widgets/tabs.rs` (inline tests) | **Medium** | Tab icon color fix, highlight style changes |
| `widgets/log_view/tests.rs` | **High** | Glass container, metadata bar, entry styling all changed |
| `widgets/log_view/mod.rs` (inline tests) | **Medium** | Source tag color changes, format changes |
| `widgets/status_bar/tests.rs` | **Medium** | StatusBar still exists as module but no longer rendered; tests may still need to pass |
| `widgets/status_bar/mod.rs` (inline tests) | **Low** | If module kept, tests should still pass (internal logic unchanged) |
| `layout.rs` (inline tests) | **Medium** | ScreenAreas struct changed, new layout proportions |

### Details

#### Types of Test Breakage

**Type 1: Snapshot test failures (`render/tests.rs`)**

The 4 existing snapshot tests capture full-screen output:
- `snapshot_normal_mode_initializing`
- `snapshot_normal_mode_running`
- `snapshot_normal_mode_reloading`
- `snapshot_normal_mode_stopped`

These will all fail because:
- Terminal background is now filled with `DEEPEST_BG`
- Header has new glass container styling
- Log view has glass container + metadata bars
- Status bar is no longer a separate section
- Layout proportions changed

**Fix:** Update all snapshot golden files. Run `cargo test` with `INSTA_UPDATE=1` (if using insta) or manually update expected output strings.

**Type 2: Header test assertions**

Tests in `header.rs` that check:
- Title rendering position and style
- Keybinding position and style
- Session tab rendering

These need updating for the new glass container layout and content positioning.

**Type 3: LogView test assertions**

Tests in `log_view/tests.rs` and inline that check:
- Border styles (now `BORDER_DIM` with `BorderType::Rounded`)
- Entry styling (source tag colors changed: App→Green, Flutter→Indigo)
- Empty state text
- No-matches state text
- Scroll calculation (visible_lines now accounts for metadata bars)

**Type 4: Layout test assertions**

Tests in `layout.rs` that check:
- `ScreenAreas` field values (status field removed)
- Layout proportions (new split without status bar)

**Type 5: StatusBar tests (keep or archive)**

If the status bar module is kept but no longer rendered, its tests should still pass since the internal rendering logic hasn't changed. However, if `ScreenAreas` no longer has a `status` field, any tests that reference it will break.

#### Strategy

1. Run `cargo test -p fdemon-tui 2>&1 | head -100` to identify all failures
2. Fix compilation errors first (struct field changes, removed types)
3. Fix layout tests (ScreenAreas changes)
4. Fix header tests (new render logic)
5. Fix log view tests (metadata bar, styling changes)
6. Fix snapshot tests (regenerate or update expected output)
7. Run full suite: `cargo test --workspace`

#### New Tests to Add

**Header tests:**
- `test_header_glass_container_border` — verify rounded border type and BORDER_DIM color
- `test_header_device_pill_rendering` — verify device icon + name appears
- `test_header_shortcut_hints` — verify all 5 shortcuts render
- `test_header_no_session` — verify graceful rendering without session data

**LogView tests:**
- `test_log_view_top_metadata_bar` — verify "TERMINAL LOGS" and "LIVE FEED" badge render
- `test_log_view_bottom_metadata_bar` — verify phase + mode + timer + errors render
- `test_log_view_visible_lines_with_metadata` — verify visible_lines accounts for both bars
- `test_log_view_source_tag_colors` — verify new color mapping (App→Green, etc.)
- `test_log_view_blinking_cursor` — verify cursor renders when auto_scroll is active

**Layout tests:**
- `test_layout_no_status_bar` — verify 2-section layout (header + logs)
- `test_layout_gap_between_sections` — verify 1-row gap exists

### Acceptance Criteria

1. `cargo test -p fdemon-tui` passes with zero failures
2. `cargo test --workspace` passes with zero failures
3. `cargo clippy --workspace` passes with no warnings
4. All existing tests are updated (not deleted)
5. New tests added for redesigned components (minimum 8 new tests listed above)
6. Snapshot tests regenerated with new expected output

### Testing

This task IS the testing task. Deliverable is a green test suite.

Run the full quality gate:
```bash
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

### Notes

- **Snapshot test strategy**: If the project uses `insta` for snapshots, update with `cargo insta review`. If using manual string comparison, update the expected strings directly.
- **Test count**: Phase 2 should add at least 8 new tests. The total test count should increase, not decrease.
- **Source tag color intentional change**: Tests that assert `App → Magenta` should be updated to `App → STATUS_GREEN`. This is an intentional design change, not a bug.
- **StatusBar tests**: If the module is kept, keep its tests passing. They validate internal rendering logic that could be reused or referenced. If the module is deleted, delete its tests too.
