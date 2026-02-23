## Task: Minor Quality Fixes (Batched)

**Objective**: Address 3 minor code quality issues found during Phase 5 review: module visibility alignment, asymmetric sort helper, and manual cell-loop background clearing.

**Depends on**: Tasks 01-05 (Wave 1)

**Severity**: LOW — Code quality and consistency

### Scope

- `crates/fdemon-app/src/session/mod.rs:7`: Module visibility
- `crates/fdemon-core/src/performance.rs:176-184`: Add `top_by_instances()` method
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs:77-84`: Use new helper
- `crates/fdemon-tui/src/widgets/devtools/mod.rs:66-73`: Replace manual cell loops
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs:123-130`: Replace manual cell loops

### Details

#### Fix A: Align Module Visibility

**File:** `crates/fdemon-app/src/session/mod.rs:7-8`

Currently:
```rust
pub mod network;           // fully public
pub(crate) mod performance; // crate-private
```

Both modules export types via `pub use` re-exports at the `session::` level. Consumers should use the re-exports, not direct module paths. Align both to `pub(crate)`:

```rust
pub(crate) mod network;
pub(crate) mod performance;
```

**Verification:** Run `cargo check -p fdemon-tui` to confirm no downstream code imports via `fdemon_app::session::network::*` directly.

---

#### Fix B: Add `top_by_instances()` to `AllocationProfile`

**File:** `crates/fdemon-core/src/performance.rs` — after `top_by_size()` (line ~184)

Currently `BySize` uses the efficient `profile.top_by_size(limit)` helper, but `ByInstances` inlines `sort_by_key + truncate` in the TUI renderer. Add the symmetric method:

```rust
/// Return classes sorted by total instance count (descending).
pub fn top_by_instances(&self, limit: usize) -> Vec<&ClassHeapStats> {
    let mut sorted: Vec<_> = self.members.iter().collect();
    sorted.sort_by_key(|s| std::cmp::Reverse(s.total_instances()));
    sorted.truncate(limit);
    sorted
}
```

Then simplify `table.rs:79-84`:
```rust
AllocationSortColumn::ByInstances => profile.top_by_instances(MAX_TABLE_ROWS),
```

---

#### Fix C: Replace Manual Cell-Loop Background Clears

**Files:**
- `crates/fdemon-tui/src/widgets/devtools/mod.rs:66-73`
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs:123-130`

Both have:
```rust
let bg_style = Style::default().bg(palette::DEEPEST_BG);
for y in area.y..area.bottom() {
    for x in area.x..area.right() {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_style(bg_style).set_char(' ');
        }
    }
}
```

Replace with ratatui's optimized buffer API:
```rust
let bg_style = Style::default().bg(palette::DEEPEST_BG);
buf.set_style(area, bg_style);
```

Note: `buf.set_style` sets the style but does not clear the char to `' '`. If the background clear also needs to reset content, use `Block::new().style(bg_style).render(area, buf)` which fills with spaces. Check the Network panel's approach for the exact pattern used there.

### Acceptance Criteria

1. Both `network` and `performance` session sub-modules are `pub(crate)`
2. `AllocationProfile::top_by_instances()` exists with unit test
3. Table renderer uses `top_by_instances()` — no inline sort logic
4. No manual `for y / for x / cell_mut` background loops in devtools widgets
5. `cargo test --workspace` passes
6. `cargo clippy --workspace` passes

### Testing

- Fix A: Compilation is the test — if `fdemon-tui` compiles with `pub(crate) mod network`, no one was importing via the module path
- Fix B: Add `test_top_by_instances_returns_sorted` in `fdemon-core/src/performance.rs` tests, mirroring the existing `test_top_by_size` test
- Fix C: Existing devtools render tests cover the rendering — the visual output should be identical

### Notes

- Fix A may require checking if any items in `fdemon-tui` import from `fdemon_app::session::network::` directly vs using the re-exports. If they do, update the imports to use the re-exported path.
- Fix C: verify whether `buf.set_style` alone is sufficient or if `Block::new().style(bg_style).render()` is needed. The Network panel at `network/mod.rs` uses the `Block` approach.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/mod.rs` | Changed `pub mod network` to `pub(crate) mod network` to match `performance` visibility |
| `crates/fdemon-core/src/performance.rs` | Added `top_by_instances()` method to `AllocationProfile`; added `test_top_by_size_returns_sorted`, `test_top_by_instances_returns_sorted`, `test_top_by_instances_limit_larger_than_members` tests |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs` | Replaced inline sort/truncate for `ByInstances` with `profile.top_by_instances(MAX_TABLE_ROWS)` |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Replaced manual `for y/for x/cell_mut` background loop with `Block::new().style(bg_style).render(area, buf)`; added `Block` to ratatui widget imports |
| `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` | Replaced manual `for y/for x/cell_mut` background loop with `Block::new().style(bg_style).render(area, buf)` |

### Notable Decisions/Tradeoffs

1. **Block::new() instead of buf.set_style()**: Used `Block::new().style(bg_style).render()` rather than `buf.set_style(area, bg_style)` to match the Network panel pattern and to ensure cell characters are also reset to `' '`, matching the original loop behavior exactly.
2. **Added test_top_by_size_returns_sorted**: The task referenced a "mirroring" test but `test_top_by_size` didn't exist yet. Added it alongside the new `test_top_by_instances_returns_sorted` to satisfy the symmetry intent and provide coverage for both methods.
3. **Helper function in tests**: Extracted `make_class()` helper in the test module to avoid struct literal repetition across the three new `AllocationProfile` tests.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (1047 fdemon-app + 360 fdemon-core + 375 fdemon-daemon + 757 fdemon-tui + all integration tests)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None**: All three fixes are pure internal improvements — no API surface changes, no behavioral changes to the rendered output.
