## Task: Final Integration Test and Cleanup

**Objective**: Verify the full Phase 3 implementation works end-to-end. Run all workspace tests, fix any regressions, ensure clippy is clean, and verify the visual output matches the Phase 3 success criteria.

**Depends on**: Task 07 (rewire panel), Task 08 (wire polling)

### Scope

- Workspace-wide: all 4 crates
- Test files: all existing + new tests from Tasks 01-08
- Documentation: update test counts if documented

### Details

#### Full quality gate

Run the complete verification suite:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

All four commands must pass cleanly.

#### Test coverage verification

Verify the target of 30+ new tests across Phase 3:

| Task | Expected New Tests | Location |
|------|--------------------|----------|
| 01 — Core types | 8-10 | `fdemon-core/src/performance.rs` |
| 02 — State + messages | 10-12 | `fdemon-app/src/session/performance.rs` |
| 03 — VM service | 4-6 | `fdemon-daemon/src/vm_service/{performance,timeline}.rs` |
| 04 — Handler + keys | 6-8 | `fdemon-app/src/handler/devtools/performance.rs` or `tests.rs` |
| 05 — Frame chart | 12-15 | `fdemon-tui/src/widgets/devtools/performance/frame_chart.rs` |
| 06 — Memory chart | 12-15 | `fdemon-tui/src/widgets/devtools/performance/memory_chart.rs` |
| 07 — Panel rewire | 6-8 | `fdemon-tui/src/widgets/devtools/performance/mod.rs` |
| 08 — Polling wire | 3-5 | `fdemon-app/src/handler/tests.rs` |
| **Total** | **61-79** | |

Run `cargo test --workspace 2>&1 | tail -5` and verify total test count has increased by ~60+ from the pre-Phase-3 baseline.

#### Regression check

Verify no existing functionality is broken:

1. **Inspector panel**: Widget tree navigation, layout explorer, 50/50 split (Phase 2)
2. **Performance panel**: Disconnected state, reconnecting state, monitoring inactive state
3. **DevTools panel switching**: `i`/`p` keys switch panels correctly
4. **Session management**: Performance state is per-session, switching sessions shows correct data
5. **VM reconnection**: Performance state resets on reconnect
6. **Debug overlays**: Ctrl+p/r/d toggles still work

#### Visual spot-check

If running against a live Flutter app, verify:

1. Frame bar chart shows colored bars (Cyan=UI, Green=Raster)
2. Jank frames appear in Red
3. 16ms budget line is visible as a horizontal dashed line
4. Left/Right arrow keys move frame selection
5. Selected frame shows detail breakdown below chart
6. Memory chart shows time-series data with colored layers
7. GC markers appear as dots on the time axis
8. Allocation table shows top classes with instance counts and sizes
9. Esc from selected frame deselects; Esc from no selection exits DevTools
10. Small terminal (< 60 cols) renders gracefully without crash

#### File cleanup

1. Verify `stats_section.rs` is deleted or empty
2. Verify `frame_section.rs` is deleted or replaced
3. Verify `memory_section.rs` is deleted or replaced
4. Verify no dead code warnings from clippy
5. Verify no unused imports

#### File size check

Verify all new/modified files stay within the 500-line guideline:

| File | Target |
|------|--------|
| `performance/mod.rs` | < 300 lines |
| `performance/frame_chart.rs` | < 500 lines |
| `performance/memory_chart.rs` | < 500 lines |
| `performance/styles.rs` | < 200 lines |
| `handler/devtools/performance.rs` | < 300 lines |
| `core/performance.rs` (total) | < 500 lines |
| `session/performance.rs` | < 300 lines |

### Acceptance Criteria

1. `cargo fmt --all` — no formatting changes needed
2. `cargo check --workspace` — clean compilation
3. `cargo test --workspace` — all tests pass (0 failures)
4. `cargo clippy --workspace -- -D warnings` — no warnings
5. 30+ new tests added across Phase 3 (target: 60+)
6. No files exceed 500-line guideline
7. `stats_section.rs` removed
8. Old `frame_section.rs` and `memory_section.rs` removed
9. All Phase 3 success criteria from PLAN.md are met
10. No dead code or unused import warnings

### Testing

This task IS the testing pass. Run:

```bash
# Full quality gate
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings

# Verify per-crate test counts
cargo test -p fdemon-core 2>&1 | grep "test result"
cargo test -p fdemon-daemon 2>&1 | grep "test result"
cargo test -p fdemon-app 2>&1 | grep "test result"
cargo test -p fdemon-tui 2>&1 | grep "test result"
```

### Notes

- **This task should be fast**: If Tasks 01-08 were done correctly, this is a 15-minute verification pass. If issues are found, fix them here rather than going back to earlier tasks.
- **Test count baseline**: Before Phase 3, the project has ~1,532 unit tests. After Phase 3, expect ~1,590-1,610+.
- **Clippy may flag new patterns**: The braille canvas and bar chart rendering may use patterns that trigger clippy lints (e.g., `as` casts for u16/u64 conversions). Fix with appropriate `#[allow(...)]` annotations or proper conversion methods.
- **No documentation updates in this task**: Documentation updates (KEYBINDINGS.md, ARCHITECTURE.md) are deferred to the overall Phase 5 of the DevTools V2 plan.

---

## Completion Summary

**Status:** Not started
