## Task: Final Test and Cleanup

**Objective**: Run the full quality gate across the workspace, verify no regressions, clean up any stale references, and confirm the merged Inspector+Layout tab works correctly end-to-end.

**Depends on**: Task 06 (wire-merged-inspector-layout)

### Scope

- All crates in the workspace — verification pass, no new features

### Details

#### 1. Full quality gate

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

All four commands must pass cleanly.

#### 2. Search for stale references

Grep the entire workspace for leftover references that should have been cleaned up:

```
layout_explorer     → Should find zero matches (struct, field, module all removed)
DevToolsPanel::Layout → Should find zero matches (variant removed)
DETAILS_WIDTH_PCT   → Should find zero matches (renamed to LAYOUT_WIDTH_PCT)
details_panel       → Should find zero matches (file deleted, mod declaration removed)
"[l] Layout"        → Should find zero matches (tab text removed)
```

If any matches found, fix them.

#### 3. Verify test coverage

Check that all new code has adequate test coverage:

| Module | Expected New Tests | What to Verify |
|--------|-------------------|----------------|
| `fdemon-core/widget_tree.rs` | 5+ | EdgeInsets parse/format, LayoutInfo defaults |
| `fdemon-daemon/extensions/layout.rs` | 5+ | Padding extraction from JSON |
| `fdemon-tui/inspector/layout_panel.rs` | 12+ | All rendering states (box model, constraints, flex, loading, error, empty, compact) |
| `fdemon-app/handler/devtools/inspector.rs` | 5+ | Auto-fetch on navigate, debounce, dedup, expand no-fetch |

Run test count:
```bash
cargo test --workspace -- --list 2>&1 | grep "test " | wc -l
```

Compare with pre-Phase-2 count. Expect net gain of 20+ tests (some old layout_explorer tests removed, many new tests added).

#### 4. Verify no runtime panics in edge cases

Run specific targeted tests for edge cases:

```bash
# Inspector with no tree loaded (empty state)
cargo test -p fdemon-tui -- inspector::tests::test_empty

# Inspector with disconnected VM
cargo test -p fdemon-tui -- inspector::tests::test_disconnected

# Very small terminal
cargo test -p fdemon-tui -- inspector::tests::test_compact
```

#### 5. Visual spot-check checklist

If a Flutter project is available for manual testing, verify:

- [ ] Enter DevTools mode (`d`) — Inspector tab loads with tree + layout side by side
- [ ] Navigate tree (Up/Down) — layout panel updates after brief loading
- [ ] Expand/collapse nodes — layout panel does NOT refresh unnecessarily
- [ ] Layout panel shows widget name + source location at top
- [ ] Layout panel shows dimensions (width x height) for sized widgets
- [ ] Layout panel shows constraints (min/max) for constrained widgets
- [ ] Layout panel shows flex properties for flex children
- [ ] Layout panel shows box model visualization for padded widgets
- [ ] Switch to Performance tab (`p`) — still works
- [ ] Switch back to Inspector (`i`) — layout data preserved
- [ ] Press `'l'` — nothing happens (no Layout tab)
- [ ] Sub-tab bar shows only `[i] Inspector  [p] Performance`
- [ ] Narrow terminal (< 100 cols) — tree stacks above layout (vertical split)
- [ ] Wide terminal (>= 100 cols) — tree beside layout (horizontal split)

#### 6. Check handler test coverage for edge cases

Ensure these handler edge cases are tested:

```
handle_inspector_navigate(Down) with no tree loaded → no crash, no fetch
handle_inspector_navigate(Up) at index 0 → stays at 0, no crash
handle_layout_data_fetched with stale session_id → ignored
handle_layout_data_fetch_timeout → error displayed, layout_loading cleared
Rapid navigation (3 navigates in < 500ms) → only 1 fetch triggered
```

### Acceptance Criteria

1. `cargo fmt --all --check` passes
2. `cargo check --workspace` passes
3. `cargo test --workspace` passes (all tests, zero failures)
4. `cargo clippy --workspace -- -D warnings` passes
5. Zero stale references to `layout_explorer`, `DevToolsPanel::Layout`, `details_panel`
6. Net gain of 20+ new tests across all crates
7. No panics in edge case tests (empty state, disconnected, small terminal)
8. Visual output matches expected behavior from checklist (if manual testing possible)

### Testing

```bash
# Full quality gate — single command
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

### Notes

- This is a verification-only task — no new code should be written. If issues are found, fix them as part of this task.
- The visual spot-check requires a running Flutter project and is optional for CI. The automated tests should catch all functional regressions.
- If the total test count has decreased significantly (more than 10 tests lost net), investigate — old layout_explorer tests should be replaced by layout_panel tests.
- Phase 2 success criteria from PLAN.md should all be checkable after this task completes.

---

## Completion Summary

**Status:** Not started
