## Task: Wire Allocation Sort Interaction

**Objective**: Wire the existing `AllocationSortColumn` enum and `PerformanceState.allocation_sort` field to an actual key binding (`s` in Performance panel) that toggles the allocation table sort order between "by size" and "by instances". Remove the `#[allow(dead_code)]` annotations. Update the allocation table widget to read and use the sort column from state.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/session/performance.rs`: MODIFIED — Remove `#[allow(dead_code)]`, make `AllocationSortColumn` and `allocation_sort` `pub`
- `crates/fdemon-app/src/message.rs`: MODIFIED — Add `ToggleAllocationSort` message
- `crates/fdemon-app/src/handler/devtools/performance.rs`: MODIFIED — Add `handle_toggle_allocation_sort()`
- `crates/fdemon-app/src/handler/keys.rs`: MODIFIED — Add `s` key binding in Performance panel context
- `crates/fdemon-app/src/handler/update.rs`: MODIFIED — Wire `ToggleAllocationSort` to handler
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs`: MODIFIED — Accept and use sort column from state

### Details

#### 1. Remove dead code annotations (`session/performance.rs`)

Remove the two `#[allow(dead_code)]` annotations and the TODO comments on `AllocationSortColumn` and `allocation_sort`:

```rust
// Before:
#[allow(dead_code)] // TODO: wire to allocation table sort interaction
pub(crate) enum AllocationSortColumn { ... }

// After:
pub enum AllocationSortColumn { ... }
```

Change `pub(crate)` to `pub` on both the enum and the field so the TUI crate can access the sort column for rendering.

#### 2. Add `ToggleAllocationSort` message (`message.rs`)

Add a new message variant:

```rust
/// Toggle the allocation table sort column (Size ↔ Instances).
ToggleAllocationSort,
```

#### 3. Implement handler (`handler/devtools/performance.rs`)

Add a handler function:

```rust
/// Toggle the allocation table sort between BySize and ByInstances.
pub(crate) fn handle_toggle_allocation_sort(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.performance.allocation_sort = match handle.session.performance.allocation_sort {
            AllocationSortColumn::BySize => AllocationSortColumn::ByInstances,
            AllocationSortColumn::ByInstances => AllocationSortColumn::BySize,
        };
    }
    UpdateResult::none()
}
```

#### 4. Wire in `update.rs`

Add the match arm for `Message::ToggleAllocationSort`:

```rust
Message::ToggleAllocationSort => {
    devtools::performance::handle_toggle_allocation_sort(state)
}
```

#### 5. Add key binding (`handler/keys.rs`)

In the `handle_key_devtools()` function, add `s` when the Performance panel is active:

```rust
// In the Performance panel section:
InputKey::Char('s') if in_performance => Some(Message::ToggleAllocationSort),
```

Verify this does not conflict with other `s` bindings in DevTools mode. Currently `s` in DevTools mode maps to `NetworkSwitchDetailTab(ResponseBody)` only when `in_network` is true, so `s` when `in_performance` is free.

#### 6. Update allocation table widget (`memory_chart/table.rs`)

The `render_allocation_table()` function currently receives `Option<&AllocationProfile>` and sorts unconditionally by `new_size` descending. Update it to also accept the sort column:

```rust
pub(super) fn render_allocation_table(
    allocation_profile: Option<&AllocationProfile>,
    sort_column: AllocationSortColumn,
    area: Rect,
    buf: &mut Buffer,
) {
```

Change the sort logic:

```rust
// Currently:
stats.sort_by(|a, b| b.new_size.cmp(&a.new_size));

// Updated:
match sort_column {
    AllocationSortColumn::BySize => {
        stats.sort_by(|a, b| b.new_size.cmp(&a.new_size));
    }
    AllocationSortColumn::ByInstances => {
        stats.sort_by(|a, b| b.new_count.cmp(&a.new_count));
    }
}
```

Update the table header to indicate the active sort column (e.g., with an arrow indicator):

```rust
// Header shows which column is sorted:
// BySize:      "Class          Instances  Size ▼    Retained"
// ByInstances: "Class          Instances ▼ Size      Retained"
```

Update the caller in `memory_chart/mod.rs` to pass the sort column from `PerformanceState`.

#### 7. Add tests

In `handler/devtools/performance.rs`, add tests for the toggle handler:

```rust
#[test]
fn test_toggle_allocation_sort_size_to_instances() {
    let mut state = make_devtools_state();
    assert_eq!(
        state.session().performance.allocation_sort,
        AllocationSortColumn::BySize
    );
    handle_toggle_allocation_sort(&mut state);
    assert_eq!(
        state.session().performance.allocation_sort,
        AllocationSortColumn::ByInstances
    );
}

#[test]
fn test_toggle_allocation_sort_instances_to_size() {
    let mut state = make_devtools_state();
    state.session_mut().performance.allocation_sort = AllocationSortColumn::ByInstances;
    handle_toggle_allocation_sort(&mut state);
    assert_eq!(
        state.session().performance.allocation_sort,
        AllocationSortColumn::BySize
    );
}
```

In `handler/keys.rs` tests, add a test that `s` in Performance mode produces `ToggleAllocationSort`.

In `memory_chart/table.rs` tests, add tests that verify the table sorts correctly for each column.

### Acceptance Criteria

1. `AllocationSortColumn` enum has no `#[allow(dead_code)]` annotation
2. `PerformanceState.allocation_sort` field has no `#[allow(dead_code)]` annotation
3. Both are `pub` (not `pub(crate)`)
4. `ToggleAllocationSort` message exists and is handled
5. `s` key in Performance panel toggles allocation sort
6. Allocation table renders sorted by the active column
7. Table header shows sort indicator on the active column
8. `s` key does not conflict with other DevTools bindings
9. `cargo test -p fdemon-app -- devtools` passes
10. `cargo test -p fdemon-tui -- allocation` passes
11. `cargo clippy --workspace` clean (no dead_code warnings for these items)

### Testing

```bash
cargo test -p fdemon-app -- toggle_allocation
cargo test -p fdemon-app -- devtools
cargo test -p fdemon-tui -- allocation_table
cargo clippy --workspace
```

### Notes

- **Sort column values**: `BySize` sorts by `new_size` (total bytes allocated since last GC), `ByInstances` sorts by `new_count` (total instances allocated since last GC). Both use descending order — largest first.
- **Sort stability**: Since the allocation profile is replaced entirely on each fetch (not incrementally merged), re-sorting on each render is acceptable. No caching needed.
- **Retained size**: The PLAN.md mentions a "Retained Size" column, but the `ClassHeapStats` struct in `fdemon-core` does not have a retained size field. Do not add one — retained size requires heap snapshots which are expensive. The table shows `new_size`, `new_count`, and `accumulated_size` columns.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/performance.rs` | Removed `#[allow(dead_code)]` from `AllocationSortColumn` and `allocation_sort`; changed both from `pub(crate)` to `pub` |
| `crates/fdemon-app/src/session/mod.rs` | Added `AllocationSortColumn` to public re-exports alongside `PerformanceState` |
| `crates/fdemon-app/src/message.rs` | Added `ToggleAllocationSort` variant to `Message` enum |
| `crates/fdemon-app/src/handler/devtools/performance.rs` | Added `handle_toggle_allocation_sort()` function; added 4 tests |
| `crates/fdemon-app/src/handler/update.rs` | Added match arm for `Message::ToggleAllocationSort` |
| `crates/fdemon-app/src/handler/keys.rs` | Added `InputKey::Char('s') if in_performance => Some(Message::ToggleAllocationSort)` binding; added 3 key tests |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs` | Updated `render_allocation_table()` to accept `sort_column: AllocationSortColumn`; added sort indicator in header; added conditional sort logic |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/mod.rs` | Added `allocation_sort: AllocationSortColumn` field to `MemoryChart`; updated `new()` signature; passed sort to `render_allocation_table()` |
| `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | Updated `MemoryChart::new()` call to pass `self.performance.allocation_sort` |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/tests.rs` | Updated all `MemoryChart::new()` and `render_allocation_table()` call sites; added 4 new sort tests |

### Notable Decisions/Tradeoffs

1. **Re-export path for `AllocationSortColumn`**: The enum was `pub(crate)` inside `session/performance.rs`, which is a `pub(crate)` submodule. Making the enum `pub` and re-exporting it through `session/mod.rs` as `pub use performance::{AllocationSortColumn, PerformanceState}` gives `fdemon-tui` clean access via `fdemon_app::session::AllocationSortColumn` without exposing internal module structure.

2. **Sort uses `total_instances()` / `total_size()`**: The task notes mention `new_count`/`new_size`, but the actual `AllocationProfile`/`ClassHeapStats` API in `fdemon-core` exposes `total_instances()` and `total_size()` methods (and `top_by_size()`). The implementation matches the existing `table.rs` patterns which already used these methods.

3. **No re-export of `handle_toggle_allocation_sort` in `devtools/mod.rs`**: `update.rs` calls the function via the full path `devtools::performance::handle_toggle_allocation_sort(state)`, so no additional re-export was needed. Adding one would have produced an unused import warning.

4. **`s` key conflict check**: The Network panel uses `s` under the `in_network` guard for `NetworkSwitchDetailTab(ResponseBody)`. Adding `s` under `in_performance` is orthogonal — guards are mutually exclusive because only one panel is active at a time.

### Testing Performed

- `cargo clippy --workspace` - Passed (no warnings)
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app -- devtools` - Passed (118 tests)
- `cargo test -p fdemon-tui -- allocation` - Passed (9 tests)
- `cargo test --workspace --lib` - Passed (748 tests)

### Risks/Limitations

1. **E2E test suite**: 25 pre-existing failures in the integration test suite are unrelated to these changes (confirmed by checking test names and failure messages). All 748 unit tests pass.
